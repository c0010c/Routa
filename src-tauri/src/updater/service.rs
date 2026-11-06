//! Update state machine: background check, download into the cache, and
//! "update-status" events for the UI. Installing lives in `install.rs`.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tauri_plugin_updater::UpdaterExt;

use super::cache::{verify, Cache, Pending};
use super::{download, is_nsis_install, updater_pubkey};

const FIRST_CHECK_DELAY: Duration = Duration::from_secs(10);
const CHECK_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);
/// Download progress events are throttled to this interval.
const PROGRESS_EVENT_INTERVAL: Duration = Duration::from_millis(250);

pub const STATUS_EVENT: &str = "update-status";

#[derive(Serialize, Clone, Debug, PartialEq, specta::Type)]
#[serde(tag = "kind")]
pub enum UpdateState {
    /// Not an NSIS install (dev build or unpacked exe): no updates.
    Unsupported,
    Idle,
    Checking,
    Downloading {
        version: String,
        received: u64,
        total: Option<u64>,
    },
    /// A verified installer for `version` is in the cache.
    Ready {
        version: String,
        notes: Option<String>,
    },
    Failed {
        message: String,
    },
}

#[derive(Serialize, Clone, Debug, PartialEq, specta::Type)]
pub struct UpdateView {
    pub current_version: String,
    pub state: UpdateState,
    pub last_checked_unix: Option<i64>,
}

struct Status {
    state: UpdateState,
    last_checked_unix: Option<i64>,
}

pub struct Updater {
    app: AppHandle,
    data_dir: PathBuf,
    current: semver::Version,
    supported: bool,
    status: Mutex<Status>,
    /// Held for the whole check; a second caller gets the current view.
    checking: tokio::sync::Mutex<()>,
}

fn ready(p: Pending) -> UpdateState {
    UpdateState::Ready {
        version: p.version,
        notes: p.notes,
    }
}

/// State after a check. `outcome` is `Ok(Some)` for a verified installer of
/// the announced version, `Ok(None)` when nothing newer is announced. A valid
/// cached installer (`cached`, newer than the running version) stays
/// installable when the check finds nothing (a stale mirror manifest) or
/// fails; `Failed` only shows when there is nothing to install.
fn post_check_state(outcome: Result<Option<Pending>, String>, cached: Option<Pending>) -> UpdateState {
    match (outcome, cached) {
        (Ok(Some(p)), _) | (_, Some(p)) => ready(p),
        (Ok(None), None) => UpdateState::Idle,
        (Err(message), None) => UpdateState::Failed { message },
    }
}

/// State at startup: a verified cached installer is offered right away.
fn initial_state(supported: bool, cached: Option<Pending>) -> UpdateState {
    match (supported, cached) {
        (false, _) => UpdateState::Unsupported,
        (true, Some(p)) => ready(p),
        (true, None) => UpdateState::Idle,
    }
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

impl Updater {
    pub fn new(app: AppHandle, data_dir: PathBuf) -> Updater {
        let current = app.package_info().version.clone();
        let supported = is_nsis_install();
        let cached = if supported {
            updater_pubkey(&app).and_then(|pk| Cache::new(&data_dir).load_verified(&pk, &current))
        } else {
            None
        };
        Updater {
            app,
            data_dir,
            current,
            supported,
            status: Mutex::new(Status {
                state: initial_state(supported, cached),
                last_checked_unix: None,
            }),
            checking: tokio::sync::Mutex::new(()),
        }
    }

    pub fn view(&self) -> UpdateView {
        let s = self.status.lock().unwrap();
        UpdateView {
            current_version: self.current.to_string(),
            state: s.state.clone(),
            last_checked_unix: s.last_checked_unix,
        }
    }

    fn set(&self, state: UpdateState) {
        self.status.lock().unwrap().state = state;
        self.emit();
    }

    fn emit(&self) {
        if let Err(e) = self.app.emit(STATUS_EVENT, self.view()) {
            eprintln!("emit {STATUS_EVENT} failed: {e}");
        }
    }

    pub async fn check_now(&self) -> UpdateView {
        if !self.supported {
            return self.view();
        }
        let Ok(_guard) = self.checking.try_lock() else {
            return self.view();
        };
        self.set(UpdateState::Checking);
        let outcome = self.run_check().await;
        if let Err(message) = &outcome {
            eprintln!("updater: {message}");
        }
        let cached = if matches!(outcome, Ok(Some(_))) {
            None
        } else {
            self.load_cached().await
        };
        self.set(post_check_state(outcome, cached));
        self.view()
    }

    /// The verified cached installer, if any. Entries not newer than the
    /// running version or failing verification are removed.
    async fn load_cached(&self) -> Option<Pending> {
        let pubkey = updater_pubkey(&self.app)?;
        let dir = self.data_dir.clone();
        let current = self.current.clone();
        tokio::task::spawn_blocking(move || Cache::new(&dir).load_verified(&pubkey, &current))
            .await
            .ok()
            .flatten()
    }

    /// Shows an install failure (e.g. at startup) until the next check.
    pub fn report_failure(&self, message: String) {
        eprintln!("updater: {message}");
        self.set(UpdateState::Failed { message });
    }

    /// Checks again that a verified update is ready and returns the cached
    /// installer path for `install::launch` (which verifies once more).
    pub async fn prepare_install(&self) -> Result<PathBuf, String> {
        if !matches!(self.view().state, UpdateState::Ready { .. }) {
            return Err("没有可安装的更新".into());
        }
        if self.load_cached().await.is_none() {
            let message = "更新安装包校验失败，已删除，请重新检查更新".to_string();
            self.set(UpdateState::Failed {
                message: message.clone(),
            });
            return Err(message);
        }
        Ok(Cache::new(&self.data_dir).installer_path())
    }

    /// One check: `Ok(Some)` with a verified installer of the announced
    /// version in the cache, `Ok(None)` when nothing newer is announced.
    /// Never removes a valid cached installer.
    async fn run_check(&self) -> Result<Option<Pending>, String> {
        let pubkey = updater_pubkey(&self.app).ok_or("缺少更新签名公钥")?;
        let checker = self
            .app
            .updater()
            .map_err(|e| format!("无法初始化更新检查：{e}"))?;
        let update = checker
            .check()
            .await
            .map_err(|e| format!("检查更新失败：{e}"))?;
        self.status.lock().unwrap().last_checked_unix = Some(now_unix());

        // Nothing newer announced. A cached installer is kept: a stale mirror
        // manifest must not delete a valid newer one (`load_cached` drops
        // entries that are not newer than the running version).
        let Some(update) = update else {
            return Ok(None);
        };

        if let Some(p) = self.load_cached().await {
            if p.version == update.version {
                return Ok(Some(p));
            }
        }

        let version = update.version.clone();
        self.set(UpdateState::Downloading {
            version: version.clone(),
            received: 0,
            total: None,
        });
        let last_emit = Mutex::new(None::<Instant>);
        let bytes = download::fetch(&update, |received, total| {
            {
                let mut s = self.status.lock().unwrap();
                s.state = UpdateState::Downloading {
                    version: version.clone(),
                    received,
                    total,
                };
            }
            let mut last = last_emit.lock().unwrap();
            let due = last.is_none_or(|t| t.elapsed() >= PROGRESS_EVENT_INTERVAL)
                || total == Some(received);
            if due {
                *last = Some(Instant::now());
                drop(last);
                self.emit();
            }
        })
        .await
        .map_err(|e| format!("下载更新失败：{e}"))?;

        // The plugin checked the signature; this also pins it to the version,
        // which is what `load_verified` will require later.
        verify(&bytes, &update.signature, &pubkey, &version)
            .map_err(|e| format!("更新安装包校验失败：{e}"))?;
        let pending = Pending {
            version: version.clone(),
            signature: update.signature.clone(),
            notes: update.body.clone(),
        };
        let dir = self.data_dir.clone();
        let to_write = pending.clone();
        tokio::task::spawn_blocking(move || Cache::new(&dir).write(&bytes, &to_write))
            .await
            .map_err(|e| format!("保存更新安装包失败：{e}"))?
            .map_err(|e| format!("保存更新安装包失败：{e}"))?;
        Ok(Some(pending))
    }

    pub fn spawn_background(self: Arc<Self>) {
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(FIRST_CHECK_DELAY).await;
            loop {
                self.check_now().await;
                tokio::time::sleep(CHECK_INTERVAL).await;
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn updater_unsupported_outside_nsis() {
        assert!(!is_nsis_install());
        let pending = Pending {
            version: "0.4.0".into(),
            signature: "s".into(),
            notes: None,
        };
        assert_eq!(
            initial_state(false, Some(pending.clone())),
            UpdateState::Unsupported
        );
        assert_eq!(initial_state(true, None), UpdateState::Idle);
        assert_eq!(
            initial_state(true, Some(pending)),
            UpdateState::Ready {
                version: "0.4.0".into(),
                notes: None
            }
        );
    }

    fn pending(version: &str) -> Pending {
        Pending {
            version: version.into(),
            signature: "s".into(),
            notes: None,
        }
    }

    #[test]
    fn nothing_announced_keeps_a_valid_newer_cache() {
        assert_eq!(
            post_check_state(Ok(None), Some(pending("0.4.0"))),
            UpdateState::Ready {
                version: "0.4.0".into(),
                notes: None
            }
        );
        assert_eq!(post_check_state(Ok(None), None), UpdateState::Idle);
    }

    #[test]
    fn failed_check_keeps_cached_update_installable() {
        assert_eq!(
            post_check_state(Err("下载更新失败".into()), Some(pending("0.4.0"))),
            UpdateState::Ready {
                version: "0.4.0".into(),
                notes: None
            }
        );
        assert_eq!(
            post_check_state(Err("下载更新失败".into()), None),
            UpdateState::Failed {
                message: "下载更新失败".into()
            }
        );
    }

    #[test]
    fn downloaded_update_wins_over_older_cache() {
        assert_eq!(
            post_check_state(Ok(Some(pending("0.5.0"))), Some(pending("0.4.0"))),
            UpdateState::Ready {
                version: "0.5.0".into(),
                notes: None
            }
        );
    }

    #[test]
    fn update_state_serializes_with_kind_tag() {
        let v = |s: UpdateState| serde_json::to_value(s).unwrap();
        assert_eq!(
            v(UpdateState::Ready {
                version: "0.4.0".into(),
                notes: None
            }),
            serde_json::json!({"kind": "Ready", "version": "0.4.0", "notes": null})
        );
        assert_eq!(v(UpdateState::Idle), serde_json::json!({"kind": "Idle"}));
        assert_eq!(
            v(UpdateState::Downloading {
                version: "0.4.0".into(),
                received: 5,
                total: Some(9)
            }),
            serde_json::json!({"kind": "Downloading", "version": "0.4.0", "received": 5, "total": 9})
        );
        assert_eq!(
            v(UpdateState::Failed {
                message: "x".into()
            }),
            serde_json::json!({"kind": "Failed", "message": "x"})
        );
        let view = UpdateView {
            current_version: "0.3.0".into(),
            state: UpdateState::Unsupported,
            last_checked_unix: Some(1),
        };
        assert_eq!(
            serde_json::to_value(view).unwrap(),
            serde_json::json!({"current_version": "0.3.0", "state": {"kind": "Unsupported"}, "last_checked_unix": 1})
        );
    }
}

//! Update state machine: background check, download into the cache, and
//! "update-status" events for the UI. Installing lives elsewhere.

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

/// State at startup: a verified cached installer is offered right away.
fn initial_state(supported: bool, cached: Option<Pending>) -> UpdateState {
    match (supported, cached) {
        (false, _) => UpdateState::Unsupported,
        (true, Some(p)) => UpdateState::Ready {
            version: p.version,
            notes: p.notes,
        },
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
        let state = match self.run_check().await {
            Ok(state) => state,
            Err(message) => {
                eprintln!("updater: {message}");
                UpdateState::Failed { message }
            }
        };
        self.set(state);
        self.view()
    }

    /// One check; any existing valid cache is left alone on error.
    async fn run_check(&self) -> Result<UpdateState, String> {
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

        let cache = Cache::new(&self.data_dir);
        // Not newer than the running version: a cached installer is no longer
        // announced, so drop it to keep the disk in line with `Idle`.
        let Some(update) = update else {
            cache.clear();
            return Ok(UpdateState::Idle);
        };

        if let Some(p) = cache.load_verified(&pubkey, &self.current) {
            if p.version == update.version {
                return Ok(UpdateState::Ready {
                    version: p.version,
                    notes: p.notes,
                });
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
        Ok(UpdateState::Ready {
            version: pending.version,
            notes: pending.notes,
        })
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

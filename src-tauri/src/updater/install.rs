//! Running a cached update. The installer is started only once the
//! process-routing service has confirmed a Goodbye from this Routa (or there
//! is no service to tell), so rules stay on `on_routa_exit` (Graceful /
//! NeverConnected) for the whole upgrade, never `on_fault`.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use semver::Version;
use tauri::AppHandle;

use super::cache::Cache;
use super::service::{UpdateState, Updater};
use super::updater_pubkey;
use crate::procroute::client::{settle_graceful, GoodbyeOutcome, SettleOutcome};

/// What the exit hook should start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallRequest {
    /// Running install: the cached installer (`Cache::installer_path()`); it is
    /// verified and copied at exit, after Goodbye.
    Cached(PathBuf),
    /// Startup install: already verified and copied to `%TEMP%`, and the
    /// service already settled; only the spawn is left.
    Prepared(PathBuf),
}

/// Installer to start once the exit sequence is done. A `Mutex<Option<_>>`
/// rather than a `OnceLock` so the exit hook can take it exactly once.
static PENDING_INSTALL: Mutex<Option<InstallRequest>> = Mutex::new(None);

/// Arguments tauri-plugin-updater passes to an NSIS installer in passive mode.
///
/// Copied from tauri-plugin-updater 2.14.1 (the version in Cargo.lock; 2.13.3
/// is identical), `src/updater.rs` `Updater::updater_parameters`, NSIS branch:
/// `install_mode.nsis_args()` (`src/config.rs`: Passive → `/P`), then `/UPDATE`,
/// then with `restart_after_install` (default `true`)
/// `nsis_restart_after_install_args()` (Passive → `/R`) and `/ARGS` followed by
/// the current process arguments. We pass no process arguments after `/ARGS`
/// so the relaunched Routa is never flagged `--autostart`.
///
/// `/UPDATE` sets `$UpdateMode`, which `nsis/hooks.nsh` relies on to keep the
/// RoutaProcSvc service and its WFP filters during the overwrite install.
pub fn installer_args() -> &'static [&'static str] {
    &["/P", "/UPDATE", "/R", "/ARGS"]
}

pub fn request_install_on_exit(request: InstallRequest) {
    *PENDING_INSTALL.lock().unwrap() = Some(request);
}

pub fn take_install_on_exit() -> Option<InstallRequest> {
    PENDING_INSTALL.lock().unwrap().take()
}

/// Whether the installer may start: the service confirmed our Goodbye, or a
/// fresh Attach + Goodbye settled it, or there is no service to tell.
/// Anything else could leave the service judging the upgrade as a crash.
pub fn may_launch(goodbye: GoodbyeOutcome, settle: Option<&SettleOutcome>) -> bool {
    goodbye == GoodbyeOutcome::Delivered
        || matches!(settle, Some(SettleOutcome::Settled | SettleOutcome::NoService))
}

/// Exit hooks call this after shutdown, with the outcome of their Goodbye.
/// When Goodbye was not confirmed (the sync task never started, or did not
/// get through) the service is settled directly; if that fails too, nothing
/// is launched and the cache is kept for the next start.
pub async fn run_pending_install(app: &AppHandle, goodbye: GoodbyeOutcome) {
    let Some(request) = take_install_on_exit() else { return };
    let result = match request {
        InstallRequest::Prepared(exe) => spawn_installer(&exe),
        InstallRequest::Cached(installer) => {
            let settle = if goodbye == GoodbyeOutcome::Delivered {
                None
            } else {
                Some(settle_graceful(std::process::id()).await)
            };
            if !may_launch(goodbye, settle.as_ref()) {
                eprintln!("updater: service did not confirm Goodbye ({goodbye:?}, {settle:?}); installer not started, cache kept");
                return;
            }
            let app = app.clone();
            tokio::task::spawn_blocking(move || launch(&app, &installer))
                .await
                .unwrap_or_else(|e| Err(io::Error::other(e)))
        }
    };
    if let Err(e) = result {
        // The cache is kept; the next start offers it again.
        eprintln!("updater: starting installer failed: {e}");
    }
}

/// Reads the cached installer once, verifies those bytes (signature, version,
/// newer than `current`) and writes the same bytes to
/// `temp_dir\routa-update-<version>-setup.exe`. Never clears the cache.
pub fn launch_into(
    cache: &Cache,
    pubkey: &str,
    current: &Version,
    temp_dir: &Path,
) -> io::Result<PathBuf> {
    let (pending, bytes) = cache
        .read_verified(pubkey, current)
        .map_err(io::Error::other)?;
    // Parsed again so only semver characters reach the file name.
    let version = Version::parse(&pending.version).map_err(io::Error::other)?;
    let target = temp_dir.join(format!("routa-update-{version}-setup.exe"));
    write_fresh(&target, &bytes)?;
    Ok(target)
}

/// Writes `bytes` into a newly created file (an existing one is replaced, not
/// written through); a partial file is removed on error.
fn write_fresh(target: &Path, bytes: &[u8]) -> io::Result<()> {
    match fs::remove_file(target) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }
    let mut f = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(target)?;
    let written = f.write_all(bytes).and_then(|()| f.sync_all());
    drop(f);
    if written.is_err() {
        let _ = fs::remove_file(target);
    }
    written
}

/// Verifies the cached installer and copies it to `%TEMP%` (see `launch_into`).
fn stage(app: &AppHandle, installer: &Path) -> io::Result<PathBuf> {
    let pubkey = updater_pubkey(app).ok_or_else(|| io::Error::other("缺少更新签名公钥"))?;
    let cache = Cache::containing(installer)
        .ok_or_else(|| io::Error::other("更新安装包路径无效"))?;
    let current = app.package_info().version.clone();
    launch_into(&cache, &pubkey, &current, &std::env::temp_dir())
}

/// Starts a staged installer without waiting for it.
fn spawn_installer(exe: &Path) -> io::Result<()> {
    std::process::Command::new(exe)
        .args(installer_args())
        .spawn()?;
    eprintln!("updater: started installer {}", exe.display());
    Ok(())
}

/// Verifies the installer at `installer` (a `Cache::installer_path()`) again,
/// copies it to `%TEMP%` and starts it without waiting. Call only once
/// `may_launch` holds. On failure the cache is left in place.
pub fn launch(app: &AppHandle, installer: &Path) -> io::Result<()> {
    spawn_installer(&stage(app, installer)?)
}

/// The native "install now?" dialog appears only for a ready update and never
/// at autostart (nobody may be there to answer; the in-app notice covers it).
pub fn should_prompt_at_startup(state: &UpdateState, autostarted: bool) -> bool {
    matches!(state, UpdateState::Ready { .. }) && !autostarted
}

/// Runs before the core, desktop init and the procroute client start. Returns
/// `true` when Routa is exiting to install; the caller must then return
/// without starting anything. Everything that can fail (verify, copy, settle)
/// happens before exiting; on failure startup continues normally and the
/// updater shows `Failed`. Only the final spawn runs after exit.
pub async fn startup_gate(app: &AppHandle, updater: &Updater, autostarted: bool) -> bool {
    // Off the UI thread; the installer about to be launched is staged later,
    // so nothing current is kept here.
    tokio::task::spawn_blocking(|| {
        clean_stale_staged(&std::env::temp_dir(), None);
    });
    let state = updater.view().state;
    if !should_prompt_at_startup(&state, autostarted) {
        return false;
    }
    let UpdateState::Ready { version, .. } = state else {
        return false;
    };
    let yes = tokio::task::spawn_blocking(move || ask_install(&version))
        .await
        .unwrap_or(false);
    if !yes {
        return false;
    }
    let installer = match updater.prepare_install().await {
        Ok(p) => p,
        Err(e) => {
            eprintln!("updater: {e}");
            return false;
        }
    };
    let handle = app.clone();
    let staged = tokio::task::spawn_blocking(move || stage(&handle, &installer))
        .await
        .unwrap_or_else(|e| Err(io::Error::other(e)));
    let exe = match staged {
        Ok(exe) => exe,
        Err(e) => {
            eprintln!("updater: preparing installer failed: {e}");
            updater.report_failure(format!("准备更新安装包失败：{e}"));
            return false;
        }
    };
    // The core has not started: tell the service this is a normal exit so it
    // falls back with Graceful even if the last run crashed.
    let settle = settle_graceful(std::process::id()).await;
    if !may_launch(GoodbyeOutcome::NotSpawned, Some(&settle)) {
        let _ = fs::remove_file(&exe);
        updater.report_failure("无法通知分流服务，已取消本次安装".into());
        return false;
    }
    request_install_on_exit(InstallRequest::Prepared(exe));
    app.exit(0);
    true
}

/// Shown before the installer runs, so a UAC prompt is never a surprise.
const UAC_NOTE: &str = "若进程分流组件有更新，安装时会请求一次管理员权限（UAC）。";

/// Deletes staged `routa-update-*-setup.exe` files in `dir` except `keep`.
/// Best effort: files that cannot be removed (e.g. still running) are skipped.
pub fn clean_stale_staged(dir: &Path, keep: Option<&Path>) -> usize {
    let Ok(rd) = fs::read_dir(dir) else { return 0 };
    let mut n = 0;
    for entry in rd.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if !(name.starts_with("routa-update-") && name.ends_with("-setup.exe")) {
            continue;
        }
        if keep.is_some_and(|k| k == path) {
            continue;
        }
        if fs::remove_file(&path).is_ok() {
            n += 1;
        }
    }
    n
}

#[cfg(windows)]
fn ask_install(version: &str) -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        MessageBoxW, IDYES, MB_ICONINFORMATION, MB_SETFOREGROUND, MB_TOPMOST, MB_YESNO,
    };
    let wide = |s: &str| s.encode_utf16().chain(Some(0)).collect::<Vec<u16>>();
    let title = wide("Routa 更新");
    let text = wide(&format!(
        "新版本 v{version} 已下载，是否现在安装？\n\n安装期间按“Routa 退出时”的分流策略处理。

{UAC_NOTE}"
    ));
    // Topmost and foreground: the main window must not hide it while the core
    // waits for the answer.
    let answer = unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            text.as_ptr(),
            title.as_ptr(),
            MB_YESNO | MB_ICONINFORMATION | MB_SETFOREGROUND | MB_TOPMOST,
        )
    };
    answer == IDYES
}

#[cfg(not(windows))]
fn ask_install(_version: &str) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::updater::cache::test_support::{cache_with, BYTES};
    use crate::updater::UpdateState;
    use std::fs;

    #[test]
    fn startup_prompt_only_when_ready_and_not_autostarted() {
        let ready = UpdateState::Ready { version: "0.4.0".into(), notes: None };
        assert!(should_prompt_at_startup(&ready, false));
        assert!(!should_prompt_at_startup(&ready, true));
        assert!(!should_prompt_at_startup(&UpdateState::Idle, false));
    }

    #[test]
    fn dialog_text_mentions_uac() {
        assert!(UAC_NOTE.contains("UAC"));
    }

    #[test]
    fn clean_stale_staged_keeps_only_the_given_file_and_others() {
        let t = tempfile::tempdir().unwrap();
        for n in ["routa-update-0.3.0-setup.exe", "routa-update-0.4.0-setup.exe", "other.exe", "routa-update-x.txt"] {
            fs::write(t.path().join(n), b"x").unwrap();
        }
        let keep = t.path().join("routa-update-0.4.0-setup.exe");
        assert_eq!(clean_stale_staged(t.path(), Some(&keep)), 1);
        assert!(!t.path().join("routa-update-0.3.0-setup.exe").exists());
        assert!(keep.exists());
        assert!(t.path().join("other.exe").exists());
        assert!(t.path().join("routa-update-x.txt").exists());
        assert_eq!(clean_stale_staged(t.path(), None), 1);
    }

    #[test]
    fn installer_args_mark_update_mode() {
        assert!(installer_args().contains(&"/UPDATE"));
    }

    #[test]
    fn failed_launch_keeps_cache() {
        let (dir, cache, pk) = cache_with("routa_0.4.0_x64-setup.exe", "0.4.0");
        let missing = dir.path().join("no-such-dir");
        assert!(launch_into(&cache, &pk, &v("0.3.0"), &missing).is_err());
        assert!(cache.load_verified(&pk, &v("0.3.0")).is_some());
    }

    #[test]
    fn launch_refuses_tampered_installer() {
        let (_dir, cache, pk) = cache_with("routa_0.4.0_x64-setup.exe", "0.4.0");
        let mut b = fs::read(cache.installer_path()).unwrap();
        b[0] ^= 1;
        fs::write(cache.installer_path(), b).unwrap();
        let temp = tempfile::tempdir().unwrap();
        assert!(launch_into(&cache, &pk, &v("0.3.0"), temp.path()).is_err());
        assert!(fs::read_dir(temp.path()).unwrap().next().is_none());
    }

    #[test]
    fn launch_copies_the_verified_bytes() {
        let (_dir, cache, pk) = cache_with("routa_0.4.0_x64-setup.exe", "0.4.0");
        let temp = tempfile::tempdir().unwrap();
        // A stale file at the target is replaced, not trusted.
        fs::write(temp.path().join("routa-update-0.4.0-setup.exe"), b"old").unwrap();
        let exe = launch_into(&cache, &pk, &v("0.3.0"), temp.path()).unwrap();
        assert_eq!(exe, temp.path().join("routa-update-0.4.0-setup.exe"));
        assert_eq!(fs::read(&exe).unwrap(), BYTES);
        assert!(cache.load_verified(&pk, &v("0.3.0")).is_some());
    }

    #[test]
    fn launch_refuses_installer_not_newer_than_current() {
        let (_dir, cache, pk) = cache_with("routa_0.4.0_x64-setup.exe", "0.4.0");
        let temp = tempfile::tempdir().unwrap();
        assert!(launch_into(&cache, &pk, &v("0.4.0"), temp.path()).is_err());
        assert!(fs::read_dir(temp.path()).unwrap().next().is_none());
    }

    #[test]
    fn install_request_is_taken_once() {
        request_install_on_exit(InstallRequest::Cached(PathBuf::from("x")));
        assert_eq!(take_install_on_exit(), Some(InstallRequest::Cached(PathBuf::from("x"))));
        assert_eq!(take_install_on_exit(), None);
    }

    #[test]
    fn launch_only_after_confirmed_goodbye_or_settle() {
        use GoodbyeOutcome::*;
        let failed = SettleOutcome::Failed("x".into());
        // Confirmed Goodbye on the sync task's session.
        assert!(may_launch(Delivered, None));
        // Not confirmed (e.g. update_install before the client started):
        // only a settled service, or no service at all, allows the launch.
        for bye in [NotSpawned, Failed] {
            assert!(!may_launch(bye, None));
            assert!(may_launch(bye, Some(&SettleOutcome::Settled)));
            assert!(may_launch(bye, Some(&SettleOutcome::NoService)));
            assert!(!may_launch(bye, Some(&failed)));
        }
    }

    fn v(s: &str) -> semver::Version {
        semver::Version::parse(s).unwrap()
    }
}

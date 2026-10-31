//! Running a cached update. The installer is started only after Routa's
//! normal exit sequence has said Goodbye to the process-routing service (or,
//! at startup, after `settle_graceful`), so rules stay on `on_routa_exit`
//! (Graceful / NeverConnected) for the whole upgrade, never `on_fault`.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use semver::Version;
use tauri::AppHandle;

use super::cache::Cache;
use super::service::{UpdateState, Updater};
use super::updater_pubkey;

/// Installer to start once the exit sequence is done. A `Mutex<Option<_>>`
/// rather than a `OnceLock` so the exit hook can take it exactly once.
static PENDING_INSTALL: Mutex<Option<PathBuf>> = Mutex::new(None);

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

pub fn request_install_on_exit(path: PathBuf) {
    *PENDING_INSTALL.lock().unwrap() = Some(path);
}

pub fn take_install_on_exit() -> Option<PathBuf> {
    PENDING_INSTALL.lock().unwrap().take()
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

/// Verifies the installer at `installer` (a `Cache::installer_path()`) again,
/// copies it to `%TEMP%` and starts it without waiting. Call only after the
/// service has been told Goodbye. On failure the cache is left in place.
pub fn launch(app: &AppHandle, installer: &Path) -> io::Result<()> {
    let pubkey = updater_pubkey(app).ok_or_else(|| io::Error::other("缺少更新签名公钥"))?;
    let cache = Cache::containing(installer)
        .ok_or_else(|| io::Error::other("更新安装包路径无效"))?;
    let current = app.package_info().version.clone();
    let exe = launch_into(&cache, &pubkey, &current, &std::env::temp_dir())?;
    std::process::Command::new(&exe)
        .args(installer_args())
        .spawn()?;
    eprintln!("updater: started installer {}", exe.display());
    Ok(())
}

/// The native "install now?" dialog appears only for a ready update and never
/// at autostart (nobody may be there to answer; the in-app notice covers it).
pub fn should_prompt_at_startup(state: &UpdateState, autostarted: bool) -> bool {
    matches!(state, UpdateState::Ready { .. }) && !autostarted
}

/// Runs before the core, desktop init and the procroute client start. Returns
/// `true` when Routa is exiting to install; the caller must then return
/// without starting anything.
pub async fn startup_gate(app: &AppHandle, updater: &Updater, autostarted: bool) -> bool {
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
    // The core has not started: tell the service this is a normal exit so it
    // falls back with Graceful even if the last run crashed.
    crate::procroute::client::settle_graceful(std::process::id()).await;
    request_install_on_exit(installer);
    app.exit(0);
    true
}

#[cfg(windows)]
fn ask_install(version: &str) -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        MessageBoxW, IDYES, MB_ICONINFORMATION, MB_YESNO,
    };
    let wide = |s: &str| s.encode_utf16().chain(Some(0)).collect::<Vec<u16>>();
    let title = wide("Routa 更新");
    let text = wide(&format!(
        "新版本 v{version} 已下载，是否现在安装？\n\n安装期间按“Routa 退出时”的分流策略处理。"
    ));
    let answer = unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            text.as_ptr(),
            title.as_ptr(),
            MB_YESNO | MB_ICONINFORMATION,
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
        request_install_on_exit(PathBuf::from("x"));
        assert_eq!(take_install_on_exit(), Some(PathBuf::from("x")));
        assert_eq!(take_install_on_exit(), None);
    }

    fn v(s: &str) -> semver::Version {
        semver::Version::parse(s).unwrap()
    }
}

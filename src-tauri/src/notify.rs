//! Native toast for monitor notices; clicking it brings the main window back
//! and tells the frontend which node/listener to focus.

use tauri::AppHandle;

use crate::monitor::MonitorNotice;

/// Shows `notice` as a toast without blocking the caller.
#[cfg(windows)]
pub fn show(app: &AppHandle, notice: MonitorNotice) {
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || show_blocking(&app, &notice));
}

#[cfg(not(windows))]
pub fn show(_app: &AppHandle, _notice: MonitorNotice) {}

#[cfg(windows)]
fn show_blocking(app: &AppHandle, notice: &MonitorNotice) {
    use tauri::Emitter;
    use tauri_winrt_notification::Toast;

    let try_show = |app_id: &str| {
        let handle = app.clone();
        let focus = notice.focus.clone();
        Toast::new(app_id)
            .title(&notice.title)
            .text1(&notice.body)
            // Runs on a WinRT thread, only while Routa is alive.
            .on_activated(move |_| {
                crate::tray::show_main_window(&handle);
                if let Err(e) = handle.emit("monitor-focus", focus.clone()) {
                    eprintln!("emit monitor-focus failed: {e}");
                }
                Ok(())
            })
            .show()
    };

    let id = app.config().identifier.clone();
    // Dev builds have no registered AppUserModelID; PowerShell's is the fallback.
    let first = if cfg!(debug_assertions) { Toast::POWERSHELL_APP_ID } else { id.as_str() };
    if let Err(e) = try_show(first) {
        eprintln!("toast with app id {first} failed: {e}");
        if first != Toast::POWERSHELL_APP_ID {
            if let Err(e) = try_show(Toast::POWERSHELL_APP_ID) {
                eprintln!("toast fallback failed: {e}");
            }
        }
    }
}

/// Registers Routa's AppUserModelID for the current user so release toasts
/// show as "Routa" even without an installer shortcut carrying the ID (the
/// portable zip). Windows accepts a toast for an unregistered ID and then
/// shows nothing, so this cannot be left to the error fallback above.
/// Idempotent; failures are logged and never stop startup.
#[cfg(windows)]
pub fn register_app_id(app_id: &str, data_dir: &std::path::Path) {
    let icon = match write_icon(data_dir) {
        Ok(path) => Some(path),
        Err(e) => {
            eprintln!("toast icon: {e}");
            None
        }
    };
    let subkey = format!(r"Software\Classes\AppUserModelId\{app_id}");
    if let Err(e) = write_app_id(&subkey, "Routa", icon.as_deref()) {
        eprintln!("register AppUserModelID {app_id}: {e}");
    }
}

/// The toast icon needs an image file (an .exe is not accepted as IconUri);
/// the app icon is embedded and written next to the app data.
#[cfg(windows)]
fn write_icon(data_dir: &std::path::Path) -> std::io::Result<std::path::PathBuf> {
    const ICON: &[u8] = include_bytes!("../icons/128x128.png");
    let path = data_dir.join("toast-icon.png");
    if std::fs::read(&path).ok().as_deref() != Some(ICON) {
        std::fs::create_dir_all(data_dir)?;
        std::fs::write(&path, ICON)?;
    }
    Ok(path)
}

/// Writes `DisplayName` (and `IconUri` when given) under `HKCU\{subkey}`.
#[cfg(windows)]
fn write_app_id(subkey: &str, name: &str, icon: Option<&std::path::Path>) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::ERROR_SUCCESS;
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegCreateKeyExW, RegSetValueExW, HKEY, HKEY_CURRENT_USER, KEY_SET_VALUE, REG_OPTION_NON_VOLATILE,
        REG_SZ,
    };

    let wide = |s: &std::ffi::OsStr| s.encode_wide().chain(Some(0)).collect::<Vec<u16>>();
    let key_name = wide(subkey.as_ref());
    let mut key: HKEY = std::ptr::null_mut();
    // SAFETY: valid NUL-terminated name and out-pointer; the key is closed below.
    let rc = unsafe {
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            key_name.as_ptr(),
            0,
            std::ptr::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_SET_VALUE,
            std::ptr::null(),
            &mut key,
            std::ptr::null_mut(),
        )
    };
    if rc != ERROR_SUCCESS {
        return Err(format!("RegCreateKeyExW failed: {rc}"));
    }
    let set = |value: &str, data: &std::ffi::OsStr| {
        let value = wide(value.as_ref());
        let data = wide(data);
        // SAFETY: `key` is open; `data` is a NUL-terminated UTF-16 buffer of the given byte length.
        let rc = unsafe {
            RegSetValueExW(key, value.as_ptr(), 0, REG_SZ, data.as_ptr().cast(), (data.len() * 2) as u32)
        };
        if rc == ERROR_SUCCESS { Ok(()) } else { Err(format!("RegSetValueExW failed: {rc}")) }
    };
    let result = set("DisplayName", name.as_ref()).and_then(|()| match icon {
        Some(path) => set("IconUri", path.as_os_str()),
        None => Ok(()),
    });
    // SAFETY: opened above.
    unsafe { RegCloseKey(key) };
    result
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use windows_sys::Win32::System::Registry::{RegDeleteTreeW, RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_SZ};

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }

    fn read(subkey: &str, value: &str) -> Option<String> {
        let (k, v) = (wide(subkey), wide(value));
        let mut buf = [0u16; 512];
        let mut len = (buf.len() * 2) as u32;
        // SAFETY: valid NUL-terminated strings and a buffer of `len` bytes.
        let rc = unsafe {
            RegGetValueW(HKEY_CURRENT_USER, k.as_ptr(), v.as_ptr(), RRF_RT_REG_SZ, std::ptr::null_mut(), buf.as_mut_ptr().cast(), &mut len)
        };
        (rc == 0).then(|| String::from_utf16_lossy(&buf[..(len as usize / 2).saturating_sub(1)]))
    }

    #[test]
    fn app_id_registration_writes_name_and_icon_idempotently() {
        let subkey = format!(r"Software\Classes\AppUserModelId\dev.routa.test-{}", std::process::id());
        let dir = std::env::temp_dir().join(format!("routa-toast-icon-{}", std::process::id()));
        let icon = write_icon(&dir).unwrap();
        assert!(icon.is_file());
        for _ in 0..2 {
            write_app_id(&subkey, "Routa", Some(&icon)).unwrap();
        }
        let got = (read(&subkey, "DisplayName"), read(&subkey, "IconUri"));
        let k = wide(&subkey);
        // SAFETY: valid NUL-terminated name.
        unsafe { RegDeleteTreeW(HKEY_CURRENT_USER, k.as_ptr()) };
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(got, (Some("Routa".into()), Some(icon.display().to_string())));
    }
}

#[cfg(test)]
mod uninstall_tests {
    /// The uninstaller deletes the key `register_app_id` writes; both follow the bundle identifier.
    #[test]
    fn uninstall_hook_removes_the_registered_app_id() {
        let conf: serde_json::Value = serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        let id = conf["identifier"].as_str().unwrap();
        let hooks = include_str!("../nsis/hooks.nsh");
        assert!(hooks.contains(&format!(r#"!define ROUTA_AUMID_KEY "Software\Classes\AppUserModelId\{id}""#)), "hooks.nsh key must match {id}");
        assert!(hooks.contains(r#"DeleteRegKey HKCU "${ROUTA_AUMID_KEY}""#));
    }
}

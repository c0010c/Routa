//! Restarting Routa elevated, for TUN. The elevated copy is started first (so a
//! declined UAC prompt leaves this one running), waits for this process to
//! finish its normal shutdown — otherwise the single-instance plugin would hand
//! it straight back to us — and then turns TUN on.

/// The elevated copy waits for this PID to exit before starting up.
const WAIT_PID_ARG: &str = "--wait-pid=";
/// The elevated copy turns TUN on once the core is running.
pub const CONNECT_TUN_ARG: &str = "--connect-tun";

fn wait_pid(args: impl IntoIterator<Item = String>) -> Option<u32> {
    args.into_iter()
        .find_map(|a| a.strip_prefix(WAIT_PID_ARG).and_then(|p| p.parse().ok()))
}

pub fn connect_tun_requested() -> bool {
    std::env::args().any(|a| a == CONNECT_TUN_ARG)
}

/// Called before the Tauri app is built. Bounded so a stuck old process cannot
/// keep the new one from ever starting.
pub fn wait_for_previous_instance() {
    let Some(pid) = wait_pid(std::env::args()) else { return };
    #[cfg(windows)]
    // SAFETY: the handle is checked for null and closed before returning.
    unsafe {
        use windows_sys::Win32::Foundation::CloseHandle;
        use windows_sys::Win32::System::Threading::{OpenProcess, WaitForSingleObject, PROCESS_SYNCHRONIZE};
        let handle = OpenProcess(PROCESS_SYNCHRONIZE, 0, pid);
        if !handle.is_null() {
            WaitForSingleObject(handle, 30_000);
            CloseHandle(handle);
        }
    }
    #[cfg(not(windows))]
    let _ = pid;
}

fn relaunch_params(pid: u32, connect_tun: bool) -> String {
    let mut params = format!("{WAIT_PID_ARG}{pid}");
    if connect_tun {
        params.push(' ');
        params.push_str(CONNECT_TUN_ARG);
    }
    params
}

/// Starts an elevated copy of this executable. `Err("已取消")` when the user
/// declines the UAC prompt.
#[cfg(windows)]
fn spawn_elevated(connect_tun: bool) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::ERROR_CANCELLED;
    use windows_sys::Win32::UI::Shell::{ShellExecuteExW, SEE_MASK_NOASYNC, SHELLEXECUTEINFOW};
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    let exe = std::env::current_exe().map_err(|e| format!("找不到 Routa 程序路径：{e}"))?;
    let params = relaunch_params(std::process::id(), connect_tun);
    let wide = |s: &std::ffi::OsStr| s.encode_wide().chain(Some(0)).collect::<Vec<u16>>();
    let verb = wide("runas".as_ref());
    let file = wide(exe.as_os_str());
    let params = wide(params.as_ref());
    // SAFETY: the strings are NUL-terminated and outlive the call; no process
    // handle is requested, so there is nothing to close.
    unsafe {
        let mut info: SHELLEXECUTEINFOW = std::mem::zeroed();
        info.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
        info.fMask = SEE_MASK_NOASYNC;
        info.lpVerb = verb.as_ptr();
        info.lpFile = file.as_ptr();
        info.lpParameters = params.as_ptr();
        info.nShow = SW_SHOWNORMAL;
        if ShellExecuteExW(&mut info) == 0 {
            let e = std::io::Error::last_os_error();
            if e.raw_os_error() == Some(ERROR_CANCELLED as i32) {
                return Err("已取消".into());
            }
            return Err(format!("无法以管理员身份重启 Routa：{e}"));
        }
    }
    Ok(())
}

#[cfg(not(windows))]
fn spawn_elevated(_connect_tun: bool) -> Result<(), String> {
    Err("仅支持 Windows".into())
}

/// Starts the elevated copy, then exits through the normal shutdown path.
#[tauri::command]
#[specta::specta]
pub async fn relaunch_as_admin(app: tauri::AppHandle, connect_tun: bool) -> Result<(), String> {
    tokio::task::spawn_blocking(move || spawn_elevated(connect_tun))
        .await
        .map_err(|e| e.to_string())??;
    app.exit(0);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn params_round_trip() {
        assert_eq!(relaunch_params(42, false), "--wait-pid=42");
        assert_eq!(relaunch_params(42, true), "--wait-pid=42 --connect-tun");
        let args = relaunch_params(7, true).split(' ').map(String::from).collect::<Vec<_>>();
        assert_eq!(wait_pid(args.clone()), Some(7));
        assert!(args.iter().any(|a| a == CONNECT_TUN_ARG));
    }

    #[test]
    fn no_wait_pid_without_the_flag() {
        assert_eq!(wait_pid(["routa.exe".to_string(), "--autostart".to_string()]), None);
        assert_eq!(wait_pid(["--wait-pid=x".to_string()]), None);
    }
}

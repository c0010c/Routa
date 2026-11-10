//! 启动 / 置前 AI 应用。经 explorer.exe 启动，使程序以普通用户权限运行（即使 Routa 以管理员运行）。

use serde::Serialize;

use super::catalog::CatalogApp;
use super::detect::LaunchTarget;

#[derive(Serialize, Clone, Copy, Debug, PartialEq, specta::Type)]
pub enum LaunchOutcome {
    Launched,
    Focused,
}

/// 有可见窗口 → Focused；没有 → 需要启动。
pub(crate) fn decide_focus(found_window: bool) -> LaunchOutcome {
    if found_window {
        LaunchOutcome::Focused
    } else {
        LaunchOutcome::Launched
    }
}

#[cfg(windows)]
pub fn launch(t: &LaunchTarget) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    let arg = match t {
        LaunchTarget::Exe(p) => p.as_os_str().to_os_string(),
        LaunchTarget::Aumid(a) => format!("shell:AppsFolder\\{a}").into(),
    };
    std::process::Command::new("explorer.exe")
        .arg(arg)
        .creation_flags(0x0800_0000) // CREATE_NO_WINDOW
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("启动失败: {e}"))
}

#[cfg(not(windows))]
pub fn launch(_t: &LaunchTarget) -> Result<(), String> {
    Err("仅支持 Windows".into())
}

/// 找到属于 pids 的顶层可见窗口并置前；找到返回 true。
#[cfg(windows)]
fn focus_window(pids: &std::collections::HashSet<u32>) -> bool {
    use windows_sys::Win32::Foundation::{HWND, LPARAM};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindow, GetWindowThreadProcessId, IsIconic, IsWindowVisible, SetForegroundWindow,
        ShowWindow, GW_OWNER, SW_RESTORE,
    };

    struct Ctx<'a> {
        pids: &'a std::collections::HashSet<u32>,
        found: HWND,
    }

    unsafe extern "system" fn cb(hwnd: HWND, lparam: LPARAM) -> i32 {
        // SAFETY: lparam 是 focus_window 传入的 &mut Ctx，枚举期间有效。
        let ctx = unsafe { &mut *(lparam as *mut Ctx) };
        unsafe {
            if IsWindowVisible(hwnd) == 0 || !GetWindow(hwnd, GW_OWNER).is_null() {
                return 1;
            }
            let mut pid = 0u32;
            GetWindowThreadProcessId(hwnd, &mut pid);
            if ctx.pids.contains(&pid) {
                ctx.found = hwnd;
                return 0;
            }
        }
        1
    }

    let mut ctx = Ctx { pids, found: std::ptr::null_mut() };
    // SAFETY: ctx 在 EnumWindows 返回前一直存活。
    unsafe {
        EnumWindows(Some(cb), &mut ctx as *mut Ctx as LPARAM);
        if ctx.found.is_null() {
            return false;
        }
        if IsIconic(ctx.found) != 0 {
            ShowWindow(ctx.found, SW_RESTORE);
        }
        SetForegroundWindow(ctx.found);
    }
    true
}

#[cfg(windows)]
pub fn focus_or_launch(app: &CatalogApp, t: &LaunchTarget) -> Result<LaunchOutcome, String> {
    // 不用 running_processes()：它按 exe 路径去重，会丢掉真正持有窗口的那个进程（如 Electron 主进程）。
    let pids = crate::procroute::processes::pids_named(&app.processes);
    let outcome = decide_focus(!pids.is_empty() && focus_window(&pids));
    if outcome == LaunchOutcome::Launched {
        launch(t)?;
    }
    Ok(outcome)
}

#[cfg(not(windows))]
pub fn focus_or_launch(_app: &CatalogApp, _t: &LaunchTarget) -> Result<LaunchOutcome, String> {
    Err("仅支持 Windows".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn focus_falls_back_to_launch() {
        assert_eq!(decide_focus(false), LaunchOutcome::Launched);
        assert_eq!(decide_focus(true), LaunchOutcome::Focused);
    }
}

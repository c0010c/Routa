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

/// 可置前的窗口：可见、无所有者、不是工具窗口（标了 WS_EX_APPWINDOW 的除外）、未被 DWM 隐藏（cloaked）。
/// 只剩托盘图标的应用因此找不到窗口，改为重新启动。
pub(crate) fn is_focus_target(visible: bool, owned: bool, ex_style: u32, cloaked: bool) -> bool {
    const WS_EX_TOOLWINDOW: u32 = 0x0000_0080;
    const WS_EX_APPWINDOW: u32 = 0x0004_0000;
    let tool = ex_style & WS_EX_TOOLWINDOW != 0 && ex_style & WS_EX_APPWINDOW == 0;
    visible && !owned && !tool && !cloaked
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
    use windows_sys::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindow, GetWindowLongW, GetWindowThreadProcessId, IsIconic, IsWindowVisible,
        SetForegroundWindow, ShowWindow, GWL_EXSTYLE, GW_OWNER, SW_RESTORE,
    };

    struct Ctx<'a> {
        pids: &'a std::collections::HashSet<u32>,
        found: HWND,
    }

    unsafe extern "system" fn cb(hwnd: HWND, lparam: LPARAM) -> i32 {
        // SAFETY: lparam 是 focus_window 传入的 &mut Ctx，枚举期间有效。
        let ctx = unsafe { &mut *(lparam as *mut Ctx) };
        unsafe {
            let mut cloaked = 0u32;
            let cloaked = DwmGetWindowAttribute(
                hwnd,
                DWMWA_CLOAKED as _,
                &mut cloaked as *mut u32 as *mut core::ffi::c_void,
                std::mem::size_of::<u32>() as u32,
            ) == 0
                && cloaked != 0;
            let visible = IsWindowVisible(hwnd) != 0;
            let owned = !GetWindow(hwnd, GW_OWNER).is_null();
            if !is_focus_target(visible, owned, GetWindowLongW(hwnd, GWL_EXSTYLE) as u32, cloaked) {
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

    #[test]
    fn tool_and_cloaked_windows_are_not_focus_targets() {
        const TOOL: u32 = 0x80;
        const APP: u32 = 0x4_0000;
        assert!(is_focus_target(true, false, 0, false));
        assert!(!is_focus_target(false, false, 0, false));
        assert!(!is_focus_target(true, true, 0, false));
        assert!(!is_focus_target(true, false, TOOL, false));
        assert!(is_focus_target(true, false, TOOL | APP, false));
        assert!(!is_focus_target(true, false, 0, true));
    }
}

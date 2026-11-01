//! System tray: open window / restart mihomo / exit, plus a monochrome icon
//! that follows the connection state.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use tauri::image::Image;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};

use crate::core::apply::ApplyOutcome;
use crate::core::process::CoreState;
use crate::model::health::{ExitStatus, ListenerStatus};
use crate::procroute::client::ProcRouteStatus;
use crate::runtime::{Runtime, RuntimeStatus};

const TRAY_ID: &str = "main";
/// Re-checks the taskbar theme and the procroute state, which raise no runtime event.
const POLL: Duration = Duration::from_secs(2);

// 32 px: Windows scales the tray image down at 100% / 150% DPI and shows it
// as is at 200%. Sources: designs/routa-icon-final/png/tray-*-32.png.
const ICON_DARK: &[u8] = include_bytes!("../icons/tray/tray-dark.png");
const ICON_LIGHT: &[u8] = include_bytes!("../icons/tray/tray-light.png");
const ICON_CONNECTED: &[u8] = include_bytes!("../icons/tray/tray-connected.png");
const ICON_WARNING: &[u8] = include_bytes!("../icons/tray/tray-warning.png");
const ICON_DISCONNECTED: &[u8] = include_bytes!("../icons/tray/tray-disconnected.png");

/// Color the white parts of the status icons take on a light taskbar
/// (same as tray-light.svg).
const LIGHT_INK: u8 = 0x1A;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrayState {
    /// Before the first start finishes: plain monochrome icon.
    Idle,
    Connected,
    /// The core runs but something configured is not in effect.
    Warning,
    Disconnected,
}

/// Three-state mapping (plus `Idle` for the initial start):
/// - Disconnected: the core is stopped, crashed or recovering from a crash, or
///   mihomo holds no config (last apply closed everything / stopped the core).
/// - Warning: the core runs, but the last apply was rejected (previous config
///   kept), a listener port is taken, an exit is broken (missing node, NIC
///   unavailable), an active listener is not in mihomo's config, a Tailscale
///   exit node failed, or process routing reports apply errors / failed rules.
/// - Connected: the core runs and none of the above.
pub fn tray_state(s: &RuntimeStatus, pr: &ProcRouteStatus) -> TrayState {
    match s.core {
        CoreState::Running => {}
        CoreState::Starting if !s.core_recovering => return TrayState::Idle,
        _ => return TrayState::Disconnected,
    }
    match &s.last_apply {
        Some(ApplyOutcome::FailedClosed { .. } | ApplyOutcome::FailedCoreStopped { .. } | ApplyOutcome::NotRunning { .. }) => {
            return TrayState::Disconnected
        }
        Some(ApplyOutcome::FailedKeptPrevious { .. }) => return TrayState::Warning,
        _ => {}
    }
    let exit_broken = s.health.exits.values().any(|h| h.status != ExitStatus::Ok);
    let listener_broken = s.health.listeners.iter().any(|(id, st)| match st {
        ListenerStatus::Active => !s.effective_listeners.contains(id),
        ListenerStatus::DisabledByUser => false,
        ListenerStatus::NoExit | ListenerStatus::ExitBroken { .. } => true,
    });
    let procroute_broken = !pr.apply_errors.is_empty() || pr.actions.values().any(|r| r.action == "Failed");
    if exit_broken
        || listener_broken
        || procroute_broken
        || !s.failed_listeners.is_empty()
        || !s.tailscale_exit_errors.is_empty()
    {
        TrayState::Warning
    } else {
        TrayState::Connected
    }
}

/// Whether Windows draws the taskbar light (`SystemUsesLightTheme` = 1).
/// Missing value (older Windows) or other platforms: dark, the Windows default.
#[cfg(windows)]
fn taskbar_is_light() -> bool {
    use windows_sys::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD};
    let key: Vec<u16> = "Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize\0".encode_utf16().collect();
    let value: Vec<u16> = "SystemUsesLightTheme\0".encode_utf16().collect();
    let mut data: u32 = 0;
    let mut size = std::mem::size_of::<u32>() as u32;
    // SAFETY: NUL-terminated UTF-16 strings; `data`/`size` describe a valid u32 buffer.
    let rc = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            key.as_ptr(),
            value.as_ptr(),
            RRF_RT_REG_DWORD,
            std::ptr::null_mut(),
            (&mut data as *mut u32).cast(),
            &mut size,
        )
    };
    rc == 0 && data == 1
}

#[cfg(not(windows))]
fn taskbar_is_light() -> bool {
    false
}

/// Turns the white parts of a status icon dark for a light taskbar; colored
/// node and the grey disconnected icon stay as they are.
fn recolor_white(rgba: &mut [u8]) {
    for px in rgba.chunks_exact_mut(4) {
        if px[0] >= 0xF0 && px[1] >= 0xF0 && px[2] >= 0xF0 {
            px[..3].fill(LIGHT_INK);
        }
    }
}

fn icon_for(state: TrayState, light: bool) -> tauri::Result<Image<'static>> {
    let bytes = match (state, light) {
        (TrayState::Idle, false) => ICON_DARK,
        (TrayState::Idle, true) => ICON_LIGHT,
        (TrayState::Connected, _) => ICON_CONNECTED,
        (TrayState::Warning, _) => ICON_WARNING,
        (TrayState::Disconnected, _) => ICON_DISCONNECTED,
    };
    let img = Image::from_bytes(bytes)?;
    if !light || matches!(state, TrayState::Idle | TrayState::Disconnected) {
        return Ok(img);
    }
    let mut rgba = img.rgba().to_vec();
    recolor_white(&mut rgba);
    Ok(Image::new_owned(rgba, img.width(), img.height()))
}

/// Icon currently shown, so unchanged polls do not touch the tray.
static SHOWN: Mutex<Option<(TrayState, bool)>> = Mutex::new(None);

/// Recomputes the tray icon from the runtime / procroute state and the
/// taskbar theme; no-op when nothing changed or the tray is not built yet.
pub fn refresh(app: &AppHandle) {
    let Some(tray) = app.tray_by_id(TRAY_ID) else { return };
    let Some(rt) = app.try_state::<Arc<Runtime>>() else { return };
    let key = (tray_state(&rt.status(), &crate::procroute::client::status()), taskbar_is_light());
    let mut shown = SHOWN.lock().unwrap_or_else(|e| e.into_inner());
    if *shown == Some(key) {
        return;
    }
    match icon_for(key.0, key.1).and_then(|icon| tray.set_icon(Some(icon))) {
        Ok(()) => *shown = Some(key),
        Err(e) => eprintln!("set tray icon failed: {e}"),
    }
}

pub fn show_main_window(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

pub fn setup(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "打开主窗口", true, None::<&str>)?;
    let restart = MenuItem::with_id(app, "restart", "重启 mihomo", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &restart, &quit])?;

    let light = taskbar_is_light();
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon_for(TrayState::Idle, light)?)
        .tooltip("Routa")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show_main_window(app),
            "restart" => {
                let rt = Arc::clone(&*app.state::<Arc<Runtime>>());
                tauri::async_runtime::spawn(async move { rt.restart_core().await });
            }
            // Goes through RunEvent::ExitRequested, which runs the shutdown.
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                show_main_window(tray.app_handle());
            }
        })
        .build(app)?;
    *SHOWN.lock().unwrap_or_else(|e| e.into_inner()) = Some((TrayState::Idle, light));

    // Runtime status changes refresh immediately (see lib.rs); this catches
    // theme switches and procroute state.
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut tick = tokio::time::interval(POLL);
        loop {
            tick.tick().await;
            refresh(&handle);
        }
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::core::apply::FailedListener;
    use crate::model::health::Health;
    use crate::model::ListenerId;
    use crate::procroute::client::{ComponentState, RuleView};
    use crate::stats::delay::DelayBook;

    fn running() -> RuntimeStatus {
        RuntimeStatus {
            core: CoreState::Running,
            last_apply: Some(ApplyOutcome::Reloaded { cut: 0, uncut: 0, failed_listeners: vec![] }),
            health: Health { exits: HashMap::new(), listeners: HashMap::new() },
            delays: DelayBook::default(),
            traffic: None,
            interfaces: vec![],
            failed_listeners: vec![],
            effective_listeners: vec![],
            startup_error: None,
            save_error: None,
            tailscale: HashMap::new(),
            tailscale_exit_errors: HashMap::new(),
            tailscale_exit_retrying: HashMap::new(),
            core_stop_reason: None,
            core_recovering: false,
        }
    }

    fn pr() -> ProcRouteStatus {
        ProcRouteStatus {
            component: ComponentState::NotInstalled,
            actions: HashMap::new(),
            last_interruption: None,
            conflicts: vec![],
            apply_errors: vec![],
            truncated_flows: false,
        }
    }

    fn lid(s: &str) -> ListenerId {
        serde_json::from_value(serde_json::json!(s)).unwrap()
    }

    #[test]
    fn core_state_maps_to_idle_or_disconnected() {
        let mut s = running();
        assert_eq!(tray_state(&s, &pr()), TrayState::Connected);
        s.core = CoreState::Starting;
        assert_eq!(tray_state(&s, &pr()), TrayState::Idle);
        s.core_recovering = true;
        assert_eq!(tray_state(&s, &pr()), TrayState::Disconnected);
        s.core = CoreState::Stopped;
        assert_eq!(tray_state(&s, &pr()), TrayState::Disconnected);
        s.core = CoreState::Crashed { exit_code: Some(1) };
        assert_eq!(tray_state(&s, &pr()), TrayState::Disconnected);
    }

    #[test]
    fn apply_outcome_maps() {
        let mut s = running();
        s.last_apply = Some(ApplyOutcome::FailedClosed { error: "x".into(), uncut: 0 });
        assert_eq!(tray_state(&s, &pr()), TrayState::Disconnected);
        s.last_apply = Some(ApplyOutcome::FailedKeptPrevious { error: "x".into() });
        assert_eq!(tray_state(&s, &pr()), TrayState::Warning);
        s.last_apply = None;
        assert_eq!(tray_state(&s, &pr()), TrayState::Connected);
    }

    #[test]
    fn listener_problems_warn() {
        let mut s = running();
        let id = lid("l1");
        s.health.listeners.insert(id.clone(), ListenerStatus::Active);
        assert_eq!(tray_state(&s, &pr()), TrayState::Warning, "active but not in mihomo's config");
        s.effective_listeners.push(id.clone());
        assert_eq!(tray_state(&s, &pr()), TrayState::Connected);
        s.failed_listeners.push(FailedListener { id: id.clone(), name: "l1".into(), port: 7890 });
        assert_eq!(tray_state(&s, &pr()), TrayState::Warning);
        s.failed_listeners.clear();
        s.health.listeners.insert(id, ListenerStatus::DisabledByUser);
        assert_eq!(tray_state(&s, &pr()), TrayState::Connected);
        s.tailscale_exit_errors.insert("手动/ts".into(), "bad".into());
        assert_eq!(tray_state(&s, &pr()), TrayState::Warning);
    }

    #[test]
    fn procroute_failures_warn() {
        let s = running();
        let mut p = pr();
        p.apply_errors.push("boom".into());
        assert_eq!(tray_state(&s, &p), TrayState::Warning);
        let mut p = pr();
        let rule = RuleView { action: "Failed".into(), reason: None, up_bps: 0, down_bps: 0 };
        p.actions.insert(serde_json::from_value(serde_json::json!("r1")).unwrap(), rule);
        assert_eq!(tray_state(&s, &p), TrayState::Warning);
    }

    #[test]
    fn icons_decode_and_recolor() {
        for state in [TrayState::Idle, TrayState::Connected, TrayState::Warning, TrayState::Disconnected] {
            for light in [false, true] {
                let img = icon_for(state, light).unwrap();
                assert_eq!((img.width(), img.height()), (32, 32));
            }
        }
        let mut px = [0xFF, 0xFF, 0xFF, 0x80, 0x4A, 0xDE, 0x80, 0xFF];
        recolor_white(&mut px);
        assert_eq!(px, [LIGHT_INK, LIGHT_INK, LIGHT_INK, 0x80, 0x4A, 0xDE, 0x80, 0xFF]);
    }
}

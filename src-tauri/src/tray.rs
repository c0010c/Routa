//! System tray: open window / restart mihomo / exit, plus an icon that
//! follows the connection state.

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
/// Re-checks the procroute state, which raises no runtime event.
const POLL: Duration = Duration::from_secs(2);

// 32 px: Windows scales the tray image down at 100% / 150% DPI and shows it
// as is at 200%. Sources: designs/routa-icon-final/png/tray-*-32.png.
const ICON_IDLE: &[u8] = include_bytes!("../icons/tray/tray-idle.png");
const ICON_CONNECTED: &[u8] = include_bytes!("../icons/tray/tray-connected.png");
const ICON_WARNING: &[u8] = include_bytes!("../icons/tray/tray-warning.png");
const ICON_DISCONNECTED: &[u8] = include_bytes!("../icons/tray/tray-disconnected.png");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrayState {
    /// Before the first start finishes: grey tile without a slash.
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

fn icon_for(state: TrayState) -> tauri::Result<Image<'static>> {
    Image::from_bytes(match state {
        TrayState::Idle => ICON_IDLE,
        TrayState::Connected => ICON_CONNECTED,
        TrayState::Warning => ICON_WARNING,
        TrayState::Disconnected => ICON_DISCONNECTED,
    })
}

/// Icon currently shown, so unchanged polls do not touch the tray.
static SHOWN: Mutex<Option<TrayState>> = Mutex::new(None);

/// Recomputes the tray icon from the runtime / procroute state; no-op when nothing changed or the tray is not built yet.
pub fn refresh(app: &AppHandle) {
    let Some(tray) = app.tray_by_id(TRAY_ID) else { return };
    let Some(rt) = app.try_state::<Arc<Runtime>>() else { return };
    let state = tray_state(&rt.status(), &crate::procroute::client::status());
    let mut shown = SHOWN.lock().unwrap_or_else(|e| e.into_inner());
    if *shown == Some(state) {
        return;
    }
    match icon_for(state).and_then(|icon| tray.set_icon(Some(icon))) {
        Ok(()) => *shown = Some(state),
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

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon_for(TrayState::Idle)?)
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
    *SHOWN.lock().unwrap_or_else(|e| e.into_inner()) = Some(TrayState::Idle);

    // Runtime status changes refresh immediately (see lib.rs); this catches
    // procroute state.
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
    fn icons_decode() {
        for state in [TrayState::Idle, TrayState::Connected, TrayState::Warning, TrayState::Disconnected] {
            let img = icon_for(state).unwrap();
            assert_eq!((img.width(), img.height()), (32, 32));
        }
    }
}

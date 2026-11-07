//! System tray: open window / restart mihomo / process routing switch / exit,
//! plus an icon that follows the connection state.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use tauri::image::Image;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager};

use crate::core::apply::ApplyOutcome;
use crate::core::process::CoreState;
use crate::model::health::{ExitStatus, ListenerStatus};
use crate::model::{FaultPolicy, Model, ProcessRouting, ProcessRule, ValidationError};
use crate::procroute::client::ProcRouteStatus;
use crate::runtime::{Runtime, RuntimeStatus};

const TRAY_ID: &str = "main";
/// Upper bound on waiting for the service to confirm the disable before exiting.
const DISABLE_CONFIRM_WAIT: Duration = Duration::from_secs(3);
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

/// Recomputes the tray icon and menu from the runtime / procroute state; no-op when nothing changed or the tray is not built yet.
pub fn refresh(app: &AppHandle) {
    refresh_menu(app);
    refresh_icon(app);
}

fn refresh_icon(app: &AppHandle) {
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

/// The process-routing part of the tray menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RoutingMenu {
    /// Label of the global switch item.
    pub toggle: &'static str,
    /// Whether "停用进程分流并退出" is shown.
    pub disable_and_quit: bool,
}

pub fn routing_menu(pr: &ProcessRouting) -> RoutingMenu {
    RoutingMenu {
        toggle: if pr.enabled { "停用进程分流" } else { "启用进程分流" },
        disable_and_quit: pr.enabled && pr.rules.iter().any(|r| r.user_enabled),
    }
}

/// Display name, same as the frontend's `ruleLabel`: the name, else the
/// programs (by file name) as "a.exe" / "a.exe 等 N 个".
pub fn rule_label(r: &ProcessRule) -> String {
    let name = r.name.trim();
    if !name.is_empty() {
        return name.to_string();
    }
    let mut programs: Vec<&str> = vec![];
    for p in &r.processes {
        let file = p.value.rsplit(['\\', '/']).next().filter(|f| !f.is_empty()).unwrap_or(&p.value);
        if !programs.iter().any(|x| x.to_lowercase() == file.to_lowercase()) {
            programs.push(file);
        }
    }
    match programs.as_slice() {
        [] => "未命名规则".into(),
        [one] => (*one).into(),
        [first, ..] => format!("{first} 等 {} 个", programs.len()),
    }
}

fn blocking_rules(m: &Model) -> impl Iterator<Item = &ProcessRule> {
    let on = m.process_routing.enabled;
    m.process_routing.rules.iter().filter(move |r| on && r.user_enabled && r.on_fault == FaultPolicy::Block)
}

/// Names of the enabled rules that cut their programs off while Routa is not running.
pub fn blocking_rule_names(m: &Model) -> Vec<String> {
    blocking_rules(m).map(rule_label).collect()
}

/// Quitting would leave some programs offline and the user still wants to be asked.
pub fn exit_needs_confirm(m: &Model) -> bool {
    m.process_routing.exit_prompt && blocking_rules(m).next().is_some()
}

/// Turns process routing off, waits (at most `DISABLE_CONFIRM_WAIT`) until the
/// service confirmed the new rules, then exits. Does not exit when the
/// disable itself fails.
pub async fn disable_routing_and_quit(app: &AppHandle, rt: &Runtime) -> Result<(), Vec<ValidationError>> {
    use crate::procroute::client::{current_rules_confirmed, runtime_changed, status, ComponentState};
    rt.mutate_process_routing(|p| p.enabled = false).await?;
    runtime_changed();
    // Without a running service there is nothing to confirm.
    let deadline = tokio::time::Instant::now() + DISABLE_CONFIRM_WAIT;
    while matches!(status().component, ComponentState::Running)
        && !current_rules_confirmed(rt)
        && tokio::time::Instant::now() < deadline
    {
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    // Goes through RunEvent::ExitRequested, which runs the shutdown.
    app.exit(0);
    Ok(())
}

/// Tray "退出": asks in the main window first when block rules would cut programs off.
fn request_quit(app: &AppHandle) {
    match app.try_state::<Arc<Runtime>>().map(|rt| rt.model()) {
        Some(m) if exit_needs_confirm(&m) => {
            show_main_window(app);
            if let Err(e) = app.emit("confirm-exit", blocking_rule_names(&m)) {
                eprintln!("emit confirm-exit failed: {e}");
                app.exit(0);
            }
        }
        // Goes through RunEvent::ExitRequested, which runs the shutdown.
        _ => app.exit(0),
    }
}

/// Routing part of the menu currently shown, so unchanged refreshes keep the menu.
static SHOWN_MENU: Mutex<Option<RoutingMenu>> = Mutex::new(None);

fn build_menu(app: &AppHandle, routing: RoutingMenu) -> tauri::Result<Menu<tauri::Wry>> {
    let open = MenuItem::with_id(app, "open", "打开主窗口", true, None::<&str>)?;
    let restart = MenuItem::with_id(app, "restart", "重启 mihomo", true, None::<&str>)?;
    let toggle = MenuItem::with_id(app, "routing-toggle", routing.toggle, true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &restart, &toggle])?;
    if routing.disable_and_quit {
        menu.append(&MenuItem::with_id(app, "routing-disable-quit", "停用进程分流并退出", true, None::<&str>)?)?;
    }
    menu.append(&MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?)?;
    Ok(menu)
}

/// Rebuilds the menu when the process-routing switch or rules changed.
fn refresh_menu(app: &AppHandle) {
    let Some(tray) = app.tray_by_id(TRAY_ID) else { return };
    let Some(rt) = app.try_state::<Arc<Runtime>>() else { return };
    let routing = routing_menu(&rt.model().process_routing);
    let mut shown = SHOWN_MENU.lock().unwrap_or_else(|e| e.into_inner());
    if *shown == Some(routing) {
        return;
    }
    match build_menu(app, routing).and_then(|menu| tray.set_menu(Some(menu))) {
        Ok(()) => *shown = Some(routing),
        Err(e) => eprintln!("set tray menu failed: {e}"),
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
    let routing = routing_menu(
        &app.try_state::<Arc<Runtime>>().map(|rt| rt.model().process_routing).unwrap_or_default(),
    );
    let menu = build_menu(app, routing)?;

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
            "routing-toggle" => {
                let rt = Arc::clone(&*app.state::<Arc<Runtime>>());
                tauri::async_runtime::spawn(async move {
                    if let Err(e) = rt.mutate_process_routing(|p| p.enabled = !p.enabled).await {
                        eprintln!("tray: toggle process routing failed: {e:?}");
                    }
                });
            }
            "routing-disable-quit" => {
                let rt = Arc::clone(&*app.state::<Arc<Runtime>>());
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(e) = disable_routing_and_quit(&app, &rt).await {
                        eprintln!("tray: disable process routing failed: {e:?}");
                    }
                });
            }
            "quit" => request_quit(app),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                show_main_window(tray.app_handle());
            }
        })
        .build(app)?;
    *SHOWN.lock().unwrap_or_else(|e| e.into_inner()) = Some(TrayState::Idle);
    *SHOWN_MENU.lock().unwrap_or_else(|e| e.into_inner()) = Some(routing);

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
    use crate::model::ProcessEntry;
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

    fn block_rule(on_fault: FaultPolicy, user_enabled: bool) -> ProcessRule {
        ProcessRule {
            name: "游戏".into(),
            processes: vec![ProcessEntry::name("game.exe")],
            on_fault,
            user_enabled,
            ..ProcessRule::default()
        }
    }

    fn with_rules(rules: Vec<ProcessRule>) -> Model {
        let mut m = Model::default();
        m.process_routing.enabled = true;
        m.process_routing.rules = rules;
        m
    }

    #[test]
    fn exit_needs_confirm_only_with_enabled_block_rules() {
        assert!(!exit_needs_confirm(&with_rules(vec![])), "no rules");
        assert!(!exit_needs_confirm(&with_rules(vec![block_rule(FaultPolicy::Direct, true)])), "direct only");
        assert!(!exit_needs_confirm(&with_rules(vec![block_rule(FaultPolicy::Block, false)])), "disabled block rule");
        let m = with_rules(vec![block_rule(FaultPolicy::Direct, true), block_rule(FaultPolicy::Block, true)]);
        assert!(exit_needs_confirm(&m), "enabled block rule");
        assert_eq!(blocking_rule_names(&m), vec!["游戏".to_string()]);
        let mut off = m.clone();
        off.process_routing.exit_prompt = false;
        assert!(!exit_needs_confirm(&off), "exit_prompt off");
        let mut off = m;
        off.process_routing.enabled = false;
        assert!(!exit_needs_confirm(&off), "routing off");
    }

    #[test]
    fn rule_label_falls_back_to_programs() {
        let mut r = block_rule(FaultPolicy::Block, true);
        assert_eq!(rule_label(&r), "游戏");
        r.name = "  ".into();
        assert_eq!(rule_label(&r), "game.exe");
        r.processes.push(ProcessEntry::path(r"C:\x\Game.exe"));
        r.processes.push(ProcessEntry::name("b.exe"));
        assert_eq!(rule_label(&r), "game.exe 等 2 个");
        r.processes.clear();
        assert_eq!(rule_label(&r), "未命名规则");
    }

    #[test]
    fn routing_menu_follows_switch_and_rules() {
        let mut m = with_rules(vec![block_rule(FaultPolicy::Direct, true)]);
        assert_eq!(routing_menu(&m.process_routing), RoutingMenu { toggle: "停用进程分流", disable_and_quit: true });
        m.process_routing.rules[0].user_enabled = false;
        assert_eq!(routing_menu(&m.process_routing), RoutingMenu { toggle: "停用进程分流", disable_and_quit: false });
        m.process_routing.rules[0].user_enabled = true;
        m.process_routing.enabled = false;
        assert_eq!(routing_menu(&m.process_routing), RoutingMenu { toggle: "启用进程分流", disable_and_quit: false });
    }

    #[test]
    fn icons_decode() {
        for state in [TrayState::Idle, TrayState::Connected, TrayState::Warning, TrayState::Disconnected] {
            let img = icon_for(state).unwrap();
            assert_eq!((img.width(), img.height()), (32, 32));
        }
    }
}

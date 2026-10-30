//! Persisted presentation preferences and the single explicitly managed proxy.
use crate::{
    core::apply::ApplyOutcome,
    model::*,
    runtime::Runtime,
    store::{read_json, write_json_atomic},
    system_proxy::SystemProxy,
};
use serde::{Deserialize, Serialize};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub enum InterfaceMode {
    Simple,
    Full,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub enum CaptureMode {
    #[default]
    System,
    Apps,
    Tun,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, specta::Type)]
#[serde(default)]
pub struct UiPreferences {
    pub mode: Option<InterfaceMode>,
    pub listener_id: Option<ListenerId>,
    pub exit_id: Option<ExitId>,
    pub system_listener_id: Option<ListenerId>,
    pub capture_mode: CaptureMode,
    pub auto_connect: bool,
    pub close_to_tray: bool,
    pub managed_rules: Vec<ProcessRuleId>,
    pub selected_apps: Vec<ProcessRuleId>,
}
#[derive(Clone, Debug, Serialize, specta::Type)]
pub struct DesktopState {
    pub preferences: UiPreferences,
    pub connection: Option<CaptureMode>,
    pub requested: Option<CaptureMode>,
    pub error: Option<String>,
    pub is_admin: bool,
    pub system_listener_id: Option<ListenerId>,
}
pub struct Desktop {
    file: PathBuf,
    prefs: Mutex<UiPreferences>,
    active: Mutex<Option<CaptureMode>>,
    active_system_listener: Mutex<Option<ListenerId>>,
    error: Mutex<Option<String>>,
    operation: tokio::sync::Mutex<()>,
    proxy: SystemProxy,
    fresh: bool,
}
fn result_ok(outcome: &ApplyOutcome) -> Result<(), String> {
    match outcome {
        ApplyOutcome::Reloaded {
            failed_listeners,
            uncut,
            ..
        } if failed_listeners.is_empty() && *uncut == 0 => Ok(()),
        ApplyOutcome::Reloaded { failed_listeners, .. } => Err(format!(
            "配置尚未完全生效，请检查监听端口或现有连接：{}",
            failed_listeners
                .iter()
                .map(|l| format!("{} ({})", l.name, l.port))
                .collect::<Vec<_>>()
                .join("、")
        )),
        ApplyOutcome::FailedKeptPrevious { error }
        | ApplyOutcome::FailedClosed { error, .. }
        | ApplyOutcome::FailedCoreStopped { error }
        | ApplyOutcome::NotRunning { error } => Err(format!("配置未能生效：{error}")),
    }
}
fn validation(e: Vec<ValidationError>) -> String {
    e.into_iter()
        .map(|e| match e {
            ValidationError::DuplicateName { name, .. } => format!("名称「{name}」已存在"),
            ValidationError::PortConflict { port, with } => format!("端口 {port} 已被「{with}」占用"),
            ValidationError::InvalidName { reason, .. }
            | ValidationError::InvalidProxy { reason, .. }
            | ValidationError::InvalidExePath { reason, .. } => reason,
            ValidationError::IntervalOutOfRange { .. } => "更新间隔不在允许范围内".into(),
            ValidationError::DanglingRef { .. } => "引用的监听器、出口或节点已不存在，请重新选择".into(),
            ValidationError::InterfaceMismatch { exit, node } => {
                format!("出口「{exit}」与节点「{node}」使用的网卡不一致")
            }
            ValidationError::CoreRejected { message } => format!("内核未接受配置：{message}"),
            ValidationError::DuplicateProcessRule { exe_path } => format!("{exe_path} 已有分流规则"),
            ValidationError::ListenerTypeNotAllowed { listener } => format!("监听器「{listener}」不支持 SOCKS5"),
            ValidationError::ListenerAddressNotAllowed { listener, .. } => {
                format!("监听器「{listener}」需要 IPv4 地址")
            }
            ValidationError::TooManyProcessRules { max } => format!("最多同时启用 {max} 条应用规则"),
        })
        .collect::<Vec<_>>()
        .join("；")
}
pub fn is_admin() -> bool {
    #[cfg(windows)]
    {
        unsafe { windows_sys::Win32::UI::Shell::IsUserAnAdmin() != 0 }
    }
    #[cfg(not(windows))]
    {
        false
    }
}
impl Desktop {
    pub fn close_to_tray(&self) -> bool {
        self.prefs().close_to_tray
    }
    pub fn open(dir: PathBuf) -> Result<Self, String> {
        let file = dir.join("ui-preferences.json");
        let fresh = !dir.join("state.json").exists() && !file.exists();
        let prefs = read_json::<UiPreferences>(&file)
            .map_err(|e| format!("读取界面设置失败：{e}"))?
            .unwrap_or_else(|| UiPreferences {
                mode: if fresh { None } else { Some(InterfaceMode::Full) },
                close_to_tray: true,
                ..Default::default()
            });
        let proxy = SystemProxy::new(dir.join("system-proxy-lease.json"));
        // Recover a previous interrupted run, but never clobber a newer proxy owner.
        proxy.release()?;
        Ok(Self {
            file,
            prefs: Mutex::new(prefs),
            active: Mutex::new(None),
            active_system_listener: Mutex::new(None),
            error: Mutex::new(None),
            operation: tokio::sync::Mutex::new(()),
            proxy,
            fresh,
        })
    }
    fn prefs(&self) -> UiPreferences {
        self.prefs.lock().unwrap().clone()
    }
    fn persist(&self, p: UiPreferences) -> Result<UiPreferences, String> {
        write_json_atomic(&self.file, &p).map_err(|e| format!("保存界面设置失败：{e}"))?;
        *self.prefs.lock().unwrap() = p.clone();
        Ok(p)
    }
    pub async fn initialize(&self, rt: &Runtime) -> Result<(), String> {
        let _guard = self.operation.lock().await;
        if self.fresh && self.prefs().listener_id.is_none() && rt.model().listeners.is_empty() {
            self.bind_inner(rt, None).await?;
        }
        Ok(())
    }
    pub fn state(&self, rt: &Runtime) -> DesktopState {
        let prefs = self.prefs();
        let requested = *self.active.lock().unwrap();
        let running = matches!(rt.status().core, crate::core::process::CoreState::Running);
        let system_listener_id = self.active_system_listener.lock().unwrap().clone();
        let effective = system_listener_id
            .as_ref()
            .or(prefs.listener_id.as_ref())
            .is_some_and(|id| rt.status().effective_listeners.contains(id));
        let valid = running
            && effective
            && match requested {
                Some(CaptureMode::System) => {
                    let id = system_listener_id.as_ref().or(prefs.listener_id.as_ref());
                    rt.model()
                        .listeners
                        .iter()
                        .find(|l| Some(&l.id) == id)
                        .is_some_and(|l| self.proxy.owns_port(l.port))
                }
                Some(CaptureMode::Apps) => {
                    let status = crate::procroute::client::status();
                    !prefs.selected_apps.is_empty()
                        && crate::procroute::client::current_rules_confirmed(rt)
                        && prefs.selected_apps.iter().all(|id| {
                            status.actions.get(id).is_some_and(|a| a.action == "Proxy")
                                && rt
                                    .model()
                                    .process_routing
                                    .rules
                                    .iter()
                                    .any(|r| &r.id == id && Some(&r.listener) == prefs.listener_id.as_ref())
                        })
                }
                Some(CaptureMode::Tun) => rt.tun_adapter_ready(),
                None => false,
            };
        DesktopState {
            preferences: prefs,
            connection: requested.filter(|_| valid),
            requested,
            error: self.error.lock().unwrap().clone(),
            is_admin: is_admin(),
            system_listener_id,
        }
    }
    pub async fn save_preferences(&self, input: UiPreferences) -> Result<UiPreferences, String> {
        let _guard = self.operation.lock().await;
        let mut p = self.prefs();
        // Management IDs are backend-owned, not mutable presentation settings.
        p.mode = input.mode;
        p.auto_connect = input.auto_connect;
        p.close_to_tray = input.close_to_tray;
        if self.active.lock().unwrap().is_none() {
            p.capture_mode = input.capture_mode;
        }
        self.persist(p)
    }
    pub async fn bind(&self, rt: &Runtime, id: Option<ListenerId>) -> Result<UiPreferences, String> {
        let _guard = self.operation.lock().await;
        if self.active.lock().unwrap().is_some() {
            return Err("请先关闭当前代理，再更改默认监听器".into());
        }
        self.bind_inner(rt, id).await
    }
    async fn bind_inner(&self, rt: &Runtime, id: Option<ListenerId>) -> Result<UiPreferences, String> {
        let m = rt.model();
        let mut p = self.prefs();
        let listener = if let Some(id) = id {
            m.listeners
                .iter()
                .find(|l| l.id == id)
                .cloned()
                .ok_or("所选监听器已不存在")?
        } else {
            if m.listeners.iter().any(|l| l.port == 7881) || m.settings.controller_port == 7881 {
                return Err("端口 7881 已被配置占用，请选择已有监听器".into());
            }
            let mut name = "默认监听器".to_string();
            for i in 2.. {
                if !m.listeners.iter().any(|l| l.name == name) {
                    break;
                }
                name = format!("默认监听器 {i}");
            }
            let mut l = Listener::new_default(name, 7881);
            l.listen = "127.0.0.1".into();
            l
        };
        if listener.listener_type == ListenerType::Http || listener.auth.is_some() {
            return Err("简洁版需要无需认证的混合或 SOCKS5 监听器".into());
        }
        p.listener_id = Some(listener.id.clone());
        p.exit_id = Some(ExitId::new());
        // Reserve identifiers durably before creating model objects.
        let saved = self.persist(p)?;
        if !m.listeners.iter().any(|l| l.id == listener.id) {
            rt.mutate(|m| m.listeners.push(listener)).await.map_err(validation)?;
        }
        Ok(saved)
    }
    pub async fn select_node(&self, rt: &Runtime, node: NodeRef) -> Result<(), String> {
        let _guard = self.operation.lock().await;
        let p = self.prefs();
        let before = rt.model();
        if crate::model::resolve::resolve(&before, &node).is_none() {
            return Err("节点已失效，请重新选择".into());
        }
        let id = p.listener_id.ok_or("请先选择默认监听器")?;
        let exit_id = p.exit_id.ok_or("默认代理未配置，请重新绑定监听器")?;
        let previous_listener = before
            .listeners
            .iter()
            .find(|l| l.id == id)
            .cloned()
            .ok_or("默认监听器已删除，请重新选择")?;
        let previous_exit = before.exits.iter().find(|e| e.id == exit_id).cloned();
        let outcome = rt
            .mutate(|m| {
                let name = previous_exit.as_ref().map(|e| e.name.clone()).unwrap_or_else(|| {
                    let mut n = "默认代理".to_string();
                    let mut i = 2;
                    while m.exits.iter().any(|e| e.name == n) {
                        n = format!("默认代理 {i}");
                        i += 1;
                    }
                    n
                });
                let exit = Exit {
                    id: exit_id.clone(),
                    name,
                    kind: ExitKind::Node { node },
                    interface: None,
                };
                if let Some(e) = m.exits.iter_mut().find(|e| e.id == exit_id) {
                    *e = exit;
                } else {
                    m.exits.push(exit);
                }
                let l = m.listeners.iter_mut().find(|l| l.id == id).unwrap();
                l.exit = Some(exit_id.clone());
                l.user_enabled = true;
            })
            .await
            .map_err(validation)?;
        // Offline edits can be saved; connect later starts and verifies the core.
        if !matches!(outcome, ApplyOutcome::NotRunning { .. }) {
            if let Err(error) = result_ok(&outcome) {
                let _ = rt
                    .mutate(|m| {
                        if let Some(l) = m.listeners.iter_mut().find(|l| l.id == id) {
                            *l = previous_listener;
                        }
                        m.exits.retain(|e| e.id != exit_id);
                        if let Some(e) = previous_exit {
                            m.exits.push(e);
                        }
                    })
                    .await;
                return Err(error);
            }
        }
        if self.active.lock().unwrap().as_ref() == Some(&CaptureMode::Tun) {
            rt.set_tun_exit(Some(exit_id)).await?;
        }
        Ok(())
    }
    pub async fn save_app(&self, rt: &Runtime, path: String, selected: bool) -> Result<(), String> {
        let _guard = self.operation.lock().await;
        let mut p = self.prefs();
        let listener = p.listener_id.clone().ok_or("请先设置默认代理")?;
        let normalized = crate::model::normalize::normalize_exe_path(&path);
        let model = rt.model();
        let existing = model
            .process_routing
            .rules
            .iter()
            .find(|r| r.exe_path.eq_ignore_ascii_case(&normalized));
        if existing.is_some_and(|r| !p.managed_rules.contains(&r.id)) {
            return Err("该程序已有完整版规则，请在进程分流中修改".into());
        }
        let mut rule = existing.cloned().unwrap_or_else(|| ProcessRule {
            exe_path: path,
            listener: listener.clone(),
            ..Default::default()
        });
        rule.listener = listener;
        rule.user_enabled = selected && self.active.lock().unwrap().as_ref() == Some(&CaptureMode::Apps);
        if !p.managed_rules.contains(&rule.id) {
            p.managed_rules.push(rule.id.clone());
        }
        p.selected_apps.retain(|id| id != &rule.id);
        if selected {
            p.selected_apps.push(rule.id.clone());
        }
        let old_preferences = self.prefs();
        self.persist(p)?;
        if let Err(errors) = rt
            .mutate_process_routing(|pr| {
                if let Some(r) = pr.rules.iter_mut().find(|r| r.id == rule.id) {
                    *r = rule;
                } else {
                    pr.rules.push(rule);
                }
            })
            .await
        {
            self.persist(old_preferences)?;
            return Err(validation(errors));
        }
        Ok(())
    }
    async fn disable_capture(&self, rt: &Runtime) -> Result<(), String> {
        self.proxy.release()?;
        let active = *self.active.lock().unwrap();
        if active == Some(CaptureMode::Tun) {
            rt.set_tun_exit(None).await?;
        }
        if active == Some(CaptureMode::Apps) {
            let ids = self.prefs().managed_rules;
            rt.mutate_process_routing(|pr| {
                for r in &mut pr.rules {
                    if ids.contains(&r.id) {
                        r.user_enabled = false;
                    }
                }
            })
            .await
            .map_err(validation)?;
        }
        *self.active.lock().unwrap() = None;
        *self.active_system_listener.lock().unwrap() = None;
        Ok(())
    }
    async fn enable_capture(
        &self,
        rt: &Runtime,
        mode: CaptureMode,
        listener_override: Option<&ListenerId>,
    ) -> Result<(), String> {
        let p = self.prefs();
        let model = rt.model();
        let id = listener_override
            .or(p.listener_id.as_ref())
            .ok_or("请先选择默认监听器")?;
        let l = model
            .listeners
            .iter()
            .find(|l| &l.id == id)
            .ok_or("默认监听器已不存在")?;
        if !l.user_enabled || l.exit.is_none() {
            return Err("请先选择节点并启用默认监听器".into());
        }
        if mode == CaptureMode::Tun && !is_admin() {
            return Err("TUN 需要管理员权限。请退出后以管理员身份运行 Routa，再开启 TUN。".into());
        }
        if !matches!(rt.status().core, crate::core::process::CoreState::Running) {
            rt.start().await.map_err(|e| e.to_string())?;
        }
        if !rt.status().effective_listeners.contains(id) {
            return Err("默认监听器未能生效，请检查端口或节点".into());
        }
        match mode {
            CaptureMode::System => {
                if l.listener_type != ListenerType::Mixed {
                    return Err("系统代理需要混合监听器，请在代理页更改类型或绑定混合监听器".into());
                }
                if !matches!(l.listen.as_str(), "127.0.0.1" | "0.0.0.0" | "localhost") {
                    return Err("系统代理需要监听本机 IPv4 地址".into());
                }
                self.proxy.enable(l.port)?;
            }
            CaptureMode::Tun => {
                rt.set_tun_exit(l.exit.clone()).await?;
                for _ in 0..20 {
                    if rt.tun_adapter_ready() {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
                if !rt.tun_adapter_ready() {
                    let _ = rt.set_tun_exit(None).await;
                    return Err("TUN 网卡未就绪，请检查管理员权限和内核日志".into());
                }
            }
            CaptureMode::Apps => {
                if p.selected_apps.is_empty() {
                    return Err("请先选择需要代理的应用".into());
                }
                if let Some(rule) = model
                    .process_routing
                    .rules
                    .iter()
                    .find(|r| p.selected_apps.contains(&r.id) && &r.listener != id)
                {
                    return Err(format!(
                        "{} 在完整版使用了其他监听器，请在简洁版关闭并重新勾选该应用，再开启代理",
                        rule.exe_path
                    ));
                }
                if !matches!(
                    crate::procroute::client::status().component,
                    crate::procroute::client::ComponentState::Running
                ) {
                    return Err("请先安装或启动应用分流组件".into());
                }
                if !model.process_routing.enabled
                    && model
                        .process_routing
                        .rules
                        .iter()
                        .any(|r| r.user_enabled && !p.managed_rules.contains(&r.id))
                {
                    return Err("完整版还有其他启用规则，请先在进程分流中确认全局开关".into());
                }
                rt.mutate_process_routing(|pr| {
                    pr.enabled = true;
                    for r in &mut pr.rules {
                        if p.managed_rules.contains(&r.id) {
                            r.user_enabled = p.selected_apps.contains(&r.id);
                        }
                    }
                })
                .await
                .map_err(validation)?;
                for _ in 0..30 {
                    let actions = crate::procroute::client::status().actions;
                    if crate::procroute::client::current_rules_confirmed(rt)
                        && p.selected_apps
                            .iter()
                            .all(|id| actions.get(id).is_some_and(|a| a.action == "Proxy"))
                    {
                        return Ok(());
                    }
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
                let _ = rt
                    .mutate_process_routing(|pr| {
                        for r in &mut pr.rules {
                            if p.managed_rules.contains(&r.id) {
                                r.user_enabled = false;
                            }
                        }
                    })
                    .await;
                return Err("应用规则未确认生效，请检查组件状态后重试".into());
            }
        }
        Ok(())
    }
    pub async fn connect(&self, rt: &Runtime, mode: Option<CaptureMode>) -> Result<DesktopState, String> {
        self.connect_for(rt, mode, None).await
    }
    pub async fn system_for_listener(&self, rt: &Runtime, id: ListenerId) -> Result<DesktopState, String> {
        self.connect_for(rt, Some(CaptureMode::System), Some(id)).await
    }
    async fn connect_for(
        &self,
        rt: &Runtime,
        mode: Option<CaptureMode>,
        listener_override: Option<ListenerId>,
    ) -> Result<DesktopState, String> {
        let _guard = self.operation.lock().await;
        let previous = *self.active.lock().unwrap();
        let previous_listener = self.active_system_listener.lock().unwrap().clone();
        if mode == previous
            && mode.is_some()
            && listener_override == previous_listener
            && self.state(rt).connection == mode
        {
            return Ok(self.state(rt));
        }
        self.disable_capture(rt).await?;
        if let Some(mode) = mode {
            if let Err(error) = self.enable_capture(rt, mode, listener_override.as_ref()).await {
                *self.error.lock().unwrap() = Some(error.clone());
                if let Some(old) = previous {
                    if self.enable_capture(rt, old, previous_listener.as_ref()).await.is_ok() {
                        *self.active.lock().unwrap() = Some(old);
                        *self.active_system_listener.lock().unwrap() = previous_listener;
                    }
                }
                return Err(error);
            }
            *self.active.lock().unwrap() = Some(mode);
            let mut p = self.prefs();
            p.capture_mode = mode;
            *self.active_system_listener.lock().unwrap() = listener_override.clone();
            if let Some(id) = listener_override {
                p.system_listener_id = Some(id);
            }
            if let Err(e) = self.persist(p) {
                let _ = self.disable_capture(rt).await;
                return Err(e);
            }
        }
        *self.error.lock().unwrap() = None;
        Ok(self.state(rt))
    }
    pub async fn set_port(&self, rt: &Runtime, port: u16) -> Result<(), String> {
        let _guard = self.operation.lock().await;
        let id = self.prefs().listener_id.ok_or("没有默认监听器")?;
        let old = rt
            .model()
            .listeners
            .iter()
            .find(|l| l.id == id)
            .cloned()
            .ok_or("默认监听器已删除")?;
        let outcome = rt
            .mutate(|m| {
                if let Some(l) = m.listeners.iter_mut().find(|l| l.id == id) {
                    l.port = port;
                }
            })
            .await
            .map_err(validation)?;
        let applied = if matches!(outcome, ApplyOutcome::NotRunning { .. }) {
            Ok(())
        } else {
            result_ok(&outcome)
        };
        let capture_listener = self
            .active_system_listener
            .lock()
            .unwrap()
            .clone()
            .or(self.prefs().listener_id);
        let update = applied.and_then(|_| {
            if self.active.lock().unwrap().as_ref() == Some(&CaptureMode::System)
                && capture_listener.as_ref() == Some(&id)
            {
                self.proxy.enable(port)
            } else {
                Ok(())
            }
        });
        if let Err(error) = update {
            let _ = rt
                .mutate(|m| {
                    if let Some(l) = m.listeners.iter_mut().find(|l| l.id == id) {
                        *l = old;
                    }
                })
                .await;
            return Err(error);
        }
        Ok(())
    }
    pub async fn shutdown(&self, rt: &Runtime) {
        let _guard = self.operation.lock().await;
        if let Err(e) = self.disable_capture(rt).await {
            eprintln!("restore capture on exit: {e}");
        }
    }
    pub fn spawn_monitor(self: Arc<Self>, rt: Arc<Runtime>) {
        tauri::async_runtime::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(2)).await;
                let _guard = self.operation.lock().await;
                if self.active.lock().unwrap().as_ref() == Some(&CaptureMode::System) {
                    let state = rt.status();
                    if matches!(state.core, crate::core::process::CoreState::Starting) {
                        continue;
                    }
                    let id = self
                        .active_system_listener
                        .lock()
                        .unwrap()
                        .clone()
                        .or(self.prefs().listener_id);
                    let model = rt.model();
                    let target = model.listeners.iter().find(|l| {
                        Some(&l.id) == id.as_ref()
                            && l.listener_type == ListenerType::Mixed
                            && matches!(l.listen.as_str(), "127.0.0.1" | "localhost" | "0.0.0.0")
                            && state.effective_listeners.contains(&l.id)
                    });
                    let owns = self.proxy.owns().unwrap_or(false);
                    let result = if let Some(l) =
                        target.filter(|_| owns && matches!(state.core, crate::core::process::CoreState::Running))
                    {
                        if self.proxy.owns_port(l.port) {
                            Ok(())
                        } else {
                            self.proxy.enable(l.port)
                        }
                    } else {
                        let result = self.proxy.release();
                        if result.is_ok() {
                            *self.active.lock().unwrap() = None;
                            *self.active_system_listener.lock().unwrap() = None;
                        }
                        result
                    };
                    if let Err(e) = result {
                        *self.error.lock().unwrap() = Some(e);
                    }
                }
            }
        });
    }
}

type Dt<'a> = tauri::State<'a, Arc<Desktop>>;
type Rt<'a> = tauri::State<'a, Arc<Runtime>>;
#[tauri::command]
#[specta::specta]
pub async fn desktop_state(dt: Dt<'_>, rt: Rt<'_>) -> Result<DesktopState, String> {
    dt.initialize(&rt).await?;
    Ok(dt.state(&rt))
}
#[tauri::command]
#[specta::specta]
pub async fn save_ui_preferences(dt: Dt<'_>, input: UiPreferences) -> Result<UiPreferences, String> {
    dt.save_preferences(input).await
}
#[tauri::command]
#[specta::specta]
pub async fn bind_default_listener(dt: Dt<'_>, rt: Rt<'_>, id: Option<ListenerId>) -> Result<UiPreferences, String> {
    dt.bind(&rt, id).await
}
#[tauri::command]
#[specta::specta]
pub async fn select_simple_node(dt: Dt<'_>, rt: Rt<'_>, node: NodeRef) -> Result<(), String> {
    dt.select_node(&rt, node).await
}
#[tauri::command]
#[specta::specta]
pub async fn set_desktop_connection(dt: Dt<'_>, rt: Rt<'_>, mode: Option<CaptureMode>) -> Result<DesktopState, String> {
    dt.connect(&rt, mode).await
}
#[tauri::command]
#[specta::specta]
pub async fn set_system_proxy_listener(dt: Dt<'_>, rt: Rt<'_>, id: ListenerId) -> Result<DesktopState, String> {
    dt.system_for_listener(&rt, id).await
}
#[tauri::command]
#[specta::specta]
pub async fn save_simple_app(dt: Dt<'_>, rt: Rt<'_>, path: String, selected: bool) -> Result<(), String> {
    dt.save_app(&rt, path, selected).await
}
#[tauri::command]
#[specta::specta]
pub async fn set_simple_port(dt: Dt<'_>, rt: Rt<'_>, port: u16) -> Result<(), String> {
    dt.set_port(&rt, port).await
}
#[tauri::command]
#[specta::specta]
pub async fn get_connection_details(rt: Rt<'_>) -> Result<Vec<crate::runtime::ConnectionInfo>, String> {
    rt.connection_details().await
}
#[tauri::command]
#[specta::specta]
pub async fn read_core_log(rt: Rt<'_>) -> Result<String, String> {
    read_log_tail(&rt.core_workdir().join("mihomo.log"), 64 * 1024)
}

fn read_log_tail(path: &std::path::Path, max_bytes: u64) -> Result<String, String> {
    use std::io::{Read, Seek, SeekFrom};
    let mut f = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(String::new()),
        Err(e) => return Err(e.to_string()),
    };
    let length = f.metadata().map_err(|e| e.to_string())?.len();
    let start = length.saturating_sub(max_bytes);
    // Include the preceding byte to tell a line boundary from a partial record.
    let offset = start.saturating_sub(1);
    f.seek(SeekFrom::Start(offset))
        .map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    f.take(length - offset).read_to_end(&mut bytes).map_err(|e| e.to_string())?;
    let first = if start > 0 {
        bytes.iter().position(|b| *b == b'\n').map_or(bytes.len(), |i| i + 1)
    } else { 0 };
    Ok(String::from_utf8_lossy(&bytes[first..]).into_owned())
}
#[tauri::command]
#[specta::specta]
pub async fn change_process_listener(
    dt: Dt<'_>,
    rt: Rt<'_>,
    id: ProcessRuleId,
    listener: ListenerId,
) -> Result<(), String> {
    let _guard = dt.operation.lock().await;
    let model = rt.model();
    let old = model
        .process_routing
        .rules
        .iter()
        .find(|r| r.id == id)
        .cloned()
        .ok_or("规则已不存在")?;
    let target = model
        .listeners
        .iter()
        .find(|l| l.id == listener)
        .ok_or("监听器已不存在")?;
    if target.listener_type == ListenerType::Http || !target.user_enabled {
        return Err("请选择已启用的混合或 SOCKS5 监听器".into());
    }
    rt.mutate_process_routing(|pr| {
        if let Some(r) = pr.rules.iter_mut().find(|r| r.id == id) {
            r.listener = listener.clone();
        }
    })
    .await
    .map_err(validation)?;
    let needs_ack = model.process_routing.enabled
        && old.user_enabled
        && matches!(
            crate::procroute::client::status().component,
            crate::procroute::client::ComponentState::Running
        );
    if !needs_ack {
        return Ok(());
    }
    crate::procroute::client::runtime_changed();
    for _ in 0..50 {
        if crate::procroute::client::current_rules_confirmed(&rt) {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    rt.mutate_process_routing(|pr| {
        if let Some(r) = pr.rules.iter_mut().find(|r| r.id == id && r.listener == listener) {
            r.listener = old.listener.clone();
        }
    })
    .await
    .map_err(validation)?;
    Err("监听器切换未能确认生效，已恢复原监听器，请检查组件状态后重试".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn log_tail_starts_at_a_complete_record_and_preserves_utf8() {
        let td = tempfile::tempdir().unwrap();
        let path = td.path().join("mihomo.log");
        let last = "time=\"2026-10-30T20:00:00+08:00\" level=info msg=\"启动成功\"\n";
        std::fs::write(&path, format!("很长的旧错误详情\n{last}")).unwrap();
        assert_eq!(read_log_tail(&path, last.len() as u64).unwrap(), last);
        assert_eq!(read_log_tail(&path, last.len() as u64 + 5).unwrap(), last);
        assert_eq!(read_log_tail(&path, 3).unwrap(), "");
        assert_eq!(read_log_tail(&path, 0).unwrap(), "");
    }
    #[test]
    fn log_tail_keeps_short_files_and_handles_missing_files() {
        let td = tempfile::tempdir().unwrap();
        let path = td.path().join("mihomo.log");
        assert_eq!(read_log_tail(&path, 65536).unwrap(), "");
        std::fs::write(&path, "启动成功\n").unwrap();
        assert_eq!(read_log_tail(&path, 65536).unwrap(), "启动成功\n");
    }
    #[test]
    fn legacy_install_keeps_full_interface() {
        let td = tempfile::tempdir().unwrap();
        std::fs::write(td.path().join("state.json"), "{}").unwrap();
        let dt = Desktop::open(td.path().into()).unwrap();
        assert_eq!(dt.prefs().mode, Some(InterfaceMode::Full));
    }
    #[test]
    fn new_install_asks_once_and_saves_mode() {
        let td = tempfile::tempdir().unwrap();
        let dt = Desktop::open(td.path().into()).unwrap();
        assert_eq!(dt.prefs().mode, None);
        let mut p = dt.prefs();
        p.mode = Some(InterfaceMode::Simple);
        dt.persist(p).unwrap();
        let dt = Desktop::open(td.path().into()).unwrap();
        assert_eq!(dt.prefs().mode, Some(InterfaceMode::Simple));
    }
}

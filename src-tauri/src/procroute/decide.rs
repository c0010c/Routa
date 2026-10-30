//! 每条规则的实际动作（spec 第 5 节）。纯函数，不做任何 IO。

use std::collections::HashMap;

use routa_procsvc_proto::{Action, DesiredState, ExitPolicy, FaultPolicy, ProxyKind, ProxyTarget, RuleSpec};

use crate::core::process::CoreState;
use crate::model::health::ListenerStatus;
use crate::model::validate::process_proxy_host;
use crate::model::{Listener, ListenerType, Model, ProcessRule, ProcessRuleId};
use crate::runtime::{CoreStopReason, RuntimeStatus};

/// 根据模型和运行状态算出下发给助手的期望状态。全局开关关闭或规则被禁用时，
/// 规则不出现在结果里（助手不再管理该进程）；其余规则保持原顺序。
pub fn decide(model: &Model, status: &RuntimeStatus, routa_pid: u32) -> DesiredState {
    let rules = active_rules(model)
        .map(|r| RuleSpec {
            id: r.id.0.clone(),
            exe_path: r.exe_path.clone(),
            action: rule_action(model, status, r).0,
            on_fault: r.on_fault.clone(),
            reset_on_change: r.reset_on_change,
        })
        .collect();
    DesiredState { rules, on_routa_exit: model.process_routing.on_routa_exit.clone(), routa_pid }
}

/// 与 [`decide`] 同一组规则：不是代理时的原因（中文，可直接展示）；代理时为 None。
pub fn reasons(model: &Model, status: &RuntimeStatus) -> HashMap<ProcessRuleId, Option<String>> {
    active_rules(model)
        .map(|r| (r.id.clone(), rule_action(model, status, r).1.map(str::to_string)))
        .collect()
}

/// 下发给助手的规则：全局开关打开时用户启用的规则。
fn active_rules(model: &Model) -> impl Iterator<Item = &ProcessRule> {
    let routing = &model.process_routing;
    routing.rules.iter().filter(move |r| routing.enabled && r.user_enabled)
}

/// spec 第 5 节，按顺序判断，先命中者生效。返回动作和（非代理时的）原因。
fn rule_action(model: &Model, status: &RuntimeStatus, rule: &ProcessRule) -> (Action, Option<&'static str>) {
    // 为防泄露自动停止的内核：无论内核状态如何都阻断。
    if status.core_stop_reason == Some(CoreStopReason::FailClosed) {
        return (Action::Block, Some("新配置无法生效，内核已停止"));
    }
    let on_exit = &model.process_routing.on_routa_exit;
    match &status.core {
        CoreState::Crashed { .. } => (fault_action(&rule.on_fault), Some("内核崩溃")),
        CoreState::Starting if status.core_recovering => (fault_action(&rule.on_fault), Some("内核恢复中")),
        CoreState::Starting => (exit_action(on_exit), Some("内核启动中")),
        CoreState::Stopped => match status.core_stop_reason {
            Some(CoreStopReason::User) => (exit_action(on_exit), Some("内核已手动停止")),
            _ => (Action::Block, Some("内核未运行")),
        },
        // Listener 被停用 / 未生效 / 已删除 / 类型不对：一律阻断，与 on_fault 无关。
        CoreState::Running => match model.listeners.iter().find(|l| l.id == rule.listener) {
            None => (Action::Block, Some("Listener 已删除")),
            Some(l) if l.listener_type == ListenerType::Http => (Action::Block, Some("HTTP 类型的 Listener 不能使用")),
            Some(l) if !listener_in_effect(status, l) => (Action::Block, Some("Listener 未生效")),
            Some(l) => match proxy_target(l) {
                Some(t) => (Action::Proxy(t), None),
                None => (Action::Block, Some("进程分流仅支持 IPv4 入口，请修改 Listener 的监听地址")),
            },
        },
    }
}

fn listener_in_effect(status: &RuntimeStatus, l: &Listener) -> bool {
    status.health.listeners.get(&l.id) == Some(&ListenerStatus::Active)
        && status.effective_listeners.contains(&l.id)
        && !status.failed_listeners.iter().any(|f| f.id == l.id)
}

fn proxy_target(l: &Listener) -> Option<ProxyTarget> {
    Some(ProxyTarget {
        kind: ProxyKind::Socks5,
        host: process_proxy_host(&l.listen)?,
        port: l.port,
        username: l.auth.as_ref().map(|a| a.username.clone()),
        password: l.auth.as_ref().map(|a| a.password.clone()),
    })
}

fn fault_action(p: &FaultPolicy) -> Action {
    match p {
        FaultPolicy::Direct => Action::Direct,
        FaultPolicy::Block => Action::Block,
    }
}

fn exit_action(p: &ExitPolicy) -> Action {
    match p {
        ExitPolicy::Direct => Action::Direct,
        ExitPolicy::Block => Action::Block,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::core::apply::FailedListener;
    use crate::model::health::Health;
    use crate::model::{Auth, ListenerId};
    use crate::stats::delay::DelayBook;

    const PID: u32 = 4242;
    const EXE: &str = r"C:\Program Files\App\app.exe";

    fn listener(listen: &str) -> Listener {
        Listener {
            routing: None,
            id: ListenerId("l1".into()),
            name: "L1".into(),
            listener_type: ListenerType::Socks,
            listen: listen.into(),
            port: 7891,
            auth: Some(Auth { username: "u".into(), password: "p".into() }),
            exit: None,
            user_enabled: true,
        }
    }

    fn rule(on_fault: FaultPolicy) -> ProcessRule {
        ProcessRule {
            id: ProcessRuleId("r1".into()),
            exe_path: EXE.into(),
            listener: ListenerId("l1".into()),
            on_fault,
            reset_on_change: true,
            user_enabled: true,
        }
    }

    fn model_with(listen: &str, on_fault: FaultPolicy, on_exit: ExitPolicy) -> Model {
        let mut m = Model::default();
        m.listeners.push(listener(listen));
        m.process_routing.enabled = true;
        m.process_routing.on_routa_exit = on_exit;
        m.process_routing.rules.push(rule(on_fault));
        m
    }

    fn model(on_fault: FaultPolicy, on_exit: ExitPolicy) -> Model {
        model_with("0.0.0.0", on_fault, on_exit)
    }

    /// 内核在运行，l1 Active 且生效。
    fn running() -> RuntimeStatus {
        RuntimeStatus {
            core: CoreState::Running,
            last_apply: None,
            health: Health {
                exits: HashMap::new(),
                listeners: HashMap::from([(ListenerId("l1".into()), ListenerStatus::Active)]),
            },
            delays: DelayBook::default(),
            traffic: None,
            interfaces: vec![],
            failed_listeners: vec![],
            effective_listeners: vec![ListenerId("l1".into())],
            startup_error: None,
            save_error: None,
            tailscale: HashMap::new(),
            tailscale_exit_errors: HashMap::new(),
            tailscale_exit_retrying: HashMap::new(),
            core_stop_reason: None,
            core_recovering: false,
        }
    }

    fn status(core: CoreState, reason: Option<CoreStopReason>, recovering: bool) -> RuntimeStatus {
        RuntimeStatus { core, core_stop_reason: reason, core_recovering: recovering, ..running() }
    }

    fn crashed() -> CoreState {
        CoreState::Crashed { exit_code: Some(1) }
    }

    fn only_action(d: &DesiredState) -> Action {
        assert_eq!(d.rules.len(), 1);
        d.rules[0].action.clone()
    }

    fn proxy(host: &str) -> Action {
        Action::Proxy(ProxyTarget {
            kind: ProxyKind::Socks5,
            host: host.into(),
            port: 7891,
            username: Some("u".into()),
            password: Some("p".into()),
        })
    }

    #[test]
    fn proxy_when_listener_effective() {
        let d = decide(&model(FaultPolicy::Direct, ExitPolicy::Direct), &running(), PID);
        assert_eq!(only_action(&d), proxy("127.0.0.1"));
        let r = &d.rules[0];
        assert_eq!((r.id.as_str(), r.exe_path.as_str()), ("r1", EXE));
        assert_eq!((r.on_fault.clone(), r.reset_on_change), (FaultPolicy::Direct, true));
        assert_eq!((d.on_routa_exit, d.routa_pid), (ExitPolicy::Direct, PID));
    }

    #[test]
    fn block_when_listener_disabled_regardless_of_policy() {
        let mut s = running();
        s.health.listeners.insert(ListenerId("l1".into()), ListenerStatus::DisabledByUser);
        let d = decide(&model(FaultPolicy::Direct, ExitPolicy::Direct), &s, PID);
        assert_eq!(only_action(&d), Action::Block);

        // 不在 effective_listeners 里同样阻断。
        let mut s = running();
        s.effective_listeners.clear();
        let d = decide(&model(FaultPolicy::Direct, ExitPolicy::Direct), &s, PID);
        assert_eq!(only_action(&d), Action::Block);
    }

    #[test]
    fn block_when_listener_failed() {
        let mut s = running();
        s.failed_listeners.push(FailedListener { id: ListenerId("l1".into()), name: "L1".into(), port: 7891 });
        let d = decide(&model(FaultPolicy::Direct, ExitPolicy::Direct), &s, PID);
        assert_eq!(only_action(&d), Action::Block);
    }

    #[test]
    fn crash_uses_on_fault_direct() {
        let d = decide(&model(FaultPolicy::Direct, ExitPolicy::Block), &status(crashed(), None, true), PID);
        assert_eq!(only_action(&d), Action::Direct);
    }

    #[test]
    fn crash_uses_on_fault_block() {
        let d = decide(&model(FaultPolicy::Block, ExitPolicy::Direct), &status(crashed(), None, true), PID);
        assert_eq!(only_action(&d), Action::Block);
    }

    #[test]
    fn recovering_start_uses_on_fault() {
        let s = status(CoreState::Starting, None, true);
        let d = decide(&model(FaultPolicy::Direct, ExitPolicy::Block), &s, PID);
        assert_eq!(only_action(&d), Action::Direct);
        let d = decide(&model(FaultPolicy::Block, ExitPolicy::Direct), &s, PID);
        assert_eq!(only_action(&d), Action::Block);
    }

    #[test]
    fn initial_start_uses_exit_policy() {
        let s = status(CoreState::Starting, None, false);
        let d = decide(&model(FaultPolicy::Block, ExitPolicy::Direct), &s, PID);
        assert_eq!(only_action(&d), Action::Direct);
        let d = decide(&model(FaultPolicy::Direct, ExitPolicy::Block), &s, PID);
        assert_eq!(only_action(&d), Action::Block);
    }

    #[test]
    fn user_stop_uses_exit_policy() {
        let s = status(CoreState::Stopped, Some(CoreStopReason::User), false);
        let d = decide(&model(FaultPolicy::Block, ExitPolicy::Direct), &s, PID);
        assert_eq!(only_action(&d), Action::Direct);
        let d = decide(&model(FaultPolicy::Direct, ExitPolicy::Block), &s, PID);
        assert_eq!(only_action(&d), Action::Block);
    }

    #[test]
    fn fail_closed_stop_blocks() {
        let m = model(FaultPolicy::Direct, ExitPolicy::Direct);
        let d = decide(&m, &status(CoreState::Stopped, Some(CoreStopReason::FailClosed), false), PID);
        assert_eq!(only_action(&d), Action::Block);
        // 没有停止原因的 Stopped 也阻断。
        let d = decide(&m, &status(CoreState::Stopped, None, false), PID);
        assert_eq!(only_action(&d), Action::Block);
    }

    #[test]
    fn fail_closed_reason_blocks_even_when_crashed() {
        let s = status(crashed(), Some(CoreStopReason::FailClosed), true);
        let d = decide(&model(FaultPolicy::Direct, ExitPolicy::Direct), &s, PID);
        assert_eq!(only_action(&d), Action::Block);
    }

    #[test]
    fn disabled_rule_omitted() {
        let mut m = model(FaultPolicy::Direct, ExitPolicy::Direct);
        m.process_routing.rules[0].user_enabled = false;
        let mut kept = rule(FaultPolicy::Direct);
        kept.id = ProcessRuleId("r2".into());
        m.process_routing.rules.push(kept);
        let d = decide(&m, &running(), PID);
        assert_eq!(d.rules.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(), ["r2"]);

        let mut m = model(FaultPolicy::Direct, ExitPolicy::Direct);
        m.process_routing.enabled = false;
        let d = decide(&m, &running(), PID);
        assert!(d.rules.is_empty());
        assert_eq!((d.on_routa_exit, d.routa_pid), (ExitPolicy::Direct, PID));
    }

    #[test]
    fn missing_listener_blocks() {
        let mut m = model(FaultPolicy::Direct, ExitPolicy::Direct);
        m.process_routing.rules[0].listener = ListenerId("gone".into());
        let mut s = running();
        s.health.listeners.insert(ListenerId("gone".into()), ListenerStatus::Active);
        s.effective_listeners.push(ListenerId("gone".into()));
        let d = decide(&m, &s, PID);
        assert_eq!(only_action(&d), Action::Block);

        // Http 类型的 Listener 不能用作进程分流目标。
        let mut m = model(FaultPolicy::Direct, ExitPolicy::Direct);
        m.listeners[0].listener_type = ListenerType::Http;
        let d = decide(&m, &running(), PID);
        assert_eq!(only_action(&d), Action::Block);
    }

    #[test]
    fn proxy_host_for_lan_listen_address() {
        let d = decide(&model_with("192.168.1.5", FaultPolicy::Block, ExitPolicy::Block), &running(), PID);
        assert_eq!(only_action(&d), proxy("192.168.1.5"));
        for listen in ["0.0.0.0", "127.0.0.1", "", "localhost", "LocalHost", "[0.0.0.0]"] {
            let d = decide(&model_with(listen, FaultPolicy::Block, ExitPolicy::Block), &running(), PID);
            assert_eq!(only_action(&d), proxy("127.0.0.1"), "listen {listen:?}");
        }
    }

    #[test]
    fn ipv6_listener_blocks_even_when_fault_policy_is_direct() {
        for listen in ["::", "::1", "[::]", " [::1] ", "[fe80::1]", "2001:db8::1", "::ffff:127.0.0.1"] {
            let m = model_with(listen, FaultPolicy::Direct, ExitPolicy::Direct);
            assert_eq!(only_action(&decide(&m, &running(), PID)), Action::Block, "listen {listen:?}");
            assert_eq!(
                reasons(&m, &running())[&ProcessRuleId("r1".into())].as_deref(),
                Some("进程分流仅支持 IPv4 入口，请修改 Listener 的监听地址")
            );
        }
    }

    #[test]
    fn non_ip_listen_blocks() {
        for listen in ["myhost.lan", "[]x", "192.168.1"] {
            let m = model_with(listen, FaultPolicy::Direct, ExitPolicy::Direct);
            let d = decide(&m, &running(), PID);
            assert_eq!(only_action(&d), Action::Block, "listen {listen:?}");
            let why = reasons(&m, &running());
            assert_eq!(why[&ProcessRuleId("r1".into())].as_deref(), Some("进程分流仅支持 IPv4 入口，请修改 Listener 的监听地址"));
        }
    }

    #[test]
    fn reasons_only_for_non_proxy() {
        let m = model(FaultPolicy::Direct, ExitPolicy::Direct);
        assert_eq!(reasons(&m, &running()), HashMap::from([(ProcessRuleId("r1".into()), None)]));
        let why = reasons(&m, &status(crashed(), None, true));
        assert_eq!(why[&ProcessRuleId("r1".into())].as_deref(), Some("内核崩溃"));
    }

    #[test]
    fn rule_order_preserved_and_no_auth_passes_none() {
        let mut m = model(FaultPolicy::Direct, ExitPolicy::Direct);
        m.listeners[0].auth = None;
        let mut second = rule(FaultPolicy::Block);
        second.id = ProcessRuleId("r0".into());
        m.process_routing.rules.push(second);
        let d = decide(&m, &running(), PID);
        assert_eq!(d.rules.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(), ["r1", "r0"]);
        let Action::Proxy(t) = &d.rules[0].action else { panic!("expected proxy") };
        assert_eq!((t.username.clone(), t.password.clone()), (None, None));
    }
}

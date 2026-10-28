//! 每条规则的实际动作（spec 第 5 节）。纯函数，不做任何 IO。

use routa_procsvc_proto::{Action, DesiredState, ExitPolicy, FaultPolicy, ProxyKind, ProxyTarget, RuleSpec};

use crate::core::process::CoreState;
use crate::model::health::ListenerStatus;
use crate::model::{Listener, ListenerType, Model, ProcessRule};
use crate::runtime::{CoreStopReason, RuntimeStatus};

/// 根据模型和运行状态算出下发给助手的期望状态。全局开关关闭或规则被禁用时，
/// 规则不出现在结果里（助手不再管理该进程）；其余规则保持原顺序。
pub fn decide(model: &Model, status: &RuntimeStatus, routa_pid: u32) -> DesiredState {
    let routing = &model.process_routing;
    let rules = if routing.enabled {
        routing
            .rules
            .iter()
            .filter(|r| r.user_enabled)
            .map(|r| RuleSpec {
                id: r.id.0.clone(),
                exe_path: r.exe_path.clone(),
                action: rule_action(model, status, r),
                on_fault: r.on_fault.clone(),
                reset_on_change: r.reset_on_change,
            })
            .collect()
    } else {
        Vec::new()
    };
    DesiredState { rules, on_routa_exit: routing.on_routa_exit.clone(), routa_pid }
}

/// spec 第 5 节，按顺序判断，先命中者生效。
fn rule_action(model: &Model, status: &RuntimeStatus, rule: &ProcessRule) -> Action {
    // 为防泄露自动停止的内核：无论内核状态如何都阻断。
    if status.core_stop_reason == Some(CoreStopReason::FailClosed) {
        return Action::Block;
    }
    match &status.core {
        CoreState::Crashed { .. } => fault_action(&rule.on_fault),
        CoreState::Starting if status.core_recovering => fault_action(&rule.on_fault),
        CoreState::Starting => exit_action(&model.process_routing.on_routa_exit),
        CoreState::Stopped => match status.core_stop_reason {
            Some(CoreStopReason::User) => exit_action(&model.process_routing.on_routa_exit),
            _ => Action::Block,
        },
        CoreState::Running => match model.listeners.iter().find(|l| l.id == rule.listener) {
            Some(l) if listener_in_effect(status, l) => Action::Proxy(proxy_target(l)),
            // Listener 被停用 / 未生效 / 已删除 / 类型不对：一律阻断，与 on_fault 无关。
            _ => Action::Block,
        },
    }
}

fn listener_in_effect(status: &RuntimeStatus, l: &Listener) -> bool {
    l.listener_type != ListenerType::Http
        && status.health.listeners.get(&l.id) == Some(&ListenerStatus::Active)
        && status.effective_listeners.contains(&l.id)
        && !status.failed_listeners.iter().any(|f| f.id == l.id)
}

fn proxy_target(l: &Listener) -> ProxyTarget {
    // 监听所有地址或回环时连 127.0.0.1，只监听某个局域网 IP 时连那个 IP。
    let host = match l.listen.as_str() {
        "0.0.0.0" | "::" | "127.0.0.1" | "" | "localhost" => "127.0.0.1",
        other => other,
    };
    ProxyTarget {
        kind: ProxyKind::Socks5,
        host: host.to_string(),
        port: l.port,
        username: l.auth.as_ref().map(|a| a.username.clone()),
        password: l.auth.as_ref().map(|a| a.password.clone()),
    }
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
    use crate::model::{Auth, ListenerId, ProcessRuleId};
    use crate::stats::delay::DelayBook;

    const PID: u32 = 4242;
    const EXE: &str = r"C:\Program Files\App\app.exe";

    fn listener(listen: &str) -> Listener {
        Listener {
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
        for listen in ["0.0.0.0", "::", "127.0.0.1", "", "localhost"] {
            let d = decide(&model_with(listen, FaultPolicy::Block, ExitPolicy::Block), &running(), PID);
            assert_eq!(only_action(&d), proxy("127.0.0.1"), "listen {listen:?}");
        }
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

//! 每条规则的实际动作（spec 第 5 节）。纯函数，不做任何 IO。

use std::collections::HashMap;

use routa_procsvc_proto::{Action, DesiredState, FaultPolicy, ProcessKind, ProxyKind, ProxyTarget, RuleSpec};

use super::learn;

use crate::core::process::CoreState;
use crate::model::health::ListenerStatus;
use crate::model::hidden::rule_listener;
use crate::model::validate::process_proxy_host;
use crate::model::{Listener, Model, ProcessRule, ProcessRuleId};
use crate::runtime::{CoreStopReason, RuntimeStatus};

/// 根据模型和运行状态算出下发给助手的期望状态。全局开关关闭或规则被禁用时，
/// 规则不出现在结果里（助手不再管理该进程）；其余规则保持原顺序。
///
/// 服务端一条规则对应一个程序：每条规则按进程展开，id 为 [`spec_id`]，用 [`rule_of_spec`] 还原。
pub fn decide(model: &Model, status: &RuntimeStatus, routa_pid: u32) -> DesiredState {
    let rules = active_rules(model)
        .flat_map(|r| {
            let action = rule_action(model, status, r).0;
            r.processes.iter().map(move |p| RuleSpec {
                id: spec_id(&r.id, &p.value),
                exe_path: p.value.clone(),
                kind: p.kind,
                // 只有按进程名阻断的条目需要守护已知位置。
                guard_paths: if p.kind == ProcessKind::Name && r.on_fault == FaultPolicy::Block {
                    learn::guard_paths(model, r, &p.value)
                } else {
                    vec![]
                },
                action: action.clone(),
                on_fault: r.on_fault.clone(),
                reset_on_change: r.reset_on_change,
            })
        })
        .collect();
    DesiredState { rules, routa_pid }
}

/// 规则 `rule` 中进程 `process` 在服务端的规则 id：`<规则 id>|<进程小写>`。
/// 按进程而不是序号命名，增删其中一个进程不会牵动其它进程的服务端规则。
pub fn spec_id(rule: &ProcessRuleId, process: &str) -> String {
    format!("{}|{}", rule.0, process.to_lowercase())
}

/// 服务端规则 id 所属的 Routa 规则 id（[`spec_id`] 的逆）。
pub fn rule_of_spec(spec: &str) -> &str {
    spec.split_once('|').map_or(spec, |(rule, _)| rule)
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
    match &status.core {
        CoreState::Crashed { .. } => (fault_action(&rule.on_fault), Some("内核崩溃")),
        CoreState::Starting if status.core_recovering => (fault_action(&rule.on_fault), Some("内核恢复中")),
        CoreState::Starting => (fault_action(&rule.on_fault), Some("内核启动中")),
        CoreState::Stopped => match status.core_stop_reason {
            Some(CoreStopReason::User) => (fault_action(&rule.on_fault), Some("内核已手动停止")),
            _ => (Action::Block, Some("内核未运行")),
        },
        // 未选节点 / 节点失效（无隐藏监听器或未生效）：一律阻断，与 on_fault 无关。
        CoreState::Running => {
            if rule.node.is_none() {
                return (Action::Block, Some("未选择节点，请选择节点"));
            }
            match rule_listener(model, rule) {
                Some(l) if listener_in_effect(status, l) => match proxy_target(l) {
                    Some(t) => (Action::Proxy(t), None),
                    None => (Action::Block, Some("节点已失效，请更换节点")),
                },
                _ => (Action::Block, Some("节点已失效，请更换节点")),
            }
        }
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

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::core::apply::FailedListener;
    use crate::model::health::Health;
    use crate::model::{ListenerId, Node, NodeRef, ProcessEntry, ProcessKind, Subscription, SubscriptionId};
    use crate::stats::delay::DelayBook;

    const PID: u32 = 4242;
    const EXE: &str = r"C:\Program Files\App\app.exe";

    fn n1() -> NodeRef {
        NodeRef::Subscription { subscription_id: SubscriptionId("s".into()), name: "n1".into() }
    }

    fn rule(on_fault: FaultPolicy) -> ProcessRule {
        ProcessRule {
            id: ProcessRuleId("r1".into()),
            name: String::new(),
            processes: vec![ProcessEntry::path(EXE)],
            learned: vec![],
            listener: ListenerId(String::new()),
            node: Some(n1()),
            interface: None,
            on_fault,
            reset_on_change: true,
            user_enabled: true,
        }
    }

    /// 一条选了节点 n1 的规则；隐藏监听器由 reconcile 生成，id 改成 "l1" 以便与 `running()` 对应。
    fn model(on_fault: FaultPolicy) -> Model {
        let mut m = Model::default();
        m.subscriptions.push(Subscription {
            id: SubscriptionId("s".into()),
            name: "s".into(),
            url: "https://x".into(),
            update_interval_mins: None,
            fallback_exit: None,
            fallback_listener: None,
            last_updated: None,
            last_result: None,
            nodes: vec![Node { name: "n1".into(), proxy_type: "ss".into(), server: "x".into(), raw: serde_json::Value::Null }],
        });
        m.process_routing.enabled = true;
        m.process_routing.rules.push(rule(on_fault));
        crate::model::hidden::reconcile_hidden_listeners(&mut m);
        let l = m.listeners.iter_mut().find(|l| l.hidden).unwrap();
        l.id = ListenerId("l1".into());
        m
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
            port: 37800,
            username: None,
            password: None,
        })
    }

    #[test]
    fn proxy_when_listener_effective() {
        let d = decide(&model(FaultPolicy::Direct), &running(), PID);
        assert_eq!(only_action(&d), proxy("127.0.0.1"));
        let r = &d.rules[0];
        assert_eq!((rule_of_spec(&r.id), r.exe_path.as_str()), ("r1", EXE));
        assert_eq!((r.kind, r.guard_paths.is_empty()), (ProcessKind::Path, true));
        assert_eq!((r.on_fault.clone(), r.reset_on_change), (FaultPolicy::Direct, true));
        assert_eq!(d.routa_pid, PID);
    }

    #[test]
    fn block_when_listener_disabled_regardless_of_policy() {
        let mut s = running();
        s.health.listeners.insert(ListenerId("l1".into()), ListenerStatus::DisabledByUser);
        let d = decide(&model(FaultPolicy::Direct), &s, PID);
        assert_eq!(only_action(&d), Action::Block);

        // 不在 effective_listeners 里同样阻断。
        let mut s = running();
        s.effective_listeners.clear();
        let d = decide(&model(FaultPolicy::Direct), &s, PID);
        assert_eq!(only_action(&d), Action::Block);
    }

    #[test]
    fn block_when_listener_failed() {
        let mut s = running();
        s.failed_listeners.push(FailedListener { id: ListenerId("l1".into()), name: "L1".into(), port: 37800 });
        let d = decide(&model(FaultPolicy::Direct), &s, PID);
        assert_eq!(only_action(&d), Action::Block);
    }

    #[test]
    fn crash_uses_on_fault_direct() {
        let d = decide(&model(FaultPolicy::Direct), &status(crashed(), None, true), PID);
        assert_eq!(only_action(&d), Action::Direct);
    }

    #[test]
    fn crash_uses_on_fault_block() {
        let d = decide(&model(FaultPolicy::Block), &status(crashed(), None, true), PID);
        assert_eq!(only_action(&d), Action::Block);
    }

    #[test]
    fn recovering_start_uses_on_fault() {
        let s = status(CoreState::Starting, None, true);
        let d = decide(&model(FaultPolicy::Direct), &s, PID);
        assert_eq!(only_action(&d), Action::Direct);
        let d = decide(&model(FaultPolicy::Block), &s, PID);
        assert_eq!(only_action(&d), Action::Block);
    }

    #[test]
    fn user_stop_and_starting_follow_on_fault() {
        let start = status(CoreState::Starting, None, false);
        let stop = status(CoreState::Stopped, Some(CoreStopReason::User), false);
        for s in [&start, &stop] {
            let d = decide(&model(FaultPolicy::Direct), s, PID);
            assert_eq!(only_action(&d), Action::Direct);
            let d = decide(&model(FaultPolicy::Block), s, PID);
            assert_eq!(only_action(&d), Action::Block);
        }
    }

    #[test]
    fn fail_closed_stop_blocks() {
        let m = model(FaultPolicy::Direct);
        let d = decide(&m, &status(CoreState::Stopped, Some(CoreStopReason::FailClosed), false), PID);
        assert_eq!(only_action(&d), Action::Block);
        // 没有停止原因的 Stopped 也阻断。
        let d = decide(&m, &status(CoreState::Stopped, None, false), PID);
        assert_eq!(only_action(&d), Action::Block);
    }

    #[test]
    fn fail_closed_reason_blocks_even_when_crashed() {
        let s = status(crashed(), Some(CoreStopReason::FailClosed), true);
        let d = decide(&model(FaultPolicy::Direct), &s, PID);
        assert_eq!(only_action(&d), Action::Block);
    }

    #[test]
    fn disabled_rule_omitted() {
        let mut m = model(FaultPolicy::Direct);
        m.process_routing.rules[0].user_enabled = false;
        let mut kept = rule(FaultPolicy::Direct);
        kept.id = ProcessRuleId("r2".into());
        m.process_routing.rules.push(kept);
        let d = decide(&m, &running(), PID);
        assert_eq!(d.rules.iter().map(|r| rule_of_spec(&r.id)).collect::<Vec<_>>(), ["r2"]);

        let mut m = model(FaultPolicy::Direct);
        m.process_routing.enabled = false;
        let d = decide(&m, &running(), PID);
        assert!(d.rules.is_empty());
        assert_eq!(d.routa_pid, PID);
    }

    #[test]
    fn rule_without_node_blocks_regardless_of_fault_policy() {
        let mut m = model(FaultPolicy::Direct);
        m.process_routing.rules[0].node = None;
        let d = decide(&m, &running(), PID);
        assert_eq!(only_action(&d), Action::Block);
        assert_eq!(reasons(&m, &running())[&ProcessRuleId("r1".into())].as_deref(), Some("未选择节点，请选择节点"));
    }

    #[test]
    fn rule_with_missing_or_inactive_node_listener_blocks() {
        // 选了节点但没有隐藏监听器。
        let mut m = model(FaultPolicy::Direct);
        m.listeners.clear();
        assert_eq!(only_action(&decide(&m, &running(), PID)), Action::Block);
        assert_eq!(reasons(&m, &running())[&ProcessRuleId("r1".into())].as_deref(), Some("节点已失效，请更换节点"));

        // 有隐藏监听器但出口损坏。
        let m = model(FaultPolicy::Direct);
        let mut s = running();
        s.health.listeners.insert(ListenerId("l1".into()), ListenerStatus::ExitBroken { exit_status: crate::model::health::ExitStatus::NodeMissing });
        assert_eq!(only_action(&decide(&m, &s, PID)), Action::Block);
        assert_eq!(reasons(&m, &s)[&ProcessRuleId("r1".into())].as_deref(), Some("节点已失效，请更换节点"));
    }

    #[test]
    fn rule_with_active_hidden_listener_proxies_to_loopback_socks() {
        let d = decide(&model(FaultPolicy::Block), &running(), PID);
        assert_eq!(only_action(&d), proxy("127.0.0.1"));
    }

    #[test]
    fn reasons_only_for_non_proxy() {
        let m = model(FaultPolicy::Direct);
        assert_eq!(reasons(&m, &running()), HashMap::from([(ProcessRuleId("r1".into()), None)]));
        let why = reasons(&m, &status(crashed(), None, true));
        assert_eq!(why[&ProcessRuleId("r1".into())].as_deref(), Some("内核崩溃"));
    }

    #[test]
    fn each_process_becomes_one_spec() {
        let mut m = model(FaultPolicy::Direct);
        m.process_routing.rules[0].processes = vec![ProcessEntry::name("ChatGPT.exe"), ProcessEntry::name("codex.exe")];
        let d = decide(&m, &running(), PID);
        let ids: Vec<&str> = d.rules.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, ["r1|chatgpt.exe", "r1|codex.exe"]);
        assert_eq!(d.rules.iter().map(|r| r.exe_path.as_str()).collect::<Vec<_>>(), ["ChatGPT.exe", "codex.exe"]);
        assert!(d.rules.iter().all(|r| r.kind == ProcessKind::Name && r.guard_paths.is_empty()));
        assert!(d.rules.iter().all(|r| r.action == proxy("127.0.0.1")));
        assert_eq!(rule_of_spec("r1|chatgpt.exe"), "r1");
        assert_eq!(rule_of_spec("plain"), "plain");
    }

    #[test]
    fn rule_order_preserved_and_no_auth_passes_none() {
        let mut m = model(FaultPolicy::Direct);
        let mut second = rule(FaultPolicy::Block);
        second.id = ProcessRuleId("r0".into());
        m.process_routing.rules.push(second);
        let d = decide(&m, &running(), PID);
        assert_eq!(d.rules.iter().map(|r| rule_of_spec(&r.id)).collect::<Vec<_>>(), ["r1", "r0"]);
        let Action::Proxy(t) = &d.rules[0].action else { panic!("expected proxy") };
        assert_eq!((t.username.clone(), t.password.clone()), (None, None));
    }

    fn name_block_model() -> Model {
        let mut m = model(FaultPolicy::Block);
        let r = &mut m.process_routing.rules[0];
        r.processes = vec![ProcessEntry::name("ChatGPT.exe")];
        r.learned = vec![r"C:\A\ChatGPT.exe".into(), r"C:\B\other.exe".into()];
        m
    }

    #[test]
    fn name_block_rule_sends_kind_and_guard_paths() {
        let d = decide(&name_block_model(), &running(), PID);
        assert_eq!(d.rules.len(), 1);
        assert_eq!(d.rules[0].kind, ProcessKind::Name);
        assert_eq!(d.rules[0].guard_paths, vec![r"C:\A\ChatGPT.exe".to_string()]);
    }

    #[test]
    fn guard_paths_yield_to_enabled_path_rules() {
        let mut m = name_block_model();
        let mut other = rule(FaultPolicy::Direct);
        other.id = ProcessRuleId("r2".into());
        other.processes = vec![ProcessEntry::path(r"c:/a/chatgpt.exe")];
        m.process_routing.rules.push(other);
        let d = decide(&m, &running(), PID);
        assert!(d.rules[0].guard_paths.is_empty());

        m.process_routing.rules[1].user_enabled = false;
        let d = decide(&m, &running(), PID);
        assert_eq!(d.rules[0].guard_paths, vec![r"C:\A\ChatGPT.exe".to_string()]);
    }

    #[test]
    fn direct_rule_sends_no_guard_paths() {
        let mut m = name_block_model();
        m.process_routing.rules[0].on_fault = FaultPolicy::Direct;
        let d = decide(&m, &running(), PID);
        assert_eq!(d.rules[0].kind, ProcessKind::Name);
        assert!(d.rules[0].guard_paths.is_empty());
    }

    #[test]
    fn path_entry_kind_is_path() {
        let mut m = name_block_model();
        m.process_routing.rules[0].processes = vec![ProcessEntry::path(r"C:\A\ChatGPT.exe")];
        let d = decide(&m, &running(), PID);
        assert_eq!(d.rules[0].kind, ProcessKind::Path);
        assert!(d.rules[0].guard_paths.is_empty());
    }
}

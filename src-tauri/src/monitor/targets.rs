//! Expands the monitor members into the proxies to probe, and finds the
//! nodes currently in use (the default members, and who uses each target).

use serde::Serialize;

use super::TargetSpec;
use crate::model::hidden::rule_listener;
use crate::model::{expand, manual_proxy_name, resolve, Exit, ExitKind, GroupStrategy, Model, NodeRef, ProcessRule, Resolved};

/// Targets in `members` order, each probed through the node's own proxy.
/// Empty while monitoring is off.
pub fn monitor_targets(model: &Model) -> Vec<TargetSpec> {
    let cfg = &model.settings.node_monitor;
    if !cfg.enabled {
        return vec![];
    }
    let uses = uses(model);
    let mut out: Vec<TargetSpec> = Vec::new();
    for r in expand(model, &cfg.members).nodes {
        let (Some(proxy), Some(label)) = (own_proxy_name(model, &r), node_name(model, &r)) else { continue };
        if out.iter().any(|t| t.proxy == proxy) {
            continue;
        }
        let mut used_by: Vec<String> = Vec::new();
        for u in uses.iter().filter(|u| u.node.as_ref() == Some(&r)) {
            if !used_by.contains(&u.user) {
                used_by.push(u.user.clone());
            }
        }
        out.push(TargetSpec { proxy, node: Some(r), label, used_by });
    }
    out
}

/// What proxies and process routing use right now: the default monitor members.
#[derive(Serialize, Clone, Debug, PartialEq, Default, specta::Type)]
pub struct InUseNodes {
    /// Nodes in listener then process-rule order, without duplicates.
    pub nodes: Vec<NodeRef>,
    /// Auto-selecting group exits in use: they have no single node to monitor.
    pub auto_exits: Vec<String>,
}

pub fn in_use_nodes(model: &Model) -> InUseNodes {
    let mut out = InUseNodes::default();
    for u in uses(model) {
        match u.node {
            Some(n) if !out.nodes.contains(&n) => out.nodes.push(n),
            Some(_) => {}
            None if !out.auto_exits.contains(&u.exit) => out.auto_exits.push(u.exit),
            None => {}
        }
    }
    out
}

/// Display name of a node: the subscription node's name or the manual node's name.
fn node_name(model: &Model, r: &NodeRef) -> Option<String> {
    match resolve(model, r)? {
        Resolved::Tailscale { node, .. } => Some(node.name.clone()),
        Resolved::Copyable { .. } => match r {
            NodeRef::Subscription { name, .. } => Some(name.clone()),
            NodeRef::Manual { manual_id } => model.manual_nodes.iter().find(|n| &n.id == manual_id).map(|n| n.name.clone()),
        },
    }
}

/// The node's own proxy name, outside any exit: `node_proxy_name` /
/// `manual_proxy_name`; Tailscale is always `manual_proxy_name`.
fn own_proxy_name(model: &Model, r: &NodeRef) -> Option<String> {
    match resolve(model, r)? {
        Resolved::Copyable { base_name, .. } => Some(base_name),
        Resolved::Tailscale { node, .. } => Some(manual_proxy_name(node)),
    }
}

/// One user of an exit: `node` is what the exit routes through, `None` for
/// an auto-selecting group.
struct Use {
    node: Option<NodeRef>,
    exit: String,
    user: String,
}

/// The node an exit routes through: a node exit's node, a select group's
/// chosen member; `None` (auto group) / skipped (direct) otherwise.
fn exit_node(model: &Model, exit: &Exit) -> Option<Option<NodeRef>> {
    match &exit.kind {
        ExitKind::Direct => None,
        ExitKind::Node { node } => Some(Some(node.clone())),
        ExitKind::Group { strategy: GroupStrategy::Select, selected: Some(n), .. } if resolve(model, n).is_some() => {
            Some(Some(n.clone()))
        }
        ExitKind::Group { .. } => Some(None),
    }
}

fn rule_label(r: &ProcessRule) -> &str {
    if r.name.is_empty() {
        r.processes.first().map_or("", |p| p.value.as_str())
    } else {
        &r.name
    }
}

/// Enabled listeners, then enabled process rules (when routing is on).
fn uses(model: &Model) -> Vec<Use> {
    let find_exit = |id: &crate::model::ExitId| model.exits.iter().find(|e| &e.id == id);
    let mut exits: Vec<(&Exit, String)> = Vec::new();
    for l in model.visible_listeners().filter(|l| l.user_enabled) {
        if let Some(e) = l.exit.as_ref().and_then(find_exit) {
            exits.push((e, format!("{} :{}", l.name, l.port)));
        }
    }
    if model.process_routing.enabled {
        for r in model.process_routing.rules.iter().filter(|r| r.user_enabled && r.node.is_some()) {
            if let Some(e) = rule_listener(model, r).and_then(|l| l.exit.as_ref()).and_then(find_exit) {
                exits.push((e, format!("进程分流 · {}", rule_label(r))));
            }
        }
    }
    exits
        .into_iter()
        .filter_map(|(e, user)| Some(Use { node: exit_node(model, e)?, exit: e.name.clone(), user }))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;

    fn sub(id: &str, names: &[&str]) -> Subscription {
        Subscription {
            id: SubscriptionId(id.into()),
            name: format!("订阅{id}"),
            nodes: names
                .iter()
                .map(|n| Node { name: (*n).into(), proxy_type: "ss".into(), server: "x".into(), raw: serde_json::json!({"name": n, "type": "ss"}) })
                .collect(),
            ..Subscription::default()
        }
    }

    fn nref(name: &str) -> NodeRef {
        NodeRef::Subscription { subscription_id: SubscriptionId("s".into()), name: name.into() }
    }

    fn exit(id: &str, kind: ExitKind) -> Exit {
        Exit { id: ExitId(id.into()), name: id.into(), kind, interface: None, hidden: false }
    }

    fn listener(name: &str, port: u16, exit: &str) -> Listener {
        let mut l = Listener::new_default(name.into(), port);
        l.id = ListenerId(name.into());
        l.exit = Some(ExitId(exit.into()));
        l
    }

    fn model(exits: Vec<Exit>, listeners: Vec<Listener>) -> Model {
        let mut m = Model { subscriptions: vec![sub("s", &["甲", "乙", "丙"])], exits, listeners, ..Model::default() };
        m.settings.node_monitor.enabled = true;
        m
    }

    #[test]
    fn in_use_follows_node_exits_and_select_groups() {
        let select = ExitKind::Group {
            strategy: GroupStrategy::Select,
            members: vec![ExitMember::Node(nref("甲")), ExitMember::Node(nref("乙"))],
            selected: Some(nref("乙")),
        };
        let auto = ExitKind::Group { strategy: GroupStrategy::UrlTest, members: vec![ExitMember::Node(nref("丙"))], selected: None };
        let m = model(
            vec![exit("香港", ExitKind::Node { node: nref("甲") }), exit("选择", select), exit("自动", auto), exit("直连", ExitKind::Direct)],
            vec![listener("A", 7890, "香港"), listener("B", 7891, "选择"), listener("C", 7892, "自动"), listener("D", 7893, "直连"), listener("E", 7894, "香港")],
        );
        assert_eq!(in_use_nodes(&m), InUseNodes { nodes: vec![nref("甲"), nref("乙")], auto_exits: vec!["自动".into()] });
    }

    #[test]
    fn in_use_skips_disabled_listener_and_routing_off() {
        let mut off = listener("关闭", 7891, "节点");
        off.user_enabled = false;
        let mut m = model(vec![exit("节点", ExitKind::Node { node: nref("甲") }), exit("隐藏", ExitKind::Node { node: nref("乙") })], vec![off]);
        m.exits[1].hidden = true;
        let mut hidden = listener("__proc_x", 37800, "隐藏");
        hidden.hidden = true;
        m.listeners.push(hidden);
        m.process_routing.enabled = false;
        m.process_routing.rules.push(ProcessRule { name: "微信".into(), node: Some(nref("乙")), ..ProcessRule::default() });
        assert_eq!(in_use_nodes(&m), InUseNodes::default());
    }

    #[test]
    fn targets_carry_every_user_of_the_node() {
        let mut m = model(vec![exit("隐藏", ExitKind::Node { node: nref("甲") })], vec![]);
        m.exits[0].hidden = true;
        let mut hidden = listener("__proc_x", 37800, "隐藏");
        hidden.hidden = true;
        m.listeners.push(listener("工作代理", 7890, "隐藏"));
        m.listeners.push(hidden);
        m.process_routing.enabled = true;
        m.process_routing.rules.push(ProcessRule {
            processes: vec![ProcessEntry::name("WeChat.exe")],
            node: Some(nref("甲")),
            ..ProcessRule::default()
        });
        m.settings.node_monitor.members = vec![ExitMember::Node(nref("甲")), ExitMember::Node(nref("乙"))];
        let t = monitor_targets(&m);
        assert_eq!(t[0].proxy, "订阅s/甲");
        assert_eq!(t[0].used_by, vec!["工作代理 :7890".to_string(), "进程分流 · WeChat.exe".to_string()]);
        assert!(t[1].used_by.is_empty());

        m.process_routing.rules[0].name = "微信".into();
        assert_eq!(monitor_targets(&m)[0].used_by[1], "进程分流 · 微信");
    }

    #[test]
    fn members_expand_groups_and_skip_missing() {
        let mut m = model(vec![], vec![]);
        m.subscriptions.push(sub("t", &["丁"]));
        m.manual_nodes.push(ManualNode {
            id: ManualNodeId("m1".into()),
            name: "自建".into(),
            groups: vec![],
            kind: ManualNodeKind::Proxy { raw: serde_json::json!({"type": "ss"}) },
        });
        m.settings.node_monitor.members = vec![
            ExitMember::Node(nref("乙")),
            ExitMember::Node(nref("已删除")),
            ExitMember::Node(NodeRef::Manual { manual_id: ManualNodeId("m1".into()) }),
            ExitMember::Group { group: GroupRef::Subscription(SubscriptionId("t".into())) },
            ExitMember::Group { group: GroupRef::Subscription(SubscriptionId("gone".into())) },
            ExitMember::Node(nref("乙")),
        ];
        let t = monitor_targets(&m);
        let got: Vec<_> = t.iter().map(|t| (t.proxy.as_str(), t.label.as_str())).collect();
        assert_eq!(got, vec![("订阅s/乙", "乙"), ("手动/自建", "自建"), ("订阅t/丁", "丁")]);
        assert_eq!(t[1].node, Some(NodeRef::Manual { manual_id: ManualNodeId("m1".into()) }));
    }

    #[test]
    fn disabled_returns_empty() {
        let mut m = model(vec![], vec![]);
        m.settings.node_monitor.members = vec![ExitMember::Node(nref("甲"))];
        m.settings.node_monitor.enabled = false;
        assert!(monitor_targets(&m).is_empty());
    }
}

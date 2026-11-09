//! Expands the monitor settings into the proxies to probe.

use super::TargetSpec;
use crate::config_gen::member_proxy_name;
use crate::model::health::Health;
use crate::model::hidden::rule_listener;
use crate::model::{
    expand, manual_proxy_name, resolve, Exit, ExitKind, GroupStrategy, InterfaceGuid, Model, MonitorScope, NodeRef,
    ProcessRule, Resolved,
};
use crate::netif::NetInterface;

/// Targets in display order: InUse follows listeners then process rules,
/// Custom follows `members`. Empty while monitoring is off.
pub fn monitor_targets(model: &Model, health: &Health, nics: &[NetInterface]) -> Vec<TargetSpec> {
    let cfg = &model.settings.node_monitor;
    if !cfg.enabled {
        return vec![];
    }
    match cfg.scope {
        MonitorScope::InUse => in_use(model, health, nics),
        MonitorScope::Custom => custom(model),
    }
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

/// The node's own proxy name, outside any exit: `base_name` is
/// `node_proxy_name` / `manual_proxy_name`; Tailscale is always `manual_proxy_name`.
fn own_proxy_name(model: &Model, r: &NodeRef) -> Option<String> {
    match resolve(model, r)? {
        Resolved::Copyable { base_name, .. } => Some(base_name),
        Resolved::Tailscale { node, .. } => Some(manual_proxy_name(node)),
    }
}

fn custom(model: &Model) -> Vec<TargetSpec> {
    let mut out: Vec<TargetSpec> = Vec::new();
    for r in expand(model, &model.settings.node_monitor.members).nodes {
        let (Some(proxy), Some(label)) = (own_proxy_name(model, &r), node_name(model, &r)) else { continue };
        if !out.iter().any(|t| t.proxy == proxy) {
            out.push(TargetSpec { proxy, node: Some(r), label, used_by: vec![] });
        }
    }
    out
}

/// One probed path while collecting InUse targets.
struct Path {
    proxy: String,
    node: Option<NodeRef>,
    name: String,
    /// Friendly name of the outbound adapter; `None` = default.
    iface: Option<String>,
    used_by: Vec<String>,
}

fn interface_name(guid: &Option<InterfaceGuid>, nics: &[NetInterface]) -> Option<String> {
    let g = guid.as_ref()?;
    Some(nics.iter().find(|n| &n.guid == g).map_or_else(|| g.0.clone(), |n| n.friendly_name.clone()))
}

/// Adapter the exit's traffic to `node` leaves through: Tailscale nodes use
/// their instance's adapter, everything else the exit's (resolved in `health`).
fn path_interface(model: &Model, health: &Health, nics: &[NetInterface], exit: &Exit, node: &NodeRef) -> Option<String> {
    match resolve(model, node) {
        Some(Resolved::Tailscale { opts, .. }) => interface_name(&opts.interface, nics),
        _ => match health.exits.get(&exit.id).and_then(|h| h.interface.as_ref()) {
            Some(n) => Some(n.friendly_name.clone()),
            None => interface_name(&exit.interface, nics),
        },
    }
}

fn exit_path(model: &Model, health: &Health, nics: &[NetInterface], exit: &Exit) -> Option<Path> {
    let node_path = |proxy: String, n: &NodeRef| Path {
        proxy,
        node: Some(n.clone()),
        name: node_name(model, n).unwrap_or_else(|| exit.name.clone()),
        iface: path_interface(model, health, nics, exit, n),
        used_by: vec![],
    };
    match &exit.kind {
        ExitKind::Direct => None,
        ExitKind::Node { node } => Some(node_path(exit.name.clone(), node)),
        ExitKind::Group { strategy: GroupStrategy::Select, selected: Some(n), .. } if resolve(model, n).is_some() => {
            let proxy = match resolve(model, n)? {
                Resolved::Copyable { base_name, .. } => member_proxy_name(exit, &base_name),
                Resolved::Tailscale { node, .. } => manual_proxy_name(node),
            };
            Some(node_path(proxy, n))
        }
        ExitKind::Group { .. } => {
            Some(Path { proxy: exit.name.clone(), node: None, name: exit.name.clone(), iface: None, used_by: vec![] })
        }
    }
}

fn rule_label(r: &ProcessRule) -> &str {
    if r.name.is_empty() {
        r.processes.first().map_or("", |p| p.value.as_str())
    } else {
        &r.name
    }
}

fn in_use(model: &Model, health: &Health, nics: &[NetInterface]) -> Vec<TargetSpec> {
    let find_exit = |id: &crate::model::ExitId| model.exits.iter().find(|e| &e.id == id);
    let mut uses: Vec<(&Exit, String)> = Vec::new();
    for l in model.visible_listeners().filter(|l| l.user_enabled) {
        if let Some(e) = l.exit.as_ref().and_then(find_exit) {
            uses.push((e, format!("{} :{}", l.name, l.port)));
        }
    }
    if model.process_routing.enabled {
        for r in model.process_routing.rules.iter().filter(|r| r.user_enabled && r.node.is_some()) {
            if let Some(e) = rule_listener(model, r).and_then(|l| l.exit.as_ref()).and_then(find_exit) {
                uses.push((e, format!("进程分流 · {}", rule_label(r))));
            }
        }
    }

    let mut paths: Vec<Path> = Vec::new();
    for (exit, user) in uses {
        let Some(path) = exit_path(model, health, nics, exit) else { continue };
        let i = match paths.iter().position(|p| p.proxy == path.proxy) {
            Some(i) => i,
            None => {
                paths.push(path);
                paths.len() - 1
            }
        };
        if !paths[i].used_by.contains(&user) {
            paths[i].used_by.push(user);
        }
    }

    paths
        .iter()
        .map(|p| {
            let shared = p.node.is_some() && paths.iter().filter(|q| q.node == p.node).count() > 1;
            let label = if shared {
                format!("{} · 经 {}", p.name, p.iface.as_deref().unwrap_or("默认网卡"))
            } else {
                p.name.clone()
            };
            TargetSpec { proxy: p.proxy.clone(), node: p.node.clone(), label, used_by: p.used_by.clone() }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::health::evaluate;
    use crate::model::*;
    use crate::netif::NetInterface;
    use std::net::Ipv4Addr;

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

    fn exit(id: &str, kind: ExitKind, iface: Option<&str>) -> Exit {
        Exit { id: ExitId(id.into()), name: id.into(), kind, interface: iface.map(|g| InterfaceGuid(g.into())), hidden: false }
    }

    fn listener(name: &str, port: u16, exit: &str) -> Listener {
        let mut l = Listener::new_default(name.into(), port);
        l.id = ListenerId(name.into());
        l.exit = Some(ExitId(exit.into()));
        l
    }

    fn nic(guid: &str, name: &str) -> NetInterface {
        NetInterface {
            guid: InterfaceGuid(guid.into()),
            friendly_name: name.into(),
            description: "d".into(),
            is_up: true,
            ipv4: vec![Ipv4Addr::new(10, 0, 0, 2)],
            ipv6: vec![],
            mihomo_name: name.into(),
        }
    }

    fn model(exits: Vec<Exit>, listeners: Vec<Listener>) -> Model {
        let mut m = Model { subscriptions: vec![sub("s", &["甲", "乙", "丙"])], exits, listeners, ..Model::default() };
        m.settings.node_monitor.enabled = true;
        m
    }

    fn run(m: &Model, nics: &[NetInterface]) -> Vec<TargetSpec> {
        monitor_targets(m, &evaluate(m, nics), nics)
    }

    #[test]
    fn in_use_node_exit_uses_exit_name() {
        let m = model(vec![exit("香港出口", ExitKind::Node { node: nref("甲") }, None)], vec![listener("工作代理", 7890, "香港出口")]);
        assert_eq!(
            run(&m, &[]),
            vec![TargetSpec { proxy: "香港出口".into(), node: Some(nref("甲")), label: "甲".into(), used_by: vec!["工作代理 :7890".into()] }]
        );
    }

    #[test]
    fn in_use_select_group_uses_member_proxy_name() {
        let kind = ExitKind::Group {
            strategy: GroupStrategy::Select,
            members: vec![ExitMember::Node(nref("甲")), ExitMember::Node(nref("乙"))],
            selected: Some(nref("乙")),
        };
        let m = model(vec![exit("选择", kind, None)], vec![listener("工作代理", 7890, "选择")]);
        let t = run(&m, &[]);
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].proxy, "选择/订阅s/乙");
        assert_eq!(t[0].node, Some(nref("乙")));
        assert_eq!(t[0].label, "乙");
    }

    #[test]
    fn in_use_auto_group_monitors_exit_itself() {
        let kind = ExitKind::Group { strategy: GroupStrategy::UrlTest, members: vec![ExitMember::Node(nref("甲"))], selected: None };
        let m = model(vec![exit("自动", kind, None)], vec![listener("工作代理", 7890, "自动")]);
        assert_eq!(
            run(&m, &[]),
            vec![TargetSpec { proxy: "自动".into(), node: None, label: "自动".into(), used_by: vec!["工作代理 :7890".into()] }]
        );
    }

    #[test]
    fn in_use_skips_direct_disabled_listener_and_routing_off() {
        let mut off = listener("关闭", 7891, "节点");
        off.user_enabled = false;
        let mut m = model(
            vec![exit("直连", ExitKind::Direct, None), exit("节点", ExitKind::Node { node: nref("甲") }, None), exit("隐藏", ExitKind::Node { node: nref("乙") }, None)],
            vec![listener("直连代理", 7890, "直连"), off],
        );
        m.exits[2].hidden = true;
        let mut hidden = listener("__proc_x", 37800, "隐藏");
        hidden.hidden = true;
        m.listeners.push(hidden);
        m.process_routing.enabled = false;
        m.process_routing.rules.push(ProcessRule { name: "微信".into(), node: Some(nref("乙")), ..ProcessRule::default() });
        assert!(run(&m, &[]).is_empty());
    }

    #[test]
    fn same_path_used_twice_is_one_target_with_both_users() {
        let mut m = model(vec![exit("隐藏", ExitKind::Node { node: nref("甲") }, None)], vec![]);
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
        m.process_routing.rules[0].name = "微信".into();
        let t = run(&m, &[]);
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].used_by, vec!["工作代理 :7890".to_string(), "进程分流 · 微信".to_string()]);

        // Unnamed rule: first process value.
        m.process_routing.rules[0].name = String::new();
        assert_eq!(run(&m, &[])[0].used_by[1], "进程分流 · WeChat.exe");
    }

    #[test]
    fn same_node_two_interfaces_gets_suffixed_labels() {
        let m = model(
            vec![exit("经网线", ExitKind::Node { node: nref("甲") }, Some("{G1}")), exit("默认", ExitKind::Node { node: nref("甲") }, None)],
            vec![listener("A", 7890, "经网线"), listener("B", 7891, "默认")],
        );
        let t = run(&m, &[nic("{G1}", "Ethernet")]);
        let labels: Vec<_> = t.iter().map(|t| t.label.as_str()).collect();
        assert_eq!(labels, vec!["甲 · 经 Ethernet", "甲 · 经 默认网卡"]);
    }

    #[test]
    fn custom_expands_groups_and_skips_missing() {
        let mut m = model(vec![], vec![]);
        m.subscriptions.push(sub("t", &["丁"]));
        m.manual_nodes.push(ManualNode {
            id: ManualNodeId("m1".into()),
            name: "自建".into(),
            groups: vec![],
            kind: ManualNodeKind::Proxy { raw: serde_json::json!({"type": "ss"}) },
        });
        let s = &mut m.settings.node_monitor;
        s.scope = MonitorScope::Custom;
        s.members = vec![
            ExitMember::Node(nref("乙")),
            ExitMember::Node(nref("已删除")),
            ExitMember::Node(NodeRef::Manual { manual_id: ManualNodeId("m1".into()) }),
            ExitMember::Group { group: GroupRef::Subscription(SubscriptionId("t".into())) },
            ExitMember::Group { group: GroupRef::Subscription(SubscriptionId("gone".into())) },
            ExitMember::Node(nref("乙")),
        ];
        let t = run(&m, &[]);
        let got: Vec<_> = t.iter().map(|t| (t.proxy.as_str(), t.label.as_str(), t.used_by.is_empty())).collect();
        assert_eq!(got, vec![("订阅s/乙", "乙", true), ("手动/自建", "自建", true), ("订阅t/丁", "丁", true)]);
        assert_eq!(t[1].node, Some(NodeRef::Manual { manual_id: ManualNodeId("m1".into()) }));
    }

    #[test]
    fn disabled_returns_empty() {
        let mut m = model(vec![exit("香港出口", ExitKind::Node { node: nref("甲") }, None)], vec![listener("工作代理", 7890, "香港出口")]);
        m.settings.node_monitor.enabled = false;
        assert!(run(&m, &[]).is_empty());
    }
}

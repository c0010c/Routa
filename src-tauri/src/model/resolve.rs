//! Resolves node references and expands exit members against the model.
//! Pure; shared by health derivation and config generation.

use super::types::*;

pub const MANUAL_PREFIX: &str = "手动";

/// mihomo proxy name of a manual node: `手动/<name>`.
pub fn manual_proxy_name(n: &ManualNode) -> String {
    format!("{MANUAL_PREFIX}/{}", n.name)
}

/// Tailscale state dir, relative to the mihomo home.
pub fn tailscale_state_dir(id: &ManualNodeId) -> String {
    format!("tailscale/{}", id.0)
}

pub enum Resolved<'a> {
    /// Copyable per exit; base_name = "<订阅>/<节点>" or "手动/<名>".
    Copyable { base_name: String, raw: &'a serde_json::Value },
    Tailscale { node: &'a ManualNode, opts: &'a TailscaleOpts },
}

pub fn resolve<'a>(model: &'a Model, r: &NodeRef) -> Option<Resolved<'a>> {
    match r {
        NodeRef::Subscription { subscription_id, name } => {
            let sub = model.subscriptions.iter().find(|s| &s.id == subscription_id)?;
            let node = sub.nodes.iter().find(|n| &n.name == name)?;
            Some(Resolved::Copyable { base_name: format!("{}/{}", sub.name, node.name), raw: &node.raw })
        }
        NodeRef::Manual { manual_id } => {
            let node = model.manual_nodes.iter().find(|n| &n.id == manual_id)?;
            match &node.kind {
                ManualNodeKind::Proxy { raw } => {
                    Some(Resolved::Copyable { base_name: manual_proxy_name(node), raw })
                }
                ManualNodeKind::Tailscale(opts) => Some(Resolved::Tailscale { node, opts }),
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Expansion {
    pub nodes: Vec<NodeRef>,
    pub missing: Vec<ExitMember>,
}

fn push_unique(nodes: &mut Vec<NodeRef>, r: NodeRef) {
    if !nodes.contains(&r) {
        nodes.push(r);
    }
}

/// Expands members in order. A subscription group is all of that subscription's
/// current nodes; a custom group is the manual nodes whose `groups` contain its
/// id (in `manual_nodes` order). First occurrence wins on duplicates. Unresolvable
/// single nodes and unknown groups go to `missing`.
pub fn expand(model: &Model, members: &[ExitMember]) -> Expansion {
    let mut nodes: Vec<NodeRef> = Vec::new();
    let mut missing: Vec<ExitMember> = Vec::new();
    for m in members {
        match m {
            ExitMember::Node(r) => {
                if resolve(model, r).is_some() {
                    push_unique(&mut nodes, r.clone());
                } else {
                    missing.push(m.clone());
                }
            }
            ExitMember::Group { group: GroupRef::Subscription(sid) } => {
                match model.subscriptions.iter().find(|s| &s.id == sid) {
                    Some(s) => {
                        for n in &s.nodes {
                            push_unique(
                                &mut nodes,
                                NodeRef::Subscription { subscription_id: sid.clone(), name: n.name.clone() },
                            );
                        }
                    }
                    None => missing.push(m.clone()),
                }
            }
            ExitMember::Group { group: GroupRef::Custom(gid) } => {
                if let Some(g) = model.node_groups.iter().find(|g| &g.id == gid) {
                    for r in &g.members {
                        if resolve(model, r).is_some() { push_unique(&mut nodes, r.clone()); }
                    }
                    for n in model.manual_nodes.iter().filter(|n| n.groups.contains(gid)) {
                        push_unique(&mut nodes, NodeRef::Manual { manual_id: n.id.clone() });
                    }
                } else {
                    missing.push(m.clone());
                }
            }
        }
    }
    Expansion { nodes, missing }
}

/// Tailscale instances an exit actually uses (Node and Group exits alike).
pub fn exit_tailscales<'a>(model: &'a Model, exit: &Exit) -> Vec<&'a ManualNode> {
    let refs: Vec<NodeRef> = match &exit.kind {
        ExitKind::Node { node } => vec![node.clone()],
        ExitKind::Group { members, .. } => expand(model, members).nodes,
        ExitKind::Direct => vec![],
    };
    let mut out: Vec<&ManualNode> = Vec::new();
    for r in &refs {
        if let Some(Resolved::Tailscale { node, .. }) = resolve(model, r) {
            if !out.iter().any(|n| n.id == node.id) {
                out.push(node);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mnode(id: &str, name: &str, groups: &[&str]) -> ManualNode {
        ManualNode {
            id: ManualNodeId(id.into()),
            name: name.into(),
            groups: groups.iter().map(|g| NodeGroupId((*g).into())).collect(),
            kind: ManualNodeKind::Proxy { raw: serde_json::json!({"type": "ss"}) },
        }
    }

    fn mref(id: &str) -> NodeRef {
        NodeRef::Manual { manual_id: ManualNodeId(id.into()) }
    }

    fn sub(id: &str, names: &[&str]) -> Subscription {
        Subscription {
            fallback_listener: None,
            id: SubscriptionId(id.into()),
            name: id.into(),
            url: "https://example.com".into(),
            update_interval_mins: None,
            fallback_exit: None,
            last_updated: None,
            last_result: None,
            nodes: names
                .iter()
                .map(|n| Node {
                    name: (*n).into(),
                    proxy_type: "ss".into(),
                    server: "x".into(),
                    raw: serde_json::Value::Null,
                })
                .collect(),
        }
    }

    #[test]
    fn custom_group_expands_across_subscription_and_manual_sources() {
        let mut model = Model::default();
        let sub = Subscription { name:"source".into(), nodes:vec![Node {name:"node".into(),proxy_type:"http".into(),server:"127.0.0.1".into(),raw:serde_json::json!({"name":"node","type":"http"})}], ..Default::default() };
        let reference=NodeRef::Subscription {subscription_id:sub.id.clone(),name:"node".into()};
        let group=NodeGroup {name:"cross source".into(),members:vec![reference.clone()],..Default::default()};
        let manual=mnode("manual","manual",&[]);
        let mut manual=manual;manual.groups.push(group.id.clone());
        let manual_ref=NodeRef::Manual {manual_id:manual.id.clone()};
        let group_ref=ExitMember::Group {group:GroupRef::Custom(group.id.clone())};
        model.subscriptions.push(sub);model.manual_nodes.push(manual);model.node_groups.push(group);
        assert_eq!(expand(&model,&[group_ref.clone(),ExitMember::Node(reference.clone())]).nodes,vec![reference,manual_ref]);
        model.subscriptions[0].nodes.clear();
        assert_eq!(expand(&model,&[group_ref]).nodes.len(),1);
    }

    #[test]
    fn expand_dedupes_node_also_in_group() {
        let m = Model {
            manual_nodes: vec![mnode("m1", "a", &["g1"]), mnode("m2", "b", &["g1"])],
            node_groups: vec![NodeGroup {members: vec![],  id: NodeGroupId("g1".into()), name: "g".into() }],
            ..Model::default()
        };
        let e = expand(
            &m,
            &[ExitMember::Node(mref("m1")), ExitMember::Group { group: GroupRef::Custom(NodeGroupId("g1".into())) }],
        );
        assert_eq!(e.nodes, vec![mref("m1"), mref("m2")]);
        assert!(e.missing.is_empty());
    }

    #[test]
    fn subscription_group_expands_to_current_nodes() {
        let m = Model { subscriptions: vec![sub("s1", &["HK", "JP"])], ..Model::default() };
        let e = expand(&m, &[ExitMember::Group { group: GroupRef::Subscription(SubscriptionId("s1".into())) }]);
        let r = |n: &str| NodeRef::Subscription { subscription_id: SubscriptionId("s1".into()), name: n.into() };
        assert_eq!(e.nodes, vec![r("HK"), r("JP")]);
    }

    #[test]
    fn missing_group_and_node_reported() {
        let m = Model::default();
        let ms = vec![
            ExitMember::Group { group: GroupRef::Subscription(SubscriptionId("s9".into())) },
            ExitMember::Node(mref("m9")),
        ];
        let e = expand(&m, &ms);
        assert!(e.nodes.is_empty());
        assert_eq!(e.missing, ms);
    }

    #[test]
    fn manual_proxy_name_uses_prefix() {
        assert_eq!(manual_proxy_name(&mnode("m", "家用 socks", &[])), "手动/家用 socks");
    }

    #[test]
    fn resolve_tailscale_vs_proxy() {
        let mut ts = mnode("t", "ts", &[]);
        ts.kind = ManualNodeKind::Tailscale(TailscaleOpts::defaults_for("ts"));
        let m = Model { manual_nodes: vec![mnode("m", "a", &[]), ts], ..Model::default() };
        match resolve(&m, &mref("m")) {
            Some(Resolved::Copyable { base_name, .. }) => assert_eq!(base_name, "手动/a"),
            _ => panic!("expected Copyable"),
        }
        assert!(matches!(resolve(&m, &mref("t")), Some(Resolved::Tailscale { .. })));
        assert!(resolve(&m, &mref("nope")).is_none());
    }
}

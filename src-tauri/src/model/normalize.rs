//! Pure model normalization; wired into the runtime elsewhere.

use super::resolve::{expand, resolve, Resolved};
use super::types::*;

fn tailscale_interface(model: &Model, r: &NodeRef) -> Option<Option<InterfaceGuid>> {
    match resolve(model, r) {
        Some(Resolved::Tailscale { opts, .. }) => Some(opts.interface.clone()),
        _ => None,
    }
}

/// exe 路径规范化：去首尾空白、`/` → `\`。判重时再对结果取 `to_lowercase()`。
pub fn normalize_exe_path(p: &str) -> String {
    p.trim().replace('/', "\\")
}

pub fn normalize(model: &mut Model) {
    for sub in &mut model.subscriptions {
        if let Some(id) = &sub.fallback_listener {
            sub.fallback_exit = model.listeners.iter().find(|l| &l.id == id).and_then(|l| l.exit.clone());
        }
    }
    for r in &mut model.process_routing.rules {
        r.exe_path = normalize_exe_path(&r.exe_path);
    }

    // Compute against the immutable model first, then apply.
    let mut ifaces: Vec<Option<Option<InterfaceGuid>>> = Vec::new();
    let mut selects: Vec<Option<Option<NodeRef>>> = Vec::new();
    for e in &model.exits {
        // (a) exits made only of Tailscale instances sharing one interface follow it.
        let nodes = match &e.kind {
            ExitKind::Node { node } => vec![node.clone()],
            ExitKind::Group { members, .. } => expand(model, members).nodes,
            ExitKind::Direct => vec![],
        };
        let all: Option<Vec<Option<InterfaceGuid>>> = nodes.iter().map(|r| tailscale_interface(model, r)).collect();
        ifaces.push(match all {
            Some(v) if !v.is_empty() && v.iter().all(|i| *i == v[0]) => Some(v[0].clone()),
            _ => None,
        });
        // (b) a Select exit's selection must be one of its expanded nodes.
        selects.push(match &e.kind {
            ExitKind::Group { strategy: GroupStrategy::Select, members, selected } => {
                let nodes = expand(model, members).nodes;
                if selected.as_ref().is_some_and(|s| nodes.contains(s)) {
                    None
                } else {
                    Some(nodes.first().cloned())
                }
            }
            _ => None,
        });
    }
    for ((e, i), s) in model.exits.iter_mut().zip(ifaces).zip(selects) {
        if let Some(i) = i {
            e.interface = i;
        }
        if let (Some(s), ExitKind::Group { selected, .. }) = (s, &mut e.kind) {
            *selected = s;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::validate;

    fn guid(s: &str) -> Option<InterfaceGuid> {
        Some(InterfaceGuid(s.into()))
    }
    fn ts(id: &str, iface: Option<InterfaceGuid>) -> ManualNode {
        let mut o = TailscaleOpts::defaults_for(id);
        o.interface = iface;
        ManualNode {
            id: ManualNodeId(id.into()),
            name: id.into(),
            groups: vec![],
            kind: ManualNodeKind::Tailscale(o),
        }
    }
    fn mref(id: &str) -> NodeRef {
        NodeRef::Manual { manual_id: ManualNodeId(id.into()) }
    }
    fn sref(name: &str) -> NodeRef {
        NodeRef::Subscription { subscription_id: SubscriptionId("s".into()), name: name.into() }
    }
    fn sub_with(node: &str) -> Subscription {
        Subscription {
            fallback_listener: None,
            id: SubscriptionId("s".into()),
            name: "s".into(),
            url: "https://x".into(),
            update_interval_mins: None,
            fallback_exit: None,
            last_updated: None,
            last_result: None,
            nodes: vec![Node {
                name: node.into(),
                proxy_type: "ss".into(),
                server: "x".into(),
                raw: serde_json::Value::Null,
            }],
        }
    }
    fn exit(kind: ExitKind, interface: Option<InterfaceGuid>) -> Exit {
        Exit { id: ExitId::new(), name: "e".into(), kind, interface }
    }

    #[test]
    fn tailscale_only_exit_follows_instance_interface() {
        let mut m = Model::default();
        m.manual_nodes.push(ts("t", guid("{B}")));
        m.exits.push(exit(ExitKind::Node { node: mref("t") }, guid("{A}")));
        normalize(&mut m);
        assert_eq!(m.exits[0].interface, guid("{B}"));
        assert_eq!(validate(&m), Ok(()));
    }

    #[test]
    fn mixed_exit_interface_not_touched() {
        let mut m = Model::default();
        m.manual_nodes.push(ts("t", guid("{B}")));
        m.subscriptions.push(sub_with("HK"));
        m.exits.push(exit(
            ExitKind::Group {
                strategy: GroupStrategy::UrlTest,
                members: vec![ExitMember::Node(sref("HK")),ExitMember::Node(mref("t"))],
                selected: None,
            },
            guid("{A}"),
        ));
        normalize(&mut m);
        assert_eq!(m.exits[0].interface, guid("{A}"));
    }

    #[test]
    fn differing_instance_interfaces_not_touched() {
        let mut m = Model::default();
        m.manual_nodes.push(ts("t1", guid("{B}")));
        m.manual_nodes.push(ts("t2", guid("{C}")));
        m.exits.push(exit(
            ExitKind::Group {
                strategy: GroupStrategy::UrlTest,
                members: vec![ExitMember::Node(mref("t1")), ExitMember::Node(mref("t2"))],
                selected: None,
            },
            guid("{A}"),
        ));
        normalize(&mut m);
        assert_eq!(m.exits[0].interface, guid("{A}"));
    }

    #[test]
    fn select_falls_back_to_first_expanded() {
        let mut m = Model::default();
        m.subscriptions.push(sub_with("HK"));
        let members = vec![ExitMember::Group { group: GroupRef::Subscription(SubscriptionId("s".into())) }];
        let sel = |selected| ExitKind::Group { strategy: GroupStrategy::Select, members: members.clone(), selected };
        m.exits.push(exit(sel(Some(sref("gone"))), None));
        m.exits.push(exit(sel(Some(sref("HK"))), None));
        m.exits.push(exit(sel(None), None));
        normalize(&mut m);
        let got = |m: &Model, i: usize| match &m.exits[i].kind {
            ExitKind::Group { selected, .. } => selected.clone(),
            _ => unreachable!(),
        };
        assert_eq!(got(&m, 0), Some(sref("HK")));
        assert_eq!(got(&m, 1), Some(sref("HK")));
        assert_eq!(got(&m, 2), Some(sref("HK")));

        m.subscriptions[0].nodes.clear();
        normalize(&mut m);
        assert_eq!(got(&m, 0), None);
        assert_eq!(got(&m, 1), None);
    }
}

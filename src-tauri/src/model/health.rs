//! Derives exit and listener health from the model and current adapters.
//! Pure: recomputed on every change, so repairs recover automatically.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::resolve::{expand, resolve};
use super::types::*;
use crate::netif::NetInterface;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, specta::Type)]
pub enum ExitStatus {
    Ok,
    NodeMissing,
    AllMembersMissing,
    InterfaceUnavailable,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, specta::Type)]
pub struct ExitHealth {
    pub status: ExitStatus,
    pub effective_members: Vec<NodeRef>,
    pub missing_members: Vec<ExitMember>,
    /// Resolved adapter; `Some` even when it is not usable.
    pub interface: Option<NetInterface>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, specta::Type)]
pub enum ListenerStatus {
    Active,
    DisabledByUser,
    NoExit,
    ExitBroken { exit_status: ExitStatus },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, specta::Type)]
pub struct Health {
    pub exits: HashMap<ExitId, ExitHealth>,
    pub listeners: HashMap<ListenerId, ListenerStatus>,
}

fn exit_health(model: &Model, exit: &Exit, nics: &[NetInterface]) -> ExitHealth {
    let (effective_members, missing_members): (Vec<NodeRef>, Vec<ExitMember>) = match &exit.kind {
        ExitKind::Node { node } => {
            if resolve(model, node).is_some() {
                (vec![node.clone()], vec![])
            } else {
                (vec![], vec![ExitMember::Node(node.clone())])
            }
        }
        ExitKind::Group { members, .. } => {
            let e = expand(model, members);
            (e.nodes, e.missing)
        }
        ExitKind::Direct => (vec![], vec![]),
    };

    let interface = exit
        .interface
        .as_ref()
        .and_then(|g| nics.iter().find(|n| &n.guid == g))
        .cloned();
    // Configured but absent or unusable: fail closed, never fall back to the default NIC.
    let interface_bad =
        exit.interface.is_some() && !interface.as_ref().is_some_and(|n| n.is_usable());

    let status = if interface_bad {
        ExitStatus::InterfaceUnavailable
    } else {
        match &exit.kind {
            ExitKind::Node { .. } if effective_members.is_empty() => ExitStatus::NodeMissing,
            ExitKind::Group { .. } if effective_members.is_empty() => {
                ExitStatus::AllMembersMissing
            }
            _ => ExitStatus::Ok,
        }
    };

    ExitHealth { status, effective_members, missing_members, interface }
}

pub fn evaluate(model: &Model, nics: &[NetInterface]) -> Health {
    let exits: HashMap<ExitId, ExitHealth> = model
        .exits
        .iter()
        .map(|e| (e.id.clone(), exit_health(model, e, nics)))
        .collect();

    let listeners = model
        .listeners
        .iter()
        .map(|l| {
            let status = if !l.user_enabled {
                ListenerStatus::DisabledByUser
            } else {
                match l.exit.as_ref().and_then(|id| exits.get(id)) {
                    None => ListenerStatus::NoExit,
                    Some(h) if h.status == ExitStatus::Ok => ListenerStatus::Active,
                    Some(h) => ListenerStatus::ExitBroken { exit_status: h.status },
                }
            };
            (l.id.clone(), status)
        })
        .collect();

    Health { exits, listeners }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(name: &str) -> Node {
        Node {
            name: name.into(),
            proxy_type: "ss".into(),
            server: "example.com".into(),
            raw: serde_json::Value::Null,
        }
    }

    fn sub(id: &str, names: &[&str]) -> Subscription {
        Subscription {
            id: SubscriptionId(id.into()),
            name: id.into(),
            url: "https://example.com".into(),
            update_interval_mins: None,
            fallback_exit: None,
            last_updated: None,
            last_result: None,
            nodes: names.iter().map(|n| node(n)).collect(),
        }
    }

    fn nref(sub: &str, name: &str) -> NodeRef {
        NodeRef::Subscription { subscription_id: SubscriptionId(sub.into()), name: name.into() }
    }

    fn exit(id: &str, kind: ExitKind, interface: Option<&str>) -> Exit {
        Exit {
            id: ExitId(id.into()),
            name: id.into(),
            kind,
            interface: interface.map(|g| InterfaceGuid(g.into())),
        }
    }

    fn listener(id: &str, exit: Option<&str>, enabled: bool) -> Listener {
        let mut l = Listener::new_default(id.into(), 7890);
        l.id = ListenerId(id.into());
        l.exit = exit.map(|e| ExitId(e.into()));
        l.user_enabled = enabled;
        l
    }

    fn nic(guid: &str, up: bool, ips: &[&str]) -> NetInterface {
        NetInterface {
            guid: InterfaceGuid(guid.into()),
            friendly_name: "Ethernet".into(),
            description: "d".into(),
            is_up: up,
            ipv4: ips.iter().map(|i| i.parse().unwrap()).collect(),
            ipv6: vec![],
            mihomo_name: "Ethernet".into(),
        }
    }

    fn model(subs: Vec<Subscription>, exits: Vec<Exit>, listeners: Vec<Listener>) -> Model {
        Model { subscriptions: subs, exits, listeners, ..Model::default() }
    }

    fn node_exit(interface: Option<&str>) -> Exit {
        exit("e", ExitKind::Node { node: nref("s", "a") }, interface)
    }

    fn est(m: &Model, nics: &[NetInterface]) -> ExitStatus {
        evaluate(m, nics).exits[&ExitId("e".into())].status
    }

    #[test]
    fn node_exit_missing_node() {
        let m = model(vec![sub("s", &["b"])], vec![node_exit(None)], vec![listener("l", Some("e"), true)]);
        let h = evaluate(&m, &[]);
        assert_eq!(h.exits[&ExitId("e".into())].status, ExitStatus::NodeMissing);
        assert_eq!(
            h.listeners[&ListenerId("l".into())],
            ListenerStatus::ExitBroken { exit_status: ExitStatus::NodeMissing }
        );
    }

    #[test]
    fn group_partial_missing_keeps_ok() {
        let g = exit(
            "e",
            ExitKind::Group {
                strategy: GroupStrategy::Fallback,
                members: [nref("s", "a"), nref("s", "gone"), nref("s", "c")].map(ExitMember::Node).into(),
                selected: None,
            },
            None,
        );
        let m = model(vec![sub("s", &["a", "c"])], vec![g], vec![listener("l", Some("e"), true)]);
        let h = evaluate(&m, &[]);
        let eh = &h.exits[&ExitId("e".into())];
        assert_eq!(eh.status, ExitStatus::Ok);
        assert_eq!(eh.effective_members, vec![nref("s", "a"), nref("s", "c")]);
        assert_eq!(eh.missing_members, vec![ExitMember::Node(nref("s", "gone"))]);
        assert_eq!(h.listeners[&ListenerId("l".into())], ListenerStatus::Active);
    }

    #[test]
    fn group_all_missing() {
        let g = exit(
            "e",
            ExitKind::Group {
                strategy: GroupStrategy::Select,
                members: [nref("s", "x"), nref("zz", "a")].map(ExitMember::Node).into(),
                selected: None,
            },
            None,
        );
        let m = model(vec![sub("s", &["a"])], vec![g], vec![]);
        assert_eq!(est(&m, &[]), ExitStatus::AllMembersMissing);
    }

    #[test]
    fn node_returns_after_update_restores() {
        let mut m = model(vec![sub("s", &["b"])], vec![node_exit(None)], vec![]);
        assert_eq!(est(&m, &[]), ExitStatus::NodeMissing);
        m.subscriptions[0].nodes.push(node("a"));
        assert_eq!(est(&m, &[]), ExitStatus::Ok);
    }

    #[test]
    fn interface_absent() {
        let m = model(vec![sub("s", &["a"])], vec![node_exit(Some("{G1}"))], vec![]);
        let h = evaluate(&m, &[nic("{OTHER}", true, &["10.0.0.2"])]);
        let eh = &h.exits[&ExitId("e".into())];
        assert_eq!(eh.status, ExitStatus::InterfaceUnavailable);
        assert!(eh.interface.is_none());
    }

    #[test]
    fn interface_present_but_no_ip() {
        let m = model(vec![sub("s", &["a"])], vec![node_exit(Some("{G1}"))], vec![]);
        for n in [nic("{G1}", true, &[]), nic("{G1}", true, &["169.254.1.1"]), nic("{G1}", false, &["10.0.0.2"])] {
            let h = evaluate(&m, std::slice::from_ref(&n));
            let eh = &h.exits[&ExitId("e".into())];
            assert_eq!(eh.status, ExitStatus::InterfaceUnavailable);
            assert_eq!(eh.interface, Some(n));
        }
        let ok = nic("{G1}", true, &["10.0.0.2"]);
        assert_eq!(est(&m, &[ok]), ExitStatus::Ok);
    }

    #[test]
    fn interface_and_node_both_bad_reports_interface() {
        let m = model(vec![sub("s", &[])], vec![node_exit(Some("{G1}"))], vec![]);
        assert_eq!(est(&m, &[]), ExitStatus::InterfaceUnavailable);
    }

    #[test]
    fn user_disabled_wins() {
        let m = model(vec![sub("s", &[])], vec![node_exit(Some("{G1}"))], vec![listener("l", Some("e"), false)]);
        let h = evaluate(&m, &[]);
        assert_eq!(h.listeners[&ListenerId("l".into())], ListenerStatus::DisabledByUser);
    }

    #[test]
    fn listener_without_exit() {
        let m = model(
            vec![],
            vec![],
            vec![listener("l1", None, true), listener("l2", Some("ghost"), true)],
        );
        let h = evaluate(&m, &[]);
        assert_eq!(h.listeners[&ListenerId("l1".into())], ListenerStatus::NoExit);
        assert_eq!(h.listeners[&ListenerId("l2".into())], ListenerStatus::NoExit);
    }

    #[test]
    fn exit_on_deleted_subscription_group_is_all_members_missing() {
        let g = exit(
            "e",
            ExitKind::Group {
                strategy: GroupStrategy::Select,
                members: vec![ExitMember::Group { group: GroupRef::Subscription(SubscriptionId("s1".into())) }],
                selected: None,
            },
            None,
        );
        let m = model(vec![], vec![g], vec![listener("l", Some("e"), true)]);
        let h = evaluate(&m, &[]);
        assert_eq!(h.exits[&ExitId("e".into())].status, ExitStatus::AllMembersMissing);
        assert_eq!(
            h.listeners[&ListenerId("l".into())],
            ListenerStatus::ExitBroken { exit_status: ExitStatus::AllMembersMissing }
        );
    }

    #[test]
    fn group_exit_with_manual_member_is_ok() {
        let mn = ManualNode {
            id: ManualNodeId("m1".into()),
            name: "a".into(),
            groups: vec![],
            kind: ManualNodeKind::Proxy { raw: serde_json::json!({"type": "ss"}) },
        };
        let g = exit(
            "e",
            ExitKind::Group {
                strategy: GroupStrategy::Select,
                members: vec![ExitMember::Node(NodeRef::Manual { manual_id: mn.id.clone() })],
                selected: None,
            },
            None,
        );
        let mut m = model(vec![], vec![g], vec![]);
        m.manual_nodes.push(mn);
        assert_eq!(est(&m, &[]), ExitStatus::Ok);
    }

    #[test]
    fn direct_exit_without_interface_ok() {
        let m = model(vec![], vec![exit("e", ExitKind::Direct, None)], vec![listener("l", Some("e"), true)]);
        let h = evaluate(&m, &[]);
        assert_eq!(h.exits[&ExitId("e".into())].status, ExitStatus::Ok);
        assert_eq!(h.listeners[&ListenerId("l".into())], ListenerStatus::Active);
    }
}

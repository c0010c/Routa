//! Pure reconciliation of the hidden SOCKS listeners (and their private exits)
//! that process-routing rules with a directly chosen node rely on.

use std::collections::HashSet;

use super::resolve::{resolve, Resolved};
use super::types::*;

/// Entry label shown for connections that came in through a hidden listener.
pub const HIDDEN_ENTRY_LABEL: &str = "进程分流";

/// Name prefix of hidden listeners (and their exits) inside the mihomo config.
pub const HIDDEN_NAME_PREFIX: &str = "__proc_";

/// Interface actually used for `node`: Tailscale nodes follow their instance's
/// interface, everything else uses the rule's own choice.
pub fn effective_interface(model: &Model, node: &NodeRef, rule_iface: &Option<InterfaceGuid>) -> Option<InterfaceGuid> {
    match resolve(model, node) {
        Some(Resolved::Tailscale { opts, .. }) => opts.interface.clone(),
        _ => rule_iface.clone(),
    }
}

/// The hidden listener whose exit is exactly `node` on `iface`.
pub fn hidden_listener_for<'a>(model: &'a Model, node: &NodeRef, iface: &Option<InterfaceGuid>) -> Option<&'a Listener> {
    model.listeners.iter().find(|l| {
        l.hidden
            && l.exit.as_ref().and_then(|id| model.exits.iter().find(|e| &e.id == id)).is_some_and(|e| {
                matches!(&e.kind, ExitKind::Node { node: n } if n == node) && &e.interface == iface
            })
    })
}

impl Model {
    /// Listeners shown to the user; hidden process-routing ones are left out.
    pub fn visible_listeners(&self) -> impl Iterator<Item = &Listener> {
        self.listeners.iter().filter(|l| !l.hidden)
    }
}

pub fn rule_listener<'a>(model: &'a Model, rule: &ProcessRule) -> Option<&'a Listener> {
    let node = rule.node.as_ref()?;
    let iface = effective_interface(model, node, &rule.interface);
    hidden_listener_for(model, node, &iface)
}

/// First port in `HIDDEN_PORT_RANGE` that is neither the controller port nor
/// used by a listener other than `skip`.
fn free_port(model: &Model, skip: Option<&ListenerId>) -> Option<u16> {
    HIDDEN_PORT_RANGE.clone().find(|p| {
        *p != model.settings.controller_port && !model.listeners.iter().any(|l| Some(&l.id) != skip && l.port == *p)
    })
}

pub fn reconcile_hidden_listeners(model: &mut Model) {
    // Keys (node, effective interface) in rule order, with "any enabled rule".
    let mut keys: Vec<(NodeRef, Option<InterfaceGuid>, bool)> = Vec::new();
    for r in &model.process_routing.rules {
        let Some(node) = &r.node else { continue };
        if resolve(model, node).is_none() {
            continue;
        }
        let iface = effective_interface(model, node, &r.interface);
        match keys.iter_mut().find(|(n, i, _)| n == node && *i == iface) {
            Some(k) => k.2 |= r.user_enabled,
            None => keys.push((node.clone(), iface, r.user_enabled)),
        }
    }

    // Existing hidden listener per key (first match); every other hidden one goes.
    let keep: Vec<Option<ListenerId>> =
        keys.iter().map(|(n, i, _)| hidden_listener_for(model, n, i).map(|l| l.id.clone())).collect();
    let kept: HashSet<ListenerId> = keep.iter().flatten().cloned().collect();
    let removed_exits: HashSet<ExitId> = model
        .listeners
        .iter()
        .filter(|l| l.hidden && !kept.contains(&l.id))
        .filter_map(|l| l.exit.clone())
        .collect();
    model.listeners.retain(|l| !l.hidden || kept.contains(&l.id));
    model.exits.retain(|e| !removed_exits.contains(&e.id));

    // Enable/disable, and move ports off user listeners / the controller port.
    for ((_, _, enabled), id) in keys.iter().zip(&keep) {
        let Some(id) = id else { continue };
        let Some(idx) = model.listeners.iter().position(|l| &l.id == id) else { continue };
        let port = model.listeners[idx].port;
        let clash =
            port == model.settings.controller_port || model.listeners.iter().any(|o| !o.hidden && o.port == port);
        let new_port = if clash { free_port(model, Some(id)) } else { None };
        let l = &mut model.listeners[idx];
        l.user_enabled = *enabled;
        if let Some(p) = new_port {
            l.port = p;
        }
    }

    // Create missing listeners (+ private exits).
    for ((node, iface, enabled), id) in keys.into_iter().zip(keep) {
        if id.is_some() {
            continue;
        }
        let Some(port) = free_port(model, None) else { continue };
        let lid = ListenerId::new();
        let eid = ExitId::new();
        model.exits.push(Exit {
            id: eid.clone(),
            name: format!("{HIDDEN_NAME_PREFIX}exit_{}", lid.0),
            kind: ExitKind::Node { node },
            interface: iface,
            hidden: true,
        });
        model.listeners.push(Listener {
            id: lid.clone(),
            name: format!("{HIDDEN_NAME_PREFIX}{}", lid.0),
            listener_type: ListenerType::Socks,
            listen: "127.0.0.1".into(),
            port,
            auth: None,
            exit: Some(eid),
            user_enabled: enabled,
            routing: None,
            hidden: true,
        });
    }

    // Drop hidden exits no hidden listener refers to.
    let used: HashSet<ExitId> = model.listeners.iter().filter(|l| l.hidden).filter_map(|l| l.exit.clone()).collect();
    model.exits.retain(|e| !e.hidden || used.contains(&e.id));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn g(s: &str) -> InterfaceGuid {
        InterfaceGuid(s.into())
    }
    fn n(name: &str) -> NodeRef {
        NodeRef::Subscription { subscription_id: SubscriptionId("s".into()), name: name.into() }
    }
    fn n1() -> NodeRef {
        n("n1")
    }
    fn ts_ref() -> NodeRef {
        NodeRef::Manual { manual_id: ManualNodeId("ts".into()) }
    }
    fn sub_node(name: &str) -> Node {
        Node { name: name.into(), proxy_type: "ss".into(), server: "x".into(), raw: serde_json::Value::Null }
    }
    fn model() -> Model {
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
            nodes: vec![sub_node("n1"), sub_node("n2")],
        });
        let mut o = TailscaleOpts::defaults_for("ts");
        o.interface = Some(g("A"));
        m.manual_nodes.push(ManualNode {
            id: ManualNodeId("ts".into()),
            name: "ts".into(),
            groups: vec![],
            kind: ManualNodeKind::Tailscale(o),
        });
        m
    }
    fn rule(node: Option<NodeRef>, iface: Option<InterfaceGuid>, enabled: bool) -> ProcessRule {
        ProcessRule { node, interface: iface, user_enabled: enabled, ..Default::default() }
    }
    fn hidden_count(m: &Model) -> usize {
        m.listeners.iter().filter(|l| l.hidden).count()
    }

    #[test]
    fn reconcile_creates_one_listener_per_node_and_shares_it() {
        let mut m = model();
        m.process_routing.rules.push(rule(Some(n1()), None, true));
        m.process_routing.rules.push(rule(Some(n1()), None, true));
        reconcile_hidden_listeners(&mut m);
        let hidden: Vec<_> = m.listeners.iter().filter(|l| l.hidden).collect();
        assert_eq!(hidden.len(), 1);
        let l = hidden[0];
        assert_eq!(
            (l.listener_type.clone(), l.listen.as_str(), l.port, l.user_enabled),
            (ListenerType::Socks, "127.0.0.1", 37800, true)
        );
        assert!(l.auth.is_none() && l.routing.is_none());
        let e = m.exits.iter().find(|e| Some(&e.id) == l.exit.as_ref()).unwrap();
        assert!(e.hidden);
        assert_eq!(e.kind, ExitKind::Node { node: n1() });
        assert_eq!(l.name, format!("__proc_{}", l.id.0));
        assert_eq!(e.name, format!("__proc_exit_{}", l.id.0));
    }

    #[test]
    fn reconcile_keeps_id_and_port_when_other_rules_change() {
        let mut m = model();
        m.process_routing.rules.push(rule(Some(n1()), None, true));
        reconcile_hidden_listeners(&mut m);
        let a = m.process_routing.rules[0].clone();
        let id_before = rule_listener(&m, &a).unwrap().id.clone();
        m.process_routing.rules.push(rule(Some(n("n2")), None, true));
        reconcile_hidden_listeners(&mut m);
        let b = m.process_routing.rules[1].clone();
        assert_eq!(rule_listener(&m, &a).unwrap().id, id_before);
        assert_eq!(rule_listener(&m, &a).unwrap().port, 37800);
        assert_eq!(rule_listener(&m, &b).unwrap().port, 37801);
    }

    #[test]
    fn reconcile_disables_listener_used_only_by_disabled_rules() {
        let mut m = model();
        m.process_routing.rules.push(rule(Some(n1()), None, true));
        reconcile_hidden_listeners(&mut m);
        m.process_routing.rules[0].user_enabled = false;
        reconcile_hidden_listeners(&mut m);
        let r = m.process_routing.rules[0].clone();
        assert!(!rule_listener(&m, &r).unwrap().user_enabled);
        m.process_routing.rules[0].user_enabled = true;
        reconcile_hidden_listeners(&mut m);
        assert!(rule_listener(&m, &r).unwrap().user_enabled);
    }

    #[test]
    fn reconcile_deletes_unreferenced_listener_and_exit() {
        let mut m = model();
        m.process_routing.rules.push(rule(Some(n1()), None, true));
        reconcile_hidden_listeners(&mut m);
        assert_eq!(hidden_count(&m), 1);
        m.process_routing.rules.clear();
        reconcile_hidden_listeners(&mut m);
        assert!(m.listeners.iter().all(|l| !l.hidden));
        assert!(m.exits.iter().all(|e| !e.hidden));
    }

    #[test]
    fn reconcile_drops_listener_for_missing_node_and_recreates_when_back() {
        let mut m = model();
        m.process_routing.rules.push(rule(Some(n1()), None, true));
        reconcile_hidden_listeners(&mut m);
        let saved = m.subscriptions[0].nodes.remove(0);
        reconcile_hidden_listeners(&mut m);
        assert_eq!(hidden_count(&m), 0);
        assert!(m.exits.is_empty());
        assert_eq!(m.process_routing.rules[0].node, Some(n1()));
        m.subscriptions[0].nodes.insert(0, saved);
        reconcile_hidden_listeners(&mut m);
        assert_eq!(hidden_count(&m), 1);
    }

    #[test]
    fn reconcile_keys_by_node_and_interface() {
        let mut m = model();
        m.process_routing.rules.push(rule(Some(n1()), None, true));
        m.process_routing.rules.push(rule(Some(n1()), Some(g("B")), true));
        reconcile_hidden_listeners(&mut m);
        assert_eq!(hidden_count(&m), 2);
    }

    #[test]
    fn reconcile_tailscale_ignores_rule_interface() {
        let mut m = model();
        m.process_routing.rules.push(rule(Some(ts_ref()), None, true));
        m.process_routing.rules.push(rule(Some(ts_ref()), Some(g("B")), true));
        reconcile_hidden_listeners(&mut m);
        assert_eq!(hidden_count(&m), 1);
        let l = m.listeners.iter().find(|l| l.hidden).unwrap();
        let e = m.exits.iter().find(|e| Some(&e.id) == l.exit.as_ref()).unwrap();
        assert_eq!(e.interface, Some(g("A")));
    }

    #[test]
    fn reconcile_moves_hidden_port_off_user_port() {
        let mut m = model();
        m.process_routing.rules.push(rule(Some(n1()), None, true));
        reconcile_hidden_listeners(&mut m);
        m.listeners.push(Listener::new_default("u".into(), 37800));
        reconcile_hidden_listeners(&mut m);
        let r = m.process_routing.rules[0].clone();
        assert_eq!(rule_listener(&m, &r).unwrap().port, 37801);
    }

    #[test]
    fn reconcile_skips_controller_port_and_user_ports_on_create() {
        let mut m = model();
        m.settings.controller_port = 37800;
        m.listeners.push(Listener::new_default("u".into(), 37801));
        m.process_routing.rules.push(rule(Some(n1()), None, true));
        reconcile_hidden_listeners(&mut m);
        let r = m.process_routing.rules[0].clone();
        assert_eq!(rule_listener(&m, &r).unwrap().port, 37802);
    }

    #[test]
    fn reconcile_ignores_rules_without_node() {
        let mut m = model();
        m.process_routing.rules.push(rule(None, None, true));
        reconcile_hidden_listeners(&mut m);
        assert_eq!(hidden_count(&m), 0);
        assert!(rule_listener(&m, &m.process_routing.rules[0]).is_none());
    }
}

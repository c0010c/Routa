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

/// 修剪规则已知的程序位置：规则停用或 `on_fault≠Block` 时清空；否则只保留文件名对得上
/// 某个 `Name` 条目、且不是本规则 `Path` 条目的路径，按不区分大小写去重。
pub fn prune_learned(r: &mut ProcessRule) {
    if !r.user_enabled || r.on_fault != FaultPolicy::Block {
        r.learned.clear();
        return;
    }
    let key = |p: &str| normalize_exe_path(p).to_lowercase();
    let names: Vec<&str> = r.processes.iter().filter(|p| p.kind == ProcessKind::Name).map(|p| p.value.as_str()).collect();
    let paths: Vec<String> = r.processes.iter().filter(|p| p.kind == ProcessKind::Path).map(|p| key(&p.value)).collect();
    let mut seen = std::collections::HashSet::new();
    r.learned = std::mem::take(&mut r.learned)
        .into_iter()
        .map(|p| normalize_exe_path(&p))
        .filter(|p| {
            let k = key(p);
            is_guard_path_for(&names, p) && !paths.contains(&k) && seen.insert(k)
        })
        .collect();
}

/// `path` 是 `names` 中某个进程名的合法守护路径（与服务端 Apply 校验同一判断，见
/// [`routa_procsvc_proto::guard_path_problem`]）：本地完整路径、不含 `,` `;`、文件名相符。
pub fn is_guard_path_for<S: AsRef<str>>(names: &[S], path: &str) -> bool {
    names.iter().any(|n| routa_procsvc_proto::guard_path_problem(n.as_ref(), path).is_none())
}

pub fn normalize(model: &mut Model) {
    super::hidden::reconcile_hidden_listeners(model);
    for sub in &mut model.subscriptions {
        if let Some(id) = &sub.fallback_listener {
            sub.fallback_exit = model.listeners.iter().find(|l| &l.id == id).and_then(|l| l.exit.clone());
        }
    }
    for r in &mut model.process_routing.rules {
        for p in &mut r.processes {
            p.value = normalize_exe_path(&p.value);
        }
        r.name = r.name.trim().to_string();
        prune_learned(r);
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
            info: None,
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
        Exit { id: ExitId::new(), name: "e".into(), kind, interface, hidden: false }
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
    fn prune_learned_clears_on_direct_disable_and_removed_name() {
        let base = ProcessRule {
            processes: vec![ProcessEntry::name("a.exe"), ProcessEntry::path(r"C:\P\a.exe")],
            learned: vec![
                r"C:\A\a.exe".into(),
                r"c:/a/A.EXE".into(),
                r"C:\P\a.exe".into(),
                r"C:\B\b.exe".into(),
            ],
            on_fault: FaultPolicy::Block,
            ..ProcessRule::default()
        };
        let mut r = base.clone();
        prune_learned(&mut r);
        assert_eq!(r.learned, vec![r"C:\A\a.exe".to_string()]);

        let mut r = ProcessRule { on_fault: FaultPolicy::Direct, ..base.clone() };
        prune_learned(&mut r);
        assert!(r.learned.is_empty());

        let mut r = ProcessRule { user_enabled: false, ..base.clone() };
        prune_learned(&mut r);
        assert!(r.learned.is_empty());

        // 去掉进程名条目后，它的位置一并清掉。
        let mut r = ProcessRule { processes: vec![ProcessEntry::path(r"C:\P\a.exe")], ..base.clone() };
        prune_learned(&mut r);
        assert!(r.learned.is_empty());

        // normalize 对每条规则都修剪。
        let mut m = Model::default();
        m.process_routing.rules.push(ProcessRule { on_fault: FaultPolicy::Direct, ..base });
        normalize(&mut m);
        assert!(m.process_routing.rules[0].learned.is_empty());
    }

    #[test]
    fn prune_learned_drops_paths_the_service_would_reject() {
        let mut r = ProcessRule {
            processes: vec![ProcessEntry::name("a.exe")],
            learned: vec![
                r"\\srv\share\a.exe".into(),
                r"C:\x,y\a.exe".into(),
                r"C:\x;y\a.exe".into(),
                r"\\?\C:\A\a.exe".into(),
                r"C:\A\a.exe".into(),
            ],
            on_fault: FaultPolicy::Block,
            ..ProcessRule::default()
        };
        prune_learned(&mut r);
        assert_eq!(r.learned, vec![r"C:\A\a.exe".to_string()]);
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

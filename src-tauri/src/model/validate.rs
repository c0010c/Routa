use super::normalize::normalize_exe_path;
use super::types::*;

#[derive(serde::Serialize, Clone, Debug, PartialEq, specta::Type)]
pub enum ValidationError {
    DuplicateName { kind: &'static str, name: String },
    PortConflict { port: u16, with: String },
    InvalidName { name: String, reason: String },
    IntervalOutOfRange { field: String, value: u32 },
    DanglingRef { what: String },
    /// A Tailscale instance used by the exit is bound to a different interface than the exit.
    InterfaceMismatch { exit: String, node: String },
    InvalidProxy { name: String, reason: String },
    /// The generated config was rejected by `mihomo -t`.
    CoreRejected { message: String },
    DuplicateProcessRule { exe_path: String },
    InvalidExePath { exe_path: String, reason: String },
    ListenerTypeNotAllowed { listener: String },
    TooManyProcessRules { max: u32 },
}

/// 已启用的进程分流规则数上限（与服务端一致）。
pub const MAX_PROCESS_RULES: u32 = 32;

fn validate_process_routing(model: &Model, errors: &mut Vec<ValidationError>) {
    let pr = &model.process_routing;
    let mut seen = std::collections::HashSet::new();
    let mut reported = std::collections::HashSet::new();
    for r in &pr.rules {
        let path = normalize_exe_path(&r.exe_path);
        if let Some(reason) = routa_procsvc_proto::exe_path_problem(&path) {
            errors.push(ValidationError::InvalidExePath { exe_path: r.exe_path.clone(), reason });
        } else {
            let key = path.to_lowercase();
            if !seen.insert(key.clone()) && reported.insert(key) {
                errors.push(ValidationError::DuplicateProcessRule { exe_path: path });
            }
        }
        match model.listeners.iter().find(|l| l.id == r.listener) {
            None => errors.push(ValidationError::DanglingRef {
                what: format!("process rule[{}].listener -> {}", r.exe_path, r.listener.0),
            }),
            Some(l) if l.listener_type == ListenerType::Http => {
                errors.push(ValidationError::ListenerTypeNotAllowed { listener: l.name.clone() })
            }
            Some(_) => {}
        }
    }
    if pr.rules.iter().filter(|r| r.user_enabled).count() > MAX_PROCESS_RULES as usize {
        errors.push(ValidationError::TooManyProcessRules { max: MAX_PROCESS_RULES });
    }
}

/// 规则的 exe 不得是 Routa 自身、内置 mihomo 或自定义内核（大小写不敏感）。
pub fn validate_forbidden_exes(pr: &ProcessRouting, forbidden: &[std::path::PathBuf]) -> Result<(), Vec<ValidationError>> {
    let keys: Vec<String> = forbidden
        .iter()
        .map(|p| normalize_exe_path(&p.to_string_lossy()).to_lowercase())
        .collect();
    let errors: Vec<ValidationError> = pr
        .rules
        .iter()
        .filter(|r| keys.contains(&normalize_exe_path(&r.exe_path).to_lowercase()))
        .map(|r| ValidationError::InvalidExePath {
            exe_path: r.exe_path.clone(),
            reason: "不能对 Routa 自身或 mihomo 内核做进程分流".to_string(),
        })
        .collect();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// An id safe to use as a single path component (e.g. `tailscale/<id>`): non-empty, `[A-Za-z0-9-]` only.
pub fn is_plain_id(id: &str) -> bool {
    !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

const MAX_INTERVAL_MINS: u32 = 525_600;

fn check_interval(field: String, v: Option<u32>, errors: &mut Vec<ValidationError>) {
    if let Some(value) = v {
        if !(1..=MAX_INTERVAL_MINS).contains(&value) {
            errors.push(ValidationError::IntervalOutOfRange { field, value });
        }
    }
}

fn check_name(name: &str, forbid_slash: bool, errors: &mut Vec<ValidationError>) {
    let mut invalid = |reason: &str| {
        errors.push(ValidationError::InvalidName {
            name: name.to_string(),
            reason: reason.to_string(),
        })
    };
    if name.trim().is_empty() {
        invalid("名称不能为空");
    }
    if name.starts_with("__") {
        invalid("名称不能以 \"__\" 开头（保留给内部使用）");
    }
    if forbid_slash && name.contains('/') {
        invalid("名称不能包含 \"/\"");
    }
}

fn check_unique<'a>(
    kind: &'static str,
    names: impl Iterator<Item = &'a str>,
    errors: &mut Vec<ValidationError>,
) {
    let mut seen = std::collections::HashSet::new();
    let mut reported = std::collections::HashSet::new();
    for n in names {
        if !seen.insert(n) && reported.insert(n) {
            errors.push(ValidationError::DuplicateName {
                kind,
                name: n.to_string(),
            });
        }
    }
}

fn validate_manual(model: &Model, errors: &mut Vec<ValidationError>) {
    check_unique("manual node", model.manual_nodes.iter().map(|n| n.name.as_str()), errors);
    check_unique("node group", model.node_groups.iter().map(|g| g.name.as_str()), errors);

    // Exit and subscription names share the proxy-name namespace with manual nodes ("手动/...").
    let reserved = model.subscriptions.iter().map(|s| &s.name).chain(model.exits.iter().map(|e| &e.name));
    for name in reserved {
        if name == super::resolve::MANUAL_PREFIX {
            errors.push(ValidationError::InvalidName {
                name: name.clone(),
                reason: "「手动」保留给手动节点".to_string(),
            });
        }
    }
    let mut seen_group_ids = std::collections::HashSet::new();
    for g in &model.node_groups {
        if g.name.trim().is_empty() {
            errors.push(ValidationError::InvalidName {
                name: g.name.clone(),
                reason: "名称不能为空".to_string(),
            });
        }
        let reason = if !is_plain_id(&g.id.0) {
            Some("分组 id 无效")
        } else if !seen_group_ids.insert(&g.id.0) {
            Some("分组 id 重复")
        } else {
            None
        };
        if let Some(reason) = reason {
            errors.push(ValidationError::InvalidName { name: g.name.clone(), reason: reason.to_string() });
        }
    }

    let group_ids: std::collections::HashSet<&NodeGroupId> = model.node_groups.iter().map(|g| &g.id).collect();
    let manual_ids: std::collections::HashSet<&ManualNodeId> = model.manual_nodes.iter().map(|n| &n.id).collect();

    let mut seen_node_ids = std::collections::HashSet::new();
    for n in &model.manual_nodes {
        check_name(&n.name, false, errors);
        let mut bad: Vec<&str> = Vec::new();
        // The id names the Tailscale state dir (`tailscale/<id>`).
        if !is_plain_id(&n.id.0) {
            bad.push("节点 id 无效");
        } else if !seen_node_ids.insert(&n.id.0) {
            bad.push("节点 id 重复");
        }
        match &n.kind {
            ManualNodeKind::Proxy { raw } => match raw.as_object() {
                None => bad.push("代理配置必须是对象"),
                Some(o) => {
                    let non_empty = |k: &str| o.get(k).and_then(|v| v.as_str()).is_some_and(|v| !v.trim().is_empty());
                    if !non_empty("type") {
                        bad.push("缺少 type");
                    } else if o["type"].as_str().is_some_and(|t| t.trim().eq_ignore_ascii_case("tailscale")) {
                        bad.push("Tailscale 节点请在 Tailscale 页创建");
                    }
                    if !non_empty("name") {
                        bad.push("缺少 name");
                    }
                }
            },
            ManualNodeKind::Tailscale(o) => {
                if o.hostname.trim().is_empty() {
                    bad.push("hostname 不能为空");
                }
            }
        }
        for reason in bad {
            errors.push(ValidationError::InvalidProxy { name: n.name.clone(), reason: reason.to_string() });
        }
        for g in &n.groups {
            if !group_ids.contains(g) {
                errors.push(ValidationError::DanglingRef {
                    what: format!("manual node[{}].groups -> {}", n.name, g.0),
                });
            }
        }
    }

    for e in &model.exits {
        let mut dangling: Vec<String> = Vec::new();
        let mut check_node = |r: &NodeRef, field: &str| {
            if let NodeRef::Manual { manual_id } = r {
                if !manual_ids.contains(manual_id) {
                    dangling.push(format!("exit[{}].{field} -> {}", e.name, manual_id.0));
                }
            }
        };
        match &e.kind {
            ExitKind::Node { node } => check_node(node, "node"),
            ExitKind::Group { members, selected, .. } => {
                for m in members {
                    match m {
                        ExitMember::Node(r) => check_node(r, "members"),
                        ExitMember::Group { group: GroupRef::Custom(gid) } => {
                            if !group_ids.contains(gid) {
                                errors.push(ValidationError::DanglingRef {
                                    what: format!("exit[{}].members -> {}", e.name, gid.0),
                                });
                            }
                        }
                        ExitMember::Group { group: GroupRef::Subscription(_) } => {}
                    }
                }
                if let Some(r) = selected {
                    check_node(r, "selected");
                }
            }
            ExitKind::Direct => {}
        }
        errors.extend(dangling.into_iter().map(|what| ValidationError::DanglingRef { what }));
        for t in super::resolve::exit_tailscales(model, e) {
            if let ManualNodeKind::Tailscale(o) = &t.kind {
                if o.interface != e.interface {
                    errors.push(ValidationError::InterfaceMismatch { exit: e.name.clone(), node: t.name.clone() });
                }
            }
        }
    }
}

pub fn validate(model: &Model) -> Result<(), Vec<ValidationError>> {
    let mut errors = Vec::new();

    check_unique("subscription", model.subscriptions.iter().map(|s| s.name.as_str()), &mut errors);
    check_unique("exit", model.exits.iter().map(|e| e.name.as_str()), &mut errors);
    // 出口名与订阅名共用一个命名空间：节点名可含 "/"，
    // "{订阅}/{节点}" 可能等于 "{出口}/{订阅}/{节点}"。
    check_unique(
        "出口/订阅",
        model
            .exits
            .iter()
            .map(|e| e.name.as_str())
            .chain(model.subscriptions.iter().map(|s| s.name.as_str())),
        &mut errors,
    );
    check_unique("listener", model.listeners.iter().map(|l| l.name.as_str()), &mut errors);

    for s in &model.subscriptions {
        check_name(&s.name, true, &mut errors);
    }
    for e in &model.exits {
        check_name(&e.name, true, &mut errors);
        const BUILTIN: [&str; 6] = ["DIRECT", "REJECT", "REJECT-DROP", "PASS", "COMPATIBLE", "GLOBAL"];
        if BUILTIN.iter().any(|b| b.eq_ignore_ascii_case(&e.name)) {
            errors.push(ValidationError::InvalidName {
                name: e.name.clone(),
                reason: "与 mihomo 内置名称冲突".to_string(),
            });
        }
    }
    for l in &model.listeners {
        check_name(&l.name, false, &mut errors);
    }

    // 端口冲突（含用户停用的 listener：宁可拒绝也不留隐患）
    let controller = model.settings.controller_port;
    let mut port_owner: std::collections::HashMap<u16, &str> = std::collections::HashMap::new();
    for l in &model.listeners {
        if l.port == controller {
            errors.push(ValidationError::PortConflict {
                port: l.port,
                with: "控制端口".to_string(),
            });
        }
        match port_owner.get(&l.port) {
            Some(other) => errors.push(ValidationError::PortConflict {
                port: l.port,
                with: other.to_string(),
            }),
            None => {
                port_owner.insert(l.port, l.name.as_str());
            }
        }
    }

    check_interval(
        "delay_test_interval_mins".to_string(),
        model.settings.delay_test_interval_mins,
        &mut errors,
    );
    for s in &model.subscriptions {
        check_interval(
            format!("subscriptions[{}].update_interval_mins", s.name),
            s.update_interval_mins,
            &mut errors,
        );
    }

    // 引用的出口必须存在（节点缺失不是校验错误，而是健康状态）
    let exit_ids: std::collections::HashSet<&ExitId> = model.exits.iter().map(|e| &e.id).collect();
    for l in &model.listeners {
        if let Some(id) = &l.exit {
            if !exit_ids.contains(id) {
                errors.push(ValidationError::DanglingRef {
                    what: format!("listener[{}].exit -> {}", l.name, id.0),
                });
            }
        }
    }
    for s in &model.subscriptions {
        if let Some(id) = &s.fallback_exit {
            if !exit_ids.contains(id) {
                errors.push(ValidationError::DanglingRef {
                    what: format!("subscriptions[{}].fallback_exit -> {}", s.name, id.0),
                });
            }
        }
    }

    validate_manual(model, &mut errors);
    validate_process_routing(model, &mut errors);

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exit(name: &str) -> Exit {
        Exit {
            id: ExitId::new(),
            name: name.into(),
            kind: ExitKind::Direct,
            interface: None,
        }
    }
    fn sub(name: &str) -> Subscription {
        Subscription {
            id: SubscriptionId::new(),
            name: name.into(),
            url: "https://x".into(),
            update_interval_mins: Some(1440),
            fallback_exit: None,
            last_updated: None,
            last_result: None,
            nodes: vec![],
        }
    }
    fn errs(m: &Model) -> Vec<ValidationError> {
        validate(m).err().unwrap_or_default()
    }
    fn count_invalid(m: &Model, name: &str) -> usize {
        errs(m)
            .iter()
            .filter(|x| matches!(x, ValidationError::InvalidName { name: n, .. } if n == name))
            .count()
    }

    fn prule(path: &str, l: &Listener) -> ProcessRule {
        ProcessRule { exe_path: path.into(), listener: l.id.clone(), ..ProcessRule::default() }
    }
    fn pmodel(rules: impl FnOnce(&Listener) -> Vec<ProcessRule>) -> Model {
        let mut m = Model::default();
        let l = Listener::new_default("l1".into(), 7891);
        m.process_routing.rules = rules(&l);
        m.listeners.push(l);
        m
    }

    #[test]
    fn process_rule_duplicate_path_case_insensitive() {
        let m = pmodel(|l| vec![prule("C:\\测试\\Ä.exe", l), prule("c:/测试/ä.EXE", l)]);
        let e = errs(&m);
        assert_eq!(e.len(), 1, "{e:?}");
        assert!(matches!(&e[0], ValidationError::DuplicateProcessRule { .. }));
        assert_eq!(validate(&pmodel(|l| vec![prule(" C:/a/b.exe ", l)])), Ok(()));
    }

    #[test]
    fn process_rule_rejects_http_listener() {
        let mut m = pmodel(|l| vec![prule("C:\\a.exe", l)]);
        m.listeners[0].listener_type = ListenerType::Http;
        assert!(errs(&m).contains(&ValidationError::ListenerTypeNotAllowed { listener: "l1".into() }));
        m.listeners[0].listener_type = ListenerType::Socks;
        assert_eq!(validate(&m), Ok(()));
    }

    #[test]
    fn process_rule_rejects_relative_or_network_path() {
        for p in [
            "a.exe",
            "sub\\a.exe",
            "C:a.exe",
            "C:\\a.txt",
            "",
            "C:\\a,b.exe",
            "C:\\a;b.exe",
            r"\\srv\share\a.exe",
            "//srv/share/a.exe",
            r"\\?\C:\a.exe",
        ] {
            let e = errs(&pmodel(|l| vec![prule(p, l)]));
            assert!(
                e.iter().any(|x| matches!(x, ValidationError::InvalidExePath { .. })),
                "{p:?} should be rejected: {e:?}"
            );
        }
    }

    #[test]
    fn process_rule_rejects_core_exe() {
        let m = pmodel(|l| vec![prule("C:/Core/Mihomo.EXE", l)]);
        let forbidden = vec![std::path::PathBuf::from("C:\\Other\\x.exe"), std::path::PathBuf::from("c:\\core\\mihomo.exe")];
        let e = validate_forbidden_exes(&m.process_routing, &forbidden).unwrap_err();
        assert!(matches!(&e[0], ValidationError::InvalidExePath { .. }));
        assert_eq!(validate_forbidden_exes(&m.process_routing, &forbidden[..1]), Ok(()));
    }

    #[test]
    fn process_rule_dangling_listener() {
        let mut m = pmodel(|l| vec![prule("C:\\a.exe", l)]);
        m.listeners.clear();
        assert!(errs(&m).iter().any(|x| matches!(x, ValidationError::DanglingRef { .. })));
    }

    #[test]
    fn process_rule_too_many_enabled() {
        let m = pmodel(|l| (0..33).map(|i| prule(&format!("C:\\a{i}.exe"), l)).collect());
        assert!(errs(&m).contains(&ValidationError::TooManyProcessRules { max: 32 }));
        let m = pmodel(|l| (0..32).map(|i| prule(&format!("C:\\a{i}.exe"), l)).collect());
        assert_eq!(validate(&m), Ok(()));
    }

    #[test]
    fn valid_model_passes() {
        let mut m = Model::default();
        m.exits.push(exit("e1"));
        let mut l = Listener::new_default("l1".into(), 7891);
        l.exit = Some(m.exits[0].id.clone());
        m.listeners.push(l);
        m.subscriptions.push(sub("s1"));
        assert_eq!(validate(&m), Ok(()));
    }

    #[test]
    fn rejects_duplicate_listener_port() {
        let mut m = Model::default();
        m.listeners.push(Listener::new_default("a".into(), 7891));
        m.listeners.push(Listener::new_default("b".into(), 7891));
        let e = errs(&m);
        assert!(
            e.iter().any(|x| matches!(x, ValidationError::PortConflict { port: 7891, with } if with == "a")),
            "{e:?}"
        );
    }

    #[test]
    fn rejects_port_equal_to_controller() {
        let mut m = Model::default();
        m.listeners.push(Listener::new_default("a".into(), 19097));
        let e = errs(&m);
        assert!(
            e.contains(&ValidationError::PortConflict { port: 19097, with: "控制端口".into() }),
            "{e:?}"
        );
    }

    #[test]
    fn rejects_duplicate_exit_name() {
        let mut m = Model::default();
        m.exits.push(exit("x"));
        m.exits.push(exit("x"));
        assert!(errs(&m).contains(&ValidationError::DuplicateName { kind: "exit", name: "x".into() }));
    }

    #[test]
    fn rejects_exit_name_equal_to_subscription_name() {
        let mut m = Model::default();
        m.exits.push(exit("x"));
        m.subscriptions.push(sub("x"));
        assert!(errs(&m).contains(&ValidationError::DuplicateName { kind: "出口/订阅", name: "x".into() }));
    }

    #[test]
    fn rejects_builtin_proxy_names_for_exits() {
        for n in ["DIRECT", "reject", "Reject-Drop", "pass", "Compatible", "global"] {
            let mut m = Model::default();
            m.exits.push(exit(n));
            assert!(
                errs(&m).contains(&ValidationError::InvalidName { name: n.into(), reason: "与 mihomo 内置名称冲突".into() }),
                "{n}"
            );
        }
        let mut m = Model::default();
        m.exits.push(exit("direct-2"));
        assert_eq!(validate(&m), Ok(()));
    }

    #[test]
    fn rejects_duplicate_listener_name() {
        let mut m = Model::default();
        m.listeners.push(Listener::new_default("x".into(), 7891));
        m.listeners.push(Listener::new_default("x".into(), 7892));
        assert!(errs(&m).contains(&ValidationError::DuplicateName { kind: "listener", name: "x".into() }));
    }

    #[test]
    fn rejects_duplicate_subscription_name() {
        let mut m = Model::default();
        m.subscriptions.push(sub("x"));
        m.subscriptions.push(sub("x"));
        assert!(errs(&m).contains(&ValidationError::DuplicateName { kind: "subscription", name: "x".into() }));
    }

    #[test]
    fn rejects_slash_in_exit_and_subscription_name() {
        let mut m = Model::default();
        m.exits.push(exit("a/b"));
        m.subscriptions.push(sub("a/b"));
        assert_eq!(count_invalid(&m, "a/b"), 2);
    }

    #[test]
    fn rejects_names_starting_with_double_underscore() {
        let mut m = Model::default();
        m.exits.push(exit("__fetch"));
        m.subscriptions.push(sub("__fetch"));
        m.listeners.push(Listener::new_default("__fetch".into(), 7891));
        assert_eq!(count_invalid(&m, "__fetch"), 3);
    }

    #[test]
    fn rejects_empty_and_whitespace_names() {
        for n in ["", "  ", "	"] {
            let mut m = Model::default();
            m.exits.push(exit(n));
            m.subscriptions.push(sub(&format!("{n}{n}")));
            m.listeners.push(Listener::new_default(n.into(), 7891));
            let e = errs(&m);
            let empty = e
                .iter()
                .filter(|x| matches!(x, ValidationError::InvalidName { reason, .. } if reason == "名称不能为空"))
                .count();
            assert_eq!(empty, 3, "{n:?} {e:?}");
        }
    }

    #[test]
    fn interval_bounds() {
        for (v, ok) in [
            (Some(0), false),
            (Some(525601), false),
            (None, true),
            (Some(1), true),
            (Some(525600), true),
        ] {
            let mut m = Model::default();
            m.settings.delay_test_interval_mins = v;
            let mut s = sub("s");
            s.update_interval_mins = v;
            m.subscriptions.push(s);
            let e = errs(&m);
            assert_eq!(e.is_empty(), ok, "{v:?} {e:?}");
            if !ok {
                let value = v.unwrap();
                assert!(e.contains(&ValidationError::IntervalOutOfRange {
                    field: "delay_test_interval_mins".into(),
                    value
                }));
                assert!(e.contains(&ValidationError::IntervalOutOfRange {
                    field: "subscriptions[s].update_interval_mins".into(),
                    value
                }));
            }
        }
    }

    #[test]
    fn rejects_listener_referencing_unknown_exit() {
        let mut m = Model::default();
        let mut l = Listener::new_default("a".into(), 7891);
        l.exit = Some(ExitId::new());
        m.listeners.push(l);
        assert!(errs(&m).iter().any(|x| matches!(x, ValidationError::DanglingRef { .. })));
    }

    #[test]
    fn rejects_subscription_fallback_unknown_exit() {
        let mut m = Model::default();
        let mut s = sub("s");
        s.fallback_exit = Some(ExitId::new());
        m.subscriptions.push(s);
        assert!(errs(&m).iter().any(|x| matches!(x, ValidationError::DanglingRef { .. })));
    }

    fn mnode(id: &str, name: &str, kind: ManualNodeKind) -> ManualNode {
        ManualNode { id: ManualNodeId(id.into()), name: name.into(), groups: vec![], kind }
    }
    fn proxy(raw: serde_json::Value) -> ManualNodeKind {
        ManualNodeKind::Proxy { raw }
    }
    fn ss() -> ManualNodeKind {
        proxy(serde_json::json!({"type": "ss", "name": "x"}))
    }
    fn mref(id: &str) -> NodeRef {
        NodeRef::Manual { manual_id: ManualNodeId(id.into()) }
    }
    fn gexit(name: &str, members: Vec<ExitMember>, selected: Option<NodeRef>) -> Exit {
        Exit {
            id: ExitId::new(),
            name: name.into(),
            kind: ExitKind::Group { strategy: GroupStrategy::Select, members, selected },
            interface: None,
        }
    }

    #[test]
    fn manual_node_names_unique() {
        let mut m = Model::default();
        m.manual_nodes.push(mnode("a", "x", ss()));
        m.manual_nodes.push(mnode("b", "x", ss()));
        m.node_groups.push(NodeGroup { id: NodeGroupId("g1".into()), name: "g".into() });
        m.node_groups.push(NodeGroup { id: NodeGroupId("g2".into()), name: "g".into() });
        let e = errs(&m);
        assert!(e.contains(&ValidationError::DuplicateName { kind: "manual node", name: "x".into() }), "{e:?}");
        assert!(e.contains(&ValidationError::DuplicateName { kind: "node group", name: "g".into() }), "{e:?}");
        let mut m = Model::default();
        m.manual_nodes.push(mnode("a", "", ss()));
        m.manual_nodes.push(mnode("b", "__x", ss()));
        assert_eq!(errs(&m).iter().filter(|x| matches!(x, ValidationError::InvalidName { .. })).count(), 2);
    }

    #[test]
    fn subscription_named_manual_rejected() {
        let mut m = Model::default();
        m.subscriptions.push(sub("手动"));
        assert!(errs(&m).contains(&ValidationError::InvalidName {
            name: "手动".into(),
            reason: "「手动」保留给手动节点".into()
        }));
    }

    #[test]
    fn exit_named_manual_rejected() {
        let mut m = Model::default();
        m.exits.push(exit("手动"));
        assert!(errs(&m).contains(&ValidationError::InvalidName {
            name: "手动".into(),
            reason: "「手动」保留给手动节点".into()
        }));
    }

    #[test]
    fn manual_node_ids_validated() {
        for bad in ["", "../x", "a/b", "a b", "a_b", ".", "a:b", "节点"] {
            let mut m = Model::default();
            m.manual_nodes.push(mnode(bad, "n", ss()));
            assert!(
                errs(&m).contains(&ValidationError::InvalidProxy { name: "n".into(), reason: "节点 id 无效".into() }),
                "{bad:?}: {:?}",
                errs(&m)
            );
        }
        let mut m = Model::default();
        m.manual_nodes.push(mnode("a", "n1", ss()));
        m.manual_nodes.push(mnode("a", "n2", ss()));
        assert!(errs(&m).contains(&ValidationError::InvalidProxy { name: "n2".into(), reason: "节点 id 重复".into() }));
        let mut m = Model::default();
        m.manual_nodes.push(ManualNode { id: ManualNodeId::new(), name: "n".into(), groups: vec![], kind: ss() });
        m.manual_nodes.push(mnode("0f8fad5b-d9cb-469f-a165-70867728950e", "n2", ss()));
        assert!(validate(&m).is_ok(), "{:?}", errs(&m));
    }

    #[test]
    fn node_group_ids_validated() {
        let mut m = Model::default();
        m.node_groups.push(NodeGroup { id: NodeGroupId("".into()), name: "g0".into() });
        m.node_groups.push(NodeGroup { id: NodeGroupId("g1".into()), name: "g1".into() });
        m.node_groups.push(NodeGroup { id: NodeGroupId("g1".into()), name: "g2".into() });
        let e = errs(&m);
        assert!(e.contains(&ValidationError::InvalidName { name: "g0".into(), reason: "分组 id 无效".into() }), "{e:?}");
        assert!(e.contains(&ValidationError::InvalidName { name: "g2".into(), reason: "分组 id 重复".into() }), "{e:?}");
        let mut m = Model::default();
        m.node_groups.push(NodeGroup { id: NodeGroupId::new(), name: "g".into() });
        assert!(validate(&m).is_ok(), "{:?}", errs(&m));
    }

    #[test]
    fn plain_id_check() {
        assert!(is_plain_id("abc-DEF-123"));
        assert!(is_plain_id(&ManualNodeId::new().0));
        for bad in ["", ".", "..", "a/b", r"a\b", "a:b", "a.b", "a_b"] {
            assert!(!is_plain_id(bad), "{bad:?}");
        }
    }

    #[test]
    fn pasted_proxy_needs_type_and_name() {
        let bad = [
            serde_json::json!({"name": "x"}),
            serde_json::json!({"type": "tailscale", "name": "x"}),
            serde_json::json!({"type": "ss"}),
            serde_json::json!({"type": " ", "name": "x"}),
            serde_json::Value::Null,
            serde_json::json!("str"),
        ];
        for raw in bad {
            let mut m = Model::default();
            m.manual_nodes.push(mnode("a", "n", proxy(raw.clone())));
            assert!(
                errs(&m).iter().any(|x| matches!(x, ValidationError::InvalidProxy { name, .. } if name == "n")),
                "{raw}"
            );
        }
        let mut m = Model::default();
        m.manual_nodes.push(mnode("a", "n", ss()));
        assert_eq!(validate(&m), Ok(()));
        // persisted default kind (Proxy with Null raw) is rejected
        let mut m = Model::default();
        m.manual_nodes.push(ManualNode { name: "d".into(), ..ManualNode::default() });
        assert!(errs(&m).iter().any(|x| matches!(x, ValidationError::InvalidProxy { .. })));
    }

    #[test]
    fn tailscale_hostname_required() {
        let mut m = Model::default();
        let mut o = TailscaleOpts::defaults_for("t");
        o.hostname = "  ".into();
        m.manual_nodes.push(mnode("a", "t", ManualNodeKind::Tailscale(o)));
        assert!(errs(&m).iter().any(|x| matches!(x, ValidationError::InvalidProxy { .. })));
        m.manual_nodes[0].kind = ManualNodeKind::Tailscale(TailscaleOpts::defaults_for("t"));
        assert_eq!(validate(&m), Ok(()));
    }

    #[test]
    fn dangling_manual_and_custom_group_refs() {
        let dangling = |m: &Model| errs(m).iter().filter(|x| matches!(x, ValidationError::DanglingRef { .. })).count();
        let mut m = Model::default();
        m.exits.push(gexit("e", vec![ExitMember::Node(mref("zz"))], None));
        assert_eq!(dangling(&m), 1);
        let mut m = Model::default();
        m.exits.push(gexit(
            "e",
            vec![ExitMember::Group { group: GroupRef::Custom(NodeGroupId("gz".into())) }],
            Some(mref("zz")),
        ));
        assert_eq!(dangling(&m), 2);
        let mut m = Model::default();
        m.exits.push(Exit { id: ExitId::new(), name: "n".into(), kind: ExitKind::Node { node: mref("zz") }, interface: None });
        assert_eq!(dangling(&m), 1);
        let mut m = Model::default();
        let mut n = mnode("a", "n", ss());
        n.groups.push(NodeGroupId("gz".into()));
        m.manual_nodes.push(n);
        assert_eq!(dangling(&m), 1);
        // subscription-side gaps stay health-state only
        let mut m = Model::default();
        m.exits.push(gexit(
            "e",
            vec![
                ExitMember::Group { group: GroupRef::Subscription(SubscriptionId("sz".into())) },
                ExitMember::Node(NodeRef::Subscription { subscription_id: SubscriptionId("sz".into()), name: "n".into() }),
            ],
            None,
        ));
        assert_eq!(validate(&m), Ok(()));
    }

    #[test]
    fn tailscale_interface_must_match_exit() {
        let a = Some(InterfaceGuid("{A}".into()));
        let mut o = TailscaleOpts::defaults_for("t");
        o.interface = a.clone();
        let mut m = Model::default();
        m.manual_nodes.push(mnode("t", "ts", ManualNodeKind::Tailscale(o)));
        m.exits.push(gexit(
            "e",
            vec![
                ExitMember::Node(NodeRef::Subscription { subscription_id: SubscriptionId("s".into()), name: "n".into() }),
                ExitMember::Node(mref("t")),
            ],
            None,
        ));
        assert!(errs(&m).contains(&ValidationError::InterfaceMismatch { exit: "e".into(), node: "ts".into() }));
        m.exits[0].interface = a;
        assert_eq!(validate(&m), Ok(()));
    }
}

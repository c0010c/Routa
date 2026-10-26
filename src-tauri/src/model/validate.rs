use super::types::*;

#[derive(serde::Serialize, Clone, Debug, PartialEq, specta::Type)]
pub enum ValidationError {
    DuplicateName { kind: &'static str, name: String },
    PortConflict { port: u16, with: String },
    InvalidName { name: String, reason: String },
    IntervalOutOfRange { field: String, value: u32 },
    DanglingRef { what: String },
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

pub fn validate(model: &Model) -> Result<(), Vec<ValidationError>> {
    let mut errors = Vec::new();

    check_unique("subscription", model.subscriptions.iter().map(|s| s.name.as_str()), &mut errors);
    check_unique("exit", model.exits.iter().map(|e| e.name.as_str()), &mut errors);
    check_unique("listener", model.listeners.iter().map(|l| l.name.as_str()), &mut errors);

    for s in &model.subscriptions {
        check_name(&s.name, true, &mut errors);
    }
    for e in &model.exits {
        check_name(&e.name, true, &mut errors);
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
        m.listeners.push(Listener::new_default("a".into(), 9097));
        let e = errs(&m);
        assert!(
            e.contains(&ValidationError::PortConflict { port: 9097, with: "控制端口".into() }),
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
}

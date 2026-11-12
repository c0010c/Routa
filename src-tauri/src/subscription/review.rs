use crate::core::apply::ApplyOutcome;
use crate::model::*;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, specta::Type)]
pub struct SubscriptionPreview {
    pub subscription: Subscription,
    pub base: String,
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub affected: Vec<String>,
    pub skipped_tailscale: u32,
    pub via_proxy: bool,
}

/// Includes raw node options, not just names: an old preview must not overwrite a newer update.
pub fn fingerprint(sub: Option<&Subscription>) -> String {
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    serde_json::to_string(&sub)
        .unwrap_or_default()
        .hash(&mut hash);
    format!("{:x}", hash.finish())
}

pub fn changes(old: &[Node], new: &[Node]) -> (Vec<String>, Vec<String>) {
    let before: HashSet<_> = old.iter().map(|n| &n.name).collect();
    let after: HashSet<_> = new.iter().map(|n| &n.name).collect();
    (
        new.iter()
            .filter(|n| !before.contains(&n.name))
            .map(|n| n.name.clone())
            .collect(),
        old.iter()
            .filter(|n| !after.contains(&n.name))
            .map(|n| n.name.clone())
            .collect(),
    )
}

fn matches_node(node: &NodeRef, id: &SubscriptionId, removed: Option<&[String]>) -> bool {
    matches!(node, NodeRef::Subscription { subscription_id, name } if subscription_id == id && removed.is_none_or(|names| names.contains(name)))
}
fn matches_member(member: &ExitMember, id: &SubscriptionId, removed: Option<&[String]>) -> bool {
    match member {
        ExitMember::Node(node) => matches_node(node, id, removed),
        ExitMember::Group {
            group: GroupRef::Subscription(sid),
        } => removed.is_none() && sid == id,
        _ => false,
    }
}

pub fn affected(model: &Model, id: &SubscriptionId, removed: Option<&[String]>) -> Vec<String> {
    let mut labels = vec![];
    for exit in &model.exits {
        if exit.hidden {
            continue;
        }
        let used = match &exit.kind {
            ExitKind::Node { node } => matches_node(node, id, removed),
            ExitKind::Group {
                members, selected, ..
            } => {
                members.iter().any(|m| matches_member(m, id, removed))
                    || selected
                        .as_ref()
                        .is_some_and(|n| matches_node(n, id, removed))
            }
            _ => false,
        };
        if used {
            labels.extend(
                model
                    .listeners
                    .iter()
                    .filter(|l| l.exit.as_ref() == Some(&exit.id))
                    .map(|l| l.name.clone()),
            );
        }
    }
    for rule in &model.process_routing.rules {
        if rule
            .node
            .as_ref()
            .is_some_and(|n| matches_node(n, id, removed))
        {
            labels.push(crate::model::validate::process_rule_label(rule));
        }
    }
    for group in &model.node_groups {
        if group.members.iter().any(|m| matches_node(m, id, removed)) {
            labels.push(format!("节点组「{}」", group.name));
        }
    }
    if model
        .settings
        .node_monitor
        .members
        .iter()
        .any(|m| matches_member(m, id, removed))
    {
        labels.push("节点监测".into());
    }
    labels.sort();
    labels.dedup();
    labels
}

pub fn replace_references(
    model: &mut Model,
    id: &SubscriptionId,
    removed: Option<&[String]>,
    replacement: &NodeRef,
) {
    let replace_node = |n: &mut NodeRef| {
        if matches_node(n, id, removed) {
            *n = replacement.clone();
        }
    };
    let replace_member = |m: &mut ExitMember| {
        if matches_member(m, id, removed) {
            *m = ExitMember::Node(replacement.clone());
        }
    };
    for e in &mut model.exits {
        match &mut e.kind {
            ExitKind::Node { node } => replace_node(node),
            ExitKind::Group {
                members, selected, ..
            } => {
                for m in members {
                    replace_member(m);
                }
                if let Some(n) = selected {
                    replace_node(n);
                }
            }
            _ => {}
        }
    }
    for r in &mut model.process_routing.rules {
        if let Some(n) = &mut r.node {
            replace_node(n);
        }
    }
    for g in &mut model.node_groups {
        for m in &mut g.members {
            replace_node(m);
        }
    }
    for m in &mut model.settings.node_monitor.members {
        replace_member(m);
    }
}

pub fn initialize_info(sub: &mut Subscription) {
    if sub.info.is_none() {
        sub.info = Some(SubscriptionInfo {
            last_success: if matches!(sub.last_result, Some(FetchResult::Ok { .. })) {
                sub.last_updated
            } else {
                None
            },
            ..Default::default()
        });
    }
}

pub fn application_failed(apply: &ApplyOutcome) -> bool {
    match apply {
        ApplyOutcome::Reloaded {
            failed_listeners,
            uncut,
            ..
        } => !failed_listeners.is_empty() || *uncut > 0,
        ApplyOutcome::NotRunning { .. } => false,
        _ => true,
    }
}

pub fn record(
    sub: &mut Subscription,
    result: FetchResult,
    added: Vec<String>,
    removed: Vec<String>,
    via_proxy: bool,
    apply: Option<ApplyOutcome>,
    at: i64,
) {
    let info = sub.info.get_or_insert_with(Default::default);
    if matches!(&result, FetchResult::Ok { .. }) && !apply.as_ref().is_some_and(application_failed)
    {
        info.last_success = Some(at);
    }
    info.history.insert(
        0,
        SubscriptionUpdate {
            at,
            result,
            added,
            removed,
            via_proxy,
            apply,
        },
    );
    info.history.truncate(20);
}

#[cfg(test)]
mod tests {
    use super::*;
    fn reference(name: &str) -> NodeRef {
        NodeRef::Subscription {
            subscription_id: SubscriptionId("s".into()),
            name: name.into(),
        }
    }
    #[test]
    fn replacement_updates_disabled_rules_groups_monitor_and_selected_members() {
        let id = SubscriptionId("s".into());
        let old = reference("old");
        let next = reference("next");
        let mut m = Model::default();
        m.exits.push(Exit {
            id: ExitId("e".into()),
            kind: ExitKind::Group {
                strategy: GroupStrategy::Select,
                members: vec![ExitMember::Node(old.clone())],
                selected: Some(old.clone()),
            },
            ..Default::default()
        });
        m.listeners.push(Listener {
            name: "工作代理".into(),
            exit: Some(ExitId("e".into())),
            ..Default::default()
        });
        m.process_routing.rules.push(ProcessRule {
            name: "已暂停应用".into(),
            node: Some(old.clone()),
            user_enabled: false,
            ..Default::default()
        });
        m.node_groups.push(NodeGroup {
            name: "常用".into(),
            members: vec![old.clone()],
            ..Default::default()
        });
        m.settings.node_monitor.members = vec![ExitMember::Node(old)];
        let removed = vec!["old".into()];
        assert_eq!(affected(&m, &id, Some(&removed)).len(), 4);
        replace_references(&mut m, &id, Some(&removed), &next);
        assert!(affected(&m, &id, Some(&removed)).is_empty());
        assert_eq!(m.process_routing.rules[0].node, Some(next.clone()));
        assert_eq!(m.node_groups[0].members, vec![next]);
    }
    #[test]
    fn bounded_history_keeps_success_time_on_download_or_apply_failure() {
        let mut s = Subscription::default();
        let ok = FetchResult::Ok {
            node_count: 1,
            skipped_tailscale: 0,
        };
        record(&mut s, ok.clone(), vec![], vec![], false, None, 10);
        record(
            &mut s,
            ok,
            vec![],
            vec![],
            false,
            Some(ApplyOutcome::FailedClosed {
                error: "failed".into(),
                uncut: 0,
            }),
            20,
        );
        for at in 30..60 {
            record(
                &mut s,
                FetchResult::Failed {
                    reason: "timeout".into(),
                },
                vec![],
                vec![],
                false,
                None,
                at,
            );
        }
        assert_eq!(s.info.as_ref().unwrap().last_success, Some(10));
        assert_eq!(s.info.as_ref().unwrap().history.len(), 20);
    }
    #[test]
    fn preview_fingerprint_changes_even_when_only_credentials_change() {
        let mut s = Subscription::default();
        let first = fingerprint(Some(&s));
        s.url = "https://example.test/new-token".into();
        assert_ne!(first, fingerprint(Some(&s)));
        assert_ne!(first, fingerprint(None));
    }
}

//! AI 应用规则的纯函数变换：冲突检测、启用、停用、与列表同步。

use super::catalog::{Catalog, CatalogApp};
use crate::model::types::{NodeRef, ProcessEntry, ProcessKind, ProcessRouting, ProcessRule, ProcessRuleId};
use crate::model::validate::{process_file_name, process_rule_label};
use serde::Serialize;

#[derive(Serialize, Clone, Debug, PartialEq, specta::Type)]
pub struct AiAppConflict {
    pub rule_id: ProcessRuleId,
    pub rule_name: String,
    pub process: String,
    pub removes_rule: bool,
}

pub fn ai_rule<'a>(pr: &'a ProcessRouting, app_id: &str) -> Option<&'a ProcessRule> {
    pr.rules.iter().find(|r| r.ai_app.as_deref() == Some(app_id))
}

fn entry_conflicts(e: &ProcessEntry, app: &CatalogApp) -> bool {
    let name = match e.kind {
        ProcessKind::Name => e.value.as_str(),
        ProcessKind::Path => process_file_name(&e.value),
    };
    app.processes.iter().any(|p| p.eq_ignore_ascii_case(name))
}

pub fn conflicts(pr: &ProcessRouting, app: &CatalogApp) -> Vec<AiAppConflict> {
    let mut out = vec![];
    for r in pr.rules.iter().filter(|r| r.ai_app.is_none()) {
        let hits: Vec<&ProcessEntry> = r.processes.iter().filter(|e| entry_conflicts(e, app)).collect();
        let removes_rule = !hits.is_empty() && hits.len() == r.processes.len();
        for e in hits {
            out.push(AiAppConflict {
                rule_id: r.id.clone(),
                rule_name: process_rule_label(r),
                process: e.value.clone(),
                removes_rule,
            });
        }
    }
    out
}

fn app_entries(app: &CatalogApp) -> Vec<ProcessEntry> {
    app.processes.iter().map(|p| ProcessEntry::name(p.clone())).collect()
}

/// 移除冲突条目（规则变空就删除），插入或更新 AI 规则（node、processes、on_fault、name、
/// reset_on_change=true、user_enabled=true），并打开总开关。返回被整条删除的规则 id。
pub fn enable(pr: &mut ProcessRouting, app: &CatalogApp, node: NodeRef) -> Vec<ProcessRuleId> {
    let mut removed = vec![];
    for r in pr.rules.iter_mut().filter(|r| r.ai_app.is_none()) {
        let before = r.processes.len();
        r.processes.retain(|e| !entry_conflicts(e, app));
        if before > 0 && r.processes.is_empty() {
            removed.push(r.id.clone());
        }
    }
    pr.rules.retain(|r| !removed.contains(&r.id));

    let idx = match pr.rules.iter().position(|r| r.ai_app.as_deref() == Some(app.id.as_str())) {
        Some(i) => i,
        None => {
            pr.rules.push(ProcessRule { ai_app: Some(app.id.clone()), ..ProcessRule::default() });
            pr.rules.len() - 1
        }
    };
    let r = &mut pr.rules[idx];
    r.name = app.name.clone();
    r.processes = app_entries(app);
    r.on_fault = app.on_fault.clone();
    r.node = Some(node);
    r.reset_on_change = true;
    r.user_enabled = true;
    pr.enabled = true;
    removed
}

pub fn disable(pr: &mut ProcessRouting, app_id: &str) {
    if let Some(r) = pr.rules.iter_mut().find(|r| r.ai_app.as_deref() == Some(app_id)) {
        r.user_enabled = false;
    }
}

/// 让现有 AI 规则跟上列表：更新 name/processes/on_fault；删除列表里已经没有的应用的规则。返回是否有改动。
pub fn sync_with_catalog(pr: &mut ProcessRouting, catalog: &Catalog) -> bool {
    let before = pr.rules.len();
    pr.rules.retain(|r| match &r.ai_app {
        Some(id) => catalog.apps.iter().any(|a| &a.id == id),
        None => true,
    });
    let mut changed = pr.rules.len() != before;
    for r in pr.rules.iter_mut() {
        let Some(app) = r.ai_app.as_ref().and_then(|id| catalog.apps.iter().find(|a| &a.id == id)) else {
            continue;
        };
        let entries = app_entries(app);
        if r.name != app.name || r.processes != entries || r.on_fault != app.on_fault {
            r.name = app.name.clone();
            r.processes = entries;
            r.on_fault = app.on_fault.clone();
            changed = true;
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::types::ManualNodeId;
    use routa_procsvc_proto::FaultPolicy;

    fn app(id: &str, procs: &[&str]) -> CatalogApp {
        CatalogApp {
            id: id.into(),
            name: id.to_uppercase(),
            icon: String::new(),
            download_url: String::new(),
            test_url: String::new(),
            on_fault: FaultPolicy::Block,
            install: vec![],
            processes: procs.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn rule(id: &str, entries: Vec<ProcessEntry>) -> ProcessRule {
        ProcessRule { id: ProcessRuleId(id.into()), processes: entries, ..ProcessRule::default() }
    }

    fn pr(rules: Vec<ProcessRule>) -> ProcessRouting {
        ProcessRouting { rules, ..ProcessRouting::default() }
    }

    fn node(n: &str) -> NodeRef {
        NodeRef::Manual { manual_id: ManualNodeId(n.into()) }
    }

    #[test]
    fn conflict_matches_name_case_insensitively() {
        let p = pr(vec![rule("r", vec![ProcessEntry::name("chatgpt.EXE")])]);
        let c = conflicts(&p, &app("chatgpt", &["ChatGPT.exe"]));
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].rule_id, ProcessRuleId("r".into()));
        assert_eq!(c[0].process, "chatgpt.EXE");
        assert!(c[0].removes_rule);
    }

    #[test]
    fn conflict_matches_path_file_name() {
        let p = pr(vec![rule("r", vec![ProcessEntry::path("C:\\X\\chatgpt.EXE"), ProcessEntry::name("a.exe")])]);
        let c = conflicts(&p, &app("chatgpt", &["ChatGPT.exe"]));
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].process, "C:\\X\\chatgpt.EXE");
        assert!(!c[0].removes_rule);
    }

    #[test]
    fn conflict_ignores_other_ai_rules() {
        let mut r = rule("r", vec![ProcessEntry::name("ChatGPT.exe")]);
        r.ai_app = Some("other".into());
        assert!(conflicts(&pr(vec![r]), &app("chatgpt", &["ChatGPT.exe"])).is_empty());
    }

    #[test]
    fn enable_removes_only_conflicting_entries() {
        let mut p = pr(vec![
            rule("a", vec![ProcessEntry::name("ChatGPT.exe"), ProcessEntry::name("notepad.exe")]),
            rule("b", vec![ProcessEntry::name("Codex.exe")]),
        ]);
        let removed = enable(&mut p, &app("chatgpt", &["ChatGPT.exe", "Codex.exe"]), node("n"));
        assert_eq!(removed, vec![ProcessRuleId("b".into())]);
        assert_eq!(p.rules.len(), 2);
        assert_eq!(p.rules[0].processes, vec![ProcessEntry::name("notepad.exe")]);
    }

    #[test]
    fn enable_creates_tagged_rule_and_turns_routing_on() {
        let mut p = pr(vec![]);
        let a = app("chatgpt", &["ChatGPT.exe", "Codex.exe"]);
        enable(&mut p, &a, node("n"));
        assert!(p.enabled);
        let r = ai_rule(&p, "chatgpt").unwrap();
        assert_eq!(r.ai_app.as_deref(), Some("chatgpt"));
        assert_eq!(r.name, a.name);
        assert_eq!(r.processes, vec![ProcessEntry::name("ChatGPT.exe"), ProcessEntry::name("Codex.exe")]);
        assert!(r.reset_on_change && r.user_enabled);
        assert_eq!(r.on_fault, FaultPolicy::Block);
        assert_eq!(r.node, Some(node("n")));
    }

    #[test]
    fn enable_again_updates_node_without_duplicating() {
        let mut p = pr(vec![]);
        let a = app("chatgpt", &["ChatGPT.exe"]);
        enable(&mut p, &a, node("n1"));
        let id = p.rules[0].id.clone();
        p.rules[0].learned = vec!["C:\\x\\ChatGPT.exe".into()];
        p.rules[0].user_enabled = false;
        enable(&mut p, &a, node("n2"));
        assert_eq!(p.rules.len(), 1);
        assert_eq!(p.rules[0].id, id);
        assert_eq!(p.rules[0].learned.len(), 1);
        assert_eq!(p.rules[0].node, Some(node("n2")));
        assert!(p.rules[0].user_enabled);
    }

    #[test]
    fn disable_keeps_rule_and_node() {
        let mut p = pr(vec![]);
        enable(&mut p, &app("chatgpt", &["ChatGPT.exe"]), node("n"));
        disable(&mut p, "chatgpt");
        let r = ai_rule(&p, "chatgpt").unwrap();
        assert!(!r.user_enabled);
        assert_eq!(r.node, Some(node("n")));
    }

    fn cat(apps: Vec<CatalogApp>) -> Catalog {
        Catalog { schema_version: 1, apps }
    }

    #[test]
    fn sync_updates_processes_and_on_fault() {
        let mut p = pr(vec![]);
        enable(&mut p, &app("chatgpt", &["ChatGPT.exe"]), node("n"));
        let mut a = app("chatgpt", &["ChatGPT.exe", "New.exe"]);
        a.name = "Renamed".into();
        a.on_fault = FaultPolicy::Direct;
        assert!(sync_with_catalog(&mut p, &cat(vec![a])));
        let r = ai_rule(&p, "chatgpt").unwrap();
        assert_eq!(r.name, "Renamed");
        assert_eq!(r.processes.len(), 2);
        assert_eq!(r.on_fault, FaultPolicy::Direct);
        assert_eq!(r.node, Some(node("n")));
    }

    #[test]
    fn sync_removes_rules_of_apps_gone_from_catalog() {
        let mut p = pr(vec![rule("plain", vec![ProcessEntry::name("a.exe")])]);
        enable(&mut p, &app("gone", &["Gone.exe"]), node("n"));
        assert!(sync_with_catalog(&mut p, &cat(vec![])));
        assert_eq!(p.rules.len(), 1);
        assert!(p.rules[0].ai_app.is_none());
    }

    #[test]
    fn sync_reports_no_change_when_equal() {
        let mut p = pr(vec![]);
        let a = app("chatgpt", &["ChatGPT.exe"]);
        enable(&mut p, &a, node("n"));
        assert!(!sync_with_catalog(&mut p, &cat(vec![a])));
    }
}

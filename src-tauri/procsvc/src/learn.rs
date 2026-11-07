//! 服务端学习位置：按进程名、Routa 不可用时阻断（Name + on_fault=Block）的规则，
//! 从 ProxyBridge 看到的连接里学到该程序的完整路径，交给 WFP 守护。
//!
//! 纯逻辑；取进程路径、检查文件是否存在由调用方注入。

use routa_procsvc_proto::{DesiredState, FaultPolicy, LearnedPath, ProcessKind, RuleSpec};

use crate::paths::{matches_process, same_path};
use crate::reconcile::exe_key;

/// 规则 `r` 可以把完整路径 `path` 学为守护路径：Name + Block、进程名相符，
/// 且 `path` 不是任何按路径的规则的 exe（同一个 exe 的阻断 / 放行只能归一条规则管）。
fn learnable(r: &RuleSpec, rules: &[RuleSpec], path: &str) -> bool {
    r.kind == ProcessKind::Name
        && r.on_fault == FaultPolicy::Block
        && matches_process(ProcessKind::Name, &r.exe_path, path)
        && !rules
            .iter()
            .any(|p| p.kind == ProcessKind::Path && same_path(&p.exe_path, path))
}

fn has_guard(r: &RuleSpec, path: &str) -> bool {
    r.guard_paths.iter().any(|g| same_path(g, path))
}

/// `exe`（进程的完整路径）命中某条 Name + Block 规则、且还没被守护时，返回要学到的位置。
/// `exe` 已在该规则的 `guard_paths` 里、与某条按路径的规则同键，或不是合法的本地完整路径时为 `None`。
pub fn observe(rules: &[RuleSpec], exe: &str) -> Option<LearnedPath> {
    if routa_procsvc_proto::exe_path_problem(exe).is_some() {
        return None;
    }
    let r = rules.iter().find(|r| learnable(r, rules, exe))?;
    (!has_guard(r, exe)).then(|| LearnedPath {
        rule_id: r.id.clone(),
        path: exe.to_string(),
    })
}

/// 实际落地的状态：`learned` 并入对应规则的 `guard_paths`（按 `exe_key` 去重），
/// 并删掉所有 `exists` 为假的守护路径（包括 Routa 下发的）。
pub fn effective(
    desired: &DesiredState,
    learned: &[LearnedPath],
    exists: &dyn Fn(&str) -> bool,
) -> DesiredState {
    let mut out = desired.clone();
    for r in &mut out.rules {
        let extra = learned
            .iter()
            .filter(|l| r.kind == ProcessKind::Name && l.rule_id == r.id)
            .map(|l| l.path.clone());
        let mut seen = std::collections::HashSet::new();
        r.guard_paths = std::mem::take(&mut r.guard_paths)
            .into_iter()
            .chain(extra)
            .filter(|g| seen.insert(exe_key(g)) && exists(g))
            .collect();
    }
    out
}

/// 删掉不再需要的学习结果：已出现在对应规则 `guard_paths` 里的、规则已不存在或不再是
/// Name + Block（或进程名已不符、与某条按路径的规则重叠）的、文件已不存在的。
pub fn forget(learned: &mut Vec<LearnedPath>, desired: &DesiredState, exists: &dyn Fn(&str) -> bool) {
    let rules = &desired.rules;
    learned.retain(|l| {
        rules.iter().find(|r| r.id == l.rule_id).is_some_and(|r| {
            learnable(r, rules, &l.path) && !has_guard(r, &l.path) && exists(&l.path)
        })
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use routa_procsvc_proto::Action;

    const P: &str = r"C:\N\ChatGPT.exe";

    fn name_rule(id: &str, name: &str, on_fault: FaultPolicy) -> RuleSpec {
        RuleSpec {
            id: id.into(),
            exe_path: name.into(),
            kind: ProcessKind::Name,
            guard_paths: vec![],
            action: Action::Direct,
            on_fault,
            reset_on_change: false,
        }
    }

    fn path_rule(id: &str, path: &str) -> RuleSpec {
        RuleSpec {
            kind: ProcessKind::Path,
            ..name_rule(id, path, FaultPolicy::Block)
        }
    }

    fn state(rules: Vec<RuleSpec>) -> DesiredState {
        DesiredState { rules, routa_pid: 1 }
    }

    fn lp(id: &str, path: &str) -> LearnedPath {
        LearnedPath { rule_id: id.into(), path: path.into() }
    }

    #[test]
    fn observe_matches_name_block_rules_only() {
        let block = name_rule("c", "chatgpt.exe", FaultPolicy::Block);
        let only = std::slice::from_ref(&block);
        assert_eq!(observe(only, P), Some(lp("c", P)));
        // 其他进程
        assert_eq!(observe(only, r"C:\N\Other.exe"), None);
        // on_fault=Direct
        assert_eq!(observe(&[name_rule("c", "chatgpt.exe", FaultPolicy::Direct)], P), None);
        // 已在 guard_paths（写法不同也算）
        let guarded = RuleSpec { guard_paths: vec![r"c:/n/CHATGPT.EXE".into()], ..block.clone() };
        assert_eq!(observe(&[guarded], P), None);
        // 某条按路径的规则就是它
        assert_eq!(observe(&[block.clone(), path_rule("p", r"c:\n\chatgpt.exe")], P), None);
        // 不是合法的本地完整路径
        assert_eq!(observe(&[block], r"\\?\C:\N\ChatGPT.exe"), None);
    }

    #[test]
    fn missing_guard_files_are_skipped_and_forgotten() {
        let gone = r"C:\Gone\chatgpt.exe";
        let routa_gone = r"C:\Old\chatgpt.exe";
        let here = r"C:\Here\chatgpt.exe";
        let exists = |p: &str| !p.contains("Gone") && !p.contains("Old");
        let desired = state(vec![
            RuleSpec {
                guard_paths: vec![routa_gone.into(), here.into()],
                ..name_rule("c", "chatgpt.exe", FaultPolicy::Block)
            },
            path_rule("p", r"C:\app\a.exe"),
        ]);
        let learned = vec![lp("c", P), lp("c", gone), lp("c", r"c:\HERE\ChatGPT.exe")];
        let eff = effective(&desired, &learned, &exists);
        assert_eq!(eff.rules[0].guard_paths, vec![here.to_string(), P.to_string()]);
        assert_eq!(eff.rules[1], desired.rules[1]);
        assert_eq!(eff.routa_pid, desired.routa_pid);

        let mut learned = vec![lp("c", P), lp("c", gone)];
        forget(&mut learned, &desired, &exists);
        assert_eq!(learned, vec![lp("c", P)]);
    }

    #[test]
    fn forget_drops_paths_routa_now_sends() {
        let all = |_: &str| true;
        let mut learned = vec![lp("c", P), lp("c", r"C:\M\chatgpt.exe")];
        let desired = state(vec![RuleSpec {
            guard_paths: vec![r"c:/n/CHATGPT.exe".into()],
            ..name_rule("c", "chatgpt.exe", FaultPolicy::Block)
        }]);
        forget(&mut learned, &desired, &all);
        assert_eq!(learned, vec![lp("c", r"C:\M\chatgpt.exe")]);

        // 规则不在了、不再是 Name+Block、进程名改了、与按路径的规则重叠：都忘掉
        for rules in [
            vec![],
            vec![name_rule("c", "chatgpt.exe", FaultPolicy::Direct)],
            vec![path_rule("c", r"C:\N\x.exe")],
            vec![name_rule("c", "other.exe", FaultPolicy::Block)],
            vec![
                name_rule("c", "chatgpt.exe", FaultPolicy::Block),
                path_rule("p", r"C:\M\chatgpt.exe"),
            ],
        ] {
            let mut learned = vec![lp("c", r"C:\M\chatgpt.exe")];
            forget(&mut learned, &state(rules.clone()), &all);
            assert!(learned.is_empty(), "{rules:?}");
        }
    }
}

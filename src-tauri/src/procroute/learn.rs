//! 按进程名阻断的规则（Name + on_fault=Block）已知的程序位置：下发给服务做 WFP 守护，
//! 合并服务从连接里学到的位置，保存规则时预先记下正在运行的同名程序。纯逻辑，不做 IO。

use std::collections::HashSet;

use routa_procsvc_proto::{guard_path_problem, FaultPolicy, LearnedPath, ProcessKind};

use super::decide::rule_of_spec;
use super::processes::RunningProcess;
use crate::model::normalize::normalize_exe_path;
use crate::model::{Model, ProcessRouting, ProcessRule, ProcessRuleId};

/// 判同用的键：去首尾空白、`/` → `\`、小写（与服务端 `exe_key` 一致）。
pub fn exe_key(p: &str) -> String {
    normalize_exe_path(p).to_lowercase()
}

/// 规则本身（不看全局开关）处在学习状态：启用、Block、至少一个按进程名的条目。
fn rule_learning(r: &ProcessRule) -> bool {
    r.user_enabled && r.on_fault == FaultPolicy::Block && r.processes.iter().any(|p| p.kind == ProcessKind::Name)
}

/// 规则正在学习位置：全局开关开、规则启用、`on_fault=Block`、至少一个 `Name` 条目。
pub fn is_learning(model: &Model, r: &ProcessRule) -> bool {
    model.process_routing.enabled && rule_learning(r)
}

/// 规则 `r` 自己有与 `path` 同键的 `Path` 条目。
fn own_path(r: &ProcessRule, path: &str) -> bool {
    let key = exe_key(path);
    r.processes.iter().any(|p| p.kind == ProcessKind::Path && exe_key(&p.value) == key)
}

/// `path` 是规则 `r` 某个 `Name` 条目的合法守护路径（文件名相符、本地完整路径、不含 `,` `;`；
/// 与服务端 Apply 校验同一判断，见 [`routa_procsvc_proto::guard_path_problem`]）。
fn names_file(r: &ProcessRule, path: &str) -> bool {
    r.processes.iter().any(|p| p.kind == ProcessKind::Name && guard_path_problem(&p.value, path).is_none())
}

/// 其他**启用**规则有与 `path` 同键的 `Path` 条目：该程序归那条规则管。
pub fn owned_by_others(model: &Model, rule: &ProcessRuleId, path: &str) -> bool {
    model.process_routing.rules.iter().any(|r| &r.id != rule && r.user_enabled && own_path(r, path))
}

/// 下发给进程名 `name` 的守护路径：`r.learned` 中文件名等于 `name`、不是本规则 `Path` 条目、
/// 也不归其他启用规则管的路径；服务端会拒收的路径（[`guard_path_problem`]）不下发，
/// 以免一条坏路径让整个 Apply 被拒。文件是否存在由服务端过滤。
pub fn guard_paths(model: &Model, r: &ProcessRule, name: &str) -> Vec<String> {
    r.learned
        .iter()
        .filter(|p| guard_path_problem(name, p).is_none())
        .filter(|p| !own_path(r, p) && !owned_by_others(model, &r.id, p))
        .cloned()
        .collect()
}

/// 把服务回报的位置并入对应规则的 `learned`（按 [`exe_key`] 去重）。只收规则仍在学习、
/// 文件名对得上某个 `Name` 条目、且不是本规则 `Path` 条目的路径（与 `prune_learned` 一致）。
/// 返回是否有改动。
pub fn merge_reported(pr: &mut ProcessRouting, reported: &[LearnedPath]) -> bool {
    let mut changed = false;
    for l in reported {
        let id = rule_of_spec(&l.rule_id);
        let Some(r) = pr.rules.iter_mut().find(|r| r.id.0 == id) else { continue };
        let path = normalize_exe_path(&l.path);
        if !rule_learning(r) || !names_file(r, &path) || own_path(r, &path) {
            continue;
        }
        let key = exe_key(&path);
        if !r.learned.iter().any(|p| exe_key(p) == key) {
            r.learned.push(path);
            changed = true;
        }
    }
    changed
}

/// 本次保存后新进入学习的进程名：`after` 处于学习状态，而 `before` 不存在、不在学习，
/// 或没有这个进程名。
pub fn newly_learning(before: Option<&ProcessRule>, after: &ProcessRule) -> Vec<String> {
    if !rule_learning(after) {
        return vec![];
    }
    let known: HashSet<String> = before
        .filter(|b| rule_learning(b))
        .map(|b| b.processes.iter().filter(|p| p.kind == ProcessKind::Name).map(|p| exe_key(&p.value)).collect())
        .unwrap_or_default();
    after
        .processes
        .iter()
        .filter(|p| p.kind == ProcessKind::Name && !known.contains(&exe_key(&p.value)))
        .map(|p| p.value.clone())
        .collect()
}

/// 保存规则时：对本次新进入学习的进程名，把正在运行的同名 exe 路径记进 `rule.learned`
/// （排除本规则的 `Path` 条目和归其他启用规则管的路径）。全局开关关闭时不记。
pub fn seed(model: &Model, before: Option<&ProcessRule>, rule: &mut ProcessRule, running: &[RunningProcess]) {
    if !model.process_routing.enabled {
        return;
    }
    for name in newly_learning(before, rule) {
        let name = exe_key(&name);
        for p in running.iter().filter(|p| exe_key(&p.name) == name) {
            let path = normalize_exe_path(&p.exe_path);
            let key = exe_key(&path);
            if guard_path_problem(&name, &path).is_some()
                || own_path(rule, &path)
                || owned_by_others(model, &rule.id, &path)
                || rule.learned.iter().any(|l| exe_key(l) == key)
            {
                continue;
            }
            rule.learned.push(path);
        }
    }
}

/// 删掉文件已不存在的已知位置；返回是否有改动。
pub fn drop_missing(pr: &mut ProcessRouting, exists: &dyn Fn(&str) -> bool) -> bool {
    let mut changed = false;
    for r in &mut pr.rules {
        let before = r.learned.len();
        r.learned.retain(|p| exists(p));
        changed |= r.learned.len() != before;
    }
    changed
}

/// 商店应用装好新版本（包全名 `full_name`，安装目录 `root`）后，各学习中规则在新目录下的
/// 同一程序：只收文件存在、不是本规则 `Path` 条目、也不归其他启用规则管的。
pub fn from_package(
    model: &Model,
    full_name: &str,
    root: &str,
    exists: &dyn Fn(&str) -> bool,
) -> Vec<(ProcessRuleId, String)> {
    model
        .process_routing
        .rules
        .iter()
        .filter(|r| is_learning(model, r))
        .flat_map(|r| {
            super::store::siblings(&r.learned, full_name, root)
                .into_iter()
                .filter(|p| {
                    names_file(r, p) && exists(p) && !own_path(r, p) && !owned_by_others(model, &r.id, p)
                })
                .map(|p| (r.id.clone(), p))
        })
        .collect()
}

/// 把 [`from_package`] 找到的位置追加到对应规则的 `learned`（按 [`exe_key`] 去重）。
pub fn add(pr: &mut ProcessRouting, found: &[(ProcessRuleId, String)]) {
    for (id, path) in found {
        let Some(r) = pr.rules.iter_mut().find(|r| &r.id == id) else { continue };
        let key = exe_key(path);
        if !r.learned.iter().any(|p| exe_key(p) == key) {
            r.learned.push(path.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::validate::process_file_name;
    use crate::model::ProcessEntry;

    fn block_rule(id: &str, entries: Vec<ProcessEntry>) -> ProcessRule {
        ProcessRule {
            id: ProcessRuleId(id.into()),
            processes: entries,
            on_fault: FaultPolicy::Block,
            ..ProcessRule::default()
        }
    }

    fn model(rules: Vec<ProcessRule>) -> Model {
        let mut m = Model::default();
        m.process_routing.enabled = true;
        m.process_routing.rules = rules;
        m
    }

    fn running(pid: u32, path: &str) -> RunningProcess {
        RunningProcess { pid, name: process_file_name(path).to_string(), exe_path: path.to_string() }
    }

    #[test]
    fn merge_reported_adds_once() {
        let mut pr = model(vec![block_rule("r1", vec![ProcessEntry::name("ChatGPT.exe")])]).process_routing;
        let reported = vec![
            LearnedPath { rule_id: "r1|chatgpt.exe".into(), path: r"C:\A\ChatGPT.exe".into() },
            LearnedPath { rule_id: "r1|chatgpt.exe".into(), path: r"c:/a/chatgpt.exe".into() },
            LearnedPath { rule_id: "gone|chatgpt.exe".into(), path: r"C:\B\ChatGPT.exe".into() },
        ];
        assert!(merge_reported(&mut pr, &reported));
        assert_eq!(pr.rules[0].learned, vec![r"C:\A\ChatGPT.exe".to_string()]);
        assert!(!merge_reported(&mut pr, &reported));
        assert_eq!(pr.rules[0].learned.len(), 1);
    }

    #[test]
    fn seed_only_for_newly_learning_names() {
        let other = ProcessRule {
            id: ProcessRuleId("r2".into()),
            processes: vec![ProcessEntry::path(r"C:\B\chrome.exe")],
            ..ProcessRule::default()
        };
        let m = model(vec![other]);
        let procs = vec![running(1, r"C:\A\chrome.exe"), running(2, r"C:\B\chrome.exe"), running(3, r"C:\A\other.exe")];

        let mut rule = block_rule("r1", vec![ProcessEntry::name("chrome.exe")]);
        seed(&m, None, &mut rule, &procs);
        assert_eq!(rule.learned, vec![r"C:\A\chrome.exe".to_string()]);

        // 之前已是同名 Block：不再预记录。
        let before = block_rule("r1", vec![ProcessEntry::name("Chrome.exe")]);
        let mut rule = block_rule("r1", vec![ProcessEntry::name("chrome.exe")]);
        seed(&m, Some(&before), &mut rule, &procs);
        assert!(rule.learned.is_empty());

        // 之前是 Direct：本次才进入学习。
        let before = ProcessRule { on_fault: FaultPolicy::Direct, ..before };
        let mut rule = block_rule("r1", vec![ProcessEntry::name("chrome.exe")]);
        seed(&m, Some(&before), &mut rule, &procs);
        assert_eq!(rule.learned, vec![r"C:\A\chrome.exe".to_string()]);
        assert_eq!(newly_learning(Some(&before), &rule), vec!["chrome.exe".to_string()]);
    }

    #[test]
    fn paths_the_service_rejects_are_never_seeded_merged_or_sent() {
        const BAD: [&str; 3] = [r"\\srv\share\chrome.exe", r"C:\a,b\chrome.exe", r"C:\a;b\chrome.exe"];
        const GOOD: &str = r"C:\A\chrome.exe";
        let m = model(vec![]);

        // seed
        let mut procs: Vec<RunningProcess> = BAD.iter().map(|p| running(1, p)).collect();
        procs.push(running(2, GOOD));
        let mut rule = block_rule("r1", vec![ProcessEntry::name("chrome.exe")]);
        seed(&m, None, &mut rule, &procs);
        assert_eq!(rule.learned, vec![GOOD.to_string()]);

        // merge_reported
        let mut pr = model(vec![block_rule("r1", vec![ProcessEntry::name("chrome.exe")])]).process_routing;
        let reported: Vec<LearnedPath> =
            BAD.iter().map(|p| LearnedPath { rule_id: "r1|chrome.exe".into(), path: p.to_string() }).collect();
        assert!(!merge_reported(&mut pr, &reported));
        assert!(pr.rules[0].learned.is_empty());

        // guard_paths: an already-stored bad path is not sent
        let mut r = block_rule("r1", vec![ProcessEntry::name("chrome.exe")]);
        r.learned = BAD.iter().map(|p| p.to_string()).chain([GOOD.to_string()]).collect();
        let m = model(vec![r.clone()]);
        assert_eq!(guard_paths(&m, &r, "chrome.exe"), vec![GOOD.to_string()]);

        // from_package: the sibling in a bad new folder is not taken
        let old = r"C:\Program Files\WindowsApps\Pkg_1.0.0.0_x64__abc";
        let mut r = block_rule("r1", vec![ProcessEntry::name("chrome.exe")]);
        r.learned = vec![format!(r"{old}\chrome.exe")];
        let m = model(vec![r]);
        let new_name = "Pkg_2.0.0.0_x64__abc";
        let all = |_: &str| true;
        assert_eq!(from_package(&m, new_name, &format!(r"C:\Program Files\WindowsApps\{new_name}"), &all).len(), 1);
        assert!(from_package(&m, new_name, &format!(r"C:\Odd,Dir\{new_name}"), &all).is_empty());
        assert!(from_package(&m, new_name, &format!(r"\\srv\share\{new_name}"), &all).is_empty());
    }

    #[test]
    fn drop_missing_removes_deleted_files() {
        let mut r = block_rule("r1", vec![ProcessEntry::name("a.exe")]);
        r.learned = vec![r"C:\A\a.exe".into(), r"C:\B\a.exe".into()];
        let mut pr = model(vec![r]).process_routing;
        let exists = |p: &str| p.starts_with(r"C:\A");
        assert!(drop_missing(&mut pr, &exists));
        assert_eq!(pr.rules[0].learned, vec![r"C:\A\a.exe".to_string()]);
        assert!(!drop_missing(&mut pr, &exists));
    }

    #[test]
    fn from_package_requires_existing_file() {
        let old = r"C:\Program Files\WindowsApps\OpenAI.Codex_26.1029.7803.0_x64__2p2nqsd0c76g0";
        let new_name = "OpenAI.Codex_26.1105.8012.0_x64__2p2nqsd0c76g0";
        let new_root = r"C:\Program Files\WindowsApps\OpenAI.Codex_26.1105.8012.0_x64__2p2nqsd0c76g0";
        let mut r = block_rule("r1", vec![ProcessEntry::name("ChatGPT.exe"), ProcessEntry::name("helper.exe")]);
        r.learned = vec![format!(r"{old}\app\ChatGPT.exe"), format!(r"{old}\app\helper.exe")];
        let mut m = model(vec![r]);
        let chatgpt = format!(r"{new_root}\app\ChatGPT.exe");
        let exists = |p: &str| p == chatgpt;

        let found = from_package(&m, new_name, new_root, &exists);
        assert_eq!(found, vec![(ProcessRuleId("r1".into()), chatgpt.clone())]);
        add(&mut m.process_routing, &found);
        add(&mut m.process_routing, &found);
        assert_eq!(m.process_routing.rules[0].learned.len(), 3);
        assert_eq!(m.process_routing.rules[0].learned[2], chatgpt);
        // 已记下后不再产出。
        assert!(from_package(&m, new_name, new_root, &exists).is_empty());

        // 归其他启用规则管的不收；全局开关关闭时不收。
        let mut m2 = model(vec![block_rule("r1", vec![ProcessEntry::name("ChatGPT.exe")])]);
        m2.process_routing.rules[0].learned = vec![format!(r"{old}\app\ChatGPT.exe")];
        m2.process_routing.rules.push(ProcessRule {
            id: ProcessRuleId("r2".into()),
            processes: vec![ProcessEntry::path(&chatgpt)],
            ..ProcessRule::default()
        });
        assert!(from_package(&m2, new_name, new_root, &exists).is_empty());
        m.process_routing.enabled = false;
        m.process_routing.rules[0].learned.truncate(2);
        assert!(from_package(&m, new_name, new_root, &exists).is_empty());
    }
}

//! 把期望规则集变成有序操作（spec 5、6.1、6.2、6.4）。
//! 顺序是"不泄露"的保证：先收紧、后放开。

use routa_procsvc_proto::{Action, FaultPolicy, ProxyTarget, RuleSpec};

use crate::engine::PbAction;

#[derive(Clone, Debug, PartialEq)]
pub enum Op {
    SetProxy {
        rule: String,
        target: ProxyTarget,
    },
    SetRule {
        rule: String,
        exe: String,
        action: PbAction,
    },
    RemoveRule {
        rule: String,
    },
    EnsureBlock {
        exe: String,
    },
    RemoveBlock {
        exe: String,
    },
    Permit {
        exe: String,
    },
    Revoke {
        exe: String,
    },
    Reset {
        exe: String,
    },
}

/// exe 路径的比较键（spec 4：路径不区分大小写）：去首尾空白、`/` 统一为 `\`、Unicode 小写。
/// Routa 侧使用相同的规范化。
pub fn exe_key(path: &str) -> String {
    path.trim().replace('/', "\\").to_lowercase()
}

/// 由旧规则集 `old` 走到新规则集 `new` 的有序操作。
///
/// ProxyBridge 规则按 id 比对；WFP 过滤器按 exe（[`exe_key`]）在新旧两个集合之间整体比对，
/// 所以 exe 从一条规则挪到另一条规则时不会出现"既无规则、又无阻断"的空窗。
/// 同 id 换了 exe（大小写不同不算）视为删除旧规则 + 新增规则。
///
/// 全局顺序（先收紧、后放开）：
/// 1. 新受保护的 exe：`EnsureBlock`
/// 2. 变成 BLOCK 的规则：`SetRule(Block)`
/// 3. 不再放行（或其 ProxyBridge 规则要短暂消失）的 exe：`Revoke`
/// 4. 保留 / 新增的非 BLOCK 规则：`SetProxy`（目标变化时）→ `SetRule`
/// 5. 换 exe 的规则：`RemoveRule` 旧的 → `SetProxy` / `SetRule` 新的
/// 6. 消失的规则：`RemoveRule`（此时同 exe 的新规则都已就位）
/// 7. 不再受保护的 exe：`RemoveBlock`
/// 8. 需要放行的 exe：`Permit`
/// 9. 动作种类变化（含新增、换 exe）且 `reset_on_change`：`Reset`
///
/// 只有 `on_fault=Block` 的规则会产生 WFP 操作（`EnsureBlock/RemoveBlock/Permit/Revoke`）。
pub fn plan(old: &[RuleSpec], new: &[RuleSpec]) -> Vec<Op> {
    let key = |r: &RuleSpec| exe_key(&r.exe_path);
    let old_of = |id: &str| old.iter().find(|o| o.id == id);
    let new_of = |id: &str| new.iter().find(|n| n.id == id);
    // 同 id、同 exe 的旧规则；None 表示新增或换了 exe。
    let prev = |n: &RuleSpec| old_of(&n.id).filter(|o| key(o) == key(n));
    let moved =
        |id: &str| matches!((old_of(id), new_of(id)), (Some(o), Some(n)) if key(o) != key(n));

    // 需要（重新）下发 ProxyBridge 规则：新增、动作种类变化或 exe 字符串变化。
    let needs_set = |n: &RuleSpec| match prev(n) {
        None => true,
        Some(o) => kind(&o.action) != kind(&n.action) || o.exe_path != n.exe_path,
    };

    // 涉及到的 exe，按首次出现的顺序；WFP 操作使用新规则里的写法。
    let mut exes: Vec<(String, &str)> = Vec::new();
    for r in new.iter().chain(old) {
        let k = key(r);
        if !exes.iter().any(|(e, _)| *e == k) {
            exes.push((k, &r.exe_path));
        }
    }
    let any = |rules: &[RuleSpec], k: &str, f: fn(&RuleSpec) -> bool| {
        rules.iter().any(|r| key(r) == k && f(r))
    };
    // 第 5 步会删掉该 exe 的某条旧规则，而第 5 步之前没有别的规则覆盖它：
    // 这段时间要靠 WFP 阻断兜底，所以先撤放行、最后再放行。
    let disturbed = |k: &str| {
        old.iter().any(|o| moved(&o.id) && key(o) == k)
            && !new.iter().any(|n| !moved(&n.id) && key(n) == k)
    };

    let mut ops = Vec::new();
    let set_rule = |n: &RuleSpec, action| Op::SetRule {
        rule: n.id.clone(),
        exe: n.exe_path.clone(),
        action,
    };
    let set_proxy = |n: &RuleSpec, ops: &mut Vec<Op>| {
        if let Action::Proxy(target) = &n.action {
            if prev(n).map(|o| &o.action) != Some(&n.action) {
                ops.push(Op::SetProxy {
                    rule: n.id.clone(),
                    target: target.clone(),
                });
            }
        }
    };

    // 1
    for (k, exe) in &exes {
        if any(new, k, guarded) && !any(old, k, guarded) {
            ops.push(Op::EnsureBlock {
                exe: exe.to_string(),
            });
        }
    }
    // 2
    for n in new {
        if !moved(&n.id) && kind(&n.action) == PbAction::Block && needs_set(n) {
            ops.push(set_rule(n, PbAction::Block));
        }
    }
    // 3
    for (k, exe) in &exes {
        if any(old, k, permitted) && (!any(new, k, permitted) || disturbed(k)) {
            ops.push(Op::Revoke {
                exe: exe.to_string(),
            });
        }
    }
    // 4
    for n in new {
        if !moved(&n.id) && kind(&n.action) != PbAction::Block {
            set_proxy(n, &mut ops);
            if needs_set(n) {
                ops.push(set_rule(n, kind(&n.action)));
            }
        }
    }
    // 5
    for o in old {
        if moved(&o.id) {
            ops.push(Op::RemoveRule { rule: o.id.clone() });
        }
    }
    for n in new {
        if moved(&n.id) {
            set_proxy(n, &mut ops);
            ops.push(set_rule(n, kind(&n.action)));
        }
    }
    // 6
    for o in old {
        if new_of(&o.id).is_none() {
            ops.push(Op::RemoveRule { rule: o.id.clone() });
        }
    }
    // 7、8
    for (k, exe) in &exes {
        if any(old, k, guarded) && !any(new, k, guarded) {
            ops.push(Op::RemoveBlock {
                exe: exe.to_string(),
            });
        }
    }
    for (k, exe) in &exes {
        if any(new, k, permitted) && (!any(old, k, permitted) || disturbed(k)) {
            ops.push(Op::Permit {
                exe: exe.to_string(),
            });
        }
    }
    // 9
    for n in new {
        if n.reset_on_change && prev(n).map(|o| kind(&o.action)) != Some(kind(&n.action)) {
            ops.push(Op::Reset {
                exe: n.exe_path.clone(),
            });
        }
    }
    ops
}

fn kind(a: &Action) -> PbAction {
    match a {
        Action::Proxy(_) => PbAction::Proxy,
        Action::Direct => PbAction::Direct,
        Action::Block => PbAction::Block,
    }
}

/// 该规则需要 WFP 永久阻断。
fn guarded(r: &RuleSpec) -> bool {
    r.on_fault == FaultPolicy::Block
}

/// 该规则需要 WFP 动态放行。
fn permitted(r: &RuleSpec) -> bool {
    guarded(r) && r.action != Action::Block
}

#[cfg(test)]
mod tests {
    use super::*;
    use routa_procsvc_proto::ProxyKind;
    use std::collections::{HashMap, HashSet};

    const EXE: &str = r"C:\app\a.exe";
    const EXE_OTHER_CASE: &str = "C:/APP/A.EXE";
    const OTHER_EXE: &str = r"C:\app\b.exe";

    fn target(port: u16) -> ProxyTarget {
        ProxyTarget {
            kind: ProxyKind::Socks5,
            host: "127.0.0.1".into(),
            port,
            username: None,
            password: None,
        }
    }

    fn spec(id: &str, exe: &str, action: Action, on_fault: FaultPolicy, reset: bool) -> RuleSpec {
        RuleSpec {
            id: id.into(),
            exe_path: exe.into(),
            action,
            on_fault,
            reset_on_change: reset,
        }
    }

    fn r(action: Action, on_fault: FaultPolicy) -> RuleSpec {
        spec("r", EXE, action, on_fault, true)
    }

    fn pos(ops: &[Op], pred: impl Fn(&Op) -> bool) -> Option<usize> {
        ops.iter().position(pred)
    }

    fn is_wfp(op: &Op) -> bool {
        matches!(
            op,
            Op::EnsureBlock { .. } | Op::RemoveBlock { .. } | Op::Permit { .. } | Op::Revoke { .. }
        )
    }

    fn count_resets(ops: &[Op]) -> usize {
        ops.iter().filter(|o| matches!(o, Op::Reset { .. })).count()
    }

    #[test]
    fn exe_key_normalizes() {
        assert_eq!(
            exe_key(" C:/Program Files/APP.exe "),
            r"c:\program files\app.exe"
        );
        assert_eq!(exe_key(r"D:\ÄÖ\Ünï.EXE"), exe_key(r"d:/äö/ünï.exe"));
    }

    #[test]
    fn to_block_tightens_before_revoke() {
        for from in [Action::Proxy(target(1)), Action::Direct] {
            let ops = plan(
                &[r(from, FaultPolicy::Block)],
                &[r(Action::Block, FaultPolicy::Block)],
            );
            let set = pos(&ops, |o| {
                matches!(
                    o,
                    Op::SetRule {
                        action: PbAction::Block,
                        ..
                    }
                )
            })
            .expect("SetRule(Block)");
            let revoke = pos(&ops, |o| matches!(o, Op::Revoke { .. })).expect("Revoke");
            assert!(set < revoke, "{ops:?}");
        }
    }

    #[test]
    fn from_block_permits_last() {
        for to in [Action::Proxy(target(1)), Action::Direct] {
            let ops = plan(
                &[r(Action::Block, FaultPolicy::Block)],
                &[r(to, FaultPolicy::Block)],
            );
            let non_reset: Vec<&Op> = ops
                .iter()
                .filter(|o| !matches!(o, Op::Reset { .. }))
                .collect();
            assert!(
                matches!(non_reset.last(), Some(Op::Permit { .. })),
                "{ops:?}"
            );
            assert!(
                pos(&ops, |o| matches!(o, Op::SetRule { .. })).is_some(),
                "{ops:?}"
            );
        }
    }

    #[test]
    fn reset_only_on_action_change() {
        let ops = plan(
            &[r(Action::Direct, FaultPolicy::Direct)],
            &[r(Action::Proxy(target(1)), FaultPolicy::Direct)],
        );
        assert_eq!(count_resets(&ops), 1, "{ops:?}");
        assert!(matches!(ops.last(), Some(Op::Reset { .. })), "{ops:?}");

        let ops = plan(
            &[r(Action::Proxy(target(1)), FaultPolicy::Direct)],
            &[r(Action::Proxy(target(2)), FaultPolicy::Direct)],
        );
        assert_eq!(count_resets(&ops), 0, "{ops:?}");

        // reset_on_change=false 时动作变化也不 Reset
        let ops = plan(
            &[spec("r", EXE, Action::Direct, FaultPolicy::Direct, false)],
            &[spec("r", EXE, Action::Block, FaultPolicy::Direct, false)],
        );
        assert_eq!(count_resets(&ops), 0, "{ops:?}");
    }

    #[test]
    fn proxy_target_change_edits_config_without_reset() {
        for fault in [FaultPolicy::Direct, FaultPolicy::Block] {
            let ops = plan(
                &[r(Action::Proxy(target(1)), fault.clone())],
                &[r(Action::Proxy(target(2)), fault)],
            );
            assert_eq!(
                ops,
                vec![Op::SetProxy {
                    rule: "r".into(),
                    target: target(2)
                }]
            );
        }
    }

    #[test]
    fn unchanged_rule_is_noop() {
        let rules = [r(Action::Proxy(target(1)), FaultPolicy::Block)];
        assert!(plan(&rules, &rules).is_empty());
    }

    fn all_specs() -> Vec<RuleSpec> {
        let mut v = Vec::new();
        for a in [
            Action::Proxy(target(1)),
            Action::Proxy(target(2)),
            Action::Direct,
            Action::Block,
        ] {
            for f in [FaultPolicy::Direct, FaultPolicy::Block] {
                for reset in [false, true] {
                    v.push(spec("r", EXE, a.clone(), f.clone(), reset));
                }
            }
        }
        v
    }

    #[test]
    fn direct_fault_rule_never_touches_wfp() {
        let direct: Vec<RuleSpec> = all_specs()
            .into_iter()
            .filter(|s| s.on_fault == FaultPolicy::Direct)
            .collect();
        let mut cases: Vec<(Vec<RuleSpec>, Vec<RuleSpec>)> = Vec::new();
        for o in &direct {
            cases.push((vec![o.clone()], vec![]));
            cases.push((vec![], vec![o.clone()]));
            for n in &direct {
                cases.push((vec![o.clone()], vec![n.clone()]));
                for exe in [OTHER_EXE, EXE_OTHER_CASE] {
                    let mut moved = n.clone();
                    moved.exe_path = exe.into();
                    cases.push((vec![o.clone()], vec![moved.clone()]));
                    // exe 在两条规则之间挪动
                    moved.id = "s".into();
                    cases.push((vec![o.clone()], vec![moved]));
                }
            }
        }
        for (old, new) in cases {
            let ops = plan(&old, &new);
            assert!(!ops.iter().any(is_wfp), "{old:?} -> {new:?}: {ops:?}");
        }
    }

    #[test]
    fn removed_rule_cleans_up() {
        // on_fault=Block 且动作非 Block：Revoke → RemoveRule → RemoveBlock
        let ops = plan(&[r(Action::Proxy(target(1)), FaultPolicy::Block)], &[]);
        assert_eq!(
            ops,
            vec![
                Op::Revoke { exe: EXE.into() },
                Op::RemoveRule { rule: "r".into() },
                Op::RemoveBlock { exe: EXE.into() },
            ]
        );
        // on_fault=Block 且动作为 Block：本来就没有放行
        let ops = plan(&[r(Action::Block, FaultPolicy::Block)], &[]);
        assert_eq!(
            ops,
            vec![
                Op::RemoveRule { rule: "r".into() },
                Op::RemoveBlock { exe: EXE.into() }
            ]
        );
        // on_fault=Direct：只删 ProxyBridge 规则
        let ops = plan(&[r(Action::Direct, FaultPolicy::Direct)], &[]);
        assert_eq!(ops, vec![Op::RemoveRule { rule: "r".into() }]);
    }

    #[test]
    fn new_block_fault_rule_tightens_first() {
        let ops = plan(&[], &[r(Action::Proxy(target(1)), FaultPolicy::Block)]);
        assert_eq!(
            ops,
            vec![
                Op::EnsureBlock { exe: EXE.into() },
                Op::SetProxy {
                    rule: "r".into(),
                    target: target(1)
                },
                Op::SetRule {
                    rule: "r".into(),
                    exe: EXE.into(),
                    action: PbAction::Proxy
                },
                Op::Permit { exe: EXE.into() },
                Op::Reset { exe: EXE.into() },
            ]
        );
    }

    #[test]
    fn exe_change_is_remove_then_add() {
        let ops = plan(
            &[spec("r", EXE, Action::Direct, FaultPolicy::Block, false)],
            &[spec(
                "r",
                OTHER_EXE,
                Action::Direct,
                FaultPolicy::Block,
                false,
            )],
        );
        assert_eq!(
            ops,
            vec![
                Op::EnsureBlock {
                    exe: OTHER_EXE.into()
                },
                Op::Revoke { exe: EXE.into() },
                Op::RemoveRule { rule: "r".into() },
                Op::SetRule {
                    rule: "r".into(),
                    exe: OTHER_EXE.into(),
                    action: PbAction::Direct
                },
                Op::RemoveBlock { exe: EXE.into() },
                Op::Permit {
                    exe: OTHER_EXE.into()
                },
            ]
        );
    }

    #[test]
    fn case_only_exe_change_keeps_wfp() {
        let ops = plan(
            &[spec(
                "r",
                EXE,
                Action::Proxy(target(1)),
                FaultPolicy::Block,
                true,
            )],
            &[spec(
                "r",
                EXE_OTHER_CASE,
                Action::Proxy(target(1)),
                FaultPolicy::Block,
                true,
            )],
        );
        assert_eq!(
            ops,
            vec![Op::SetRule {
                rule: "r".into(),
                exe: EXE_OTHER_CASE.into(),
                action: PbAction::Proxy
            }]
        );
    }

    #[test]
    fn exe_moving_between_rules_has_no_unmanaged_window() {
        // 规则 a 删除、规则 b 新增，二者是同一个 on_fault=Block 的 exe
        let ops = plan(
            &[spec(
                "a",
                EXE,
                Action::Proxy(target(1)),
                FaultPolicy::Block,
                false,
            )],
            &[spec(
                "b",
                EXE,
                Action::Proxy(target(1)),
                FaultPolicy::Block,
                false,
            )],
        );
        assert_eq!(
            ops,
            vec![
                Op::SetProxy {
                    rule: "b".into(),
                    target: target(1)
                },
                Op::SetRule {
                    rule: "b".into(),
                    exe: EXE.into(),
                    action: PbAction::Proxy
                },
                Op::RemoveRule { rule: "a".into() },
            ]
        );
    }

    #[test]
    fn exe_swap_between_ids_falls_back_to_wfp_block() {
        // a: X→Y，b: Y→X。删旧规则时 X、Y 都暂时没有 ProxyBridge 规则，必须先撤放行
        let ops = plan(
            &[
                spec(
                    "a",
                    EXE,
                    Action::Proxy(target(1)),
                    FaultPolicy::Block,
                    false,
                ),
                spec(
                    "b",
                    OTHER_EXE,
                    Action::Proxy(target(1)),
                    FaultPolicy::Block,
                    false,
                ),
            ],
            &[
                spec(
                    "a",
                    OTHER_EXE,
                    Action::Proxy(target(1)),
                    FaultPolicy::Block,
                    false,
                ),
                spec(
                    "b",
                    EXE,
                    Action::Proxy(target(1)),
                    FaultPolicy::Block,
                    false,
                ),
            ],
        );
        let first_remove = pos(&ops, |o| matches!(o, Op::RemoveRule { .. })).unwrap();
        let last_set = ops
            .iter()
            .rposition(|o| matches!(o, Op::SetRule { .. }))
            .unwrap();
        for exe in [EXE, OTHER_EXE] {
            let revoke = pos(&ops, |o| *o == Op::Revoke { exe: exe.into() }).unwrap();
            let permit = pos(&ops, |o| *o == Op::Permit { exe: exe.into() }).unwrap();
            assert!(revoke < first_remove && permit > last_set, "{ops:?}");
        }
        check(
            &[
                spec(
                    "a",
                    EXE,
                    Action::Proxy(target(1)),
                    FaultPolicy::Block,
                    false,
                ),
                spec(
                    "b",
                    OTHER_EXE,
                    Action::Proxy(target(1)),
                    FaultPolicy::Block,
                    false,
                ),
            ],
            &[
                spec(
                    "a",
                    OTHER_EXE,
                    Action::Proxy(target(1)),
                    FaultPolicy::Block,
                    false,
                ),
                spec(
                    "b",
                    EXE,
                    Action::Proxy(target(1)),
                    FaultPolicy::Block,
                    false,
                ),
            ],
        );
    }

    #[test]
    fn fault_policy_switch() {
        // Block → Direct：最终既无永久阻断也无放行
        let ops = plan(
            &[r(Action::Direct, FaultPolicy::Block)],
            &[r(Action::Direct, FaultPolicy::Direct)],
        );
        assert_eq!(
            ops,
            vec![
                Op::Revoke { exe: EXE.into() },
                Op::RemoveBlock { exe: EXE.into() }
            ]
        );
        // Direct → Block：先 EnsureBlock，动作非 Block 再 Permit
        let ops = plan(
            &[r(Action::Direct, FaultPolicy::Direct)],
            &[r(Action::Direct, FaultPolicy::Block)],
        );
        assert_eq!(
            ops,
            vec![
                Op::EnsureBlock { exe: EXE.into() },
                Op::Permit { exe: EXE.into() }
            ]
        );
        let ops = plan(
            &[r(Action::Block, FaultPolicy::Direct)],
            &[r(Action::Block, FaultPolicy::Block)],
        );
        assert_eq!(ops, vec![Op::EnsureBlock { exe: EXE.into() }]);
    }

    // ---------- 模拟执行：逐步检查安全不变量与终态 ----------

    #[derive(Debug, Default)]
    struct Sim {
        rules: HashMap<String, (String, PbAction)>,
        proxies: HashMap<String, ProxyTarget>,
        /// 以 exe_key 为键。
        blocks: HashSet<String>,
        permits: HashSet<String>,
    }

    impl Sim {
        fn built(rules: &[RuleSpec]) -> Sim {
            let mut s = Sim::default();
            for op in plan(&[], rules) {
                s.apply(&op);
            }
            s
        }

        fn apply(&mut self, op: &Op) {
            match op {
                Op::SetProxy { rule, target } => {
                    self.proxies.insert(rule.clone(), target.clone());
                }
                Op::SetRule { rule, exe, action } => {
                    self.rules.insert(rule.clone(), (exe.clone(), *action));
                }
                Op::RemoveRule { rule } => {
                    assert!(
                        self.rules.remove(rule).is_some(),
                        "RemoveRule of absent {rule}"
                    );
                    self.proxies.remove(rule);
                }
                Op::EnsureBlock { exe } => {
                    self.blocks.insert(exe_key(exe));
                }
                Op::RemoveBlock { exe } => {
                    self.blocks.remove(&exe_key(exe));
                }
                Op::Permit { exe } => {
                    self.permits.insert(exe_key(exe));
                }
                Op::Revoke { exe } => {
                    self.permits.remove(&exe_key(exe));
                }
                Op::Reset { .. } => {}
            }
        }

        fn has_pb_rule_for(&self, k: &str) -> bool {
            self.rules.values().any(|(e, _)| exe_key(e) == k)
        }
    }

    fn guarded_keys(rules: &[RuleSpec]) -> HashSet<String> {
        rules
            .iter()
            .filter(|r| guarded(r))
            .map(|r| exe_key(&r.exe_path))
            .collect()
    }

    /// 对 old → new 执行 plan，逐步检查不变量，最后检查终态与 Reset。
    fn check(old: &[RuleSpec], new: &[RuleSpec]) {
        let ctx = format!("{old:?} -> {new:?}");
        let ops = plan(old, new);
        let ctx = format!("{ctx}: {ops:?}");
        let both_guarded: Vec<String> = guarded_keys(old)
            .intersection(&guarded_keys(new))
            .cloned()
            .collect();
        let same_exe = |a: &RuleSpec, b: &RuleSpec| exe_key(&a.exe_path) == exe_key(&b.exe_path);
        let prev = |n: &RuleSpec| old.iter().find(|o| o.id == n.id && same_exe(o, n));

        let mut sim = Sim::built(old);
        for op in &ops {
            sim.apply(op);
            // 新旧都受保护的 exe：任何时刻要么"有阻断无放行"，要么有 ProxyBridge 规则管着
            for k in &both_guarded {
                assert!(
                    (sim.blocks.contains(k) && !sim.permits.contains(k)) || sim.has_pb_rule_for(k),
                    "{ctx}: {k} unmanaged after {op:?}: {sim:?}"
                );
            }
            // 撤放行时：同 exe、同 id 保留下来的 on_fault=Block 且目标为 BLOCK 的规则必须已是 Block
            if let Op::Revoke { exe } = op {
                for n in new.iter().filter(|n| {
                    exe_key(&n.exe_path) == exe_key(exe)
                        && guarded(n)
                        && n.action == Action::Block
                        && old.iter().all(|o| o.id != n.id || same_exe(o, n))
                }) {
                    assert_eq!(
                        sim.rules.get(&n.id).map(|(_, a)| *a),
                        Some(PbAction::Block),
                        "{ctx}: Revoke before SetRule(Block)"
                    );
                }
            }
            // SetRule(Proxy) 时代理目标必须已是新目标
            if let Op::SetRule {
                rule,
                action: PbAction::Proxy,
                ..
            } = op
            {
                let n = new.iter().find(|n| &n.id == rule).unwrap();
                let Action::Proxy(t) = &n.action else {
                    panic!("{ctx}")
                };
                assert_eq!(
                    sim.proxies.get(rule),
                    Some(t),
                    "{ctx}: SetProxy must precede SetRule(Proxy)"
                );
            }
        }

        let want = Sim::built(new);
        assert_eq!(sim.rules, want.rules, "{ctx}");
        assert_eq!(sim.blocks, want.blocks, "{ctx}");
        assert_eq!(sim.permits, want.permits, "{ctx}");
        for n in new {
            if let Action::Proxy(t) = &n.action {
                assert_eq!(sim.proxies.get(&n.id), Some(t), "{ctx}");
            }
        }

        // Reset：动作种类变化（含新增、换 exe）且 reset_on_change 时每条规则一次，都在最后
        let want_resets = new
            .iter()
            .filter(|n| {
                n.reset_on_change && prev(n).map(|o| kind(&o.action)) != Some(kind(&n.action))
            })
            .count();
        assert_eq!(count_resets(&ops), want_resets, "{ctx}");
        assert!(
            ops[ops.len() - want_resets..]
                .iter()
                .all(|o| matches!(o, Op::Reset { .. })),
            "{ctx}"
        );
    }

    #[test]
    fn exhaustive_single_rule_transitions() {
        let mut olds: Vec<Option<RuleSpec>> = vec![None];
        olds.extend(all_specs().into_iter().map(Some));
        let mut news = olds.clone();
        for exe in [OTHER_EXE, EXE_OTHER_CASE] {
            news.extend(all_specs().into_iter().map(|mut s| {
                s.exe_path = exe.into();
                Some(s)
            }));
        }
        for old in &olds {
            for new in &news {
                let old_v: Vec<RuleSpec> = old.iter().cloned().collect();
                let new_v: Vec<RuleSpec> = new.iter().cloned().collect();
                check(&old_v, &new_v);
            }
        }
    }

    #[test]
    fn exhaustive_two_rule_transitions() {
        // 每个槽位：不存在，或 3 种 exe 写法（含大小写变体）× 3 种动作 × 2 种 on_fault
        let slot = |id: &str| {
            let mut v: Vec<Option<RuleSpec>> = vec![None];
            for exe in [EXE, EXE_OTHER_CASE, OTHER_EXE] {
                for a in [Action::Proxy(target(1)), Action::Direct, Action::Block] {
                    for f in [FaultPolicy::Direct, FaultPolicy::Block] {
                        v.push(Some(spec(id, exe, a.clone(), f, true)));
                    }
                }
            }
            v
        };
        // 同一集合内 exe 不重复
        let mut sets: Vec<Vec<RuleSpec>> = Vec::new();
        for a in slot("a") {
            for b in slot("b") {
                let set: Vec<RuleSpec> = a.iter().chain(b.iter()).cloned().collect();
                if set.len() == 2 && exe_key(&set[0].exe_path) == exe_key(&set[1].exe_path) {
                    continue;
                }
                sets.push(set);
            }
        }
        for old in &sets {
            for new in &sets {
                check(old, new);
            }
        }
    }

    #[test]
    fn multiple_rules_are_planned_independently() {
        let a = spec(
            "a",
            r"C:\a.exe",
            Action::Proxy(target(1)),
            FaultPolicy::Block,
            false,
        );
        let b = spec("b", r"C:\b.exe", Action::Direct, FaultPolicy::Direct, false);
        let c = spec("c", r"C:\c.exe", Action::Block, FaultPolicy::Block, false);
        let a2 = spec("a", r"C:\a.exe", Action::Block, FaultPolicy::Block, false);
        check(&[a, b.clone()], &[b, a2, c]);
    }
}

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

/// 由旧规则集 `old` 走到新规则集 `new` 的有序操作。
///
/// 先处理消失的规则（以及换了 exe 的同 id 规则的旧一半），再按 `new` 的顺序逐条处理。
/// 单条规则内部顺序为"先收紧、后放开"：
/// 1. 新增防护：`EnsureBlock`
/// 2. 变成 BLOCK：`SetRule(Block)`，之后才 `Revoke`
/// 3. 放开：`SetProxy`（若目标变化）→ `SetRule` → `RemoveBlock` / `Permit`
/// 4. 动作种类变化且 `reset_on_change`：最后 `Reset`
///
/// 只有 `on_fault=Block` 的规则会产生 WFP 操作（`EnsureBlock/RemoveBlock/Permit/Revoke`）。
pub fn plan(old: &[RuleSpec], new: &[RuleSpec]) -> Vec<Op> {
    let mut ops = Vec::new();
    for o in old {
        let kept = new.iter().any(|n| n.id == o.id && n.exe_path == o.exe_path);
        if !kept {
            plan_remove(o, &mut ops);
        }
    }
    for n in new {
        let prev = old
            .iter()
            .find(|o| o.id == n.id && o.exe_path == n.exe_path);
        plan_update(prev, n, &mut ops);
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

/// 该规则有 WFP 永久阻断。
fn guarded(r: &RuleSpec) -> bool {
    r.on_fault == FaultPolicy::Block
}

/// 该规则有 WFP 动态放行。
fn permitted(r: &RuleSpec) -> bool {
    guarded(r) && r.action != Action::Block
}

fn plan_remove(o: &RuleSpec, ops: &mut Vec<Op>) {
    let exe = &o.exe_path;
    if permitted(o) {
        ops.push(Op::Revoke { exe: exe.clone() });
    }
    ops.push(Op::RemoveRule { rule: o.id.clone() });
    if guarded(o) {
        ops.push(Op::RemoveBlock { exe: exe.clone() });
    }
}

/// `old` 为 None 表示新增规则；Some 时 id 与 exe 都与 `n` 相同。
fn plan_update(old: Option<&RuleSpec>, n: &RuleSpec, ops: &mut Vec<Op>) {
    let exe = &n.exe_path;
    let old_kind = old.map(|o| kind(&o.action));
    let new_kind = kind(&n.action);
    let was_guarded = old.is_some_and(guarded);
    let was_permitted = old.is_some_and(permitted);
    let set_rule = |action| Op::SetRule {
        rule: n.id.clone(),
        exe: exe.clone(),
        action,
    };

    // 收紧
    if guarded(n) && !was_guarded {
        ops.push(Op::EnsureBlock { exe: exe.clone() });
    }
    if new_kind == PbAction::Block && old_kind != Some(PbAction::Block) {
        ops.push(set_rule(PbAction::Block));
    }
    if was_permitted && !permitted(n) {
        ops.push(Op::Revoke { exe: exe.clone() });
    }

    // 放开
    if let Action::Proxy(target) = &n.action {
        if old.map(|o| &o.action) != Some(&n.action) {
            ops.push(Op::SetProxy {
                rule: n.id.clone(),
                target: target.clone(),
            });
        }
    }
    if new_kind != PbAction::Block && old_kind != Some(new_kind) {
        ops.push(set_rule(new_kind));
    }
    if was_guarded && !guarded(n) {
        ops.push(Op::RemoveBlock { exe: exe.clone() });
    }
    if permitted(n) && !was_permitted {
        ops.push(Op::Permit { exe: exe.clone() });
    }

    if n.reset_on_change && old_kind != Some(new_kind) {
        ops.push(Op::Reset { exe: exe.clone() });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use routa_procsvc_proto::ProxyKind;
    use std::collections::HashMap;

    const EXE: &str = r"C:\app\a.exe";
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
                let mut moved = n.clone();
                moved.exe_path = OTHER_EXE.into();
                cases.push((vec![o.clone()], vec![moved]));
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
                Op::Revoke { exe: EXE.into() },
                Op::RemoveRule { rule: "r".into() },
                Op::RemoveBlock { exe: EXE.into() },
                Op::EnsureBlock {
                    exe: OTHER_EXE.into()
                },
                Op::SetRule {
                    rule: "r".into(),
                    exe: OTHER_EXE.into(),
                    action: PbAction::Direct
                },
                Op::Permit {
                    exe: OTHER_EXE.into()
                },
            ]
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

    // ---------- 模拟执行：穷举单规则变化，逐步检查安全不变量与终态 ----------

    #[derive(Debug, Default)]
    struct Sim {
        rules: HashMap<String, (String, PbAction)>,
        proxies: HashMap<String, ProxyTarget>,
        blocks: HashMap<String, ()>,
        permits: HashMap<String, ()>,
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
                    self.rules.remove(rule);
                    self.proxies.remove(rule);
                }
                Op::EnsureBlock { exe } => {
                    self.blocks.insert(exe.clone(), ());
                }
                Op::RemoveBlock { exe } => {
                    self.blocks.remove(exe);
                }
                Op::Permit { exe } => {
                    self.permits.insert(exe.clone(), ());
                }
                Op::Revoke { exe } => {
                    self.permits.remove(exe);
                }
                Op::Reset { .. } => {}
            }
        }
    }

    fn kind(a: &Action) -> PbAction {
        match a {
            Action::Proxy(_) => PbAction::Proxy,
            Action::Direct => PbAction::Direct,
            Action::Block => PbAction::Block,
        }
    }

    /// 期望终态。
    fn assert_final(sim: &Sim, new: Option<&RuleSpec>, ctx: &str) {
        let Some(n) = new else {
            assert!(sim.rules.is_empty(), "{ctx}: {sim:?}");
            assert!(sim.blocks.is_empty(), "{ctx}: {sim:?}");
            assert!(sim.permits.is_empty(), "{ctx}: {sim:?}");
            return;
        };
        let fb = n.on_fault == FaultPolicy::Block;
        let want_permit = fb && n.action != Action::Block;
        assert_eq!(sim.rules.len(), 1, "{ctx}: {sim:?}");
        assert_eq!(
            sim.rules.get(&n.id),
            Some(&(n.exe_path.clone(), kind(&n.action))),
            "{ctx}"
        );
        if let Action::Proxy(t) = &n.action {
            assert_eq!(sim.proxies.get(&n.id), Some(t), "{ctx}");
        }
        let blocks: Vec<&String> = sim.blocks.keys().collect();
        let want: Vec<&String> = if fb { vec![&n.exe_path] } else { vec![] };
        assert_eq!(blocks, want, "{ctx}");
        let permits: Vec<&String> = sim.permits.keys().collect();
        let want: Vec<&String> = if want_permit {
            vec![&n.exe_path]
        } else {
            vec![]
        };
        assert_eq!(permits, want, "{ctx}");
    }

    /// 每一步之后都要成立的安全不变量。
    fn assert_step(sim: &Sim, op: &Op, new: Option<&RuleSpec>, ctx: &str) {
        let Some(n) = new else { return };
        // 目标是 BLOCK 且 on_fault=Block：放行存在时，ProxyBridge 规则若存在必须已是 Block
        if n.on_fault == FaultPolicy::Block
            && n.action == Action::Block
            && sim.permits.contains_key(&n.exe_path)
        {
            if let Some((_, a)) = sim.rules.get(&n.id) {
                assert_eq!(*a, PbAction::Block, "{ctx}: after {op:?}: {sim:?}");
            }
        }
        // SetRule(Proxy) 时代理目标必须已是新目标
        if let (
            Op::SetRule {
                rule,
                action: PbAction::Proxy,
                ..
            },
            Action::Proxy(t),
        ) = (op, &n.action)
        {
            assert_eq!(
                sim.proxies.get(rule),
                Some(t),
                "{ctx}: SetProxy must precede SetRule(Proxy)"
            );
        }
    }

    #[test]
    fn exhaustive_transitions_are_safe_and_converge() {
        let mut olds: Vec<Option<RuleSpec>> = vec![None];
        olds.extend(all_specs().into_iter().map(Some));
        let mut news = olds.clone();
        news.extend(all_specs().into_iter().map(|mut s| {
            s.exe_path = OTHER_EXE.into();
            Some(s)
        }));
        for old in &olds {
            for new in &news {
                let old_v: Vec<RuleSpec> = old.iter().cloned().collect();
                let new_v: Vec<RuleSpec> = new.iter().cloned().collect();
                let ctx = format!("{old:?} -> {new:?}");
                let mut sim = Sim::built(&old_v);
                let ops = plan(&old_v, &new_v);
                for op in &ops {
                    sim.apply(op);
                    assert_step(&sim, op, new.as_ref(), &ctx);
                }
                assert_final(&sim, new.as_ref(), &ctx);

                // Reset：仅在动作种类变化（含新建、换 exe）且 reset_on_change 时，恰好一次且在最后
                let want_reset = match (old, new) {
                    (_, None) => false,
                    (None, Some(n)) => n.reset_on_change,
                    (Some(o), Some(n)) => {
                        n.reset_on_change
                            && (o.exe_path != n.exe_path || kind(&o.action) != kind(&n.action))
                    }
                };
                assert_eq!(count_resets(&ops), want_reset as usize, "{ctx}: {ops:?}");
                if want_reset {
                    assert!(
                        matches!(ops.last(), Some(Op::Reset { .. })),
                        "{ctx}: {ops:?}"
                    );
                }
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
        let old = vec![a, b.clone()];
        let new = vec![b, a2, c];
        let mut sim = Sim::built(&old);
        for op in plan(&old, &new) {
            sim.apply(&op);
        }
        // 规则改成 Block 后残留的代理配置不影响行为，只比较规则与 WFP 状态
        let want = Sim::built(&new);
        assert_eq!(sim.rules, want.rules);
        assert_eq!(sim.blocks, want.blocks);
        assert_eq!(sim.permits, want.permits);
    }
}

//! 把期望规则集变成有序操作（spec 5、6.1、6.2、6.4）。
//! 顺序是"不泄露"的保证：先收紧、后放开。`Reconciler` 把这些操作落到引擎上。

use routa_procsvc_proto::{Action, FaultPolicy, ProxyTarget, RuleSpec, RuleStatus};

use std::collections::{HashMap, HashSet};

use crate::engine::{ConnKiller, Guard, Interceptor, PbAction};

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

/// Reconciler 所知的、已经落到引擎上的状态。exe 状态以 [`exe_key`] 为键，值为下发时的写法。
#[derive(Debug, Default)]
struct Installed {
    /// 规则 id → (exe, 动作)。
    rules: HashMap<String, (String, PbAction)>,
    blocks: HashMap<String, String>,
    permits: HashMap<String, String>,
}

/// 一次 `apply` 中失败的范围及错误。
#[derive(Default)]
struct Failures {
    ids: HashSet<String>,
    keys: HashSet<String>,
    errors: Vec<String>,
}

/// 把期望规则集按 [`plan`] 的顺序落到三个引擎上。
///
/// 失败处理（失败时只会更紧、不会更松）：某个操作失败后，与它相关的规则 id 和 exe
/// （经新旧规则集里的 id↔exe 关系传递闭包）都视为失败——其余放开类操作全部跳过，
/// 只保留收紧操作（`EnsureBlock`、`Revoke`）；这些规则改为 ProxyBridge `Block`
/// 并撤销其 exe 的放行，在 `current()` 里记为 `Block`。其它规则照常执行。
/// 由于 `current()` 与期望不同，下次以同样的期望 `apply` 会重试。
pub struct Reconciler<I: Interceptor, G: Guard, K: ConnKiller> {
    interceptor: I,
    guard: G,
    killer: K,
    current: Vec<RuleSpec>,
    installed: Installed,
    status: Vec<RuleStatus>,
    pending_resets: HashMap<String, String>,
    unknown_ids: HashSet<String>,
    unknown_keys: HashSet<String>,
}

impl<I: Interceptor, G: Guard, K: ConnKiller> Reconciler<I, G, K> {
    pub fn new(interceptor: I, guard: G, killer: K) -> Self {
        Reconciler {
            interceptor,
            guard,
            killer,
            current: Vec::new(),
            installed: Installed::default(),
            status: Vec::new(),
            pending_resets: HashMap::new(),
            unknown_ids: HashSet::new(),
            unknown_keys: HashSet::new(),
        }
    }

    /// 已生效的规则集（失败的规则以 `Block` 记录）。
    pub fn current(&self) -> &[RuleSpec] {
        &self.current
    }

    pub fn status(&self) -> Vec<RuleStatus> {
        self.status.clone()
    }

    pub fn retry(&mut self, desired: Vec<RuleSpec>) -> Vec<String> {
        self.apply_inner(desired, true)
    }

    pub fn interceptor(&mut self) -> &mut I {
        &mut self.interceptor
    }

    /// 直接访问 WFP 防护，供服务启动时核对残留的永久阻断。
    pub fn guard(&mut self) -> &mut G {
        &mut self.guard
    }

    /// 走到期望规则集，返回非致命错误（中文，可直接展示）。
    pub fn apply(&mut self, desired: Vec<RuleSpec>) -> Vec<String> {
        self.apply_inner(desired, false)
    }

    fn apply_inner(&mut self, desired: Vec<RuleSpec>, retry_resets: bool) -> Vec<String> {
        let old = std::mem::take(&mut self.current);
        self.pending_resets.retain(|k, _| desired.iter().any(|r| {
            r.reset_on_change && exe_key(&r.exe_path) == *k
                && old.iter().any(|o| o.id == r.id && o.exe_path == r.exe_path && o.action == r.action)
        }));
        let mut fail = Failures::default();

        // 上次没撤掉的放行先撤掉：它们不在 `old` 里，plan 不知道
        let stray: Vec<(String, String)> = self
            .installed
            .permits
            .iter()
            .filter(|(k, _)| {
                !old.iter()
                    .any(|r| permitted(r) && exe_key(&r.exe_path) == **k)
            })
            .map(|(k, e)| (k.clone(), e.clone()))
            .collect();
        for (k, exe) in stray {
            match self.guard.revoke(&exe) {
                Ok(()) => {
                    self.installed.permits.remove(&k);
                    self.unknown_keys.remove(&k);
                }
                Err(e) => {
                    fail.errors.push(format!("撤销 {exe} 的放行失败：{e}"));
                    self.fail(&mut fail, &old, &desired, None, Some(k));
                }
            }
        }

        let mut ops = plan(&old, &desired);
        if retry_resets {
            for r in &desired {
                if self.unknown_keys.contains(&exe_key(&r.exe_path))
                    && !ops.iter().any(|op| matches!(op, Op::Revoke { exe } if exe_key(exe) == exe_key(&r.exe_path))) {
                    ops.insert(0, Op::Revoke { exe: r.exe_path.clone() });
                }
            }
            // 失败时 current 按 Block 保守记录，不能把它当作执行成功的证明。
            for r in &desired {
                if r.action == Action::Block && self.status.iter().any(|s| s.id == r.id && s.action.is_none())
                    && !ops.iter().any(|op| matches!(op, Op::SetRule { rule, .. } if rule == &r.id)) {
                    ops.insert(0, Op::SetRule { rule: r.id.clone(), exe: r.exe_path.clone(), action: PbAction::Block });
                }
            }
            for r in &desired {
                if self.pending_resets.contains_key(&exe_key(&r.exe_path))
                    && !ops.iter().any(|op| matches!(op, Op::Reset { exe } if exe_key(exe) == exe_key(&r.exe_path))) {
                    ops.push(Op::Reset { exe: r.exe_path.clone() });
                }
            }
        }
        for op in ops {
            self.exec(&op, &mut fail, &old, &desired);
        }

        self.current = self.record(&old, &desired, &fail);

        // 失败的规则被删掉后可能留下没有规则对应的永久阻断，这里补删
        let stale: Vec<(String, String)> = self
            .installed
            .blocks
            .iter()
            .filter(|(k, _)| {
                // 本次失败的 exe 保持原状（宁可多挡），下次 apply 再清理
                !fail.keys.contains(*k)
                    && !self
                        .current
                        .iter()
                        .any(|r| guarded(r) && exe_key(&r.exe_path) == **k)
            })
            .map(|(k, e)| (k.clone(), e.clone()))
            .collect();
        for (k, exe) in stale {
            match self.guard.remove_block(&exe) {
                Ok(()) => {
                    self.installed.blocks.remove(&k);
                }
                Err(e) => fail.errors.push(format!("移除 {exe} 的永久阻断失败：{e}")),
            }
        }

        // 没有任何规则时停止拦截，不让全机流量白白经过用户态（WFP 阻断不依赖拦截器）。
        // 有规则删除失败时不停：那条规则仍记在 current 里，停掉会让它在下次 plan 认为"未变"时失效。
        if self.current.is_empty() && self.installed.rules.is_empty() {
            if let Err(e) = self.interceptor.suspend() {
                fail.errors.push(format!("停止 ProxyBridge 失败：{e}"));
            }
        }
        self.status = desired.iter().map(|r| {
            let k = exe_key(&r.exe_path);
            let failed = fail.ids.contains(&r.id) || fail.keys.contains(&k);
            let unknown = self.unknown_ids.contains(&r.id) || self.unknown_keys.contains(&k)
                || (!guarded(r) && self.installed.blocks.contains_key(&k));
            let (action, reason) = if unknown {
                (None, Some("应用失败，无法确认防护状态".into()))
            } else if failed {
                (Some("Block".into()), Some("应用失败，已阻断；请手动重试".into()))
            } else {
                let action = match r.action { Action::Proxy(_) => "Proxy", Action::Direct => "Direct", Action::Block => "Block" };
                (Some(action.into()), self.pending_resets.get(&k).cloned())
            };
            RuleStatus { id: r.id.clone(), action, reason }
        }).collect();
        for rule in &self.status {
            if rule.action.is_none() {
                fail.errors.push(format!("规则 {} 的执行结果未知，请手动重试", rule.id));
            }
        }
        // 普通配置同步不会重试断连，相关提示一直保留到用户主动重试或改变该规则。
        for warning in self.pending_resets.values() {
            if !fail.errors.contains(warning) { fail.errors.push(warning.clone()); }
        }
        fail.errors
    }

    fn exec(&mut self, op: &Op, fail: &mut Failures, old: &[RuleSpec], desired: &[RuleSpec]) {
        let (id, key) = match op {
            Op::SetProxy { rule, .. } | Op::RemoveRule { rule } => (Some(rule), None),
            Op::SetRule { rule, exe, .. } => (Some(rule), Some(exe_key(exe))),
            Op::EnsureBlock { exe }
            | Op::RemoveBlock { exe }
            | Op::Permit { exe }
            | Op::Revoke { exe }
            | Op::Reset { exe } => (None, Some(exe_key(exe))),
        };
        let tightening = matches!(op, Op::EnsureBlock { .. } | Op::Revoke { .. });
        let failed = id.is_some_and(|i| fail.ids.contains(i))
            || key.as_ref().is_some_and(|k| fail.keys.contains(k));
        if failed && !tightening {
            return;
        }
        let inst = &mut self.installed;
        let result = match op {
            Op::SetProxy { rule, target } => self.interceptor.set_proxy(rule, target),
            Op::SetRule { rule, exe, action } => {
                self.interceptor.set_rule(rule, exe, *action).map(|()| {
                    inst.rules.insert(rule.clone(), (exe.clone(), *action));
                    self.unknown_ids.remove(rule);
                })
            }
            Op::RemoveRule { rule } => self.interceptor.remove_rule(rule).map(|()| {
                inst.rules.remove(rule);
                self.unknown_ids.remove(rule);
            }),
            Op::EnsureBlock { exe } => self.guard.ensure_block(exe).map(|()| {
                inst.blocks.insert(exe_key(exe), exe.clone());
            }),
            Op::RemoveBlock { exe } => self.guard.remove_block(exe).map(|()| {
                inst.blocks.remove(&exe_key(exe));
            }),
            Op::Permit { exe } => self.guard.permit(exe).map(|()| {
                inst.permits.insert(exe_key(exe), exe.clone());
            }),
            Op::Revoke { exe } => self.guard.revoke(exe).map(|()| {
                inst.permits.remove(&exe_key(exe));
                self.unknown_keys.remove(&exe_key(exe));
            }),
            Op::Reset { exe } => self.killer.reset_ipv4_tcp(exe).map(|_| {
                self.pending_resets.remove(&exe_key(exe));
            }),
        };
        if let Err(e) = result {
            fail.errors.push(format!("{}失败：{e}", describe(op)));
            // 断开旧连接只是收尾，失败不影响分流与防护状态，仅报错
            if matches!(op, Op::Reset { .. }) {
                if let Op::Reset { exe } = op {
                    self.pending_resets.insert(exe_key(exe), format!("断开 {exe} 的旧连接失败：{e}"));
                }
                return;
            }
            self.fail(fail, old, desired, id.cloned(), key);
        }
    }

    /// 把 `id` / `key` 及与之相关的规则和 exe 标为失败，并强制阻断。
    fn fail(
        &mut self,
        fail: &mut Failures,
        old: &[RuleSpec],
        desired: &[RuleSpec],
        id: Option<String>,
        key: Option<String>,
    ) {
        let edges: Vec<(&str, String)> = old
            .iter()
            .chain(desired)
            .map(|r| (r.id.as_str(), exe_key(&r.exe_path)))
            .chain(
                self.installed
                    .rules
                    .iter()
                    .map(|(i, (e, _))| (i.as_str(), exe_key(e))),
            )
            .collect();
        let mut new_ids: Vec<String> = id.into_iter().filter(|i| !fail.ids.contains(i)).collect();
        let mut new_keys: Vec<String> =
            key.into_iter().filter(|k| !fail.keys.contains(k)).collect();
        fail.ids.extend(new_ids.iter().cloned());
        fail.keys.extend(new_keys.iter().cloned());
        loop {
            let mut grew = false;
            for (i, k) in &edges {
                let (has_i, has_k) = (fail.ids.contains(*i), fail.keys.contains(k));
                if has_i && !has_k {
                    fail.keys.insert(k.clone());
                    new_keys.push(k.clone());
                    grew = true;
                } else if has_k && !has_i {
                    fail.ids.insert(i.to_string());
                    new_ids.push(i.to_string());
                    grew = true;
                }
            }
            if !grew {
                break;
            }
        }

        // 先把 ProxyBridge 规则改成 Block：期望里有的按期望 exe（尚未下发的也补上），
        // 只在旧集合里的按已下发的 exe。换 exe 后旧 exe 也在失败范围内，放行会被撤掉。
        for i in new_ids {
            let exe = match desired.iter().find(|r| r.id == i) {
                Some(r) => r.exe_path.clone(),
                None => match self.installed.rules.get(&i) {
                    Some((exe, _)) => exe.clone(),
                    None => continue,
                },
            };
            if let Err(e) = self.interceptor.set_rule(&i, &exe, PbAction::Block) {
                self.unknown_ids.insert(i.clone());
                fail.errors
                    .push(format!("强制阻断规则 {i}（{exe}）失败：{e}"));
            } else {
                self.unknown_ids.remove(&i);
            }
            // 失败时状态未知，仍按 Block 记录
            self.installed.rules.insert(i, (exe, PbAction::Block));
        }
        // 再撤放行
        for k in new_keys {
            let exe = self.installed.permits.get(&k).cloned().or_else(|| {
                desired
                    .iter()
                    .chain(old)
                    .find(|r| exe_key(&r.exe_path) == k)
                    .map(|r| r.exe_path.clone())
            });
            let Some(exe) = exe else { continue };
            match self.guard.revoke(&exe) {
                Ok(()) => {
                    self.installed.permits.remove(&k);
                    self.unknown_keys.remove(&k);
                }
                Err(e) => {
                    self.unknown_keys.insert(k);
                    fail.errors.push(format!("撤销 {exe} 的放行失败：{e}"));
                }
            }
        }
    }

    /// 本次 apply 之后的实际规则集：成功的规则即期望；失败的规则按已下发状态记为 Block，
    /// `on_fault` 按永久阻断是否在位记录，使下次 plan 会补上缺的阻断。
    fn record(&self, old: &[RuleSpec], desired: &[RuleSpec], fail: &Failures) -> Vec<RuleSpec> {
        let mut out = Vec::new();
        let olds = old.iter().filter(|o| !desired.iter().any(|n| n.id == o.id));
        for r in desired.iter().chain(olds) {
            if !fail.ids.contains(&r.id) {
                if desired.iter().any(|n| n.id == r.id) {
                    out.push(r.clone());
                }
                continue;
            }
            if let Some((exe, _)) = self.installed.rules.get(&r.id) {
                out.push(RuleSpec {
                    id: r.id.clone(),
                    exe_path: exe.clone(),
                    action: Action::Block,
                    on_fault: if self.installed.blocks.contains_key(&exe_key(exe)) {
                        FaultPolicy::Block
                    } else {
                        FaultPolicy::Direct
                    },
                    reset_on_change: r.reset_on_change,
                });
            }
        }
        out
    }
}

fn describe(op: &Op) -> String {
    match op {
        Op::SetProxy { rule, .. } => format!("设置规则 {rule} 的代理"),
        Op::SetRule { rule, exe, action } => {
            let a = match action {
                PbAction::Proxy => "代理",
                PbAction::Direct => "直连",
                PbAction::Block => "阻断",
            };
            format!("将规则 {rule}（{exe}）设为{a}")
        }
        Op::RemoveRule { rule } => format!("删除规则 {rule}"),
        Op::EnsureBlock { exe } => format!("添加 {exe} 的永久阻断"),
        Op::RemoveBlock { exe } => format!("移除 {exe} 的永久阻断"),
        Op::Permit { exe } => format!("放行 {exe}"),
        Op::Revoke { exe } => format!("撤销 {exe} 的放行"),
        Op::Reset { exe } => format!("断开 {exe} 的旧连接"),
    }
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

    // ---------- Reconciler：用假实现驱动三个 trait ----------

    mod apply {
        use super::*;
        use crate::engine::{ConnKiller, Guard, Interceptor};
        use routa_procsvc_proto::Flow;
        use std::cell::RefCell;
        use std::rc::Rc;

        /// 三个假实现共享的"系统"：调用日志 + 已安装状态（exe 状态以 exe_key 为键）。
        #[derive(Debug, Default)]
        struct World {
            log: Vec<String>,
            calls: usize,
            /// 第 n 次（从 0 起）变更调用返回 Err。
            fail_at: Option<usize>,
            /// 这些方法每次都返回 Err。
            fail_ops: HashSet<&'static str>,
            rules: HashMap<String, (String, PbAction)>,
            proxies: HashMap<String, ProxyTarget>,
            blocks: HashMap<String, String>,
            permits: HashSet<String>,
            /// `suspend` 被调用的次数（不进日志，免得影响按日志断言的测试）。
            suspends: usize,
        }

        impl World {
            fn call(&mut self, name: &'static str, line: String) -> Result<(), String> {
                self.log.push(line);
                let n = self.calls;
                self.calls += 1;
                if self.fail_at == Some(n) || self.fail_ops.contains(name) {
                    Err(format!("{name} 注入失败"))
                } else {
                    Ok(())
                }
            }
        }

        type Shared = Rc<RefCell<World>>;
        struct FakeI(Shared);
        struct FakeG(Shared);
        struct FakeK(Shared);

        /// 与 `Op` 一一对应的日志行，假实现按同样格式记录调用。
        fn line(op: &Op) -> String {
            match op {
                Op::SetProxy { rule, target } => format!("set_proxy {rule} {}", target.port),
                Op::SetRule { rule, exe, action } => format!("set_rule {rule} {exe} {action:?}"),
                Op::RemoveRule { rule } => format!("remove_rule {rule}"),
                Op::EnsureBlock { exe } => format!("ensure_block {exe}"),
                Op::RemoveBlock { exe } => format!("remove_block {exe}"),
                Op::Permit { exe } => format!("permit {exe}"),
                Op::Revoke { exe } => format!("revoke {exe}"),
                Op::Reset { exe } => format!("reset {exe}"),
            }
        }

        impl Interceptor for FakeI {
            fn set_proxy(&mut self, rule: &str, target: &ProxyTarget) -> Result<(), String> {
                let mut w = self.0.borrow_mut();
                let op = Op::SetProxy {
                    rule: rule.into(),
                    target: target.clone(),
                };
                w.call("set_proxy", line(&op))?;
                w.proxies.insert(rule.into(), target.clone());
                Ok(())
            }
            fn set_rule(&mut self, rule: &str, exe: &str, action: PbAction) -> Result<(), String> {
                let mut w = self.0.borrow_mut();
                let op = Op::SetRule {
                    rule: rule.into(),
                    exe: exe.into(),
                    action,
                };
                w.call("set_rule", line(&op))?;
                w.rules.insert(rule.into(), (exe.into(), action));
                Ok(())
            }
            fn remove_rule(&mut self, rule: &str) -> Result<(), String> {
                let mut w = self.0.borrow_mut();
                w.call("remove_rule", line(&Op::RemoveRule { rule: rule.into() }))?;
                w.proxies.remove(rule);
                w.rules
                    .remove(rule)
                    .map(|_| ())
                    .ok_or_else(|| format!("规则 {rule} 不存在"))
            }
            fn flows(&mut self) -> Vec<Flow> {
                Vec::new()
            }
            fn suspend(&mut self) -> Result<(), String> {
                self.0.borrow_mut().suspends += 1;
                Ok(())
            }
        }

        impl Guard for FakeG {
            fn ensure_block(&mut self, exe: &str) -> Result<(), String> {
                let mut w = self.0.borrow_mut();
                w.call("ensure_block", line(&Op::EnsureBlock { exe: exe.into() }))?;
                w.blocks.entry(exe_key(exe)).or_insert_with(|| exe.into());
                Ok(())
            }
            fn remove_block(&mut self, exe: &str) -> Result<(), String> {
                let mut w = self.0.borrow_mut();
                w.call("remove_block", line(&Op::RemoveBlock { exe: exe.into() }))?;
                w.blocks.remove(&exe_key(exe));
                Ok(())
            }
            fn permit(&mut self, exe: &str) -> Result<(), String> {
                let mut w = self.0.borrow_mut();
                w.call("permit", line(&Op::Permit { exe: exe.into() }))?;
                w.permits.insert(exe_key(exe));
                Ok(())
            }
            fn revoke(&mut self, exe: &str) -> Result<(), String> {
                let mut w = self.0.borrow_mut();
                w.call("revoke", line(&Op::Revoke { exe: exe.into() }))?;
                w.permits.remove(&exe_key(exe));
                Ok(())
            }
            fn cleanup_all(&mut self) -> Result<(), String> {
                let mut w = self.0.borrow_mut();
                w.call("cleanup_all", "cleanup_all".into())?;
                w.blocks.clear();
                w.permits.clear();
                Ok(())
            }
            fn blocked_exes(&mut self) -> Result<Vec<String>, String> {
                Ok(self.0.borrow().blocks.values().cloned().collect())
            }
        }

        impl ConnKiller for FakeK {
            fn reset_ipv4_tcp(&mut self, exe: &str) -> Result<u32, String> {
                let mut w = self.0.borrow_mut();
                w.call("reset", line(&Op::Reset { exe: exe.into() }))?;
                Ok(0)
            }
        }

        type Rec = Reconciler<FakeI, FakeG, FakeK>;

        fn rig() -> (Rec, Shared) {
            let w = Shared::default();
            let rec = Reconciler::new(FakeI(w.clone()), FakeG(w.clone()), FakeK(w.clone()));
            (rec, w)
        }

        fn take_log(w: &Shared) -> Vec<String> {
            std::mem::take(&mut w.borrow_mut().log)
        }

        fn lines(ops: &[Op]) -> Vec<String> {
            ops.iter().map(line).collect()
        }

        fn proxy_guarded(id: &str, exe: &str) -> RuleSpec {
            spec(id, exe, Action::Proxy(target(1)), FaultPolicy::Block, true)
        }

        /// 假实现里的状态与 `rules` 完全对应（这正是 Reconciler 下次 plan 的前提）。
        fn assert_world_is(w: &Shared, rules: &[RuleSpec], ctx: &str) {
            let w = w.borrow();
            let want = Sim::built(rules);
            assert_eq!(w.rules, want.rules, "{ctx}");
            let blocks: HashSet<String> = w.blocks.keys().cloned().collect();
            assert_eq!(blocks, want.blocks, "{ctx}");
            assert_eq!(w.permits, want.permits, "{ctx}");
            for n in rules {
                if let Action::Proxy(t) = &n.action {
                    assert_eq!(w.proxies.get(&n.id), Some(t), "{ctx}");
                }
            }
        }

        #[test]
        fn apply_executes_plan_in_order() {
            let (mut rec, w) = rig();
            let v1 = vec![
                proxy_guarded("a", r"C:\a.exe"),
                spec("b", r"C:\b.exe", Action::Direct, FaultPolicy::Direct, true),
                spec("c", r"C:\c.exe", Action::Block, FaultPolicy::Block, false),
            ];
            assert!(rec.apply(v1.clone()).is_empty());
            assert_eq!(take_log(&w), lines(&plan(&[], &v1)));
            assert_eq!(rec.current(), &v1[..]);
            assert_world_is(&w, &v1, "v1");

            let v2 = vec![
                spec("a", r"C:\a.exe", Action::Block, FaultPolicy::Block, true),
                spec(
                    "b",
                    r"C:\b.exe",
                    Action::Proxy(target(2)),
                    FaultPolicy::Block,
                    true,
                ),
                spec("d", r"C:\c.exe", Action::Direct, FaultPolicy::Block, false),
            ];
            assert!(rec.apply(v2.clone()).is_empty());
            assert_eq!(take_log(&w), lines(&plan(&v1, &v2)));
            assert_eq!(rec.current(), &v2[..]);
            assert_world_is(&w, &v2, "v2");
        }

        #[test]
        fn failed_permit_leaves_rule_blocked() {
            let (mut rec, w) = rig();
            w.borrow_mut().fail_ops.insert("permit");
            let errors = rec.apply(vec![proxy_guarded("r", EXE)]);
            assert_eq!(errors.len(), 1, "{errors:?}");

            assert_eq!(rec.current().len(), 1);
            assert_eq!(rec.current()[0].action, Action::Block);
            let log = take_log(&w);
            let permit = log.iter().position(|l| l.starts_with("permit")).unwrap();
            let forced = format!("set_rule r {EXE} Block");
            assert!(log[permit..].contains(&forced), "{log:?}");
            // 放开类操作（Reset）不再执行
            assert!(!log.iter().any(|l| l.starts_with("reset")), "{log:?}");
            let world = w.borrow();
            assert_eq!(world.rules["r"], (EXE.to_string(), PbAction::Block));
            assert!(world.blocks.contains_key(&exe_key(EXE)));
            assert!(world.permits.is_empty());
        }

        #[test]
        fn failed_reset_is_error_only() {
            let (mut rec, w) = rig();
            w.borrow_mut().fail_ops.insert("reset");
            let rules = vec![proxy_guarded("r", EXE)];
            let errors = rec.apply(rules.clone());
            assert_eq!(errors.len(), 1, "{errors:?}");
            // 规则保持 Proxy，放行保留，没有强制阻断
            assert_eq!(rec.current(), &rules[..]);
            let world = w.borrow();
            assert_eq!(world.rules["r"].1, PbAction::Proxy);
            assert!(world.permits.contains(&exe_key(EXE)));
            assert!(!world.log.contains(&format!("set_rule r {EXE} Block")));
            assert!(!world.log.iter().any(|l| l.starts_with("revoke")));
        }

        #[test]
        fn status_distinguishes_confirmed_block_unknown_and_successful_rules() {
            let (mut rec, w) = rig();
            let rules = vec![proxy_guarded("a", EXE), spec("b", OTHER_EXE, Action::Direct, FaultPolicy::Direct, false)];
            w.borrow_mut().fail_ops.insert("set_proxy");
            assert!(!rec.apply(rules.clone()).is_empty());
            let status = rec.status();
            assert_eq!(status[0].action.as_deref(), Some("Block"));
            assert!(status[0].reason.as_ref().unwrap().contains("应用失败"));
            assert_eq!(status[1].action.as_deref(), Some("Direct"));
            w.borrow_mut().fail_ops.insert("set_rule");
            assert!(!rec.retry(rules.clone()).is_empty());
            assert_eq!(rec.status()[0].action, None);
            w.borrow_mut().fail_ops.clear();
            assert!(rec.retry(rules).is_empty());
            assert_eq!(rec.status()[0].action.as_deref(), Some("Proxy"));
            assert_eq!(rec.status()[0].reason, None);
        }

        #[test]
        fn unknown_block_stays_unknown_until_explicit_success() {
            let (mut rec, w) = rig();
            let rules = vec![r(Action::Block, FaultPolicy::Direct)];
            w.borrow_mut().fail_ops.insert("set_rule");
            assert!(!rec.apply(rules.clone()).is_empty());
            assert_eq!(rec.status()[0].action, None);
            w.borrow_mut().fail_ops.clear();
            rec.apply(rules.clone()); // no-op current must not become proof of success
            assert_eq!(rec.status()[0].action, None);
            assert!(rec.retry(rules).is_empty());
            assert_eq!(rec.status()[0].action.as_deref(), Some("Block"));
        }

        #[test]
        fn only_manual_retry_repeats_failed_reset() {
            let (mut rec, w) = rig();
            let rules = vec![proxy_guarded("r", EXE)];
            w.borrow_mut().fail_ops.insert("reset");
            assert_eq!(rec.apply(rules.clone()).len(), 1);
            assert_eq!(rec.status()[0].action.as_deref(), Some("Proxy"));
            assert!(rec.status()[0].reason.as_ref().unwrap().contains("断开"));
            take_log(&w);
            assert_eq!(rec.apply(rules.clone()).len(), 1);
            assert!(take_log(&w).is_empty());
            assert_eq!(rec.retry(rules.clone()).len(), 1); // still fails, exactly one attempt
            assert_eq!(take_log(&w), vec![format!("reset {EXE}")]);
            w.borrow_mut().fail_ops.clear();
            assert!(rec.retry(rules.clone()).is_empty());
            assert_eq!(take_log(&w), vec![format!("reset {EXE}")]);
            assert!(rec.retry(rules).is_empty());
            assert!(take_log(&w).is_empty());
        }

        #[test]
        fn failed_key_keeps_block_until_next_apply() {
            let (mut rec, w) = rig();
            assert!(rec.apply(vec![proxy_guarded("r", EXE)]).is_empty());
            take_log(&w);

            // RemoveBlock 失败：该 exe 属于失败范围，本轮收尾不再重试清理
            w.borrow_mut().fail_ops.insert("remove_block");
            assert_eq!(rec.apply(vec![]).len(), 1);
            assert!(rec.current().is_empty());
            assert!(w.borrow().blocks.contains_key(&exe_key(EXE)));
            let removes = take_log(&w)
                .iter()
                .filter(|l| l.starts_with("remove_block"))
                .count();
            assert_eq!(removes, 1);

            // 下一轮不再失败：残留的阻断被清掉
            w.borrow_mut().fail_ops.clear();
            assert!(rec.apply(vec![]).is_empty());
            assert_world_is(&w, &[], "cleaned");
        }

        #[test]
        fn reapply_same_state_is_noop() {
            let (mut rec, w) = rig();
            let rules = vec![
                proxy_guarded("a", r"C:\a.exe"),
                spec("b", r"C:\b.exe", Action::Block, FaultPolicy::Direct, true),
            ];
            assert!(rec.apply(rules.clone()).is_empty());
            take_log(&w);
            assert!(rec.apply(rules.clone()).is_empty());
            assert!(take_log(&w).is_empty());
            assert_eq!(rec.current(), &rules[..]);
        }

        #[test]
        fn reapply_after_failure_retries() {
            let (mut rec, w) = rig();
            let rules = vec![proxy_guarded("r", EXE)];
            w.borrow_mut().fail_ops.insert("permit");
            assert!(!rec.apply(rules.clone()).is_empty());
            assert_ne!(rec.current(), &rules[..]);
            take_log(&w);

            w.borrow_mut().fail_ops.clear();
            assert!(rec.apply(rules.clone()).is_empty());
            assert!(take_log(&w).contains(&format!("permit {EXE}")));
            assert_eq!(rec.current(), &rules[..]);
            assert_world_is(&w, &rules, "retry");

            assert!(rec.apply(rules.clone()).is_empty());
            assert!(take_log(&w).is_empty());
        }

        #[test]
        fn failure_does_not_stop_other_rules() {
            let (mut rec, w) = rig();
            let a = proxy_guarded("a", EXE);
            let b = proxy_guarded("b", OTHER_EXE);
            let ops = plan(&[], &[a.clone(), b.clone()]);
            let n = ops
                .iter()
                .position(|o| *o == Op::Permit { exe: EXE.into() })
                .unwrap();
            w.borrow_mut().fail_at = Some(n);
            let errors = rec.apply(vec![a.clone(), b.clone()]);
            assert_eq!(errors.len(), 1, "{errors:?}");
            let current = rec.current();
            assert_eq!(current[0].action, Action::Block);
            assert_eq!(current[1], b);
            let world = w.borrow();
            assert!(world.permits.contains(&exe_key(OTHER_EXE)));
            assert!(!world.permits.contains(&exe_key(EXE)));
            assert_eq!(world.rules["b"].1, PbAction::Proxy);
            assert!(world.log.contains(&format!("reset {OTHER_EXE}")));
            assert!(!world.log.contains(&format!("reset {EXE}")));
        }

        #[test]
        fn failed_revoke_on_removal_keeps_rule_blocked() {
            let (mut rec, w) = rig();
            assert!(rec.apply(vec![proxy_guarded("r", EXE)]).is_empty());
            take_log(&w);

            w.borrow_mut().fail_ops.insert("revoke");
            assert!(!rec.apply(vec![]).is_empty());
            {
                let world = w.borrow();
                // 放行撤不掉：ProxyBridge 规则不能删，改成 Block 兜住
                assert_eq!(world.rules["r"], (EXE.to_string(), PbAction::Block));
                assert!(
                    !world.log.iter().any(|l| l.starts_with("remove_")),
                    "{:?}",
                    world.log
                );
            }
            assert_eq!(rec.current().len(), 1);
            assert_eq!(rec.current()[0].action, Action::Block);

            w.borrow_mut().fail_ops.clear();
            take_log(&w);
            assert!(rec.apply(vec![]).is_empty());
            assert!(rec.current().is_empty());
            assert_world_is(&w, &[], "cleaned");
        }

        #[test]
        fn suspends_interceptor_only_when_no_rules_remain() {
            let (mut rec, w) = rig();
            let suspends = |w: &Shared| w.borrow().suspends;
            assert!(rec.apply(vec![proxy_guarded("r", EXE)]).is_empty());
            assert_eq!(suspends(&w), 0);

            // 删除规则失败：规则仍在（记为 Block），不能停
            w.borrow_mut().fail_ops.insert("remove_rule");
            assert!(!rec.apply(vec![]).is_empty());
            assert_eq!(rec.current().len(), 1);
            assert_eq!(suspends(&w), 0);

            w.borrow_mut().fail_ops.clear();
            assert!(rec.apply(vec![]).is_empty());
            assert!(rec.current().is_empty());
            assert_eq!(suspends(&w), 1);

            // 空集合再 apply 一次：照常调用（由拦截器自己保证幂等）
            assert!(rec.apply(vec![]).is_empty());
            assert_eq!(suspends(&w), 2);

            // 有规则时不调用
            assert!(rec.apply(vec![proxy_guarded("r", EXE)]).is_empty());
            assert_eq!(suspends(&w), 2);
        }

        #[test]
        fn failed_ensure_block_retries_next_apply() {
            let (mut rec, w) = rig();
            let rules = vec![proxy_guarded("r", EXE)];
            w.borrow_mut().fail_ops.insert("ensure_block");
            assert!(!rec.apply(rules.clone()).is_empty());
            {
                let world = w.borrow();
                // 没有永久阻断时绝不放行，规则落成 Block
                assert!(world.permits.is_empty());
                assert_eq!(world.rules["r"].1, PbAction::Block);
            }
            w.borrow_mut().fail_ops.clear();
            assert!(rec.apply(rules.clone()).is_empty());
            assert_eq!(rec.current(), &rules[..]);
            assert_world_is(&w, &rules, "retry");
        }

        /// 同 `assert_world_is`，但允许假实现里多出 `rules` 之外的永久阻断。
        fn assert_world_is_ignoring_extra_blocks(w: &Shared, rules: &[RuleSpec], ctx: &str) {
            let want = Sim::built(rules);
            let mut world = w.borrow_mut();
            let extra: Vec<String> = world
                .blocks
                .keys()
                .filter(|k| !want.blocks.contains(*k))
                .cloned()
                .collect();
            let saved: Vec<(String, String)> = extra
                .iter()
                .filter_map(|k| world.blocks.remove(k).map(|e| (k.clone(), e)))
                .collect();
            drop(world);
            assert_world_is(w, rules, ctx);
            w.borrow_mut().blocks.extend(saved);
        }

        /// 对 old → new 的每一个操作位置注入一次失败，检查：失败只会更紧、
        /// 记录的 current 与假实现状态一致、下次 apply 同样的 new 能恢复且之后为空操作。
        fn check_single_failures(old: &[RuleSpec], new: &[RuleSpec]) {
            for n in 0..plan(old, new).len() {
                let ctx = format!("{old:?} -> {new:?}, fail #{n}");
                let (mut rec, w) = rig();
                assert!(rec.apply(old.to_vec()).is_empty(), "{ctx}");
                {
                    let mut world = w.borrow_mut();
                    world.fail_at = Some(world.calls + n);
                }
                assert!(!rec.apply(new.to_vec()).is_empty(), "{ctx}");

                let current = rec.current().to_vec();
                let ctx = format!("{ctx}: recorded {current:?}");
                // 未完成的规则只会落成 Block
                for c in &current {
                    assert!(
                        c.action == Action::Block || new.contains(c),
                        "{ctx}: loosened {c:?}"
                    );
                }
                // 失败范围内的 exe 本轮可能多留一份永久阻断（只会更紧），下次 apply 清理
                assert_world_is_ignoring_extra_blocks(&w, &current, &ctx);
                // 期望受保护的 exe：要么已按期望就位，要么被阻断
                {
                    let world = w.borrow();
                    for k in guarded_keys(new) {
                        let mine: Vec<&RuleSpec> = current
                            .iter()
                            .filter(|c| exe_key(&c.exe_path) == k)
                            .collect();
                        let settled = mine.iter().all(|c| new.contains(c));
                        let pb_blocks = mine.iter().all(|c| c.action == Action::Block);
                        let wfp_blocks =
                            world.blocks.contains_key(&k) && !world.permits.contains(&k);
                        assert!(
                            !mine.is_empty() && (settled || pb_blocks || wfp_blocks),
                            "{ctx}: {k} not held"
                        );
                    }
                }

                w.borrow_mut().fail_at = None;
                assert_eq!(rec.retry(new.to_vec()), Vec::<String>::new(), "{ctx}");
                assert_eq!(rec.current(), new, "{ctx}");
                assert_world_is(&w, new, &ctx);
                take_log(&w);
                assert!(rec.apply(new.to_vec()).is_empty(), "{ctx}");
                assert!(take_log(&w).is_empty(), "{ctx}");
            }
        }

        /// 两条规则的所有集合（同一集合内 exe 不重复）。b 槽不含大小写变体以控制用时，
        /// 大小写变化已由 a 槽覆盖。
        fn two_rule_sets() -> Vec<Vec<RuleSpec>> {
            let slot = |id: &str, exes: &[&str]| {
                let mut v: Vec<Option<RuleSpec>> = vec![None];
                for exe in exes {
                    for a in [Action::Proxy(target(1)), Action::Direct, Action::Block] {
                        for f in [FaultPolicy::Direct, FaultPolicy::Block] {
                            v.push(Some(spec(id, exe, a.clone(), f, true)));
                        }
                    }
                }
                v
            };
            let mut sets = Vec::new();
            for a in slot("a", &[EXE, EXE_OTHER_CASE, OTHER_EXE]) {
                for b in slot("b", &[EXE, OTHER_EXE]) {
                    let set: Vec<RuleSpec> = a.iter().chain(b.iter()).cloned().collect();
                    if set.len() == 2 && exe_key(&set[0].exe_path) == exe_key(&set[1].exe_path) {
                        continue;
                    }
                    sets.push(set);
                }
            }
            sets
        }

        /// 按 old 的下标分片，让测试并行跑。
        fn single_failures_shard(shard: usize) {
            let sets = two_rule_sets();
            for old in sets.iter().skip(shard).step_by(4) {
                for new in &sets {
                    check_single_failures(old, new);
                }
            }
        }

        #[test]
        fn single_failure_fails_closed_and_recovers_0() {
            single_failures_shard(0);
        }

        #[test]
        fn single_failure_fails_closed_and_recovers_1() {
            single_failures_shard(1);
        }

        #[test]
        fn single_failure_fails_closed_and_recovers_2() {
            single_failures_shard(2);
        }

        #[test]
        fn single_failure_fails_closed_and_recovers_3() {
            single_failures_shard(3);
        }
    }
}

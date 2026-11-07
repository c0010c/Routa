//! Routa 不在时的回落策略（spec 5、6.3）。

use routa_procsvc_proto::{Action, DesiredState, FaultPolicy, RuleSpec};

/// Routa 为什么不在。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoutaGone {
    /// 收到 Goodbye 后正常退出。
    Graceful,
    /// 没收到 Goodbye，进程句柄失效。
    Crashed,
    /// 开机或服务重启后尚未有 Routa 连上。
    NeverConnected,
}

/// Routa 不在时每条规则应回落成的规则集：无论 `why`（正常退出、崩溃、尚未连上），
/// 都按每条规则的 `on_fault`；`why` 只由调用方用来记录中断原因。
/// 其余字段（含 `kind`、`guard_paths`）原样保留。结果里不会出现 `Action::Proxy`。
pub fn fallback(state: &DesiredState, _why: RoutaGone) -> Vec<RuleSpec> {
    state
        .rules
        .iter()
        .map(|r| RuleSpec {
            action: match r.on_fault {
                FaultPolicy::Block => Action::Block,
                FaultPolicy::Direct => Action::Direct,
            },
            ..r.clone()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use routa_procsvc_proto::{ProcessKind, ProxyKind, ProxyTarget};

    const GUARD: &str = r"C:\x\a.exe";

    fn rule(id: &str, action: Action, on_fault: FaultPolicy) -> RuleSpec {
        RuleSpec {
            id: id.into(),
            exe_path: format!(r"C:\app\{id}.exe"),
            kind: ProcessKind::Path,
            guard_paths: vec![],
            action,
            on_fault,
            reset_on_change: true,
        }
    }

    fn proxy() -> Action {
        Action::Proxy(ProxyTarget {
            kind: ProxyKind::Socks5,
            host: "127.0.0.1".into(),
            port: 7890,
            username: None,
            password: None,
        })
    }

    /// a：按进程名、Proxy、on_fault=Block（带守护路径）；b：Proxy、on_fault=Direct；
    /// c：Direct、on_fault=Block；d：Block、on_fault=Direct。
    fn state() -> DesiredState {
        DesiredState {
            rules: vec![
                RuleSpec {
                    exe_path: "a.exe".into(),
                    kind: ProcessKind::Name,
                    guard_paths: vec![GUARD.into()],
                    ..rule("a", proxy(), FaultPolicy::Block)
                },
                rule("b", proxy(), FaultPolicy::Direct),
                rule("c", Action::Direct, FaultPolicy::Block),
                rule("d", Action::Block, FaultPolicy::Direct),
            ],
            routa_pid: 1,
        }
    }

    fn actions(rules: &[RuleSpec]) -> Vec<Action> {
        rules.iter().map(|r| r.action.clone()).collect()
    }

    #[test]
    fn graceful_and_never_connected_follow_on_fault() {
        for why in [RoutaGone::Graceful, RoutaGone::NeverConnected, RoutaGone::Crashed] {
            let out = fallback(&state(), why);
            assert_eq!(
                actions(&out),
                vec![Action::Block, Action::Direct, Action::Block, Action::Direct],
                "{why:?}"
            );
            assert_eq!(out[0].guard_paths, vec![GUARD.to_string()], "{why:?}");
        }
    }

    #[test]
    fn other_fields_preserved() {
        let st = state();
        let out = fallback(&st, RoutaGone::Crashed);
        assert_eq!(out.len(), st.rules.len());
        for (o, r) in out.iter().zip(&st.rules) {
            assert_eq!(o.id, r.id);
            assert_eq!(o.exe_path, r.exe_path);
            assert_eq!(o.kind, r.kind);
            assert_eq!(o.guard_paths, r.guard_paths);
            assert_eq!(o.on_fault, r.on_fault);
            assert_eq!(o.reset_on_change, r.reset_on_change);
        }
    }
}

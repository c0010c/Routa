//! Routa 不在时的回落策略（spec 5、6.3）。

use routa_procsvc_proto::{Action, DesiredState, ExitPolicy, FaultPolicy, RuleSpec};

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

/// Routa 不在时每条规则应回落成的规则集：正常退出 / 尚未连上按 `on_routa_exit`，
/// 崩溃按每条规则的 `on_fault`。结果里不会出现 `Action::Proxy`。
pub fn fallback(state: &DesiredState, why: RoutaGone) -> Vec<RuleSpec> {
    state
        .rules
        .iter()
        .map(|r| {
            let block = match why {
                RoutaGone::Graceful | RoutaGone::NeverConnected => {
                    state.on_routa_exit == ExitPolicy::Block
                }
                RoutaGone::Crashed => r.on_fault == FaultPolicy::Block,
            };
            RuleSpec {
                action: if block { Action::Block } else { Action::Direct },
                ..r.clone()
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use routa_procsvc_proto::{ProxyKind, ProxyTarget};

    fn rule(id: &str, action: Action, on_fault: FaultPolicy) -> RuleSpec {
        RuleSpec {
            id: id.into(),
            exe_path: format!(r"C:\app\{id}.exe"),
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

    fn state(on_routa_exit: ExitPolicy) -> DesiredState {
        DesiredState {
            rules: vec![
                rule("a", proxy(), FaultPolicy::Block),
                rule("b", proxy(), FaultPolicy::Direct),
                rule("c", Action::Direct, FaultPolicy::Block),
                rule("d", Action::Block, FaultPolicy::Direct),
            ],
            on_routa_exit,
            routa_pid: 1,
        }
    }

    fn actions(rules: &[RuleSpec]) -> Vec<Action> {
        rules.iter().map(|r| r.action.clone()).collect()
    }

    #[test]
    fn graceful_uses_exit_policy() {
        let out = fallback(&state(ExitPolicy::Block), RoutaGone::Graceful);
        assert_eq!(actions(&out), vec![Action::Block; 4]);
        let out = fallback(&state(ExitPolicy::Direct), RoutaGone::Graceful);
        assert_eq!(actions(&out), vec![Action::Direct; 4]);
    }

    #[test]
    fn never_connected_uses_exit_policy() {
        let out = fallback(&state(ExitPolicy::Block), RoutaGone::NeverConnected);
        assert_eq!(actions(&out), vec![Action::Block; 4]);
        let out = fallback(&state(ExitPolicy::Direct), RoutaGone::NeverConnected);
        assert_eq!(actions(&out), vec![Action::Direct; 4]);
    }

    #[test]
    fn crash_uses_on_fault() {
        for exit in [ExitPolicy::Direct, ExitPolicy::Block] {
            let out = fallback(&state(exit), RoutaGone::Crashed);
            assert_eq!(
                actions(&out),
                vec![Action::Block, Action::Direct, Action::Block, Action::Direct]
            );
        }
    }

    #[test]
    fn other_fields_preserved() {
        let st = state(ExitPolicy::Direct);
        let out = fallback(&st, RoutaGone::Crashed);
        assert_eq!(out.len(), st.rules.len());
        for (o, r) in out.iter().zip(&st.rules) {
            assert_eq!(o.id, r.id);
            assert_eq!(o.exe_path, r.exe_path);
            assert_eq!(o.on_fault, r.on_fault);
            assert_eq!(o.reset_on_change, r.reset_on_change);
        }
    }
}

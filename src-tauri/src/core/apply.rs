//! Fail-closed apply pipeline: write config, hot-reload, verify listeners,
//! cut stale connections. Safety rule: better disconnected than leaking.

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::api::{ApiError, MihomoApi};
use super::ports::PortOwners;
use crate::config_gen::{empty_config, EffectivePlan, Generated};
use crate::model::ListenerId;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
pub struct FailedListener {
    pub id: ListenerId,
    pub name: String,
    pub port: u16,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
pub enum ApplyOutcome {
    Reloaded {
        cut: usize,
        /// Affected connections that could not be closed even after a retry.
        /// If the connection list itself was unavailable this is the number of
        /// affected listeners (a lower bound, at least 1). The caller must
        /// escalate (restart the core) when it is > 0.
        uncut: usize,
        failed_listeners: Vec<FailedListener>,
    },
    FailedKeptPrevious {
        error: String,
    },
    FailedClosed {
        error: String,
        /// Same meaning as in `Reloaded`.
        uncut: usize,
    },
    FailedCoreStopped {
        error: String,
    },
    /// No core is running: the model was saved but nothing was applied; it
    /// takes effect when mihomo starts. Never produced by `Applier`.
    NotRunning {
        error: String,
    },
}

/// Listener names that existed in `old` and whose traffic must not continue as
/// before: removed in `new`, or routed to another exit / a changed exit.
/// Port-only changes are not affected. Sorted by name.
pub fn affected_listeners(old: &EffectivePlan, new: &EffectivePlan) -> Vec<String> {
    old.listeners
        .iter()
        .filter(|(name, o)| match new.listeners.get(*name) {
            None => true,
            Some(n) => n.exit_name != o.exit_name || n.exit_fingerprint != o.exit_fingerprint,
        })
        .map(|(name, _)| name.clone())
        .collect()
}

const PORT_POLL_INTERVAL: Duration = Duration::from_millis(200);
const PORT_POLL_MAX: u32 = 10; // 2 s

/// Precondition: mihomo must have been started with `empty_config` before the
/// first `apply`, so `last_plan == None` really means "nothing is listening".
pub struct Applier {
    api: Arc<dyn MihomoApi>,
    ports: Arc<dyn PortOwners>,
    config_path: PathBuf,
    last_plan: Option<EffectivePlan>,
    /// Last config mihomo accepted (what it currently holds in memory).
    last_yaml: Option<String>,
}

fn write_atomic(path: &Path, content: &str) -> std::io::Result<()> {
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    std::fs::write(&tmp, content)?;
    std::fs::rename(&tmp, path)
}

impl Applier {
    pub fn new(api: Arc<dyn MihomoApi>, ports: Arc<dyn PortOwners>, config_path: PathBuf) -> Self {
        Self {
            api,
            ports,
            config_path,
            last_plan: None,
            last_yaml: None,
        }
    }

    /// `Err((rejected, message))`; `rejected` is true only when mihomo
    /// answered with a non-2xx status (so the old config is certainly intact).
    async fn write_and_reload(&self, yaml: &str) -> Result<(), (bool, String)> {
        write_atomic(&self.config_path, yaml)
            .map_err(|e| (false, format!("write config: {e}")))?;
        self.api
            .reload(&self.config_path)
            .await
            .map_err(|e: ApiError| (matches!(e, ApiError::Rejected { .. }), e.to_string()))
    }

    /// One list-and-close pass: `Ok((closed, failed))`, `Err` if the list failed.
    /// A 404 on close means the connection is already gone: counts as closed.
    async fn cut_once(&self, wanted: &HashSet<&str>) -> Result<(usize, usize), ()> {
        let snapshot = match self.api.connections().await {
            Ok(s) => s,
            Err(e) => {
                eprintln!("apply: list connections failed: {e}");
                return Err(());
            }
        };
        let (mut closed, mut failed) = (0, 0);
        for c in snapshot
            .connections
            .iter()
            .filter(|c| wanted.contains(c.metadata.inbound_name.as_str()))
        {
            match self.api.close_connection(&c.id).await {
                Ok(()) | Err(ApiError::Rejected { status: 404, .. }) => closed += 1,
                Err(e) => {
                    eprintln!("apply: close connection {} failed: {e}", c.id);
                    failed += 1;
                }
            }
        }
        Ok((closed, failed))
    }

    /// Closes every connection coming in through one of `names`; returns
    /// `(cut, uncut)`. Any failure retries the whole pass once after 200 ms.
    /// If the list is unavailable on the retry too, `uncut` is `names.len()`
    /// (a lower bound, at least 1).
    async fn cut(&self, names: &[String]) -> (usize, usize) {
        if names.is_empty() {
            return (0, 0);
        }
        let wanted: HashSet<&str> = names.iter().map(String::as_str).collect();
        let first = self.cut_once(&wanted).await;
        if let Ok((closed, 0)) = first {
            return (closed, 0);
        }
        let first_closed = first.map(|(c, _)| c).unwrap_or(0);
        tokio::time::sleep(PORT_POLL_INTERVAL).await;
        match self.cut_once(&wanted).await {
            Ok((closed, failed)) => (first_closed + closed, failed),
            Err(()) => (first_closed, names.len().max(1)),
        }
    }

    /// Polls port owners until every planned port belongs to `core_pid` (max 2 s).
    /// Never a TCP connect: the process squatting on a port accepts too.
    async fn failed_listeners(&self, plan: &EffectivePlan, core_pid: u32) -> Vec<FailedListener> {
        let ports: Vec<u16> = plan.listeners.values().map(|l| l.port).collect();
        if ports.is_empty() {
            return Vec::new();
        }
        let mut attempt = 0;
        loop {
            let owners = self.ports.listening_owners(&ports);
            let bad: Vec<FailedListener> = plan
                .listeners
                .iter()
                .filter(|(_, l)| owners.get(&l.port) != Some(&core_pid))
                .map(|(name, l)| FailedListener {
                    id: l.id.clone(),
                    name: name.clone(),
                    port: l.port,
                })
                .collect();
            if bad.is_empty() || attempt >= PORT_POLL_MAX {
                return bad;
            }
            attempt += 1;
            tokio::time::sleep(PORT_POLL_INTERVAL).await;
        }
    }

    pub async fn apply(
        &mut self,
        generated: Generated,
        controller_port: u16,
        secret: &str,
        core_pid: u32,
        stop_core: impl AsyncFnOnce(),
    ) -> ApplyOutcome {
        let old_plan = self.last_plan.clone().unwrap_or(EffectivePlan {
            listeners: BTreeMap::new(),
        });
        let affected = affected_listeners(&old_plan, &generated.plan);

        match self.write_and_reload(&generated.yaml).await {
            Ok(()) => {
                self.last_yaml = Some(generated.yaml.clone());
                self.last_plan = Some(generated.plan.clone());
                for (group, member) in &generated.selects {
                    if let Err(e) = self.api.select(group, member).await {
                        eprintln!("apply: restore select {group} -> {member} failed: {e}");
                    }
                }
                let (cut, uncut) = self.cut(&affected).await;
                let failed_listeners = self.failed_listeners(&generated.plan, core_pid).await;
                ApplyOutcome::Reloaded {
                    cut,
                    uncut,
                    failed_listeners,
                }
            }
            Err((rejected, error)) if !rejected || !affected.is_empty() => {
                // Either a tightening change was rejected (mihomo may keep
                // routing the old way), or the outcome is ambiguous (timeout,
                // transport error: mihomo may have applied the new config).
                // Load an empty config so nothing can leak.
                let empty = empty_config(controller_port, secret);
                match self.write_and_reload(&empty).await {
                    Ok(()) => {
                        self.last_yaml = Some(empty);
                        self.last_plan = Some(EffectivePlan {
                            listeners: BTreeMap::new(),
                        });
                        // Existing connections survive reloads (spike S5), and
                        // mihomo may have been running either plan: cut both.
                        let mut names: Vec<String> = old_plan
                            .listeners
                            .keys()
                            .chain(generated.plan.listeners.keys())
                            .cloned()
                            .collect();
                        names.sort();
                        names.dedup();
                        let (_, uncut) = self.cut(&names).await;
                        ApplyOutcome::FailedClosed { error, uncut }
                    }
                    Err((_, e2)) => {
                        stop_core().await;
                        ApplyOutcome::FailedCoreStopped {
                            error: format!("{error}; empty config also failed: {e2}"),
                        }
                    }
                }
            }
            Err((_, error)) => {
                // Rejected, non-tightening: mihomo still holds the previous config.
                if let Some(prev) = &self.last_yaml {
                    if let Err(e) = write_atomic(&self.config_path, prev) {
                        eprintln!("apply: restoring previous config failed: {e}");
                    }
                }
                ApplyOutcome::FailedKeptPrevious { error }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config_gen::PlannedListener;
    use crate::core::api::{Conn, ConnMeta, ConnectionsSnapshot};
    use crate::core::fake::FakeApi;
    use crate::core::ports::FakePortOwners;
    use std::cell::Cell;
    use std::collections::HashMap;
    use std::sync::Mutex;

    const PID: u32 = 4242;

    fn pl(name: &str, exit: &str, fp: u64, port: u16) -> (String, PlannedListener) {
        (
            name.to_string(),
            PlannedListener {
                id: ListenerId(format!("id-{name}")),
                exit_name: exit.into(),
                exit_fingerprint: fp,
                port,
            },
        )
    }

    fn plan(items: Vec<(String, PlannedListener)>) -> EffectivePlan {
        EffectivePlan {
            listeners: items.into_iter().collect(),
        }
    }

    fn generated(yaml: &str, plan: EffectivePlan) -> Generated {
        Generated {
            yaml: yaml.into(),
            plan,
            selects: vec![],
        }
    }

    fn owning(ports: &[u16]) -> Arc<FakePortOwners> {
        Arc::new(FakePortOwners(Mutex::new(
            ports.iter().map(|p| (*p, PID)).collect::<HashMap<_, _>>(),
        )))
    }

    fn conn(id: &str, inbound: &str) -> Conn {
        Conn {
            id: id.into(),
            upload: 0,
            download: 0,
            metadata: ConnMeta {
                inbound_name: inbound.into(),
            },
        }
    }

    fn setup(
        api: FakeApi,
        ports: Arc<FakePortOwners>,
    ) -> (Applier, Arc<FakeApi>, tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.yaml");
        let api = Arc::new(api);
        let a = Applier::new(api.clone(), ports, path.clone());
        (a, api, dir, path)
    }

    fn rejected() -> ApiError {
        ApiError::Rejected {
            status: 400,
            message: "bad".into(),
        }
    }

    #[test]
    fn affected_detects_removed_changed_and_fingerprint() {
        let old = plan(vec![
            pl("removed", "E1", 1, 7001),
            pl("moved-exit", "E1", 1, 7002),
            pl("fp", "E1", 1, 7003),
            pl("same", "E1", 1, 7004),
            pl("port-only", "E1", 1, 7005),
        ]);
        let new = plan(vec![
            pl("moved-exit", "E2", 1, 7002),
            pl("fp", "E1", 2, 7003),
            pl("same", "E1", 1, 7004),
            pl("port-only", "E1", 1, 7999),
            pl("brand-new", "E1", 1, 7006),
        ]);
        assert_eq!(
            affected_listeners(&old, &new),
            vec!["fp", "moved-exit", "removed"]
        );
    }

    #[tokio::test]
    async fn success_cuts_only_affected_connections() {
        let (mut a, api, _d, _p) = setup(FakeApi::new(), owning(&[7002]));
        a.last_plan = Some(plan(vec![pl("a", "E1", 1, 7001), pl("b", "E1", 1, 7002)]));
        api.set_connections(ConnectionsSnapshot {
            connections: vec![conn("c1", "a"), conn("c2", "a"), conn("c3", "b")],
            ..Default::default()
        });
        let out = a
            .apply(
                generated("new", plan(vec![pl("b", "E1", 1, 7002)])),
                9097,
                "s",
                PID,
                async || {},
            )
            .await;
        assert_eq!(
            out,
            ApplyOutcome::Reloaded {
                cut: 2,
                uncut: 0,
                failed_listeners: vec![]
            }
        );
        assert_eq!(*api.closed.lock().unwrap(), vec!["c1", "c2"]);
    }

    #[tokio::test]
    async fn restrictive_failure_loads_empty_config() {
        let api = FakeApi::with_reload_results(vec![Err(rejected()), Ok(())]);
        let (mut a, api, _d, path) = setup(api, owning(&[]));
        a.last_plan = Some(plan(vec![pl("a", "E1", 1, 7001)]));
        api.set_connections(ConnectionsSnapshot {
            connections: vec![conn("c1", "a")],
            ..Default::default()
        });
        let out = a
            .apply(
                generated("NEW", plan(vec![pl("a", "E2", 1, 7001)])),
                9097,
                "s",
                PID,
                async || {},
            )
            .await;
        assert!(matches!(out, ApplyOutcome::FailedClosed { .. }), "{out:?}");
        let reloads = api.reloads.lock().unwrap();
        assert_eq!(reloads.len(), 2);
        assert_eq!(reloads[0].1, "NEW");
        assert_eq!(reloads[1].1, empty_config(9097, "s"));
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            empty_config(9097, "s")
        );
        assert_eq!(a.last_plan, Some(plan(vec![])));
        assert_eq!(*api.closed.lock().unwrap(), vec!["c1"]);
    }

    #[tokio::test]
    async fn restrictive_double_failure_stops_core() {
        let api = FakeApi::with_reload_results(vec![Err(rejected()), Err(rejected())]);
        let (mut a, _api, _d, path) = setup(api, owning(&[]));
        a.last_plan = Some(plan(vec![pl("a", "E1", 1, 7001)]));
        let stopped = Cell::new(false);
        let out = a
            .apply(
                generated("NEW", plan(vec![])),
                9097,
                "s",
                PID,
                async || stopped.set(true),
            )
            .await;
        assert!(
            matches!(out, ApplyOutcome::FailedCoreStopped { .. }),
            "{out:?}"
        );
        assert!(stopped.get());
        assert_eq!(a.last_plan, Some(plan(vec![pl("a", "E1", 1, 7001)])));
        assert_eq!(a.last_yaml, None);
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            empty_config(9097, "s")
        );
    }

    #[tokio::test]
    async fn non_restrictive_failure_keeps_previous() {
        let api = FakeApi::with_reload_results(vec![Ok(()), Err(rejected())]);
        let (mut a, api, _d, path) = setup(api, owning(&[7001, 7002]));
        let p1 = plan(vec![pl("a", "E1", 1, 7001)]);
        a.apply(generated("OLD", p1.clone()), 9097, "s", PID, async || {})
            .await;
        let p2 = plan(vec![pl("a", "E1", 1, 7001), pl("b", "E1", 1, 7002)]);
        let stopped = Cell::new(false);
        let out = a
            .apply(generated("NEW", p2), 9097, "s", PID, async || {
                stopped.set(true)
            })
            .await;
        assert!(
            matches!(out, ApplyOutcome::FailedKeptPrevious { .. }),
            "{out:?}"
        );
        assert!(!stopped.get());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "OLD");
        assert_eq!(api.reloads.lock().unwrap().len(), 2);
        assert_eq!(a.last_plan, Some(p1));
    }

    #[tokio::test(start_paused = true)]
    async fn occupied_port_reported_as_failed_listener() {
        let ports = Arc::new(FakePortOwners(Mutex::new(HashMap::from([
            (7891, PID),
            (7892, 999),
        ]))));
        let (mut a, _api, _d, _p) = setup(FakeApi::new(), ports);
        let out = a
            .apply(
                generated(
                    "Y",
                    plan(vec![pl("ok", "E1", 1, 7891), pl("busy", "E1", 1, 7892)]),
                ),
                9097,
                "s",
                PID,
                async || {},
            )
            .await;
        assert_eq!(
            out,
            ApplyOutcome::Reloaded {
                cut: 0,
                uncut: 0,
                failed_listeners: vec![FailedListener {
                    id: ListenerId("id-busy".into()),
                    name: "busy".into(),
                    port: 7892
                }]
            }
        );
    }

    #[tokio::test(start_paused = true)]
    async fn port_that_comes_up_late_is_not_failed() {
        let ports = owning(&[]);
        let (mut a, _api, _d, _p) = setup(FakeApi::new(), ports.clone());
        let late = ports.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(500)).await;
            late.0.lock().unwrap().insert(7891, PID);
        });
        let out = a
            .apply(
                generated("Y", plan(vec![pl("a", "E1", 1, 7891)])),
                9097,
                "s",
                PID,
                async || {},
            )
            .await;
        assert_eq!(
            out,
            ApplyOutcome::Reloaded {
                cut: 0,
                uncut: 0,
                failed_listeners: vec![]
            }
        );
    }

    #[tokio::test]
    async fn restores_select_choice_after_reload() {
        let (mut a, api, _d, _p) = setup(FakeApi::new(), owning(&[]));
        let mut g = generated("Y", plan(vec![]));
        g.selects = vec![("grp".into(), "grp/m1".into())];
        a.apply(g, 9097, "s", PID, async || {}).await;
        assert_eq!(
            *api.selects.lock().unwrap(),
            vec![("grp".to_string(), "grp/m1".to_string())]
        );
    }

    #[tokio::test(start_paused = true)]
    async fn close_and_list_errors_do_not_fail_apply() {
        let (mut a, api, _d, _p) = setup(FakeApi::new(), owning(&[]));
        a.last_plan = Some(plan(vec![pl("a", "E1", 1, 7001)]));
        api.set_connections(ConnectionsSnapshot {
            connections: vec![conn("c1", "a"), conn("c2", "a")],
            ..Default::default()
        });
        api.close_fails.lock().unwrap().insert("c1".into());
        let out = a
            .apply(generated("Y", plan(vec![])), 9097, "s", PID, async || {})
            .await;
        assert_eq!(
            out,
            ApplyOutcome::Reloaded {
                cut: 1,
                uncut: 1,
                failed_listeners: vec![]
            }
        );
        *api.connections_error.lock().unwrap() = Some(ApiError::Unreachable("x".into()));
        a.last_plan = Some(plan(vec![pl("a", "E1", 1, 7001)]));
        let out = a
            .apply(generated("Z", plan(vec![])), 9097, "s", PID, async || {})
            .await;
        assert_eq!(
            out,
            ApplyOutcome::Reloaded {
                cut: 0,
                uncut: 1,
                failed_listeners: vec![]
            }
        );
    }

    fn unreachable() -> ApiError {
        ApiError::Unreachable("timeout".into())
    }

    #[tokio::test(start_paused = true)]
    async fn unreachable_reload_on_non_restrictive_change_fails_closed() {
        let api = FakeApi::with_reload_results(vec![Err(unreachable()), Ok(())]);
        let (mut a, api, _d, path) = setup(api, owning(&[]));
        a.last_plan = Some(plan(vec![pl("a", "E1", 1, 7001)]));
        a.last_yaml = Some("OLD".into());
        api.set_connections(ConnectionsSnapshot {
            connections: vec![conn("c1", "a"), conn("c2", "b"), conn("c3", "z")],
            ..Default::default()
        });
        // Adds listener b (non-restrictive), keeps a.
        let out = a
            .apply(
                generated(
                    "NEW",
                    plan(vec![pl("a", "E1", 1, 7001), pl("b", "E1", 1, 7002)]),
                ),
                9097,
                "s",
                PID,
                async || {},
            )
            .await;
        assert!(
            matches!(out, ApplyOutcome::FailedClosed { uncut: 0, .. }),
            "{out:?}"
        );
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            empty_config(9097, "s")
        );
        assert_eq!(a.last_plan, Some(plan(vec![])));
        // Union of old and new listener names is cut.
        assert_eq!(*api.closed.lock().unwrap(), vec!["c1", "c2"]);
    }

    #[tokio::test]
    async fn unreachable_then_unreachable_stops_core() {
        let api = FakeApi::with_reload_results(vec![Err(unreachable()), Err(unreachable())]);
        let (mut a, _api, _d, _p) = setup(api, owning(&[]));
        let stopped = Cell::new(false);
        let out = a
            .apply(
                generated("NEW", plan(vec![pl("a", "E1", 1, 7001)])),
                9097,
                "s",
                PID,
                async || stopped.set(true),
            )
            .await;
        assert!(
            matches!(out, ApplyOutcome::FailedCoreStopped { .. }),
            "{out:?}"
        );
        assert!(stopped.get());
    }

    #[tokio::test(start_paused = true)]
    async fn connections_failure_reported_as_uncut() {
        let (mut a, api, _d, _p) = setup(FakeApi::new(), owning(&[]));
        a.last_plan = Some(plan(vec![pl("a", "E1", 1, 7001), pl("b", "E1", 1, 7002)]));
        *api.connections_error.lock().unwrap() = Some(unreachable());
        let out = a
            .apply(generated("Y", plan(vec![])), 9097, "s", PID, async || {})
            .await;
        assert_eq!(
            out,
            ApplyOutcome::Reloaded {
                cut: 0,
                uncut: 2,
                failed_listeners: vec![]
            }
        );
    }

    #[tokio::test(start_paused = true)]
    async fn close_404_counts_as_cut() {
        let (mut a, api, _d, _p) = setup(FakeApi::new(), owning(&[]));
        a.last_plan = Some(plan(vec![pl("a", "E1", 1, 7001)]));
        api.set_connections(ConnectionsSnapshot {
            connections: vec![conn("c1", "a")],
            ..Default::default()
        });
        api.close_gone.lock().unwrap().insert("c1".into());
        let out = a
            .apply(generated("Y", plan(vec![])), 9097, "s", PID, async || {})
            .await;
        assert_eq!(
            out,
            ApplyOutcome::Reloaded {
                cut: 1,
                uncut: 0,
                failed_listeners: vec![]
            }
        );
    }

    #[tokio::test(start_paused = true)]
    async fn failed_closed_reports_uncut_when_connections_fail() {
        let api = FakeApi::with_reload_results(vec![Err(rejected()), Ok(())]);
        let (mut a, api, _d, _p) = setup(api, owning(&[]));
        a.last_plan = Some(plan(vec![pl("a", "E1", 1, 7001)]));
        *api.connections_error.lock().unwrap() = Some(unreachable());
        let out = a
            .apply(
                generated("NEW", plan(vec![pl("a", "E2", 1, 7001)])),
                9097,
                "s",
                PID,
                async || {},
            )
            .await;
        assert!(
            matches!(out, ApplyOutcome::FailedClosed { uncut: 1, .. }),
            "{out:?}"
        );
    }
}

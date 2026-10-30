//! Fail-closed apply pipeline: write config, hot-reload, verify listeners,
//! cut stale connections. Safety rule: better disconnected than leaking.

use std::collections::{BTreeMap, BTreeSet, HashSet};
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
        /// affected listeners (a lower bound, at least 1). Summed over both
        /// cut passes, so only "> 0" is meaningful: the caller must escalate
        /// (restart the core) then.
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
/// before: removed in `new`, routed to another exit / a changed exit, or with
/// changed own settings (type, listen, port, users). Sorted by name.
pub fn affected_listeners(old: &EffectivePlan, new: &EffectivePlan) -> Vec<String> {
    old.listeners
        .iter()
        .filter(|(name, o)| match new.listeners.get(*name) {
            None => true,
            Some(n) => {
                n.exit_name != o.exit_name
                    || n.exit_fingerprint != o.exit_fingerprint
                    || n.listener_fingerprint != o.listener_fingerprint
            }
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

    /// Ids of the listeners in the config mihomo currently holds.
    pub fn effective_listeners(&self) -> Vec<ListenerId> {
        self.last_plan
            .iter()
            .flat_map(|p| p.listeners.values().map(|l| l.id.clone()))
            .collect()
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

    /// Polls port owners until none of `ports` belongs to `core_pid` (max 2 s);
    /// returns the ports still held at the end.
    async fn ports_still_held(&self, ports: &[u16], core_pid: u32) -> Vec<u16> {
        if ports.is_empty() {
            return Vec::new();
        }
        let mut attempt = 0;
        loop {
            let owners = self.ports.listening_owners(ports);
            let held: Vec<u16> = ports
                .iter()
                .copied()
                .filter(|p| owners.get(p) == Some(&core_pid))
                .collect();
            if held.is_empty() || attempt >= PORT_POLL_MAX {
                return held;
            }
            attempt += 1;
            tokio::time::sleep(PORT_POLL_INTERVAL).await;
        }
    }

    /// Loads an empty config so nothing can leak, then cuts every connection
    /// of `old` and `new` (mihomo may have been running either plan, and
    /// existing connections survive reloads, spike S5). Stops the core if
    /// even the empty config fails.
    async fn fail_closed(
        &mut self,
        error: String,
        old: &EffectivePlan,
        new: &EffectivePlan,
        controller_port: u16,
        secret: &str,
        stop_core: impl AsyncFnOnce(),
    ) -> ApplyOutcome {
        let empty = empty_config(controller_port, secret);
        match self.write_and_reload(&empty).await {
            Ok(()) => {
                self.last_yaml = Some(empty);
                self.last_plan = Some(EffectivePlan {
                    listeners: BTreeMap::new(),
                });
                let names: Vec<String> = old
                    .listeners
                    .keys()
                    .chain(new.listeners.keys())
                    .cloned()
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect();
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
                for (group, member) in &generated.selects {
                    if let Err(e) = self.api.select(group, member).await {
                        // The new config is already loaded; keeping the previous plan is
                        // not a valid rollback. Reuse the existing fail-closed path.
                        let error = format!("切换节点失败：策略组「{group}」无法切换到「{member}」：{e}");
                        return self
                            .fail_closed(error, &old_plan, &generated.plan, controller_port, secret, stop_core)
                            .await;
                    }
                }
                self.last_yaml = Some(generated.yaml.clone());
                self.last_plan = Some(generated.plan.clone());
                // A connection accepted on a port mihomo still holds would
                // keep the old route: cut only once those ports are gone.
                let new_ports: HashSet<u16> = generated.plan.listeners.values().map(|l| l.port).collect();
                let released: Vec<u16> = old_plan
                    .listeners
                    .values()
                    .map(|l| l.port)
                    .filter(|p| !new_ports.contains(p))
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect();
                let still_held = self.ports_still_held(&released, core_pid).await;
                if !still_held.is_empty() {
                    let list: Vec<String> = still_held.iter().map(u16::to_string).collect();
                    let error = format!("旧端口 {} 未能释放", list.join(", "));
                    return self
                        .fail_closed(error, &old_plan, &generated.plan, controller_port, secret, stop_core)
                        .await;
                }
                let first_cut_at = tokio::time::Instant::now();
                let (mut cut, mut uncut) = self.cut(&affected).await;
                let failed_listeners = self.failed_listeners(&generated.plan, core_pid).await;
                // Affected listeners whose port mihomo keeps (changed in place,
                // or the port now belongs to another listener) cannot be
                // watched via port owners: cut them once more a little later.
                let same_port: Vec<String> = affected
                    .iter()
                    .filter(|n| new_ports.contains(&old_plan.listeners[*n].port))
                    .cloned()
                    .collect();
                if !same_port.is_empty() {
                    tokio::time::sleep_until(first_cut_at + PORT_POLL_INTERVAL).await;
                    let (c2, u2) = self.cut(&same_port).await;
                    cut += c2;
                    uncut += u2;
                }
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
                self.fail_closed(error, &old_plan, &generated.plan, controller_port, secret, stop_core)
                    .await
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
                // Stands in for type/listen/port/users: the port is enough here.
                listener_fingerprint: u64::from(port),
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
            chains: vec![],
            upload: 0,
            download: 0,
            metadata: ConnMeta {
                inbound_name: inbound.into(),
                ..Default::default()
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

    fn with_listener_fp(mut p: (String, PlannedListener), lfp: u64) -> (String, PlannedListener) {
        p.1.listener_fingerprint = lfp;
        p
    }

    #[test]
    fn affected_detects_removed_changed_and_fingerprint() {
        let old = plan(vec![
            pl("removed", "E1", 1, 7001),
            pl("moved-exit", "E1", 1, 7002),
            pl("fp", "E1", 1, 7003),
            pl("same", "E1", 1, 7004),
            pl("port-only", "E1", 1, 7005),
            pl("auth", "E1", 1, 7007),
        ]);
        let new = plan(vec![
            pl("moved-exit", "E2", 1, 7002),
            pl("fp", "E1", 2, 7003),
            pl("same", "E1", 1, 7004),
            pl("port-only", "E1", 1, 7999),
            with_listener_fp(pl("auth", "E1", 1, 7007), 42),
            pl("brand-new", "E1", 1, 7006),
        ]);
        assert_eq!(
            affected_listeners(&old, &new),
            vec!["auth", "fp", "moved-exit", "port-only", "removed"]
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
    async fn rejected_listener_settings_change_fails_closed() {
        let api = FakeApi::with_reload_results(vec![Err(rejected()), Ok(())]);
        let (mut a, api, _d, _p) = setup(api, owning(&[]));
        a.last_plan = Some(plan(vec![pl("a", "E1", 1, 7001)]));
        api.set_connections(ConnectionsSnapshot {
            connections: vec![conn("c1", "a")],
            ..Default::default()
        });
        let new = plan(vec![with_listener_fp(pl("a", "E1", 1, 7001), 99)]);
        let out = a.apply(generated("NEW", new), 9097, "s", PID, async || {}).await;
        assert!(matches!(out, ApplyOutcome::FailedClosed { .. }), "{out:?}");
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
        let out = a.apply(g, 9097, "s", PID, async || {}).await;
        assert!(matches!(out, ApplyOutcome::Reloaded { .. }), "{out:?}");
        assert_eq!(
            *api.selects.lock().unwrap(),
            vec![("grp".to_string(), "grp/m1".to_string())]
        );
    }

    #[tokio::test(start_paused = true)]
    async fn select_failure_closes_old_and_new_listeners_after_partial_success() {
        let (mut a, api, _d, path) = setup(FakeApi::new(), owning(&[]));
        a.last_plan = Some(plan(vec![pl("old", "E1", 1, 7001)]));
        a.last_yaml = Some("OLD".into());
        *api.select_results.lock().unwrap() = vec![Ok(()), Err(rejected())].into();
        api.set_connections(ConnectionsSnapshot {
            connections: vec![conn("old-conn", "old"), conn("new-conn", "new"), conn("unrelated", "other")],
            ..Default::default()
        });
        let mut g = generated("NEW", plan(vec![pl("new", "E2", 2, 7002)]));
        g.selects = vec![
            ("first".into(), "first/m1".into()),
            ("second".into(), "second/m2".into()),
            ("third".into(), "third/m3".into()),
        ];
        let stopped = Cell::new(false);
        let out = a.apply(g, 9097, "s", PID, async || stopped.set(true)).await;
        match out {
            ApplyOutcome::FailedClosed { error, uncut: 0 } => {
                assert!(error.contains("切换节点失败") && error.contains("second") && error.contains("second/m2"), "{error}");
            }
            other => panic!("expected FailedClosed, got {other:?}"),
        }
        assert!(!stopped.get());
        assert_eq!(*api.selects.lock().unwrap(), vec![("first".into(), "first/m1".into()), ("second".into(), "second/m2".into())]);
        let expected = empty_config(9097, "s");
        let reloads = api.reloads.lock().unwrap();
        assert_eq!(reloads.len(), 2);
        assert_eq!(reloads[0].1, "NEW");
        assert_eq!(reloads[1].1, expected);
        assert_eq!(std::fs::read_to_string(path).unwrap(), expected);
        assert_eq!(a.last_yaml.as_deref(), Some(expected.as_str()));
        assert!(a.effective_listeners().is_empty());
        let mut closed = api.closed.lock().unwrap().clone();
        closed.sort();
        assert_eq!(closed, ["new-conn", "old-conn"]);
    }

    #[tokio::test]
    async fn select_timeout_fails_closed_even_on_initial_apply() {
        let (mut a, api, _d, _p) = setup(FakeApi::new(), owning(&[]));
        api.select_results.lock().unwrap().push_back(Err(ApiError::Unreachable("timeout".into())));
        let mut g = generated("NEW", plan(vec![]));
        g.selects = vec![("grp".into(), "grp/m1".into())];
        let out = a.apply(g, 9097, "s", PID, async || panic!("empty config succeeds")).await;
        match out {
            ApplyOutcome::FailedClosed { error, uncut: 0 } => assert!(error.contains("grp/m1") && error.contains("timeout"), "{error}"),
            other => panic!("expected FailedClosed, got {other:?}"),
        }
        assert_eq!(api.selects.lock().unwrap().len(), 1); // no automatic retry
        assert_eq!(api.reloads.lock().unwrap().len(), 2);
        assert!(a.effective_listeners().is_empty());
    }

    #[tokio::test]
    async fn select_failure_stops_core_if_empty_config_also_fails() {
        let (mut a, api, _d, _p) = setup(FakeApi::with_reload_results(vec![Ok(()), Err(rejected())]), owning(&[]));
        api.select_results.lock().unwrap().push_back(Err(rejected()));
        let mut g = generated("NEW", plan(vec![pl("new", "E1", 1, 7001)]));
        g.selects = vec![("grp".into(), "grp/m1".into())];
        let stopped = Cell::new(false);
        let out = a.apply(g, 9097, "s", PID, async || stopped.set(true)).await;
        match out {
            ApplyOutcome::FailedCoreStopped { error } => assert!(error.contains("grp/m1") && error.contains("empty config also failed"), "{error}"),
            other => panic!("expected FailedCoreStopped, got {other:?}"),
        }
        assert!(stopped.get());
        assert_eq!(api.selects.lock().unwrap().len(), 1);
        assert_eq!(api.reloads.lock().unwrap().len(), 2);
        assert!(a.effective_listeners().is_empty()); // the failed new plan was not committed
    }

    #[tokio::test(start_paused = true)]
    async fn select_failure_reports_connections_that_could_not_be_closed() {
        let (mut a, api, _d, _p) = setup(FakeApi::new(), owning(&[]));
        a.last_plan = Some(plan(vec![pl("old", "E1", 1, 7001)]));
        api.select_results.lock().unwrap().push_back(Err(rejected()));
        *api.connections_error.lock().unwrap() = Some(ApiError::Unreachable("offline".into()));
        let mut g = generated("NEW", plan(vec![pl("new", "E2", 2, 7002)]));
        g.selects = vec![("grp".into(), "grp/m1".into())];
        let out = a.apply(g, 9097, "s", PID, async || {}).await;
        assert!(matches!(out, ApplyOutcome::FailedClosed { uncut: 2, .. }), "{out:?}");
        assert!(a.effective_listeners().is_empty());
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

    #[tokio::test(start_paused = true)]
    async fn cuts_removed_listener_only_after_its_port_is_released() {
        let ports = owning(&[7001, 7002]);
        let (mut a, api, _d, _p) = setup(FakeApi::new(), ports.clone());
        a.last_plan = Some(plan(vec![pl("a", "E1", 1, 7001), pl("b", "E1", 1, 7002)]));
        api.set_connections(ConnectionsSnapshot {
            connections: vec![conn("c1", "a")],
            ..Default::default()
        });
        let (late_ports, late_api) = (ports.clone(), api.clone());
        tokio::spawn(async move {
            // Accepted by the old listener after the reload returned.
            tokio::time::sleep(Duration::from_millis(300)).await;
            late_api.connections.lock().unwrap().connections.push(conn("late", "a"));
            tokio::time::sleep(Duration::from_millis(200)).await;
            late_ports.0.lock().unwrap().remove(&7001);
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
        assert_eq!(*api.closed.lock().unwrap(), vec!["c1", "late"]);
    }

    #[tokio::test(start_paused = true)]
    async fn moved_listener_waits_for_old_port() {
        let ports = owning(&[7001, 7101]);
        let (mut a, api, _d, _p) = setup(FakeApi::new(), ports.clone());
        a.last_plan = Some(plan(vec![pl("a", "E1", 1, 7001)]));
        let late = ports.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(400)).await;
            late.0.lock().unwrap().remove(&7001);
        });
        let started = tokio::time::Instant::now();
        let out = a
            .apply(
                generated("new", plan(vec![pl("a", "E1", 1, 7101)])),
                9097,
                "s",
                PID,
                async || {},
            )
            .await;
        assert!(matches!(out, ApplyOutcome::Reloaded { uncut: 0, .. }), "{out:?}");
        assert!(started.elapsed() >= Duration::from_millis(400));
        assert_eq!(api.reloads.lock().unwrap().len(), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn old_port_never_released_fails_closed() {
        let (mut a, api, _d, path) = setup(FakeApi::new(), owning(&[7001, 7002]));
        a.last_plan = Some(plan(vec![pl("a", "E1", 1, 7001), pl("b", "E1", 1, 7002)]));
        api.set_connections(ConnectionsSnapshot {
            connections: vec![conn("c1", "a"), conn("c2", "b"), conn("c3", "z")],
            ..Default::default()
        });
        let out = a
            .apply(
                generated("NEW", plan(vec![pl("b", "E1", 1, 7002)])),
                9097,
                "s",
                PID,
                async || {},
            )
            .await;
        match &out {
            ApplyOutcome::FailedClosed { error, uncut: 0 } => assert!(error.contains("7001"), "{error}"),
            _ => panic!("{out:?}"),
        }
        let reloads = api.reloads.lock().unwrap();
        assert_eq!(reloads.len(), 2);
        assert_eq!(reloads[1].1, empty_config(9097, "s"));
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            empty_config(9097, "s")
        );
        assert_eq!(a.last_plan, Some(plan(vec![])));
        assert!(a.effective_listeners().is_empty());
        // Old ∪ new is cut.
        assert_eq!(*api.closed.lock().unwrap(), vec!["c1", "c2"]);
    }

    #[tokio::test(start_paused = true)]
    async fn in_place_change_is_cut_twice() {
        let (mut a, api, _d, _p) = setup(FakeApi::new(), owning(&[7001]));
        a.last_plan = Some(plan(vec![pl("a", "E1", 1, 7001)]));
        api.set_connections(ConnectionsSnapshot {
            connections: vec![conn("c1", "a")],
            ..Default::default()
        });
        let late_api = api.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(100)).await;
            late_api.connections.lock().unwrap().connections.push(conn("late", "a"));
        });
        let out = a
            .apply(
                generated("new", plan(vec![pl("a", "E2", 1, 7001)])),
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
        assert_eq!(*api.closed.lock().unwrap(), vec!["c1", "late"]);
    }

    #[tokio::test]
    async fn effective_listeners_follow_what_mihomo_holds() {
        let api = FakeApi::with_reload_results(vec![Ok(()), Err(rejected())]);
        let (mut a, _api, _d, _p) = setup(api, owning(&[7001, 7002]));
        assert!(a.effective_listeners().is_empty());
        a.apply(generated("A", plan(vec![pl("a", "E1", 1, 7001)])), 9097, "s", PID, async || {})
            .await;
        assert_eq!(a.effective_listeners(), vec![ListenerId("id-a".into())]);
        // Adding b is rejected: mihomo keeps holding only a.
        let out = a
            .apply(
                generated("AB", plan(vec![pl("a", "E1", 1, 7001), pl("b", "E1", 1, 7002)])),
                9097,
                "s",
                PID,
                async || {},
            )
            .await;
        assert!(matches!(out, ApplyOutcome::FailedKeptPrevious { .. }), "{out:?}");
        assert_eq!(a.effective_listeners(), vec![ListenerId("id-a".into())]);
    }
}

use std::collections::VecDeque;
use std::io;
use std::net::Ipv4Addr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::mpsc;

use super::*;
use crate::core::api::{Conn, ConnMeta, ConnectionsSnapshot, MihomoApi};
use crate::core::apply::ApplyOutcome;
use crate::core::fake::FakeApi;
use crate::core::ports::FakePortOwners;
use crate::core::process::{CheckError, CoreHandleLike, CoreLauncher, CoreState};
use crate::model::health::ListenerStatus;
use crate::model::*;
use crate::netif::{FakeInterfaces, NetInterface};
use crate::store::{Bytes, Store};

const PID: u32 = 4242;
const PORT: u16 = 7891;
const NIC_GUID: &str = "{11111111-0000-0000-0000-000000000001}";

type ExitCell = Arc<Mutex<Option<Option<i32>>>>;

struct FakeHandle {
    exit: ExitCell,
    stopped: Arc<AtomicBool>,
}

#[async_trait]
impl CoreHandleLike for FakeHandle {
    async fn stop(&mut self) -> io::Result<()> {
        self.stopped.store(true, Ordering::SeqCst);
        let mut e = self.exit.lock().unwrap();
        if e.is_none() {
            *e = Some(None);
        }
        Ok(())
    }
    fn try_wait(&mut self) -> Option<Option<i32>> {
        *self.exit.lock().unwrap()
    }
    fn pid(&self) -> u32 {
        PID
    }
}

#[derive(Default)]
struct FakeLauncher {
    /// Config file content at the moment of each launch.
    launches: Mutex<Vec<String>>,
    exits: Mutex<Vec<ExitCell>>,
    stopped: Mutex<Vec<Arc<AtomicBool>>>,
    /// `check_config` results consumed in call order; once exhausted every check is Ok.
    check_results: Mutex<VecDeque<Result<(), CheckError>>>,
    /// Config file content at the moment of each check.
    checks: Mutex<Vec<String>>,
}

impl FakeLauncher {
    fn count(&self) -> usize {
        self.launches.lock().unwrap().len()
    }
    fn crash(&self, i: usize, code: i32) {
        *self.exits.lock().unwrap()[i].lock().unwrap() = Some(Some(code));
    }
    fn stopped(&self, i: usize) -> bool {
        self.stopped.lock().unwrap()[i].load(Ordering::SeqCst)
    }
    fn push_check(&self, r: Result<(), CheckError>) {
        self.check_results.lock().unwrap().push_back(r);
    }
}

#[async_trait]
impl CoreLauncher for FakeLauncher {
    async fn launch(&self, config: &Path) -> io::Result<Box<dyn CoreHandleLike + Send>> {
        let content = std::fs::read_to_string(config).unwrap_or_default();
        self.launches.lock().unwrap().push(content);
        let exit: ExitCell = Arc::new(Mutex::new(None));
        let stopped = Arc::new(AtomicBool::new(false));
        self.exits.lock().unwrap().push(exit.clone());
        self.stopped.lock().unwrap().push(stopped.clone());
        Ok(Box::new(FakeHandle { exit, stopped }))
    }
    async fn check_config(&self, config: &Path) -> Result<(), CheckError> {
        let content = std::fs::read_to_string(config).unwrap_or_default();
        self.checks.lock().unwrap().push(content);
        self.check_results.lock().unwrap().pop_front().unwrap_or(Ok(()))
    }
}

struct Harness {
    _td: tempfile::TempDir,
    data: PathBuf,
    rt: Runtime,
    api: Arc<FakeApi>,
    launcher: Arc<FakeLauncher>,
    nics: Arc<FakeInterfaces>,
    ports: Arc<FakePortOwners>,
    events: mpsc::Receiver<RuntimeEvent>,
    api_args: Arc<Mutex<Vec<(u16, String)>>>,
}

fn nic() -> NetInterface {
    NetInterface {
        guid: InterfaceGuid(NIC_GUID.into()),
        friendly_name: "以太网 2".into(),
        description: "Test Adapter".into(),
        is_up: true,
        ipv4: vec![Ipv4Addr::new(192, 168, 1, 10)],
        ipv6: vec![],
        mihomo_name: "以太网 2".into(),
    }
}

fn harness_with(model: Option<&Model>, prepare: impl FnOnce(&Path)) -> Harness {
    let td = tempfile::tempdir().unwrap();
    let data = td.path().join("data");
    let store = Store::open(data.clone()).unwrap();
    if let Some(m) = model {
        store.save_model(m).unwrap();
    }
    prepare(&data);
    let api = Arc::new(FakeApi::new());
    let launcher = Arc::new(FakeLauncher::default());
    let nics = Arc::new(FakeInterfaces(Mutex::new(vec![nic()])));
    let ports = Arc::new(FakePortOwners(Mutex::new([(PORT, PID), (19097, PID)].into_iter().collect())));
    let (tx, rx) = mpsc::channel(4096);
    let api_args = Arc::new(Mutex::new(Vec::new()));
    let launcher_for: LauncherFor = {
        let l = launcher.clone();
        Arc::new(move |_s: &Settings| l.clone() as Arc<dyn CoreLauncher>)
    };
    let api_for: ApiFor = {
        let a = api.clone();
        let args = api_args.clone();
        Box::new(move |port, secret| {
            args.lock().unwrap().push((port, secret));
            a.clone() as Arc<dyn MihomoApi>
        })
    };
    let rt = Runtime::new(store, nics.clone(), launcher_for, api_for, ports.clone(), tx);
    Harness { _td: td, data, rt, api, launcher, nics, ports, events: rx, api_args }
}

fn harness(model: &Model) -> Harness {
    harness_with(Some(model), |_| {})
}

/// One direct exit (optionally bound to the test NIC) and listener `L1` on it.
fn model_with_listener(bind_nic: bool) -> Model {
    let exit = Exit {
        id: ExitId("exit-1".into()),
        name: "直连出口".into(),
        kind: ExitKind::Direct,
        interface: bind_nic.then(|| InterfaceGuid(NIC_GUID.into())),
    };
    let mut l = Listener::new_default("L1".into(), PORT);
    l.id = ListenerId("lis-1".into());
    l.exit = Some(exit.id.clone());
    let mut m = Model { exits: vec![exit], listeners: vec![l], ..Model::default() };
    m.settings.delay_test_interval_mins = None;
    m
}

fn listener_names(yaml: &str) -> Vec<String> {
    let v: serde_yaml_ng::Value = serde_yaml_ng::from_str(yaml).unwrap();
    v["listeners"]
        .as_sequence()
        .map(|seq| {
            seq.iter()
                .map(|l| l["name"].as_str().unwrap().to_string())
                .collect()
        })
        .unwrap_or_default()
}

fn last_reload(api: &FakeApi) -> String {
    api.reloads.lock().unwrap().last().expect("no reload").1.clone()
}

fn conn(id: &str, inbound: &str, up: u64, down: u64) -> Conn {
    Conn {
        id: id.into(),
        upload: up,
        download: down,
        metadata: ConnMeta { inbound_name: inbound.into() },
    }
}

fn drain(rx: &mut mpsc::Receiver<RuntimeEvent>) -> Vec<RuntimeEvent> {
    let mut out = Vec::new();
    while let Ok(e) = rx.try_recv() {
        out.push(e);
    }
    out
}

#[tokio::test(start_paused = true)]
async fn mutate_invalid_returns_errors_and_does_not_save() {
    let m = model_with_listener(false);
    let h = harness(&m);
    h.rt.restart_core().await;
    let reloads_before = h.api.reloads.lock().unwrap().len();
    let before = std::fs::read_to_string(h.data.join("state.json")).unwrap();

    let res = h
        .rt
        .mutate(|m| {
            let mut dup = m.listeners[0].clone();
            dup.id = ListenerId("lis-2".into());
            m.listeners.push(dup); // duplicate name and port
        })
        .await;

    let errs = res.unwrap_err();
    assert!(!errs.is_empty());
    assert_eq!(std::fs::read_to_string(h.data.join("state.json")).unwrap(), before);
    assert_eq!(h.api.reloads.lock().unwrap().len(), reloads_before);
    assert_eq!(h.rt.model(), m);
}

#[tokio::test(start_paused = true)]
async fn mutate_valid_saves_and_applies() {
    let m = model_with_listener(false);
    let h = harness(&m);
    h.rt.restart_core().await;

    let out = h
        .rt
        .mutate(|m| m.listeners[0].name = "监听 😀".into())
        .await
        .unwrap();

    assert!(matches!(out, ApplyOutcome::Reloaded { uncut: 0, .. }), "{out:?}");
    let saved = Store::open(h.data.clone()).unwrap().load_model().unwrap();
    assert_eq!(saved.listeners[0].name, "监听 😀");
    assert_eq!(h.rt.model(), saved);
    assert_eq!(listener_names(&last_reload(&h.api)), vec!["监听 😀".to_string()]);
    let st = h.rt.status();
    assert_eq!(st.last_apply, Some(out));
    assert_eq!(st.core, CoreState::Running);
}

#[tokio::test(start_paused = true)]
async fn first_apply_after_empty_config() {
    let m = model_with_listener(false);
    let h = harness(&m);
    h.rt.restart_core().await;

    let (port, secret) = h.api_args.lock().unwrap()[0].clone();
    assert_eq!(port, 19097);
    assert_eq!(secret.len(), 64);
    assert!(secret.chars().all(|c| c.is_ascii_hexdigit()));
    // mihomo is launched with the empty config, then the model is applied.
    assert_eq!(
        h.launcher.launches.lock().unwrap()[0],
        crate::config_gen::empty_config(19097, &secret)
    );
    let reloads = h.api.reloads.lock().unwrap().clone();
    assert_eq!(reloads.len(), 1);
    assert_eq!(listener_names(&reloads[0].1), vec!["L1".to_string()]);
    assert!(reloads[0].1.contains(&secret));
    // The secret never lands in the persisted model.
    assert!(!std::fs::read_to_string(h.data.join("state.json")).unwrap().contains(&secret));

    // A restart uses a fresh secret.
    h.rt.restart_core().await;
    let args = h.api_args.lock().unwrap().clone();
    assert_eq!(args.len(), 2);
    assert_ne!(args[0].1, args[1].1);
}

#[tokio::test(start_paused = true)]
async fn interface_unplug_disables_listener() {
    let m = model_with_listener(true);
    let h = harness(&m);
    h.rt.restart_core().await;
    assert_eq!(listener_names(&last_reload(&h.api)), vec!["L1".to_string()]);
    h.api.set_connections(ConnectionsSnapshot {
        connections: vec![conn("c1", "L1", 0, 0)],
        ..Default::default()
    });

    // mihomo closes the removed listener's socket.
    h.ports.0.lock().unwrap().remove(&PORT);
    h.nics.0.lock().unwrap().clear();
    assert!(h.rt.refresh_interfaces().await);

    assert!(listener_names(&last_reload(&h.api)).is_empty());
    assert_eq!(h.api.closed.lock().unwrap().clone(), vec!["c1".to_string()]);
    let st = h.rt.status();
    assert!(matches!(st.last_apply, Some(ApplyOutcome::Reloaded { .. })), "{:?}", st.last_apply);
    assert!(st.interfaces.is_empty());
    assert!(matches!(
        st.health.listeners[&ListenerId("lis-1".into())],
        ListenerStatus::ExitBroken { .. }
    ));
    // No further change: nothing to do.
    assert!(!h.rt.refresh_interfaces().await);
}

#[tokio::test(start_paused = true)]
async fn interface_replug_restores_listener() {
    let m = model_with_listener(true);
    let h = harness(&m);
    h.nics.0.lock().unwrap().clear();
    h.rt.restart_core().await;
    assert!(listener_names(&last_reload(&h.api)).is_empty());

    h.nics.0.lock().unwrap().push(nic());
    assert!(h.rt.refresh_interfaces().await);

    assert_eq!(listener_names(&last_reload(&h.api)), vec!["L1".to_string()]);
    assert!(last_reload(&h.api).contains("以太网 2"));
    assert_eq!(
        h.rt.status().health.listeners[&ListenerId("lis-1".into())],
        ListenerStatus::Active
    );
}

#[tokio::test(start_paused = true)]
async fn failed_listener_retried_until_port_free() {
    let m = model_with_listener(false);
    let h = harness(&m);
    h.ports.0.lock().unwrap().insert(PORT, 999); // someone else holds the port
    h.rt.start().await.unwrap();

    let st = h.rt.status();
    assert_eq!(st.failed_listeners.len(), 1);
    assert_eq!(st.failed_listeners[0].port, PORT);
    let reloads = h.api.reloads.lock().unwrap().len();

    h.ports.0.lock().unwrap().insert(PORT, PID); // port freed, mihomo got it
    tokio::time::sleep(Duration::from_secs(31)).await;

    assert!(h.rt.status().failed_listeners.is_empty());
    let after = h.api.reloads.lock().unwrap().clone();
    assert!(after.len() > reloads);
    assert_eq!(listener_names(&after.last().unwrap().1), vec!["L1".to_string()]);
    let reloads = after.len();

    // Once empty, no further retries.
    tokio::time::sleep(Duration::from_secs(61)).await;
    assert_eq!(h.api.reloads.lock().unwrap().len(), reloads);
    h.rt.shutdown().await;
}

#[tokio::test(start_paused = true)]
async fn crash_restart_uses_latest_model() {
    let m = model_with_listener(false);
    let mut h = harness(&m);
    h.rt.start().await.unwrap();
    h.rt.mutate(|m| m.listeners[0].name = "新改动".into()).await.unwrap();

    h.launcher.crash(0, 3);
    tokio::time::sleep(Duration::from_secs(5)).await;

    assert_eq!(h.launcher.count(), 2);
    assert_eq!(listener_names(&last_reload(&h.api)), vec!["新改动".to_string()]);
    assert_eq!(h.rt.status().core, CoreState::Running);
    let crashed = drain(&mut h.events).into_iter().any(|e| {
        matches!(e, RuntimeEvent::Status(s) if s.core == CoreState::Crashed { exit_code: Some(3) })
    });
    assert!(crashed);

    // stop_core stops supervision: a later exit is not restarted.
    h.rt.stop_core().await;
    assert_eq!(h.rt.status().core, CoreState::Stopped);
    assert!(h.launcher.stopped(1));
    tokio::time::sleep(Duration::from_secs(60)).await;
    assert_eq!(h.launcher.count(), 2);
    h.rt.shutdown().await;
}

#[tokio::test(start_paused = true)]
async fn uncut_connections_trigger_core_restart() {
    let m = model_with_listener(false);
    let h = harness(&m);
    h.rt.restart_core().await;
    h.api.set_connections(ConnectionsSnapshot {
        connections: vec![conn("c1", "L1", 0, 0)],
        ..Default::default()
    });
    h.api.close_fails.lock().unwrap().insert("c1".into());
    // mihomo closes the disabled listener's socket.
    h.ports.0.lock().unwrap().remove(&PORT);

    let out = h.rt.mutate(|m| m.listeners[0].user_enabled = false).await.unwrap();

    assert!(matches!(out, ApplyOutcome::Reloaded { uncut: 1, .. }), "{out:?}");
    assert_eq!(h.launcher.count(), 2);
    assert!(h.launcher.stopped(0));
    assert_eq!(h.rt.status().core, CoreState::Running);
    // The restarted core gets the empty config first, then the latest model.
    let secret = h.api_args.lock().unwrap()[1].1.clone();
    assert_eq!(
        h.launcher.launches.lock().unwrap()[1],
        crate::config_gen::empty_config(19097, &secret)
    );
    assert!(listener_names(&last_reload(&h.api)).is_empty());
}

#[tokio::test(start_paused = true)]
async fn corrupt_state_is_quarantined_not_overwritten() {
    let h = harness_with(None, |data| {
        std::fs::write(data.join("state.json"), "{ not json").unwrap();
        std::fs::write(data.join("traffic.json"), "").unwrap();
    });

    let st = h.rt.status();
    let err = st.startup_error.expect("startup_error");
    assert!(err.contains("state.json"), "{err}");
    assert!(err.contains("traffic.json"), "{err}");
    assert_eq!(h.rt.model(), Model::default());

    let names: Vec<String> = std::fs::read_dir(&h.data)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    let corrupt = names
        .iter()
        .find(|n| n.starts_with("state.json.corrupt-"))
        .unwrap_or_else(|| panic!("{names:?}"));
    assert!(names.iter().any(|n| n.starts_with("traffic.json.corrupt-")), "{names:?}");
    assert!(!names.contains(&"state.json".to_string()), "{names:?}");

    h.rt.restart_core().await;
    h.rt.mutate(|m| m.settings.autostart = true).await.unwrap();
    assert_eq!(
        std::fs::read_to_string(h.data.join(corrupt)).unwrap(),
        "{ not json"
    );
    assert!(Store::open(h.data.clone()).unwrap().load_model().unwrap().settings.autostart);
}

#[tokio::test]
async fn subscription_due_is_fetched_and_applied() {
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            "proxies:\n  - {name: a, type: ss, server: 1.1.1.1, port: 1, cipher: aes-128-gcm, password: x}\n",
        ))
        .mount(&server)
        .await;

    let sub_id = SubscriptionId("sub-1".into());
    let mut m = model_with_listener(false);
    m.exits[0].kind = ExitKind::Node {
        node: NodeRef::Subscription { subscription_id: sub_id.clone(), name: "a".into() },
    };
    m.subscriptions.push(Subscription {
        id: sub_id.clone(),
        name: "订阅".into(),
        url: format!("{}/sub", server.uri()),
        update_interval_mins: Some(60),
        fallback_exit: None,
        last_updated: None,
        last_result: None,
        nodes: vec![],
    });
    let h = harness(&m);
    h.rt.restart_core().await;
    assert!(listener_names(&last_reload(&h.api)).is_empty()); // node missing

    h.rt.update_due_subscriptions().await;

    let model = h.rt.model();
    assert_eq!(model.subscriptions[0].nodes.len(), 1);
    assert_eq!(model.subscriptions[0].last_result, Some(FetchResult::Ok { node_count: 1, skipped_tailscale: 0 }));
    assert!(model.subscriptions[0].last_updated.is_some());
    let saved = Store::open(h.data.clone()).unwrap().load_model().unwrap();
    assert_eq!(saved, model);
    let yaml = last_reload(&h.api);
    assert!(yaml.contains("订阅/a"), "{yaml}");
    assert_eq!(listener_names(&yaml), vec!["L1".to_string()]);

    // Not due any more.
    let reloads = h.api.reloads.lock().unwrap().len();
    h.rt.update_due_subscriptions().await;
    assert_eq!(h.api.reloads.lock().unwrap().len(), reloads);
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
}

#[tokio::test(start_paused = true)]
async fn shutdown_persists_totals() {
    let m = model_with_listener(false);
    let h = harness(&m);
    h.api.set_connections(ConnectionsSnapshot {
        upload_total: 100,
        download_total: 200,
        connections: vec![conn("c1", "L1", 100, 200)],
    });
    h.rt.start().await.unwrap();
    tokio::time::sleep(Duration::from_millis(1500)).await;
    let tick = h.rt.status().traffic.expect("traffic sampled");
    assert_eq!(tick.totals.per_listener[&ListenerId("lis-1".into())], Bytes { up: 100, down: 200 });

    h.rt.shutdown().await;

    let totals = Store::open(h.data.clone()).unwrap().load_totals().unwrap();
    assert_eq!(totals.per_listener[&ListenerId("lis-1".into())], Bytes { up: 100, down: 200 });
    assert!(h.launcher.stopped(0));
    assert_eq!(h.rt.status().core, CoreState::Stopped);
    // No background task survives shutdown.
    tokio::time::sleep(Duration::from_secs(120)).await;
    assert_eq!(h.launcher.count(), 1);
}

#[tokio::test(start_paused = true)]
async fn delay_test_records_results() {
    let mut m = model_with_listener(false);
    m.settings.delay_test_interval_mins = Some(10);
    let h = harness(&m);
    h.api.set_delay("直连出口", Ok(42));
    h.rt.start().await.unwrap();
    tokio::time::sleep(Duration::from_secs(1)).await;

    let st = h.rt.status();
    assert_eq!(st.delays.get("直连出口").unwrap().ms, Some(42));
    let calls = h.api.delay_calls.lock().unwrap().clone();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].1, "https://www.gstatic.com/generate_204");

    let res = h.rt.test_delays(vec!["x".into()]).await;
    assert_eq!(res[0].1.ms, None);
    assert!(h.rt.status().delays.get("x").is_some());
    h.rt.shutdown().await;
}

/// Task 14 awaits these from Tauri commands, which require `Send` futures.
#[allow(dead_code)]
fn runtime_futures_are_send(rt: &Runtime) {
    fn send<T: Send>(_: T) {}
    send(rt.start());
    send(rt.shutdown());
    send(rt.mutate(|_| {}));
    send(rt.refresh_interfaces());
    send(rt.restart_core());
    send(rt.stop_core());
    send(rt.refresh_subscription(SubscriptionId("x".into())));
    send(rt.test_delays(vec![]));
    send(rt.reset_traffic());
}

#[tokio::test(start_paused = true)]
async fn save_failure_is_reported_in_status() {
    let m = model_with_listener(false);
    let h = harness(&m);
    h.rt.restart_core().await;
    // A directory where state.json should be: the atomic rename fails.
    let state = h.data.join("state.json");
    std::fs::remove_file(&state).unwrap();
    std::fs::create_dir(&state).unwrap();

    let out = h.rt.mutate(|m| m.listeners[0].name = "改名".into()).await.unwrap();
    assert!(matches!(out, ApplyOutcome::Reloaded { .. }), "{out:?}");
    let err = h.rt.status().save_error.expect("save_error");
    assert!(err.contains("保存配置失败"), "{err}");

    std::fs::remove_dir(&state).unwrap();
    h.rt.mutate(|m| m.listeners[0].name = "再改".into()).await.unwrap();
    assert_eq!(h.rt.status().save_error, None);
    assert_eq!(Store::open(h.data.clone()).unwrap().load_model().unwrap().listeners[0].name, "再改");
}

#[tokio::test(start_paused = true)]
async fn no_core_launch_after_shutdown() {
    let mut m = model_with_listener(false);
    m.settings.delay_test_interval_mins = Some(10);
    let h = harness(&m);
    h.api.set_delay("直连出口", Ok(42));
    h.rt.start().await.unwrap();
    tokio::time::sleep(Duration::from_secs(1)).await;
    assert_eq!(h.rt.status().delays.get("直连出口").unwrap().ms, Some(42));
    h.rt.shutdown().await;
    let state_before = std::fs::read_to_string(h.data.join("state.json")).unwrap();
    let reloads_before = h.api.reloads.lock().unwrap().len();

    h.rt.restart_core().await;
    assert!(h.rt.start().await.is_err());
    let out = h.rt.mutate(|m| m.listeners[0].name = "x".into()).await.unwrap();
    assert!(matches!(out, ApplyOutcome::FailedCoreStopped { .. }), "{out:?}");
    assert!(matches!(
        h.rt.refresh_subscription(SubscriptionId("none".into())).await,
        FetchResult::Failed { .. }
    ));
    // No core: Nones go to the caller only, the DelayBook keeps old results.
    let res = h.rt.test_delays(vec!["直连出口".into()]).await;
    assert_eq!(res[0].1.ms, None);
    assert_eq!(h.rt.status().delays.get("直连出口").unwrap().ms, Some(42));
    assert!(!h.rt.refresh_interfaces().await);
    tokio::time::sleep(Duration::from_secs(120)).await;

    assert_eq!(h.launcher.count(), 1);
    assert_eq!(h.rt.status().core, CoreState::Stopped);
    assert_eq!(std::fs::read_to_string(h.data.join("state.json")).unwrap(), state_before);
    assert_eq!(h.api.reloads.lock().unwrap().len(), reloads_before);
    assert_eq!(h.rt.model(), m);
}

#[tokio::test]
async fn stale_fetch_discarded_when_url_changed() {
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(
                    "proxies:\n  - {name: a, type: ss, server: 1.1.1.1, port: 1, cipher: aes-128-gcm, password: x}\n",
                )
                .set_delay(Duration::from_millis(500)),
        )
        .mount(&server)
        .await;

    let sub_id = SubscriptionId("sub-1".into());
    let mut m = model_with_listener(false);
    m.subscriptions.push(Subscription {
        id: sub_id.clone(),
        name: "订阅".into(),
        url: format!("{}/old", server.uri()),
        update_interval_mins: Some(60),
        fallback_exit: None,
        last_updated: None,
        last_result: None,
        nodes: vec![],
    });
    let h = harness(&m);
    h.rt.restart_core().await;
    let new_url = format!("{}/new", server.uri());

    let (res, _) = tokio::join!(h.rt.refresh_subscription(sub_id.clone()), async {
        tokio::time::sleep(Duration::from_millis(100)).await;
        let u = new_url.clone();
        h.rt.mutate(move |m| m.subscriptions[0].url = u).await.unwrap();
    });

    assert!(matches!(&res, FetchResult::Failed { reason } if reason.contains("已更改")), "{res:?}");
    let sub = h.rt.model().subscriptions[0].clone();
    assert_eq!(sub.url, new_url);
    assert!(sub.nodes.is_empty());
    assert_eq!(sub.last_updated, None);
    assert_eq!(sub.last_result, None);
}

#[tokio::test(start_paused = true)]
async fn foreign_controller_is_not_used() {
    let m = model_with_listener(false);
    let h = harness(&m);
    h.ports.0.lock().unwrap().insert(19097, PID + 1);
    h.rt.restart_core().await;
    let st = h.rt.status();
    assert!(matches!(st.core, CoreState::Crashed { .. }), "{:?}", st.core);
    let msg = st.startup_error.expect("startup_error set");
    assert!(msg.contains("控制端口 19097 已被其他程序占用，请在设置中更换"), "{msg}");
    assert!(h.launcher.stopped(0), "our child was stopped");
    assert!(h.api_args.lock().unwrap().is_empty(), "no API client was even created");
    assert!(h.api.reloads.lock().unwrap().is_empty(), "no reload reached the foreign controller");
}

/// Names of the leftover `check-*.yaml` files in the core workdir.
fn check_files(data: &Path) -> Vec<String> {
    std::fs::read_dir(data.join("mihomo"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("check-"))
        .collect()
}

#[tokio::test]
async fn invalid_nodes_rejected_by_core_keep_old_nodes() {
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            "proxies:
  - {name: bad, type: ss, server: 2.2.2.2, port: 1, password: x}
",
        ))
        .mount(&server)
        .await;

    let sub_id = SubscriptionId("sub-1".into());
    let old = Node {
        name: "a".into(),
        proxy_type: "ss".into(),
        server: "1.1.1.1".into(),
        raw: serde_json::json!({"name": "a", "type": "ss", "server": "1.1.1.1", "port": 1, "cipher": "aes-128-gcm", "password": "x"}),
    };
    let mut m = model_with_listener(false);
    m.subscriptions.push(Subscription {
        id: sub_id.clone(),
        name: "订阅".into(),
        url: format!("{}/sub", server.uri()),
        update_interval_mins: Some(60),
        fallback_exit: None,
        last_updated: None,
        last_result: None,
        nodes: vec![old.clone()],
    });
    let h = harness(&m);
    h.rt.restart_core().await;
    let reloads = h.api.reloads.lock().unwrap().len();
    h.launcher.push_check(Err(CheckError::Rejected("proxy 0: '' has unset fields: cipher".into())));

    let res = h.rt.refresh_subscription(sub_id).await;

    let reason = match &res {
        FetchResult::Failed { reason } => reason.clone(),
        other => panic!("{other:?}"),
    };
    assert_eq!(reason, "订阅节点未通过 mihomo 校验：proxy 0: '' has unset fields: cipher");
    let sub = h.rt.model().subscriptions[0].clone();
    assert_eq!(sub.nodes, vec![old]);
    assert_eq!(sub.last_result, Some(res));
    assert_eq!(Store::open(h.data.clone()).unwrap().load_model().unwrap(), h.rt.model());
    assert_eq!(h.api.reloads.lock().unwrap().len(), reloads, "nothing applied");
    // The checked config held just the fetched nodes, under their generated names.
    let checked = h.launcher.checks.lock().unwrap().last().cloned().unwrap();
    let v: serde_yaml_ng::Value = serde_yaml_ng::from_str(&checked).unwrap();
    assert_eq!(v["proxies"].as_sequence().unwrap().len(), 1);
    assert_eq!(v["proxies"][0]["name"], "订阅/bad");
    assert!(v["listeners"].as_sequence().unwrap().is_empty());
    assert!(check_files(&h.data).is_empty(), "temp config removed");
}

#[tokio::test(start_paused = true)]
async fn mutate_rejected_by_core_is_not_saved_or_applied() {
    let m = model_with_listener(false);
    let h = harness(&m);
    h.rt.restart_core().await;
    let reloads = h.api.reloads.lock().unwrap().len();
    let before = std::fs::read_to_string(h.data.join("state.json")).unwrap();
    h.launcher.push_check(Err(CheckError::Rejected("proxy group[0]: format error".into())));

    let errs = h.rt.mutate(|m| m.listeners[0].name = "新名字".into()).await.unwrap_err();

    assert_eq!(
        errs,
        vec![ValidationError::CoreRejected { message: "proxy group[0]: format error".into() }]
    );
    assert_eq!(std::fs::read_to_string(h.data.join("state.json")).unwrap(), before);
    assert_eq!(h.api.reloads.lock().unwrap().len(), reloads);
    assert_eq!(h.rt.model(), m);
    // The generated config was what got checked, without the real secret.
    let checked = h.launcher.checks.lock().unwrap().last().cloned().unwrap();
    assert_eq!(listener_names(&checked), vec!["新名字".to_string()]);
    let secret = h.api_args.lock().unwrap()[0].1.clone();
    assert!(!checked.contains(&secret));
    assert!(check_files(&h.data).is_empty(), "temp config removed");
}

#[tokio::test(start_paused = true)]
async fn mutate_skips_check_when_core_cannot_run() {
    let m = model_with_listener(false);
    let h = harness(&m);
    h.rt.restart_core().await;
    h.launcher.push_check(Err(CheckError::Unavailable("无法运行 mihomo".into())));

    let out = h.rt.mutate(|m| m.listeners[0].name = "新名字".into()).await.unwrap();

    assert!(matches!(out, ApplyOutcome::Reloaded { .. }), "{out:?}");
    assert_eq!(h.rt.model().listeners[0].name, "新名字");
    assert_eq!(listener_names(&last_reload(&h.api)), vec!["新名字".to_string()]);
}

#[tokio::test(start_paused = true)]
async fn failed_apply_is_retried() {
    let m = model_with_listener(false);
    let h = harness(&m);
    h.ports.0.lock().unwrap().insert(7892, PID);
    h.rt.start().await.unwrap();
    let rejected = || crate::core::api::ApiError::Rejected { status: 400, message: "bad".into() };

    // Renaming is restrictive: the rejected reload fails closed.
    h.api.reload_results.lock().unwrap().push_back(Err(rejected()));
    let out = h.rt.mutate(|m| m.listeners[0].name = "改名".into()).await.unwrap();
    assert!(matches!(out, ApplyOutcome::FailedClosed { .. }), "{out:?}");
    assert!(listener_names(&last_reload(&h.api)).is_empty());
    tokio::time::sleep(Duration::from_secs(31)).await;
    assert!(matches!(h.rt.status().last_apply, Some(ApplyOutcome::Reloaded { .. })));
    assert_eq!(listener_names(&last_reload(&h.api)), vec!["改名".to_string()]);

    // Adding a listener is not: the rejected reload keeps the previous config.
    h.api.reload_results.lock().unwrap().push_back(Err(rejected()));
    let out = h
        .rt
        .mutate(|m| {
            let mut l = Listener::new_default("L2".into(), 7892);
            l.exit = m.listeners[0].exit.clone();
            m.listeners.push(l);
        })
        .await
        .unwrap();
    assert!(matches!(out, ApplyOutcome::FailedKeptPrevious { .. }), "{out:?}");
    tokio::time::sleep(Duration::from_secs(31)).await;
    assert!(matches!(h.rt.status().last_apply, Some(ApplyOutcome::Reloaded { .. })));
    assert_eq!(listener_names(&last_reload(&h.api)), vec!["改名".to_string(), "L2".to_string()]);

    // Once applied, no further retries.
    let reloads = h.api.reloads.lock().unwrap().len();
    tokio::time::sleep(Duration::from_secs(61)).await;
    assert_eq!(h.api.reloads.lock().unwrap().len(), reloads);
    h.rt.shutdown().await;
}

#[tokio::test(start_paused = true)]
async fn effective_listeners_stay_empty_while_mihomo_holds_the_empty_config() {
    let m = model_with_listener(false);
    let h = harness(&m);
    h.rt.start().await.unwrap();
    let id = h.rt.model().listeners[0].id.clone();
    assert_eq!(h.rt.status().effective_listeners, vec![id.clone()]);
    let rejected = || crate::core::api::ApiError::Rejected { status: 400, message: "bad".into() };

    h.api.reload_results.lock().unwrap().push_back(Err(rejected()));
    let out = h.rt.mutate(|m| m.listeners[0].name = "改名".into()).await.unwrap();
    assert!(matches!(out, ApplyOutcome::FailedClosed { .. }), "{out:?}");
    assert!(h.rt.status().effective_listeners.is_empty());

    // The retry is rejected again; from the empty config that is no longer
    // tightening, so it reports FailedKeptPrevious: still nothing in effect.
    h.api.reload_results.lock().unwrap().push_back(Err(rejected()));
    tokio::time::sleep(Duration::from_secs(31)).await;
    let st = h.rt.status();
    assert!(matches!(st.last_apply, Some(ApplyOutcome::FailedKeptPrevious { .. })), "{:?}", st.last_apply);
    assert!(st.effective_listeners.is_empty());

    tokio::time::sleep(Duration::from_secs(30)).await;
    let st = h.rt.status();
    assert!(matches!(st.last_apply, Some(ApplyOutcome::Reloaded { .. })), "{:?}", st.last_apply);
    assert_eq!(st.effective_listeners, vec![id]);

    h.rt.stop_core().await;
    assert!(h.rt.status().effective_listeners.is_empty());
    h.rt.shutdown().await;
}

#[tokio::test(start_paused = true)]
async fn mutate_without_core_is_saved_but_not_running() {
    let m = model_with_listener(false);
    let h = harness(&m);
    let out = h.rt.mutate(|m| m.listeners[0].name = "改名".into()).await.unwrap();
    assert!(matches!(out, ApplyOutcome::NotRunning { .. }), "{out:?}");
    assert_eq!(Store::open(h.data.clone()).unwrap().load_model().unwrap().listeners[0].name, "改名");
}

#[tokio::test(start_paused = true)]
async fn controller_port_or_core_path_change_restarts_core() {
    let m = model_with_listener(false);
    let h = harness(&m);
    h.ports.0.lock().unwrap().insert(19100, PID);
    h.rt.start().await.unwrap();

    // Unrelated settings: hot reload only.
    h.rt.mutate(|m| m.settings.autostart = true).await.unwrap();
    assert_eq!(h.launcher.count(), 1);

    let out = h.rt.mutate(|m| m.settings.controller_port = 19100).await.unwrap();
    assert!(matches!(out, ApplyOutcome::Reloaded { .. }), "{out:?}");
    assert_eq!(h.launcher.count(), 2);
    assert!(h.launcher.stopped(0));
    let secret = h.api_args.lock().unwrap().last().unwrap().1.clone();
    assert_eq!(h.api_args.lock().unwrap().last().unwrap().0, 19100);
    assert_eq!(h.launcher.launches.lock().unwrap()[1], crate::config_gen::empty_config(19100, &secret));
    assert_eq!(listener_names(&last_reload(&h.api)), vec!["L1".to_string()]);

    h.rt.mutate(|m| m.settings.core_path = Some("C:/mihomo.exe".into())).await.unwrap();
    assert_eq!(h.launcher.count(), 3);
    assert_eq!(h.rt.status().core, CoreState::Running);

    // A stopped core stays stopped.
    h.rt.stop_core().await;
    let out = h.rt.mutate(|m| m.settings.controller_port = 19097).await.unwrap();
    assert!(matches!(out, ApplyOutcome::NotRunning { .. }), "{out:?}");
    assert_eq!(h.launcher.count(), 3);
    h.rt.shutdown().await;
}

#[tokio::test]
async fn fallback_fetch_only_through_a_port_our_core_owns() {
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(method("GET")).respond_with(ResponseTemplate::new(403)).mount(&server).await;

    let sub_id = SubscriptionId("sub-1".into());
    let mut m = model_with_listener(false);
    m.subscriptions.push(Subscription {
        id: sub_id.clone(),
        name: "订阅".into(),
        url: format!("{}/sub?token=x", server.uri()),
        update_interval_mins: None,
        fallback_exit: Some(ExitId("exit-1".into())),
        last_updated: None,
        last_result: None,
        nodes: vec![],
    });
    let h = harness(&m);
    h.rt.restart_core().await;
    let yaml = last_reload(&h.api);
    let v: serde_yaml_ng::Value = serde_yaml_ng::from_str(&yaml).unwrap();
    let fetch = v["listeners"]
        .as_sequence()
        .unwrap()
        .iter()
        .find(|l| l["name"] == "__fetch__exit-1")
        .expect("fetch listener");
    let port = fetch["port"].as_u64().unwrap() as u16;
    let reason = |r: FetchResult| match r {
        FetchResult::Failed { reason } => reason,
        other => panic!("{other:?}"),
    };

    // Another process holds the fetch port: no proxy attempt at all.
    h.ports.0.lock().unwrap().insert(port, PID + 1);
    let r = reason(h.rt.refresh_subscription(sub_id.clone()).await);
    assert_eq!(r, "HTTP 403");

    // Our mihomo holds it: the fallback is tried.
    h.ports.0.lock().unwrap().insert(port, PID);
    let r = reason(h.rt.refresh_subscription(sub_id).await);
    assert!(r.contains("通过出口失败"), "{r}");
}

fn ts_manual(name: &str, iface: Option<&str>) -> ManualNode {
    let mut o = TailscaleOpts::defaults_for(name);
    o.interface = iface.map(|g| InterfaceGuid(g.into()));
    ManualNode {
        id: ManualNodeId(format!("ts-{}", name.bytes().map(|b| format!("{b:02x}")).collect::<String>())),
        name: name.into(),
        groups: vec![],
        kind: ManualNodeKind::Tailscale(o),
    }
}

fn delay_proxies(api: &FakeApi) -> Vec<String> {
    api.delay_calls.lock().unwrap().iter().map(|c| c.0.clone()).collect()
}

#[tokio::test(start_paused = true)]
async fn applied_mutation_prewarms_tailscale_instances() {
    let m = model_with_listener(false);
    let h = harness(&m);
    h.rt.restart_core().await;
    tokio::time::sleep(Duration::from_secs(1)).await;
    assert!(delay_proxies(&h.api).is_empty(), "no Tailscale instance -> no prewarm");

    let out = h
        .rt
        .mutate(|m| {
            m.manual_nodes.push(ts_manual("家里", None));
            // Bound to an adapter that does not exist: not emitted, not prewarmed.
            m.manual_nodes.push(ts_manual("缺网卡", Some("{99999999-0000-0000-0000-000000000000}")));
        })
        .await
        .unwrap();
    assert!(matches!(out, ApplyOutcome::Reloaded { .. }), "{out:?}");
    tokio::time::sleep(Duration::from_secs(1)).await;
    let calls = h.api.delay_calls.lock().unwrap().clone();
    assert_eq!(delay_proxies(&h.api), vec!["手动/家里".to_string()]);
    assert_eq!(calls[0].1, h.rt.model().settings.delay_test_url);
    assert_eq!(calls[0].2, crate::stats::delay::DEFAULT_TIMEOUT_MS);
    // The cold-start dial is not a measurement: nothing lands in the DelayBook.
    assert!(h.rt.status().delays.get("手动/家里").is_none(), "{:?}", h.rt.status().delays);

    // A core (re)start prewarms too.
    h.rt.restart_core().await;
    tokio::time::sleep(Duration::from_secs(1)).await;
    assert_eq!(delay_proxies(&h.api).len(), 2);
    assert!(h.rt.status().delays.get("手动/家里").is_none());
}

#[tokio::test(start_paused = true)]
async fn applied_mutation_without_tailscale_does_not_prewarm() {
    let m = model_with_listener(false);
    let h = harness(&m);
    h.rt.restart_core().await;
    h.rt.mutate(|m| m.listeners[0].name = "L2".into()).await.unwrap();
    tokio::time::sleep(Duration::from_secs(1)).await;
    assert!(delay_proxies(&h.api).is_empty());
}

#[tokio::test(start_paused = true)]
async fn group_now_reports_known_selections() {
    let m = model_with_listener(false);
    let h = harness(&m);
    assert!(h.rt.group_now(vec!["组".into()]).await.is_empty(), "no core -> empty");
    h.rt.restart_core().await;
    h.api.proxy_nows.lock().unwrap().insert("组".into(), "组/订阅/HK".into());
    let got = h.rt.group_now(vec!["组".into(), "别的".into()]).await;
    assert_eq!(got, [("组".to_string(), "组/订阅/HK".to_string())].into_iter().collect());
}

const TS_NEEDS: &str = "[Tailscale](手动/家里) LocalBackend state is NeedsLogin; running StartLoginInteractive...";
const TS_URL: &str =
    "[Tailscale](手动/家里) To start this tsnet server, restart with TS_AUTHKEY set, or go to: https://login.tailscale.com/a/1654b85a0197ca";
const TS_RUNNING: &str = "[Tailscale](手动/家里) AuthLoop: state is Running; done";

fn ts_state(h: &Harness, name: &str) -> Option<crate::tailscale::TailscaleState> {
    h.rt.status().tailscale.get(name).cloned()
}

#[tokio::test(start_paused = true)]
async fn tailscale_state_follows_core_log() {
    use crate::tailscale::TailscaleState;
    let mut m = model_with_listener(false);
    m.manual_nodes.push(ts_manual("家里", None));
    let mut h = harness(&m);
    assert!(h.rt.status().tailscale.is_empty());
    h.rt.restart_core().await;
    h.api.push_log(TS_NEEDS);
    h.api.push_log(TS_URL);
    drain(&mut h.events);
    tokio::time::sleep(Duration::from_secs(3)).await;
    let want = TailscaleState::NeedsLogin {
        url: Some("https://login.tailscale.com/a/1654b85a0197ca".into()),
    };
    assert_eq!(ts_state(&h, "手动/家里"), Some(want.clone()));
    // The change reached the frontend as a status event.
    let emitted = drain(&mut h.events).into_iter().any(|e| {
        matches!(e, RuntimeEvent::Status(s) if s.tailscale.get("手动/家里") == Some(&want))
    });
    assert!(emitted);

    // A new core process starts from scratch: the old URL is gone.
    h.rt.restart_core().await;
    assert_eq!(ts_state(&h, "手动/家里"), None);
    h.api.push_log(TS_RUNNING);
    tokio::time::sleep(Duration::from_secs(3)).await;
    assert_eq!(ts_state(&h, "手动/家里"), Some(TailscaleState::LoggedIn { ip: None }));

    // A deleted node is forgotten after the next apply.
    h.rt.mutate(|m| m.manual_nodes.clear()).await.unwrap();
    assert_eq!(ts_state(&h, "手动/家里"), None);

    // Stopping the core clears every state.
    h.rt.mutate(|m| m.manual_nodes.push(ts_manual("家里", None))).await.unwrap();
    h.api.push_log(TS_RUNNING);
    tokio::time::sleep(Duration::from_secs(3)).await;
    assert!(ts_state(&h, "手动/家里").is_some());
    h.rt.stop_core().await;
    assert!(h.rt.status().tailscale.is_empty());
}

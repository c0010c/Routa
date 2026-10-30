//! Tauri-independent orchestration: model + store + health + config generation
//! + fail-closed apply + mihomo supervision + schedulers + NIC watch.
//!
//! Locking: every model mutation, Store save, apply and core (re)start runs
//! under the one async `engine` mutex. `status`, the model mirror and the
//! live api/plan are std mutexes, never held across an await. Traffic updates
//! take `live` before `traffic`/`status` so samples and corrections stay ordered.

mod tasks;

use crate::model::ExitId;

use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::Instant;

use crate::config_gen::{
    delay_fingerprints, emitted_tailscale_names, empty_config, generate_for_capture, node_proxy_name, nodes_check_config, GenInput,
};
use crate::core::api::{ConnectionsSnapshot, MihomoApi};
use crate::core::apply::{Applier, ApplyOutcome, FailedListener};
use crate::core::ports::PortOwners;
use crate::core::process::{wait_ready, CheckError, CoreHandleLike, CoreLauncher, CoreState};
use crate::model::health::{evaluate, ExitStatus, Health};
use crate::model::normalize::normalize;
use crate::model::{
    validate, validate::validate_forbidden_exes, FetchResult, ListenerId, Model, Node, ProcessRouting, Settings,
    Subscription, SubscriptionId, ValidationError,
};
use crate::netif::{InterfaceSource, NetInterface};
use crate::stats::delay::{test_many, DelayBook, DelayError, DelayResult, DEFAULT_CONCURRENCY, DEFAULT_TIMEOUT_MS};
use crate::stats::traffic::{Rate, TrafficAggregator, TrafficTick};
use crate::store::{Store, TrafficTotals};
use crate::subscription::fetch::{fetch_listener_ports, record_result, update_subscription, FetchError, PortAllocator};
use crate::subscription::Parsed;
use crate::tailscale::{TailscaleState, TailscaleTracker};

/// Builds the launcher for the current settings (core_path or sidecar); called
/// at every (re)start so a changed `core_path` takes effect on restart.
pub type LauncherFor = Arc<dyn Fn(&Settings) -> Arc<dyn CoreLauncher> + Send + Sync>;
/// `(controller_port, secret)` -> API client for that core instance.
pub type ApiFor = Box<dyn Fn(u16, String) -> Arc<dyn MihomoApi> + Send + Sync>;

#[derive(Serialize, Debug, specta::Type)]
#[serde(untagged)]
pub enum SettingsSaveError {
    Validation(Vec<ValidationError>),
    Operation(String),
}

const READY_TIMEOUT: Duration = Duration::from_secs(5);
const CONTROLLER_BUSY_PREFIX: &str = "控制端口";
/// Stands in for the real secret in configs that are only checked (`mihomo -t`).
const CHECK_SECRET: &str = "config-check";

#[derive(Serialize, Clone, Debug, PartialEq, specta::Type)]
pub struct RuntimeStatus {
    pub core: CoreState,
    pub last_apply: Option<ApplyOutcome>,
    pub health: Health,
    pub delays: DelayBook,
    pub traffic: Option<TrafficTick>,
    pub interfaces: Vec<NetInterface>,
    pub failed_listeners: Vec<FailedListener>,
    /// Listeners in the config mihomo currently holds (empty when no core
    /// runs or after a fail-closed apply). An active listener missing here is
    /// not in effect, whatever `last_apply` says.
    pub effective_listeners: Vec<ListenerId>,
    /// Problems found while loading persisted state (e.g. a corrupt state.json
    /// that was moved aside).
    pub startup_error: Option<String>,
    /// Last state.json / traffic.json save failure; cleared once that file
    /// saves successfully again.
    pub save_error: Option<String>,
    /// Login state per Tailscale proxy name (`手动/<名>`), from the log of
    /// the current mihomo process; empty while no core runs.
    pub tailscale: HashMap<String, TailscaleState>,
    /// Per Tailscale proxy name: why mihomo could not apply its `exit-node`
    /// (until that instance restarts).
    pub tailscale_exit_errors: HashMap<String, String>,
    /// Per Tailscale proxy name: the automatic retry (1..=EXIT_NODE_MAX_RETRIES)
    /// scheduled for its exit node failure. A failure without an entry here is final.
    pub tailscale_exit_retrying: HashMap<String, u32>,
    /// Why Routa last stopped the core itself; `None` once a start succeeds
    /// (and before any stop). `shutdown` leaves it as it was.
    pub core_stop_reason: Option<CoreStopReason>,
    /// The core exited unexpectedly and has not reached `Running` since: a
    /// `Starting` in this state is a recovery, not a normal start. Not set by
    /// a fail-closed stop (`core_stop_reason` says `FailClosed` instead).
    pub core_recovering: bool,
}

#[derive(Serialize, Clone, Debug, PartialEq, specta::Type)]
pub enum CoreStopReason {
    /// `stop_core`.
    User,
    /// An apply failed and even the empty config could not be loaded.
    FailClosed,
}

#[derive(Serialize, Clone, Debug, PartialEq, specta::Type)]
pub enum RuntimeEvent {
    Status(RuntimeStatus),
    Traffic(TrafficTick),
    /// A stop/reset correction, not a new rate sample for the history chart.
    TrafficSnapshot(TrafficTick),
}

/// One running mihomo instance.
struct CoreRun {
    handle: Box<dyn CoreHandleLike + Send>,
    secret: String,
    pid: u32,
    applier: Applier,
    started: Instant,
    traffic_stream: JoinHandle<()>,
    log_stream: JoinHandle<()>,
}

/// Tailscale login tracking for the current core process. `generation` is
/// bumped whenever a core is dropped so a log task that outlives its abort
/// cannot write states of an old process.
#[derive(Default)]
struct TailscaleTrack {
    generation: u64,
    tracker: TailscaleTracker,
    /// Automatic reloads spent per proxy name on an exit node mihomo failed
    /// to apply; reset by any reload that is not one of them.
    exit_retries: HashMap<String, u32>,
    /// Names waiting for the scheduled reload (status: "retrying n/MAX").
    exit_retrying: HashMap<String, u32>,
    /// The reload in flight is one of those retries.
    healing: bool,
}

/// mihomo v1.19.33 applies `exit-node` once, right after the instance starts,
/// and gives up when the netmap has not arrived yet (about half of the starts
/// here): the instance then has no exit node until it restarts. A reload
/// restarts it.
pub const EXIT_NODE_MAX_RETRIES: u32 = 5;
const EXIT_NODE_RETRY_DELAY: Duration = Duration::from_secs(2);

/// State guarded by the engine mutex.
struct Engine {
    model: Model,
    core: Option<CoreRun>,
    /// Whether the supervisor restarts the core when it is not running.
    supervise: bool,
    tun_exit: Option<ExitId>,
    alloc: PortAllocator,
}

/// What the samplers need without taking the engine lock.
#[derive(Default)]
struct Live {
    api: Option<Arc<dyn MihomoApi>>,
    /// Invalidates requests started before a core change or totals reset.
    traffic_generation: u64,
    name_to_id: HashMap<String, ListenerId>,
    /// Display names for routes in the last accepted config, retained on rejected saves.
    connection_nodes: HashMap<String, String>,
    stream_rate: Option<Rate>,
    last_delay_run: Option<i64>,
    delay_contexts: HashMap<String, u64>,
    /// Revisions also detect an A -> B -> A path change during a request.
    delay_versions: HashMap<String, u64>,
    delay_generation: u64,
}

struct Inner {
    store: Store,
    nics: Arc<dyn InterfaceSource>,
    launcher_for: LauncherFor,
    api_for: ApiFor,
    ports: Arc<dyn PortOwners>,
    events: mpsc::Sender<RuntimeEvent>,
    engine: tokio::sync::Mutex<Engine>,
    model: Mutex<Model>,
    status: Mutex<RuntimeStatus>,
    live: Mutex<Live>,
    traffic: Mutex<TrafficAggregator>,
    /// Lock order: `tailscale` before `status`.
    tailscale: Mutex<TailscaleTrack>,
    /// false when a corrupt file could not be moved aside: never overwrite it.
    persist_model: bool,
    persist_totals: bool,
    /// (state.json error, traffic.json error) feeding `RuntimeStatus::save_error`.
    save_errors: Mutex<(Option<String>, Option<String>)>,
    /// Set by `shutdown`: nothing may launch a core or start work afterwards.
    shut_down: AtomicBool,
    tasks: Mutex<Vec<JoinHandle<()>>>,
}

#[derive(Clone, Debug, serde::Serialize, specta::Type)]
pub struct ConnectionInfo {
    pub id: String,
    pub listener: String,
    pub process: String,
    pub target: String,
    pub node: String,
    pub protocol: String,
    pub uploaded: u64,
    pub downloaded: u64,
}

fn connection_node_names(model: &Model) -> HashMap<String, String> {
    use crate::model::{ExitKind, NodeRef, expand, manual_proxy_name};
    let mut names = HashMap::from([("DIRECT".into(), "直连".into())]);
    for n in &model.manual_nodes {
        names.insert(manual_proxy_name(n), n.name.clone());
    }
    for s in &model.subscriptions {
        for n in &s.nodes { names.insert(node_proxy_name(s, n), n.name.clone()); }
    }
    for e in &model.exits {
        match &e.kind {
            ExitKind::Direct => { names.insert(e.name.clone(), "直连".into()); }
            ExitKind::Node { node } => {
                let label = match node {
                    NodeRef::Subscription { name, .. } => Some(name.clone()),
                    NodeRef::Manual { manual_id } => model.manual_nodes.iter().find(|n| &n.id == manual_id).map(|n| n.name.clone()),
                };
                if let Some(label) = label { names.insert(e.name.clone(), label); }
            }
            ExitKind::Group { members, .. } => {
                for n in expand(model, members).nodes {
                    if let Some(crate::model::Resolved::Copyable { base_name, .. }) = crate::model::resolve(model, &n) {
                        if let Some(label) = names.get(&base_name).cloned() {
                            names.insert(crate::config_gen::member_proxy_name(e, &base_name), label);
                        }
                    }
                }
            }
        }
    }
    names
}

fn connection_info(c: crate::core::api::Conn, names: &HashMap<String, String>) -> ConnectionInfo {
    let route = c.chains.first().map(String::as_str).unwrap_or("");
    let node = names.get(route).map(String::as_str).unwrap_or(route);
    let host = if c.metadata.host.is_empty() { &c.metadata.destination_ip } else { &c.metadata.host };
    let target = if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]:{}", c.metadata.destination_port)
    } else {
        format!("{host}:{}", c.metadata.destination_port)
    };
    ConnectionInfo {
        id: c.id,
        listener: c.metadata.inbound_name,
        process: if c.metadata.process_path.is_empty() { c.metadata.process } else { c.metadata.process_path },
        target,
        node: node.to_owned(),
        protocol: c.metadata.network.to_uppercase(),
        uploaded: c.upload,
        downloaded: c.download,
    }
}

pub struct Runtime {
    inner: Arc<Inner>,
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn random_secret() -> String {
    let mut buf = [0u8; 32];
    getrandom::fill(&mut buf).expect("OS random source");
    buf.iter().map(|b| format!("{b:02x}")).collect()
}

/// Loads a persisted file; on failure moves it to `<name>.corrupt-<unix ts>`
/// and starts from the default. Returns `(value, may_save)`.
fn load_or_quarantine<T: Default>(loaded: io::Result<T>, path: &Path, errors: &mut Vec<String>) -> (T, bool) {
    let e = match loaded {
        Ok(v) => return (v, true),
        Err(e) => e,
    };
    let mut target = path.as_os_str().to_owned();
    target.push(format!(".corrupt-{}", now_unix()));
    match std::fs::rename(path, &target) {
        Ok(()) => {
            errors.push(format!(
                "{} 无法读取（{e}），已备份为 {} 并使用默认值",
                path.display(),
                Path::new(&target).display()
            ));
            (T::default(), true)
        }
        Err(re) => {
            errors.push(format!(
                "{} 无法读取（{e}），备份失败（{re}），本次运行不会保存以免覆盖",
                path.display()
            ));
            (T::default(), false)
        }
    }
}

fn uncut(o: &ApplyOutcome) -> usize {
    match o {
        ApplyOutcome::Reloaded { uncut, .. } | ApplyOutcome::FailedClosed { uncut, .. } => *uncut,
        _ => 0,
    }
}

/// Returned by every call made after `shutdown`.
const SHUT_DOWN: &str = "程序已退出，操作未执行";

fn core_not_running() -> ApplyOutcome {
    ApplyOutcome::NotRunning {
        error: "mihomo 未运行，配置将在内核启动后生效".into(),
    }
}

impl Inner {
    /// Called with `live` held: corrections and samples must be published in
    /// the same order in which they update the traffic state.
    fn publish_traffic(&self, tick: TrafficTick, sampled: bool) {
        self.status.lock().unwrap().traffic = Some(tick.clone());
        let event = if sampled {
            RuntimeEvent::Traffic(tick)
        } else {
            RuntimeEvent::TrafficSnapshot(tick)
        };
        let _ = self.events.try_send(event);
    }

    fn record_traffic_sample(&self, generation: u64, snap: &ConnectionsSnapshot, elapsed: Duration) {
        let live = self.live.lock().unwrap();
        if live.api.is_none() || live.traffic_generation != generation {
            return;
        }
        let mut tick = self.traffic.lock().unwrap().ingest(snap, &live.name_to_id, elapsed);
        if let Some(rate) = &live.stream_rate {
            tick.total_rate = rate.clone();
        }
        self.publish_traffic(tick, true);
    }

    fn emit_status(&self) {
        let s = self.status.lock().unwrap().clone();
        // Status events are full snapshots: dropping one when the consumer
        // lags loses nothing the next one will not carry.
        let _ = self.events.try_send(RuntimeEvent::Status(s));
    }

    fn set_core_state(&self, state: CoreState) {
        set_core(&mut self.status.lock().unwrap(), state);
        self.emit_status();
    }

    fn is_shut_down(&self) -> bool {
        self.shut_down.load(Ordering::SeqCst)
    }

    /// Records one file's save result in `status.save_error`; emits on change.
    fn note_save(&self, model_file: bool, result: io::Result<()>) {
        let new = result.err().map(|e| {
            if model_file {
                format!("保存配置失败：{e}，修改在重启后会丢失")
            } else {
                format!("保存流量统计失败：{e}")
            }
        });
        if let Some(msg) = &new {
            eprintln!("runtime: {msg}");
        }
        let changed = {
            let mut errs = self.save_errors.lock().unwrap();
            let slot = if model_file { &mut errs.0 } else { &mut errs.1 };
            let changed = *slot != new;
            *slot = new;
            let joined: Vec<&str> = [&errs.0, &errs.1].into_iter().flatten().map(String::as_str).collect();
            self.status.lock().unwrap().save_error = (!joined.is_empty()).then(|| joined.join("\n"));
            changed
        };
        if changed {
            self.emit_status();
        }
    }

    fn save_model(&self, m: &Model) {
        if self.persist_model {
            self.note_save(true, self.store.save_model(m));
        }
    }

    fn save_totals(&self) {
        if !self.persist_totals {
            return;
        }
        let totals = self.traffic.lock().unwrap().totals.clone();
        self.note_save(false, self.store.save_totals(&totals));
    }

    /// Replaces the model (already validated) and persists it.
    fn commit_model(&self, eng: &mut Engine, m: Model) {
        self.save_model(&m);
        *self.model.lock().unwrap() = m.clone();
        eng.model = m;
        self.sync_delay_contexts(&eng.model);
    }

    fn update_delay_contexts(&self, next: HashMap<String, u64>) {
        let changed = {
            let mut live = self.live.lock().unwrap();
            let changed: std::collections::HashSet<_> = live.delay_contexts.keys().chain(next.keys())
                .filter(|name| live.delay_contexts.get(*name) != next.get(*name)).cloned().collect();
            for name in &changed { *live.delay_versions.entry(name.clone()).or_default() += 1; }
            live.delay_contexts = next;
            changed
        };
        let mut st = self.status.lock().unwrap();
        for (name, result) in &mut st.delays.0 { if changed.contains(name) { result.stale = true; } }
    }

    fn sync_delay_contexts(&self, model: &Model) {
        let nics = self.nics.list();
        self.update_delay_contexts(delay_fingerprints(model, &evaluate(model, &nics), &nics));
    }

    /// Executables a process rule must not name: Routa itself, the bundled
    /// mihomo, the custom core (`settings.core_path`) if one is set, and the
    /// process routing service exe (bundled helper and installed copy).
    fn forbidden_exes(&self, settings: &Settings) -> Vec<PathBuf> {
        let bundled = Settings {
            core_path: None,
            ..settings.clone()
        };
        let mut out = Vec::new();
        match std::env::current_exe() {
            Ok(p) => out.push(p),
            Err(e) => eprintln!("runtime: cannot resolve Routa's own exe for process rule checks: {e}"),
        }
        out.extend((self.launcher_for)(&bundled).core_binary());
        out.extend(
            settings
                .core_path
                .as_deref()
                .filter(|p| !p.trim().is_empty())
                .map(PathBuf::from),
        );
        out.extend(crate::procroute::elevate::component_exes());
        out
    }

    /// Writes `yaml` to a temp file inside the core workdir, runs `mihomo -t`
    /// on it (with the launcher for `settings`) and deletes the file.
    async fn check_yaml(&self, settings: &Settings, yaml: &str) -> Result<(), CheckError> {
        let path = self
            .store
            .core_workdir()
            .join(format!("check-{}.yaml", &random_secret()[..16]));
        std::fs::write(&path, yaml).map_err(|e| CheckError::Unavailable(format!("写入校验配置失败: {e}")))?;
        let res = (self.launcher_for)(settings).check_config(&path).await;
        if let Err(e) = std::fs::remove_file(&path) {
            eprintln!("runtime: removing {} failed: {e}", path.display());
        }
        res
    }

    /// The config `m` would generate must pass `mihomo -t`, or mihomo rejects
    /// the whole config on every later apply. A check that cannot run (e.g.
    /// the core binary is missing) is skipped: nothing would start anyway.
    async fn check_model(&self, eng: &mut Engine, m: &Model) -> Result<(), Vec<ValidationError>> {
        let nics = self.nics.list();
        let health = evaluate(m, &nics);
        let fetch = fetch_listener_ports(m, &mut eng.alloc);
        let generated = generate_for_capture(
            &GenInput {
                model: m,
                health: &health,
                secret: CHECK_SECRET,
                fetch_listeners: &fetch,
                nics: &nics,
            },
            eng.tun_exit.as_ref(),
        );
        match self.check_yaml(&m.settings, &generated.yaml).await {
            Ok(()) => Ok(()),
            Err(CheckError::Rejected(message)) => Err(vec![ValidationError::CoreRejected { message }]),
            Err(CheckError::Unavailable(e)) => {
                eprintln!("runtime: config check skipped: {e}");
                Ok(())
            }
        }
    }

    /// Fetched nodes are accepted only if mihomo accepts them (one invalid
    /// proxy makes it reject the entire config). Skipped when the check
    /// cannot run.
    async fn check_nodes(&self, sub: &Subscription, nodes: Vec<Node>) -> Result<Vec<Node>, FetchError> {
        let settings = self.model.lock().unwrap().settings.clone();
        let yaml = nodes_check_config(settings.controller_port, CHECK_SECRET, sub, &nodes);
        match self.check_yaml(&settings, &yaml).await {
            Ok(()) => Ok(nodes),
            Err(CheckError::Rejected(msg)) => Err(FetchError::CoreRejected(msg)),
            Err(CheckError::Unavailable(e)) => {
                eprintln!("runtime: subscription node check skipped: {e}");
                Ok(nodes)
            }
        }
    }

    /// Stops and forgets the current core instance (no-op without one).
    async fn drop_core(&self, eng: &mut Engine) {
        if let Some(mut run) = eng.core.take() {
            run.traffic_stream.abort();
            run.log_stream.abort();
            if let Err(e) = run.handle.stop().await {
                eprintln!("runtime: stopping mihomo failed: {e}");
            }
        }
        {
            let mut live = self.live.lock().unwrap();
            live.api = None;
            live.delay_generation += 1;
            live.traffic_generation += 1;
            live.name_to_id.clear();
            live.connection_nodes.clear();
            live.stream_rate = None;
            let totals = self.traffic.lock().unwrap().totals.clone();
            self.publish_traffic(
                TrafficTick {
                    totals,
                    ..TrafficTick::default()
                },
                false,
            );
        }
        {
            // A new process logs every login state afresh.
            let mut ts = self.tailscale.lock().unwrap();
            ts.generation += 1;
            ts.tracker.reset();
            ts.exit_retrying.clear();
            let mut st = self.status.lock().unwrap();
            st.effective_listeners.clear();
            for result in st.delays.0.values_mut() { result.stale = true; }
            st.tailscale.clear();
            st.tailscale_exit_errors.clear();
            st.tailscale_exit_retrying.clear();
        }
    }

    /// Feeds one core log line from the log task of `generation`; updates the
    /// status and emits when a login state changed.
    fn on_core_log(self: &Arc<Self>, generation: u64, line: &str) {
        let heal = {
            let mut ts = self.tailscale.lock().unwrap();
            if ts.generation != generation {
                return;
            }
            let before = ts.tracker.exit_errors();
            if !ts.tracker.on_log_line(line) {
                return;
            }
            let mut heal = false;
            for name in ts.tracker.exit_errors().into_keys().filter(|n| !before.contains_key(n)) {
                let used = ts.exit_retries.get(&name).copied().unwrap_or(0);
                if used < EXIT_NODE_MAX_RETRIES {
                    ts.exit_retries.insert(name.clone(), used + 1);
                    ts.exit_retrying.insert(name, used + 1);
                    heal = true;
                }
            }
            sync_tailscale(&ts, &mut self.status.lock().unwrap());
            heal
        };
        self.emit_status();
        if heal {
            self.schedule_exit_node_retry();
        }
    }

    /// Reloads the config shortly (once for every failure reported meanwhile).
    fn schedule_exit_node_retry(self: &Arc<Self>) {
        let me = self.clone();
        tokio::spawn(async move {
            tokio::time::sleep(EXIT_NODE_RETRY_DELAY).await;
            let mut eng = me.engine.lock().await;
            {
                let mut ts = me.tailscale.lock().unwrap();
                if ts.exit_retrying.is_empty() {
                    return; // An earlier scheduled retry covered it.
                }
                ts.exit_retrying.clear();
                ts.healing = true;
                sync_tailscale(&ts, &mut me.status.lock().unwrap());
            }
            if !me.is_shut_down() && eng.core.is_some() {
                eprintln!("runtime: a Tailscale exit node was not applied; reloading to restart the instance");
                me.apply(&mut eng).await;
            }
            me.tailscale.lock().unwrap().healing = false;
            me.emit_status();
        });
    }

    /// Forgets automatic retries, e.g. before a reload the user asked for.
    fn reset_exit_node_retries(&self) {
        let mut ts = self.tailscale.lock().unwrap();
        if ts.healing {
            return;
        }
        ts.exit_retries.clear();
        ts.exit_retrying.clear();
        sync_tailscale(&ts, &mut self.status.lock().unwrap());
    }

    /// Forgets login states of Tailscale nodes no longer in the config.
    fn retain_tailscale(&self, names: &[String]) {
        let mut ts = self.tailscale.lock().unwrap();
        if ts.tracker.retain(names) {
            sync_tailscale(&ts, &mut self.status.lock().unwrap());
        }
    }

    /// (Re)starts mihomo: fresh secret, empty config, launch, wait for the
    /// API, fresh Applier, then apply the latest model.
    async fn start_core(self: &Arc<Self>, eng: &mut Engine) -> Result<(), String> {
        if self.is_shut_down() {
            return Err(SHUT_DOWN.into());
        }
        self.drop_core(eng).await;
        self.set_core_state(CoreState::Starting);
        let res = self.launch_core(eng).await;
        match &res {
            Ok(()) => {
                {
                    let mut st = self.status.lock().unwrap();
                    set_core(&mut st, CoreState::Running);
                    st.core_stop_reason = None;
                    if st
                        .startup_error
                        .as_deref()
                        .is_some_and(|e| e.starts_with(CONTROLLER_BUSY_PREFIX))
                    {
                        st.startup_error = None;
                    }
                }
                // A fresh core has no connections, so `uncut` here is moot.
                self.apply_once(eng).await;
            }
            Err(e) => {
                eprintln!("runtime: {e}");
                self.set_core_state(CoreState::Crashed { exit_code: None });
            }
        }
        res
    }

    async fn launch_core(self: &Arc<Self>, eng: &mut Engine) -> Result<(), String> {
        let secret = random_secret();
        let port = eng.model.settings.controller_port;
        let config = self.store.config_path();
        std::fs::write(&config, empty_config(port, &secret)).map_err(|e| format!("写入 mihomo 配置失败: {e}"))?;
        let launcher = (self.launcher_for)(&eng.model.settings);
        let mut handle = launcher
            .launch(&config)
            .await
            .map_err(|e| format!("启动 mihomo 失败: {e}"))?;
        let pid = handle.pid();
        // Never talk to a controller we do not own (it could be another
        // app's mihomo without a secret): wait until our child holds the port.
        let deadline = Instant::now() + READY_TIMEOUT;
        loop {
            match self.ports.listening_owners(&[port]).get(&port) {
                Some(&owner) if owner != pid => {
                    let _ = handle.stop().await;
                    let msg = format!("{CONTROLLER_BUSY_PREFIX} {port} 已被其他程序占用，请在设置中更换");
                    self.status.lock().unwrap().startup_error = Some(msg.clone());
                    return Err(msg);
                }
                Some(_) => break,
                None if Instant::now() >= deadline => {
                    let _ = handle.stop().await;
                    return Err(format!("mihomo 未在 5 秒内监听控制端口 {port}"));
                }
                None => tokio::time::sleep(Duration::from_millis(50)).await,
            }
        }
        let api = (self.api_for)(port, secret.clone());
        let left = deadline
            .saturating_duration_since(Instant::now())
            .max(Duration::from_secs(1));
        if let Err(e) = wait_ready(api.as_ref(), left).await {
            let _ = handle.stop().await;
            return Err(format!("mihomo 未在 5 秒内就绪: {e}"));
        }
        let traffic_stream = tokio::spawn(tasks::traffic_stream(self.clone(), api.clone()));
        // Subscribed before the first apply, so the Tailscale prewarm's log
        // lines are seen.
        let generation = self.tailscale.lock().unwrap().generation;
        let log_stream = tokio::spawn(tasks::log_stream(self.clone(), api.clone(), generation));
        self.live.lock().unwrap().api = Some(api.clone());
        eng.core = Some(CoreRun {
            handle,
            secret,
            pid,
            applier: Applier::new(api, self.ports.clone(), config),
            started: Instant::now(),
            traffic_stream,
            log_stream,
        });
        Ok(())
    }

    /// evaluate → generate → apply (when a core runs) → update status → emit.
    /// `None` when no core is running (health and interfaces still update).
    /// After a successful reload the Tailscale instances are prewarmed.
    async fn apply_once(self: &Arc<Self>, eng: &mut Engine) -> Option<ApplyOutcome> {
        let nics = self.nics.list();
        let Engine {
            model,
            core,
            alloc,
            tun_exit,
            ..
        } = eng;
        let health = evaluate(model, &nics);
        self.update_delay_contexts(delay_fingerprints(model, &health, &nics));
        let fetch = fetch_listener_ports(model, alloc);
        let tailscales = emitted_tailscale_names(model, &nics);
        let outcome = match core.as_mut() {
            Some(run) => {
                let generated = generate_for_capture(
                    &GenInput {
                        model,
                        health: &health,
                        secret: &run.secret,
                        fetch_listeners: &fetch,
                        nics: &nics,
                    },
                    tun_exit.as_ref(),
                );
                let plan = generated.plan.clone();
                let CoreRun {
                    handle,
                    applier,
                    secret,
                    pid,
                    ..
                } = run;
                let out = applier
                    .apply(generated, model.settings.controller_port, secret, *pid, async || {
                        if let Err(e) = handle.stop().await {
                            eprintln!("runtime: stopping mihomo failed: {e}");
                        }
                    })
                    .await;
                if !matches!(out, ApplyOutcome::Reloaded { .. }) {
                    eprintln!("runtime: applying configuration failed: {out:?}");
                }
                self.status.lock().unwrap().effective_listeners = applier.effective_listeners();
                // Keep login states only for nodes in the config mihomo now holds.
                match &out {
                    ApplyOutcome::Reloaded { .. } => self.retain_tailscale(&tailscales),
                    ApplyOutcome::FailedClosed { .. } => self.retain_tailscale(&[]),
                    _ => {}
                }
                let mut live = self.live.lock().unwrap();
                match &out {
                    ApplyOutcome::Reloaded { .. } => {
                        live.connection_nodes = connection_node_names(model);
                        live.name_to_id = plan
                            .listeners
                            .iter()
                            .map(|(name, l)| (name.clone(), l.id.clone()))
                            .collect();
                    }
                    ApplyOutcome::FailedClosed { .. } | ApplyOutcome::FailedCoreStopped { .. } => {
                        live.name_to_id.clear();
                        live.connection_nodes.clear();
                    }
                    ApplyOutcome::FailedKeptPrevious { .. } | ApplyOutcome::NotRunning { .. } => {}
                }
                Some(out)
            }
            None => None,
        };

        if let Some(ApplyOutcome::FailedCoreStopped { .. }) = &outcome {
            // The applier already stopped it; the supervisor restarts it
            // (empty config first) with backoff.
            self.drop_core(eng).await;
            let mut st = self.status.lock().unwrap();
            set_core(&mut st, CoreState::Crashed { exit_code: None });
            // Routa stopped it on purpose (fail closed), it did not crash:
            // the reason, not a recovery, describes it until a start reaches
            // Running, including the supervisor's Starting retries.
            st.core_recovering = false;
            st.core_stop_reason = Some(CoreStopReason::FailClosed);
        }
        {
            let mut st = self.status.lock().unwrap();
            st.health = health;
            st.interfaces = nics;
            match &outcome {
                Some(ApplyOutcome::Reloaded { failed_listeners, .. }) => {
                    st.failed_listeners = failed_listeners.clone();
                }
                Some(ApplyOutcome::FailedKeptPrevious { .. }) => {}
                _ => st.failed_listeners.clear(),
            }
            if let Some(o) = &outcome {
                st.last_apply = Some(o.clone());
            }
        }
        self.emit_status();
        if matches!(outcome, Some(ApplyOutcome::Reloaded { .. })) {
            self.prewarm_tailscales(tailscales);
        }
        outcome
    }

    /// mihomo recreates each Tailscale instance lazily on its first dial
    /// after a reload, so new connections would fail for a few seconds: dial
    /// them now with a delay call. Fire-and-forget; the results (typically a
    /// cold-start timeout) are not measurements, so they stay out of the DelayBook.
    fn prewarm_tailscales(self: &Arc<Self>, names: Vec<String>) {
        if names.is_empty() {
            return;
        }
        let api = self.live.lock().unwrap().api.clone();
        let Some(api) = api.filter(|_| !self.is_shut_down()) else {
            return;
        };
        let url = self.model.lock().unwrap().settings.delay_test_url.clone();
        tokio::spawn(async move {
            test_many(api.as_ref(), &names, &url, DEFAULT_CONCURRENCY, DEFAULT_TIMEOUT_MS).await;
        });
    }

    /// `apply_once`, escalating to a core restart when connections that must
    /// not continue could not be cut (a restart drops every connection).
    async fn apply(self: &Arc<Self>, eng: &mut Engine) -> ApplyOutcome {
        self.reset_exit_node_retries();
        match self.apply_once(eng).await {
            Some(out) => {
                if uncut(&out) > 0 {
                    eprintln!(
                        "runtime: {} connection(s) could not be cut; restarting mihomo",
                        uncut(&out)
                    );
                    let _ = self.start_core(eng).await;
                }
                out
            }
            None => core_not_running(),
        }
    }

    /// Fetches one subscription (fallback through its exit's internal
    /// listener when that exit is healthy), records the result, saves, and
    /// re-applies when the nodes changed. `None` if the subscription is gone.
    async fn update_one_subscription(self: &Arc<Self>, id: &SubscriptionId) -> Option<FetchResult> {
        if self.is_shut_down() {
            return Some(FetchResult::Failed {
                reason: SHUT_DOWN.into(),
            });
        }
        let (sub, port) = {
            let mut eng = self.engine.lock().await;
            let sub = eng.model.subscriptions.iter().find(|s| &s.id == id)?.clone();
            let core_pid = eng.core.as_ref().map(|c| c.pid);
            let Engine { model, alloc, .. } = &mut *eng;
            let fetch = fetch_listener_ports(model, alloc);
            let st = self.status.lock().unwrap();
            let port = sub.fallback_exit.as_ref().and_then(|exit| {
                if sub.fallback_listener.as_ref().is_some_and(|id| !st.effective_listeners.contains(id)) { return None; }
                let ok = st.health.exits.get(exit).is_some_and(|h| h.status == ExitStatus::Ok);
                let pid = core_pid.filter(|_| ok)?;
                let port = fetch.iter().find(|(e, _)| e == exit).map(|(_, p)| *p)?;
                // The URL carries the subscription token: only hand it to a
                // port our own mihomo listens on.
                let owned = self.ports.listening_owners(&[port]).get(&port) == Some(&pid);
                if !owned {
                    eprintln!("runtime: fetch port {port} is not held by mihomo; skipping the fallback");
                }
                owned.then_some(port)
            });
            (sub, port)
        };

        let result = match update_subscription(&sub, port).await {
            Ok(p) => self.check_nodes(&sub, p.nodes).await.map(|nodes| Parsed {
                nodes,
                skipped_tailscale: p.skipped_tailscale,
            }),
            Err(e) => Err(e),
        };

        let mut eng = self.engine.lock().await;
        if self.is_shut_down() {
            return Some(FetchResult::Failed {
                reason: SHUT_DOWN.into(),
            });
        }
        let mut m = eng.model.clone();
        let target = m.subscriptions.iter_mut().find(|s| &s.id == id)?;
        if target.url != sub.url {
            // Edited while the fetch was in flight: the result belongs to the
            // old URL. Record nothing (not even last_updated).
            return Some(FetchResult::Failed {
                reason: "订阅地址已更改，本次结果已丢弃".into(),
            });
        }
        let old_nodes = target.nodes.clone();
        record_result(target, result, now_unix());
        let fetch_result = target.last_result.clone();
        let changed = target.nodes != old_nodes;
        // New nodes may invalidate a Select exit's selection.
        normalize(&mut m);
        self.commit_model(&mut eng, m);
        if changed {
            self.apply(&mut eng).await;
        } else {
            self.emit_status();
        }
        fetch_result
    }

    async fn update_due_subscriptions(self: &Arc<Self>) {
        let subs = self.model.lock().unwrap().subscriptions.clone();
        for id in crate::scheduler::due_subscriptions(&subs, now_unix()) {
            self.update_one_subscription(&id).await;
        }
    }

    async fn refresh_interfaces(self: &Arc<Self>) -> bool {
        let mut eng = self.engine.lock().await;
        if self.is_shut_down() {
            return false;
        }
        let current = self.nics.list();
        if self.status.lock().unwrap().interfaces == current {
            return false;
        }
        self.apply(&mut eng).await;
        true
    }

    /// Every node proxy plus every healthy exit.
    fn delay_targets(&self) -> Vec<String> {
        let m = self.model.lock().unwrap().clone();
        let st = self.status.lock().unwrap();
        let mut names: Vec<String> = m
            .subscriptions
            .iter()
            .flat_map(|s| s.nodes.iter().map(move |n| node_proxy_name(s, n)))
            .collect();
        names.extend(
            m.exits
                .iter()
                .filter(|e| st.health.exits.get(&e.id).is_some_and(|h| h.status == ExitStatus::Ok))
                .map(|e| e.name.clone()),
        );
        names
    }

    async fn test_delays(&self, names: Vec<String>) -> Vec<(String, DelayResult)> {
        // Take the snapshot after any in-flight apply, but release the engine
        // while measuring so node switches remain responsive.
        let (api, versions, generation, url, applied) = {
            let eng = self.engine.lock().await;
            self.sync_delay_contexts(&eng.model);
            let live = self.live.lock().unwrap();
            let applied = matches!(self.status.lock().unwrap().last_apply, Some(ApplyOutcome::Reloaded { .. }));
            (live.api.clone(), live.delay_versions.clone(), live.delay_generation, eng.model.settings.delay_test_url.clone(), applied)
        };
        let Some(api) = api.filter(|_| !self.is_shut_down()) else {
            // No core to test through: report failures, keep the DelayBook.
            return names
                .into_iter()
                .map(|n| (n, DelayResult::failed(DelayError::NoCore, now_unix())))
                .collect();
        };
        let mut results = test_many(api.as_ref(), &names, &url, DEFAULT_CONCURRENCY, DEFAULT_TIMEOUT_MS).await;
        {
            let eng = self.engine.lock().await;
            self.sync_delay_contexts(&eng.model);
            let live = self.live.lock().unwrap();
            let mut st = self.status.lock().unwrap();
            let same_core = live.delay_generation == generation && live.api.as_ref().is_some_and(|now| Arc::ptr_eq(now, &api));
            let current_applied = matches!(st.last_apply, Some(ApplyOutcome::Reloaded { .. }));
            for (name, r) in &mut results {
                let same_path = live.delay_versions.get(name).copied().unwrap_or(0) == versions.get(name).copied().unwrap_or(0);
                r.stale = !same_core || !same_path || !applied || !current_applied;
                // A late old-path result must never replace a fresh current one.
                if !r.stale { st.delays.record(name.clone(), r.clone()); }
            }
        }
        self.emit_status();
        results
    }
}

/// Sets the core state and tracks `core_recovering`: an unexpected stop
/// (`Crashed`) starts a recovery, reaching `Running` or a stop ends it.
fn set_core(st: &mut RuntimeStatus, state: CoreState) {
    match state {
        CoreState::Crashed { .. } => st.core_recovering = true,
        CoreState::Running | CoreState::Stopped => st.core_recovering = false,
        CoreState::Starting => {}
    }
    st.core = state;
}

fn sync_tailscale(ts: &TailscaleTrack, st: &mut RuntimeStatus) {
    st.tailscale = ts.tracker.states();
    st.tailscale_exit_errors = ts.tracker.exit_errors();
    st.tailscale_exit_retrying = ts.exit_retrying.clone();
}

impl Runtime {
    /// Loads persisted state. A corrupt state.json / traffic.json is moved
    /// aside (`*.corrupt-<unix ts>`), the default is used and the problem is
    /// reported in `RuntimeStatus::startup_error`. Does not start anything.
    pub fn new(
        store: Store,
        nics: Arc<dyn InterfaceSource>,
        launcher_for: LauncherFor,
        api_for: ApiFor,
        ports: Arc<dyn PortOwners>,
        events: mpsc::Sender<RuntimeEvent>,
    ) -> Self {
        let mut errors = Vec::new();
        let (model, persist_model) = load_or_quarantine(store.load_model(), &store.state_path(), &mut errors);
        let (totals, persist_totals): (TrafficTotals, bool) =
            load_or_quarantine(store.load_totals(), &store.traffic_path(), &mut errors);
        let interfaces = nics.list();
        let status = RuntimeStatus {
            core: CoreState::Stopped,
            last_apply: None,
            health: evaluate(&model, &interfaces),
            delays: DelayBook::default(),
            traffic: None,
            interfaces,
            failed_listeners: Vec::new(),
            effective_listeners: Vec::new(),
            startup_error: (!errors.is_empty()).then(|| errors.join("\n")),
            save_error: None,
            tailscale: HashMap::new(),
            tailscale_exit_errors: HashMap::new(),
            tailscale_exit_retrying: HashMap::new(),
            core_stop_reason: None,
            core_recovering: false,
        };
        let inner = Inner {
            store,
            nics,
            launcher_for,
            api_for,
            ports,
            events,
            engine: tokio::sync::Mutex::new(Engine {
                model: model.clone(),
                core: None,
                supervise: false,
                tun_exit: None,
                alloc: PortAllocator::new(),
            }),
            model: Mutex::new(model),
            status: Mutex::new(status),
            live: Mutex::new(Live::default()),
            traffic: Mutex::new(TrafficAggregator::new(totals)),
            tailscale: Mutex::new(TailscaleTrack::default()),
            persist_model,
            persist_totals,
            save_errors: Mutex::new((None, None)),
            shut_down: AtomicBool::new(false),
            tasks: Mutex::new(Vec::new()),
        };
        Self { inner: Arc::new(inner) }
    }

    /// Starts mihomo (empty config → latest model) and the background tasks.
    /// The tasks run even if the first start fails; the supervisor retries.
    /// After `shutdown` it does nothing and returns an error.
    pub async fn start(&self) -> anyhow::Result<()> {
        let res = {
            let mut eng = self.inner.engine.lock().await;
            if self.inner.is_shut_down() {
                anyhow::bail!(SHUT_DOWN);
            }
            eng.supervise = true;
            if eng.core.is_none() {
                self.inner.start_core(&mut eng).await
            } else {
                Ok(())
            }
        };
        {
            let mut t = self.inner.tasks.lock().unwrap();
            if t.is_empty() {
                *t = tasks::spawn_all(&self.inner);
            }
        }
        res.map_err(anyhow::Error::msg)
    }

    /// Stops the background tasks, persists traffic totals, stops mihomo.
    /// Final: later calls do not launch a core or start work.
    pub async fn shutdown(&self) {
        self.inner.shut_down.store(true, Ordering::SeqCst);
        for t in self.inner.tasks.lock().unwrap().drain(..) {
            t.abort();
        }
        let mut eng = self.inner.engine.lock().await;
        eng.supervise = false;
        self.inner.save_totals();
        self.inner.drop_core(&mut eng).await;
        self.inner.set_core_state(CoreState::Stopped);
    }

    /// Settings preferences do not belong to the core config. Persist them
    /// without a reload; only changed core parameters take the apply path.
    /// Keep OS autostart and persistence failures observable to the caller.
    pub async fn save_settings(
        &self,
        mut settings: Settings,
        set_autostart: impl Fn(bool) -> Result<(), String>,
    ) -> Result<ApplyOutcome, SettingsSaveError> {
        let mut eng = self.inner.engine.lock().await;
        if self.inner.is_shut_down() {
            return Err(SettingsSaveError::Operation("程序正在退出，设置未保存".into()));
        }
        settings.core_path = None;
        let old_autostart = eng.model.settings.autostart;
        let autostart_changed = settings.autostart != old_autostart;
        let core_changed = settings.controller_port != eng.model.settings.controller_port
            || settings.core_path != eng.model.settings.core_path;
        let mut model = eng.model.clone();
        model.settings = settings;
        normalize(&mut model);
        validate(&model).map_err(SettingsSaveError::Validation)?;
        if core_changed {
            self.inner.check_model(&mut eng, &model).await.map_err(SettingsSaveError::Validation)?;
        }
        if autostart_changed {
            set_autostart(model.settings.autostart).map_err(|_| {
                SettingsSaveError::Operation("开机自启未能保存，请重试".into())
            })?;
        }
        if self.inner.persist_model {
            let result = self.inner.store.save_model(&model);
            let failed = result.is_err();
            self.inner.note_save(true, result);
            if failed {
                let rollback_failed = autostart_changed && set_autostart(old_autostart).is_err();
                return Err(SettingsSaveError::Operation(if rollback_failed {
                    "设置未能保存，开机自启未能恢复，请检查系统启动项".into()
                } else {
                    "设置未能写入磁盘，请重试".into()
                }));
            }
        }
        *self.inner.model.lock().unwrap() = model.clone();
        eng.model = model;
        self.inner.sync_delay_contexts(&eng.model);
        if !core_changed {
            self.inner.emit_status();
            return Ok(ApplyOutcome::Reloaded { cut: 0, uncut: 0, failed_listeners: vec![] });
        }
        if eng.supervise {
            return Ok(match self.inner.start_core(&mut eng).await {
                Ok(()) => self.inner.status.lock().unwrap().last_apply.clone().unwrap_or_else(core_not_running),
                Err(error) => ApplyOutcome::NotRunning { error },
            });
        }
        Ok(self.inner.apply(&mut eng).await)
    }

    /// clone → f → validate → `mihomo -t` → save → evaluate → generate → apply.
    /// A changed controller port or core path restarts a supervised core
    /// instead (the restart applies the new model).
    /// Validation errors (including a config mihomo rejects) leave everything
    /// untouched. After `shutdown` nothing
    /// is saved or applied and `FailedCoreStopped` is returned.
    pub async fn mutate(&self, f: impl FnOnce(&mut Model)) -> Result<ApplyOutcome, Vec<ValidationError>> {
        self.mutate_with(|m| {
            f(m);
            Ok(())
        })
        .await
        .map(|(outcome, ())| outcome)
    }

    /// Like `mutate`, but `f` may reject the change (nothing is saved) and
    /// hand a value back, computed against the model it actually edited.
    pub async fn mutate_with<R>(
        &self,
        f: impl FnOnce(&mut Model) -> Result<R, Vec<ValidationError>>,
    ) -> Result<(ApplyOutcome, R), Vec<ValidationError>> {
        let mut eng = self.inner.engine.lock().await;
        if self.inner.is_shut_down() {
            return Ok((
                ApplyOutcome::FailedCoreStopped {
                    error: SHUT_DOWN.into(),
                },
                f(&mut eng.model.clone())?,
            ));
        }
        let mut m = eng.model.clone();
        let r = f(&mut m)?;
        normalize(&mut m);
        validate(&m)?;
        self.inner.check_model(&mut eng, &m).await?;
        // A new controller port or binary only takes effect on a fresh core.
        let (old, new) = (&eng.model.settings, &m.settings);
        let restart = eng.supervise && (old.controller_port != new.controller_port || old.core_path != new.core_path);
        self.inner.commit_model(&mut eng, m);
        if restart {
            let outcome = match self.inner.start_core(&mut eng).await {
                Ok(()) => self
                    .inner
                    .status
                    .lock()
                    .unwrap()
                    .last_apply
                    .clone()
                    .unwrap_or_else(core_not_running),
                Err(error) => ApplyOutcome::NotRunning { error },
            };
            return Ok((outcome, r));
        }
        Ok((self.inner.apply(&mut eng).await, r))
    }

    /// clone → f → normalize → validate (the rules must not name Routa or a
    /// mihomo binary) → save → update the model → emit. Never reloads or
    /// restarts mihomo: process routing is not part of its config. Validation
    /// errors leave everything untouched. After `shutdown` nothing is saved
    /// and `Ok(())` is returned (the app is exiting; there is nobody to tell).
    pub async fn mutate_process_routing(
        &self,
        f: impl FnOnce(&mut ProcessRouting),
    ) -> Result<(), Vec<ValidationError>> {
        let mut eng = self.inner.engine.lock().await;
        if self.inner.is_shut_down() {
            return Ok(());
        }
        let mut m = eng.model.clone();
        f(&mut m.process_routing);
        normalize(&mut m);
        let mut errors = validate_forbidden_exes(&m.process_routing, &self.inner.forbidden_exes(&m.settings))
            .err()
            .unwrap_or_default();
        errors.extend(validate(&m).err().unwrap_or_default());
        if !errors.is_empty() {
            return Err(errors);
        }
        self.inner.commit_model(&mut eng, m);
        self.inner.emit_status();
        Ok(())
    }

    /// Re-lists adapters; on any change re-evaluates and re-applies.
    pub async fn refresh_interfaces(&self) -> bool {
        self.inner.refresh_interfaces().await
    }

    /// (Re)starts mihomo with the latest model and re-enables supervision.
    pub async fn restart_core(&self) {
        let mut eng = self.inner.engine.lock().await;
        if self.inner.is_shut_down() {
            return;
        }
        eng.supervise = true;
        // Asked for by the user: not a recovery and no longer a stop,
        // whatever happened before.
        {
            let mut st = self.inner.status.lock().unwrap();
            st.core_recovering = false;
            st.core_stop_reason = None;
        }
        let _ = self.inner.start_core(&mut eng).await;
    }

    /// Stops mihomo; the supervisor leaves it stopped until `restart_core`/`start`.
    pub async fn stop_core(&self) {
        let mut eng = self.inner.engine.lock().await;
        eng.supervise = false;
        self.inner.drop_core(&mut eng).await;
        {
            let mut st = self.inner.status.lock().unwrap();
            st.failed_listeners.clear();
            st.core_stop_reason = Some(CoreStopReason::User);
        }
        self.inner.set_core_state(CoreState::Stopped);
    }

    /// Change TUN under the same engine lock used by config edits and restarts.
    pub async fn set_tun_exit(&self, target: Option<ExitId>) -> Result<(), String> {
        let mut eng = self.inner.engine.lock().await;
        if self.inner.is_shut_down() {
            return Err(SHUT_DOWN.into());
        }
        if let Some(id) = &target {
            if !self
                .status()
                .health
                .exits
                .get(id)
                .is_some_and(|h| h.status == crate::model::health::ExitStatus::Ok)
            {
                return Err("TUN 对应的出口不可用".into());
            }
            if eng.core.is_none() {
                return Err("请先启动内核".into());
            }
        }
        let old = eng.tun_exit.clone();
        if target.is_none() && eng.core.is_none() {
            eng.tun_exit = None;
            return Ok(());
        }
        eng.tun_exit = target;
        let outcome = self.inner.apply(&mut eng).await;
        if !matches!(&outcome, ApplyOutcome::Reloaded { failed_listeners, .. } if failed_listeners.is_empty()) {
            eng.tun_exit = old;
            let _ = self.inner.apply(&mut eng).await;
            return Err(format!("TUN 设置未能生效：{outcome:?}"));
        }
        Ok(())
    }

    pub fn tun_adapter_ready(&self) -> bool {
        self.inner
            .nics
            .list()
            .iter()
            .any(|n| n.is_up && (n.friendly_name == "RoutaTun" || n.mihomo_name == "RoutaTun"))
    }

    pub fn status(&self) -> RuntimeStatus {
        self.inner.status.lock().unwrap().clone()
    }

    pub async fn connection_details(&self) -> Result<Vec<ConnectionInfo>, String> {
        let (api, names) = {
            let live = self.inner.live.lock().unwrap();
            (live.api.clone().ok_or("内核未运行")?, live.connection_nodes.clone())
        };
        let snapshot = api.connections().await.map_err(|e| e.to_string())?;
        {
            let live = self.inner.live.lock().unwrap();
            if !live.api.as_ref().is_some_and(|now| Arc::ptr_eq(now, &api)) || live.connection_nodes != names {
                return Err("代理配置已更新，请稍后重试".into());
            }
        }
        Ok(snapshot
            .connections
            .into_iter()
            .map(|c| connection_info(c, &names))
            .collect())
    }

    /// Directory holding mihomo's working files (config.yaml, mihomo.log).
    pub fn core_workdir(&self) -> std::path::PathBuf {
        self.inner.store.core_workdir()
    }

    pub fn model(&self) -> Model {
        self.inner.model.lock().unwrap().clone()
    }

    /// Fetches one subscription now (regardless of its schedule).
    pub async fn refresh_subscription(&self, id: SubscriptionId) -> FetchResult {
        self.inner
            .update_one_subscription(&id)
            .await
            .unwrap_or_else(|| FetchResult::Failed {
                reason: "订阅不存在".into(),
            })
    }

    /// One pass of the subscription scheduler (normally run every 60 s).
    pub async fn update_due_subscriptions(&self) {
        self.inner.update_due_subscriptions().await;
    }

    /// Tests `names` through mihomo and records the results in `status().delays`.
    /// Without a running core every result is `ms: None` and nothing is recorded.
    pub async fn test_delays(&self, names: Vec<String>) -> Vec<(String, DelayResult)> {
        self.inner.test_delays(names).await
    }

    /// Reloads the config so every Tailscale instance restarts and applies
    /// its exit node again, with a fresh budget of automatic retries.
    pub async fn retry_tailscale_exit_nodes(&self) -> ApplyOutcome {
        let mut eng = self.inner.engine.lock().await;
        if self.inner.is_shut_down() {
            return ApplyOutcome::FailedCoreStopped {
                error: SHUT_DOWN.into(),
            };
        }
        self.inner.apply(&mut eng).await
    }

    /// Exit name -> the member mihomo currently uses for it. Names mihomo
    /// does not know (or that are not groups) are omitted; empty without a core.
    pub async fn group_now(&self, names: Vec<String>) -> HashMap<String, String> {
        let api = self.inner.live.lock().unwrap().api.clone();
        let Some(api) = api.filter(|_| !self.inner.is_shut_down()) else {
            return HashMap::new();
        };
        let mut out = HashMap::new();
        for name in names {
            match api.proxy_now(&name).await {
                Ok(Some(now)) => {
                    out.insert(name, now);
                }
                Ok(None) => {}
                Err(e) => eprintln!("runtime: reading {name} current member failed: {e}"),
            }
        }
        out
    }

    /// Zeroes the cumulative traffic totals and persists that.
    pub async fn reset_traffic(&self) {
        {
            let mut live = self.inner.live.lock().unwrap();
            live.traffic_generation += 1;
            let totals = {
                let mut t = self.inner.traffic.lock().unwrap();
                t.reset();
                t.totals.clone()
            };
            let mut tick = self.inner.status.lock().unwrap().traffic.clone().unwrap_or_default();
            tick.totals = totals;
            self.inner.publish_traffic(tick, false);
        }
        self.inner.save_totals();
        self.inner.emit_status();
    }
}

#[cfg(test)]
mod tests;

//! Tauri-independent orchestration: model + store + health + config generation
//! + fail-closed apply + mihomo supervision + schedulers + NIC watch.
//!
//! Locking: every model mutation, Store save, apply and core (re)start runs
//! under the one async `engine` mutex. `status`, the model mirror and the
//! live api/plan are std mutexes that are only held for copies, never across
//! an await.

mod tasks;

use std::collections::HashMap;
use std::io;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::Instant;

use crate::config_gen::{empty_config, generate, node_proxy_name, nodes_check_config, GenInput};
use crate::core::api::MihomoApi;
use crate::core::apply::{ApplyOutcome, Applier, FailedListener};
use crate::core::ports::PortOwners;
use crate::core::process::{wait_ready, CheckError, CoreHandleLike, CoreLauncher, CoreState};
use crate::model::health::{evaluate, ExitStatus, Health};
use crate::model::{
    validate, FetchResult, ListenerId, Model, Node, Settings, Subscription, SubscriptionId, ValidationError,
};
use crate::netif::{InterfaceSource, NetInterface};
use crate::stats::delay::{test_many, DelayBook, DelayResult, DEFAULT_CONCURRENCY, DEFAULT_TIMEOUT_MS};
use crate::stats::traffic::{Rate, TrafficAggregator, TrafficTick};
use crate::store::{Store, TrafficTotals};
use crate::subscription::fetch::{
    fetch_listener_ports, record_result, update_subscription, FetchError, PortAllocator,
};

/// Builds the launcher for the current settings (core_path or sidecar); called
/// at every (re)start so a changed `core_path` takes effect on restart.
pub type LauncherFor = Arc<dyn Fn(&Settings) -> Arc<dyn CoreLauncher> + Send + Sync>;
/// `(controller_port, secret)` -> API client for that core instance.
pub type ApiFor = Box<dyn Fn(u16, String) -> Arc<dyn MihomoApi> + Send + Sync>;

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
    /// Problems found while loading persisted state (e.g. a corrupt state.json
    /// that was moved aside).
    pub startup_error: Option<String>,
    /// Last state.json / traffic.json save failure; cleared once that file
    /// saves successfully again.
    pub save_error: Option<String>,
}

#[derive(Serialize, Clone, Debug, PartialEq, specta::Type)]
pub enum RuntimeEvent {
    Status(RuntimeStatus),
    Traffic(TrafficTick),
}

/// One running mihomo instance.
struct CoreRun {
    handle: Box<dyn CoreHandleLike + Send>,
    secret: String,
    pid: u32,
    applier: Applier,
    started: Instant,
    traffic_stream: JoinHandle<()>,
}

/// State guarded by the engine mutex.
struct Engine {
    model: Model,
    core: Option<CoreRun>,
    /// Whether the supervisor restarts the core when it is not running.
    supervise: bool,
    alloc: PortAllocator,
}

/// What the samplers need without taking the engine lock.
#[derive(Default)]
struct Live {
    api: Option<Arc<dyn MihomoApi>>,
    name_to_id: HashMap<String, ListenerId>,
    stream_rate: Option<Rate>,
    last_delay_run: Option<i64>,
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
    /// false when a corrupt file could not be moved aside: never overwrite it.
    persist_model: bool,
    persist_totals: bool,
    /// (state.json error, traffic.json error) feeding `RuntimeStatus::save_error`.
    save_errors: Mutex<(Option<String>, Option<String>)>,
    /// Set by `shutdown`: nothing may launch a core or start work afterwards.
    shut_down: AtomicBool,
    tasks: Mutex<Vec<JoinHandle<()>>>,
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
fn load_or_quarantine<T: Default>(
    loaded: io::Result<T>,
    path: &Path,
    errors: &mut Vec<String>,
) -> (T, bool) {
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
    ApplyOutcome::NotRunning { error: "mihomo 未运行，配置将在内核启动后生效".into() }
}

impl Inner {
    fn emit_status(&self) {
        let s = self.status.lock().unwrap().clone();
        // Status events are full snapshots: dropping one when the consumer
        // lags loses nothing the next one will not carry.
        let _ = self.events.try_send(RuntimeEvent::Status(s));
    }

    fn set_core_state(&self, state: CoreState) {
        self.status.lock().unwrap().core = state;
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
    }

    /// Writes `yaml` to a temp file inside the core workdir, runs `mihomo -t`
    /// on it (with the launcher for `settings`) and deletes the file.
    async fn check_yaml(&self, settings: &Settings, yaml: &str) -> Result<(), CheckError> {
        let path = self.store.core_workdir().join(format!("check-{}.yaml", &random_secret()[..16]));
        std::fs::write(&path, yaml)
            .map_err(|e| CheckError::Unavailable(format!("写入校验配置失败: {e}")))?;
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
        let generated = generate(&GenInput {
            model: m,
            health: &health,
            secret: CHECK_SECRET,
            fetch_listeners: &fetch,
        });
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
            if let Err(e) = run.handle.stop().await {
                eprintln!("runtime: stopping mihomo failed: {e}");
            }
        }
        let mut live = self.live.lock().unwrap();
        live.api = None;
        live.name_to_id.clear();
        live.stream_rate = None;
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
                    st.core = CoreState::Running;
                    if st.startup_error.as_deref().is_some_and(|e| e.starts_with(CONTROLLER_BUSY_PREFIX)) {
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
        std::fs::write(&config, empty_config(port, &secret))
            .map_err(|e| format!("写入 mihomo 配置失败: {e}"))?;
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
        let left = deadline.saturating_duration_since(Instant::now()).max(Duration::from_secs(1));
        if let Err(e) = wait_ready(api.as_ref(), left).await {
            let _ = handle.stop().await;
            return Err(format!("mihomo 未在 5 秒内就绪: {e}"));
        }
        let traffic_stream = tokio::spawn(tasks::traffic_stream(self.clone(), api.clone()));
        self.live.lock().unwrap().api = Some(api.clone());
        eng.core = Some(CoreRun {
            handle,
            secret,
            pid,
            applier: Applier::new(api, self.ports.clone(), config),
            started: Instant::now(),
            traffic_stream,
        });
        Ok(())
    }

    /// evaluate → generate → apply (when a core runs) → update status → emit.
    /// `None` when no core is running (health and interfaces still update).
    async fn apply_once(&self, eng: &mut Engine) -> Option<ApplyOutcome> {
        let nics = self.nics.list();
        let Engine { model, core, alloc, .. } = eng;
        let health = evaluate(model, &nics);
        let fetch = fetch_listener_ports(model, alloc);
        let outcome = match core.as_mut() {
            Some(run) => {
                let generated = generate(&GenInput {
                    model,
                    health: &health,
                    secret: &run.secret,
                    fetch_listeners: &fetch,
                });
                let plan = generated.plan.clone();
                let CoreRun { handle, applier, secret, pid, .. } = run;
                let out = applier
                    .apply(generated, model.settings.controller_port, secret, *pid, async || {
                        if let Err(e) = handle.stop().await {
                            eprintln!("runtime: stopping mihomo failed: {e}");
                        }
                    })
                    .await;
                let mut live = self.live.lock().unwrap();
                match &out {
                    ApplyOutcome::Reloaded { .. } => {
                        live.name_to_id = plan
                            .listeners
                            .iter()
                            .map(|(name, l)| (name.clone(), l.id.clone()))
                            .collect();
                    }
                    ApplyOutcome::FailedClosed { .. } | ApplyOutcome::FailedCoreStopped { .. } => {
                        live.name_to_id.clear();
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
            self.status.lock().unwrap().core = CoreState::Crashed { exit_code: None };
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
        outcome
    }

    /// `apply_once`, escalating to a core restart when connections that must
    /// not continue could not be cut (a restart drops every connection).
    async fn apply(self: &Arc<Self>, eng: &mut Engine) -> ApplyOutcome {
        match self.apply_once(eng).await {
            Some(out) => {
                if uncut(&out) > 0 {
                    eprintln!("runtime: {} connection(s) could not be cut; restarting mihomo", uncut(&out));
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
            return Some(FetchResult::Failed { reason: SHUT_DOWN.into() });
        }
        let (sub, port) = {
            let mut eng = self.engine.lock().await;
            let sub = eng.model.subscriptions.iter().find(|s| &s.id == id)?.clone();
            let core_up = eng.core.is_some();
            let Engine { model, alloc, .. } = &mut *eng;
            let fetch = fetch_listener_ports(model, alloc);
            let st = self.status.lock().unwrap();
            let port = sub.fallback_exit.as_ref().and_then(|exit| {
                let ok = st.health.exits.get(exit).is_some_and(|h| h.status == ExitStatus::Ok);
                (core_up && ok)
                    .then(|| fetch.iter().find(|(e, _)| e == exit).map(|(_, p)| *p))
                    .flatten()
            });
            (sub, port)
        };

        let result = match update_subscription(&sub, port).await {
            Ok(nodes) => self.check_nodes(&sub, nodes).await,
            Err(e) => Err(e),
        };

        let mut eng = self.engine.lock().await;
        if self.is_shut_down() {
            return Some(FetchResult::Failed { reason: SHUT_DOWN.into() });
        }
        let mut m = eng.model.clone();
        let target = m.subscriptions.iter_mut().find(|s| &s.id == id)?;
        if target.url != sub.url {
            // Edited while the fetch was in flight: the result belongs to the
            // old URL. Record nothing (not even last_updated).
            return Some(FetchResult::Failed { reason: "订阅地址已更改，本次结果已丢弃".into() });
        }
        let old_nodes = target.nodes.clone();
        record_result(target, result, now_unix());
        let fetch_result = target.last_result.clone();
        let changed = target.nodes != old_nodes;
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
        let api = self.live.lock().unwrap().api.clone();
        let Some(api) = api.filter(|_| !self.is_shut_down()) else {
            // No core to test through: report failures, keep the DelayBook.
            return names
                .into_iter()
                .map(|n| (n, DelayResult { ms: None, tested_at: now_unix() }))
                .collect();
        };
        let url = self.model.lock().unwrap().settings.delay_test_url.clone();
        let results = test_many(api.as_ref(), &names, &url, DEFAULT_CONCURRENCY, DEFAULT_TIMEOUT_MS).await;
        {
            let mut st = self.status.lock().unwrap();
            for (name, r) in &results {
                st.delays.record(name.clone(), r.clone());
            }
        }
        self.emit_status();
        results
    }
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
            startup_error: (!errors.is_empty()).then(|| errors.join("\n")),
            save_error: None,
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
                alloc: PortAllocator::new(),
            }),
            model: Mutex::new(model),
            status: Mutex::new(status),
            live: Mutex::new(Live::default()),
            traffic: Mutex::new(TrafficAggregator::new(totals)),
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
            return Ok((ApplyOutcome::FailedCoreStopped { error: SHUT_DOWN.into() }, f(&mut eng.model.clone())?));
        }
        let mut m = eng.model.clone();
        let r = f(&mut m)?;
        validate(&m)?;
        self.inner.check_model(&mut eng, &m).await?;
        // A new controller port or binary only takes effect on a fresh core.
        let (old, new) = (&eng.model.settings, &m.settings);
        let restart = eng.supervise && (old.controller_port != new.controller_port || old.core_path != new.core_path);
        self.inner.commit_model(&mut eng, m);
        if restart {
            let outcome = match self.inner.start_core(&mut eng).await {
                Ok(()) => self.inner.status.lock().unwrap().last_apply.clone().unwrap_or_else(core_not_running),
                Err(error) => ApplyOutcome::NotRunning { error },
            };
            return Ok((outcome, r));
        }
        Ok((self.inner.apply(&mut eng).await, r))
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
        let _ = self.inner.start_core(&mut eng).await;
    }

    /// Stops mihomo; the supervisor leaves it stopped until `restart_core`/`start`.
    pub async fn stop_core(&self) {
        let mut eng = self.inner.engine.lock().await;
        eng.supervise = false;
        self.inner.drop_core(&mut eng).await;
        self.inner.status.lock().unwrap().failed_listeners.clear();
        self.inner.set_core_state(CoreState::Stopped);
    }

    pub fn status(&self) -> RuntimeStatus {
        self.inner.status.lock().unwrap().clone()
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
            .unwrap_or_else(|| FetchResult::Failed { reason: "订阅不存在".into() })
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

    /// Zeroes the cumulative traffic totals and persists that.
    pub async fn reset_traffic(&self) {
        let totals = {
            let mut t = self.inner.traffic.lock().unwrap();
            t.reset();
            t.totals.clone()
        };
        self.inner.save_totals();
        if let Some(tick) = self.inner.status.lock().unwrap().traffic.as_mut() {
            tick.totals = totals;
        }
        self.inner.emit_status();
    }
}

#[cfg(test)]
mod tests;

//! 与进程分流服务（RoutaProcSvc）的连接：命名管道客户端、同步任务、按规则的速率统计。
//!
//! 同步任务独占唯一的管道会话（服务同一时刻只接受一个会话）：运行状态变化时、以及每秒一次，
//! 用 [`decide`](super::decide) 算出期望状态并下发（与上次成功下发的相同则不发），每秒取一次
//! 连接列表和服务状态，向前端推送 `procroute-status` 与 `procroute-flows`。
//! 退出时的 Goodbye、通过服务清理残留阻断，都经由命令通道交给这个任务在同一会话里发送。

use std::collections::HashMap;
use std::io;
use std::os::windows::io::AsRawHandle;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock, Mutex, OnceLock};
use std::time::Duration;

use async_trait::async_trait;
use routa_procsvc_proto::{
    decode, decode_len, encode, DesiredState, Flow, Request, Response, SvcStatus, PIPE_NAME,
    PROTO_VERSION, SERVICE_NAME,
};
use serde::Serialize;
use tauri::async_runtime::JoinHandle;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::windows::named_pipe::{ClientOptions, NamedPipeClient};
use tokio::sync::{mpsc, oneshot, Notify};
use tokio::time::{interval, timeout, Instant, MissedTickBehavior};
use windows_sys::Win32::Foundation::{
    ERROR_ACCESS_DENIED, ERROR_FILE_NOT_FOUND, ERROR_PIPE_BUSY, ERROR_SERVICE_ALREADY_RUNNING,
    ERROR_SERVICE_DOES_NOT_EXIST,
};
use windows_sys::Win32::System::Pipes::GetNamedPipeServerProcessId;
use windows_sys::Win32::System::Services::{
    CloseServiceHandle, OpenSCManagerW, OpenServiceW, QueryServiceStatusEx, StartServiceW, SC_HANDLE,
    SC_MANAGER_CONNECT, SC_STATUS_PROCESS_INFO, SERVICE_QUERY_STATUS, SERVICE_RUNNING, SERVICE_START,
    SERVICE_STATUS_PROCESS,
};

use super::decide::{self, decide};
use crate::model::ProcessRuleId;
use crate::runtime::Runtime;

/// 定时同步（取连接列表、服务状态）的间隔。
const TICK: Duration = Duration::from_secs(1);
/// 未连上时重试连接的间隔。
const RECONNECT: Duration = Duration::from_secs(2);
/// 单次请求（写请求 + 读回复）的上限；超时视为连接出错。
const CALL_TIMEOUT: Duration = Duration::from_secs(5);
/// 退出时等 Goodbye 送达的上限。
const GOODBYE_TIMEOUT: Duration = Duration::from_secs(1);

#[derive(Serialize, Clone, Debug, PartialEq, specta::Type)]
pub enum ComponentState {
    NotInstalled,
    Stopped,
    Running,
    /// 服务的协议版本与本程序不一致，需要重新安装组件；此时不下发任何状态。
    NeedsUpdate,
    /// 管道只放行注册组件的用户：组件归其他 Windows 用户所有。
    OwnedByOtherUser,
    Error { message: String },
}

#[derive(Serialize, Clone, Debug, PartialEq, specta::Type)]
pub struct ProcRouteStatus {
    pub component: ComponentState,
    /// 当前启用规则的服务确认结果；未确认时为 Pending / Failed / Unknown。
    pub actions: HashMap<ProcessRuleId, RuleView>,
    pub last_interruption: Option<InterruptionView>,
    pub conflicts: Vec<String>,
    /// 最近一次应用错误；由用户主动重试。
    pub apply_errors: Vec<String>,
    /// 连接列表达到服务的上限、被截断了（界面标明"仅显示最近 400 条"）。
    pub truncated_flows: bool,
}

#[derive(Serialize, Clone, Debug, PartialEq, specta::Type)]
pub struct RuleView {
    /// "Proxy" | "Direct" | "Block" | "Pending" | "Failed" | "Unknown"
    pub action: String,
    /// 回落、执行失败或无法确认状态的原因（中文）。
    pub reason: Option<String>,
    pub up_bps: u64,
    pub down_bps: u64,
}

#[derive(Serialize, Clone, Debug, PartialEq, specta::Type)]
pub struct InterruptionView {
    pub at_unix: i64,
    pub reason: String,
}

impl ProcRouteStatus {
    fn initial() -> Self {
        Self {
            component: ComponentState::Stopped,
            actions: HashMap::new(),
            last_interruption: None,
            conflicts: Vec::new(),
            apply_errors: Vec::new(),
            truncated_flows: false,
        }
    }
}

fn error(message: impl Into<String>) -> ComponentState {
    ComponentState::Error { message: message.into() }
}

// ---------------------------------------------------------------- 连接

/// 与服务的一条会话。`Err` 只表示连接本身出了问题（读写失败、超时、帧不合法）。
#[async_trait]
pub trait SvcConn: Send {
    async fn call(&mut self, req: Request) -> Result<Response, String>;
}

struct PipeConn(NamedPipeClient);

impl PipeConn {
    async fn exchange(&mut self, req: &Request) -> Result<Response, String> {
        let io_err = |e: io::Error| format!("管道读写失败：{e}");
        self.0.write_all(&encode(req)?).await.map_err(io_err)?;
        let mut header = [0u8; 4];
        self.0.read_exact(&mut header).await.map_err(io_err)?;
        let mut body = vec![0u8; decode_len(header)?];
        self.0.read_exact(&mut body).await.map_err(io_err)?;
        decode(&body)
    }
}

#[async_trait]
impl SvcConn for PipeConn {
    async fn call(&mut self, req: Request) -> Result<Response, String> {
        timeout(CALL_TIMEOUT, self.exchange(&req))
            .await
            .unwrap_or_else(|_| Err("分流服务响应超时".into()))
    }
}

/// 打开 [`PIPE_NAME`]、校验管道服务端确实是 RoutaProcSvc 服务进程，再握手。
/// 失败时返回此刻的组件状态：管道不存在时按服务是否存在、是否在运行给出
/// `NotInstalled` / `Stopped`；拒绝访问为 `OwnedByOtherUser`；版本不一致为 `NeedsUpdate`。
pub async fn connect() -> Result<Box<dyn SvcConn>, ComponentState> {
    handshake(Box::new(open_pipe()?)).await
}

/// 发送 Hello；服务拒绝（版本不一致）时为 `NeedsUpdate`。
async fn handshake(mut conn: Box<dyn SvcConn>) -> Result<Box<dyn SvcConn>, ComponentState> {
    match conn.call(Request::Hello { version: PROTO_VERSION }).await {
        Ok(Response::Hello { accepted: true, .. }) => Ok(conn),
        Ok(Response::Hello { accepted: false, .. }) => Err(ComponentState::NeedsUpdate),
        Ok(other) => Err(error(format!("分流服务握手回复异常：{other:?}"))),
        Err(e) => Err(error(e)),
    }
}

fn open_pipe() -> Result<PipeConn, ComponentState> {
    match ClientOptions::new().open(PIPE_NAME) {
        Ok(pipe) => {
            verify_server(&pipe)?;
            Ok(PipeConn(pipe))
        }
        Err(e) => Err(match e.raw_os_error().map(|c| c as u32) {
            Some(ERROR_FILE_NOT_FOUND) => match query_service() {
                Ok(None) => ComponentState::NotInstalled,
                Ok(Some(s)) if s.dwCurrentState == SERVICE_RUNNING => error("分流服务没有响应"),
                Ok(Some(_)) => ComponentState::Stopped,
                Err(e) => error(e),
            },
            Some(ERROR_ACCESS_DENIED) => ComponentState::OwnedByOtherUser,
            Some(ERROR_PIPE_BUSY) => error("分流服务正被另一个连接占用"),
            _ => error(format!("无法连接分流服务：{e}")),
        }),
    }
}

/// 防抢注：管道的服务端进程必须是 RoutaProcSvc 服务此刻的进程。
fn verify_server(pipe: &NamedPipeClient) -> Result<(), ComponentState> {
    let mut server_pid = 0u32;
    // SAFETY: 句柄在 pipe 存活期间有效；server_pid 是可写的局部变量。
    let ok = unsafe { GetNamedPipeServerProcessId(pipe.as_raw_handle(), &mut server_pid) };
    let service_pid = query_service().ok().flatten().map(|s| s.dwProcessId);
    if ok == 0 || server_pid == 0 || service_pid != Some(server_pid) {
        return Err(error("管道服务身份校验失败"));
    }
    Ok(())
}

/// 服务控制管理器的句柄，离开作用域时关闭。
struct ScHandle(SC_HANDLE);

impl Drop for ScHandle {
    fn drop(&mut self) {
        // SAFETY: self.0 由 OpenSCManagerW / OpenServiceW 返回且非空。
        unsafe { CloseServiceHandle(self.0) };
    }
}

/// 以 `access` 打开 RoutaProcSvc；服务不存在时为 `Ok(None)`。
fn open_service(access: u32) -> Result<Option<ScHandle>, String> {
    let name: Vec<u16> = SERVICE_NAME.encode_utf16().chain(Some(0)).collect();
    // SAFETY: name 以 NUL 结尾；返回的句柄交给 ScHandle 关闭。
    unsafe {
        let scm = OpenSCManagerW(std::ptr::null(), std::ptr::null(), SC_MANAGER_CONNECT);
        if scm.is_null() {
            return Err(format!("无法打开服务管理器：{}", io::Error::last_os_error()));
        }
        let scm = ScHandle(scm);
        let svc = OpenServiceW(scm.0, name.as_ptr(), access);
        if svc.is_null() {
            let e = io::Error::last_os_error();
            if e.raw_os_error() == Some(ERROR_SERVICE_DOES_NOT_EXIST as i32) {
                return Ok(None);
            }
            return Err(format!("无法查询分流服务：{e}"));
        }
        Ok(Some(ScHandle(svc)))
    }
}

/// 服务的状态和进程 id；服务不存在时为 `Ok(None)`。
fn query_service() -> Result<Option<SERVICE_STATUS_PROCESS>, String> {
    let Some(svc) = open_service(SERVICE_QUERY_STATUS)? else {
        return Ok(None);
    };
    // SAFETY: status 是可写的局部变量，大小如实传入。
    unsafe {
        let mut status: SERVICE_STATUS_PROCESS = std::mem::zeroed();
        let mut needed = 0u32;
        let ok = QueryServiceStatusEx(
            svc.0,
            SC_STATUS_PROCESS_INFO,
            &mut status as *mut _ as *mut u8,
            std::mem::size_of::<SERVICE_STATUS_PROCESS>() as u32,
            &mut needed,
        );
        if ok == 0 {
            return Err(format!("无法查询分流服务：{}", io::Error::last_os_error()));
        }
        Ok(Some(status))
    }
}

/// 启动已停止的服务（安装时授予了组件所有者 SERVICE_START）。
fn start_service() -> Result<(), String> {
    let Some(svc) = open_service(SERVICE_START)? else {
        return Err("分流服务不存在".into());
    };
    // SAFETY: 句柄有效；不传服务参数。
    let ok = unsafe { StartServiceW(svc.0, 0, std::ptr::null()) };
    let e = io::Error::last_os_error();
    if ok == 0 && e.raw_os_error() != Some(ERROR_SERVICE_ALREADY_RUNNING as i32) {
        return Err(format!("无法启动分流服务：{e}"));
    }
    Ok(())
}

// ---------------------------------------------------------------- 下发

/// 相同配置每次只尝试一次；失败由用户主动重试。
#[derive(Default)]
pub struct Syncer {
    /// 服务接受的配置；部分失败也有确认结果，须结合 SvcStatus 展示。
    last_sent: Option<DesiredState>,
    attempted: Option<DesiredState>,
    manual_retry: bool,
    errors: Vec<String>,
}

impl Syncer {
    pub async fn push(&mut self, conn: &mut dyn SvcConn, s: DesiredState) -> Result<bool, String> {
        if self.attempted.as_ref() == Some(&s) && !self.manual_retry {
            return Ok(false);
        }
        let retry = std::mem::take(&mut self.manual_retry);
        self.attempted = Some(s.clone());
        let req = if retry { Request::RetryApply(s.clone()) } else { Request::Apply(s.clone()) };
        let response = match conn.call(req).await {
            Ok(r) => r,
            Err(e) => {
                self.last_sent = None;
                self.errors = vec![format!("规则应用结果未知：{e}；请手动重试")];
                return Err(e);
            }
        };
        match response {
            Response::Ok => {
                self.errors.clear();
                self.last_sent = Some(s);
            }
            Response::Applied { errors } => {
                self.errors = errors;
                self.last_sent = Some(s);
            }
            Response::Error(e) => {
                self.errors = vec![e];
                self.last_sent = None;
            }
            other => {
                self.last_sent = None;
                self.errors = vec!["服务回复异常，无法确认规则是否生效；请手动重试".into()];
                return Err(format!("Apply 的回复不合协议：{other:?}"));
            }
        }
        Ok(true)
    }

    pub fn errors(&self) -> &[String] { &self.errors }
}

// ---------------------------------------------------------------- 速率

/// 区分一条连接：服务给的连接没有 id，用（进程, 目标, 端口, 协议）近似。同一进程到同一目标的
/// 并发连接会落在同一个键上，按和计算，差值仍然正确；其中一条关闭时这一秒少算（不会算成负数）。
type FlowKey = (u32, String, u16, bool);

/// 由相邻两次连接列表算出每条规则的速率。
#[derive(Default)]
struct Rates {
    prev: Option<(Instant, HashMap<FlowKey, (u64, u64)>)>,
}

impl Rates {
    /// 规则 id → (上传, 下载) 字节/秒。上一次样本里没有的连接按 0 起算；
    /// 第一次样本（刚连上）只作基准，没有速率，免得把已有连接的累计流量算成一秒的速率。
    fn update(&mut self, flows: &[Flow], now: Instant) -> HashMap<String, (u64, u64)> {
        let mut cur: HashMap<FlowKey, (u64, u64)> = HashMap::new();
        let mut rule_of: HashMap<FlowKey, &str> = HashMap::new();
        for f in flows {
            let Some(rule) = f.rule_id.as_deref() else { continue };
            let key = (f.pid, f.host.clone(), f.port, f.udp);
            let e = cur.entry(key.clone()).or_default();
            e.0 += f.sent;
            e.1 += f.received;
            rule_of.insert(key, rule);
        }
        let mut rates = HashMap::new();
        if let Some((then, prev)) = &self.prev {
            let secs = now.saturating_duration_since(*then).as_secs_f64().max(0.001);
            let mut bytes: HashMap<&str, (u64, u64)> = HashMap::new();
            for (key, (sent, received)) in &cur {
                let (ps, pr) = prev.get(key).copied().unwrap_or_default();
                let e = bytes.entry(rule_of[key]).or_default();
                e.0 += sent.saturating_sub(ps);
                e.1 += received.saturating_sub(pr);
            }
            for (rule, (up, down)) in bytes {
                let per_sec = |b: u64| (b as f64 / secs).round() as u64;
                rates.insert(rule.to_string(), (per_sec(up), per_sec(down)));
            }
        }
        self.prev = Some((now, cur));
        rates
    }
}

// ---------------------------------------------------------------- 会话状态

/// 同步任务里与服务相关的全部状态（不含定时和 Tauri）。
struct Link {
    conn: Option<Box<dyn SvcConn>>,
    registered: bool,
    syncer: Syncer,
    rates: Rates,
    component: ComponentState,
    svc: Option<SvcStatus>,
    bps: HashMap<String, (u64, u64)>,
    flows: Vec<Flow>,
    /// 退出已开始：不再同步。
    stopped: bool,
}

impl Default for Link {
    fn default() -> Self {
        Self {
            conn: None,
            registered: false,
            syncer: Syncer::default(),
            rates: Rates::default(),
            component: ProcRouteStatus::initial().component,
            svc: None,
            bps: HashMap::new(),
            flows: Vec::new(),
            stopped: false,
        }
    }
}

/// 一轮同步：仅发送变化的配置；应用后立即取服务确认，定时只查询连接和状态。
async fn round(
    conn: &mut dyn SvcConn,
    syncer: &mut Syncer,
    desired: DesiredState,
    poll: bool,
) -> Result<Option<(Vec<Flow>, SvcStatus)>, String> {
    let pushed = syncer.push(conn, desired).await?;
    if !poll && !pushed {
        return Ok(None);
    }
    let flows = if poll { match conn.call(Request::GetFlows).await? {
        Response::Flows(f) => f,
        other => return Err(format!("GetFlows 的回复不合协议：{other:?}")),
    }} else { Vec::new() };
    let status = match conn.call(Request::GetStatus).await? {
        Response::Status(s) => s,
        other => return Err(format!("GetStatus 的回复不合协议：{other:?}")),
    };
    Ok(Some((flows, status)))
}

impl Link {
    fn connected(&self) -> bool {
        self.conn.is_some()
    }

    /// 连接结果：成功则开始新会话（全量重发：服务可能重启过），失败则记下组件状态。
    fn attach(&mut self, res: Result<Box<dyn SvcConn>, ComponentState>) {
        match res {
            Ok(conn) => {
                self.conn = Some(conn);
                self.registered = false;
                // 已失败的配置在重新连接后也不自动重发。
                if self.syncer.errors.is_empty() { self.syncer = Syncer::default(); }
                self.svc = None;
                self.rates = Rates::default();
                self.component = ComponentState::Running;
            }
            Err(state) => self.disconnect(state),
        }
    }

    fn disconnect(&mut self, state: ComponentState) {
        self.conn = None;
        self.registered = false;
        self.component = state;
        self.svc = None;
        self.bps.clear();
        self.flows.clear();
    }

    /// 退出开始后：不再下发任何状态（只剩 Goodbye）。
    fn stop(&mut self) {
        self.stopped = true;
    }

    async fn sync(&mut self, desired: DesiredState, poll: bool) {
        if self.stopped {
            return;
        }
        let Some(conn) = self.conn.as_deref_mut() else { return };
        if !self.registered {
            match conn.call(Request::Attach { routa_pid: desired.routa_pid }).await {
                Ok(Response::Ok) => self.registered = true,
                result => {
                    self.disconnect(error(format!("登记分流管理会话失败：{result:?}")));
                    return;
                }
            }
        }
        match round(conn, &mut self.syncer, desired, poll).await {
            Ok(Some((flows, status))) => {
                if poll {
                    self.bps = self.rates.update(&flows, Instant::now());
                    self.flows = flows;
                }
                self.svc = Some(status);
            }
            Ok(None) => {}
            Err(e) => self.disconnect(error(format!("与分流服务的连接中断：{e}"))),
        }
    }

    async fn goodbye(&mut self) {
        if let Some(conn) = self.conn.as_deref_mut() {
            let _ = conn.call(Request::Goodbye).await;
        }
    }

    /// 通过服务清理残留阻断；没连上时为 `None`。
    async fn cleanup(&mut self) -> Option<Result<(), String>> {
        let conn = self.conn.as_deref_mut()?;
        Some(match conn.call(Request::CleanupFilters).await {
            Ok(Response::Ok) => Ok(()),
            Ok(Response::Applied { errors }) => Err(errors.join("；")),
            Ok(Response::Error(e)) => Err(e),
            Ok(other) => Err(format!("CleanupFilters 的回复不合协议：{other:?}")),
            Err(e) => {
                self.disconnect(error(format!("与分流服务的连接中断：{e}")));
                Err(e)
            }
        })
    }

    fn view(&self, desired: &DesiredState, reasons: &HashMap<ProcessRuleId, Option<String>>) -> ProcRouteStatus {
        let actions = desired
            .rules
            .iter()
            .map(|r| {
                let id = ProcessRuleId(r.id.clone());
                let (up_bps, down_bps) = self.bps.get(&r.id).copied().unwrap_or_default();
                let pending = self.syncer.manual_retry || self.syncer.attempted.as_ref() != Some(desired);
                let confirmed = self.connected() && !pending && self.syncer.last_sent.as_ref() == Some(desired);
                let actual = confirmed.then(|| self.svc.as_ref()?.rules.iter().find(|a| a.id == r.id)).flatten();
                let (action, reason) = if let Some(actual) = actual {
                    (actual.action.as_deref().unwrap_or("Unknown"), actual.reason.clone().or_else(|| reasons.get(&id).cloned().flatten()))
                } else if !self.connected() {
                    ("Unknown", Some("无法连接分流服务，状态未知".into()))
                } else if pending {
                    ("Pending", Some("等待服务确认当前设置".into()))
                } else if !self.syncer.errors.is_empty() {
                    ("Failed", Some("应用失败；请手动重试".into()))
                } else {
                    ("Pending", Some("等待服务确认当前设置".into()))
                };
                (id, RuleView { action: action.into(), reason, up_bps, down_bps })
            })
            .collect();
        let svc = self.svc.as_ref();
        ProcRouteStatus {
            component: self.component.clone(),
            actions,
            last_interruption: svc
                .and_then(|s| s.last_interruption.as_ref())
                .map(|i| InterruptionView { at_unix: i.at_unix, reason: i.reason.clone() }),
            conflicts: svc.map(|s| s.conflicts.clone()).unwrap_or_default(),
            apply_errors: self.syncer.errors().to_vec(),
            truncated_flows: svc.is_some_and(|s| s.truncated_flows),
        }
    }
}

// ---------------------------------------------------------------- 同步任务

enum Command {
    Goodbye(oneshot::Sender<()>),
    Cleanup(oneshot::Sender<Option<Result<(), String>>>),
    Reconnect,
    Retry(oneshot::Sender<Result<(), String>>),
}

static STATUS: LazyLock<Mutex<ProcRouteStatus>> = LazyLock::new(|| Mutex::new(ProcRouteStatus::initial()));
static COMMANDS: OnceLock<mpsc::Sender<Command>> = OnceLock::new();
static RUNTIME_CHANGED: Notify = Notify::const_new();
/// 已开始退出（调用过 [`goodbye`]）：之后不再启动同步任务。
static EXITING: AtomicBool = AtomicBool::new(false);

/// 最近一次推送给前端的状态。
pub fn status() -> ProcRouteStatus {
    STATUS.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// 运行状态或模型变了：让同步任务立即重算并下发。
pub fn runtime_changed() {
    RUNTIME_CHANGED.notify_one();
}

/// 不等重连间隔，立即重新连接（例如刚安装完组件）。
pub fn reconnect_now() {
    if let Some(tx) = COMMANDS.get() {
        let _ = tx.try_send(Command::Reconnect);
    }
}

/// 尽力在当前会话里发送 Goodbye（服务据此按"Routa 正常退出"处理），最多等 1 秒。
/// 之后同步任务结束，不再下发任何状态。
pub async fn goodbye() {
    // 先置标志再看通道（spawn 的顺序相反）：要么同步任务收到 Goodbye，要么它根本不会启动。
    EXITING.store(true, Ordering::SeqCst);
    let Some(tx) = COMMANDS.get() else { return };
    let _ = timeout(GOODBYE_TIMEOUT, async {
        let (done, wait) = oneshot::channel();
        if tx.send(Command::Goodbye(done)).await.is_ok() {
            let _ = wait.await;
        }
    })
    .await;
}

/// 通过当前会话让服务清理残留阻断；没有连上服务（或同步任务不在）时为 `None`。
pub async fn cleanup() -> Option<Result<(), String>> {
    let tx = COMMANDS.get()?;
    let (done, wait) = oneshot::channel();
    tx.send(Command::Cleanup(done)).await.ok()?;
    wait.await.ok().flatten()
}

/// 用户主动重试，等待这一轮应用结果。
pub async fn retry() -> Result<(), String> {
    let tx = COMMANDS.get().ok_or("分流同步任务未启动")?;
    let (done, wait) = oneshot::channel();
    tx.send(Command::Retry(done)).await.map_err(|_| "分流同步任务已结束")?;
    wait.await.map_err(|_| "分流同步任务已结束")?
}

pub fn spawn(rt: Arc<Runtime>, app: AppHandle) -> JoinHandle<()> {
    let (tx, rx) = mpsc::channel(8);
    if COMMANDS.set(tx).is_err() {
        eprintln!("procroute: sync task already running");
    }
    if EXITING.load(Ordering::SeqCst) {
        // 退出流程已经说过（或跳过了）Goodbye：不能再下发状态。
        return tauri::async_runtime::spawn(async {});
    }
    tauri::async_runtime::spawn(run(rt, app, rx))
}

async fn run(rt: Arc<Runtime>, app: AppHandle, mut commands: mpsc::Receiver<Command>) {
    let pid = std::process::id();
    let mut link = Link::default();
    let mut tick = interval(TICK);
    tick.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut next_connect = Instant::now();
    // 发现服务已停止时只尝试启动一次，连上后复位。
    let mut start_tried = false;
    let mut emitted_flows: Option<Vec<Flow>> = None;
    loop {
        let mut retry_done = None;
        let poll = tokio::select! {
            // 命令优先：Goodbye 不能排在一轮同步之后。
            biased;
            cmd = commands.recv() => match cmd {
                None => return,
                Some(Command::Goodbye(done)) => {
                    link.goodbye().await;
                    let _ = done.send(());
                    return;
                }
                Some(Command::Cleanup(done)) => {
                    let _ = done.send(link.cleanup().await);
                    false
                }
                Some(Command::Retry(done)) => {
                    retry_done = Some(done);
                    next_connect = Instant::now();
                    true
                }
                Some(Command::Reconnect) => {
                    next_connect = Instant::now();
                    false
                }
            },
            _ = RUNTIME_CHANGED.notified() => false,
            _ = tick.tick() => true,
        };

        // goodbye() 已开始（可能已超时放弃等待）：不再连接或下发，只等 Goodbye 命令。
        if EXITING.load(Ordering::SeqCst) {
            link.stop();
            if let Some(done) = retry_done { let _ = done.send(Err("程序正在退出".into())); }
            continue;
        }

        if !link.connected() && Instant::now() >= next_connect {
            next_connect = Instant::now() + RECONNECT;
            let res = connect().await;
            match &res {
                Ok(_) => start_tried = false,
                Err(ComponentState::Stopped) if !start_tried => {
                    start_tried = true;
                    if let Err(e) = start_service() {
                        eprintln!("procroute: {e}");
                    }
                }
                Err(_) => {}
            }
            link.attach(res);
        }

        let (model, st) = (rt.model(), rt.status());
        // 全局开关关闭时 decide 不含任何规则：照样下发，服务据此撤掉全部规则。
        let desired = decide(&model, &st, pid);
        if retry_done.is_some() {
            if link.connected() {
                link.syncer.manual_retry = true;
            } else {
                link.syncer.attempted = Some(desired.clone());
                link.syncer.errors = vec!["无法连接分流服务，请手动重试".into()];
            }
        }
        if link.connected() && (link.syncer.manual_retry || link.syncer.attempted.as_ref() != Some(&desired)) {
            link.svc = None;
            let pending = link.view(&desired, &decide::reasons(&model, &st));
            *STATUS.lock().unwrap_or_else(|e| e.into_inner()) = pending.clone();
            let _ = app.emit("procroute-status", pending);
        }
        let was_connected = link.connected();
        link.sync(desired.clone(), poll).await;
        if was_connected && !link.connected() {
            next_connect = Instant::now() + RECONNECT;
        }

        let view = link.view(&desired, &decide::reasons(&model, &st));
        let changed = {
            let mut cur = STATUS.lock().unwrap_or_else(|e| e.into_inner());
            let changed = *cur != view;
            *cur = view.clone();
            changed
        };
        if changed {
            if let Err(e) = app.emit("procroute-status", view) {
                eprintln!("emit procroute-status failed: {e}");
            }
        }
        if let Some(done) = retry_done {
            let result = if !link.connected() { Err("无法连接分流服务，请手动重试".into()) }
                else if link.syncer.errors.is_empty() { Ok(()) }
                else { Err(link.syncer.errors.join("；")) };
            let _ = done.send(result);
        }
        if emitted_flows.as_ref() != Some(&link.flows) {
            if let Err(e) = app.emit("procroute-flows", &link.flows) {
                eprintln!("emit procroute-flows failed: {e}");
            }
            emitted_flows = Some(link.flows.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use routa_procsvc_proto::{Action, ExitPolicy, FaultPolicy, RuleSpec};

    use super::*;

    /// 按顺序回放预设回复的假连接，记录收到的请求。
    struct FakeConn {
        sent: Arc<Mutex<Vec<Request>>>,
        replies: Vec<Result<Response, String>>,
        attach_error: Option<String>,
    }

    impl FakeConn {
        fn new(replies: Vec<Result<Response, String>>) -> (Self, Arc<Mutex<Vec<Request>>>) {
            let sent = Arc::new(Mutex::new(Vec::new()));
            (Self { sent: sent.clone(), replies, attach_error: None }, sent)
        }
    }

    #[async_trait]
    impl SvcConn for FakeConn {
        async fn call(&mut self, req: Request) -> Result<Response, String> {
            self.sent.lock().unwrap().push(req.clone());
            if matches!(req, Request::Attach { .. }) {
                return Ok(self.attach_error.as_ref().map(|e| Response::Error(e.clone())).unwrap_or(Response::Ok));
            }
            if matches!(req, Request::GetStatus) && !matches!(self.replies.first(), Some(Ok(Response::Status(_)))) {
                return Ok(Response::Status(SvcStatus {
                    version: PROTO_VERSION, owner_sid: "test".into(),
                    last_interruption: None, truncated_flows: false, conflicts: vec![], rules: vec![],
                }));
            }
            if self.replies.is_empty() {
                return Err("no more replies".into());
            }
            self.replies.remove(0)
        }
    }

    fn state(exe: &str) -> DesiredState {
        DesiredState {
            rules: vec![RuleSpec {
                id: "r1".into(),
                exe_path: exe.into(),
                action: Action::Block,
                on_fault: FaultPolicy::Block,
                reset_on_change: false,
            }],
            on_routa_exit: ExitPolicy::Block,
            routa_pid: 1,
        }
    }

    fn applies(sent: &Mutex<Vec<Request>>) -> usize {
        sent.lock().unwrap().iter().filter(|r| matches!(r, Request::Apply(_))).count()
    }

    #[tokio::test]
    async fn identical_state_is_not_resent() {
        let (mut conn, sent) = FakeConn::new(vec![Ok(Response::Ok)]);
        let mut s = Syncer::default();
        assert_eq!(s.push(&mut conn, state(r"C:\a.exe")).await, Ok(true));
        assert_eq!(s.push(&mut conn, state(r"C:\a.exe")).await, Ok(false));
        assert_eq!(applies(&sent), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn failures_wait_for_manual_retry_and_changed_settings_are_sent() {
        let (mut conn, sent) = FakeConn::new(vec![
            Ok(Response::Applied { errors: vec!["代理失败".into()] }),
            Ok(Response::Error("拒绝".into())),
            Ok(Response::Ok),
        ]);
        let mut s = Syncer::default();
        let a = state(r"C:\a.exe");
        let b = state(r"C:\b.exe");
        assert_eq!(s.push(&mut conn, a.clone()).await, Ok(true));
        tokio::time::advance(Duration::from_secs(60)).await;
        assert_eq!(s.push(&mut conn, a).await, Ok(false));
        assert_eq!(s.errors(), ["代理失败"]);
        assert_eq!(s.push(&mut conn, b.clone()).await, Ok(true));
        assert_eq!(s.push(&mut conn, b.clone()).await, Ok(false));
        s.manual_retry = true;
        assert_eq!(s.push(&mut conn, b.clone()).await, Ok(true));
        assert!(s.errors().is_empty());
        assert_eq!(s.last_sent, Some(b.clone()));
        assert_eq!(sent.lock().unwrap().last(), Some(&Request::RetryApply(b)));
    }

    #[tokio::test]
    async fn reconnect_resends_full_state() {
        let mut link = Link::default();
        let (conn, sent) = FakeConn::new(vec![
            Ok(Response::Hello { version: PROTO_VERSION, accepted: true }),
            Ok(Response::Ok),
        ]);
        link.attach(handshake(Box::new(conn)).await);
        assert_eq!(link.component, ComponentState::Running);
        link.sync(state(r"C:\a.exe"), false).await;
        link.sync(state(r"C:\a.exe"), false).await;
        assert_eq!(applies(&sent), 1);

        let (conn, sent) = FakeConn::new(vec![
            Ok(Response::Hello { version: PROTO_VERSION, accepted: true }),
            Ok(Response::Ok),
        ]);
        link.attach(handshake(Box::new(conn)).await);
        link.sync(state(r"C:\a.exe"), false).await;
        assert_eq!(applies(&sent), 1);
    }

    #[tokio::test]
    async fn needs_update_when_hello_rejected() {
        let mut link = Link::default();
        let (conn, sent) = FakeConn::new(vec![
            Ok(Response::Hello { version: PROTO_VERSION + 1, accepted: false }),
            Ok(Response::Ok),
        ]);
        link.attach(handshake(Box::new(conn)).await);
        assert_eq!(link.component, ComponentState::NeedsUpdate);
        assert!(!link.connected());
        link.sync(state(r"C:\a.exe"), true).await;
        assert_eq!(*sent.lock().unwrap(), vec![Request::Hello { version: PROTO_VERSION }]);
    }

    #[tokio::test]
    async fn rejected_registration_disconnects_without_applying_or_clearing_errors() {
        let mut link = Link::default();
        link.syncer.errors = vec!["原来的应用错误".into()];
        let (mut conn, sent) = FakeConn::new(vec![]);
        conn.attach_error = Some("进程已退出".into());
        link.attach(Ok(Box::new(conn)));
        link.sync(state(r"C:\a.exe"), true).await;
        assert!(!link.connected());
        assert_eq!(applies(&sent), 0);
        assert_eq!(link.syncer.errors(), ["原来的应用错误"]);
    }

    #[tokio::test]
    async fn failed_state_is_not_retried_after_reconnect() {
        for reply in [Ok(Response::Error("拒绝".into())), Ok(Response::Applied { errors: vec!["部分失败".into()] }), Err("timeout".into())] {
            let mut link = Link::default();
            let (conn, _) = FakeConn::new(vec![reply]);
            link.attach(Ok(Box::new(conn)));
            link.sync(state(r"C:\a.exe"), false).await;
            let (conn, sent) = FakeConn::new(vec![]);
            link.attach(Ok(Box::new(conn)));
            link.sync(state(r"C:\a.exe"), false).await;
            assert_eq!(applies(&sent), 0);
            assert!(matches!(sent.lock().unwrap().as_slice(), [Request::Attach { routa_pid: 1 }]));
            link.goodbye().await;
            assert!(matches!(sent.lock().unwrap().last(), Some(Request::Goodbye)));
            assert!(!link.syncer.errors().is_empty());
        }
    }

    #[tokio::test]
    async fn stopped_link_sends_nothing_but_goodbye() {
        let mut link = Link::default();
        let (conn, sent) = FakeConn::new(vec![
            Ok(Response::Hello { version: PROTO_VERSION, accepted: true }),
            Ok(Response::Ok),
        ]);
        link.attach(handshake(Box::new(conn)).await);
        link.stop();
        link.sync(state(r"C:.exe"), true).await;
        link.goodbye().await;
        assert_eq!(
            *sent.lock().unwrap(),
            vec![Request::Hello { version: PROTO_VERSION }, Request::Goodbye]
        );
    }

    #[tokio::test]
    async fn transport_error_disconnects() {
        let mut link = Link::default();
        let (conn, _sent) = FakeConn::new(vec![Ok(Response::Hello { version: PROTO_VERSION, accepted: true })]);
        link.attach(handshake(Box::new(conn)).await);
        link.sync(state(r"C:\a.exe"), false).await;
        assert!(!link.connected());
        assert!(matches!(link.component, ComponentState::Error { .. }));
    }

    fn flow(pid: u32, rule: Option<&str>, host: &str, sent: u64, received: u64) -> Flow {
        Flow {
            pid,
            rule_id: rule.map(str::to_string),
            exe_path: None,
            host: host.into(),
            port: 443,
            udp: false,
            age_ms: None,
            sent,
            received,
        }
    }

    #[test]
    fn rates_from_consecutive_flow_samples() {
        let t0 = Instant::now();
        let mut r = Rates::default();
        // 第一次样本只作基准。
        let first = r.update(&[flow(1, Some("r1"), "a", 1000, 2000), flow(9, None, "x", 50, 50)], t0);
        assert!(first.is_empty());
        let rates = r.update(
            &[
                flow(1, Some("r1"), "a", 3000, 2000), // +2000 / +0
                flow(1, Some("r1"), "b", 400, 600),   // 新连接，按 0 起算
                flow(2, Some("r2"), "c", 100, 0),
                flow(9, None, "x", 500, 500), // 不属于任何规则
            ],
            t0 + Duration::from_secs(2),
        );
        assert_eq!(rates.len(), 2);
        assert_eq!(rates["r1"], (1200, 300));
        assert_eq!(rates["r2"], (50, 0));
        // 计数变小（同键的一条连接关闭）不会算成负数。
        let rates = r.update(&[flow(1, Some("r1"), "a", 10, 10)], t0 + Duration::from_secs(3));
        assert_eq!(rates["r1"], (0, 0));
    }

    #[test]
    fn view_never_treats_desired_or_stale_state_as_confirmed() {
        let mut link = Link::default();
        let desired = state(r"C:\a.exe");
        let key = ProcessRuleId("r1".into());
        assert_eq!(link.view(&desired, &HashMap::new()).actions[&key].action, "Unknown");
        let (conn, _) = FakeConn::new(vec![]);
        link.attach(Ok(Box::new(conn)));
        assert_eq!(link.view(&desired, &HashMap::new()).actions[&key].action, "Pending");
        link.syncer.attempted = Some(desired.clone());
        link.syncer.errors = vec!["被拒绝".into()];
        assert_eq!(link.view(&desired, &HashMap::new()).actions[&key].action, "Failed");
        link.syncer.last_sent = Some(desired.clone());
        link.svc = Some(SvcStatus {
            version: PROTO_VERSION, owner_sid: "test".into(), last_interruption: None,
            truncated_flows: false, conflicts: vec![],
            rules: vec![routa_procsvc_proto::RuleStatus { id: "r1".into(), action: Some("Block".into()), reason: Some("应用失败，已阻断".into()) }],
        });
        assert_eq!(link.view(&desired, &HashMap::new()).actions[&key].action, "Block");
        assert_eq!(link.view(&state(r"C:\b.exe"), &HashMap::new()).actions[&key].action, "Pending");
        link.disconnect(error("断开"));
        assert_eq!(link.view(&desired, &HashMap::new()).actions[&key].action, "Unknown");
    }

    #[tokio::test(start_paused = true)]
    async fn polling_failed_apply_only_reads_status_until_user_retries() {
        let desired = state(r"C:\a.exe");
        let (conn, sent) = FakeConn::new(vec![
            Ok(Response::Applied { errors: vec!["部分失败".into()] }),
            Ok(Response::Flows(vec![])),
            Ok(Response::Flows(vec![])),
            Ok(Response::Ok),
        ]);
        let mut link = Link::default();
        link.attach(Ok(Box::new(conn)));
        link.sync(desired.clone(), false).await;
        for _ in 0..2 {
            tokio::time::advance(Duration::from_secs(60)).await;
            link.sync(desired.clone(), true).await;
        }
        assert_eq!(applies(&sent), 1);
        link.syncer.manual_retry = true;
        link.sync(desired, false).await;
        assert!(link.syncer.errors().is_empty());
        assert_eq!(sent.lock().unwrap().iter().filter(|r| matches!(r, Request::RetryApply(_))).count(), 1);
    }

    #[test]
    fn view_reports_actions_rates_and_service_status() {
        let mut link = Link::default();
        let (conn, _) = FakeConn::new(vec![]);
        link.attach(Ok(Box::new(conn)));
        link.syncer.last_sent = Some(state(r"C:\a.exe"));
        link.syncer.attempted = link.syncer.last_sent.clone();
        link.bps.insert("r1".into(), (10, 20));
        link.svc = Some(SvcStatus {
            version: PROTO_VERSION,
            owner_sid: "S-1-5-21-1".into(),
            last_interruption: Some(routa_procsvc_proto::Interruption { at_unix: 5, reason: "崩溃".into() }),
            truncated_flows: true,
            conflicts: vec!["冲突".into()],
            rules: vec![routa_procsvc_proto::RuleStatus { id: "r1".into(), action: Some("Block".into()), reason: None }],
        });
        let why = HashMap::from([(ProcessRuleId("r1".into()), Some("内核崩溃".into()))]);
        let v = link.view(&state(r"C:\a.exe"), &why);
        assert_eq!(
            v.actions[&ProcessRuleId("r1".into())],
            RuleView { action: "Block".into(), reason: Some("内核崩溃".into()), up_bps: 10, down_bps: 20 }
        );
        assert_eq!(v.last_interruption, Some(InterruptionView { at_unix: 5, reason: "崩溃".into() }));
        assert_eq!((v.conflicts, v.truncated_flows), (vec!["冲突".to_string()], true));
    }
}

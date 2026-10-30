//! 管道服务：会话、Apply 校验与落地、Routa 进程监视、回落（spec 6.3、6.5、6.6）。
//!
//! 业务逻辑都在同步的 [`Server`] 上，便于用假引擎测试；[`run_pipe`] 只负责命名管道、
//! 分帧收发和定时检查 Routa 进程是否已退出。

use std::collections::HashSet;
use std::ffi::c_void;
use std::future::Future;
use std::io;
use std::net::IpAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use routa_procsvc_proto::{
    decode, decode_len, encode, Action, DesiredState, FaultPolicy, Interruption, Request, Response,
    RuleSpec, SvcStatus, PROTO_VERSION,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};
use tokio::time::{interval, timeout_at, Instant, Interval, MissedTickBehavior};
use windows_sys::Win32::Foundation::{CloseHandle, LocalFree, HANDLE, WAIT_OBJECT_0};
use windows_sys::Win32::Security::Authorization::{
    ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
};
use windows_sys::Win32::Security::{PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES};
use windows_sys::Win32::System::RemoteDesktop::ProcessIdToSessionId;
use windows_sys::Win32::System::Services::{
    CloseServiceHandle, OpenSCManagerW, OpenServiceW, QueryServiceStatus, SC_MANAGER_CONNECT,
    SERVICE_QUERY_STATUS, SERVICE_STATUS, SERVICE_STOPPED,
};
use windows_sys::Win32::System::Threading::{
    OpenProcess, WaitForSingleObject, PROCESS_SYNCHRONIZE,
};

use crate::engine::{ConnKiller, Guard, Interceptor};
use crate::fallback::{fallback, RoutaGone};
use crate::persist;
use crate::reconcile::{exe_key, Reconciler};

/// ProxyBridge 最多 32 个代理配置（ruling R13）。
const MAX_PROXY_RULES: usize = 32;
/// `ProxyBridge_GetFlows` 一次最多返回的条数；达到即视为截断。
const FLOW_LIMIT: usize = 400;
/// 检查 Routa 进程是否已退出的间隔。
const WATCH_INTERVAL: Duration = Duration::from_millis(250);
/// 连上后必须在这段时间内握手成功，否则断开（防止空占唯一的管道实例）。
const HELLO_TIMEOUT: Duration = Duration::from_secs(5);
const ROUTA_CRASHED: &str = "Routa 意外退出";
const SERVICE_RESTARTED: &str = "分流服务意外退出后已重启";
const CONFLICT_SERVICE: &str = "ProxyBridgeSvc";
const CONFLICT_MESSAGE: &str = "检测到官方 ProxyBridge 服务正在运行";

pub struct Server<I: Interceptor, G: Guard, K: ConnKiller> {
    reconciler: Reconciler<I, G, K>,
    state_path: PathBuf,
    owner_sid: String,
    /// 最近一次完整状态（Routa 下发的，或启动时读回的），回落以它为准。
    last: Option<DesiredState>,
    /// 当前在场的 Routa：其状态正在生效；离开时按它回落一次。
    routa: Option<Routa>,
    last_interruption: Option<Interruption>,
    truncated_flows: bool,
    next_session: u64,
}

/// 一条管道连接的会话状态。由 [`Server::new_session`] 创建。
pub struct Session {
    pub hello_ok: bool,
    pub said_goodbye: bool,
    /// 收到过版本不一致的 Hello。
    hello_rejected: bool,
    id: u64,
}

/// 下发了当前状态的那个 Routa。
struct Routa {
    session: u64,
    process: Process,
    /// Routa 进程所在的 Windows 会话；查不到时为 None（注销不会触发它的回落）。
    windows_session: Option<u32>,
    said_goodbye: bool,
}

impl<I: Interceptor, G: Guard, K: ConnKiller> Server<I, G, K> {
    /// 服务启动：读回上次的状态并按"Routa 不在"回落。
    ///
    /// - 读到状态、上次正常停止：按 `on_routa_exit`（`NeverConnected`）；
    /// - 读到状态、上次非正常停止（崩溃、被杀）：按每条规则的 `on_fault`（`Crashed`），并记一次中断；
    /// - 读到状态时，删掉不在该状态里受保护的残留永久阻断（ruling R11）；
    /// - 状态缺失或损坏：什么都不动，已有的永久阻断全部保留（宁可多挡）。
    ///
    /// 启动时的引擎错误无处上报，忽略；Routa 连上后的全量 Apply 会重试。
    pub fn boot(
        reconciler: Reconciler<I, G, K>,
        state_path: PathBuf,
        owner_sid: String,
        previous_stop_clean: bool,
    ) -> Self {
        let mut srv = Server {
            reconciler,
            state_path,
            owner_sid,
            last: None,
            routa: None,
            last_interruption: None,
            truncated_flows: false,
            next_session: 0,
        };
        match persist::load(&srv.state_path) {
            Some(state) => {
                let why = if previous_stop_clean {
                    RoutaGone::NeverConnected
                } else {
                    RoutaGone::Crashed
                };
                srv.reconciler.apply(fallback(&state, why));
                if !previous_stop_clean {
                    srv.interrupted(SERVICE_RESTARTED);
                }
                let _ = srv.remove_stale_blocks(&guarded_keys(&state.rules));
                srv.last = Some(state);
            }
            None => {
                srv.reconciler.apply(Vec::new());
            }
        }
        srv
    }

    pub fn new_session(&mut self) -> Session {
        self.next_session += 1;
        Session {
            hello_ok: false,
            said_goodbye: false,
            hello_rejected: false,
            id: self.next_session,
        }
    }

    pub fn handle(&mut self, req: Request, session: &mut Session) -> Response {
        match req {
            Request::Hello { version } => {
                session.hello_ok = version == PROTO_VERSION;
                session.hello_rejected = !session.hello_ok;
                Response::Hello {
                    version: PROTO_VERSION,
                    accepted: session.hello_ok,
                }
            }
            _ if !session.hello_ok => Response::Error(
                if session.hello_rejected {
                    "版本不一致"
                } else {
                    "会话的第一条消息必须是 Hello"
                }
                .into(),
            ),
            Request::Attach { routa_pid } => match Process::open(routa_pid) {
                Ok(process) => {
                    self.routa = Some(Routa {
                        session: session.id,
                        process,
                        windows_session: windows_session_of(routa_pid),
                        said_goodbye: false,
                    });
                    session.said_goodbye = false;
                    Response::Ok
                }
                Err(e) => Response::Error(e),
            },
            Request::Apply(state) => self.apply(state, session),
            Request::RetryApply(state) => self.apply_inner(state, session, true),
            Request::Goodbye => {
                session.said_goodbye = true;
                if let Some(r) = self.routa.as_mut().filter(|r| r.session == session.id) {
                    r.said_goodbye = true;
                }
                Response::Ok
            }
            Request::GetFlows => {
                let flows = self.reconciler.interceptor().flows();
                self.truncated_flows = flows.len() >= FLOW_LIMIT;
                Response::Flows(flows)
            }
            Request::GetStatus => Response::Status(SvcStatus {
                version: PROTO_VERSION,
                owner_sid: self.owner_sid.clone(),
                last_interruption: self.last_interruption.clone(),
                truncated_flows: self.truncated_flows,
                conflicts: conflicts(),
                rules: self.reconciler.status(),
            }),
            Request::CleanupFilters => {
                let keep = guarded_keys(self.reconciler.current());
                match self.remove_stale_blocks(&keep) {
                    Ok(errors) => applied(errors),
                    Err(e) => Response::Error(e),
                }
            }
        }
    }

    /// 校验 → 打开 Routa 进程句柄 → 保存 → 落地。校验、打开句柄或保存失败时什么都不动，
    /// 回 `Error`（保存不了就不落地：否则服务重启后会按旧状态回落）。
    /// 落地时引擎的非致命错误以 `Applied` 返回（失败的规则已按 Block 处理）。
    fn apply(&mut self, state: DesiredState, session: &mut Session) -> Response {
        self.apply_inner(state, session, false)
    }

    fn apply_inner(&mut self, state: DesiredState, session: &mut Session, retry: bool) -> Response {
        if let Err(e) = validate(&state) {
            return Response::Error(e);
        }
        let process = match Process::open(state.routa_pid) {
            Ok(p) => p,
            Err(e) => return Response::Error(e),
        };
        if let Err(e) = persist::save(&self.state_path, &state) {
            return Response::Error(format!("保存状态失败：{e}"));
        }
        let errors = if retry { self.reconciler.retry(state.rules.clone()) } else { self.reconciler.apply(state.rules.clone()) };
        let state_pid = state.routa_pid;
        self.last = Some(state);
        self.routa = Some(Routa {
            session: session.id,
            process,
            windows_session: windows_session_of(state_pid),
            said_goodbye: false,
        });
        session.said_goodbye = false;
        applied(errors)
    }

    /// 会话结束（管道断开）。若是在场 Routa 的会话，按是否说过 Goodbye 回落。
    pub fn end_session(&mut self, session: &Session) {
        if self.routa.as_ref().is_some_and(|r| r.session == session.id) {
            self.routa_left();
        }
    }

    /// 在场的 Routa 进程已退出时，按是否说过 Goodbye 回落。
    pub fn check_routa(&mut self) {
        if self.routa.as_ref().is_some_and(|r| r.process.exited()) {
            self.routa_left();
        }
    }

    fn routa_left(&mut self) {
        let why = if self.routa.as_ref().is_some_and(|r| r.said_goodbye) {
            RoutaGone::Graceful
        } else {
            RoutaGone::Crashed
        };
        self.routa_gone(why);
    }

    /// Routa 离开：按 `why` 回落一次（崩溃时记录中断）。没有在场的 Routa 时什么都不做，
    /// 所以管道断开、进程退出、关机几路同时触发也只回落一次。
    pub fn routa_gone(&mut self, why: RoutaGone) {
        if self.routa.take().is_none() {
            return;
        }
        if let Some(state) = &self.last {
            let rules = fallback(state, why);
            // 无处上报；失败的规则已按 Block 处理
            self.reconciler.apply(rules);
        }
        if why == RoutaGone::Crashed {
            self.interrupted(ROUTA_CRASHED);
        }
    }

    /// 关机：等同 Routa 正常退出。
    pub fn system_ending(&mut self) {
        self.routa_gone(RoutaGone::Graceful);
    }

    /// Windows 会话 `windows_session` 注销：只有在场 Routa 所在的会话注销时才等同 Routa 正常退出；
    /// 别的用户注销（快速切换用户、断开的远程桌面）不影响。
    pub fn session_logoff(&mut self, windows_session: u32) {
        if self
            .routa
            .as_ref()
            .is_some_and(|r| r.windows_session == Some(windows_session))
        {
            self.system_ending();
        }
    }

    fn interrupted(&mut self, reason: &str) {
        let at_unix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        self.last_interruption = Some(Interruption {
            at_unix,
            reason: reason.into(),
        });
    }

    /// 删除 exe（按 [`exe_key`]）不在 `keep` 里的永久阻断，返回逐条删除的错误。
    /// 读不到现有阻断时什么都不删，返回 `Err`。
    fn remove_stale_blocks(&mut self, keep: &HashSet<String>) -> Result<Vec<String>, String> {
        let guard = self.reconciler.guard();
        let exes = guard
            .blocked_exes()
            .map_err(|e| format!("读取永久阻断失败：{e}"))?;
        Ok(exes
            .into_iter()
            .filter(|exe| !keep.contains(&exe_key(exe)))
            .filter_map(|exe| {
                guard
                    .remove_block(&exe)
                    .err()
                    .map(|e| format!("移除 {exe} 的永久阻断失败：{e}"))
            })
            .collect())
    }
}

/// 已执行的请求的回复：没有错误为 `Ok`，部分失败为 `Applied`。
fn applied(errors: Vec<String>) -> Response {
    if errors.is_empty() {
        Response::Ok
    } else {
        Response::Applied { errors }
    }
}

/// 需要 WFP 永久阻断的 exe（on_fault=Block）。
fn guarded_keys(rules: &[RuleSpec]) -> HashSet<String> {
    rules
        .iter()
        .filter(|r| r.on_fault == FaultPolicy::Block)
        .map(|r| exe_key(&r.exe_path))
        .collect()
}

/// 不能作为规则目标的 exe（spec 4）：服务自身（安装后的路径与正在运行的路径）。
fn own_exes() -> Vec<String> {
    let installed = crate::install::installed_exe().ok();
    let running = std::env::current_exe().ok();
    installed
        .into_iter()
        .chain(running)
        .map(|p: PathBuf| exe_key(&p.to_string_lossy()))
        .collect()
}

/// 服务端对 Apply 的校验（spec 4、6.5、ruling R15）。
fn validate(state: &DesiredState) -> Result<(), String> {
    let own = own_exes();
    let mut ids = HashSet::new();
    let mut exes = HashSet::new();
    let mut proxies = 0;
    for r in &state.rules {
        if !ids.insert(r.id.as_str()) {
            return Err(format!("规则 id 重复：{}", r.id));
        }
        let exe = &r.exe_path;
        if let Some(reason) = routa_procsvc_proto::exe_path_problem(exe) {
            return Err(format!("程序路径无效（{reason}）：{exe}"));
        }
        if own.contains(&exe_key(exe)) {
            return Err(format!("不能对进程分流组件自身设置规则：{exe}"));
        }
        if !exes.insert(exe_key(exe)) {
            return Err(format!("程序路径重复：{exe}"));
        }
        if let Action::Proxy(t) = &r.action {
            proxies += 1;
            if t.host.parse::<IpAddr>().is_err() {
                return Err(format!("规则 {} 的代理地址必须是 IP：{}", r.id, t.host));
            }
            if t.port == 0 {
                return Err(format!("规则 {} 的代理端口不能为 0", r.id));
            }
        }
    }
    if proxies > MAX_PROXY_RULES {
        return Err(format!(
            "代理规则最多 {MAX_PROXY_RULES} 条，收到 {proxies} 条"
        ));
    }
    Ok(())
}

/// 进程所在的 Windows 会话 id；进程不存在或无权查询时为 None。
pub fn windows_session_of(pid: u32) -> Option<u32> {
    let mut id = 0u32;
    // SAFETY: id 是可写的局部变量。
    let ok = unsafe { ProcessIdToSessionId(pid, &mut id) };
    (ok != 0).then_some(id)
}

/// 等待用的进程句柄。
struct Process(HANDLE);

// SAFETY: 进程句柄是内核对象句柄，可在任意线程使用和关闭。
unsafe impl Send for Process {}

impl Process {
    /// 打开仍在运行的进程；不存在、无权访问或已退出时报错。
    fn open(pid: u32) -> Result<Process, String> {
        // SAFETY: 普通 Win32 调用；返回的句柄由 Process 持有并在 Drop 时关闭。
        let h = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, pid) };
        if h.is_null() {
            return Err(format!(
                "无法打开 Routa 进程 {pid}：{}",
                io::Error::last_os_error()
            ));
        }
        let p = Process(h);
        if p.exited() {
            return Err(format!("Routa 进程 {pid} 已退出"));
        }
        Ok(p)
    }

    fn exited(&self) -> bool {
        // SAFETY: self.0 是有效的、带 SYNCHRONIZE 权限的进程句柄。
        unsafe { WaitForSingleObject(self.0, 0) == WAIT_OBJECT_0 }
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        // SAFETY: self.0 由 OpenProcess 返回，只关闭这一次。
        unsafe { CloseHandle(self.0) };
    }
}

/// 可能与本服务冲突的情况（中文，可直接展示）。
pub fn conflicts() -> Vec<String> {
    if service_running(CONFLICT_SERVICE) {
        vec![CONFLICT_MESSAGE.into()]
    } else {
        Vec::new()
    }
}

/// 服务存在且不处于已停止状态（含启动中、停止中）。不存在或无法查询时为 false。
fn service_running(name: &str) -> bool {
    let wide: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
    // SAFETY: 句柄只在本函数内使用并关闭；wide 以 NUL 结尾；status 是可写的局部变量。
    unsafe {
        let scm = OpenSCManagerW(std::ptr::null(), std::ptr::null(), SC_MANAGER_CONNECT);
        if scm.is_null() {
            return false;
        }
        let svc = OpenServiceW(scm, wide.as_ptr(), SERVICE_QUERY_STATUS);
        let mut running = false;
        if !svc.is_null() {
            let mut status: SERVICE_STATUS = std::mem::zeroed();
            if QueryServiceStatus(svc, &mut status) != 0 {
                running = status.dwCurrentState != SERVICE_STOPPED;
            }
            CloseServiceHandle(svc);
        }
        CloseServiceHandle(scm);
        running
    }
}

/// 管道 DACL：SYSTEM、Administrators 完全控制，组件所有者读写（spec 6.5）。
///
/// 所有者的权限是 `FILE_GENERIC_READ | FILE_GENERIC_WRITE` 去掉 `FILE_CREATE_PIPE_INSTANCE`
/// （0x4）：能连接收发，但不能创建同名管道实例冒充服务。
///
/// 服务自己要不断创建新实例，所以管道对象的属主（`OW`，即创建管道的进程的默认属主）完全控制。
/// 服务以 LocalSystem 运行时属主是 SYSTEM / Administrators，与前两条重复；
/// 测试里以组件所有者身份运行服务，靠这一条才能创建后续实例。
fn pipe_sddl(owner_sid: &str) -> io::Result<String> {
    let valid = owner_sid.strip_prefix("S-1-").is_some_and(|rest| {
        !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit() || b == b'-')
    });
    if !valid {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("无效的所有者 SID：{owner_sid}"),
        ));
    }
    Ok(format!(
        "D:P(A;;GA;;;SY)(A;;GA;;;BA)(A;;GA;;;OW)(A;;0x12019b;;;{owner_sid})"
    ))
}

/// 由 SDDL 转换得到的安全描述符（LocalAlloc 分配）。
pub struct SecurityDescriptor(pub PSECURITY_DESCRIPTOR);

impl SecurityDescriptor {
    pub fn from_sddl(sddl: &str) -> io::Result<Self> {
        let wide: Vec<u16> = sddl.encode_utf16().chain(Some(0)).collect();
        let mut sd: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
        // SAFETY: wide 以 NUL 结尾；成功时 sd 指向 LocalAlloc 的内存，由 Drop 释放。
        let ok = unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                wide.as_ptr(),
                SDDL_REVISION_1,
                &mut sd,
                std::ptr::null_mut(),
            )
        };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(SecurityDescriptor(sd))
    }
}

impl Drop for SecurityDescriptor {
    fn drop(&mut self) {
        // SAFETY: self.0 由 ConvertStringSecurityDescriptorToSecurityDescriptorW 分配。
        unsafe { LocalFree(self.0) };
    }
}

// SAFETY: 安全描述符是 LocalAlloc 分配的普通内存，可在任意线程使用和释放。
unsafe impl Send for SecurityDescriptor {}

/// 创建一个管道实例。服务同一时刻只有一个空闲实例：正在服务客户端时没有空闲实例，
/// 其余连接得到 ERROR_PIPE_BUSY。`first` 时要求是该名字的第一个实例，防止别的进程抢先占用管道名；
/// 之后在旧实例关闭前先建好新实例（所以上限是 2），管道名始终被本服务占着。
fn create_pipe(name: &str, sd: &SecurityDescriptor, first: bool) -> io::Result<NamedPipeServer> {
    let mut sa = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: sd.0,
        bInheritHandle: 0,
    };
    // SAFETY: sa 及其指向的安全描述符在调用期间有效。
    unsafe {
        ServerOptions::new()
            .first_pipe_instance(first)
            .max_instances(2)
            .reject_remote_clients(true)
            .create_with_security_attributes_raw(name, &mut sa as *mut _ as *mut c_void)
    }
}

/// 取锁；锁中毒（持锁时 panic）时照常取用里面的值。
pub fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// 等 `fut` 完成，期间定时检查 Routa 进程是否已退出。
async fn watching<T, I, G, K>(
    fut: impl Future<Output = T>,
    server: &Mutex<Server<I, G, K>>,
    tick: &mut Interval,
) -> T
where
    I: Interceptor,
    G: Guard,
    K: ConnKiller,
{
    tokio::pin!(fut);
    loop {
        tokio::select! {
            out = &mut fut => return out,
            _ = tick.tick() => lock(server).check_routa(),
        }
    }
}

/// 读一帧请求。对端关闭时为 `Ok(None)`；正文不是合法请求时为 `Ok(Some(Err))`。
async fn read_frame(pipe: &mut NamedPipeServer) -> io::Result<Option<Result<Request, String>>> {
    let mut header = [0u8; 4];
    match pipe.read_exact(&mut header).await {
        Ok(_) => {}
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e),
    }
    let len = decode_len(header).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let mut body = vec![0u8; len];
    pipe.read_exact(&mut body).await?;
    Ok(Some(decode(&body)))
}

async fn write_frame(pipe: &mut NamedPipeServer, resp: &Response) -> io::Result<()> {
    let buf = match encode(resp) {
        Ok(b) => b,
        Err(e) => encode(&Response::Error(format!("回复编码失败：{e}")))
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?,
    };
    pipe.write_all(&buf).await?;
    pipe.flush().await
}

/// 在管道 `pipe_name` 上一个接一个地服务客户端，直到出现无法恢复的错误。
/// DACL 按服务的 `owner_sid` 生成。连上后 [`HELLO_TIMEOUT`] 内没有握手成功的客户端被断开。
/// 会话结束（断开、协议错误、握手超时）时调用 [`Server::end_session`]；
/// 全程每 250 ms 调用一次 [`Server::check_routa`]。
///
/// 不在内部 spawn 任务，引擎类型不需要 `Send`。
pub async fn run_pipe<I, G, K>(
    server: Arc<Mutex<Server<I, G, K>>>,
    pipe_name: &str,
) -> io::Result<()>
where
    I: Interceptor,
    G: Guard,
    K: ConnKiller,
{
    let owner_sid = lock(&server).owner_sid.clone();
    let sd = SecurityDescriptor::from_sddl(&pipe_sddl(&owner_sid)?)?;
    let mut pipe = create_pipe(pipe_name, &sd, true)?;
    let mut tick = interval(WATCH_INTERVAL);
    tick.set_missed_tick_behavior(MissedTickBehavior::Delay);
    loop {
        // 例如客户端连上又立刻断开：换一个新实例再等
        if watching(pipe.connect(), &server, &mut tick).await.is_ok() {
            let mut session = lock(&server).new_session();
            let hello_deadline = Instant::now() + HELLO_TIMEOUT;
            loop {
                let read = read_frame(&mut pipe);
                let req = if session.hello_ok {
                    watching(read, &server, &mut tick).await
                } else {
                    match watching(timeout_at(hello_deadline, read), &server, &mut tick).await {
                        Ok(r) => r,
                        Err(_) => break,
                    }
                };
                let Ok(Some(req)) = req else { break };
                let resp = match req {
                    Ok(req) => lock(&server).handle(req, &mut session),
                    Err(e) => Response::Error(format!("无法解析请求：{e}")),
                };
                let written = watching(write_frame(&mut pipe, &resp), &server, &mut tick).await;
                if written.is_err() {
                    break;
                }
            }
            lock(&server).end_session(&session);
        }
        // 实例不复用（断开后的读写状态不可靠）：先建新实例，再关旧实例
        let next = create_pipe(pipe_name, &sd, false)?;
        let _ = pipe.disconnect();
        pipe = next;
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::engine::PbAction;
    use crate::persist;
    use crate::reconcile::exe_key;
    use routa_procsvc_proto::{
        Action, ExitPolicy, FaultPolicy, Flow, ProxyKind, ProxyTarget, RuleSpec, PROTO_VERSION,
    };
    use std::collections::{HashMap, HashSet};
    use std::path::Path;
    use std::sync::{Arc, Mutex};

    const A: &str = r"C:\app\a.exe";
    const B: &str = r"C:\app\b.exe";

    /// 三个假引擎共享的"系统"。
    #[derive(Debug, Default)]
    pub(crate) struct World {
        pub(crate) log: Vec<String>,
        pub(crate) rules: HashMap<String, (String, PbAction)>,
        pub(crate) blocks: HashMap<String, String>,
        pub(crate) permits: HashSet<String>,
        pub(crate) flows: usize,
        /// 这些规则 id 的非 Block `set_rule` 失败。
        pub(crate) fail_rules: HashSet<String>,
    }

    pub(crate) type Shared = Arc<Mutex<World>>;
    pub(crate) struct FakeI(pub(crate) Shared);
    pub(crate) struct FakeG(pub(crate) Shared);
    pub(crate) struct FakeK(pub(crate) Shared);

    pub(crate) fn w(s: &Shared) -> std::sync::MutexGuard<'_, World> {
        s.lock().unwrap()
    }

    impl Interceptor for FakeI {
        fn set_proxy(&mut self, rule: &str, t: &ProxyTarget) -> Result<(), String> {
            w(&self.0).log.push(format!("set_proxy {rule} {}", t.port));
            Ok(())
        }
        fn set_rule(&mut self, rule: &str, exe: &str, action: PbAction) -> Result<(), String> {
            let mut w = w(&self.0);
            w.log.push(format!("set_rule {rule} {exe} {action:?}"));
            if action != PbAction::Block && w.fail_rules.contains(rule) {
                return Err(format!("{rule} 坏了"));
            }
            w.rules.insert(rule.into(), (exe.into(), action));
            Ok(())
        }
        fn remove_rule(&mut self, rule: &str) -> Result<(), String> {
            let mut w = w(&self.0);
            w.log.push(format!("remove_rule {rule}"));
            w.rules.remove(rule);
            Ok(())
        }
        fn flows(&mut self) -> Vec<Flow> {
            let n = w(&self.0).flows;
            vec![
                Flow {
                    pid: 1,
                    rule_id: None,
                    exe_path: None,
                    host: "1.1.1.1".into(),
                    port: 443,
                    udp: false,
                    age_ms: None,
                    sent: 0,
                    received: 0,
                };
                n
            ]
        }
    }

    impl Guard for FakeG {
        fn ensure_block(&mut self, exe: &str) -> Result<(), String> {
            let mut w = w(&self.0);
            w.log.push(format!("ensure_block {exe}"));
            w.blocks.insert(exe_key(exe), exe.into());
            Ok(())
        }
        fn remove_block(&mut self, exe: &str) -> Result<(), String> {
            let mut w = w(&self.0);
            w.log.push(format!("remove_block {exe}"));
            w.blocks.remove(&exe_key(exe));
            Ok(())
        }
        fn permit(&mut self, exe: &str) -> Result<(), String> {
            let mut w = w(&self.0);
            w.log.push(format!("permit {exe}"));
            w.permits.insert(exe_key(exe));
            Ok(())
        }
        fn revoke(&mut self, exe: &str) -> Result<(), String> {
            let mut w = w(&self.0);
            w.log.push(format!("revoke {exe}"));
            w.permits.remove(&exe_key(exe));
            Ok(())
        }
        fn cleanup_all(&mut self) -> Result<(), String> {
            let mut w = w(&self.0);
            w.log.push("cleanup_all".into());
            w.blocks.clear();
            w.permits.clear();
            Ok(())
        }
        fn blocked_exes(&mut self) -> Result<Vec<String>, String> {
            Ok(w(&self.0).blocks.values().cloned().collect())
        }
    }

    impl ConnKiller for FakeK {
        fn reset_ipv4_tcp(&mut self, exe: &str) -> Result<u32, String> {
            w(&self.0).log.push(format!("reset {exe}"));
            Ok(0)
        }
    }

    pub(crate) type Srv = Server<FakeI, FakeG, FakeK>;

    pub(crate) fn rig(blocks: &[&str]) -> (Reconciler<FakeI, FakeG, FakeK>, Shared) {
        let world = Shared::default();
        for b in blocks {
            w(&world).blocks.insert(exe_key(b), b.to_string());
        }
        let rec = Reconciler::new(
            FakeI(world.clone()),
            FakeG(world.clone()),
            FakeK(world.clone()),
        );
        (rec, world)
    }

    /// 用假引擎启动服务：`blocks` 是启动前就存在的永久阻断。
    pub(crate) fn boot(dir: &Path, blocks: &[&str], clean: bool) -> (Srv, Shared) {
        let (rec, world) = rig(blocks);
        let srv = Server::boot(
            rec,
            dir.join(persist::STATE_FILE),
            "S-1-5-21-1".into(),
            clean,
        );
        (srv, world)
    }

    pub(crate) fn proxy() -> Action {
        Action::Proxy(ProxyTarget {
            kind: ProxyKind::Socks5,
            host: "127.0.0.1".into(),
            port: 7890,
            username: None,
            password: None,
        })
    }

    pub(crate) fn rule(id: &str, exe: &str, on_fault: FaultPolicy) -> RuleSpec {
        RuleSpec {
            id: id.into(),
            exe_path: exe.into(),
            action: proxy(),
            on_fault,
            reset_on_change: false,
        }
    }

    /// a：on_fault=Block；b：on_fault=Direct；两条都是 Proxy。
    pub(crate) fn state(exit: ExitPolicy) -> DesiredState {
        DesiredState {
            rules: vec![
                rule("a", A, FaultPolicy::Block),
                rule("b", B, FaultPolicy::Direct),
            ],
            on_routa_exit: exit,
            routa_pid: std::process::id(),
        }
    }

    pub(crate) fn action(world: &Shared, id: &str) -> Option<PbAction> {
        w(world).rules.get(id).map(|(_, a)| *a)
    }

    pub(crate) fn permitted(world: &Shared, exe: &str) -> bool {
        w(world).permits.contains(&exe_key(exe))
    }

    pub(crate) fn log(world: &Shared) -> Vec<String> {
        w(world).log.clone()
    }

    pub(crate) fn hello(srv: &mut Srv) -> Session {
        let mut s = srv.new_session();
        let r = srv.handle(
            Request::Hello {
                version: PROTO_VERSION,
            },
            &mut s,
        );
        assert_eq!(
            r,
            Response::Hello {
                version: PROTO_VERSION,
                accepted: true
            }
        );
        s
    }

    pub(crate) fn status(srv: &mut Srv, s: &mut Session) -> routa_procsvc_proto::SvcStatus {
        match srv.handle(Request::GetStatus, s) {
            Response::Status(st) => st,
            other => panic!("{other:?}"),
        }
    }

    pub(crate) fn applied(srv: &mut Srv, s: &mut Session, st: DesiredState) {
        assert_eq!(srv.handle(Request::Apply(st), s), Response::Ok);
    }

    /// 当前用户的 SID（非提权测试里 Administrators 只是拒绝项，DACL 要靠它放行）。
    pub(crate) fn current_user_sid() -> String {
        let out = std::process::Command::new("whoami")
            .args(["/user", "/fo", "csv", "/nh"])
            .output()
            .unwrap();
        let text = String::from_utf8_lossy(&out.stdout);
        let sid = text.trim().rsplit(',').next().unwrap().trim_matches('"');
        assert!(sid.starts_with("S-1-"), "{text}");
        sid.to_string()
    }

    pub(crate) fn unique_pipe_name() -> String {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT_ID: AtomicU64 = AtomicU64::new(0);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        // Windows 的系统时间可在并发调用中重复，不能单独用作实例标识。
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        format!(r"\\.\pipe\RoutaProcSvc-test-{}-{nanos}-{id}", std::process::id())
    }

    #[test]
    fn pipe_names_are_unique_across_threads() {
        let start = Arc::new(std::sync::Barrier::new(8));
        let workers: Vec<_> = (0..8)
            .map(|_| {
                let start = start.clone();
                std::thread::spawn(move || {
                    start.wait();
                    (0..1024).map(|_| unique_pipe_name()).collect::<Vec<_>>()
                })
            })
            .collect();
        let names: Vec<_> = workers.into_iter().flat_map(|w| w.join().unwrap()).collect();
        assert_eq!(
            names.iter().collect::<HashSet<_>>().len(),
            names.len(),
            "并发测试不能复用命名管道名"
        );
    }

    #[test]
    fn hello_version_mismatch_rejects_apply() {
        let dir = tempfile::tempdir().unwrap();
        let (mut srv, world) = boot(dir.path(), &[], true);
        let mut s = srv.new_session();

        // 未握手
        assert!(matches!(
            srv.handle(Request::Apply(state(ExitPolicy::Direct)), &mut s),
            Response::Error(_)
        ));

        let r = srv.handle(
            Request::Hello {
                version: PROTO_VERSION + 1,
            },
            &mut s,
        );
        assert_eq!(
            r,
            Response::Hello {
                version: PROTO_VERSION,
                accepted: false
            }
        );
        assert!(!s.hello_ok);
        assert_eq!(
            srv.handle(Request::Apply(state(ExitPolicy::Direct)), &mut s),
            Response::Error("版本不一致".into())
        );
        assert!(log(&world).is_empty(), "{:?}", log(&world));
        assert!(!dir.path().join(persist::STATE_FILE).exists());
    }

    #[test]
    fn corrupt_state_file_fails_closed() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(persist::STATE_FILE), "{oops").unwrap();
        for clean in [true, false] {
            let (_srv, world) = boot(dir.path(), &[A, B], clean);
            let l = log(&world);
            assert!(!l.iter().any(|x| x.starts_with("permit")), "{l:?}");
            assert!(!l.iter().any(|x| x.starts_with("remove_block")), "{l:?}");
            assert_eq!(w(&world).blocks.len(), 2);
        }
    }

    #[test]
    fn boot_with_saved_state_applies_exit_policy() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(persist::STATE_FILE);

        persist::save(&path, &state(ExitPolicy::Direct)).unwrap();
        let (mut srv, world) = boot(dir.path(), &[], true);
        assert_eq!(action(&world, "a"), Some(PbAction::Direct));
        assert_eq!(action(&world, "b"), Some(PbAction::Direct));
        assert!(permitted(&world, A));
        let mut s = hello(&mut srv);
        assert_eq!(status(&mut srv, &mut s).last_interruption, None);

        persist::save(&path, &state(ExitPolicy::Block)).unwrap();
        let (_srv, world) = boot(dir.path(), &[], true);
        assert_eq!(action(&world, "a"), Some(PbAction::Block));
        assert_eq!(action(&world, "b"), Some(PbAction::Block));
        assert!(!permitted(&world, A));
        assert!(w(&world).blocks.contains_key(&exe_key(A)));
    }

    #[test]
    fn boot_after_unclean_stop_uses_on_fault() {
        let dir = tempfile::tempdir().unwrap();
        persist::save(
            &dir.path().join(persist::STATE_FILE),
            &state(ExitPolicy::Direct),
        )
        .unwrap();
        let (mut srv, world) = boot(dir.path(), &[], false);
        assert_eq!(action(&world, "a"), Some(PbAction::Block));
        assert_eq!(action(&world, "b"), Some(PbAction::Direct));
        let l = log(&world);
        assert!(!l.iter().any(|x| x.starts_with("permit")), "{l:?}");
        let mut s = hello(&mut srv);
        let st = status(&mut srv, &mut s);
        assert_eq!(
            st.last_interruption.map(|i| i.reason),
            Some("分流服务意外退出后已重启".to_string())
        );
    }

    #[test]
    fn boot_removes_stale_blocks_not_in_state() {
        let dir = tempfile::tempdir().unwrap();
        persist::save(
            &dir.path().join(persist::STATE_FILE),
            &state(ExitPolicy::Block),
        )
        .unwrap();
        // a 受保护（保留，写法不同也算同一个）；b 是 on_fault=Direct、x 已不在状态里：都是残留
        let (_srv, world) = boot(dir.path(), &["c:/APP/A.EXE", B, r"C:\old\x.exe"], true);
        let keys: HashSet<String> = w(&world).blocks.keys().cloned().collect();
        assert_eq!(keys, HashSet::from([exe_key(A)]));
        let l = log(&world);
        assert!(l.contains(&format!("remove_block {B}")), "{l:?}");
        assert!(
            l.contains(&r"remove_block C:\old\x.exe".to_string()),
            "{l:?}"
        );
    }

    #[test]
    fn disconnect_without_goodbye_applies_on_fault() {
        let dir = tempfile::tempdir().unwrap();
        let (mut srv, world) = boot(dir.path(), &[], true);
        let mut s = hello(&mut srv);
        applied(&mut srv, &mut s, state(ExitPolicy::Block));
        assert_eq!(action(&world, "a"), Some(PbAction::Proxy));
        assert!(permitted(&world, A));
        assert_eq!(
            persist::load(&dir.path().join(persist::STATE_FILE)),
            Some(state(ExitPolicy::Block))
        );

        srv.end_session(&s);
        assert_eq!(action(&world, "a"), Some(PbAction::Block));
        assert_eq!(action(&world, "b"), Some(PbAction::Direct));
        assert!(!permitted(&world, A));
        let mut s2 = hello(&mut srv);
        assert!(status(&mut srv, &mut s2).last_interruption.is_some());
    }

    #[test]
    fn reconnect_registration_keeps_fallback_until_goodbye_without_reapplying() {
        let dir = tempfile::tempdir().unwrap();
        let (mut srv, world) = boot(dir.path(), &[], true);
        let mut s = hello(&mut srv);
        w(&world).fail_rules.insert("a".into());
        assert!(matches!(srv.handle(Request::Apply(state(ExitPolicy::Block)), &mut s), Response::Applied { .. }));
        w(&world).fail_rules.clear();
        srv.end_session(&s);
        assert_eq!(action(&world, "b"), Some(PbAction::Direct));
        let saved = persist::load(&dir.path().join(persist::STATE_FILE));
        for restart in [false, true] {
            if restart {
                srv = boot(dir.path(), &[], false).0;
            }
            let before = srv.reconciler.current().to_vec();
            let mut reconnected = hello(&mut srv);
            assert_eq!(srv.handle(Request::Attach { routa_pid: std::process::id() }, &mut reconnected), Response::Ok);
            assert_eq!(srv.reconciler.current(), before);
            assert_eq!(persist::load(&dir.path().join(persist::STATE_FILE)), saved);
            assert_eq!(srv.handle(Request::Goodbye, &mut reconnected), Response::Ok);
            srv.end_session(&reconnected);
            assert!(srv.reconciler.current().iter().all(|r| r.action == Action::Block));
        }
    }

    #[test]
    fn registration_rejects_dead_process_and_disconnect_still_uses_fault_policy() {
        let dir = tempfile::tempdir().unwrap();
        let (mut srv, world) = boot(dir.path(), &[], true);
        let mut s = hello(&mut srv);
        applied(&mut srv, &mut s, state(ExitPolicy::Block));
        srv.end_session(&s);
        let mut s = hello(&mut srv);
        assert!(matches!(srv.handle(Request::Attach { routa_pid: 0 }, &mut s), Response::Error(_)));
        assert!(srv.routa.is_none());
        assert_eq!(srv.handle(Request::Attach { routa_pid: std::process::id() }, &mut s), Response::Ok);
        srv.end_session(&s);
        assert_eq!(action(&world, "b"), Some(PbAction::Direct));
    }

    #[test]
    fn disconnect_after_goodbye_applies_exit_policy() {
        let dir = tempfile::tempdir().unwrap();
        let (mut srv, world) = boot(dir.path(), &[], true);
        let mut s = hello(&mut srv);
        applied(&mut srv, &mut s, state(ExitPolicy::Block));
        assert_eq!(srv.handle(Request::Goodbye, &mut s), Response::Ok);
        assert!(s.said_goodbye);
        srv.end_session(&s);
        assert_eq!(action(&world, "a"), Some(PbAction::Block));
        assert_eq!(action(&world, "b"), Some(PbAction::Block));
        let mut s2 = hello(&mut srv);
        assert_eq!(status(&mut srv, &mut s2).last_interruption, None);
    }

    #[test]
    fn system_ending_counts_as_graceful() {
        let dir = tempfile::tempdir().unwrap();
        let (mut srv, world) = boot(dir.path(), &[], true);
        let mut s = hello(&mut srv);
        applied(&mut srv, &mut s, state(ExitPolicy::Block));
        srv.system_ending();
        assert_eq!(action(&world, "b"), Some(PbAction::Block));
        // 关机时 Routa 随后被杀、管道断开：不再按崩溃处理
        let before = log(&world);
        srv.end_session(&s);
        srv.check_routa();
        assert_eq!(log(&world), before);
        assert_eq!(status(&mut srv, &mut s).last_interruption, None);
    }

    #[test]
    fn logoff_only_of_routa_session_falls_back() {
        let dir = tempfile::tempdir().unwrap();
        let (mut srv, world) = boot(dir.path(), &[], true);
        let mine = windows_session_of(std::process::id()).unwrap();
        // 没有在场的 Routa：什么都不做
        srv.session_logoff(mine);
        let mut s = hello(&mut srv);
        applied(&mut srv, &mut s, state(ExitPolicy::Block));
        let before = log(&world);
        srv.session_logoff(mine + 1);
        assert_eq!(log(&world), before);
        assert_eq!(action(&world, "b"), Some(PbAction::Proxy));
        // Routa 所在会话注销：按正常退出
        srv.session_logoff(mine);
        assert_eq!(action(&world, "a"), Some(PbAction::Block));
        assert_eq!(action(&world, "b"), Some(PbAction::Block));
        assert_eq!(status(&mut srv, &mut s).last_interruption, None);
    }

    #[test]
    fn routa_gone_applies_once_per_session() {
        let dir = tempfile::tempdir().unwrap();
        let (mut srv, world) = boot(dir.path(), &[], true);
        let mut s = hello(&mut srv);
        applied(&mut srv, &mut s, state(ExitPolicy::Direct));
        srv.end_session(&s);
        assert_eq!(action(&world, "a"), Some(PbAction::Block));
        let before = log(&world);
        srv.end_session(&s);
        srv.check_routa();
        srv.system_ending();
        assert_eq!(log(&world), before);

        // 新会话的 Hello + Apply 接管；旧会话的迟到事件不影响它
        let mut s2 = hello(&mut srv);
        applied(&mut srv, &mut s2, state(ExitPolicy::Direct));
        assert_eq!(action(&world, "a"), Some(PbAction::Proxy));
        srv.end_session(&s);
        assert_eq!(action(&world, "a"), Some(PbAction::Proxy));
    }

    fn spawn_routa() -> std::process::Child {
        std::process::Command::new("ping")
            .args(["-n", "60", "127.0.0.1"])
            .stdout(std::process::Stdio::null())
            .spawn()
            .unwrap()
    }

    #[test]
    fn routa_process_exit_applies_on_fault() {
        let dir = tempfile::tempdir().unwrap();
        let (mut srv, world) = boot(dir.path(), &[], true);
        let mut child = spawn_routa();
        let mut s = hello(&mut srv);
        let mut st = state(ExitPolicy::Block);
        st.routa_pid = child.id();
        applied(&mut srv, &mut s, st);
        srv.check_routa();
        assert_eq!(action(&world, "a"), Some(PbAction::Proxy));

        child.kill().unwrap();
        child.wait().unwrap();
        srv.check_routa();
        assert_eq!(action(&world, "a"), Some(PbAction::Block));
        assert_eq!(action(&world, "b"), Some(PbAction::Direct));
        // 随后管道断开：已处理过
        let before = log(&world);
        srv.end_session(&s);
        assert_eq!(log(&world), before);
    }

    #[test]
    fn apply_rejects_invalid_state() {
        type Edit = fn(&mut DesiredState);
        fn set_target(s: &mut DesiredState, f: fn(&mut ProxyTarget)) {
            if let Action::Proxy(t) = &mut s.rules[0].action {
                f(t);
            }
        }
        let cases: Vec<(&str, Edit, &str)> = vec![
            ("dup id", |s| s.rules[1].id = "a".into(), "规则 id 重复"),
            (
                "dup exe",
                |s| s.rules[1].exe_path = r"c:\APP\A.EXE".into(),
                "程序路径重复",
            ),
            (
                "relative",
                |s| s.rules[0].exe_path = r"app\a.exe".into(),
                "绝对路径",
            ),
            (
                "drive relative",
                |s| s.rules[0].exe_path = r"C:a.exe".into(),
                "绝对路径",
            ),
            (
                "unc",
                |s| s.rules[0].exe_path = r"\\srv\share\a.exe".into(),
                "网络路径",
            ),
            (
                "device path",
                |s| s.rules[0].exe_path = r"\\?\C:\app\a.exe".into(),
                "网络路径",
            ),
            (
                "not exe",
                |s| s.rules[0].exe_path = r"C:\app\a.bat".into(),
                ".exe",
            ),
            (
                "comma",
                |s| s.rules[0].exe_path = r"C:\a,b\a.exe".into(),
                "不能包含",
            ),
            (
                "semicolon",
                |s| s.rules[0].exe_path = r"C:\a;b\a.exe".into(),
                "不能包含",
            ),
            (
                "host name",
                |s| set_target(s, |t| t.host = "localhost".into()),
                "IP",
            ),
            ("port 0", |s| set_target(s, |t| t.port = 0), "端口"),
            (
                "33 proxy rules",
                |s| {
                    s.rules = (0..33)
                        .map(|i| {
                            rule(
                                &format!("r{i}"),
                                &format!(r"C:\app\{i}.exe"),
                                FaultPolicy::Direct,
                            )
                        })
                        .collect();
                },
                "32",
            ),
            ("dead routa", |s| s.routa_pid = 0, "Routa 进程"),
            (
                "installed service exe",
                |s| {
                    let p = crate::install::installed_exe().unwrap();
                    s.rules[0].exe_path = p.to_string_lossy().to_uppercase();
                },
                "组件自身",
            ),
            (
                "running service exe",
                |s| {
                    let p = std::env::current_exe().unwrap();
                    s.rules[1].exe_path = p.to_string_lossy().into_owned();
                },
                "组件自身",
            ),
        ];
        for (name, edit, want) in cases {
            let dir = tempfile::tempdir().unwrap();
            let (mut srv, world) = boot(dir.path(), &[], true);
            let mut s = hello(&mut srv);
            let mut st = state(ExitPolicy::Direct);
            edit(&mut st);
            match srv.handle(Request::Apply(st), &mut s) {
                Response::Error(e) => assert!(e.contains(want), "{name}: {e}"),
                other => panic!("{name}: {other:?}"),
            }
            assert!(log(&world).is_empty(), "{name}: {:?}", log(&world));
            assert!(!dir.path().join(persist::STATE_FILE).exists(), "{name}");
        }

        // 32 条代理规则 + 不计数的直连规则、IPv6 字面量：允许
        let dir = tempfile::tempdir().unwrap();
        let (mut srv, _world) = boot(dir.path(), &[], true);
        let mut s = hello(&mut srv);
        let mut st = state(ExitPolicy::Direct);
        st.rules = (0..34)
            .map(|i| {
                let mut r = rule(
                    &format!("r{i}"),
                    &format!(r"C:\app\{i}.exe"),
                    FaultPolicy::Direct,
                );
                if i >= 32 {
                    r.action = Action::Direct;
                }
                r
            })
            .collect();
        set_target(&mut st, |t| t.host = "::1".into());
        applied(&mut srv, &mut s, st);
    }

    #[test]
    fn apply_refused_when_state_cannot_be_saved() {
        let dir = tempfile::tempdir().unwrap();
        let (rec, world) = rig(&[]);
        // 状态文件所在目录不存在：保存必然失败
        let path = dir.path().join("missing").join(persist::STATE_FILE);
        let mut srv = Server::boot(rec, path, "S-1-5-21-1".into(), true);
        let mut s = hello(&mut srv);
        let before = log(&world);
        match srv.handle(Request::Apply(state(ExitPolicy::Direct)), &mut s) {
            Response::Error(e) => assert!(e.contains("保存状态失败"), "{e}"),
            other => panic!("{other:?}"),
        }
        assert_eq!(log(&world), before);
        // 没有接管：断开时不回落
        srv.end_session(&s);
        assert_eq!(log(&world), before);
    }

    #[test]
    fn apply_with_engine_errors_returns_applied() {
        let dir = tempfile::tempdir().unwrap();
        let (mut srv, world) = boot(dir.path(), &[], true);
        w(&world).fail_rules.insert("a".into());
        let mut s = hello(&mut srv);
        match srv.handle(Request::Apply(state(ExitPolicy::Direct)), &mut s) {
            Response::Applied { errors } => {
                assert!(errors.iter().any(|e| e.contains("a 坏了")), "{errors:?}")
            }
            other => panic!("{other:?}"),
        }
        // 已落地：失败的 a 按 Block，b 照常
        assert_eq!(action(&world, "a"), Some(PbAction::Block));
        assert_eq!(action(&world, "b"), Some(PbAction::Proxy));
        assert!(dir.path().join(persist::STATE_FILE).exists());
        let report = status(&mut srv, &mut s);
        assert_eq!(report.rules.iter().find(|r| r.id == "a").unwrap().action.as_deref(), Some("Block"));
        assert_eq!(report.rules.iter().find(|r| r.id == "b").unwrap().action.as_deref(), Some("Proxy"));
        w(&world).fail_rules.clear();
        assert_eq!(srv.handle(Request::RetryApply(state(ExitPolicy::Direct)), &mut s), Response::Ok);
        assert!(status(&mut srv, &mut s).rules.iter().all(|r| r.action.as_deref() == Some("Proxy")));
    }

    #[test]
    fn flows_and_status() {
        let dir = tempfile::tempdir().unwrap();
        let (mut srv, world) = boot(dir.path(), &[], true);
        let mut s = hello(&mut srv);
        w(&world).flows = 3;
        assert!(
            matches!(srv.handle(Request::GetFlows, &mut s), Response::Flows(f) if f.len() == 3)
        );
        let st = status(&mut srv, &mut s);
        assert!(!st.truncated_flows);
        assert_eq!(st.version, PROTO_VERSION);
        assert_eq!(st.owner_sid, "S-1-5-21-1");

        w(&world).flows = 400;
        assert!(
            matches!(srv.handle(Request::GetFlows, &mut s), Response::Flows(f) if f.len() == 400)
        );
        assert!(status(&mut srv, &mut s).truncated_flows);
    }

    #[test]
    fn cleanup_filters_removes_only_stale_blocks() {
        let dir = tempfile::tempdir().unwrap();
        let (mut srv, world) = boot(dir.path(), &[r"C:\old\x.exe"], true);
        let mut s = hello(&mut srv);
        applied(&mut srv, &mut s, state(ExitPolicy::Direct));
        assert_eq!(srv.handle(Request::CleanupFilters, &mut s), Response::Ok);
        let keys: HashSet<String> = w(&world).blocks.keys().cloned().collect();
        assert_eq!(keys, HashSet::from([exe_key(A)]));
        assert!(permitted(&world, A));
    }

    mod pipe {
        use super::*;
        use std::time::{Duration, Instant};
        use tokio::net::windows::named_pipe::{ClientOptions, NamedPipeClient};

        async fn open(name: &str) -> NamedPipeClient {
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                match ClientOptions::new().open(name) {
                    Ok(c) => return c,
                    Err(e) if Instant::now() < deadline => {
                        let _ = e;
                        tokio::time::sleep(Duration::from_millis(10)).await;
                    }
                    Err(e) => panic!("open {name}: {e}"),
                }
            }
        }

        async fn call(c: &mut NamedPipeClient, req: Request) -> Response {
            c.write_all(&encode(&req).unwrap()).await.unwrap();
            let mut header = [0u8; 4];
            c.read_exact(&mut header).await.unwrap();
            let mut body = vec![0u8; decode_len(header).unwrap()];
            c.read_exact(&mut body).await.unwrap();
            decode(&body).unwrap()
        }

        async fn eventually(what: &str, f: impl Fn() -> bool) {
            let deadline = Instant::now() + Duration::from_secs(5);
            while !f() {
                assert!(Instant::now() < deadline, "timed out: {what}");
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        }

        async fn hello(c: &mut NamedPipeClient) {
            let r = call(
                c,
                Request::Hello {
                    version: PROTO_VERSION,
                },
            )
            .await;
            assert_eq!(
                r,
                Response::Hello {
                    version: PROTO_VERSION,
                    accepted: true
                }
            );
        }

        #[tokio::test]
        async fn sessions_over_real_pipe() {
            let dir = tempfile::tempdir().unwrap();
            let (rec, world) = rig(&[]);
            let srv = Server::boot(
                rec,
                dir.path().join(persist::STATE_FILE),
                current_user_sid(),
                true,
            );
            let server = Arc::new(Mutex::new(srv));
            let name = unique_pipe_name();

            let client = async {
                // 会话 1：正常退出 → on_routa_exit
                let mut c = open(&name).await;
                hello(&mut c).await;
                assert_eq!(
                    call(&mut c, Request::Apply(state(ExitPolicy::Block))).await,
                    Response::Ok
                );
                assert_eq!(action(&world, "a"), Some(PbAction::Proxy));
                // 同时只服务一个客户端
                assert!(ClientOptions::new().open(&name).is_err());
                assert_eq!(call(&mut c, Request::Goodbye).await, Response::Ok);
                drop(c);
                eventually("graceful fallback", || {
                    action(&world, "b") == Some(PbAction::Block)
                })
                .await;

                // 会话 2（同一个管道实例复用）：Routa 进程退出、管道未断 → on_fault
                let mut routa = spawn_routa();
                let mut c = open(&name).await;
                hello(&mut c).await;
                let mut st = state(ExitPolicy::Block);
                st.routa_pid = routa.id();
                assert_eq!(call(&mut c, Request::Apply(st)).await, Response::Ok);
                assert_eq!(action(&world, "b"), Some(PbAction::Proxy));
                routa.kill().unwrap();
                routa.wait().unwrap();
                eventually("crash fallback", || {
                    action(&world, "a") == Some(PbAction::Block)
                        && action(&world, "b") == Some(PbAction::Direct)
                })
                .await;
                match call(&mut c, Request::GetStatus).await {
                    Response::Status(s) => assert_eq!(
                        s.last_interruption.map(|i| i.reason),
                        Some(ROUTA_CRASHED.to_string())
                    ),
                    other => panic!("{other:?}"),
                }
                // 非法帧正文：回 Error，连接保持
                let junk = b"{oops";
                c.write_all(&(junk.len() as u32).to_le_bytes())
                    .await
                    .unwrap();
                c.write_all(junk).await.unwrap();
                let mut header = [0u8; 4];
                c.read_exact(&mut header).await.unwrap();
                let mut body = vec![0u8; decode_len(header).unwrap()];
                c.read_exact(&mut body).await.unwrap();
                assert!(matches!(decode(&body).unwrap(), Response::Error(_)));
                assert!(matches!(
                    call(&mut c, Request::GetFlows).await,
                    Response::Flows(_)
                ));
            };

            tokio::select! {
                r = run_pipe(server.clone(), &name) => panic!("run_pipe ended: {r:?}"),
                () = client => {}
            }
        }

        #[tokio::test]
        async fn disconnects_client_without_hello() {
            let dir = tempfile::tempdir().unwrap();
            let (rec, _world) = rig(&[]);
            let srv = Server::boot(
                rec,
                dir.path().join(persist::STATE_FILE),
                current_user_sid(),
                true,
            );
            let server = Arc::new(Mutex::new(srv));
            let name = unique_pipe_name();

            let client = async {
                let mut c = open(&name).await;
                let start = Instant::now();
                let mut buf = [0u8; 1];
                let r = tokio::time::timeout(Duration::from_secs(10), c.read(&mut buf))
                    .await
                    .expect("服务应在握手超时后断开");
                assert!(matches!(r, Ok(0) | Err(_)), "{r:?}");
                let waited = start.elapsed();
                assert!(waited >= Duration::from_millis(4500), "{waited:?}");
                // 之后的客户端照常服务
                let mut c = open(&name).await;
                hello(&mut c).await;
            };

            tokio::select! {
                r = run_pipe(server.clone(), &name) => panic!("run_pipe ended: {r:?}"),
                () = client => {}
            }
        }

        #[tokio::test]
        async fn rejects_malformed_owner_sid() {
            let dir = tempfile::tempdir().unwrap();
            for sid in ["", "S-1-", "S-1-5)(A;;GA;;;WD", "WD"] {
                let (rec, _world) = rig(&[]);
                let srv = Server::boot(rec, dir.path().join(persist::STATE_FILE), sid.into(), true);
                let r = run_pipe(Arc::new(Mutex::new(srv)), &unique_pipe_name()).await;
                assert_eq!(
                    r.map_err(|e| e.kind()),
                    Err(io::ErrorKind::InvalidInput),
                    "{sid}"
                );
            }
        }
    }
}

//! 服务模式：由 SCM 启动，装配真实引擎并在 [`PIPE_NAME`] 上服务 Routa（spec 6.3）。
//!
//! - `STOP` / `PRESHUTDOWN`：按 Routa 正常退出回落（[`Server::system_ending`]），记"正常停止"，停止；
//! - `SESSIONCHANGE(WTS_SESSION_LOGOFF)`：只回落，服务继续运行；
//! - 启动失败或管道出错：以非 0 退出码停止、不记"正常停止"，由 SCM 故障恢复重启。

use std::ffi::OsString;
use std::io;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use routa_procsvc_proto::{PIPE_NAME, SERVICE_NAME};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver};
use windows_service::service::{
    ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus,
    ServiceType, SessionChangeReason,
};
use windows_service::service_control_handler::{
    self, ServiceControlHandlerResult, ServiceStatusHandle,
};
use windows_service::{define_windows_service, service_dispatcher};

use crate::engine::{ConnKiller, Guard, Interceptor};
use crate::install::{install_dir, OWNER_SID_FILE};
use crate::lazy::Lazy;
use crate::pb_ffi::{self, ProxyBridge};
use crate::persist;
use crate::reconcile::Reconciler;
use crate::server::{lock, run_pipe, Server};
use crate::tcpkill::TcpKiller;
use crate::wfp::{self, Wfp};

/// 服务特定退出码（`sc query` 的 SERVICE_EXIT_CODE），便于定位启动失败的原因。
const EXIT_OWNER_SID: u32 = 1;
const EXIT_PROXYBRIDGE: u32 = 3;
const EXIT_WFP: u32 = 4;
const EXIT_RUNTIME: u32 = 5;
const EXIT_PIPE: u32 = 6;

const START_WAIT_HINT: Duration = Duration::from_secs(30);
const STOP_WAIT_HINT: Duration = Duration::from_secs(10);

type RealServer = Server<Lazy<ProxyBridge>, Wfp, TcpKiller>;

define_windows_service!(ffi_service_main, service_main);

/// 交给 SCM 的服务分派器，服务停止后返回。不是由 SCM 启动时立即返回错误。
pub fn run() -> windows_service::Result<()> {
    service_dispatcher::start(SERVICE_NAME, ffi_service_main)
}

/// SCM 控制请求，经通道转交给服务主线程处理。
#[derive(Debug)]
enum Event {
    Stop,
    /// 该 Windows 会话注销。
    Logoff(u32),
}

fn service_main(_args: Vec<OsString>) {
    let (tx, rx) = unbounded_channel();
    let handler = move |control| match control {
        ServiceControl::Stop | ServiceControl::Preshutdown => {
            let _ = tx.send(Event::Stop);
            ServiceControlHandlerResult::NoError
        }
        ServiceControl::SessionChange(p) => {
            if p.reason == SessionChangeReason::SessionLogoff {
                let _ = tx.send(Event::Logoff(p.notification.session_id));
            }
            ServiceControlHandlerResult::NoError
        }
        ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
        _ => ServiceControlHandlerResult::NotImplemented,
    };
    let Ok(status) = service_control_handler::register(SERVICE_NAME, handler) else {
        return;
    };
    let code = run_service(&status, rx);
    let _ = status.set_service_status(service_status(ServiceState::Stopped, code));
}

/// 装配、运行直到被要求停止，返回退出码。返回前服务（及其引擎）已析构。
fn run_service(status: &ServiceStatusHandle, rx: UnboundedReceiver<Event>) -> ServiceExitCode {
    let _ = status.set_service_status(ServiceStatus {
        wait_hint: START_WAIT_HINT,
        ..service_status(ServiceState::StartPending, ServiceExitCode::NO_ERROR)
    });
    let dir = install_dir();
    let previous_stop_clean = persist::take_clean_stop(&dir);
    // 真实引擎含裸句柄、不是 Send：服务只在本线程（current_thread 运行时）上使用它，
    // Arc<Mutex> 只是 run_pipe 的接口形状，不跨线程。
    #[allow(clippy::arc_with_non_send_sync)]
    let server = match boot(&dir, previous_stop_clean) {
        Ok(s) => Arc::new(Mutex::new(s)),
        Err(code) => return ServiceExitCode::ServiceSpecific(code),
    };
    let Ok(rt) = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    else {
        return ServiceExitCode::ServiceSpecific(EXIT_RUNTIME);
    };
    let _ = status.set_service_status(ServiceStatus {
        controls_accepted: ServiceControlAccept::STOP
            | ServiceControlAccept::PRESHUTDOWN
            | ServiceControlAccept::SESSION_CHANGE,
        ..service_status(ServiceState::Running, ServiceExitCode::NO_ERROR)
    });

    if rt.block_on(serve(&server, PIPE_NAME, rx)).is_err() {
        return ServiceExitCode::ServiceSpecific(EXIT_PIPE);
    }
    let _ = status.set_service_status(ServiceStatus {
        wait_hint: STOP_WAIT_HINT,
        ..service_status(ServiceState::StopPending, ServiceExitCode::NO_ERROR)
    });
    // 写不了标记只会让下次启动按崩溃处理（更紧），无处上报
    let _ = persist::mark_clean_stop(&dir);
    ServiceExitCode::NO_ERROR
}

/// 用安装目录里的 owner.sid、ProxyBridgeCore.dll 和 WFP 装配服务。失败时返回服务特定退出码。
/// ProxyBridge 此时只加载不启动：有规则时才启动（见 [`Lazy`]）。
fn boot(dir: &Path, previous_stop_clean: bool) -> Result<RealServer, u32> {
    let owner_sid = std::fs::read_to_string(dir.join(OWNER_SID_FILE))
        .map(|s| s.trim().to_string())
        .map_err(|_| EXIT_OWNER_SID)?;
    let pb = pb_ffi::load(dir).map_err(|_| EXIT_PROXYBRIDGE)?;
    let wfp = wfp::open().map_err(|_| EXIT_WFP)?;
    Ok(Server::boot(
        Reconciler::new(Lazy::new(pb), wfp, TcpKiller),
        dir.join(persist::STATE_FILE),
        owner_sid,
        previous_stop_clean,
    ))
}

/// 在 `pipe_name` 上服务管道，同时处理 SCM 事件：
/// - 停止：按 Routa 正常退出回落一次，返回 `Ok`；
/// - 注销：该会话是在场 Routa 所在的会话时按正常退出回落，继续运行；
/// - 管道出错：返回 `Err`（不回落，由服务重启后的启动流程按崩溃处理）。
async fn serve<I, G, K>(
    server: &Arc<Mutex<Server<I, G, K>>>,
    pipe_name: &str,
    mut rx: UnboundedReceiver<Event>,
) -> io::Result<()>
where
    I: Interceptor,
    G: Guard,
    K: ConnKiller,
{
    let pipe = run_pipe(server.clone(), pipe_name);
    tokio::pin!(pipe);
    loop {
        tokio::select! {
            r = &mut pipe => {
                return Err(r.err().unwrap_or_else(|| io::Error::other("管道服务意外结束")));
            }
            ev = rx.recv() => match ev {
                Some(Event::Logoff(id)) => lock(server).session_logoff(id),
                Some(Event::Stop) | None => {
                    lock(server).system_ending();
                    return Ok(());
                }
            },
        }
    }
}

fn service_status(state: ServiceState, exit_code: ServiceExitCode) -> ServiceStatus {
    ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: state,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code,
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::PbAction;
    use crate::server::tests::{
        action, applied, current_user_sid, hello, log, rig, state, unique_pipe_name, Shared,
    };
    use crate::server::windows_session_of;
    use routa_procsvc_proto::ExitPolicy;
    use std::future::Future;
    use std::pin::Pin;
    use tokio::sync::mpsc::UnboundedSender;

    type TestServer = Arc<Mutex<crate::server::tests::Srv>>;

    /// 用假引擎启动服务，并以本进程为 Routa 下发 a(on_fault=Block)、b(on_fault=Direct)，
    /// on_routa_exit=Block。
    fn applied_server(dir: &Path, sid: String) -> (TestServer, Shared) {
        let (rec, world) = rig(&[]);
        let mut srv = Server::boot(rec, dir.join(persist::STATE_FILE), sid, true);
        let mut s = hello(&mut srv);
        applied(&mut srv, &mut s, state(ExitPolicy::Block));
        (Arc::new(Mutex::new(srv)), world)
    }

    /// 让 `serve` 跑一小会儿处理事件，断言它仍在运行。
    async fn settle(fut: &mut Pin<&mut impl Future<Output = io::Result<()>>>) {
        let r = tokio::time::timeout(Duration::from_millis(200), fut.as_mut()).await;
        assert!(r.is_err(), "serve ended early: {r:?}");
    }

    fn send(tx: &UnboundedSender<Event>, ev: Event) {
        tx.send(ev).unwrap();
    }

    #[tokio::test]
    async fn serve_handles_logoff_and_stop() {
        let dir = tempfile::tempdir().unwrap();
        let (server, world) = applied_server(dir.path(), current_user_sid());
        let mine = windows_session_of(std::process::id()).unwrap();
        let name = unique_pipe_name();
        let (tx, rx) = unbounded_channel();
        let fut = serve(&server, &name, rx);
        tokio::pin!(fut);
        settle(&mut fut).await;
        assert_eq!(action(&world, "b"), Some(PbAction::Proxy));

        // 别的会话注销：不回落
        let before = log(&world);
        send(&tx, Event::Logoff(mine + 1));
        settle(&mut fut).await;
        assert_eq!(log(&world), before);

        // Routa 所在会话注销：按 on_routa_exit 回落，服务继续
        send(&tx, Event::Logoff(mine));
        settle(&mut fut).await;
        assert_eq!(action(&world, "a"), Some(PbAction::Block));
        assert_eq!(action(&world, "b"), Some(PbAction::Block));
        let after_logoff = log(&world);
        assert_ne!(after_logoff, before);

        // 停止：Ok；Routa 已不在，不再重复回落
        send(&tx, Event::Stop);
        fut.await.unwrap();
        assert_eq!(log(&world), after_logoff);
    }

    #[tokio::test]
    async fn serve_stop_falls_back_exactly_once() {
        let dir = tempfile::tempdir().unwrap();
        let (server, world) = applied_server(dir.path(), current_user_sid());
        let (tx, rx) = unbounded_channel();
        send(&tx, Event::Stop);
        let before = log(&world).len();
        serve(&server, &unique_pipe_name(), rx).await.unwrap();
        assert_eq!(action(&world, "b"), Some(PbAction::Block));
        let after = log(&world);
        // 再要一次回落（例如随后的关机路径）：已无在场的 Routa，没有新操作
        lock(&server).system_ending();
        assert_eq!(log(&world), after);
        assert!(after.len() > before);
    }

    #[tokio::test]
    async fn serve_returns_err_when_pipe_fails() {
        let dir = tempfile::tempdir().unwrap();
        // 非法 SID：run_pipe 立即出错
        let (server, world) = applied_server(dir.path(), "WD".into());
        let (_tx, rx) = unbounded_channel();
        let before = log(&world);
        assert!(serve(&server, &unique_pipe_name(), rx).await.is_err());
        // 管道出错不回落（由重启后的启动流程处理）
        assert_eq!(log(&world), before);
    }
}

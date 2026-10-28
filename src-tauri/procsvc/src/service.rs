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
use crate::pb_ffi::{self, ProxyBridge};
use crate::persist;
use crate::reconcile::Reconciler;
use crate::server::{lock, run_pipe, Server};
use crate::tcpkill::TcpKiller;
use crate::wfp::{self, Wfp};

/// 服务特定退出码（`sc query` 的 SERVICE_EXIT_CODE），便于定位启动失败的原因。
const EXIT_OWNER_SID: u32 = 1;
const EXIT_RELAY_PORTS: u32 = 2;
const EXIT_PROXYBRIDGE: u32 = 3;
const EXIT_WFP: u32 = 4;
const EXIT_RUNTIME: u32 = 5;
const EXIT_PIPE: u32 = 6;

const START_WAIT_HINT: Duration = Duration::from_secs(30);
const STOP_WAIT_HINT: Duration = Duration::from_secs(10);

type RealServer = Server<ProxyBridge, Wfp, TcpKiller>;

define_windows_service!(ffi_service_main, service_main);

/// 交给 SCM 的服务分派器，服务停止后返回。不是由 SCM 启动时立即返回错误。
pub fn run() -> windows_service::Result<()> {
    service_dispatcher::start(SERVICE_NAME, ffi_service_main)
}

/// SCM 控制请求，经通道转交给服务主线程处理。
enum Event {
    Stop,
    Logoff,
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
                let _ = tx.send(Event::Logoff);
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

    if rt.block_on(serve(&server, rx)).is_err() {
        return ServiceExitCode::ServiceSpecific(EXIT_PIPE);
    }
    let _ = status.set_service_status(ServiceStatus {
        wait_hint: STOP_WAIT_HINT,
        ..service_status(ServiceState::StopPending, ServiceExitCode::NO_ERROR)
    });
    lock(&server).system_ending();
    // 写不了标记只会让下次启动按崩溃处理（更紧），无处上报
    let _ = persist::mark_clean_stop(&dir);
    ServiceExitCode::NO_ERROR
}

/// 用安装目录里的 owner.sid、ProxyBridgeCore.dll 和 WFP 装配服务。失败时返回服务特定退出码。
fn boot(dir: &Path, previous_stop_clean: bool) -> Result<RealServer, u32> {
    let owner_sid = std::fs::read_to_string(dir.join(OWNER_SID_FILE))
        .map(|s| s.trim().to_string())
        .map_err(|_| EXIT_OWNER_SID)?;
    let relay = pb_ffi::pick_relay_ports().map_err(|_| EXIT_RELAY_PORTS)?;
    let pb = pb_ffi::load(dir, relay).map_err(|_| EXIT_PROXYBRIDGE)?;
    let wfp = wfp::open().map_err(|_| EXIT_WFP)?;
    Ok(Server::boot(
        Reconciler::new(pb, wfp, TcpKiller),
        dir.join(persist::STATE_FILE),
        owner_sid,
        previous_stop_clean,
    ))
}

/// 服务管道，同时处理 SCM 事件。收到停止请求时返回 `Ok`；管道出错时返回 `Err`。
async fn serve<I, G, K>(
    server: &Arc<Mutex<Server<I, G, K>>>,
    mut rx: UnboundedReceiver<Event>,
) -> io::Result<()>
where
    I: Interceptor,
    G: Guard,
    K: ConnKiller,
{
    let pipe = run_pipe(server.clone(), PIPE_NAME);
    tokio::pin!(pipe);
    loop {
        tokio::select! {
            r = &mut pipe => {
                return Err(r.err().unwrap_or_else(|| io::Error::other("管道服务意外结束")));
            }
            ev = rx.recv() => match ev {
                Some(Event::Logoff) => lock(server).system_ending(),
                Some(Event::Stop) | None => return Ok(()),
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

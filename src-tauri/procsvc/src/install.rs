//! `--install` / `--uninstall` / `--cleanup`（spec 6.5、8）。均需管理员，均幂等。

use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::thread::sleep;
use std::time::{Duration, Instant};

use routa_procsvc_proto::SERVICE_NAME;
use windows_service::service::{
    Service, ServiceAccess, ServiceAction, ServiceActionType, ServiceErrorControl,
    ServiceFailureActions, ServiceFailureResetPeriod, ServiceInfo, ServiceStartType, ServiceState,
    ServiceType,
};
use windows_service::service_manager::{ServiceManager, ServiceManagerAccess};
use windows_sys::Win32::Foundation::{
    CloseHandle, LocalFree, ERROR_INSUFFICIENT_BUFFER, ERROR_SERVICE_ALREADY_RUNNING,
    ERROR_SERVICE_CANNOT_ACCEPT_CTRL, ERROR_SERVICE_DOES_NOT_EXIST,
    ERROR_SERVICE_MARKED_FOR_DELETE, ERROR_SERVICE_NOT_ACTIVE, HANDLE,
};
use windows_sys::Win32::Security::Authorization::{
    ConvertSecurityDescriptorToStringSecurityDescriptorW, SDDL_REVISION_1,
};
use windows_sys::Win32::Security::{
    GetTokenInformation, TokenElevation, DACL_SECURITY_INFORMATION, TOKEN_ELEVATION, TOKEN_QUERY,
};
use windows_sys::Win32::System::Services::{
    QueryServiceObjectSecurity, SetServiceObjectSecurity, SERVICE_QUERY_STATUS, SERVICE_START,
    SERVICE_STOP,
};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

use crate::engine::Guard;
use crate::paths::same_path;
use crate::server::SecurityDescriptor;
use crate::{pb_ffi, wfp};

/// 安装目录里记录组件所有者 SID 的文件。
pub const OWNER_SID_FILE: &str = "owner.sid";
const EXE_NAME: &str = "routa-procsvc.exe";
const DISPLAY_NAME: &str = "Routa 进程分流";
const DESCRIPTION: &str = "按进程把程序的网络连接转发到 Routa 的代理或阻断（Routa 进程分流组件）。";
/// 授予组件所有者的服务权限。
const OWNER_SERVICE_RIGHTS: u32 = SERVICE_START | SERVICE_STOP | SERVICE_QUERY_STATUS;
const STOP_TIMEOUT: Duration = Duration::from_secs(10);
const START_TIMEOUT: Duration = Duration::from_secs(10);
/// 服务刚停止时文件可能仍被占用：复制、删除的重试时长。
const FILE_RETRY: Duration = Duration::from_secs(5);
const POLL: Duration = Duration::from_millis(250);

/// `%ProgramFiles%\Routa\ProcessRouting`：组件文件、状态文件所在的受保护目录。
pub fn install_dir() -> PathBuf {
    let base = std::env::var_os("ProgramFiles")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Program Files"));
    base.join("Routa").join("ProcessRouting")
}

/// 复制到安装目录的文件。第一个（服务 exe）取自正在运行的 exe，其余取自 `--source`。
pub fn files_to_copy() -> [&'static str; 4] {
    [
        EXE_NAME,
        pb_ffi::DLL_NAME,
        "WinDivert.dll",
        "WinDivert64.sys",
    ]
}

/// 本机或域用户的 SID：`S-1-5-21-` 后跟 4 段十进制数。
pub fn valid_owner_sid(sid: &str) -> bool {
    sid.strip_prefix("S-1-5-21-").is_some_and(|rest| {
        let parts: Vec<&str> = rest.split('-').collect();
        parts.len() == 4
            && parts
                .iter()
                .all(|p| p.parse::<u32>().is_ok() && !p.starts_with('+'))
    })
}

/// 当前进程是否以提升后的管理员身份运行。
pub fn is_elevated() -> bool {
    let mut token: HANDLE = std::ptr::null_mut();
    let mut elevation = TOKEN_ELEVATION { TokenIsElevated: 0 };
    let mut len = 0u32;
    // SAFETY: 伪句柄无需关闭；token 只在本函数内使用并关闭；elevation/len 是可写的局部变量。
    unsafe {
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return false;
        }
        let ok = GetTokenInformation(
            token,
            TokenElevation,
            &mut elevation as *mut _ as *mut _,
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut len,
        );
        CloseHandle(token);
        ok != 0 && elevation.TokenIsElevated != 0
    }
}

/// 停止旧服务 → 复制文件 → 写 owner.sid → 创建或更新服务（自动启动、LocalSystem）
/// → 服务 DACL 给所有者启动/停止/查询 → 故障恢复 → 启动服务并等它进入运行状态。
pub fn install(owner_sid: &str, source: &Path) -> Result<(), String> {
    if !valid_owner_sid(owner_sid) {
        return Err(format!("无效的所有者 SID：{owner_sid}"));
    }
    let exe = std::env::current_exe().map_err(|e| format!("无法确定本程序路径：{e}"))?;
    let [exe_name, rest @ ..] = files_to_copy();
    let mut copies = vec![(exe, exe_name)];
    for name in rest {
        let src = source.join(name);
        if !src.is_file() {
            return Err(format!("找不到组件文件：{}", src.display()));
        }
        copies.push((src, name));
    }

    let mgr = manager(ServiceManagerAccess::CONNECT | ServiceManagerAccess::CREATE_SERVICE)?;
    if let Some(svc) = open(&mgr, ServiceAccess::STOP | ServiceAccess::QUERY_STATUS)? {
        stop_and_wait(&svc)?;
    }

    let dir = install_dir();
    fs::create_dir_all(&dir).map_err(|e| format!("创建 {} 失败：{e}", dir.display()))?;
    for (src, name) in copies {
        let dst = dir.join(name);
        // 用已安装的 exe 重新安装：就是同一个文件
        if same_path(&src.to_string_lossy(), &dst.to_string_lossy()) {
            continue;
        }
        retry(|| fs::copy(&src, &dst).map(drop))
            .map_err(|e| format!("复制 {} 到 {} 失败：{e}", src.display(), dst.display()))?;
    }
    let sid_file = dir.join(OWNER_SID_FILE);
    fs::write(&sid_file, owner_sid)
        .map_err(|e| format!("写入 {} 失败：{e}", sid_file.display()))?;

    let info = ServiceInfo {
        name: SERVICE_NAME.into(),
        display_name: DISPLAY_NAME.into(),
        service_type: ServiceType::OWN_PROCESS,
        start_type: ServiceStartType::AutoStart,
        error_control: ServiceErrorControl::Normal,
        executable_path: dir.join(EXE_NAME),
        launch_arguments: Vec::<OsString>::new(),
        dependencies: Vec::new(),
        account_name: None,
        account_password: None,
    };
    let access = ServiceAccess::QUERY_STATUS
        | ServiceAccess::START
        | ServiceAccess::CHANGE_CONFIG
        | ServiceAccess::READ_CONTROL
        | ServiceAccess::WRITE_DAC;
    let svc = match open(&mgr, access)? {
        Some(svc) => {
            svc.change_config(&info)
                .map_err(|e| failed("更新服务配置", e))?;
            svc
        }
        None => mgr
            .create_service(&info, access)
            .map_err(|e| failed("创建服务", e))?,
    };
    svc.set_description(DESCRIPTION)
        .map_err(|e| failed("设置服务描述", e))?;
    grant_owner(&svc, owner_sid)?;
    let restart = |secs| ServiceAction {
        action_type: ServiceActionType::Restart,
        delay: Duration::from_secs(secs),
    };
    svc.update_failure_actions(ServiceFailureActions {
        reset_period: ServiceFailureResetPeriod::After(Duration::from_secs(24 * 60 * 60)),
        reboot_msg: None,
        command: None,
        actions: Some(vec![restart(1), restart(5), restart(30)]),
    })
    .map_err(|e| failed("设置故障恢复", e))?;
    // 服务以非 0 退出码自行停止（例如管道出错）时也按故障恢复重启
    svc.set_failure_actions_on_non_crash_failures(true)
        .map_err(|e| failed("设置故障恢复", e))?;

    match svc.start::<&str>(&[]) {
        Err(e) if os_code(&e) == Some(ERROR_SERVICE_ALREADY_RUNNING) => {}
        r => r.map_err(|e| failed("启动服务", e))?,
    }
    wait_running(&svc)
}

/// 停止并删除服务 → 删除全部 Routa WFP 过滤器 → 删除安装目录。尽量做完每一步，最后汇总错误。
pub fn uninstall() -> Result<(), String> {
    let mut errors = Vec::new();
    let mgr = manager(ServiceManagerAccess::CONNECT)?;
    let access = ServiceAccess::STOP | ServiceAccess::QUERY_STATUS | ServiceAccess::DELETE;
    if let Some(svc) = open(&mgr, access)? {
        if let Err(e) = stop_and_wait(&svc) {
            errors.push(e);
        }
        match svc.delete() {
            Err(e) if os_code(&e) != Some(ERROR_SERVICE_MARKED_FOR_DELETE) => {
                errors.push(failed("删除服务", e))
            }
            _ => {}
        }
    }
    if let Err(e) = cleanup_filters() {
        errors.push(e);
    }
    let dir = install_dir();
    match retry(|| fs::remove_dir_all(&dir)) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => {
            errors.push(format!("删除 {} 失败：{e}", dir.display()))
        }
        _ => {}
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("；"))
    }
}

/// 停止服务（如在运行，之后不再启动）并删除全部 Routa WFP 过滤器。
pub fn cleanup() -> Result<(), String> {
    let mgr = manager(ServiceManagerAccess::CONNECT)?;
    if let Some(svc) = open(&mgr, ServiceAccess::STOP | ServiceAccess::QUERY_STATUS)? {
        stop_and_wait(&svc)?;
    }
    cleanup_filters()
}

fn cleanup_filters() -> Result<(), String> {
    wfp::open()
        .and_then(|mut w| w.cleanup_all())
        .map_err(|e| format!("清理 WFP 过滤器失败：{e}"))
}

fn manager(access: ServiceManagerAccess) -> Result<ServiceManager, String> {
    ServiceManager::local_computer(None::<&str>, access).map_err(|e| failed("连接服务管理器", e))
}

/// 打开本服务；不存在时为 None。
fn open(mgr: &ServiceManager, access: ServiceAccess) -> Result<Option<Service>, String> {
    match mgr.open_service(SERVICE_NAME, access) {
        Ok(svc) => Ok(Some(svc)),
        Err(e) if os_code(&e) == Some(ERROR_SERVICE_DOES_NOT_EXIST) => Ok(None),
        Err(e) => Err(failed("打开服务", e)),
    }
}

/// 停止服务并等到它停下（最多 [`STOP_TIMEOUT`]）。正在启动时等它启动完再停。
fn stop_and_wait(svc: &Service) -> Result<(), String> {
    let deadline = Instant::now() + STOP_TIMEOUT;
    loop {
        let state = svc
            .query_status()
            .map_err(|e| failed("查询服务状态", e))?
            .current_state;
        if state == ServiceState::Stopped {
            return Ok(());
        }
        if state == ServiceState::Running {
            match svc.stop() {
                Err(e)
                    if !matches!(
                        os_code(&e),
                        Some(ERROR_SERVICE_NOT_ACTIVE | ERROR_SERVICE_CANNOT_ACCEPT_CTRL)
                    ) =>
                {
                    return Err(failed("停止服务", e))
                }
                _ => {}
            }
        }
        if Instant::now() >= deadline {
            return Err(format!("服务在 {} 秒内没有停止", STOP_TIMEOUT.as_secs()));
        }
        sleep(POLL);
    }
}

/// 等服务进入运行状态（最多 [`START_TIMEOUT`]）。
fn wait_running(svc: &Service) -> Result<(), String> {
    let deadline = Instant::now() + START_TIMEOUT;
    loop {
        let status = svc.query_status().map_err(|e| failed("查询服务状态", e))?;
        match status.current_state {
            ServiceState::Running => return Ok(()),
            ServiceState::Stopped => {
                return Err(format!("服务启动失败（退出码 {:?}）", status.exit_code))
            }
            _ if Instant::now() >= deadline => {
                return Err(format!(
                    "服务在 {} 秒内没有进入运行状态",
                    START_TIMEOUT.as_secs()
                ))
            }
            _ => sleep(POLL),
        }
    }
}

/// 在服务现有 DACL 上加一条所有者 ACE（启动、停止、查询状态）。
fn grant_owner(svc: &Service, owner_sid: &str) -> Result<(), String> {
    let current = service_dacl(svc).map_err(|e| format!("读取服务权限失败：{e}"))?;
    let sd = SecurityDescriptor::from_sddl(&with_owner_ace(&current, owner_sid))
        .map_err(|e| format!("生成服务权限失败：{e}"))?;
    // SAFETY: 服务句柄由 svc 持有且有 WRITE_DAC；sd 是有效的自相对安全描述符。
    let ok = unsafe { SetServiceObjectSecurity(svc.raw_handle(), DACL_SECURITY_INFORMATION, sd.0) };
    if ok == 0 {
        return Err(format!("设置服务权限失败：{}", io::Error::last_os_error()));
    }
    Ok(())
}

/// 服务当前 DACL 的 SDDL 形式。
fn service_dacl(svc: &Service) -> io::Result<String> {
    let h = svc.raw_handle();
    let mut buf: Vec<u8> = Vec::new();
    let mut needed = 0u32;
    // SAFETY: 第一次以空缓冲区询问所需大小，第二次 buf 长度即为 needed。
    unsafe {
        if QueryServiceObjectSecurity(
            h,
            DACL_SECURITY_INFORMATION,
            std::ptr::null_mut(),
            0,
            &mut needed,
        ) == 0
        {
            let e = io::Error::last_os_error();
            if e.raw_os_error() != Some(ERROR_INSUFFICIENT_BUFFER as i32) {
                return Err(e);
            }
        }
        buf.resize(needed as usize, 0);
        if QueryServiceObjectSecurity(
            h,
            DACL_SECURITY_INFORMATION,
            buf.as_mut_ptr() as *mut _,
            needed,
            &mut needed,
        ) == 0
        {
            return Err(io::Error::last_os_error());
        }
        let mut s: *mut u16 = std::ptr::null_mut();
        if ConvertSecurityDescriptorToStringSecurityDescriptorW(
            buf.as_mut_ptr() as *mut _,
            SDDL_REVISION_1,
            DACL_SECURITY_INFORMATION,
            &mut s,
            std::ptr::null_mut(),
        ) == 0
        {
            return Err(io::Error::last_os_error());
        }
        let len = (0..).take_while(|&i| *s.add(i) != 0).count();
        let sddl = String::from_utf16_lossy(std::slice::from_raw_parts(s, len));
        LocalFree(s as *mut _);
        Ok(sddl)
    }
}

/// 去掉 DACL（SDDL）里已有的 `sid` 的 ACE，再追加所有者 ACE，所以重复安装结果不变。
fn with_owner_ace(sddl: &str, sid: &str) -> String {
    let start = sddl.find('(').unwrap_or(sddl.len());
    let mut out = sddl[..start].to_string();
    for ace in sddl[start..].split_inclusive(')') {
        let trustee = ace.trim_end_matches(')').rsplit(';').next().unwrap_or("");
        if !trustee.eq_ignore_ascii_case(sid) {
            out.push_str(ace);
        }
    }
    out.push_str(&format!("(A;;{OWNER_SERVICE_RIGHTS:#x};;;{sid})"));
    out
}

/// 在 [`FILE_RETRY`] 内重试文件操作（服务刚停止时文件可能仍被占用）；`NotFound` 不重试。
fn retry(mut f: impl FnMut() -> io::Result<()>) -> io::Result<()> {
    let deadline = Instant::now() + FILE_RETRY;
    loop {
        match f() {
            Err(e) if e.kind() != io::ErrorKind::NotFound && Instant::now() < deadline => {
                sleep(POLL)
            }
            r => return r,
        }
    }
}

fn os_code(e: &windows_service::Error) -> Option<u32> {
    match e {
        windows_service::Error::Winapi(e) => e.raw_os_error().map(|c| c as u32),
        _ => None,
    }
}

/// windows_service 的 `Winapi` 错误本身只显示 "IO error in winapi call"，这里展开系统错误信息。
fn failed(what: &str, e: windows_service::Error) -> String {
    match e {
        windows_service::Error::Winapi(e) => format!("{what}失败：{e}"),
        e => format!("{what}失败：{e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_plan_paths() {
        let base = PathBuf::from(std::env::var_os("ProgramFiles").unwrap());
        let dir = install_dir();
        assert_eq!(dir, base.join(r"Routa\ProcessRouting"));
        assert!(dir.ends_with(r"Routa\ProcessRouting"));
        assert_eq!(
            files_to_copy(),
            [
                "routa-procsvc.exe",
                "ProxyBridgeCore.dll",
                "WinDivert.dll",
                "WinDivert64.sys"
            ]
        );
    }

    #[test]
    fn owner_sid_validation() {
        assert!(valid_owner_sid(
            "S-1-5-21-1004336348-1177238915-682003330-1001"
        ));
        for bad in [
            "",
            "S-1-5-18",
            "S-1-5-21-1-2-3",
            "S-1-5-21-1-2-3-4-5",
            "S-1-5-21-1-2-3-x",
            "S-1-5-21-1-2-3-+4",
            "S-1-5-21-1-2-3-4)(A;;GA;;;WD",
            "S-1-5-21-1-2-3-99999999999",
        ] {
            assert!(!valid_owner_sid(bad), "{bad}");
        }
    }

    #[test]
    fn owner_ace_appended_once() {
        let sid = "S-1-5-21-1-2-3-1001";
        let base = "D:(A;;CCLCSWRPWPDTLOCRRC;;;SY)(A;;CCLCSWLOCRRC;;;IU)";
        let once = with_owner_ace(base, sid);
        assert_eq!(once, format!("{base}(A;;0x34;;;{sid})"));
        // 重复安装（SID 大小写不同也算同一个）
        assert_eq!(with_owner_ace(&once, sid), once);
        assert_eq!(
            with_owner_ace(&format!("{base}(A;;LC;;;s-1-5-21-1-2-3-1001)"), sid),
            once
        );
        assert_eq!(with_owner_ace("D:", sid), format!("D:(A;;0x34;;;{sid})"));
    }
}

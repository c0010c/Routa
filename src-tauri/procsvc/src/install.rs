//! `--install` / `--uninstall` / `--cleanup`（spec 6.5、8）。均需管理员，均幂等。

use std::ffi::OsString;
use std::fs::{self, OpenOptions};
use std::io;
use std::os::windows::ffi::OsStringExt;
use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
use std::os::windows::io::AsRawHandle;
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
use windows_sys::core::PWSTR;
use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_SERVICE_ALREADY_RUNNING, ERROR_SERVICE_CANNOT_ACCEPT_CTRL,
    ERROR_SERVICE_DOES_NOT_EXIST, ERROR_SERVICE_MARKED_FOR_DELETE, ERROR_SERVICE_NOT_ACTIVE,
    HANDLE,
};
use windows_sys::Win32::Security::Authorization::{SetSecurityInfo, SE_FILE_OBJECT};
use windows_sys::Win32::Security::{
    GetSecurityDescriptorDacl, GetTokenInformation, TokenElevation, ACL, DACL_SECURITY_INFORMATION,
    PROTECTED_DACL_SECURITY_INFORMATION, TOKEN_ELEVATION, TOKEN_QUERY,
};
use windows_sys::Win32::Storage::FileSystem::{
    FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
    FILE_SHARE_READ, FILE_SHARE_WRITE, READ_CONTROL, WRITE_DAC,
};
use windows_sys::Win32::System::Com::CoTaskMemFree;
use windows_sys::Win32::System::Services::{
    SetServiceObjectSecurity, SERVICE_QUERY_STATUS, SERVICE_START, SERVICE_STOP,
};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows_sys::Win32::UI::Shell::{FOLDERID_ProgramFiles, SHGetKnownFolderPath, KF_FLAG_DEFAULT};

use crate::engine::Guard;
use crate::localfile::{self, Expect};
use crate::paths::same_path;
use crate::server::SecurityDescriptor;
use crate::{pb_ffi, wfp};

/// 安装目录里记录组件所有者 SID 的文件。
pub const OWNER_SID_FILE: &str = "owner.sid";
const EXE_NAME: &str = "routa-procsvc.exe";
const DISPLAY_NAME: &str = "Routa 进程分流";
const DESCRIPTION: &str = "按进程把程序的网络连接转发到 Routa 的代理或阻断（Routa 进程分流组件）。";
/// SCM 给新建服务的默认 DACL：SYSTEM 管理、Administrators 完全控制、交互/服务登录用户只读。
const DEFAULT_SERVICE_DACL: &str = "D:(A;;CCLCSWRPWPDTLOCRRC;;;SY)     (A;;CCDCLCSWRPWPDTLOCRSDRCWDWO;;;BA)(A;;CCLCSWLOCRRC;;;IU)(A;;CCLCSWLOCRRC;;;SU)";
/// 安装目录的 DACL：受保护（不继承上级）；SYSTEM、Administrators 完全控制，Users 只读与执行
/// （0x1200a9 = FILE_GENERIC_READ | FILE_GENERIC_EXECUTE）。对象与子目录都继承。
pub const INSTALL_DIR_SDDL: &str = "D:PAI(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)(A;OICI;0x1200a9;;;BU)";
/// 授予组件所有者的服务权限。
const OWNER_SERVICE_RIGHTS: u32 = SERVICE_START | SERVICE_STOP | SERVICE_QUERY_STATUS;
const STOP_TIMEOUT: Duration = Duration::from_secs(10);
const START_TIMEOUT: Duration = Duration::from_secs(10);
/// 服务刚停止时文件可能仍被占用：复制、删除的重试时长。
const FILE_RETRY: Duration = Duration::from_secs(5);
const POLL: Duration = Duration::from_millis(250);

/// `%ProgramFiles%\Routa\ProcessRouting`：组件文件、状态文件所在的受保护目录。
///
/// Program Files 取自已知文件夹（`FOLDERID_ProgramFiles`）而不是环境变量：安装命令和
/// LocalSystem 服务必须得到同一个目录，且用户改环境变量不能把服务的二进制路径引到可写目录。
/// 取不到时报错，不猜测位置（安装、卸载、服务启动都随之失败）。
pub fn install_dir() -> Result<PathBuf, String> {
    Ok(program_files()?.join("Routa").join("ProcessRouting"))
}

/// 安装后的服务 exe：`<安装目录>\routa-procsvc.exe`。
pub fn installed_exe() -> Result<PathBuf, String> {
    Ok(install_dir()?.join(EXE_NAME))
}

fn program_files() -> Result<PathBuf, String> {
    let mut p: PWSTR = std::ptr::null_mut();
    // SAFETY: 成功与否 p 都要用 CoTaskMemFree 释放（失败时为 null，释放 null 无害）；
    // 成功时 p 是以 NUL 结尾的宽字符串。
    unsafe {
        let hr = SHGetKnownFolderPath(
            &FOLDERID_ProgramFiles,
            KF_FLAG_DEFAULT as u32,
            std::ptr::null_mut(),
            &mut p,
        );
        let path = (hr >= 0 && !p.is_null()).then(|| {
            let len = (0..).take_while(|&i| *p.add(i) != 0).count();
            PathBuf::from(OsString::from_wide(std::slice::from_raw_parts(p, len)))
        });
        CoTaskMemFree(p as *const _);
        path.ok_or_else(|| format!("无法确定 Program Files 目录（HRESULT {hr:#010x}）"))
    }
}

/// 给安装目录设上 [`INSTALL_DIR_SDDL`]（目录已存在也重设）。路径上任何一层是重解析点
/// （符号链接、目录联接、挂载点）都拒绝：否则文件可能被写到别处，或 DACL 设到别的目录上。
fn protect_dir(dir: &Path) -> Result<(), String> {
    // 检查各层并占住（不能被改名或删除），直到 DACL 设完。
    let _held = localfile::hold(dir, Expect::Dir)?;
    let f = OpenOptions::new()
        .access_mode(READ_CONTROL | WRITE_DAC)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS)
        .open(dir)
        .map_err(|e| format!("打开 {} 失败：{e}", dir.display()))?;
    let attrs = f
        .metadata()
        .map_err(|e| format!("读取 {} 的属性失败：{e}", dir.display()))?
        .file_attributes();
    if attrs & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err(format!("{} 是重解析点，出于安全原因拒绝", dir.display()));
    }
    set_dacl(&f, INSTALL_DIR_SDDL).map_err(|e| format!("设置 {} 的权限失败：{e}", dir.display()))
}

/// 把 `sddl` 里的 DACL 设为 `f` 的受保护 DACL（系统会把可继承的 ACE 传播到已有的子项）。
fn set_dacl(f: &fs::File, sddl: &str) -> Result<(), String> {
    let sd = SecurityDescriptor::from_sddl(sddl).map_err(|e| e.to_string())?;
    let (mut present, mut defaulted) = (0, 0);
    let mut dacl: *mut ACL = std::ptr::null_mut();
    // SAFETY: sd 有效；dacl 指向 sd 内部，在 sd 存活期间使用。
    if unsafe { GetSecurityDescriptorDacl(sd.0, &mut present, &mut dacl, &mut defaulted) } == 0
        || present == 0
        || dacl.is_null()
    {
        return Err("安全描述符里没有 DACL".into());
    }
    // SAFETY: f 是以 WRITE_DAC 打开的有效句柄；dacl 有效。
    let err = unsafe {
        SetSecurityInfo(
            f.as_raw_handle() as _,
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            dacl,
            std::ptr::null(),
        )
    };
    if err != 0 {
        return Err(io::Error::from_raw_os_error(err as i32).to_string());
    }
    Ok(())
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
    ["S-1-5-21-", "S-1-12-1-"].iter().any(|prefix| {
        sid.strip_prefix(prefix).is_some_and(|rest| {
            let parts: Vec<&str> = rest.split('-').collect();
            parts.len() == 4
                && parts
                    .iter()
                    .all(|p| p.parse::<u32>().is_ok() && !p.starts_with('+'))
        })
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

    let dir = install_dir()?;
    fs::create_dir_all(&dir).map_err(|e| format!("创建 {} 失败：{e}", dir.display()))?;
    protect_dir(&dir)?;
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
    match install_dir() {
        Ok(dir) => match retry(|| fs::remove_dir_all(&dir)) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => {
                errors.push(format!("删除 {} 失败：{e}", dir.display()))
            }
            _ => {}
        },
        Err(e) => errors.push(e),
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

/// 把服务 DACL 设为 [`service_sddl`]。
fn grant_owner(svc: &Service, owner_sid: &str) -> Result<(), String> {
    let sd = SecurityDescriptor::from_sddl(&service_sddl(owner_sid))
        .map_err(|e| format!("生成服务权限失败：{e}"))?;
    // SAFETY: 服务句柄由 svc 持有且有 WRITE_DAC；sd 是有效的自相对安全描述符。
    let ok = unsafe { SetServiceObjectSecurity(svc.raw_handle(), DACL_SECURITY_INFORMATION, sd.0) };
    if ok == 0 {
        return Err(format!("设置服务权限失败：{}", io::Error::last_os_error()));
    }
    Ok(())
}

/// 服务 DACL：SCM 给新服务的默认 DACL（[`DEFAULT_SERVICE_DACL`]）加一条所有者 ACE
/// （启动、停止、查询状态）。每次安装都从固定基线生成，换所有者重新安装时旧所有者的权限随之消失。
fn service_sddl(owner_sid: &str) -> String {
    format!("{DEFAULT_SERVICE_DACL}(A;;{OWNER_SERVICE_RIGHTS:#x};;;{owner_sid})")
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
        // 测试进程未改环境变量：已知文件夹与 ProgramFiles 一致
        let base = PathBuf::from(std::env::var_os("ProgramFiles").unwrap());
        assert!(base.is_absolute());
        let dir = install_dir().unwrap();
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

    /// 句柄上当前 DACL 的 SDDL（含 P/AI 标志）。
    fn dacl_sddl_of(dir: &Path) -> String {
        use windows_sys::Win32::Foundation::LocalFree;
        use windows_sys::Win32::Security::Authorization::{
            ConvertSecurityDescriptorToStringSecurityDescriptorW, GetSecurityInfo, SDDL_REVISION_1,
        };
        let f = OpenOptions::new()
            .access_mode(READ_CONTROL)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
            .open(dir)
            .unwrap();
        let mut sd = std::ptr::null_mut();
        let mut s: PWSTR = std::ptr::null_mut();
        let mut len = 0u32;
        // SAFETY: 句柄有效；sd、s 由系统 LocalAlloc，用后释放。
        unsafe {
            let err = GetSecurityInfo(
                f.as_raw_handle() as _,
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut sd,
            );
            assert_eq!(err, 0);
            let ok = ConvertSecurityDescriptorToStringSecurityDescriptorW(
                sd,
                SDDL_REVISION_1,
                DACL_SECURITY_INFORMATION,
                &mut s,
                &mut len,
            );
            assert_ne!(ok, 0);
            let out = String::from_utf16_lossy(std::slice::from_raw_parts(s, len as usize));
            LocalFree(s as _);
            LocalFree(sd);
            out.trim_end_matches('\0').to_string()
        }
    }

    #[test]
    fn install_dir_sddl_is_protected_and_parses() {
        use windows_sys::Win32::Security::{GetSecurityDescriptorControl, SE_DACL_PROTECTED};
        assert_eq!(
            INSTALL_DIR_SDDL,
            "D:PAI(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)(A;OICI;0x1200a9;;;BU)"
        );
        let sd = SecurityDescriptor::from_sddl(INSTALL_DIR_SDDL).unwrap();
        let (mut control, mut rev) = (0u16, 0u32);
        // SAFETY: sd 有效。
        assert_ne!(
            unsafe { GetSecurityDescriptorControl(sd.0, &mut control, &mut rev) },
            0
        );
        assert_ne!(control & SE_DACL_PROTECTED, 0);
        assert!(set_dacl(
            &fs::File::open(std::env::current_exe().unwrap()).unwrap(),
            "not sddl"
        )
        .is_err());
    }

    #[test]
    fn protect_dir_sets_dacl_and_refuses_reparse_points() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("ProcessRouting");
        fs::create_dir(&dir).unwrap();
        fs::write(dir.join("a.txt"), b"x").unwrap();
        // 已存在的目录照样重设（本进程是目录所有者，非管理员也能改 DACL）
        protect_dir(&dir).unwrap();
        let got = dacl_sddl_of(&dir);
        assert!(got.starts_with("D:PAI"), "{got}");
        for ace in [
            "(A;OICI;FA;;;SY)",
            "(A;OICI;FA;;;BA)",
            "(A;OICI;0x1200a9;;;BU)",
        ] {
            assert!(got.contains(ace), "{ace} missing in {got}");
        }
        assert_eq!(got.matches("(A;").count(), 3, "{got}");

        // 目录联接（以及经过联接的路径）拒绝
        let link = tmp.path().join("link");
        fs::create_dir(tmp.path().join("other")).unwrap();
        let ok = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&link)
            .arg(tmp.path().join("other"))
            .output()
            .unwrap()
            .status
            .success();
        assert!(ok, "mklink /J failed");
        let e = protect_dir(&link).unwrap_err();
        assert!(e.contains("目录联接"), "{e}");

        // 还原成本用户可删的权限，好让临时目录被清理
        let f = OpenOptions::new()
            .access_mode(READ_CONTROL | WRITE_DAC)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
            .open(&dir)
            .unwrap();
        set_dacl(&f, "D:PAI(A;OICI;FA;;;WD)").unwrap();
    }

    #[test]
    fn owner_sid_validation() {
        assert!(valid_owner_sid(
            "S-1-12-1-1234567890-1234567890-123456789-1234567890"
        ));
        assert!(valid_owner_sid(
            "S-1-5-21-1004336348-1177238915-682003330-1001"
        ));
        for bad in [
            "",
            "S-1-5-18",
            "S-1-12-1-1-2-3",
            "S-1-12-2-1-2-3-4",
            "S-1-5-32-544",
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
    fn service_dacl_from_fixed_baseline() {
        let old = "S-1-5-21-1-2-3-1001";
        let new = "S-1-12-1-5-6-7-8";
        let sddl = service_sddl(new);
        assert_eq!(sddl, format!("{DEFAULT_SERVICE_DACL}(A;;0x34;;;{new})"));
        // 换所有者：只有新所有者的 ACE，与上次安装无关
        assert!(!sddl.contains(old), "{sddl}");
        assert_eq!(sddl.matches(";;;S-1-").count(), 1, "{sddl}");
        // 生成的 SDDL 能被系统解析
        SecurityDescriptor::from_sddl(&service_sddl(old)).unwrap();
        SecurityDescriptor::from_sddl(&sddl).unwrap();
    }
}

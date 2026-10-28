//! 以管理员身份运行一次性的组件命令（`routa-procsvc.exe --install / --uninstall / --cleanup`），
//! 经 UAC 提权，等待其退出码。

use std::io;
use std::iter::repeat_n;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use windows_sys::Win32::Foundation::{CloseHandle, LocalFree, ERROR_CANCELLED, HANDLE};
use windows_sys::Win32::Security::Authorization::ConvertSidToStringSidW;
use windows_sys::Win32::Security::{GetTokenInformation, TokenUser, TOKEN_QUERY, TOKEN_USER};
use windows_sys::Win32::System::Threading::{
    GetCurrentProcess, GetExitCodeProcess, OpenProcessToken, WaitForSingleObject, INFINITE,
};
use windows_sys::Win32::UI::Shell::{
    ShellExecuteExW, SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::SW_HIDE;

const HELPER: &str = "routa-procsvc.exe";
/// 组件的其余文件（ProxyBridgeCore.dll、WinDivert）所在目录名。
const SOURCE_DIR: &str = "procsvc";

pub enum ElevatedCmd {
    Install,
    Uninstall,
    Cleanup,
}

/// 经 UAC 以管理员身份运行组件命令并等它结束。用户取消 UAC 时为 `Err("已取消")`。
pub async fn run_elevated(cmd: ElevatedCmd) -> Result<(), String> {
    let exe_dir = std::env::current_exe().ok().and_then(|e| e.parent().map(Path::to_path_buf));
    let manifest = dev_manifest_dir();
    let helper = locate_helper(exe_dir.as_deref(), manifest)?;
    let args: Vec<String> = match cmd {
        ElevatedCmd::Install => {
            let source = locate_source(exe_dir.as_deref(), manifest)?;
            vec![
                "--install".into(),
                "--owner-sid".into(),
                current_user_sid()?,
                "--source".into(),
                source.to_string_lossy().into_owned(),
            ]
        }
        ElevatedCmd::Uninstall => vec!["--uninstall".into()],
        ElevatedCmd::Cleanup => vec!["--cleanup".into()],
    };
    let params = command_line(&args);
    let code = tokio::task::spawn_blocking(move || run_as_admin(&helper, &params))
        .await
        .map_err(|e| format!("组件命令没有完成：{e}"))??;
    exit_status(code)
}

/// 组件命令的退出码（见 routa-procsvc 的用法说明）。
fn exit_status(code: u32) -> Result<(), String> {
    match code {
        0 => Ok(()),
        1 => Err("组件命令参数错误".into()),
        2 => Err("组件命令需要管理员权限".into()),
        3 => Err("组件命令执行失败".into()),
        c => Err(format!("组件命令异常退出（退出码 {c}）")),
    }
}

/// 源码树（只在 debug 构建里）：release 构建只认 exe 旁边的文件。否则组件文件缺失时，
/// 会从普通用户可写的路径（例如在盘符根目录下建出的同名目录）复制 DLL 进受保护目录、
/// 甚至以管理员身份运行那里的 exe（spec 6.5）。
#[cfg(debug_assertions)]
fn dev_manifest_dir() -> Option<&'static Path> {
    Some(Path::new(env!("CARGO_MANIFEST_DIR")))
}

#[cfg(not(debug_assertions))]
fn dev_manifest_dir() -> Option<&'static Path> {
    None
}

/// 与 Routa 同目录的 `routa-procsvc.exe`；有源码树（debug）时退回到 `target/{debug,release}` 里较新的那个。
fn locate_helper(exe_dir: Option<&Path>, manifest: Option<&Path>) -> Result<PathBuf, String> {
    if let Some(p) = exe_dir.map(|d| d.join(HELPER)).filter(|p| p.is_file()) {
        return Ok(p);
    }
    let Some(manifest) = manifest else {
        return Err(format!("找不到进程分流组件 {HELPER}"));
    };
    ["debug", "release"]
        .iter()
        .map(|profile| manifest.join("target").join(profile).join(HELPER))
        .filter_map(|p| Some((p.metadata().ok()?.modified().ok()?, p)))
        .max_by_key(|(modified, _)| *modified)
        .map(|(_, p)| p)
        .ok_or_else(|| format!("找不到进程分流组件 {HELPER}"))
}

/// 与 Routa 同目录的 `procsvc\`；有源码树（debug）时退回到 `binaries\procsvc`。
fn locate_source(exe_dir: Option<&Path>, manifest: Option<&Path>) -> Result<PathBuf, String> {
    exe_dir
        .map(|d| d.join(SOURCE_DIR))
        .filter(|p| p.is_dir())
        .or_else(|| manifest.map(|m| m.join("binaries").join(SOURCE_DIR)).filter(|p| p.is_dir()))
        .ok_or_else(|| format!("找不到进程分流组件的文件目录 {SOURCE_DIR}"))
}

/// 按 Windows 命令行的解析规则（CommandLineToArgvW / MSVC CRT）拼接参数。
fn command_line(args: &[String]) -> String {
    args.iter().map(|a| quote(a)).collect::<Vec<_>>().join(" ")
}

fn quote(arg: &str) -> String {
    if !arg.is_empty() && !arg.contains([' ', '\t', '\n', '\x0b', '"']) {
        return arg.to_string();
    }
    let mut out = String::from('"');
    let mut backslashes = 0;
    for c in arg.chars() {
        if c == '\\' {
            backslashes += 1;
            continue;
        }
        // 引号前的反斜杠要加倍，引号本身再转义一次；其余位置的反斜杠原样保留。
        let n = if c == '"' { backslashes * 2 + 1 } else { backslashes };
        out.extend(repeat_n('\\', n));
        out.push(c);
        backslashes = 0;
    }
    // 结尾的反斜杠后面紧跟收尾的引号，同样加倍。
    out.extend(repeat_n('\\', backslashes * 2));
    out.push('"');
    out
}

fn wide(s: &std::ffi::OsStr) -> Vec<u16> {
    s.encode_wide().chain(Some(0)).collect()
}

/// ShellExecuteExW("runas") 并等待进程退出，返回退出码。
fn run_as_admin(file: &Path, params: &str) -> Result<u32, String> {
    let verb = wide("runas".as_ref());
    let file = wide(file.as_os_str());
    let params = wide(params.as_ref());
    // SAFETY: 各字符串以 NUL 结尾且在调用期间有效；返回的进程句柄在本函数内关闭。
    unsafe {
        let mut info: SHELLEXECUTEINFOW = std::mem::zeroed();
        info.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
        info.fMask = SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC;
        info.lpVerb = verb.as_ptr();
        info.lpFile = file.as_ptr();
        info.lpParameters = params.as_ptr();
        info.nShow = SW_HIDE;
        if ShellExecuteExW(&mut info) == 0 {
            let e = io::Error::last_os_error();
            if e.raw_os_error() == Some(ERROR_CANCELLED as i32) {
                return Err("已取消".into());
            }
            return Err(format!("无法以管理员身份运行组件命令：{e}"));
        }
        let process: HANDLE = info.hProcess;
        if process.is_null() {
            return Err("无法等待组件命令结束".into());
        }
        WaitForSingleObject(process, INFINITE);
        let mut code = 0u32;
        let ok = GetExitCodeProcess(process, &mut code);
        CloseHandle(process);
        if ok == 0 {
            return Err(format!("无法读取组件命令的退出码：{}", io::Error::last_os_error()));
        }
        Ok(code)
    }
}

/// 当前用户的 SID 字符串（`S-1-5-21-…`）。
fn current_user_sid() -> Result<String, String> {
    let fail = |what: &str| format!("无法取得当前用户 SID（{what}）：{}", io::Error::last_os_error());
    // SAFETY: token 句柄在本函数内关闭；buf 按 8 字节对齐、大小如实传入；
    // ConvertSidToStringSidW 分配的字符串以 NUL 结尾，读完后 LocalFree。
    unsafe {
        let mut token: HANDLE = std::ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return Err(fail("OpenProcessToken"));
        }
        let mut len = 0u32;
        GetTokenInformation(token, TokenUser, std::ptr::null_mut(), 0, &mut len);
        let mut buf = vec![0u64; (len as usize).div_ceil(8)];
        let ok = GetTokenInformation(token, TokenUser, buf.as_mut_ptr().cast(), len, &mut len);
        let err = (ok == 0).then(|| fail("GetTokenInformation"));
        CloseHandle(token);
        if let Some(e) = err {
            return Err(e);
        }
        let user = &*(buf.as_ptr() as *const TOKEN_USER);
        let mut s: *mut u16 = std::ptr::null_mut();
        if ConvertSidToStringSidW(user.User.Sid, &mut s) == 0 {
            return Err(fail("ConvertSidToStringSidW"));
        }
        let n = (0..).take_while(|&i| *s.add(i) != 0).count();
        let sid = String::from_utf16_lossy(std::slice::from_raw_parts(s, n));
        LocalFree(s.cast());
        Ok(sid)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{Duration, SystemTime};

    use super::*;

    fn args(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn command_line_quotes_like_argv() {
        assert_eq!(command_line(&args(&["--uninstall"])), "--uninstall");
        assert_eq!(
            command_line(&args(&["--install", "--owner-sid", "S-1-5-21-1-2-3-1001", "--source", r"C:\Program Files\Routa\procsvc"])),
            r#"--install --owner-sid S-1-5-21-1-2-3-1001 --source "C:\Program Files\Routa\procsvc""#
        );
        // 结尾的反斜杠在收尾引号前加倍，否则会把引号转义掉。
        assert_eq!(quote(r"C:\my dir\"), r#""C:\my dir\\""#);
        assert_eq!(quote(r#"a\"b"#), r#""a\\\"b""#);
        assert_eq!(quote(""), r#""""#);
        assert_eq!(quote(r"C:\无空格\路径"), r"C:\无空格\路径");
    }

    #[test]
    fn current_user_sid_looks_like_a_sid() {
        let sid = current_user_sid().unwrap();
        assert!(sid.starts_with("S-1-"), "{sid}");
    }

    #[test]
    fn exit_codes() {
        assert_eq!(exit_status(0), Ok(()));
        for c in [1, 2, 3, 7] {
            assert!(exit_status(c).is_err());
        }
    }

    #[test]
    fn locates_next_to_exe_first() {
        let td = tempfile::tempdir().unwrap();
        let (app, manifest) = (td.path().join("app"), td.path().join("src-tauri"));
        fs::create_dir_all(app.join(SOURCE_DIR)).unwrap();
        fs::write(app.join(HELPER), b"").unwrap();
        fs::create_dir_all(manifest.join("target/debug")).unwrap();
        fs::write(manifest.join("target/debug").join(HELPER), b"").unwrap();
        fs::create_dir_all(manifest.join("binaries").join(SOURCE_DIR)).unwrap();
        assert_eq!(locate_helper(Some(&app), Some(&manifest)), Ok(app.join(HELPER)));
        assert_eq!(locate_source(Some(&app), Some(&manifest)), Ok(app.join(SOURCE_DIR)));
    }

    #[test]
    fn no_source_tree_fallback_without_manifest() {
        let td = tempfile::tempdir().unwrap();
        let (app, manifest) = (td.path().join("app"), td.path().join("src-tauri"));
        fs::create_dir_all(&app).unwrap();
        fs::create_dir_all(manifest.join("target/release")).unwrap();
        fs::write(manifest.join("target/release").join(HELPER), b"").unwrap();
        fs::create_dir_all(manifest.join("binaries").join(SOURCE_DIR)).unwrap();
        assert!(locate_helper(Some(&app), Some(&manifest)).is_ok());
        assert!(locate_helper(Some(&app), None).is_err());
        assert!(locate_source(Some(&app), None).is_err());
    }

    #[test]
    #[cfg(not(debug_assertions))]
    fn release_build_has_no_source_tree() {
        assert_eq!(dev_manifest_dir(), None);
    }

    #[test]
    fn dev_fallback_prefers_newer_build() {
        let td = tempfile::tempdir().unwrap();
        let (app, manifest) = (td.path().join("app"), td.path().join("src-tauri"));
        fs::create_dir_all(&app).unwrap();
        assert!(locate_helper(Some(&app), Some(&manifest)).is_err());
        assert!(locate_source(Some(&app), Some(&manifest)).is_err());

        let (debug, release) = (manifest.join("target/debug"), manifest.join("target/release"));
        for d in [&debug, &release] {
            fs::create_dir_all(d).unwrap();
            fs::write(d.join(HELPER), b"").unwrap();
        }
        let old = SystemTime::now() - Duration::from_secs(3600);
        fs::File::options().write(true).open(debug.join(HELPER)).unwrap().set_modified(old).unwrap();
        assert_eq!(locate_helper(Some(&app), Some(&manifest)), Ok(release.join(HELPER)));
        fs::File::options().write(true).open(release.join(HELPER)).unwrap()
            .set_modified(old - Duration::from_secs(60)).unwrap();
        assert_eq!(locate_helper(None, Some(&manifest)), Ok(debug.join(HELPER)));

        fs::create_dir_all(manifest.join("binaries").join(SOURCE_DIR)).unwrap();
        assert_eq!(locate_source(Some(&app), Some(&manifest)), Ok(manifest.join("binaries").join(SOURCE_DIR)));
    }
}

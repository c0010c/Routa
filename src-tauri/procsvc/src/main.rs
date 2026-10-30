//! RoutaProcSvc：以 LocalSystem 运行的进程分流助手服务。
//!
//! 无参数且由 SCM 启动时进入服务模式；另有安装、卸载、清理三个一次性命令（均需管理员）。
//! 退出码：0 成功；1 参数错误；2 需要管理员；3 其他失败（stderr 写原因）。
//! 一次性命令可另带 `--error-file <路径>`：失败时把原因（中文）也写进这个新文件
//! （经 UAC 提权运行时调用方看不到 stderr）。

mod engine;
mod fallback;
mod install;
mod lazy;
mod localfile;
mod paths;
mod pb_ffi;
mod persist;
mod reconcile;
mod server;
mod service;
mod tcpkill;
mod wfp;

use std::ffi::OsString;
use std::fs::OpenOptions;
use std::io::Write;
use std::os::windows::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use windows_sys::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT;

const USAGE: &str = "用法：
  routa-procsvc.exe --install --owner-sid <SID> --source <目录>
  routa-procsvc.exe --uninstall
  routa-procsvc.exe --cleanup
以上命令都可另加 --error-file <路径>：失败时把原因写进该文件（必须是尚不存在的新文件）。
不带参数时只能由服务管理器启动。";

#[derive(Debug, PartialEq, Eq)]
enum Command {
    Service,
    Install { owner_sid: String, source: PathBuf },
    Uninstall,
    Cleanup,
}

/// 取出 `--error-file <路径>`（可在任意位置，最多一次），其余参数原样返回。
fn take_error_file(args: Vec<OsString>) -> (Vec<OsString>, Result<Option<PathBuf>, String>) {
    let mut rest = Vec::with_capacity(args.len());
    let mut file = Ok(None);
    let mut args = args.into_iter();
    while let Some(a) = args.next() {
        if a != "--error-file" {
            rest.push(a);
            continue;
        }
        file = match (file, args.next()) {
            (Ok(None), Some(p)) => Ok(Some(PathBuf::from(p))),
            (Ok(None), None) => Err("--error-file 缺少值".to_string()),
            (Ok(Some(_)), _) => Err("重复的参数：--error-file".to_string()),
            (Err(e), _) => Err(e),
        };
    }
    (rest, file)
}

/// 把失败原因写进 `--error-file` 指定的文件。以管理员身份运行，所以只新建文件、不覆盖已有文件，
/// 也不跟随重解析点（目录各层用 localfile 检查，文件本身 CREATE_NEW + 不跟随）。尽力而为。
fn write_error_file(path: &Path, msg: &str) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("错误文件路径无效：{}", path.display()))?;
    let _held = localfile::hold(parent, localfile::Expect::Dir)?;
    let mut f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
        .map_err(|e| format!("创建 {} 失败：{e}", path.display()))?;
    f.write_all(msg.as_bytes())
        .map_err(|e| format!("写入 {} 失败：{e}", path.display()))
}

fn parse_args(args: Vec<OsString>) -> Result<Command, String> {
    let mut args = args.into_iter();
    let Some(first) = args.next() else {
        return Ok(Command::Service);
    };
    let text = |a: OsString| {
        a.into_string()
            .map_err(|a| format!("参数不是有效文本：{a:?}"))
    };
    let first = text(first)?;
    let simple = match first.as_str() {
        "--uninstall" => Some(Command::Uninstall),
        "--cleanup" => Some(Command::Cleanup),
        "--install" => None,
        _ => return Err(format!("未知参数：{first}")),
    };
    if let Some(cmd) = simple {
        return match args.next() {
            None => Ok(cmd),
            Some(a) => Err(format!("多余的参数：{}", a.to_string_lossy())),
        };
    }
    let (mut owner_sid, mut source) = (None, None);
    while let Some(flag) = args.next() {
        let flag = text(flag)?;
        let slot = match flag.as_str() {
            "--owner-sid" if owner_sid.is_none() => &mut owner_sid,
            "--source" if source.is_none() => &mut source,
            _ => return Err(format!("未知或重复的参数：{flag}")),
        };
        *slot = Some(args.next().ok_or_else(|| format!("{flag} 缺少值"))?);
    }
    let owner_sid = text(owner_sid.ok_or("缺少 --owner-sid")?)?;
    if !install::valid_owner_sid(&owner_sid) {
        return Err(format!("无效的所有者 SID：{owner_sid}"));
    }
    let source = PathBuf::from(source.ok_or("缺少 --source")?);
    Ok(Command::Install { owner_sid, source })
}

fn main() -> ExitCode {
    if std::env::args_os().skip(1).collect::<Vec<_>>() == [OsString::from("--build-info")] {
        println!("{}", serde_json::json!({ "format": "ROUTA_BUILD_INFO_V1", "protocolVersion": routa_procsvc_proto::PROTO_VERSION }));
        return ExitCode::SUCCESS;
    }
    let (args, error_file) = take_error_file(std::env::args_os().skip(1).collect());
    let fail = |code: u8, msg: &str| {
        eprintln!("{msg}");
        if let Ok(Some(path)) = &error_file {
            if let Err(e) = write_error_file(path, msg) {
                eprintln!("{e}");
            }
        }
        ExitCode::from(code)
    };
    let parsed = error_file
        .clone()
        .and_then(|f| match (parse_args(args), f) {
            (Ok(Command::Service), Some(_)) => Err("--error-file 只能用于一次性命令".into()),
            (r, _) => r,
        });
    let cmd = match parsed {
        Ok(c) => c,
        Err(e) => return fail(1, &format!("{e}\n{USAGE}")),
    };
    if cmd == Command::Service {
        return match service::run() {
            Ok(()) => ExitCode::SUCCESS,
            Err(_) => {
                eprintln!("{USAGE}");
                ExitCode::from(1)
            }
        };
    }
    if !install::is_elevated() {
        return fail(2, "需要管理员权限");
    }
    let result = match cmd {
        Command::Install { owner_sid, source } => install::install(&owner_sid, &source),
        Command::Uninstall => install::uninstall(),
        Command::Cleanup => install::cleanup(),
        Command::Service => unreachable!(),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => fail(3, &e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Command, String> {
        parse_args(args.iter().map(OsString::from).collect())
    }

    const SID: &str = "S-1-5-21-1-2-3-1001";

    #[test]
    fn parses_commands() {
        assert_eq!(parse(&[]), Ok(Command::Service));
        assert_eq!(parse(&["--uninstall"]), Ok(Command::Uninstall));
        assert_eq!(parse(&["--cleanup"]), Ok(Command::Cleanup));
        let want = Ok(Command::Install {
            owner_sid: SID.into(),
            source: PathBuf::from(r"C:\src"),
        });
        assert_eq!(
            parse(&["--install", "--owner-sid", SID, "--source", r"C:\src"]),
            want
        );
        assert_eq!(
            parse(&["--install", "--source", r"C:\src", "--owner-sid", SID]),
            want
        );
    }

    fn split(args: &[&str]) -> (Vec<OsString>, Result<Option<PathBuf>, String>) {
        take_error_file(args.iter().map(OsString::from).collect())
    }

    #[test]
    fn takes_error_file_anywhere() {
        let (rest, f) = split(&["--error-file", r"C:\t\e.txt", "--uninstall"]);
        assert_eq!(rest, [OsString::from("--uninstall")]);
        assert_eq!(f, Ok(Some(PathBuf::from(r"C:\t\e.txt"))));
        let (rest, f) = split(&[
            "--install",
            "--owner-sid",
            SID,
            "--error-file",
            "e.txt",
            "--source",
            r"C:\src",
        ]);
        assert_eq!(f, Ok(Some(PathBuf::from("e.txt"))));
        assert!(parse_args(rest).is_ok());
        assert_eq!(split(&["--cleanup"]).1, Ok(None));
        assert!(split(&["--cleanup", "--error-file"]).1.is_err());
        assert!(split(&["--error-file", "a", "--error-file", "b"])
            .1
            .is_err());
    }

    #[test]
    fn error_file_is_created_new_only() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("e.txt");
        write_error_file(&p, "服务在 10 秒内没有停止").unwrap();
        assert_eq!(
            std::fs::read_to_string(&p).unwrap(),
            "服务在 10 秒内没有停止"
        );
        // 已存在：不覆盖
        assert!(write_error_file(&p, "x").is_err());
        assert_eq!(
            std::fs::read_to_string(&p).unwrap(),
            "服务在 10 秒内没有停止"
        );
        assert!(write_error_file(&dir.path().join("missing").join("e.txt"), "x").is_err());
    }

    #[test]
    fn rejects_bad_args() {
        for args in [
            &["--bogus"][..],
            &["--uninstall", "--cleanup"],
            &["--install"],
            &["--install", "--owner-sid", SID],
            &["--install", "--source", r"C:\src"],
            &["--install", "--owner-sid"],
            &[
                "--install",
                "--owner-sid",
                SID,
                "--owner-sid",
                SID,
                "--source",
                "x",
            ],
            &[
                "--install",
                "--owner-sid",
                "S-1-5-18",
                "--source",
                r"C:\src",
            ],
            &[
                "--install",
                "--owner-sid",
                SID,
                "--source",
                r"C:\src",
                "extra",
            ],
        ] {
            assert!(parse(args).is_err(), "{args:?}");
        }
    }
}

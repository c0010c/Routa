//! RoutaProcSvc：以 LocalSystem 运行的进程分流助手服务。
//!
//! 无参数且由 SCM 启动时进入服务模式；另有安装、卸载、清理三个一次性命令（均需管理员）。
//! 退出码：0 成功；1 参数错误；2 需要管理员；3 其他失败（stderr 写原因）。

mod engine;
mod fallback;
mod install;
mod paths;
mod pb_ffi;
mod persist;
mod reconcile;
mod server;
mod service;
mod tcpkill;
mod wfp;

use std::ffi::OsString;
use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "用法：
  routa-procsvc.exe --install --owner-sid <SID> --source <目录>
  routa-procsvc.exe --uninstall
  routa-procsvc.exe --cleanup
不带参数时只能由服务管理器启动。";

#[derive(Debug, PartialEq, Eq)]
enum Command {
    Service,
    Install { owner_sid: String, source: PathBuf },
    Uninstall,
    Cleanup,
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
    let cmd = match parse_args(std::env::args_os().skip(1).collect()) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}\n{USAGE}");
            return ExitCode::from(1);
        }
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
        eprintln!("需要管理员权限");
        return ExitCode::from(2);
    }
    let result = match cmd {
        Command::Install { owner_sid, source } => install::install(&owner_sid, &source),
        Command::Uninstall => install::uninstall(),
        Command::Cleanup => install::cleanup(),
        Command::Service => unreachable!(),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::from(3)
        }
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

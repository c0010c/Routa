//! 最近一次完整状态的持久化（spec 6.3），以及"上次是正常停止"标记。
//! 文件放在安装目录（只有管理员能写）。

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use routa_procsvc_proto::{Action, DesiredState};

/// 状态文件名：`<安装目录>\state.json`。
pub const STATE_FILE: &str = "state.json";
/// 正常停止标记文件名：`<安装目录>\clean-stop`。
const CLEAN_STOP_FILE: &str = "clean-stop";

/// 原子地保存状态：先写同目录的临时文件并落盘，再 rename 覆盖。
///
/// 代理认证信息不落盘：读回的状态只用于回落（结果里没有 `Proxy`），用不到它们，
/// 而安装目录对普通用户可读。
pub fn save(path: &Path, s: &DesiredState) -> io::Result<()> {
    let mut s = s.clone();
    for r in &mut s.rules {
        if let Action::Proxy(t) = &mut r.action {
            t.username = None;
            t.password = None;
        }
    }
    let body = serde_json::to_vec_pretty(&s).map_err(io::Error::other)?;
    let tmp = tmp_path(path);
    let written = (|| {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(&body)?;
        f.sync_all()
    })();
    if let Err(e) = written.and_then(|()| fs::rename(&tmp, path)) {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }
    Ok(())
}

/// 读取状态；文件缺失或损坏返回 None。
pub fn load(path: &Path) -> Option<DesiredState> {
    let body = fs::read(path).ok()?;
    serde_json::from_slice(&body).ok()
}

/// 记录"服务是正常停止的"（有序停止、关机前调用）。
pub fn mark_clean_stop(dir: &Path) -> io::Result<()> {
    fs::write(dir.join(CLEAN_STOP_FILE), b"")
}

/// 取走正常停止标记：返回它是否存在，并删除它（服务启动时调用一次）。
pub fn take_clean_stop(dir: &Path) -> bool {
    fs::remove_file(dir.join(CLEAN_STOP_FILE)).is_ok()
}

fn tmp_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".tmp");
    path.with_file_name(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use routa_procsvc_proto::{ExitPolicy, FaultPolicy, ProxyKind, ProxyTarget, RuleSpec};

    fn state() -> DesiredState {
        DesiredState {
            rules: vec![RuleSpec {
                id: "r1".into(),
                exe_path: r"C:\测试\a.exe".into(),
                action: Action::Proxy(ProxyTarget {
                    kind: ProxyKind::Socks5,
                    host: "127.0.0.1".into(),
                    port: 7890,
                    username: Some("u".into()),
                    password: Some("p".into()),
                }),
                on_fault: FaultPolicy::Block,
                reset_on_change: true,
            }],
            on_routa_exit: ExitPolicy::Direct,
            routa_pid: 42,
        }
    }

    #[test]
    fn save_then_load_roundtrip_without_credentials() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(STATE_FILE);
        save(&path, &state()).unwrap();
        // 覆盖已有文件
        save(&path, &state()).unwrap();

        let mut want = state();
        if let Action::Proxy(t) = &mut want.rules[0].action {
            t.username = None;
            t.password = None;
        }
        assert_eq!(load(&path), Some(want));
        let text = fs::read_to_string(&path).unwrap();
        assert!(!text.contains("\"p\""), "{text}");
        // 不留临时文件
        let names: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, vec![std::ffi::OsString::from(STATE_FILE)]);
    }

    #[test]
    fn load_missing_or_corrupt_is_none() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(STATE_FILE);
        assert_eq!(load(&path), None);
        fs::write(&path, "{oops").unwrap();
        assert_eq!(load(&path), None);
    }

    #[test]
    fn clean_stop_marker_is_taken_once() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!take_clean_stop(dir.path()));
        mark_clean_stop(dir.path()).unwrap();
        mark_clean_stop(dir.path()).unwrap();
        assert!(take_clean_stop(dir.path()));
        assert!(!take_clean_stop(dir.path()));
    }
}

//! 以 LocalSystem 身份打开本地路径前的检查：只走本地磁盘，不跟随任何重解析点。
//!
//! 服务会以机器账户打开规则里的程序文件（WFP 取 app id）。若路径经由网络盘、符号链接、
//! 目录联接或挂载点被引到 UNC，系统会以机器账户做 SMB/WebDAV 认证（可被 NTLM 中继利用）。
//! 这里逐级打开路径的每一层（`FILE_FLAG_OPEN_REPARSE_POINT`，不跟随），遇到重解析点即拒绝；
//! 打开的句柄不共享删除权限，持有期间各层不能被改名或删除，从而不能被换成链接。

use std::fs::{File, OpenOptions};
use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Component, Path, PathBuf, Prefix};

use windows_sys::Win32::Storage::FileSystem::{
    GetDriveTypeW, FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_REPARSE_POINT,
    FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_READ_ATTRIBUTES,
    FILE_SHARE_READ, FILE_SHARE_WRITE,
};

/// `GetDriveTypeW` 的网络驱动器返回值。
const DRIVE_REMOTE: u32 = 4;

/// 期望路径最后一层是什么。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Expect {
    File,
    Dir,
}

/// 已检查过的路径：持有从盘符根到最后一层的全部句柄，析构时释放。
#[derive(Debug)]
pub struct Held {
    #[allow(dead_code)] // 只为持有句柄
    handles: Vec<File>,
}

/// 检查并“占住”本地路径：必须是 `X:\…`、盘符不是网络驱动器、每一层都存在且不是重解析点，
/// 中间层是目录，最后一层符合 `expect`。
pub fn hold(path: &Path, expect: Expect) -> Result<Held, String> {
    let shown = path.display();
    let mut comps = path.components();
    let letter = match comps.next() {
        Some(Component::Prefix(p)) => match p.kind() {
            Prefix::Disk(l) if l.is_ascii_alphabetic() => l as char,
            _ => return Err(format!("只支持本地磁盘路径：{shown}")),
        },
        _ => return Err(format!("只支持本地磁盘路径：{shown}")),
    };
    if comps.next() != Some(Component::RootDir) {
        return Err(format!("只支持本地磁盘上的绝对路径：{shown}"));
    }
    let root = format!("{letter}:\\");
    let wroot: Vec<u16> = root.encode_utf16().chain(Some(0)).collect();
    // SAFETY: wroot 以 0 结尾。
    if unsafe { GetDriveTypeW(wroot.as_ptr()) } == DRIVE_REMOTE {
        return Err(format!("不支持网络驱动器上的路径：{shown}"));
    }

    let rest: Vec<Component> = comps.collect();
    if rest.is_empty() {
        return Err(format!("路径缺少文件部分：{shown}"));
    }
    let mut cur = PathBuf::from(&root);
    let mut handles = Vec::with_capacity(rest.len() + 1);
    handles.push(open_no_follow(&cur)?);
    for (i, c) in rest.iter().enumerate() {
        match c {
            Component::Normal(n) => cur.push(n),
            _ => return Err(format!("路径不能包含 . 或 ..：{shown}")),
        }
        let f = open_no_follow(&cur)?;
        let attrs = f
            .metadata()
            .map_err(|e| format!("读取 {} 的属性失败：{e}", cur.display()))?
            .file_attributes();
        if attrs & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(format!(
                "路径经过符号链接、目录联接或挂载点，出于安全原因拒绝：{}",
                cur.display()
            ));
        }
        let is_dir = attrs & FILE_ATTRIBUTE_DIRECTORY != 0;
        let last = i + 1 == rest.len();
        let want_dir = !last || expect == Expect::Dir;
        if is_dir != want_dir {
            return Err(if want_dir {
                format!("不是目录：{}", cur.display())
            } else {
                format!("不是文件：{}", cur.display())
            });
        }
        handles.push(f);
    }
    Ok(Held { handles })
}

/// 打开一层路径本身（是重解析点也不跟随），只要读属性权限；不共享删除，防止被改名或删除。
fn open_no_follow(p: &Path) -> Result<File, String> {
    OpenOptions::new()
        .access_mode(FILE_READ_ATTRIBUTES)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS)
        .open(p)
        .map_err(|e| format!("无法打开 {}：{e}", p.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    #[test]
    fn plain_local_file_and_dir_ok() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("a.exe");
        std::fs::write(&exe, b"x").unwrap();
        hold(&exe, Expect::File).unwrap();
        hold(dir.path(), Expect::Dir).unwrap();
    }

    #[test]
    fn wrong_kind_or_missing_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("a.exe");
        std::fs::write(&exe, b"x").unwrap();
        assert!(hold(dir.path(), Expect::File).is_err());
        assert!(hold(&exe, Expect::Dir).is_err());
        assert!(hold(&dir.path().join("missing.exe"), Expect::File).is_err());
        assert!(hold(&exe.join("x.exe"), Expect::File).is_err());
    }

    #[test]
    fn non_local_paths_rejected() {
        for p in [
            r"\\srv\share\a.exe",
            r"\\?\C:\Windows\notepad.exe",
            r"\\.\C:\Windows\notepad.exe",
            r"C:a.exe",
            r"\Windows\notepad.exe",
            r"C:\",
            r"C:\Windows\..\Windows\notepad.exe",
        ] {
            let e = hold(Path::new(p), Expect::File).err();
            assert!(e.is_some(), "{p} should be rejected");
        }
        hold(Path::new(r"C:\Windows\notepad.exe"), Expect::File).unwrap();
    }

    #[test]
    fn junction_rejected_at_any_level() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("real");
        std::fs::create_dir(&real).unwrap();
        std::fs::write(real.join("a.exe"), b"x").unwrap();
        let link = dir.path().join("link");
        // 目录联接无需特权即可创建
        let ok = Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&link)
            .arg(&real)
            .output()
            .unwrap()
            .status
            .success();
        assert!(ok, "mklink /J failed");
        hold(&real.join("a.exe"), Expect::File).unwrap();
        let e = hold(&link.join("a.exe"), Expect::File).unwrap_err();
        assert!(e.contains("目录联接"), "{e}");
        let e = hold(&link, Expect::Dir).unwrap_err();
        assert!(e.contains("目录联接"), "{e}");
    }

    #[test]
    fn held_path_cannot_be_renamed() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("sub");
        std::fs::create_dir(&sub).unwrap();
        std::fs::write(sub.join("a.exe"), b"x").unwrap();
        let held = hold(&sub.join("a.exe"), Expect::File).unwrap();
        assert!(std::fs::rename(&sub, dir.path().join("moved")).is_err());
        drop(held);
        std::fs::rename(&sub, dir.path().join("moved")).unwrap();
    }
}

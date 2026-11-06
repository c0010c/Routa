//! 正在运行的进程（新建规则时从中挑选 exe）。

use std::collections::HashSet;
use std::path::Path;

use serde::Serialize;
use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows_sys::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};

#[derive(Serialize, Clone, Debug, PartialEq, specta::Type)]
pub struct RunningProcess {
    pub pid: u32,
    /// exe 文件名。
    pub name: String,
    pub exe_path: String,
}

/// 每个 exe 路径一项（不区分大小写，取先遇到的进程），按名称排序。打不开的进程（权限不够、已退出）跳过。
pub fn running_processes() -> Vec<RunningProcess> {
    let mut seen = HashSet::new();
    let mut out: Vec<RunningProcess> = process_ids()
        .into_iter()
        .filter_map(|pid| Some((pid, image_path(pid)?)))
        .filter(|(_, path)| seen.insert(path.to_lowercase()))
        .map(|(pid, exe_path)| RunningProcess {
            pid,
            name: Path::new(&exe_path).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
            exe_path,
        })
        .collect();
    out.sort_by_key(|p| (p.name.to_lowercase(), p.exe_path.to_lowercase()));
    out
}

fn process_ids() -> Vec<u32> {
    let mut pids = Vec::new();
    // SAFETY: 快照句柄在本函数内关闭；entry.dwSize 如实设置。
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snap == INVALID_HANDLE_VALUE {
            return pids;
        }
        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut ok = Process32FirstW(snap, &mut entry);
        while ok != 0 {
            pids.push(entry.th32ProcessID);
            ok = Process32NextW(snap, &mut entry);
        }
        CloseHandle(snap);
    }
    pids
}

/// 进程 exe 的完整路径；打不开或查不到时为 None。
fn image_path(pid: u32) -> Option<String> {
    // SAFETY: 进程句柄在本函数内关闭；buf 的长度如实传入。
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if h.is_null() {
            return None;
        }
        let mut buf = [0u16; 32768];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, buf.as_mut_ptr(), &mut len);
        CloseHandle(h);
        (ok != 0).then(|| String::from_utf16_lossy(&buf[..len as usize]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_this_process_once() {
        let me = std::env::current_exe().unwrap().to_string_lossy().to_lowercase();
        let list = running_processes();
        assert_eq!(list.iter().filter(|p| p.exe_path.to_lowercase() == me).count(), 1);
        assert!(list.iter().all(|p| !p.name.is_empty() && p.exe_path.ends_with(&p.name)));
    }
}

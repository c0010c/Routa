//! 断开某 exe 的 IPv4 TCP 连接：枚举 TCP 表，把属于该 exe 的 ESTABLISHED 行置为 DELETE_TCB。

use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_ACCESS_DENIED, ERROR_INSUFFICIENT_BUFFER, NO_ERROR,
};
use windows_sys::Win32::NetworkManagement::IpHelper::{
    GetExtendedTcpTable, SetTcpEntry, MIB_TCPROW_OWNER_PID, MIB_TCPTABLE_OWNER_PID,
    MIB_TCP_STATE_DELETE_TCB, MIB_TCP_STATE_ESTAB, TCP_TABLE_OWNER_PID_ALL,
};
use windows_sys::Win32::Networking::WinSock::AF_INET;
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};

use crate::engine::ConnKiller;
use crate::paths::{exe_of_pid, same_path};

/// `SetTcpEntry` 使用的 `MIB_TCPROW`：与 `MIB_TCPROW_OWNER_PID` 前五个字段布局一致。
#[repr(C)]
struct TcpRow {
    state: u32,
    local_addr: u32,
    local_port: u32,
    remote_addr: u32,
    remote_port: u32,
}

/// 映像路径等于 `exe`（不区分大小写）的所有进程 PID。
pub fn pids_for_exe(exe: &str) -> Vec<u32> {
    let mut pids = Vec::new();
    // SAFETY: 快照句柄只在本函数内使用并关闭；entry 的 dwSize 已按要求初始化。
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snap.is_null() || snap as isize == -1 {
            return pids;
        }
        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut ok = Process32FirstW(snap, &mut entry);
        while ok != 0 {
            let pid = entry.th32ProcessID;
            if pid != 0 && exe_of_pid(pid).is_some_and(|p| same_path(&p, exe)) {
                pids.push(pid);
            }
            ok = Process32NextW(snap, &mut entry);
        }
        CloseHandle(snap);
    }
    pids
}

/// 读取 IPv4 TCP 连接表（含所属 PID）。
fn tcp_rows() -> Result<Vec<MIB_TCPROW_OWNER_PID>, String> {
    let mut size = 0u32;
    // 表在两次调用之间可能增长，留出重试
    for _ in 0..8 {
        // u32 缓冲区保证 MIB_TCPTABLE_OWNER_PID 所需的对齐
        let mut words = vec![0u32; (size as usize).div_ceil(4)];
        // SAFETY: words 至少有 size 字节，可写。
        let rc = unsafe {
            GetExtendedTcpTable(
                words.as_mut_ptr().cast(),
                &mut size,
                0,
                AF_INET as u32,
                TCP_TABLE_OWNER_PID_ALL,
                0,
            )
        };
        match rc {
            NO_ERROR => {
                // SAFETY: 调用成功，缓冲区开头是 MIB_TCPTABLE_OWNER_PID，随后有 dwNumEntries 行。
                let table = words.as_ptr().cast::<MIB_TCPTABLE_OWNER_PID>();
                let n = unsafe { (*table).dwNumEntries } as usize;
                let first =
                    unsafe { std::ptr::addr_of!((*table).table).cast::<MIB_TCPROW_OWNER_PID>() };
                return Ok((0..n).map(|i| unsafe { first.add(i).read() }).collect());
            }
            ERROR_INSUFFICIENT_BUFFER => continue,
            e => return Err(format!("读取 TCP 连接表失败（错误码 {e}）")),
        }
    }
    Err("读取 TCP 连接表失败：表持续增长".into())
}

/// 真实的 [`ConnKiller`]。
pub struct TcpKiller;

impl ConnKiller for TcpKiller {
    fn reset_ipv4_tcp(&mut self, exe: &str) -> Result<u32, String> {
        let pids = pids_for_exe(exe);
        if pids.is_empty() {
            return Ok(0);
        }
        let rows = tcp_rows()?;
        let (mut ok, mut denied, mut other) = (0u32, 0u32, 0u32);
        for r in rows
            .iter()
            .filter(|r| r.dwState == MIB_TCP_STATE_ESTAB as u32 && pids.contains(&r.dwOwningPid))
        {
            let row = TcpRow {
                state: MIB_TCP_STATE_DELETE_TCB as u32,
                local_addr: r.dwLocalAddr,
                local_port: r.dwLocalPort,
                remote_addr: r.dwRemoteAddr,
                remote_port: r.dwRemotePort,
            };
            // SAFETY: TcpRow 与 MIB_TCPROW 布局一致，指针在调用期间有效。
            match unsafe { SetTcpEntry(std::ptr::addr_of!(row).cast()) } {
                NO_ERROR => ok += 1,
                ERROR_ACCESS_DENIED => denied += 1,
                _ => other += 1,
            }
        }
        if ok == 0 && denied > 0 && other == 0 {
            return Err("断开连接被拒绝：需要管理员权限".into());
        }
        Ok(ok)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};

    #[test]
    fn finds_own_pid() {
        let me = std::env::current_exe().unwrap();
        let pids = pids_for_exe(&me.to_string_lossy());
        assert!(pids.contains(&std::process::id()), "{pids:?}");
    }

    #[test]
    fn unknown_exe_resets_nothing() {
        let n = TcpKiller
            .reset_ipv4_tcp(r"C:\definitely\not\here.exe")
            .unwrap();
        assert_eq!(n, 0);
    }

    #[test]
    #[ignore = "needs admin"]
    fn resets_own_connection() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let mut client = TcpStream::connect(addr).unwrap();
        let (_server, _) = listener.accept().unwrap();
        client.write_all(b"x").unwrap();

        let me = std::env::current_exe().unwrap();
        let n = TcpKiller.reset_ipv4_tcp(&me.to_string_lossy()).unwrap();
        assert!(n >= 1, "reset {n}");

        std::thread::sleep(std::time::Duration::from_millis(300));
        let mut buf = [0u8; 8];
        let read = client.read(&mut buf);
        let write = client.write_all(b"y");
        assert!(
            read.is_err() || matches!(read, Ok(0)) || write.is_err(),
            "connection still alive: {read:?} {write:?}"
        );
        assert!(read.is_err(), "expected reset on read, got {read:?}");
    }
}

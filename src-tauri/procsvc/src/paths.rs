//! exe 路径工具：不区分大小写的比较、按 PID 取完整映像路径。

use windows_sys::Win32::Foundation::CloseHandle;
use windows_sys::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};

use crate::reconcile::exe_key;

/// 两个 exe 路径是否指同一文件（按 [`exe_key`] 比较）。
pub fn same_path(a: &str, b: &str) -> bool {
    exe_key(a) == exe_key(b)
}

/// 进程的完整映像路径（Win32 形式）；进程已退出或无权访问时为 None。
pub fn exe_of_pid(pid: u32) -> Option<String> {
    // 长路径上限：最多 32767 个 UTF-16 单元。
    let mut buf = vec![0u16; 32768];
    let mut len = buf.len() as u32;
    // SAFETY: 句柄只在本函数内使用并关闭；buf/len 描述的是一块有效、可写的缓冲区。
    let ok = unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if h.is_null() {
            return None;
        }
        let ok = QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, buf.as_mut_ptr(), &mut len);
        CloseHandle(h);
        ok
    };
    (ok != 0).then(|| String::from_utf16_lossy(&buf[..len as usize]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_path_variants() {
        assert!(same_path(r"C:\测试\Ä.exe", "c:/测试/ä.EXE "));
        assert!(!same_path(r"C:\a\b.exe", r"C:\a\b.exe.bak"));
    }

    #[test]
    fn exe_of_own_pid() {
        let me = std::env::current_exe().unwrap();
        let got = exe_of_pid(std::process::id()).expect("own image path");
        assert!(same_path(&got, &me.to_string_lossy()), "{got} vs {me:?}");
    }
}

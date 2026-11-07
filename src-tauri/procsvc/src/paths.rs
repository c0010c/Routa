//! exe 路径工具：不区分大小写的比较、按 PID 取完整映像路径。

use windows_sys::Win32::Foundation::CloseHandle;
use windows_sys::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};

use routa_procsvc_proto::ProcessKind;

use crate::reconcile::exe_key;

/// 两个 exe 路径是否指同一文件（按 [`exe_key`] 比较）。
pub fn same_path(a: &str, b: &str) -> bool {
    exe_key(a) == exe_key(b)
}

/// 规则里的进程 `pattern` 是否命中完整映像路径 `exe`：`kind=Name` 时比较文件名，
/// `kind=Path` 时比较整条路径。与 ProxyBridge 的匹配方式一致。
pub fn matches_process(kind: ProcessKind, pattern: &str, exe: &str) -> bool {
    match kind {
        ProcessKind::Name => {
            let key = exe_key(exe);
            exe_key(pattern) == key.rsplit('\\').next().unwrap_or(&key)
        }
        ProcessKind::Path => same_path(pattern, exe),
    }
}

/// 引擎接口只收到规则里的进程字符串（不带种类）时，按写法判断种类。
/// 服务端校验保证规则的 `kind` 与写法一致（进程名不含路径分隔符或盘符，完整路径必含），
/// 所以结果与规则的 `kind` 相同。
pub fn pattern_kind(pattern: &str) -> ProcessKind {
    if routa_procsvc_proto::is_process_name(pattern.trim()) {
        ProcessKind::Name
    } else {
        ProcessKind::Path
    }
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
    fn matches_process_by_name_or_path() {
        use ProcessKind::{Name, Path};
        let exe = r"C:\Apps\ChatGPT\ChatGPT.exe";
        assert!(matches_process(Name, "chatgpt.EXE", exe));
        assert!(matches_process(Path, r"c:/apps/chatgpt/chatgpt.exe", exe));
        assert!(!matches_process(Path, r"D:\ChatGPT.exe", exe));
        assert!(!matches_process(Name, "GPT.exe", exe));
        assert!(!matches_process(Name, "ChatGPT", exe));
        // 种类由调用方给出，不从内容推断
        assert!(!matches_process(Path, "chatgpt.exe", exe));
        assert!(!matches_process(Name, r"c:\apps\chatgpt\chatgpt.exe", exe));
    }

    #[test]
    fn pattern_kind_of_engine_string() {
        assert_eq!(pattern_kind("a.exe"), ProcessKind::Name);
        assert_eq!(pattern_kind(r"C:\a\a.exe"), ProcessKind::Path);
    }

    #[test]
    fn exe_of_own_pid() {
        let me = std::env::current_exe().unwrap();
        let got = exe_of_pid(std::process::id()).expect("own image path");
        assert!(same_path(&got, &me.to_string_lossy()), "{got} vs {me:?}");
    }
}

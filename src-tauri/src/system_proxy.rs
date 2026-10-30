//! Per-user WinINET proxy ownership. Restore only settings that are still ours.
//! References: https://learn.microsoft.com/windows/win32/wininet/setting-and-retrieving-internet-options
use crate::store::{read_json, write_json_atomic};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ProxySettings {
    pub flags: u32,
    pub server: String,
    pub bypass: String,
    pub pac: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Lease {
    previous: ProxySettings,
    applied: ProxySettings,
}

pub trait ProxyBackend: Send + Sync {
    fn read(&self) -> Result<ProxySettings, String>;
    fn write(&self, settings: &ProxySettings) -> Result<(), String>;
}

pub struct SystemProxy {
    path: PathBuf,
    backend: Box<dyn ProxyBackend>,
}
impl SystemProxy {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            backend: Box::new(WindowsProxy),
        }
    }

    pub fn owns(&self) -> Result<bool, String> {
        let lease: Option<Lease> = read_json(&self.path).map_err(|e| e.to_string())?;
        match lease {
            Some(l) => Ok(self.backend.read()? == l.applied),
            None => Ok(false),
        }
    }

    pub fn enable(&self, port: u16) -> Result<(), String> {
        let current = self.backend.read()?;
        let old: Option<Lease> = read_json(&self.path).map_err(|e| e.to_string())?;
        let previous = match old {
            Some(l) if l.applied == current => l.previous,
            _ => current.clone(),
        };
        let applied = ProxySettings {
            flags: 3, // PROXY_TYPE_DIRECT | PROXY_TYPE_PROXY; PAC is restored on release.
            server: format!("http=127.0.0.1:{port};https=127.0.0.1:{port};socks=127.0.0.1:{port}"),
            bypass: "<local>;localhost;127.*;[::1]".into(),
            pac: String::new(),
        };
        // Write recovery data before touching Windows, including after a crash.
        write_json_atomic(
            &self.path,
            &Lease {
                previous: previous.clone(),
                applied: applied.clone(),
            },
        )
        .map_err(|e| format!("无法保存代理恢复信息：{e}"))?;
        if let Err(error) = self.backend.write(&applied) {
            // A WinINET refresh can fail after a successful write. Restore only our value.
            if self.backend.read().ok().as_ref() == Some(&applied) {
                let _ = self.backend.write(&previous);
            }
            return Err(error);
        }
        if self.backend.read()? != applied {
            return Err("系统代理设置未能生效".into());
        }
        Ok(())
    }

    pub fn owns_port(&self, port: u16) -> bool {
        let lease: Option<Lease> = read_json(&self.path).ok().flatten();
        lease.is_some_and(|l| {
            l.applied.server == format!("http=127.0.0.1:{port};https=127.0.0.1:{port};socks=127.0.0.1:{port}")
                && self.backend.read().ok().as_ref() == Some(&l.applied)
        })
    }

    pub fn release(&self) -> Result<(), String> {
        let Some(lease) = read_json::<Lease>(&self.path).map_err(|e| e.to_string())? else {
            return Ok(());
        };
        if self.backend.read()? == lease.applied {
            self.backend.write(&lease.previous)?;
        }
        // A different app's settings win: do not overwrite them on exit.
        std::fs::remove_file(&self.path).map_err(|e| format!("移除代理恢复记录失败：{e}"))
    }
}

struct WindowsProxy;
#[cfg(not(windows))]
impl ProxyBackend for WindowsProxy {
    fn read(&self) -> Result<ProxySettings, String> {
        Err("系统代理目前仅支持 Windows".into())
    }
    fn write(&self, _: &ProxySettings) -> Result<(), String> {
        Err("系统代理目前仅支持 Windows".into())
    }
}

#[cfg(windows)]
mod win {
    use super::*;
    use std::{ffi::c_void, mem::size_of, ptr};
    use windows_sys::Win32::Foundation::GlobalFree;
    use windows_sys::Win32::Networking::WinInet::*;

    fn wide(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(Some(0)).collect()
    }
    unsafe fn take_string(p: *mut u16) -> String {
        if p.is_null() {
            return String::new();
        }
        let mut len = 0;
        while *p.add(len) != 0 {
            len += 1;
        }
        let value = String::from_utf16_lossy(std::slice::from_raw_parts(p, len));
        GlobalFree(p as *mut c_void);
        value
    }
    fn options() -> [INTERNET_PER_CONN_OPTIONW; 4] {
        [
            INTERNET_PER_CONN_FLAGS,
            INTERNET_PER_CONN_PROXY_SERVER,
            INTERNET_PER_CONN_PROXY_BYPASS,
            INTERNET_PER_CONN_AUTOCONFIG_URL,
        ]
        .map(|dw_option| INTERNET_PER_CONN_OPTIONW {
            dwOption: dw_option,
            ..Default::default()
        })
    }
    fn list(opts: &mut [INTERNET_PER_CONN_OPTIONW; 4]) -> INTERNET_PER_CONN_OPTION_LISTW {
        INTERNET_PER_CONN_OPTION_LISTW {
            dwSize: size_of::<INTERNET_PER_CONN_OPTION_LISTW>() as u32,
            dwOptionCount: 4,
            pOptions: opts.as_mut_ptr(),
            ..Default::default()
        }
    }
    impl ProxyBackend for WindowsProxy {
        fn read(&self) -> Result<ProxySettings, String> {
            let mut opts = options();
            let mut l = list(&mut opts);
            let mut size = l.dwSize;
            unsafe {
                if InternetQueryOptionW(
                    ptr::null(),
                    INTERNET_OPTION_PER_CONNECTION_OPTION,
                    &mut l as *mut _ as *mut c_void,
                    &mut size,
                ) == 0
                {
                    return Err(format!("读取 Windows 代理失败：{}", std::io::Error::last_os_error()));
                }
                Ok(ProxySettings {
                    flags: opts[0].Value.dwValue,
                    server: take_string(opts[1].Value.pszValue),
                    bypass: take_string(opts[2].Value.pszValue),
                    pac: take_string(opts[3].Value.pszValue),
                })
            }
        }
        fn write(&self, value: &ProxySettings) -> Result<(), String> {
            let mut server = wide(&value.server);
            let mut bypass = wide(&value.bypass);
            let mut pac = wide(&value.pac);
            let mut opts = options();
            opts[0].Value.dwValue = value.flags;
            opts[1].Value.pszValue = server.as_mut_ptr();
            opts[2].Value.pszValue = bypass.as_mut_ptr();
            opts[3].Value.pszValue = pac.as_mut_ptr();
            let mut l = list(&mut opts);
            unsafe {
                if InternetSetOptionW(
                    ptr::null(),
                    INTERNET_OPTION_PER_CONNECTION_OPTION,
                    &mut l as *mut _ as *mut c_void,
                    l.dwSize,
                ) == 0
                {
                    return Err(format!("更新 Windows 代理失败：{}", std::io::Error::last_os_error()));
                }
                if InternetSetOptionW(ptr::null(), INTERNET_OPTION_SETTINGS_CHANGED, ptr::null(), 0) == 0 {
                    return Err(format!(
                        "通知 Windows 代理变更失败：{}",
                        std::io::Error::last_os_error()
                    ));
                }
                InternetSetOptionW(ptr::null(), INTERNET_OPTION_REFRESH, ptr::null(), 0);
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    struct Fake(Arc<Mutex<ProxySettings>>);
    impl ProxyBackend for Fake {
        fn read(&self) -> Result<ProxySettings, String> {
            Ok(self.0.lock().unwrap().clone())
        }
        fn write(&self, v: &ProxySettings) -> Result<(), String> {
            *self.0.lock().unwrap() = v.clone();
            Ok(())
        }
    }
    #[test]
    fn restores_previous_pac_after_repeated_port_changes() {
        let td = tempfile::tempdir().unwrap();
        let previous = ProxySettings {
            flags: 5,
            pac: "https://example.invalid/pac".into(),
            ..Default::default()
        };
        let state = Arc::new(Mutex::new(previous.clone()));
        let proxy = SystemProxy {
            path: td.path().join("lease.json"),
            backend: Box::new(Fake(state.clone())),
        };
        proxy.enable(7881).unwrap();
        proxy.enable(7882).unwrap();
        assert!(proxy.owns().unwrap());
        proxy.release().unwrap();
        assert_eq!(*state.lock().unwrap(), previous);
    }
    #[test]
    fn never_overwrites_a_proxy_changed_by_another_app() {
        let td = tempfile::tempdir().unwrap();
        let state = Arc::new(Mutex::new(ProxySettings::default()));
        let proxy = SystemProxy {
            path: td.path().join("lease.json"),
            backend: Box::new(Fake(state.clone())),
        };
        proxy.enable(7881).unwrap();
        let other = ProxySettings {
            flags: 3,
            server: "127.0.0.1:9999".into(),
            ..Default::default()
        };
        *state.lock().unwrap() = other.clone();
        proxy.release().unwrap();
        assert_eq!(*state.lock().unwrap(), other);
    }
}

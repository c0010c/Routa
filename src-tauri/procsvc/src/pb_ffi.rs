//! `Interceptor` 的真实实现：经 FFI 驱动 ProxyBridge 分支 DLL（ProxyBridgeCore.dll，C API 见
//! third_party/proxybridge/src/ProxyBridge.h）。
//!
//! 每个 Routa 规则 id 对应一条 ProxyBridge 规则，代理规则另有一份 ProxyBridge 代理配置
//! （首次 `set_proxy` 时创建、之后编辑、`remove_rule` 时删除）。

use std::collections::HashMap;
use std::ffi::{c_char, c_int, CString};
use std::mem::ManuallyDrop;
use std::net::{Ipv4Addr, TcpListener, UdpSocket};
use std::path::Path;
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};

use libloading::os::windows::{Library, LOAD_WITH_ALTERED_SEARCH_PATH};
use routa_procsvc_proto::{Flow, ProxyKind, ProxyTarget};

use crate::engine::{Interceptor, PbAction};
use crate::paths::{exe_of_pid, same_path};

pub const DLL_NAME: &str = "ProxyBridgeCore.dll";

type Bool = c_int;
const TRUE: Bool = 1;
const FALSE: Bool = 0;

// ProxyType
const PROXY_TYPE_HTTP: c_int = 0;
const PROXY_TYPE_SOCKS5: c_int = 1;
// RuleAction
const RULE_ACTION_PROXY: c_int = 0;
const RULE_ACTION_DIRECT: c_int = 1;
const RULE_ACTION_BLOCK: c_int = 2;
// RuleProtocol
const RULE_PROTOCOL_BOTH: c_int = 2;

const PB_FLOWS_MAX: usize = 400;
const AGE_UNKNOWN: u32 = 0xFFFF_FFFF;

/// 与 ProxyBridge.h 中的 `PB_FLOW` 逐字段一致。
#[repr(C)]
#[derive(Clone, Copy)]
struct PbFlow {
    id: u64,
    pid: u32,
    proxy_config_id: u32,
    rule_id: u32,
    age_ms: u32,
    sent: u64,
    received: u64,
    port: u16,
    is_udp: u8,
    is_ipv6: u8,
    dest: [u8; 16],
    name: [c_char; 40],
    host: [c_char; 64],
}

type AddProxyConfigFn =
    unsafe extern "C" fn(c_int, *const c_char, u16, *const c_char, *const c_char, Bool) -> u32;
type EditProxyConfigFn = unsafe extern "C" fn(
    u32,
    c_int,
    *const c_char,
    u16,
    *const c_char,
    *const c_char,
    Bool,
) -> Bool;
type AddRuleFn = unsafe extern "C" fn(
    *const c_char,
    *const c_char,
    *const c_char,
    *const c_char,
    c_int,
    c_int,
    u32,
    Bool,
) -> u32;
type EditRuleFn = unsafe extern "C" fn(
    u32,
    *const c_char,
    *const c_char,
    *const c_char,
    *const c_char,
    c_int,
    c_int,
    u32,
    Bool,
) -> Bool;
type IdFn = unsafe extern "C" fn(u32) -> Bool;
type GetFlowsFn = unsafe extern "C" fn(*mut PbFlow, u32) -> u32;
type SetRelayPortsFn = unsafe extern "C" fn(u16, u16) -> Bool;
type VoidBoolFn = unsafe extern "C" fn() -> Bool;

/// 加载时一次性解析好的导出函数。
struct Api {
    add_proxy_config: AddProxyConfigFn,
    edit_proxy_config: EditProxyConfigFn,
    delete_proxy_config: IdFn,
    add_rule: AddRuleFn,
    edit_rule: EditRuleFn,
    delete_rule: IdFn,
    get_flows: GetFlowsFn,
    set_relay_ports: SetRelayPortsFn,
    start: VoidBoolFn,
    stop: VoidBoolFn,
}

impl Api {
    /// # Safety
    /// `lib` 必须是 ProxyBridgeCore.dll，且在返回的函数指针使用期间保持加载。
    unsafe fn resolve(lib: &Library) -> Result<Api, String> {
        macro_rules! sym {
            ($name:literal) => {
                *lib.get(concat!($name, "\0").as_bytes())
                    .map_err(|e| format!("{DLL_NAME} 缺少导出 {}：{e}", $name))?
            };
        }
        Ok(Api {
            add_proxy_config: sym!("ProxyBridge_AddProxyConfig"),
            edit_proxy_config: sym!("ProxyBridge_EditProxyConfig"),
            delete_proxy_config: sym!("ProxyBridge_DeleteProxyConfig"),
            add_rule: sym!("ProxyBridge_AddRule"),
            edit_rule: sym!("ProxyBridge_EditRule"),
            delete_rule: sym!("ProxyBridge_DeleteRule"),
            get_flows: sym!("ProxyBridge_GetFlows"),
            set_relay_ports: sym!("ProxyBridge_SetRelayPorts"),
            start: sym!("ProxyBridge_Start"),
            stop: sym!("ProxyBridge_Stop"),
        })
    }
}

/// DLL 持有全局状态，一个进程里只能有一个 [`ProxyBridge`]。
static LOADED: AtomicBool = AtomicBool::new(false);

/// 一条 Routa 规则在 ProxyBridge 里的落地。
#[derive(Debug, Default)]
struct Entry {
    /// ProxyBridge 规则 id；只设过代理、还没 `set_rule` 时为 None。
    pb_rule: Option<u32>,
    /// ProxyBridge 代理配置 id。
    cfg: Option<u32>,
    /// 规则的 exe（`set_rule` 时的写法），用于把连接归到规则上。
    exe: String,
}

pub struct ProxyBridge {
    api: Api,
    /// 从不卸载：Stop 等待工作线程有超时，线程可能比 Stop 活得久，卸载后会执行已卸载的代码。
    _lib: ManuallyDrop<Library>,
    rules: HashMap<String, Entry>,
}

/// 各取一个当前空闲的本机 TCP / UDP 端口（随即释放），作为 ProxyBridge 的中转端口。两者保证不同。
pub fn pick_relay_ports() -> Result<(u16, u16), String> {
    let err = |e: std::io::Error| format!("获取空闲中转端口失败：{e}");
    for _ in 0..8 {
        let tcp = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .and_then(|l| l.local_addr())
            .map_err(err)?
            .port();
        let udp = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0))
            .and_then(|s| s.local_addr())
            .map_err(err)?
            .port();
        if tcp != udp {
            return Ok((tcp, udp));
        }
    }
    Err("获取空闲中转端口失败：TCP 与 UDP 端口总是相同".into())
}

/// 从 `dir`（必须是绝对路径）加载 ProxyBridgeCore.dll，设置中转端口 `(tcp, udp)` 并启动。
/// 同一进程内已有实例时返回错误。
pub fn load(dir: &Path, relay_ports: (u16, u16)) -> Result<ProxyBridge, String> {
    if !dir.is_absolute() {
        return Err(format!("{DLL_NAME} 目录必须是绝对路径：{}", dir.display()));
    }
    if LOADED.swap(true, Ordering::AcqRel) {
        return Err(format!("{DLL_NAME} 已在本进程中加载"));
    }
    let result = start(&dir.join(DLL_NAME), relay_ports);
    if result.is_err() {
        LOADED.store(false, Ordering::Release);
    }
    result
}

fn start(path: &Path, (tcp, udp): (u16, u16)) -> Result<ProxyBridge, String> {
    // SAFETY: 绝对路径 + LOAD_WITH_ALTERED_SEARCH_PATH，依赖（WinDivert.dll）从 DLL 自身目录解析，
    // 不搜索默认 DLL 路径。DllMain 无额外前置条件。
    let lib = unsafe { Library::load_with_flags(path, LOAD_WITH_ALTERED_SEARCH_PATH) }
        .map_err(|e| format!("加载 {} 失败：{e}", path.display()))?;
    let lib = ManuallyDrop::new(lib);
    // SAFETY: lib 永不卸载。
    let api = unsafe { Api::resolve(&lib)? };
    // SAFETY: 以下均为无指针参数的 C 调用。
    if unsafe { (api.set_relay_ports)(tcp, udp) } == FALSE {
        return Err(format!("设置 ProxyBridge 中转端口 {tcp}/{udp} 失败"));
    }
    if unsafe { (api.start)() } == FALSE {
        return Err("启动 ProxyBridge 失败（需要管理员权限与 WinDivert 驱动）".into());
    }
    Ok(ProxyBridge {
        api,
        _lib: lib,
        rules: HashMap::new(),
    })
}

fn cstring(what: &str, s: &str) -> Result<CString, String> {
    CString::new(s).map_err(|_| format!("{what} 含 NUL 字符：{s:?}"))
}

fn opt_ptr(s: &Option<CString>) -> *const c_char {
    s.as_ref().map_or(ptr::null(), |c| c.as_ptr())
}

/// 定长 C 字符串字段（UTF-8，NUL 结尾；没有 NUL 时取整段）。
fn field_str(field: &[c_char]) -> String {
    // c_char 与 u8 同宽，按字节解释。
    let bytes: Vec<u8> = field.iter().map(|&c| c as u8).collect();
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

/// 把一条原始连接转成 `Flow`：`exe` 为该 PID 的完整路径，按 exe 归到有 ProxyBridge 规则的 Routa 规则上。
fn to_flow(raw: &PbFlow, exe: Option<String>, rules: &HashMap<String, Entry>) -> Flow {
    let rule_id = exe.as_deref().and_then(|exe| {
        rules
            .iter()
            .find(|(_, e)| e.pb_rule.is_some() && same_path(&e.exe, exe))
            .map(|(id, _)| id.clone())
    });
    Flow {
        pid: raw.pid,
        rule_id,
        exe_path: exe,
        host: field_str(&raw.host),
        port: raw.port,
        udp: raw.is_udp != 0,
        age_ms: (raw.age_ms != AGE_UNKNOWN).then_some(raw.age_ms),
        sent: raw.sent,
        received: raw.received,
    }
}

impl Interceptor for ProxyBridge {
    fn set_proxy(&mut self, rule: &str, target: &ProxyTarget) -> Result<(), String> {
        let kind = match target.kind {
            ProxyKind::Socks5 => PROXY_TYPE_SOCKS5,
            ProxyKind::Http => PROXY_TYPE_HTTP,
        };
        let host = cstring("代理地址", &target.host)?;
        let user = target
            .username
            .as_deref()
            .map(|u| cstring("代理用户名", u))
            .transpose()?;
        let pass = target
            .password
            .as_deref()
            .map(|p| cstring("代理密码", p))
            .transpose()?;
        let entry = self.rules.entry(rule.to_string()).or_default();
        // SAFETY: 所有指针指向本函数内存活的 CString 或为 NULL。
        match entry.cfg {
            Some(cfg) => {
                let ok = unsafe {
                    (self.api.edit_proxy_config)(
                        cfg,
                        kind,
                        host.as_ptr(),
                        target.port,
                        opt_ptr(&user),
                        opt_ptr(&pass),
                        TRUE,
                    )
                };
                if ok == FALSE {
                    return Err(format!("修改规则 {rule} 的 ProxyBridge 代理配置失败"));
                }
            }
            None => {
                let cfg = unsafe {
                    (self.api.add_proxy_config)(
                        kind,
                        host.as_ptr(),
                        target.port,
                        opt_ptr(&user),
                        opt_ptr(&pass),
                        TRUE,
                    )
                };
                if cfg == 0 {
                    if entry.pb_rule.is_none() {
                        self.rules.remove(rule);
                    }
                    return Err(format!("添加规则 {rule} 的 ProxyBridge 代理配置失败"));
                }
                entry.cfg = Some(cfg);
            }
        }
        Ok(())
    }

    fn set_rule(&mut self, rule: &str, exe: &str, action: PbAction) -> Result<(), String> {
        let existing = self.rules.get(rule);
        let (pb_action, cfg) = match action {
            PbAction::Proxy => match existing.and_then(|e| e.cfg) {
                Some(cfg) => (RULE_ACTION_PROXY, cfg),
                None => return Err(format!("规则 {rule} 还没有代理配置")),
            },
            PbAction::Direct => (RULE_ACTION_DIRECT, 0),
            PbAction::Block => (RULE_ACTION_BLOCK, 0),
        };
        let exe_c = cstring("exe 路径", exe)?;
        let (any, empty) = (c"*", c"");
        let pb_rule = existing.and_then(|e| e.pb_rule);
        // SAFETY: 所有指针指向本函数内存活的 C 字符串。
        match pb_rule {
            Some(id) => {
                let ok = unsafe {
                    (self.api.edit_rule)(
                        id,
                        exe_c.as_ptr(),
                        any.as_ptr(),
                        any.as_ptr(),
                        empty.as_ptr(),
                        RULE_PROTOCOL_BOTH,
                        pb_action,
                        cfg,
                        FALSE,
                    )
                };
                if ok == FALSE {
                    return Err(format!("修改规则 {rule} 的 ProxyBridge 规则失败"));
                }
            }
            None => {
                let id = unsafe {
                    (self.api.add_rule)(
                        exe_c.as_ptr(),
                        any.as_ptr(),
                        any.as_ptr(),
                        empty.as_ptr(),
                        RULE_PROTOCOL_BOTH,
                        pb_action,
                        cfg,
                        FALSE,
                    )
                };
                if id == 0 {
                    return Err(format!("添加规则 {rule} 的 ProxyBridge 规则失败"));
                }
                self.rules.entry(rule.to_string()).or_default().pb_rule = Some(id);
            }
        }
        if let Some(e) = self.rules.get_mut(rule) {
            e.exe = exe.to_string();
        }
        Ok(())
    }

    fn remove_rule(&mut self, rule: &str) -> Result<(), String> {
        let Some(entry) = self.rules.get_mut(rule) else {
            return Ok(());
        };
        if let Some(id) = entry.pb_rule {
            // SAFETY: 无指针参数。
            if unsafe { (self.api.delete_rule)(id) } == FALSE {
                return Err(format!("删除规则 {rule} 的 ProxyBridge 规则失败"));
            }
            entry.pb_rule = None;
        }
        if let Some(cfg) = entry.cfg {
            // SAFETY: 无指针参数。
            if unsafe { (self.api.delete_proxy_config)(cfg) } == FALSE {
                return Err(format!("删除规则 {rule} 的 ProxyBridge 代理配置失败"));
            }
        }
        self.rules.remove(rule);
        Ok(())
    }

    fn flows(&mut self) -> Vec<Flow> {
        let mut buf: Vec<PbFlow> = Vec::with_capacity(PB_FLOWS_MAX);
        // SAFETY: buf 有 PB_FLOWS_MAX 个元素的容量；DLL 最多写入 max 个，返回写入的个数。
        let n = unsafe { (self.api.get_flows)(buf.as_mut_ptr(), PB_FLOWS_MAX as u32) };
        unsafe { buf.set_len((n as usize).min(PB_FLOWS_MAX)) };
        let mut exes: HashMap<u32, Option<String>> = HashMap::new();
        buf.iter()
            .map(|raw| {
                let exe = exes
                    .entry(raw.pid)
                    .or_insert_with(|| exe_of_pid(raw.pid))
                    .clone();
                to_flow(raw, exe, &self.rules)
            })
            .collect()
    }
}

impl Drop for ProxyBridge {
    fn drop(&mut self) {
        // 清掉自己下发的规则与代理配置，再停止；DLL 不卸载，之后重新 load 时状态是干净的。
        let ids: Vec<String> = self.rules.keys().cloned().collect();
        for id in ids {
            let _ = self.remove_rule(&id);
        }
        // SAFETY: 无指针参数。
        unsafe { (self.api.stop)() };
        LOADED.store(false, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{offset_of, size_of};

    #[test]
    fn pb_flow_layout_matches_header() {
        assert_eq!(offset_of!(PbFlow, id), 0);
        assert_eq!(offset_of!(PbFlow, pid), 8);
        assert_eq!(offset_of!(PbFlow, proxy_config_id), 12);
        assert_eq!(offset_of!(PbFlow, rule_id), 16);
        assert_eq!(offset_of!(PbFlow, age_ms), 20);
        assert_eq!(offset_of!(PbFlow, sent), 24);
        assert_eq!(offset_of!(PbFlow, received), 32);
        assert_eq!(offset_of!(PbFlow, port), 40);
        assert_eq!(offset_of!(PbFlow, is_udp), 42);
        assert_eq!(offset_of!(PbFlow, is_ipv6), 43);
        assert_eq!(offset_of!(PbFlow, dest), 44);
        assert_eq!(offset_of!(PbFlow, name), 60);
        assert_eq!(offset_of!(PbFlow, host), 100);
        assert_eq!(size_of::<PbFlow>(), 168);
    }

    fn raw(pid: u32, host: &str, age_ms: u32, udp: bool) -> PbFlow {
        let mut f = PbFlow {
            id: 1,
            pid,
            proxy_config_id: 1,
            rule_id: 1,
            age_ms,
            sent: 10,
            received: 20,
            port: 443,
            is_udp: udp as u8,
            is_ipv6: 0,
            dest: [0; 16],
            name: [0; 40],
            host: [0; 64],
        };
        for (d, s) in f.host.iter_mut().zip(host.bytes()) {
            *d = s as c_char;
        }
        f
    }

    #[test]
    fn field_str_reads_utf8_up_to_nul() {
        let f = raw(1, "例子.com", 0, false);
        assert_eq!(field_str(&f.host), "例子.com");
        let full = [b'a' as c_char; 4];
        assert_eq!(field_str(&full), "aaaa");
    }

    #[test]
    fn to_flow_maps_rule_by_exe_and_unknown_age() {
        let mut rules = HashMap::new();
        rules.insert(
            "r1".to_string(),
            Entry {
                pb_rule: Some(7),
                cfg: Some(3),
                exe: r"C:\Apps\Foo.exe".into(),
            },
        );
        rules.insert(
            "r2".to_string(),
            Entry {
                pb_rule: None,
                cfg: Some(4),
                exe: r"C:\Apps\Bar.exe".into(),
            },
        );

        let f = to_flow(
            &raw(42, "example.com", AGE_UNKNOWN, true),
            Some("c:/apps/foo.EXE".into()),
            &rules,
        );
        assert_eq!(f.rule_id.as_deref(), Some("r1"));
        assert_eq!(f.exe_path.as_deref(), Some("c:/apps/foo.EXE"));
        assert_eq!(f.host, "example.com");
        assert_eq!((f.pid, f.port, f.udp, f.age_ms), (42, 443, true, None));
        assert_eq!((f.sent, f.received), (10, 20));

        let f = to_flow(&raw(43, "1.2.3.4", 1500, false), None, &rules);
        assert_eq!((f.rule_id, f.exe_path, f.age_ms), (None, None, Some(1500)));

        // 没有 ProxyBridge 规则的条目不参与归属。
        let f = to_flow(
            &raw(44, "x", 0, false),
            Some(r"C:\Apps\Bar.exe".into()),
            &rules,
        );
        assert_eq!(f.rule_id, None);
    }

    #[test]
    fn relay_ports_differ() {
        let (tcp, udp) = pick_relay_ports().unwrap();
        assert!(tcp != 0 && udp != 0 && tcp != udp);
    }

    #[test]
    fn load_rejects_relative_dir() {
        let err = load(Path::new("binaries"), (1, 2)).err().unwrap();
        assert!(err.contains("绝对路径"), "{err}");
    }

    #[test]
    #[ignore = "needs admin + built DLL"]
    fn start_add_rule_stop() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("binaries")
            .join("procsvc");
        let mut pb = load(&dir, pick_relay_ports().unwrap()).expect("load + start");
        assert!(
            load(&dir, (1, 2)).is_err(),
            "second instance must be refused"
        );
        pb.set_rule("t", r"C:\Windows\System32\curl.exe", PbAction::Block)
            .expect("add block rule");
        // 期间 `curl https://example.com` 应失败。
        std::thread::sleep(std::time::Duration::from_secs(15));
        let _ = pb.flows();
        pb.remove_rule("t").expect("remove rule");
        pb.remove_rule("t").expect("remove unknown rule is Ok");
        drop(pb);
    }
}

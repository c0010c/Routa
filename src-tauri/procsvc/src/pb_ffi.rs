//! `Interceptor` 的真实实现：经 FFI 驱动 ProxyBridge 分支 DLL（ProxyBridgeCore.dll，C API 见
//! third_party/proxybridge/src/ProxyBridge.h）。
//!
//! 每个 Routa 规则 id 对应一条 ProxyBridge 规则。代理配置按 `ProxyTarget` 去重共用（[`ProxyPool`]，
//! 引用归零时删除）；目标变化时换一份配置，不原地编辑，以免影响共用它的其他规则。
//! ProxyBridge 按列表顺序匹配、先中先得，所以按完整路径的规则新增后挪到最前，压过按进程名的规则。
//!
//! [`load`] 只加载 DLL，不启动；启停由 [`crate::lazy::Lazy`] 按规则集是否为空决定（[`Bridge`]）。

use std::collections::HashMap;
use std::ffi::{c_char, c_int, CString};
use std::mem::ManuallyDrop;
use std::net::{Ipv4Addr, TcpListener, UdpSocket};
use std::path::Path;
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};

use libloading::os::windows::{
    Library, LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR, LOAD_LIBRARY_SEARCH_SYSTEM32,
};
use routa_procsvc_proto::{Flow, ProxyKind, ProxyTarget};

use crate::engine::{Interceptor, PbAction};
use crate::lazy::Bridge;
use crate::paths::{exe_of_pid, matches_process, pattern_kind};
use crate::proxypool::{Attach, ProxyPool};
use routa_procsvc_proto::ProcessKind;

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
type MoveRuleFn = unsafe extern "C" fn(u32, u32) -> Bool;
type GetFlowsFn = unsafe extern "C" fn(*mut PbFlow, u32) -> u32;
type SetRelayPortsFn = unsafe extern "C" fn(u16, u16) -> Bool;
type VoidBoolFn = unsafe extern "C" fn() -> Bool;

/// 加载时一次性解析好的导出函数。
struct Api {
    add_proxy_config: AddProxyConfigFn,
    delete_proxy_config: IdFn,
    add_rule: AddRuleFn,
    edit_rule: EditRuleFn,
    delete_rule: IdFn,
    move_rule_to_position: MoveRuleFn,
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
            delete_proxy_config: sym!("ProxyBridge_DeleteProxyConfig"),
            add_rule: sym!("ProxyBridge_AddRule"),
            edit_rule: sym!("ProxyBridge_EditRule"),
            delete_rule: sym!("ProxyBridge_DeleteRule"),
            move_rule_to_position: sym!("ProxyBridge_MoveRuleToPosition"),
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
    /// 已不被引用、等 ProxyBridge 规则改指别处（或删除）后再删的代理配置。
    stale: Vec<u32>,
    /// 规则的 exe（`set_rule` 时的写法），用于把连接归到规则上。
    exe: String,
    /// ProxyBridge 规则当前的动作是代理（因而引用着一份代理配置）。
    proxied: bool,
}

pub struct ProxyBridge {
    api: Api,
    /// 从不卸载：Stop 等待工作线程有超时，线程可能比 Stop 活得久，卸载后会执行已卸载的代码。
    _lib: ManuallyDrop<Library>,
    rules: HashMap<String, Entry>,
    pool: ProxyPool,
    /// `ProxyBridge_Start` 成功、尚未 `ProxyBridge_Stop`。
    running: bool,
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

/// 从 `dir`（必须是绝对路径）加载 ProxyBridgeCore.dll（不启动）。同一进程内已有实例时返回错误。
pub fn load(dir: &Path) -> Result<ProxyBridge, String> {
    if !dir.is_absolute() {
        return Err(format!("{DLL_NAME} 目录必须是绝对路径：{}", dir.display()));
    }
    if LOADED.swap(true, Ordering::AcqRel) {
        return Err(format!("{DLL_NAME} 已在本进程中加载"));
    }
    let result = open(&dir.join(DLL_NAME));
    if result.is_err() {
        LOADED.store(false, Ordering::Release);
    }
    result
}

fn open(path: &Path) -> Result<ProxyBridge, String> {
    // SAFETY: 绝对路径 + LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32：
    // 依赖（WinDivert.dll）只从 DLL 自身目录和 System32 解析，从不搜索当前目录或 PATH
    // （以 LocalSystem 运行的服务不能被这些位置的同名 DLL 劫持）。DllMain 无额外前置条件。
    let flags = LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32;
    let lib = unsafe { Library::load_with_flags(path, flags) }
        .map_err(|e| format!("加载 {} 失败：{e}", path.display()))?;
    let lib = ManuallyDrop::new(lib);
    // SAFETY: lib 永不卸载。
    let api = unsafe { Api::resolve(&lib)? };
    Ok(ProxyBridge {
        api,
        _lib: lib,
        rules: HashMap::new(),
        pool: ProxyPool::default(),
        running: false,
    })
}

impl Bridge for ProxyBridge {
    /// 每次启动都重新挑空闲的中转端口：停止期间原端口可能已被别的程序占用。
    fn start(&mut self) -> Result<(), String> {
        if self.running {
            return Ok(());
        }
        let (tcp, udp) = pick_relay_ports()?;
        // SAFETY: 以下均为无指针参数的 C 调用。
        if unsafe { (self.api.set_relay_ports)(tcp, udp) } == FALSE {
            return Err(format!("设置 ProxyBridge 中转端口 {tcp}/{udp} 失败"));
        }
        if unsafe { (self.api.start)() } == FALSE {
            return Err("启动 ProxyBridge 失败（需要管理员权限与 WinDivert 驱动）".into());
        }
        self.running = true;
        Ok(())
    }

    fn stop(&mut self) {
        if self.running {
            // SAFETY: 无指针参数。DLL 的 Stop 会等工作线程退出并清空连接表、端口缓存，之后可再次 Start。
            unsafe { (self.api.stop)() };
            self.running = false;
        }
    }

    fn clear(&mut self) -> Result<(), String> {
        let ids: Vec<String> = self.rules.keys().cloned().collect();
        let errors: Vec<String> = ids
            .iter()
            .filter_map(|id| self.remove_rule(id).err())
            .collect();
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors.join("；"))
        }
    }
}

fn cstring(what: &str, s: &str) -> Result<CString, String> {
    CString::new(s).map_err(|_| format!("{what} 含 NUL 字符：{s:?}"))
}

/// 交给 DLL 的进程路径：去首尾空白、`/` 统一为 `\`（DLL 按 OS 路径的 `\` 写法匹配）。
/// DLL 把 `,` `;` 当作列表分隔符，含这两个字符的路径永远匹配不上，直接拒绝，免得规则"装上却不生效"。
fn pb_exe(exe: &str) -> Result<CString, String> {
    let exe = exe.trim().replace('/', "\\");
    if exe.contains([',', ';']) {
        return Err(format!("exe 路径含 ProxyBridge 不支持的 `,` 或 `;`：{exe}"));
    }
    cstring("exe 路径", &exe)
}

/// 删掉 `entry` 里不再被引用的代理配置；删除失败的留待下次。
fn flush_stale(api: &Api, entry: &mut Entry) {
    // SAFETY: 无指针参数。
    entry
        .stale
        .retain(|&cfg| unsafe { (api.delete_proxy_config)(cfg) } == FALSE);
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
            .find(|(_, e)| e.pb_rule.is_some() && matches_process(pattern_kind(&e.exe), &e.exe, exe))
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
        // 目标没变：沿用现有配置（编辑它会影响共用者，也没有必要）。
        if self.pool.target_of(rule) == Some(target) && self.pool.cfg_of(rule).is_some() {
            return Ok(());
        }
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
        // 旧配置可能还被这条 ProxyBridge 规则引用：归零时先记下，等规则改指新配置后再删。
        if let Some(old) = self.pool.detach(rule) {
            entry.stale.push(old);
        }
        let mut cfg = match self.pool.attach(rule, target) {
            Attach::Reuse(cfg) => cfg,
            Attach::Create => 0,
        };
        if cfg == 0 {
            // SAFETY: 所有指针指向本函数内存活的 CString 或为 NULL。
            cfg = unsafe {
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
                self.pool.detach(rule);
                if entry.pb_rule.is_none() && entry.stale.is_empty() {
                    self.rules.remove(rule);
                }
                return Err(format!("添加规则 {rule} 的 ProxyBridge 代理配置失败"));
            }
            self.pool.created(rule, target, cfg);
        }
        // plan 在只有目标变化时只发 SetProxy：已按代理动作生效的 ProxyBridge 规则要在这里改指新配置，
        // 然后才能删掉旧配置。
        if let (Some(id), true) = (entry.pb_rule, entry.proxied) {
            let exe_c = pb_exe(&entry.exe)?;
            let (any, empty) = (c"*", c"");
            // SAFETY: 所有指针指向本函数内存活的 C 字符串。
            let ok = unsafe {
                (self.api.edit_rule)(
                    id,
                    exe_c.as_ptr(),
                    any.as_ptr(),
                    any.as_ptr(),
                    empty.as_ptr(),
                    RULE_PROTOCOL_BOTH,
                    RULE_ACTION_PROXY,
                    cfg,
                    FALSE,
                )
            };
            if ok == FALSE {
                return Err(format!("修改规则 {rule} 的 ProxyBridge 规则失败"));
            }
            flush_stale(&self.api, entry);
        }
        Ok(())
    }

    fn set_rule(&mut self, rule: &str, exe: &str, action: PbAction) -> Result<(), String> {
        let (pb_action, cfg) = match action {
            PbAction::Proxy => match self.pool.cfg_of(rule) {
                Some(cfg) => (RULE_ACTION_PROXY, cfg),
                None => return Err(format!("规则 {rule} 还没有代理配置")),
            },
            PbAction::Direct => (RULE_ACTION_DIRECT, 0),
            PbAction::Block => (RULE_ACTION_BLOCK, 0),
        };
        let exe_c = pb_exe(exe)?;
        let (any, empty) = (c"*", c"");
        let pb_rule = self.rules.get(rule).and_then(|e| e.pb_rule);
        // SAFETY: 所有指针指向本函数内存活的 C 字符串。
        let id = match pb_rule {
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
                id
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
                id
            }
        };
        let e = self.rules.entry(rule.to_string()).or_default();
        e.exe = exe.to_string();
        e.proxied = action == PbAction::Proxy;
        // 非代理动作不再需要代理配置：放掉引用，归零的配置删除（DLL 最多 32 份）。
        if action != PbAction::Proxy {
            if let Some(old) = self.pool.detach(rule) {
                e.stale.push(old);
            }
        }
        // 规则已改指别处：删掉不再被引用的配置。失败时留着，下次再试；规则本身已生效，不报错。
        flush_stale(&self.api, e);
        // 按完整路径的规则排到最前，先于按进程名的规则匹配（先中先得）。
        if pattern_kind(exe) == ProcessKind::Path
            // SAFETY: 无指针参数。
            && unsafe { (self.api.move_rule_to_position)(id, 1) } == FALSE
        {
            return Err(format!("移动规则 {rule} 的 ProxyBridge 规则失败"));
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
        if let Some(old) = self.pool.detach(rule) {
            entry.stale.push(old);
        }
        flush_stale(&self.api, entry);
        if !entry.stale.is_empty() {
            return Err(format!("删除规则 {rule} 的 ProxyBridge 代理配置失败"));
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
        let _ = self.clear();
        self.stop();
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
                stale: vec![],
                proxied: false,
                exe: r"C:\Apps\Foo.exe".into(),
            },
        );
        rules.insert(
            "r2".to_string(),
            Entry {
                pb_rule: None,
                stale: vec![],
                proxied: false,
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

        // 只写进程名的规则按文件名归属。
        rules.insert(
            "r3".to_string(),
            Entry {
                pb_rule: Some(9),
                stale: vec![],
                proxied: false,
                exe: "Baz.exe".into(),
            },
        );
        let f = to_flow(&raw(45, "x", 0, false), Some(r"D:\Any\where\baz.EXE".into()), &rules);
        assert_eq!(f.rule_id.as_deref(), Some("r3"));
    }

    #[test]
    fn pb_exe_normalizes_and_rejects_separators() {
        let ok = |s: &str| pb_exe(s).unwrap().into_string().unwrap();
        assert_eq!(ok(" C:/Apps/测试/x.exe "), r"C:\Apps\测试\x.exe");
        assert_eq!(ok(r"C:\a\b.exe"), r"C:\a\b.exe");
        assert!(pb_exe(r"D:\a;b\x.exe").is_err());
        assert!(pb_exe(r"D:\a,b\x.exe").is_err());
        assert!(pb_exe("C:\\a\0.exe").is_err());
    }

    /// 假 DLL：记录调用，用来验证簿记。只给 `bookkeeping_with_fake_dll` 一个测试用（共享静态状态）。
    mod fake {
        use super::*;
        use std::ffi::CStr;
        use std::sync::Mutex;

        pub static LOG: Mutex<Vec<String>> = Mutex::new(Vec::new());
        static NEXT: Mutex<u32> = Mutex::new(0);

        fn log(s: String) {
            LOG.lock().unwrap().push(s);
        }
        fn next() -> u32 {
            let mut n = NEXT.lock().unwrap();
            *n += 1;
            *n
        }
        unsafe extern "C" fn add_cfg(
            _: c_int,
            _: *const c_char,
            _: u16,
            _: *const c_char,
            _: *const c_char,
            _: Bool,
        ) -> u32 {
            let id = next();
            log(format!("add_cfg {id}"));
            id
        }
        unsafe extern "C" fn move_rule(id: u32, pos: u32) -> Bool {
            log(format!("move_rule {id} {pos}"));
            TRUE
        }
        unsafe extern "C" fn del_cfg(id: u32) -> Bool {
            log(format!("del_cfg {id}"));
            TRUE
        }
        #[allow(clippy::too_many_arguments)]
        unsafe extern "C" fn add_rule(
            exe: *const c_char,
            _: *const c_char,
            _: *const c_char,
            _: *const c_char,
            _: c_int,
            action: c_int,
            cfg: u32,
            _: Bool,
        ) -> u32 {
            let id = next();
            let exe = CStr::from_ptr(exe).to_string_lossy();
            log(format!("add_rule {id} {exe} {action} {cfg}"));
            id
        }
        #[allow(clippy::too_many_arguments)]
        unsafe extern "C" fn edit_rule(
            id: u32,
            exe: *const c_char,
            _: *const c_char,
            _: *const c_char,
            _: *const c_char,
            _: c_int,
            action: c_int,
            cfg: u32,
            _: Bool,
        ) -> Bool {
            let exe = CStr::from_ptr(exe).to_string_lossy();
            log(format!("edit_rule {id} {exe} {action} {cfg}"));
            TRUE
        }
        unsafe extern "C" fn del_rule(id: u32) -> Bool {
            log(format!("del_rule {id}"));
            TRUE
        }
        unsafe extern "C" fn get_flows(_: *mut PbFlow, _: u32) -> u32 {
            0
        }
        unsafe extern "C" fn set_ports(_: u16, _: u16) -> Bool {
            TRUE
        }
        unsafe extern "C" fn noop() -> Bool {
            TRUE
        }

        pub fn bridge() -> ProxyBridge {
            ProxyBridge {
                api: Api {
                    add_proxy_config: add_cfg,
                    delete_proxy_config: del_cfg,
                    add_rule,
                    edit_rule,
                    delete_rule: del_rule,
                    move_rule_to_position: move_rule,
                    get_flows,
                    set_relay_ports: set_ports,
                    start: noop,
                    stop: noop,
                },
                _lib: ManuallyDrop::new(Library::this().unwrap()),
                rules: HashMap::new(),
                pool: ProxyPool::default(),
                running: false,
            }
        }
    }

    #[test]
    fn bookkeeping_with_fake_dll() {
        let target = |port| ProxyTarget {
            kind: ProxyKind::Socks5,
            host: "127.0.0.1".into(),
            port,
            username: None,
            password: None,
        };
        let (t1, t2) = (target(7890), target(7891));
        let take = || std::mem::take(&mut *fake::LOG.lock().unwrap());
        let mut pb = fake::bridge();

        assert!(pb.set_rule("r", r"C:\x.exe", PbAction::Proxy).is_err());
        assert!(pb.set_rule("r", "C:/a;b/x.exe", PbAction::Block).is_err());
        assert!(take().is_empty());

        // 完整路径的规则新增后挪到最前；目标没变时不再动代理配置。
        pb.set_proxy("r", &t1).unwrap();
        pb.set_rule("r", " C:/Apps/x.exe", PbAction::Proxy).unwrap();
        pb.set_proxy("r", &t1).unwrap();
        assert_eq!(
            take(),
            ["add_cfg 1", r"add_rule 2 C:\Apps\x.exe 0 1", "move_rule 2 1"]
        );

        // 改成非代理动作：规则改写后删掉代理配置。
        pb.set_rule("r", r"C:\Apps\x.exe", PbAction::Direct)
            .unwrap();
        assert_eq!(
            take(),
            [r"edit_rule 2 C:\Apps\x.exe 1 0", "del_cfg 1", "move_rule 2 1"]
        );
        assert_eq!(pb.pool.cfg_of("r"), None);

        // 改回代理：plan 先 SetProxy 重建配置。
        pb.set_proxy("r", &t1).unwrap();
        pb.set_rule("r", r"C:\Apps\x.exe", PbAction::Proxy).unwrap();
        assert_eq!(
            take(),
            ["add_cfg 3", r"edit_rule 2 C:\Apps\x.exe 0 3", "move_rule 2 1"]
        );

        pb.remove_rule("r").unwrap();
        assert_eq!(take(), ["del_rule 2", "del_cfg 3"]);
        pb.remove_rule("r").unwrap();
        pb.remove_rule("unknown").unwrap();
        assert!(take().is_empty());

        // 同一目标共用一份配置；进程名规则不挪位置；最后一个引用走了才删配置。
        pb.set_proxy("a", &t1).unwrap();
        pb.set_proxy("b", &t1).unwrap();
        pb.set_rule("a", "A.exe", PbAction::Proxy).unwrap();
        pb.set_rule("b", r"C:\b.exe", PbAction::Proxy).unwrap();
        assert_eq!(
            take(),
            [
                "add_cfg 4",
                "add_rule 5 A.exe 0 4",
                r"add_rule 6 C:\b.exe 0 4",
                "move_rule 6 1"
            ]
        );
        pb.remove_rule("a").unwrap();
        assert_eq!(take(), ["del_rule 5"]);

        // 目标变化：换一份新配置，规则改指新配置后才删旧的。
        // plan 只发 SetProxy（不再发 SetRule）：规则在这一步就改指新配置。
        pb.set_proxy("b", &t2).unwrap();
        assert_eq!(
            take(),
            ["add_cfg 7", r"edit_rule 6 C:\b.exe 0 7", "del_cfg 4"]
        );
        pb.remove_rule("b").unwrap();
        assert_eq!(take(), ["del_rule 6", "del_cfg 7"]);

        // clear：删掉全部规则与代理配置，簿记清空
        pb.set_proxy("p", &t1).unwrap();
        pb.set_rule("q", r"C:\q.exe", PbAction::Block).unwrap();
        take();
        pb.clear().unwrap();
        let mut log = take();
        log.sort();
        assert_eq!(log, ["del_cfg 8", "del_rule 9"]);
        assert!(pb.rules.is_empty());
        pb.clear().unwrap();
        assert!(take().is_empty());
    }

    #[test]
    fn relay_ports_differ() {
        let (tcp, udp) = pick_relay_ports().unwrap();
        assert!(tcp != 0 && udp != 0 && tcp != udp);
    }

    #[test]
    fn load_rejects_relative_dir() {
        let err = load(Path::new("binaries")).err().unwrap();
        assert!(err.contains("绝对路径"), "{err}");
    }

    fn built_dll_dir() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("binaries")
            .join("procsvc")
    }

    const CURL: &str = r"C:\Windows\System32\curl.exe";

    fn curl_ok() -> bool {
        std::process::Command::new(CURL)
            .args(["-s", "-m", "8", "-o", "NUL", "https://example.com"])
            .status()
            .expect("run curl")
            .success()
    }

    #[test]
    #[ignore = "needs admin + built DLL"]
    fn start_add_rule_stop() {
        // 基线：没有规则时必须能访问，否则下面的"被阻断"断言没有意义。
        assert!(curl_ok(), "需要网络：基线 curl https://example.com 失败");

        let mut pb = load(&built_dll_dir()).expect("load");
        assert!(
            load(&built_dll_dir()).is_err(),
            "second instance must be refused"
        );
        pb.start().expect("start");
        pb.set_rule("t", CURL, PbAction::Block)
            .expect("add block rule");
        assert!(!curl_ok(), "Block 规则生效时 curl 应失败");
        let _ = pb.flows();
        pb.remove_rule("t").expect("remove rule");
        assert!(curl_ok(), "删除 Block 规则后 curl 应恢复");
        pb.remove_rule("t").expect("remove unknown rule is Ok");
        drop(pb);
    }

    /// final review I2：规则集为空时 ProxyBridge 停止；同一个已加载的实例能反复启停。
    #[test]
    #[ignore = "needs admin + built DLL"]
    fn lazy_restart_on_same_instance() {
        use crate::lazy::Lazy;
        assert!(curl_ok(), "需要网络：基线 curl https://example.com 失败");

        let mut l = Lazy::new(load(&built_dll_dir()).expect("load"));
        assert!(!l.running(), "加载后不应启动");
        for round in 0..2 {
            l.set_rule("t", CURL, PbAction::Block)
                .expect("add block rule");
            assert!(l.running(), "round {round}: 有规则时应已启动");
            assert!(!curl_ok(), "round {round}: Block 规则生效时 curl 应失败");
            l.remove_rule("t").expect("remove rule");
            l.suspend().expect("suspend");
            assert!(!l.running(), "round {round}: 规则集为空时应已停止");
            assert!(curl_ok(), "round {round}: 停止后 curl 应恢复");
        }
        // 已停止时再 suspend 是空操作。
        l.suspend().expect("suspend again");
        drop(l);
    }
}

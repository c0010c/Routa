//! `Guard` 的真实实现：Windows 筛选平台（WFP）用户态 API。
//!
//! 本服务注册自己的 provider 与 sublayer（都持久），对每个受防护的 exe 在
//! `ALE_AUTH_CONNECT_V4/V6`、`ALE_AUTH_RECV_ACCEPT_V4/V6` 四层各加：
//! - 阻断（权重 1，持久）；
//! - 回环放行（权重 15，持久，条件含 `FWP_CONDITION_FLAG_IS_LOOPBACK`）；
//! - 存活放行（权重 10，加在 `FWPM_SESSION_FLAG_DYNAMIC` 会话里，服务退出即被系统删除）。
//!
//! 过滤器 id 不落盘：每次操作都按 provider 枚举，按名字识别种类、按描述（安装时的 exe 写法）
//! 经 [`exe_key`] 识别 exe，从而幂等、且不区分大小写。

use std::ffi::c_void;
use std::mem::zeroed;
use std::path::Path;
use std::ptr::{null, null_mut};

use windows_sys::core::GUID;
use windows_sys::Win32::Foundation::{
    FWP_E_ALREADY_EXISTS, FWP_E_FILTER_NOT_FOUND, FWP_E_PROVIDER_NOT_FOUND,
    FWP_E_SUBLAYER_NOT_FOUND, HANDLE,
};
use windows_sys::Win32::NetworkManagement::WindowsFilteringPlatform::*;
use windows_sys::Win32::System::Rpc::RPC_C_AUTHN_WINNT;

use crate::engine::Guard;
use crate::reconcile::exe_key;

const PROVIDER_KEY: GUID = GUID::from_u128(0x5f3b2c1e_7a44_4d2b_9c51_0b6f2a7e1d01);
const SUBLAYER_KEY: GUID = GUID::from_u128(0x5f3b2c1e_7a44_4d2b_9c51_0b6f2a7e1d02);
const PROVIDER_NAME: &str = "Routa";
const SUBLAYER_NAME: &str = "Routa 进程分流";
const SUBLAYER_WEIGHT: u16 = 0x8000;

const LAYERS: [GUID; 4] = [
    FWPM_LAYER_ALE_AUTH_CONNECT_V4,
    FWPM_LAYER_ALE_AUTH_CONNECT_V6,
    FWPM_LAYER_ALE_AUTH_RECV_ACCEPT_V4,
    FWPM_LAYER_ALE_AUTH_RECV_ACCEPT_V6,
];

/// 本服务的三种过滤器。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Block,
    Loopback,
    Permit,
}

impl Kind {
    const ALL: [Kind; 3] = [Kind::Block, Kind::Loopback, Kind::Permit];

    fn label(self) -> &'static str {
        match self {
            Kind::Block => "阻断",
            Kind::Loopback => "回环",
            Kind::Permit => "放行",
        }
    }

    fn weight(self) -> u8 {
        match self {
            Kind::Block => 1,
            Kind::Loopback => 15,
            Kind::Permit => 10,
        }
    }

    /// 持久过滤器加在持久会话里；否则加在动态会话里。
    fn persistent(self) -> bool {
        self != Kind::Permit
    }
}

fn filter_name(kind: Kind, exe: &str) -> String {
    format!("Routa – {} {exe}", kind.label())
}

fn kind_of(name: &str) -> Option<Kind> {
    let rest = name.strip_prefix("Routa – ")?;
    Kind::ALL.into_iter().find(|k| {
        rest.strip_prefix(k.label())
            .is_some_and(|r| r.starts_with(' '))
    })
}

/// 已安装的一条本 provider 过滤器。
struct Installed {
    id: u64,
    layer: usize,
    /// 名字不认识（别的版本留下的）时为 None，只会被 `cleanup_all` 删除。
    kind: Option<Kind>,
    /// 安装时的 exe 写法（存在过滤器描述里）。
    exe: String,
}

fn check(what: &str, rc: u32) -> Result<(), String> {
    if rc == 0 {
        Ok(())
    } else {
        Err(format!("{what} 失败：0x{rc:08X}"))
    }
}

fn is(rc: u32, err: windows_sys::core::HRESULT) -> bool {
    rc == err as u32
}

fn same_guid(a: &GUID, b: &GUID) -> bool {
    (a.data1, a.data2, a.data3, a.data4) == (b.data1, b.data2, b.data3, b.data4)
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

/// # Safety
/// `p` 为空或指向以 0 结尾的 UTF-16 串。
unsafe fn from_wide(p: *const u16) -> String {
    if p.is_null() {
        return String::new();
    }
    let mut n = 0;
    while *p.add(n) != 0 {
        n += 1;
    }
    String::from_utf16_lossy(std::slice::from_raw_parts(p, n))
}

/// 引擎句柄，析构时关闭（动态会话随之结束）。
struct Engine(HANDLE);

impl Engine {
    fn open(dynamic: bool) -> Result<Engine, String> {
        // SAFETY: session 在调用期间有效；成功时 h 为有效句柄，交给 Engine 管理。
        unsafe {
            let mut session: FWPM_SESSION0 = zeroed();
            if dynamic {
                session.flags = FWPM_SESSION_FLAG_DYNAMIC;
            }
            let mut h: HANDLE = null_mut();
            check(
                "FwpmEngineOpen0",
                FwpmEngineOpen0(null(), RPC_C_AUTHN_WINNT, null(), &session, &mut h),
            )?;
            Ok(Engine(h))
        }
    }

    /// 在一个 WFP 事务里执行 `f`：要么全部生效，要么全部回滚。
    fn txn(&self, f: impl FnOnce() -> Result<(), String>) -> Result<(), String> {
        // SAFETY: self.0 是有效引擎句柄。
        unsafe {
            check("FwpmTransactionBegin0", FwpmTransactionBegin0(self.0, 0))?;
            match f() {
                Ok(()) => check("FwpmTransactionCommit0", FwpmTransactionCommit0(self.0)),
                Err(e) => {
                    FwpmTransactionAbort0(self.0);
                    Err(e)
                }
            }
        }
    }

    fn delete(&self, id: u64) -> Result<(), String> {
        // SAFETY: self.0 是有效引擎句柄。
        let rc = unsafe { FwpmFilterDeleteById0(self.0, id) };
        if is(rc, FWP_E_FILTER_NOT_FOUND) {
            return Ok(());
        }
        check("FwpmFilterDeleteById0", rc)
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        // SAFETY: 句柄由 FwpmEngineOpen0 打开，只关闭这一次。
        unsafe {
            FwpmEngineClose0(self.0);
        }
    }
}

/// `FwpmGetAppIdFromFileName0` 返回的 app id，析构时释放。
struct AppId(*mut FWP_BYTE_BLOB);

impl AppId {
    fn of(exe: &str) -> Result<AppId, String> {
        if !Path::new(exe).is_file() {
            return Err(format!("找不到程序文件：{exe}"));
        }
        let w = wide(exe);
        let mut blob = null_mut();
        // SAFETY: w 以 0 结尾；成功时 blob 由 WFP 分配，交给 AppId 释放。
        check("FwpmGetAppIdFromFileName0", unsafe {
            FwpmGetAppIdFromFileName0(w.as_ptr(), &mut blob)
        })?;
        Ok(AppId(blob))
    }
}

impl Drop for AppId {
    fn drop(&mut self) {
        // SAFETY: self.0 由 WFP 分配。
        unsafe { FwpmFreeMemory0(&mut self.0 as *mut _ as *mut *mut c_void) }
    }
}

/// WFP 防护。持有一个持久会话（provider / sublayer / 持久过滤器）和一个动态会话（放行）。
pub struct Wfp {
    persistent: Engine,
    dynamic: Engine,
    /// provider 与 sublayer 是否已确认存在；`cleanup_all` 后置 false，下次添加前重建。
    registered: bool,
}

/// 打开两个引擎会话并注册 provider 与 sublayer（已存在则复用）。需要管理员。
pub fn open() -> Result<Wfp, String> {
    let mut w = Wfp {
        persistent: Engine::open(false)?,
        dynamic: Engine::open(true)?,
        registered: false,
    };
    w.register()?;
    Ok(w)
}

impl Wfp {
    fn register(&mut self) -> Result<(), String> {
        if self.registered {
            return Ok(());
        }
        let mut pname = wide(PROVIDER_NAME);
        let mut sname = wide(SUBLAYER_NAME);
        let mut pkey = PROVIDER_KEY;
        let h = self.persistent.0;
        // SAFETY: 结构体里的指针都指向本函数内存活的缓冲区。
        unsafe {
            let mut provider: FWPM_PROVIDER0 = zeroed();
            provider.providerKey = PROVIDER_KEY;
            provider.displayData.name = pname.as_mut_ptr();
            provider.flags = FWPM_PROVIDER_FLAG_PERSISTENT;
            let rc = FwpmProviderAdd0(h, &provider, null_mut());
            if !is(rc, FWP_E_ALREADY_EXISTS) {
                check("FwpmProviderAdd0", rc)?;
            }

            let mut sublayer: FWPM_SUBLAYER0 = zeroed();
            sublayer.subLayerKey = SUBLAYER_KEY;
            sublayer.displayData.name = sname.as_mut_ptr();
            sublayer.flags = FWPM_SUBLAYER_FLAG_PERSISTENT;
            sublayer.providerKey = &mut pkey;
            sublayer.weight = SUBLAYER_WEIGHT;
            let rc = FwpmSubLayerAdd0(h, &sublayer, null_mut());
            if !is(rc, FWP_E_ALREADY_EXISTS) {
                check("FwpmSubLayerAdd0", rc)?;
            }
        }
        self.registered = true;
        Ok(())
    }

    /// 放行过滤器属于动态会话，其余属于持久会话。
    fn engine(&self, kind: Option<Kind>) -> &Engine {
        if kind == Some(Kind::Permit) {
            &self.dynamic
        } else {
            &self.persistent
        }
    }

    /// 枚举过滤器，对每条调用 `each`。`tpl` 为空时枚举系统里的全部过滤器。
    /// provider 不存在时视为没有过滤器。
    fn enum_filters(
        &self,
        tpl: Option<&FWPM_FILTER_ENUM_TEMPLATE0>,
        mut each: impl FnMut(&FWPM_FILTER0),
    ) -> Result<(), String> {
        const PAGE: u32 = 128;
        let h = self.persistent.0;
        let tpl = tpl.map_or(null(), |t| t as *const _);
        // SAFETY: 模板只在调用期间使用；枚举句柄在本函数内销毁；
        // 返回的数组在交给 `each` 之后用 FwpmFreeMemory0 释放。
        unsafe {
            let mut eh: HANDLE = null_mut();
            let rc = FwpmFilterCreateEnumHandle0(h, tpl, &mut eh);
            if is(rc, FWP_E_PROVIDER_NOT_FOUND) {
                return Ok(());
            }
            check("FwpmFilterCreateEnumHandle0", rc)?;
            let r = loop {
                let mut entries: *mut *mut FWPM_FILTER0 = null_mut();
                let mut n = 0u32;
                let rc = FwpmFilterEnum0(h, eh, PAGE, &mut entries, &mut n);
                if let Err(e) = check("FwpmFilterEnum0", rc) {
                    break Err(e);
                }
                for i in 0..n as usize {
                    each(&**entries.add(i));
                }
                if !entries.is_null() {
                    FwpmFreeMemory0(&mut entries as *mut _ as *mut *mut c_void);
                }
                if n < PAGE {
                    break Ok(());
                }
            };
            FwpmFilterDestroyEnumHandle0(h, eh);
            r
        }
    }

    /// 本 provider 在四个层上的全部过滤器（含动态会话里的）。
    fn list(&self) -> Result<Vec<Installed>, String> {
        let mut out = Vec::new();
        for (layer, key) in LAYERS.iter().enumerate() {
            let mut pkey = PROVIDER_KEY;
            // SAFETY: 全零是 FWPM_FILTER_ENUM_TEMPLATE0 的合法初值。
            let mut tpl: FWPM_FILTER_ENUM_TEMPLATE0 = unsafe { zeroed() };
            tpl.providerKey = &mut pkey;
            tpl.layerKey = *key;
            tpl.enumType = FWP_FILTER_ENUM_OVERLAPPING;
            tpl.actionMask = 0xFFFF_FFFF;
            self.enum_filters(Some(&tpl), |f| {
                // SAFETY: WFP 返回的名字/描述为空或以 0 结尾。
                let (name, desc) = unsafe {
                    (
                        from_wide(f.displayData.name),
                        from_wide(f.displayData.description),
                    )
                };
                out.push(Installed {
                    id: f.filterId,
                    layer,
                    kind: kind_of(&name),
                    exe: desc,
                });
            })?;
        }
        Ok(out)
    }

    /// 本 provider 在任意层上的全部过滤器（id 与种类），供 `cleanup_all` 清掉旧版本可能留在
    /// 其它层的过滤器，否则删 sublayer / provider 会因仍被引用而失败。
    fn list_any_layer(&self) -> Result<Vec<(u64, Option<Kind>)>, String> {
        let mut out = Vec::new();
        self.enum_filters(None, |f| {
            // SAFETY: providerKey 为空或指向有效 GUID；名字为空或以 0 结尾。
            unsafe {
                if !f.providerKey.is_null() && same_guid(&*f.providerKey, &PROVIDER_KEY) {
                    out.push((f.filterId, kind_of(&from_wide(f.displayData.name))));
                }
            }
        })?;
        Ok(out)
    }

    fn of_exe(&self, exe: &str, kinds: &[Kind]) -> Result<Vec<Installed>, String> {
        let key = exe_key(exe);
        Ok(self
            .list()?
            .into_iter()
            .filter(|f| f.kind.is_some_and(|k| kinds.contains(&k)) && exe_key(&f.exe) == key)
            .collect())
    }

    /// 确保 `exe` 在四个层上各有一条 `kinds` 中每种过滤器；不是恰好这一整套时，
    /// 在一个事务里删掉已有的再整套重装（不会出现半套）。`kinds` 必须同属一个会话。
    fn ensure(&mut self, exe: &str, kinds: &[Kind]) -> Result<(), String> {
        self.register()?;
        let have = self.of_exe(exe, kinds)?;
        let complete = have.len() == kinds.len() * LAYERS.len()
            && kinds.iter().all(|k| {
                (0..LAYERS.len()).all(|l| have.iter().any(|f| f.kind == Some(*k) && f.layer == l))
            });
        if complete {
            return Ok(());
        }
        let app = AppId::of(exe)?;
        let eng = self.engine(Some(kinds[0]));
        eng.txn(|| {
            for f in &have {
                eng.delete(f.id)?;
            }
            for &kind in kinds {
                for layer in LAYERS {
                    add_filter(eng, kind, layer, exe, &app)?;
                }
            }
            Ok(())
        })
    }

    /// 删除 `exe` 的 `kinds` 过滤器（同属一个会话）；没有则什么也不做。
    fn remove(&mut self, exe: &str, kinds: &[Kind]) -> Result<(), String> {
        let have = self.of_exe(exe, kinds)?;
        if have.is_empty() {
            return Ok(());
        }
        let eng = self.engine(Some(kinds[0]));
        eng.txn(|| have.iter().try_for_each(|f| eng.delete(f.id)))
    }
}

fn add_filter(eng: &Engine, kind: Kind, layer: GUID, exe: &str, app: &AppId) -> Result<(), String> {
    let mut name = wide(&filter_name(kind, exe));
    let mut desc = wide(exe);
    let mut pkey = PROVIDER_KEY;
    // SAFETY: 结构体里的指针都指向本函数内存活的缓冲区或 app 持有的 blob。
    unsafe {
        let mut conds: [FWPM_FILTER_CONDITION0; 2] = zeroed();
        conds[0].fieldKey = FWPM_CONDITION_ALE_APP_ID;
        conds[0].matchType = FWP_MATCH_EQUAL;
        conds[0].conditionValue.r#type = FWP_BYTE_BLOB_TYPE;
        conds[0].conditionValue.Anonymous.byteBlob = app.0;
        conds[1].fieldKey = FWPM_CONDITION_FLAGS;
        conds[1].matchType = FWP_MATCH_FLAGS_ALL_SET;
        conds[1].conditionValue.r#type = FWP_UINT32;
        conds[1].conditionValue.Anonymous.uint32 = FWP_CONDITION_FLAG_IS_LOOPBACK;

        let mut f: FWPM_FILTER0 = zeroed();
        f.displayData.name = name.as_mut_ptr();
        f.displayData.description = desc.as_mut_ptr();
        f.flags = if kind.persistent() {
            FWPM_FILTER_FLAG_PERSISTENT
        } else {
            FWPM_FILTER_FLAG_NONE
        };
        f.providerKey = &mut pkey;
        f.layerKey = layer;
        f.subLayerKey = SUBLAYER_KEY;
        f.weight.r#type = FWP_UINT8;
        f.weight.Anonymous.uint8 = kind.weight();
        f.numFilterConditions = if kind == Kind::Loopback { 2 } else { 1 };
        f.filterCondition = conds.as_mut_ptr();
        f.action.r#type = if kind == Kind::Block {
            FWP_ACTION_BLOCK
        } else {
            FWP_ACTION_PERMIT
        };
        let mut id = 0u64;
        check(
            "FwpmFilterAdd0",
            FwpmFilterAdd0(eng.0, &f, null_mut(), &mut id),
        )
    }
}

impl Guard for Wfp {
    fn ensure_block(&mut self, exe: &str) -> Result<(), String> {
        self.ensure(exe, &[Kind::Block, Kind::Loopback])
    }

    fn remove_block(&mut self, exe: &str) -> Result<(), String> {
        self.remove(exe, &[Kind::Block, Kind::Loopback])
    }

    fn permit(&mut self, exe: &str) -> Result<(), String> {
        self.ensure(exe, &[Kind::Permit])
    }

    fn revoke(&mut self, exe: &str) -> Result<(), String> {
        self.remove(exe, &[Kind::Permit])
    }

    fn cleanup_all(&mut self) -> Result<(), String> {
        for (id, kind) in self.list_any_layer()? {
            self.engine(kind).delete(id)?;
        }
        let h = self.persistent.0;
        // SAFETY: h 是有效引擎句柄；key 在调用期间有效。
        unsafe {
            let rc = FwpmSubLayerDeleteByKey0(h, &SUBLAYER_KEY);
            if !is(rc, FWP_E_SUBLAYER_NOT_FOUND) {
                check("FwpmSubLayerDeleteByKey0", rc)?;
            }
            let rc = FwpmProviderDeleteByKey0(h, &PROVIDER_KEY);
            if !is(rc, FWP_E_PROVIDER_NOT_FOUND) {
                check("FwpmProviderDeleteByKey0", rc)?;
            }
        }
        self.registered = false;
        Ok(())
    }

    fn blocked_exes(&mut self) -> Result<Vec<String>, String> {
        let mut out: Vec<String> = Vec::new();
        for f in self.list()? {
            if f.kind == Some(Kind::Block) && !out.iter().any(|e| exe_key(e) == exe_key(&f.exe)) {
                out.push(f.exe);
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::Guard;
    use std::io::{Read, Write};
    use std::net::{TcpListener, UdpSocket};
    use std::process::Command;
    use std::sync::{Mutex, MutexGuard};
    use std::time::Duration;

    const CURL: &str = r"C:\Windows\System32\curl.exe";

    #[test]
    fn filter_names_round_trip() {
        for kind in [Kind::Block, Kind::Loopback, Kind::Permit] {
            let name = filter_name(kind, r"C:\测试\a b.exe");
            assert!(name.starts_with("Routa – "), "{name}");
            assert_eq!(kind_of(&name), Some(kind));
        }
        assert_eq!(
            filter_name(Kind::Block, r"C:\x.exe"),
            r"Routa – 阻断 C:\x.exe"
        );
        assert_eq!(
            filter_name(Kind::Loopback, r"C:\x.exe"),
            r"Routa – 回环 C:\x.exe"
        );
        assert_eq!(
            filter_name(Kind::Permit, r"C:\x.exe"),
            r"Routa – 放行 C:\x.exe"
        );
        assert_eq!(kind_of("Routa – 其它 C:\\x.exe"), None);
        assert_eq!(kind_of("别人的过滤器"), None);
    }

    #[test]
    fn kind_values_match_spec() {
        assert_eq!(
            [Kind::Block, Kind::Loopback, Kind::Permit].map(Kind::weight),
            [1, 15, 10]
        );
        assert!(Kind::Block.persistent() && Kind::Loopback.persistent());
        assert!(!Kind::Permit.persistent());
    }

    // ---- 以下需要管理员；`-- --ignored wfp::` 一次跑完（测试之间由 LOCK 串行）----

    /// 管理员测试共用全局 WFP 状态（同一 provider、同一个 curl），必须串行。
    /// 先于 `Cleanup` 取得、后于它释放；某个测试 panic 造成的中毒不影响其它测试。
    static LOCK: Mutex<()> = Mutex::new(());

    fn serial() -> MutexGuard<'static, ()> {
        LOCK.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// 无论断言是否失败，测试结束时都清掉本 provider 下的一切。
    struct Cleanup;
    impl Drop for Cleanup {
        fn drop(&mut self) {
            match open() {
                Ok(mut w) => {
                    if let Err(e) = w.cleanup_all() {
                        eprintln!("cleanup_all 失败：{e}");
                    }
                }
                Err(e) => eprintln!("清理时 open 失败：{e}"),
            }
        }
    }

    fn curl_ok(url: &str) -> bool {
        Command::new(CURL)
            .args(["-s", "-m", "8", "--noproxy", "*", "-o", "NUL", url])
            .status()
            .expect("run curl")
            .success()
    }

    fn need_network() {
        assert!(
            curl_ok("https://example.com"),
            "需要网络：基线 curl https://example.com 失败"
        );
    }

    /// 用 netsh 导出全部过滤器，确认没有本 provider 的残留。
    fn assert_no_traces() {
        let file = std::env::temp_dir().join("routa-wfp-filters.xml");
        let out = Command::new("netsh")
            .args(["wfp", "show", "filters"])
            .arg(format!("file={}", file.display()))
            .output()
            .expect("run netsh");
        assert!(out.status.success(), "netsh 失败：{out:?}");
        let xml = std::fs::read(&file).expect("read filters.xml");
        let _ = std::fs::remove_file(&file);
        let text = String::from_utf8_lossy(&xml).to_lowercase();
        assert!(
            !text.contains("5f3b2c1e-7a44-4d2b-9c51-0b6f2a7e1d0"),
            "filters.xml 中仍有 Routa 的 provider/sublayer"
        );
    }

    #[test]
    #[ignore = "needs admin"]
    fn permit_disappears_with_session() {
        let _serial = serial();
        need_network();
        let _c = Cleanup;
        let mut w = open().expect("open");
        w.ensure_block(CURL).expect("ensure_block");
        w.permit(CURL).expect("permit");
        assert!(curl_ok("https://example.com"), "阻断 + 放行时 curl 应成功");
        // 模拟服务退出：关闭全部引擎句柄，动态会话随之结束
        drop(w);
        assert!(
            !curl_ok("https://example.com"),
            "动态会话关闭后 curl 应被阻断"
        );
        let mut w = open().expect("reopen");
        assert_eq!(w.blocked_exes().unwrap(), vec![CURL.to_string()]);
        w.cleanup_all().expect("cleanup_all");
        assert!(curl_ok("https://example.com"), "cleanup_all 后 curl 应恢复");
        assert!(w.blocked_exes().unwrap().is_empty());
        assert_no_traces();
    }

    #[test]
    #[ignore = "needs admin"]
    fn loopback_allowed_while_blocked() {
        let _serial = serial();
        need_network();
        let srv = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = srv.local_addr().unwrap().port();
        std::thread::spawn(move || {
            for mut s in srv.incoming().flatten() {
                let mut buf = [0u8; 2048];
                let _ = s.read(&mut buf);
                let _ = s.write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok",
                );
            }
        });
        let local = format!("http://127.0.0.1:{port}/");
        assert!(curl_ok(&local), "基线：本地 HTTP 服务不可达");

        let _c = Cleanup;
        let mut w = open().expect("open");
        w.ensure_block(CURL).expect("ensure_block");
        assert!(!curl_ok("https://example.com"), "阻断时外网应不通");
        assert!(curl_ok(&local), "阻断时回环应放行");
    }

    #[test]
    #[ignore = "needs admin"]
    fn ops_idempotent_and_case_insensitive() {
        let _serial = serial();
        let _c = Cleanup;
        let mut w = open().expect("open");
        let upper = CURL.to_uppercase();
        let count = |w: &Wfp, k: Kind| {
            w.list()
                .unwrap()
                .iter()
                .filter(|f| f.kind == Some(k))
                .count()
        };

        w.ensure_block(CURL).unwrap();
        w.ensure_block(&upper).unwrap();
        assert_eq!(count(&w, Kind::Block), 4);
        assert_eq!(count(&w, Kind::Loopback), 4);
        assert_eq!(
            w.blocked_exes().unwrap(),
            vec![CURL.to_string()],
            "按安装时的写法"
        );

        w.permit(&upper).unwrap();
        w.permit(CURL).unwrap();
        assert_eq!(count(&w, Kind::Permit), 4);
        w.revoke(CURL).unwrap();
        w.revoke(&upper).unwrap();
        assert_eq!(count(&w, Kind::Permit), 0);

        w.remove_block(&upper).unwrap();
        w.remove_block(CURL).unwrap();
        assert!(w.list().unwrap().is_empty());
        assert!(w.blocked_exes().unwrap().is_empty());

        let missing = r"C:\不存在\nope.exe";
        assert!(w.ensure_block(missing).is_err());
        assert!(w.permit(missing).is_err());
        assert!(w.list().unwrap().is_empty(), "失败的添加不应留下半套过滤器");

        // cleanup_all 之后同一个实例仍可用（provider/sublayer 按需重建）
        w.cleanup_all().unwrap();
        w.cleanup_all().unwrap();
        w.ensure_block(CURL).unwrap();
        assert_eq!(count(&w, Kind::Block), 4);

        // 旧版本可能在别的层留下过滤器：cleanup_all 也要清掉，否则删 sublayer/provider 会失败
        let app = AppId::of(CURL).unwrap();
        add_filter(
            &w.persistent,
            Kind::Block,
            FWPM_LAYER_ALE_AUTH_LISTEN_V4,
            CURL,
            &app,
        )
        .unwrap();
        assert_eq!(w.list_any_layer().unwrap().len(), 9);
        w.cleanup_all().expect("cleanup_all 应清掉任意层上的过滤器");
        assert!(w.list_any_layer().unwrap().is_empty());
    }

    // ---- spec 第 10 节：服务崩溃时已授权的 UDP 会话是否漏包 ----

    fn dns_query(id: u16) -> Vec<u8> {
        let mut q = id.to_be_bytes().to_vec();
        q.extend_from_slice(&[1, 0, 0, 1, 0, 0, 0, 0, 0, 0]);
        for label in ["example", "com"] {
            q.push(label.len() as u8);
            q.extend_from_slice(label.as_bytes());
        }
        q.extend_from_slice(&[0, 0, 1, 0, 1]);
        q
    }

    /// 在已 connect 的 UDP 套接字上发一个 DNS 查询，等对应 id 的回包。
    fn udp_echo(sock: &UdpSocket, id: u16) -> bool {
        sock.set_read_timeout(Some(Duration::from_millis(1500)))
            .unwrap();
        if sock.send(&dns_query(id)).is_err() {
            return false;
        }
        let mut buf = [0u8; 1500];
        while let Ok(n) = sock.recv(&mut buf) {
            if n >= 2 && buf[..2] == id.to_be_bytes() {
                return true;
            }
        }
        false
    }

    /// 单个 UDP 包在公网上可能丢（或被解析器限速），所以"应通"的检查最多试 `tries` 次，
    /// 返回第几次成功。每次用不同的查询 id，迟到的旧回包不会被误认。
    fn udp_echo_retry(sock: &UdpSocket, base_id: u16, tries: u16) -> Option<u16> {
        (0..tries)
            .find(|i| udp_echo(sock, base_id + i))
            .map(|i| i + 1)
    }

    fn udp_sock(server: &str) -> UdpSocket {
        let s = UdpSocket::bind("0.0.0.0:0").unwrap();
        s.connect(server).unwrap();
        s
    }

    /// 被防护的 exe 是测试程序自己：本进程的 UDP 收发即"该 exe 的 UDP 会话"。
    /// 关闭动态会话后观察已授权套接字是否还能收到回包（spec 第 10 节：应为 0）。
    #[test]
    #[ignore = "needs admin"]
    fn udp_leak_after_session_close() {
        let _serial = serial();
        let servers = [
            "223.5.5.5:53",
            "119.29.29.29:53",
            "8.8.8.8:53",
            "1.1.1.1:53",
        ];
        let server = *servers
            .iter()
            .find(|s| udp_echo_retry(&udp_sock(s), 1, 3).is_some())
            .expect("需要网络：没有可达的 DNS 服务器做 UDP 回显");
        let me = std::env::current_exe().unwrap();
        let me = me.to_str().unwrap();

        let _c = Cleanup;
        let mut w = open().expect("open");
        w.ensure_block(me).unwrap();
        assert!(!udp_echo(&udp_sock(server), 2), "阻断时新 UDP 套接字应不通");
        w.permit(me).unwrap();
        let authorized = udp_sock(server);
        let tries = udp_echo_retry(&authorized, 10, 5);
        if tries.is_none() {
            let fresh = udp_echo_retry(&udp_sock(server), 20, 3).is_some();
            let n = w.list().map(|l| l.len());
            panic!(
                "放行时 UDP 应通（5 次均无回包；新套接字{}；本 provider 过滤器数 {n:?}）",
                if fresh { "能通" } else { "也不通" }
            );
        }
        println!("放行后第 {} 次查询收到回包", tries.unwrap());

        drop(w); // 动态会话结束 = 服务崩溃
        let mut leaked = 0;
        for i in 0..6u16 {
            if udp_echo(&authorized, 100 + i) {
                leaked += 1;
            }
            std::thread::sleep(Duration::from_millis(300));
        }
        let fresh = udp_echo(&udp_sock(server), 200);
        println!(
            "UDP 漏包测量（{server}）：会话关闭后已授权套接字收到 {leaked}/6 个回包；新套接字{}",
            if fresh { "仍通" } else { "被阻断" }
        );
        assert!(!fresh, "会话关闭后新 UDP 套接字应被阻断");
        assert_eq!(
            leaked, 0,
            "spec 第 10 节结论：会话关闭后已授权的 UDP 会话不应漏包"
        );
    }
}

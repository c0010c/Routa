//! 引擎抽象：ProxyBridge 拦截器、WFP 防护、断开旧 TCP 连接。
//! 真实实现由后续任务提供；`reconcile::plan` 产出的操作最终落到这些 trait 上。

use routa_procsvc_proto::{Flow, ProxyTarget};

/// ProxyBridge 规则动作。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PbAction {
    Proxy,
    Direct,
    Block,
}

/// ProxyBridge（WinDivert）拦截器。
///
/// 规则按 id 唯一：`set_rule` 对已存在的 id 是覆盖（含换 exe）；`remove_rule` 对不存在的 id 返回 Ok（幂等）。
/// `set_rule(Proxy)` 之前必须先对同一 id `set_proxy`。
/// 实现必须按 [`crate::reconcile::exe_key`] 不区分大小写地对待 exe 路径。
pub trait Interceptor {
    fn set_proxy(&mut self, rule: &str, target: &ProxyTarget) -> Result<(), String>;
    fn set_rule(&mut self, rule: &str, exe: &str, action: PbAction) -> Result<(), String>;
    fn remove_rule(&mut self, rule: &str) -> Result<(), String>;
    fn flows(&mut self) -> Vec<Flow>;
}

/// WFP 防护：低权重永久阻断 + 动态会话里的高权重放行。
///
/// 实现必须按 [`crate::reconcile::exe_key`] 不区分大小写地对待 exe 路径（同一 exe 只有一份状态），
/// 且各操作幂等：重复 `ensure_block` / `permit`、对不存在的过滤器 `remove_block` / `revoke` 都返回 Ok。
pub trait Guard {
    /// 永久阻断过滤器（服务退出后仍在）。
    fn ensure_block(&mut self, exe: &str) -> Result<(), String>;
    fn remove_block(&mut self, exe: &str) -> Result<(), String>;
    /// 动态会话放行过滤器（服务退出即被系统删除）。
    fn permit(&mut self, exe: &str) -> Result<(), String>;
    fn revoke(&mut self, exe: &str) -> Result<(), String>;
    fn cleanup_all(&mut self) -> Result<(), String>;
    /// 当前持有永久阻断的 exe（按安装时的写法），供服务启动时核对残留。
    fn blocked_exes(&mut self) -> Result<Vec<String>, String>;
}

/// 断开某 exe 的 IPv4 TCP 连接，返回断开的条数。
/// 实现必须按 [`crate::reconcile::exe_key`] 不区分大小写地匹配 exe 路径。
pub trait ConnKiller {
    fn reset_ipv4_tcp(&mut self, exe: &str) -> Result<u32, String>;
}

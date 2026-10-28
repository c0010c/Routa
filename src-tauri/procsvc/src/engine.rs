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
pub trait Interceptor {
    fn set_proxy(&mut self, rule: &str, target: &ProxyTarget) -> Result<(), String>;
    fn set_rule(&mut self, rule: &str, exe: &str, action: PbAction) -> Result<(), String>;
    fn remove_rule(&mut self, rule: &str) -> Result<(), String>;
    fn flows(&mut self) -> Vec<Flow>;
}

/// WFP 防护：低权重永久阻断 + 动态会话里的高权重放行。
pub trait Guard {
    /// 永久阻断过滤器（服务退出后仍在）。
    fn ensure_block(&mut self, exe: &str) -> Result<(), String>;
    fn remove_block(&mut self, exe: &str) -> Result<(), String>;
    /// 动态会话放行过滤器（服务退出即被系统删除）。
    fn permit(&mut self, exe: &str) -> Result<(), String>;
    fn revoke(&mut self, exe: &str) -> Result<(), String>;
    fn cleanup_all(&mut self) -> Result<(), String>;
}

/// 断开某 exe 的 IPv4 TCP 连接，返回断开的条数。
pub trait ConnKiller {
    fn reset_ipv4_tcp(&mut self, exe: &str) -> Result<u32, String>;
}

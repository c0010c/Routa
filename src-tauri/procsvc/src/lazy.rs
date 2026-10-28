//! 按需启动的拦截器：规则集为空时 ProxyBridge 不运行。
//!
//! ProxyBridge 一旦启动，WinDivert 就把全机的 TCP/UDP 都转到用户态判一遍——没有规则时这纯属开销与风险。
//! 所以服务启动时只加载 DLL，第一次 `set_rule` 前才启动；`Reconciler` 发现已生效规则集为空时调用
//! `suspend`，删掉残留的规则与代理配置后停止。同一个已加载的实例可以反复启动、停止。
//! WFP 阻断不依赖 ProxyBridge，停止期间照常生效。

use routa_procsvc_proto::{Flow, ProxyTarget};

use crate::engine::{Interceptor, PbAction};

/// 可启停的拦截器（真实实现是 [`crate::pb_ffi::ProxyBridge`]）。
/// 规则与代理配置的增删在未运行时也可以调用。
pub trait Bridge: Interceptor {
    /// 启动拦截（未运行时才会被调用）。
    fn start(&mut self) -> Result<(), String>;
    /// 停止拦截（运行中才会被调用）。
    fn stop(&mut self);
    /// 删除所有已下发的规则与代理配置，并清空相应簿记。
    fn clear(&mut self) -> Result<(), String>;
}

pub struct Lazy<B: Bridge> {
    inner: B,
    running: bool,
}

impl<B: Bridge> Lazy<B> {
    pub fn new(inner: B) -> Self {
        Lazy {
            inner,
            running: false,
        }
    }

    #[cfg(test)]
    pub fn running(&self) -> bool {
        self.running
    }

    fn ensure_running(&mut self) -> Result<(), String> {
        if !self.running {
            self.inner.start()?;
            self.running = true;
        }
        Ok(())
    }
}

impl<B: Bridge> Interceptor for Lazy<B> {
    fn set_proxy(&mut self, rule: &str, target: &ProxyTarget) -> Result<(), String> {
        // 代理配置本身不拦截任何流量，未运行时也能设置；
        // 先于启动设置还能让 ProxyBridge 在启动时看到 SOCKS5 配置、开启 UDP 中转。
        self.inner.set_proxy(rule, target)
    }

    fn set_rule(&mut self, rule: &str, exe: &str, action: PbAction) -> Result<(), String> {
        self.ensure_running()?;
        self.inner.set_rule(rule, exe, action)
    }

    fn remove_rule(&mut self, rule: &str) -> Result<(), String> {
        self.inner.remove_rule(rule)
    }

    fn flows(&mut self) -> Vec<Flow> {
        if self.running {
            self.inner.flows()
        } else {
            Vec::new()
        }
    }

    fn suspend(&mut self) -> Result<(), String> {
        // 先清干净再停：删不掉的规则若留在 DLL 里，下次启动会带着它生效。清不掉就继续运行。
        self.inner.clear()?;
        if self.running {
            self.inner.stop();
            self.running = false;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use routa_procsvc_proto::ProxyKind;
    use std::collections::HashSet;

    #[derive(Default)]
    struct Fake {
        log: Vec<String>,
        rules: HashSet<String>,
        fail_start: bool,
        fail_clear: bool,
    }

    impl Interceptor for Fake {
        fn set_proxy(&mut self, rule: &str, _: &ProxyTarget) -> Result<(), String> {
            self.log.push(format!("set_proxy {rule}"));
            Ok(())
        }
        fn set_rule(&mut self, rule: &str, _: &str, _: PbAction) -> Result<(), String> {
            self.log.push(format!("set_rule {rule}"));
            self.rules.insert(rule.into());
            Ok(())
        }
        fn remove_rule(&mut self, rule: &str) -> Result<(), String> {
            self.log.push(format!("remove_rule {rule}"));
            self.rules.remove(rule);
            Ok(())
        }
        fn flows(&mut self) -> Vec<Flow> {
            self.log.push("flows".into());
            Vec::new()
        }
    }

    impl Bridge for Fake {
        fn start(&mut self) -> Result<(), String> {
            self.log.push("start".into());
            if self.fail_start {
                Err("启动注入失败".into())
            } else {
                Ok(())
            }
        }
        fn stop(&mut self) {
            self.log.push("stop".into());
        }
        fn clear(&mut self) -> Result<(), String> {
            self.log.push("clear".into());
            if self.fail_clear {
                return Err("清理注入失败".into());
            }
            self.rules.clear();
            Ok(())
        }
    }

    fn target() -> ProxyTarget {
        ProxyTarget {
            kind: ProxyKind::Socks5,
            host: "127.0.0.1".into(),
            port: 7890,
            username: None,
            password: None,
        }
    }

    fn take(l: &mut Lazy<Fake>) -> Vec<String> {
        std::mem::take(&mut l.inner.log)
    }

    #[test]
    fn starts_on_first_rule_and_stops_on_suspend() {
        let mut l = Lazy::new(Fake::default());
        // 没有规则：不启动，flows 不碰 DLL，suspend 只清理
        assert!(l.flows().is_empty());
        l.suspend().unwrap();
        assert!(!l.running());
        assert_eq!(take(&mut l), ["clear"]);

        // 代理配置不启动；第一条规则前启动，之后不再重复启动
        l.set_proxy("a", &target()).unwrap();
        assert!(!l.running());
        l.set_rule("a", r"C:\a.exe", PbAction::Proxy).unwrap();
        l.set_rule("b", r"C:\b.exe", PbAction::Block).unwrap();
        l.flows();
        assert!(l.running());
        assert_eq!(
            take(&mut l),
            ["set_proxy a", "start", "set_rule a", "set_rule b", "flows"]
        );

        l.remove_rule("a").unwrap();
        l.remove_rule("b").unwrap();
        l.suspend().unwrap();
        assert!(!l.running());
        assert_eq!(
            take(&mut l),
            ["remove_rule a", "remove_rule b", "clear", "stop"]
        );

        // 停止后可以再次启动同一个实例
        l.set_rule("c", r"C:\c.exe", PbAction::Direct).unwrap();
        assert!(l.running());
        assert_eq!(take(&mut l), ["start", "set_rule c"]);
    }

    #[test]
    fn failed_start_does_not_set_rule_and_retries() {
        let mut l = Lazy::new(Fake {
            fail_start: true,
            ..Fake::default()
        });
        assert!(l.set_rule("a", r"C:\a.exe", PbAction::Block).is_err());
        assert!(!l.running());
        assert_eq!(take(&mut l), ["start"]);

        l.inner.fail_start = false;
        l.set_rule("a", r"C:\a.exe", PbAction::Block).unwrap();
        assert!(l.running());
        assert_eq!(take(&mut l), ["start", "set_rule a"]);
    }

    #[test]
    fn failed_clear_keeps_running() {
        let mut l = Lazy::new(Fake::default());
        l.set_rule("a", r"C:\a.exe", PbAction::Block).unwrap();
        l.inner.fail_clear = true;
        assert!(l.suspend().is_err());
        assert!(l.running());
        take(&mut l);

        l.inner.fail_clear = false;
        l.suspend().unwrap();
        assert!(!l.running());
        assert_eq!(take(&mut l), ["clear", "stop"]);
    }
}

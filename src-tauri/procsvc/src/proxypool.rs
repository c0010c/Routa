//! ProxyBridge 代理配置池：相同的 `ProxyTarget` 共用一份配置（DLL 最多 32 份），按引用计数回收。
//!
//! 纯簿记，不碰 DLL：调用方按 [`Attach`] / [`ProxyPool::detach`] 的结果去增删真正的配置。

use std::collections::HashMap;

use routa_procsvc_proto::ProxyTarget;

/// [`ProxyPool::attach`] 的结果。
#[derive(Debug, PartialEq, Eq)]
pub enum Attach {
    /// 已有同目标的配置，直接用。
    Reuse(u32),
    /// 还没有：调用方创建后用 [`ProxyPool::created`] 登记。
    Create,
}

#[derive(Default)]
pub struct ProxyPool {
    /// 规则当前引用的目标。
    targets: HashMap<String, ProxyTarget>,
    /// 已创建的配置（`ProxyTarget` 不可哈希，数量 ≤ 32，线性查找）。
    cfgs: Vec<(ProxyTarget, u32)>,
}

impl ProxyPool {
    fn cfg_for(&self, target: &ProxyTarget) -> Option<u32> {
        self.cfgs.iter().find(|(t, _)| t == target).map(|(_, c)| *c)
    }

    /// 规则开始引用 `target`。规则原有的引用应先 [`detach`](Self::detach)。
    pub fn attach(&mut self, rule: &str, target: &ProxyTarget) -> Attach {
        self.targets.insert(rule.to_string(), target.clone());
        match self.cfg_for(target) {
            Some(cfg) => Attach::Reuse(cfg),
            None => Attach::Create,
        }
    }

    /// 登记 `attach` 返回 `Create` 后创建出的配置。
    pub fn created(&mut self, rule: &str, target: &ProxyTarget, cfg: u32) {
        self.targets.insert(rule.to_string(), target.clone());
        if self.cfg_for(target).is_none() {
            self.cfgs.push((target.clone(), cfg));
        }
    }

    /// 规则不再引用任何目标；返回引用归零、需要删除的配置 id。
    pub fn detach(&mut self, rule: &str) -> Option<u32> {
        let target = self.targets.remove(rule)?;
        if self.targets.values().any(|t| *t == target) {
            return None;
        }
        let i = self.cfgs.iter().position(|(t, _)| *t == target)?;
        Some(self.cfgs.remove(i).1)
    }

    pub fn target_of(&self, rule: &str) -> Option<&ProxyTarget> {
        self.targets.get(rule)
    }

    pub fn cfg_of(&self, rule: &str) -> Option<u32> {
        self.cfg_for(self.targets.get(rule)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use routa_procsvc_proto::ProxyKind;

    fn target(port: u16) -> ProxyTarget {
        ProxyTarget {
            kind: ProxyKind::Socks5,
            host: "127.0.0.1".into(),
            port,
            username: None,
            password: None,
        }
    }

    #[test]
    fn rules_share_one_config_per_target() {
        let (mut p, a) = (ProxyPool::default(), target(1));
        assert_eq!(p.attach("r1", &a), Attach::Create);
        p.created("r1", &a, 7);
        assert_eq!(p.attach("r2", &a), Attach::Reuse(7));
        assert_eq!((p.cfg_of("r1"), p.cfg_of("r2")), (Some(7), Some(7)));
        assert_eq!(p.detach("r1"), None);
        assert_eq!(p.cfg_of("r1"), None);
        assert_eq!(p.detach("r2"), Some(7));
        assert_eq!(p.detach("r2"), None);
        assert_eq!(p.attach("r3", &a), Attach::Create);
    }

    #[test]
    fn target_change_moves_to_new_config() {
        let (mut p, a, b) = (ProxyPool::default(), target(1), target(2));
        p.attach("r1", &a);
        p.created("r1", &a, 7);
        assert_eq!(p.detach("r1"), Some(7));
        assert_eq!(p.attach("r1", &b), Attach::Create);
        p.created("r1", &b, 8);
        assert_eq!(p.cfg_of("r1"), Some(8));
    }

    #[test]
    fn shared_config_survives_one_rule_changing_target() {
        let (mut p, a, b) = (ProxyPool::default(), target(1), target(2));
        p.attach("r1", &a);
        p.created("r1", &a, 7);
        p.attach("r2", &a);
        assert_eq!(p.detach("r1"), None);
        p.attach("r1", &b);
        assert_eq!(p.cfg_of("r2"), Some(7));
        assert_eq!(p.target_of("r1"), Some(&b));
    }
}

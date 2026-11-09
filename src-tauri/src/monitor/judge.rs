//! Per-target counters, round judging and notice copy.

use super::{MonitorFailure, MonitorFocus, MonitorNotice, MonitorTarget, TargetSpec};
use crate::model::NodeMonitorSettings;
use crate::stats::delay::{DelayError, DelayResult};

/// New downs in one round from which they merge into a single notice.
const MERGE_AT: usize = 3;
/// Names listed in a merged notice.
const MERGED_NAMES: usize = 5;

#[derive(Default)]
pub struct MonitorBook {
    targets: Vec<MonitorTarget>,
    /// Consecutive rounds in which every valid result had no latency.
    network_fails: u32,
    network_down: bool,
}

pub enum RoundOutcome {
    Applied(Vec<MonitorNotice>),
    /// The core failed mid-round, or every result went stale (the config
    /// was re-applied meanwhile); nothing changed.
    Voided,
}

/// What one result means for its target.
enum Verdict {
    Ok(u32),
    Fail(MonitorFailure),
}

impl Verdict {
    /// Timeout / connect failure, as opposed to merely over the limit.
    fn no_latency(&self) -> bool {
        matches!(self, Verdict::Fail(MonitorFailure::Timeout | MonitorFailure::Failed))
    }
}

fn reason(f: &MonitorFailure) -> String {
    match f {
        MonitorFailure::Timeout => "超时".into(),
        MonitorFailure::Failed => "连接失败".into(),
        MonitorFailure::OverLimit(ms) => format!("延迟 {ms} ms 超过上限"),
    }
}

fn focus_of(t: &MonitorTarget) -> MonitorFocus {
    t.node.clone().map_or(MonitorFocus::Listeners, MonitorFocus::Node)
}

/// "不到 1 分钟" / "约 15 分钟" / "约 2 小时 5 分钟".
pub fn format_down_for(secs: i64) -> String {
    let mins = secs.max(0) / 60;
    match mins {
        0 => "不到 1 分钟".into(),
        1..=59 => format!("约 {mins} 分钟"),
        _ if mins % 60 == 0 => format!("约 {} 小时", mins / 60),
        _ => format!("约 {} 小时 {} 分钟", mins / 60, mins % 60),
    }
}

fn down_notice(t: &MonitorTarget) -> MonitorNotice {
    let mut body = format!("连续 {} 次探测失败（{}）", t.fails, t.last_failure.as_ref().map(reason).unwrap_or_default());
    if !t.used_by.is_empty() {
        let users: Vec<String> = t.used_by.iter().map(|u| format!("「{u}」")).collect();
        body.push_str(&format!(" · 正在被{}使用", users.join("、")));
    }
    MonitorNotice { title: format!("节点不可用：{}", t.label), body, focus: focus_of(t) }
}

fn merged_down_notice(downs: &[&MonitorTarget]) -> MonitorNotice {
    let names: Vec<&str> = downs.iter().take(MERGED_NAMES).map(|t| t.label.as_str()).collect();
    let mut body = names.join("、");
    if downs.len() > MERGED_NAMES {
        body.push('等');
    }
    let focus = downs.iter().find(|t| t.node.is_some()).map_or(MonitorFocus::Listeners, |t| focus_of(t));
    MonitorNotice { title: format!("{} 个节点不可用", downs.len()), body, focus }
}

impl MonitorBook {
    /// Replaces the target set with `specs` (in their order). A target is kept
    /// (with its counters) only when both proxy and node match; anything else
    /// is a new target. Removed ones are dropped silently. Returns
    /// whether the set, its order or any metadata changed.
    pub fn reconcile(&mut self, specs: &[TargetSpec]) -> bool {
        let same = self.targets.len() == specs.len()
            && self.targets.iter().zip(specs).all(|(t, s)| {
                t.proxy == s.proxy && t.node == s.node && t.label == s.label && t.used_by == s.used_by
            });
        if same {
            return false;
        }
        let mut old = std::mem::take(&mut self.targets);
        for s in specs {
            let t = match old.iter().position(|t| t.proxy == s.proxy && t.node == s.node) {
                Some(j) => {
                    let mut t = old.swap_remove(j);
                    t.node = s.node.clone();
                    t.label = s.label.clone();
                    t.used_by = s.used_by.clone();
                    t
                }
                None => {
                    MonitorTarget {
                        proxy: s.proxy.clone(),
                        node: s.node.clone(),
                        label: s.label.clone(),
                        used_by: s.used_by.clone(),
                        fails: 0,
                        down: false,
                        down_since: None,
                        last_ms: None,
                        last_failure: None,
                        probed: false,
                        not_applied: false,
                    }
                }
            };
            self.targets.push(t);
        }
        true
    }

    /// `apply_round_with` when the OS cannot say whether it is online.
    pub fn apply_round(&mut self, results: &[(String, DelayResult)], cfg: &NodeMonitorSettings, now: i64) -> RoundOutcome {
        self.apply_round_with(results, cfg, now, None)
    }

    /// Judges one round of results keyed by proxy name (spec §4). Results
    /// for proxies not in the set are ignored. `internet` is the OS's own
    /// verdict: with it, a round is "network down" only when Windows sees no
    /// internet and nothing answered, so several nodes failing together while
    /// online count as node failures. Without it (`None`), every one of ≥2
    /// results failing to answer is taken as the network.
    pub fn apply_round_with(
        &mut self,
        results: &[(String, DelayResult)],
        cfg: &NodeMonitorSettings,
        now: i64,
        internet: Option<bool>,
    ) -> RoundOutcome {
        if results.iter().any(|(_, r)| matches!(r.error, Some(DelayError::NoCore | DelayError::CoreUnreachable(_)))) {
            return RoundOutcome::Voided;
        }
        // Every result for our targets is stale: nothing measured the current
        // config, so retry soon instead of counting the round as done.
        let mut ours = results.iter().filter(|(p, _)| self.targets.iter().any(|t| &t.proxy == p)).peekable();
        if ours.peek().is_some() && ours.all(|(_, r)| r.stale) {
            return RoundOutcome::Voided;
        }

        let mut verdicts: Vec<(usize, Verdict)> = Vec::new();
        for (i, t) in self.targets.iter_mut().enumerate() {
            let Some((_, r)) = results.iter().find(|(p, _)| *p == t.proxy) else { continue };
            if r.stale {
                continue;
            }
            if r.error == Some(DelayError::NotFound) {
                t.not_applied = true;
                continue;
            }
            t.not_applied = false;
            let v = match (r.ms, cfg.latency_limit_ms) {
                (Some(ms), Some(limit)) if ms > limit => Verdict::Fail(MonitorFailure::OverLimit(ms)),
                (Some(ms), _) => Verdict::Ok(ms),
                (None, _) if r.error == Some(DelayError::Timeout) => Verdict::Fail(MonitorFailure::Timeout),
                (None, _) => Verdict::Fail(MonitorFailure::Failed),
            };
            verdicts.push((i, v));
        }

        let mut notices = Vec::new();
        let all_no_latency = verdicts.iter().all(|(_, v)| v.no_latency());
        let network_round = !verdicts.is_empty()
            && all_no_latency
            && match internet {
                Some(online) => !online,
                None => verdicts.len() >= 2,
            };
        if network_round {
            self.network_fails += 1;
            if self.network_fails >= cfg.fail_threshold && !self.network_down {
                self.network_down = true;
                let body = if internet == Some(false) {
                    "Windows 显示本机未连接互联网，节点状态暂不更新。".to_string()
                } else {
                    format!("{} 个监测节点同时探测失败，请检查本机网络。", verdicts.len())
                };
                notices.push(MonitorNotice { title: "网络可能不可用".into(), body, focus: MonitorFocus::Settings });
            }
            return RoundOutcome::Applied(notices);
        }
        // Nothing usable, or (without the OS verdict) too few results to tell
        // the network is back (a lone failure is still "all failed"): leave
        // every state as is.
        if verdicts.is_empty() || (self.network_down && all_no_latency && internet != Some(true)) {
            return RoundOutcome::Applied(notices);
        }
        if self.network_down && cfg.notify_recovery {
            let body = if all_no_latency { "本机已重新连上互联网。" } else { "监测节点重新可以连通。" };
            notices.push(MonitorNotice { title: "网络已恢复".into(), body: body.into(), focus: MonitorFocus::Settings });
        }
        self.network_down = false;
        self.network_fails = 0;

        let mut new_downs: Vec<usize> = Vec::new();
        for (i, v) in verdicts {
            let t = &mut self.targets[i];
            t.probed = true;
            match v {
                Verdict::Ok(ms) => {
                    t.fails = 0;
                    t.last_ms = Some(ms);
                    t.last_failure = None;
                    if t.down {
                        t.down = false;
                        let since = t.down_since.take().unwrap_or(now);
                        if cfg.notify_recovery {
                            notices.push(MonitorNotice {
                                title: format!("节点已恢复：{}", t.label),
                                body: format!("不可用{}，当前延迟 {ms} ms", format_down_for(now - since)),
                                focus: focus_of(t),
                            });
                        }
                    }
                }
                Verdict::Fail(f) => {
                    t.fails += 1;
                    t.last_ms = match f {
                        MonitorFailure::OverLimit(ms) => Some(ms),
                        _ => None,
                    };
                    t.last_failure = Some(f);
                    if !t.down && t.fails >= cfg.fail_threshold {
                        t.down = true;
                        t.down_since = Some(now);
                        new_downs.push(i);
                    }
                }
            }
        }

        let downs: Vec<&MonitorTarget> = new_downs.iter().map(|&i| &self.targets[i]).collect();
        if downs.len() >= MERGE_AT {
            notices.push(merged_down_notice(&downs));
        } else {
            notices.extend(downs.into_iter().map(down_notice));
        }
        RoundOutcome::Applied(notices)
    }

    /// Monitoring turned off: forget everything, no notices.
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn is_empty(&self) -> bool {
        self.targets.is_empty()
    }

    pub fn targets(&self) -> Vec<MonitorTarget> {
        self.targets.clone()
    }

    pub fn network_down(&self) -> bool {
        self.network_down
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{NodeMonitorSettings, NodeRef, SubscriptionId};
    use crate::monitor::MonitorPause;
    use crate::stats::delay::{DelayError, DelayResult};

    fn nref(name: &str) -> NodeRef {
        NodeRef::Subscription { subscription_id: SubscriptionId("s".into()), name: name.into() }
    }

    fn spec(name: &str) -> TargetSpec {
        TargetSpec { proxy: format!("p/{name}"), node: Some(nref(name)), label: name.into(), used_by: vec![] }
    }

    fn book(specs: &[TargetSpec]) -> MonitorBook {
        let mut b = MonitorBook::default();
        b.reconcile(specs);
        b
    }

    fn ok(ms: u32) -> DelayResult {
        DelayResult { ms: Some(ms), tested_at: 0, error: None, stale: false }
    }

    fn err(e: DelayError) -> DelayResult {
        DelayResult::failed(e, 0)
    }

    fn all(specs: &[TargetSpec], r: DelayResult) -> Vec<(String, DelayResult)> {
        specs.iter().map(|s| (s.proxy.clone(), r.clone())).collect()
    }

    fn notices(o: RoundOutcome) -> Vec<MonitorNotice> {
        match o {
            RoundOutcome::Applied(n) => n,
            RoundOutcome::Voided => panic!("round voided"),
        }
    }

    fn cfg() -> NodeMonitorSettings {
        NodeMonitorSettings::default()
    }

    #[test]
    fn two_failures_mark_down_and_notify_once() {
        let s = vec![TargetSpec { used_by: vec!["工作代理 :7890".into()], ..spec("甲") }];
        let mut b = book(&s);
        let r = all(&s, err(DelayError::Timeout));
        assert!(notices(b.apply_round(&r, &cfg(), 10)).is_empty());
        let n = notices(b.apply_round(&r, &cfg(), 20));
        assert_eq!(
            n,
            vec![MonitorNotice {
                title: "节点不可用：甲".into(),
                body: "连续 2 次探测失败（超时） · 正在被「工作代理 :7890」使用".into(),
                focus: MonitorFocus::Node(nref("甲")),
            }]
        );
        let t = &b.targets()[0];
        assert!(t.down);
        assert_eq!(t.down_since, Some(20));
        assert_eq!(t.last_failure, Some(MonitorFailure::Timeout));
        assert!(notices(b.apply_round(&r, &cfg(), 30)).is_empty());
        assert_eq!(b.targets()[0].fails, 3);
    }

    #[test]
    fn down_notice_lists_every_user_and_none() {
        let s = vec![
            TargetSpec { used_by: vec!["A".into(), "B".into()], ..spec("甲") },
            TargetSpec { node: None, ..spec("自动") },
        ];
        let mut b = book(&s);
        let r = vec![(s[0].proxy.clone(), err(DelayError::Failed)), (s[1].proxy.clone(), ok(5000))];
        let c = NodeMonitorSettings { fail_threshold: 1, ..cfg() };
        let n = notices(b.apply_round(&r, &c, 0));
        assert_eq!(n[0].body, "连续 1 次探测失败（连接失败） · 正在被「A」、「B」使用");
        assert_eq!(n[1].body, "连续 1 次探测失败（延迟 5000 ms 超过上限）");
        assert_eq!(n[1].focus, MonitorFocus::Listeners);
    }

    #[test]
    fn recovery_notifies_with_duration() {
        let s = vec![spec("甲")];
        let mut b = book(&s);
        let fail = all(&s, err(DelayError::Timeout));
        b.apply_round(&fail, &cfg(), 0);
        b.apply_round(&fail, &cfg(), 0);
        assert_eq!(b.targets()[0].down_since, Some(0));
        let n = notices(b.apply_round(&all(&s, ok(48)), &cfg(), 900));
        assert_eq!(
            n,
            vec![MonitorNotice {
                title: "节点已恢复：甲".into(),
                body: "不可用约 15 分钟，当前延迟 48 ms".into(),
                focus: MonitorFocus::Node(nref("甲")),
            }]
        );
        let t = &b.targets()[0];
        assert!(!t.down && t.down_since.is_none() && t.fails == 0 && t.last_ms == Some(48));

        let quiet = NodeMonitorSettings { notify_recovery: false, ..cfg() };
        b.apply_round(&fail, &quiet, 0);
        b.apply_round(&fail, &quiet, 0);
        assert!(b.targets()[0].down);
        assert!(notices(b.apply_round(&all(&s, ok(48)), &quiet, 900)).is_empty());
        assert!(!b.targets()[0].down);
    }

    #[test]
    fn over_limit_counts_as_failure() {
        let s = vec![spec("甲")];
        let mut b = book(&s);
        let r = all(&s, ok(1280));
        b.apply_round(&r, &cfg(), 0);
        let n = notices(b.apply_round(&r, &cfg(), 0));
        assert_eq!(n[0].body, "连续 2 次探测失败（延迟 1280 ms 超过上限）");
        assert_eq!(b.targets()[0].last_failure, Some(MonitorFailure::OverLimit(1280)));

        // No limit: the same latency is fine.
        let mut b = book(&s);
        let unlimited = NodeMonitorSettings { latency_limit_ms: None, ..cfg() };
        b.apply_round(&r, &unlimited, 0);
        assert_eq!(b.targets()[0].fails, 0);
    }

    #[test]
    fn three_new_downs_merge_into_one() {
        let s: Vec<_> = ["甲", "乙", "丙"].map(spec).into();
        let mut b = book(&s);
        // One healthy target keeps this from being a network-down round.
        let mut r = all(&s, err(DelayError::Timeout));
        r.push(("其它".into(), ok(10)));
        let c = NodeMonitorSettings { fail_threshold: 1, ..cfg() };
        let mut specs = s.clone();
        specs.push(TargetSpec { proxy: "其它".into(), ..spec("其它") });
        b.reconcile(&specs);
        let n = notices(b.apply_round(&r, &c, 0));
        assert_eq!(
            n,
            vec![MonitorNotice { title: "3 个节点不可用".into(), body: "甲、乙、丙".into(), focus: MonitorFocus::Node(nref("甲")) }]
        );

        let names = ["一", "二", "三", "四", "五", "六", "七"];
        let mut s: Vec<_> = names.map(spec).into();
        s[0].node = None;
        let mut b = book(&s);
        let mut r = all(&s, ok(5000));
        r[6].1 = err(DelayError::Timeout);
        let n = notices(b.apply_round(&r, &c, 0));
        assert_eq!(n.len(), 1);
        assert_eq!(n[0].title, "7 个节点不可用");
        assert_eq!(n[0].body, "一、二、三、四、五等");
        assert_eq!(n[0].focus, MonitorFocus::Node(nref("二")));
    }

    #[test]
    fn all_failed_is_network_down_once() {
        let s: Vec<_> = ["甲", "乙", "丙"].map(spec).into();
        let mut b = book(&s);
        let r = all(&s, err(DelayError::Timeout));
        assert!(notices(b.apply_round(&r, &cfg(), 0)).is_empty());
        let n = notices(b.apply_round(&r, &cfg(), 0));
        assert_eq!(
            n,
            vec![MonitorNotice {
                title: "网络可能不可用".into(),
                body: "3 个监测节点同时探测失败，请检查本机网络。".into(),
                focus: MonitorFocus::Settings,
            }]
        );
        assert!(b.network_down());
        assert!(b.targets().iter().all(|t| t.fails == 0 && !t.down));
        assert!(notices(b.apply_round(&r, &cfg(), 0)).is_empty());

        let mut r = r;
        r[0].1 = ok(30);
        let n = notices(b.apply_round(&r, &cfg(), 0));
        assert_eq!(
            n,
            vec![MonitorNotice { title: "网络已恢复".into(), body: "监测节点重新可以连通。".into(), focus: MonitorFocus::Settings }]
        );
        assert!(!b.network_down());
        let fails: Vec<_> = b.targets().iter().map(|t| t.fails).collect();
        assert_eq!(fails, vec![0, 1, 1]);
    }

    #[test]
    fn online_os_turns_a_shared_outage_into_node_downs() {
        let s: Vec<_> = ["甲", "乙"].map(spec).into();
        let mut b = book(&s);
        let r = all(&s, err(DelayError::Timeout));
        b.apply_round_with(&r, &cfg(), 0, Some(true));
        let n = notices(b.apply_round_with(&r, &cfg(), 0, Some(true)));
        assert!(!b.network_down());
        assert_eq!(n.iter().map(|n| n.title.as_str()).collect::<Vec<_>>(), vec!["节点不可用：甲", "节点不可用：乙"]);
    }

    #[test]
    fn offline_os_is_network_down_even_for_one_target() {
        let s = vec![spec("甲")];
        let mut b = book(&s);
        let r = all(&s, err(DelayError::Failed));
        b.apply_round_with(&r, &cfg(), 0, Some(false));
        let n = notices(b.apply_round_with(&r, &cfg(), 0, Some(false)));
        assert_eq!(
            n,
            vec![MonitorNotice {
                title: "网络可能不可用".into(),
                body: "Windows 显示本机未连接互联网，节点状态暂不更新。".into(),
                focus: MonitorFocus::Settings,
            }]
        );
        assert_eq!(b.targets()[0].fails, 0);

        // Back online while the node still fails: the network recovers and the node starts counting.
        let n = notices(b.apply_round_with(&r, &cfg(), 0, Some(true)));
        assert_eq!(n[0], MonitorNotice { title: "网络已恢复".into(), body: "本机已重新连上互联网。".into(), focus: MonitorFocus::Settings });
        assert!(!b.network_down());
        assert_eq!(b.targets()[0].fails, 1);
    }

    #[test]
    fn offline_os_but_a_node_answers_is_not_network_down() {
        let s: Vec<_> = ["甲", "乙"].map(spec).into();
        let mut b = book(&s);
        let r = vec![(s[0].proxy.clone(), ok(30)), (s[1].proxy.clone(), err(DelayError::Timeout))];
        b.apply_round_with(&r, &cfg(), 0, Some(false));
        assert!(!b.network_down());
        assert_eq!(b.targets()[1].fails, 1);
    }

    #[test]
    fn single_target_has_no_network_detection() {
        let s = vec![spec("甲")];
        let mut b = book(&s);
        let r = all(&s, err(DelayError::Failed));
        b.apply_round(&r, &cfg(), 0);
        let n = notices(b.apply_round(&r, &cfg(), 0));
        assert_eq!(n[0].title, "节点不可用：甲");
        assert!(!b.network_down());
    }

    #[test]
    fn over_limit_only_round_is_not_network_down() {
        let s: Vec<_> = ["甲", "乙"].map(spec).into();
        let mut b = book(&s);
        let r = all(&s, ok(2000));
        b.apply_round(&r, &cfg(), 0);
        let n = notices(b.apply_round(&r, &cfg(), 0));
        assert!(!b.network_down());
        assert_eq!(n.len(), 2);
        assert!(b.targets().iter().all(|t| t.down));
    }

    #[test]
    fn core_errors_void_the_round() {
        let s: Vec<_> = ["甲", "乙"].map(spec).into();
        for core in [DelayError::NoCore, DelayError::CoreUnreachable("x".into())] {
            let mut b = book(&s);
            b.apply_round(&all(&s, err(DelayError::Failed)), &cfg(), 0);
            b.apply_round(&[(s[0].proxy.clone(), ok(10)), (s[1].proxy.clone(), err(DelayError::Failed))], &cfg(), 0);
            let before = b.targets();
            let r = vec![(s[0].proxy.clone(), err(DelayError::Timeout)), (s[1].proxy.clone(), err(core))];
            assert!(matches!(b.apply_round(&r, &cfg(), 0), RoundOutcome::Voided));
            assert_eq!(b.targets(), before);
        }
    }

    #[test]
    fn not_found_voids_only_that_target() {
        let s: Vec<_> = ["甲", "乙"].map(spec).into();
        let mut b = book(&s);
        let r = vec![(s[0].proxy.clone(), err(DelayError::NotFound)), (s[1].proxy.clone(), err(DelayError::Timeout))];
        assert!(notices(b.apply_round(&r, &cfg(), 0)).is_empty());
        let t = b.targets();
        assert!(t[0].not_applied && t[0].fails == 0 && !t[0].probed);
        assert!(!t[1].not_applied && t[1].fails == 1);
        // Applied again: the flag clears.
        b.apply_round(&all(&s, ok(10)), &cfg(), 0);
        assert!(!b.targets()[0].not_applied);
    }

    #[test]
    fn stale_result_is_ignored() {
        let s: Vec<_> = ["甲", "乙"].map(spec).into();
        let mut b = book(&s);
        let stale = DelayResult { stale: true, ..err(DelayError::Timeout) };
        let r = vec![(s[0].proxy.clone(), stale), (s[1].proxy.clone(), ok(10))];
        b.apply_round(&r, &cfg(), 0);
        let t = b.targets();
        assert!(!t[0].probed && t[0].fails == 0 && t[0].last_failure.is_none());
        assert!(t[1].probed);
    }

    #[test]
    fn all_stale_round_is_voided() {
        let s: Vec<_> = ["甲", "乙"].map(spec).into();
        let mut b = book(&s);
        let stale = DelayResult { stale: true, ..err(DelayError::Timeout) };
        // A result for a proxy outside the set does not count either way.
        let mut r = all(&s, stale);
        r.push(("其它".into(), ok(10)));
        assert!(matches!(b.apply_round(&r, &cfg(), 0), RoundOutcome::Voided));
        assert!(b.targets().iter().all(|t| !t.probed && t.fails == 0));
    }

    #[test]
    fn reconcile_drops_removed_target_silently() {
        let s: Vec<_> = ["甲", "乙"].map(spec).into();
        let mut b = book(&s);
        let r = vec![(s[0].proxy.clone(), err(DelayError::Failed)), (s[1].proxy.clone(), ok(10))];
        b.apply_round(&r, &cfg(), 0);
        b.apply_round(&r, &cfg(), 0);
        assert!(b.targets()[0].down);

        assert!(b.reconcile(&s[1..]));
        assert!(!b.reconcile(&s[1..]));
        // A late result for the removed proxy is ignored.
        let n = notices(b.apply_round(&r, &cfg(), 0));
        assert!(n.is_empty());
        let t = b.targets();
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].proxy, "p/乙");

        // Metadata changes are reported and keep the counters.
        b.apply_round(&[(s[1].proxy.clone(), err(DelayError::Failed))], &cfg(), 0);
        let renamed = TargetSpec { label: "乙 · 经 默认网卡".into(), ..s[1].clone() };
        assert!(b.reconcile(&[renamed]));
        assert_eq!(b.targets()[0].fails, 1);
        assert_eq!(b.targets()[0].label, "乙 · 经 默认网卡");
    }

    #[test]
    fn repointed_exit_starts_fresh() {
        // A Node exit keeps its proxy name (exit.name) but now points at another node.
        let s = vec![TargetSpec { proxy: "出口".into(), ..spec("甲") }];
        let mut b = book(&s);
        let fail = all(&s, err(DelayError::Timeout));
        b.apply_round(&fail, &cfg(), 0);
        b.apply_round(&fail, &cfg(), 0);
        assert!(b.targets()[0].down);

        let moved = vec![TargetSpec { proxy: "出口".into(), ..spec("乙") }];
        assert!(b.reconcile(&moved));
        let t = &b.targets()[0];
        assert!(!t.down && t.fails == 0 && !t.probed && t.down_since.is_none());
        assert!(notices(b.apply_round(&all(&moved, ok(48)), &cfg(), 900)).is_empty());

        // Becoming an auto group under the same name also starts fresh.
        b.apply_round(&all(&moved, err(DelayError::Timeout)), &cfg(), 0);
        let auto = vec![TargetSpec { node: None, label: "出口".into(), ..moved[0].clone() }];
        assert!(b.reconcile(&auto));
        assert_eq!(b.targets()[0].fails, 0);
    }

    #[test]
    fn threshold_change_applies_next_round() {
        let s = vec![spec("甲")];
        let mut b = book(&s);
        let r = all(&s, err(DelayError::Timeout));
        b.apply_round(&r, &cfg(), 0);
        assert_eq!(b.targets()[0].fails, 1);
        let c = NodeMonitorSettings { fail_threshold: 1, ..cfg() };
        let n = notices(b.apply_round(&r, &c, 0));
        assert_eq!(n[0].body, "连续 2 次探测失败（超时）");
        assert!(b.targets()[0].down);
    }

    #[test]
    fn clear_resets_everything() {
        let s: Vec<_> = ["甲", "乙"].map(spec).into();
        let mut b = book(&s);
        let r = all(&s, err(DelayError::Timeout));
        b.apply_round(&r, &cfg(), 0);
        b.apply_round(&r, &cfg(), 0);
        assert!(b.network_down());
        b.clear();
        assert!(!b.network_down());
        assert!(b.targets().is_empty());
    }

    #[test]
    fn format_down_for_cases() {
        assert_eq!(format_down_for(30), "不到 1 分钟");
        assert_eq!(format_down_for(900), "约 15 分钟");
        assert_eq!(format_down_for(7500), "约 2 小时 5 分钟");
        assert_eq!(format_down_for(3600), "约 1 小时");
        assert_eq!(format_down_for(7200), "约 2 小时");
    }

    #[test]
    fn serialized_shapes() {
        assert_eq!(serde_json::to_value(MonitorPause::CoreNotRunning).unwrap(), serde_json::json!("CoreNotRunning"));
        assert_eq!(serde_json::to_value(MonitorFailure::OverLimit(5)).unwrap(), serde_json::json!({"OverLimit": 5}));
        assert_eq!(serde_json::to_value(MonitorFocus::Settings).unwrap(), serde_json::json!("Settings"));
    }
}

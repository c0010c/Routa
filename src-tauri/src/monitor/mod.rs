//! Node monitoring: which proxies to watch, how each probe round is judged,
//! and the notice copy. Pure; the runtime loop drives it.

mod judge;
mod targets;

pub use judge::{format_down_for, MonitorBook, RoundOutcome};
pub use targets::monitor_targets;

use serde::{Deserialize, Serialize};

use crate::model::NodeRef;

/// One path to probe, as expanded from the settings.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, specta::Type)]
pub struct TargetSpec {
    /// mihomo proxy name to probe; also the DelayBook key.
    pub proxy: String,
    /// `None` when the target is an auto-selecting group exit.
    pub node: Option<NodeRef>,
    pub label: String,
    /// Who uses this path (InUse scope only).
    pub used_by: Vec<String>,
}

/// Why a target failed a round.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, specta::Type)]
pub enum MonitorFailure {
    Timeout,
    Failed,
    OverLimit(u32),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, specta::Type)]
pub enum MonitorPause {
    CoreNotRunning,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, specta::Type)]
pub struct MonitorTarget {
    pub proxy: String,
    pub node: Option<NodeRef>,
    pub label: String,
    pub used_by: Vec<String>,
    /// Consecutive failed rounds.
    pub fails: u32,
    pub down: bool,
    /// Unix seconds.
    pub down_since: Option<i64>,
    /// Latency of the last measured round (also set when over the limit).
    pub last_ms: Option<u32>,
    pub last_failure: Option<MonitorFailure>,
    /// At least one round produced a usable result.
    pub probed: bool,
    /// mihomo has no proxy by that name (exit not applied).
    pub not_applied: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default, specta::Type)]
pub struct MonitorStatus {
    pub last_run: Option<i64>,
    pub next_run: Option<i64>,
    pub probing: bool,
    pub paused_reason: Option<MonitorPause>,
    pub network_down: bool,
    pub targets: Vec<MonitorTarget>,
}

/// Where clicking a notice leads.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, specta::Type)]
pub enum MonitorFocus {
    Node(NodeRef),
    Listeners,
    Settings,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, specta::Type)]
pub struct MonitorNotice {
    pub title: String,
    pub body: String,
    pub focus: MonitorFocus,
}

//! What the status bar shows, free of Win32 so every rule is unit-testable.

use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

use crate::desktop::{CaptureMode, TaskbarStatusMode};
use crate::model::{Exit, ExitKind, GroupStrategy, Listener, ListenerId, Model, NodeRef};
use crate::stats::delay::DelayBook;
use crate::stats::traffic::Rate;

/// Seconds of traffic on the chart, one point per second.
pub const HISTORY_POINTS: usize = 10;
const SLIDE_HOLD: Duration = Duration::from_secs(3);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Good,
    Warn,
    Bad,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DelayView {
    Ms(u32, Tone),
    Failed,
    Unknown,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ListenerView {
    pub id: ListenerId,
    pub node: String,
    pub delay: DelayView,
    pub up: u64,
    pub down: u64,
    pub series: Vec<Rate>,
    /// Set while system proxy or TUN is on: the view shows that capture.
    pub capture: Option<CaptureMode>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Screen {
    NotRunning,
    NoListeners,
    Listener(ListenerView),
}

/// System proxy or TUN, and the listener whose exit it uses. While one is on
/// the bar shows only its traffic instead of rotating through listeners.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Capture {
    pub mode: CaptureMode,
    pub listener: ListenerId,
}

/// Per-app capture has no traffic of its own: the bar keeps rotating.
pub fn capture_of(active: Option<(CaptureMode, ListenerId)>) -> Option<Capture> {
    match active? {
        (CaptureMode::Apps, _) => None,
        (mode, listener) => Some(Capture { mode, listener }),
    }
}

/// System proxy traffic is its listener's own; TUN traffic comes in on no
/// listener, so it stands in for the listener TUN routes through.
pub fn effective_rates(rates: &HashMap<ListenerId, Rate>, capture: Option<&Capture>, tun: &Rate) -> HashMap<ListenerId, Rate> {
    let mut rates = rates.clone();
    if let Some(Capture { mode: CaptureMode::Tun, listener }) = capture {
        rates.insert(listener.clone(), tun.clone());
    }
    rates
}

/// The runtime values the view needs.
pub struct Input<'a> {
    pub running: bool,
    pub delays: &'a DelayBook,
    pub rates: &'a HashMap<ListenerId, Rate>,
    pub capture: Option<&'a Capture>,
}

/// Always with a unit and at most three digits, so it fits the values column.
pub fn format_rate(bytes_per_sec: u64) -> String {
    if bytes_per_sec < 1000 {
        return format!("{bytes_per_sec}B");
    }
    let mut value = bytes_per_sec as f64 / 1024.0;
    let mut unit = 0;
    // Round up a unit before "1000K" would need four digits.
    while value >= 999.5 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    if value < 9.95 {
        format!("{value:.1}{}", UNITS[unit])
    } else {
        format!("{}{}", value.round() as u64, UNITS[unit])
    }
}

const UNITS: [&str; 4] = ["K", "M", "G", "T"];

fn exit_of<'a>(model: &'a Model, listener: &Listener) -> Option<&'a Exit> {
    model.exits.iter().find(|e| Some(&e.id) == listener.exit.as_ref())
}

fn node_label(model: &Model, node: &NodeRef) -> String {
    match node {
        NodeRef::Subscription { subscription_id, name } => {
            let known = model
                .subscriptions
                .iter()
                .any(|s| &s.id == subscription_id && s.nodes.iter().any(|n| &n.name == name));
            if known {
                name.clone()
            } else {
                format!("{name}（已删除）")
            }
        }
        NodeRef::Manual { manual_id } => model
            .manual_nodes
            .iter()
            .find(|n| &n.id == manual_id)
            .map_or_else(|| "（已删除）".into(), |n| n.name.clone()),
    }
}

/// Auto groups show their own name: the live member is not looked up.
pub fn node_text(model: &Model, listener: &Listener) -> String {
    let Some(exit) = exit_of(model, listener) else { return "未设置".into() };
    match &exit.kind {
        ExitKind::Direct => "直连".into(),
        ExitKind::Node { node } => node_label(model, node),
        ExitKind::Group { strategy: GroupStrategy::Select, selected: Some(node), .. } => node_label(model, node),
        ExitKind::Group { strategy: GroupStrategy::Select, selected: None, .. } => "尚未选择".into(),
        ExitKind::Group { .. } => exit.name.clone(),
    }
}

/// Same thresholds as `delayColor` in `src/lib/format.ts`.
pub fn delay_view(delays: &DelayBook, model: &Model, listener: &Listener) -> DelayView {
    let Some(exit) = exit_of(model, listener) else { return DelayView::Unknown };
    if matches!(exit.kind, ExitKind::Direct) {
        return DelayView::Unknown;
    }
    match delays.get(&exit.name) {
        None => DelayView::Unknown,
        Some(result) if result.stale => DelayView::Unknown,
        Some(result) => match result.ms {
            None => DelayView::Failed,
            Some(ms) if ms <= 300 => DelayView::Ms(ms, Tone::Good),
            Some(ms) if ms <= 600 => DelayView::Ms(ms, Tone::Warn),
            Some(ms) => DelayView::Ms(ms, Tone::Bad),
        },
    }
}

/// Up and down polylines on one scale; y grows downward, zero sits on `height`.
pub fn chart_points(series: &[Rate], width: f32, height: f32) -> (Vec<(f32, f32)>, Vec<(f32, f32)>) {
    let max = series.iter().map(|r| r.up.max(r.down)).max().unwrap_or(0).max(1) as f32;
    let step = width / series.len().saturating_sub(1).max(1) as f32;
    let line = |value: fn(&Rate) -> u64| {
        series
            .iter()
            .enumerate()
            .map(|(i, r)| (i as f32 * step, height - value(r) as f32 / max * height))
            .collect()
    };
    (line(|r| r.up), line(|r| r.down))
}

#[derive(Default)]
pub struct History {
    points: HashMap<ListenerId, VecDeque<Rate>>,
    capture: Option<Capture>,
}

impl History {
    /// Starts over when the capture changes: the same listener id would
    /// otherwise chart TUN and listener traffic as one line.
    pub fn record(&mut self, running: bool, rates: &HashMap<ListenerId, Rate>, model: &Model, capture: Option<&Capture>) {
        if !running || self.capture.as_ref() != capture {
            self.points.clear();
            self.capture = capture.cloned();
        }
        if !running {
            return;
        }
        self.points.retain(|id, _| model.listeners.iter().any(|l| &l.id == id));
        for listener in &model.listeners {
            let points = self.points.entry(listener.id.clone()).or_default();
            points.push_back(rates.get(&listener.id).cloned().unwrap_or_default());
            if points.len() > HISTORY_POINTS {
                points.pop_front();
            }
        }
    }

    /// Always `HISTORY_POINTS` long, oldest first, zero-padded.
    pub fn series(&self, id: &ListenerId) -> Vec<Rate> {
        let points = self.points.get(id);
        let known = points.map_or(0, VecDeque::len);
        let mut series = vec![Rate::default(); HISTORY_POINTS - known];
        series.extend(points.into_iter().flatten().cloned());
        series
    }
}

#[derive(Default)]
pub struct Rotation {
    current: Option<ListenerId>,
    since: Option<Instant>,
}

impl Rotation {
    /// The listener to show now. A slide waits for `scroll_done` so a long node
    /// name is read to its end first.
    pub fn next_slide(
        &mut self,
        now: Instant,
        scroll_done: bool,
        candidates: &[ListenerId],
        mode: &TaskbarStatusMode,
    ) -> Option<ListenerId> {
        if let TaskbarStatusMode::Fixed(id) = mode {
            if candidates.contains(id) {
                self.current = Some(id.clone());
                self.since = Some(now);
                return self.current.clone();
            }
        }
        let position = self.current.as_ref().and_then(|c| candidates.iter().position(|id| id == c));
        let Some(position) = position else {
            self.current = candidates.first().cloned();
            self.since = Some(now);
            return self.current.clone();
        };
        let held = self.since.is_some_and(|since| now.duration_since(since) >= SLIDE_HOLD);
        if held && scroll_done && candidates.len() > 1 {
            self.current = Some(candidates[(position + 1) % candidates.len()].clone());
            self.since = Some(now);
        }
        self.current.clone()
    }
}

/// Only the capture's listener while system proxy or TUN is on.
pub fn candidates(model: &Model, capture: Option<&Capture>) -> Vec<ListenerId> {
    if let Some(c) = capture.filter(|c| model.listeners.iter().any(|l| l.id == c.listener)) {
        return vec![c.listener.clone()];
    }
    model.listeners.iter().filter(|l| l.user_enabled).map(|l| l.id.clone()).collect()
}

pub fn screen(model: &Model, input: &Input, history: &History, current: Option<&ListenerId>) -> Screen {
    if !input.running {
        return Screen::NotRunning;
    }
    let Some(listener) = current.and_then(|id| model.listeners.iter().find(|l| &l.id == id)) else {
        return Screen::NoListeners;
    };
    let rate = input.rates.get(&listener.id).cloned().unwrap_or_default();
    Screen::Listener(ListenerView {
        id: listener.id.clone(),
        node: node_text(model, listener),
        delay: delay_view(input.delays, model, listener),
        up: rate.up,
        down: rate.down,
        series: history.series(&listener.id),
        capture: input.capture.filter(|c| c.listener == listener.id).map(|c| c.mode),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;
    use crate::stats::delay::DelayResult;
    use std::collections::HashMap;
    use std::time::Duration;

    fn lid(id: &str) -> ListenerId {
        ListenerId(id.into())
    }

    fn listener(id: &str, exit: Option<&str>) -> Listener {
        serde_json::from_value(serde_json::json!({
            "id": id, "name": id, "listener_type": "Mixed", "listen": "127.0.0.1", "port": 1,
            "auth": null, "exit": exit, "user_enabled": true,
        }))
        .unwrap()
    }

    fn exit(id: &str, name: &str, kind: ExitKind) -> Exit {
        Exit { id: ExitId(id.into()), name: name.into(), kind, interface: None }
    }

    fn manual(id: &str, name: &str) -> ManualNode {
        ManualNode { id: ManualNodeId(id.into()), name: name.into(), ..Default::default() }
    }

    fn manual_ref(id: &str) -> NodeRef {
        NodeRef::Manual { manual_id: ManualNodeId(id.into()) }
    }

    fn group(strategy: GroupStrategy, selected: Option<NodeRef>) -> ExitKind {
        ExitKind::Group { strategy, members: vec![], selected }
    }

    fn model_with(kind: ExitKind) -> (Model, Listener) {
        let model = Model {
            exits: vec![exit("e", "香港自动", kind)],
            manual_nodes: vec![manual("m", "甲")],
            ..Default::default()
        };
        (model, listener("l", Some("e")))
    }

    fn delays(name: &str, ms: Option<u32>, stale: bool) -> DelayBook {
        let mut book = DelayBook::default();
        book.record(name.into(), DelayResult { ms, tested_at: 1, error: None, stale });
        book
    }

    fn rates(pairs: &[(&str, u64, u64)]) -> HashMap<ListenerId, Rate> {
        pairs.iter().map(|(id, up, down)| (lid(id), Rate { up: *up, down: *down })).collect()
    }

    #[test]
    fn format_rate_always_has_a_unit() {
        assert_eq!(format_rate(0), "0B");
        assert_eq!(format_rate(512), "512B");
        assert_eq!(format_rate(999), "999B");
        assert_eq!(format_rate(1000), "1.0K");
        assert_eq!(format_rate(12_400), "12K");
        assert_eq!(format_rate(3_355_443), "3.2M");
        assert_eq!(format_rate(52_428_800), "50M");
        assert_eq!(format_rate(2_147_483_648), "2.0G");
    }

    #[test]
    fn format_rate_keeps_to_three_digits() {
        assert_eq!(format_rate(1_022_976), "999K");
        assert_eq!(format_rate(1_023_488), "1.0M");
        assert_eq!(format_rate(1_048_575), "1.0M");
        assert_eq!(format_rate(1_073_741_823), "1.0G");
        for n in (0..40).map(|i| 1u64 << i).flat_map(|n| [n - 1, n, n + n / 3]) {
            let digits = format_rate(n).chars().filter(char::is_ascii_digit).count();
            assert!(digits <= 3, "{n} -> {}", format_rate(n));
        }
    }

    #[test]
    fn node_text_auto_group_uses_group_name() {
        for strategy in [GroupStrategy::UrlTest, GroupStrategy::Fallback, GroupStrategy::LoadBalance] {
            let (model, l) = model_with(group(strategy, None));
            assert_eq!(node_text(&model, &l), "香港自动");
        }
    }

    #[test]
    fn node_text_select_group_uses_selected_node() {
        let (model, l) = model_with(group(GroupStrategy::Select, Some(manual_ref("m"))));
        assert_eq!(node_text(&model, &l), "甲");
        let (model, l) = model_with(group(GroupStrategy::Select, None));
        assert_eq!(node_text(&model, &l), "尚未选择");
    }

    #[test]
    fn node_text_fixed_node_and_direct() {
        let (model, l) = model_with(ExitKind::Node { node: manual_ref("m") });
        assert_eq!(node_text(&model, &l), "甲");
        let (model, l) = model_with(ExitKind::Direct);
        assert_eq!(node_text(&model, &l), "直连");
        let (model, _) = model_with(ExitKind::Direct);
        assert_eq!(node_text(&model, &listener("l", None)), "未设置");
        assert_eq!(node_text(&model, &listener("l", Some("gone"))), "未设置");
    }

    #[test]
    fn node_text_marks_deleted_nodes() {
        let (model, l) = model_with(ExitKind::Node { node: manual_ref("gone") });
        assert_eq!(node_text(&model, &l), "（已删除）");
        let (model, l) = model_with(ExitKind::Node {
            node: NodeRef::Subscription { subscription_id: SubscriptionId("s".into()), name: "乙".into() },
        });
        assert_eq!(node_text(&model, &l), "乙（已删除）");
    }

    #[test]
    fn delay_uses_exit_name_key() {
        let (model, l) = model_with(group(GroupStrategy::UrlTest, None));
        let at = |ms| delay_view(&delays("香港自动", Some(ms), false), &model, &l);
        assert_eq!(at(300), DelayView::Ms(300, Tone::Good));
        assert_eq!(at(301), DelayView::Ms(301, Tone::Warn));
        assert_eq!(at(600), DelayView::Ms(600, Tone::Warn));
        assert_eq!(at(601), DelayView::Ms(601, Tone::Bad));
        assert_eq!(delay_view(&delays("香港自动", None, false), &model, &l), DelayView::Failed);
        assert_eq!(delay_view(&delays("别的", Some(1), false), &model, &l), DelayView::Unknown);
        assert_eq!(delay_view(&delays("香港自动", Some(1), true), &model, &l), DelayView::Unknown);
    }

    #[test]
    fn delay_is_unknown_for_direct_and_missing_exit() {
        let (model, l) = model_with(ExitKind::Direct);
        assert_eq!(delay_view(&delays("香港自动", Some(1), false), &model, &l), DelayView::Unknown);
        assert_eq!(delay_view(&DelayBook::default(), &model, &listener("l", None)), DelayView::Unknown);
    }

    #[test]
    fn chart_points_scale_to_window_max() {
        let series = [Rate { up: 0, down: 100 }, Rate { up: 50, down: 0 }];
        let (up, down) = chart_points(&series, 10.0, 20.0);
        assert_eq!(up, vec![(0.0, 20.0), (10.0, 10.0)]);
        assert_eq!(down, vec![(0.0, 0.0), (10.0, 20.0)]);
    }

    #[test]
    fn chart_points_all_zero_stays_on_baseline() {
        let (up, down) = chart_points(&vec![Rate::default(); 10], 26.0, 36.0);
        assert!(up.iter().chain(&down).all(|(x, y)| *y == 36.0 && x.is_finite()));
    }

    #[test]
    fn history_pads_to_ten_and_drops_old() {
        let model = Model { listeners: vec![listener("l", None)], ..Default::default() };
        let mut history = History::default();
        history.record(true, &rates(&[("l", 1, 1)]), &model, None);
        let series = history.series(&lid("l"));
        assert_eq!(series.len(), 10);
        assert_eq!(series[8], Rate::default());
        assert_eq!(series[9], Rate { up: 1, down: 1 });
        for n in 2..=12 {
            history.record(true, &rates(&[("l", n, n)]), &model, None);
        }
        let series = history.series(&lid("l"));
        assert_eq!(series.len(), 10);
        assert_eq!(series[0].up, 3);
        assert_eq!(series[9].up, 12);
    }

    #[test]
    fn history_records_zero_for_a_quiet_listener() {
        let model = Model { listeners: vec![listener("l", None)], ..Default::default() };
        let mut history = History::default();
        history.record(true, &rates(&[("l", 5, 5)]), &model, None);
        history.record(true, &rates(&[]), &model, None);
        assert_eq!(history.series(&lid("l"))[9], Rate::default());
        assert_eq!(history.series(&lid("l"))[8].up, 5);
    }

    #[test]
    fn history_forgets_deleted_listener_and_clears_when_core_stops() {
        let mut model = Model { listeners: vec![listener("l", None)], ..Default::default() };
        let mut history = History::default();
        history.record(true, &rates(&[("l", 5, 5)]), &model, None);
        model.listeners.clear();
        history.record(true, &rates(&[]), &model, None);
        model.listeners.push(listener("l", None));
        assert!(history.series(&lid("l")).iter().all(|r| r.up == 0));
        history.record(true, &rates(&[("l", 5, 5)]), &model, None);
        history.record(false, &rates(&[]), &model, None);
        assert!(history.series(&lid("l")).iter().all(|r| r.up == 0));
    }

    #[test]
    fn rotation_holds_three_seconds_and_waits_for_scroll() {
        let ids = [lid("a"), lid("b")];
        let start = Instant::now();
        let mut r = Rotation::default();
        let mode = TaskbarStatusMode::Rotate;
        assert_eq!(r.next_slide(start, true, &ids, &mode), Some(lid("a")));
        assert_eq!(r.next_slide(start + Duration::from_secs(2), true, &ids, &mode), Some(lid("a")));
        assert_eq!(r.next_slide(start + Duration::from_secs(4), false, &ids, &mode), Some(lid("a")));
        assert_eq!(r.next_slide(start + Duration::from_secs(5), true, &ids, &mode), Some(lid("b")));
        assert_eq!(r.next_slide(start + Duration::from_secs(9), true, &ids, &mode), Some(lid("a")));
    }

    #[test]
    fn rotation_single_candidate_never_slides() {
        let ids = [lid("a")];
        let start = Instant::now();
        let mut r = Rotation::default();
        r.next_slide(start, true, &ids, &TaskbarStatusMode::Rotate);
        assert_eq!(r.next_slide(start + Duration::from_secs(60), true, &ids, &TaskbarStatusMode::Rotate), Some(lid("a")));
    }

    #[test]
    fn rotation_fixed_missing_falls_back_to_rotate() {
        let ids = [lid("a"), lid("b")];
        let start = Instant::now();
        let mut r = Rotation::default();
        let fixed = TaskbarStatusMode::Fixed(lid("b"));
        assert_eq!(r.next_slide(start, true, &ids, &fixed), Some(lid("b")));
        assert_eq!(r.next_slide(start + Duration::from_secs(60), true, &ids, &fixed), Some(lid("b")));
        let gone = TaskbarStatusMode::Fixed(lid("x"));
        assert_eq!(r.next_slide(start + Duration::from_secs(120), true, &ids, &gone), Some(lid("a")));
    }

    #[test]
    fn rotation_moves_off_a_removed_listener_and_empties() {
        let start = Instant::now();
        let mut r = Rotation::default();
        r.next_slide(start, true, &[lid("a"), lid("b")], &TaskbarStatusMode::Rotate);
        assert_eq!(r.next_slide(start, true, &[lid("b")], &TaskbarStatusMode::Rotate), Some(lid("b")));
        assert_eq!(r.next_slide(start, true, &[], &TaskbarStatusMode::Rotate), None);
    }

    #[test]
    fn candidates_are_enabled_listeners_in_model_order() {
        let mut off = listener("b", None);
        off.user_enabled = false;
        let model = Model { listeners: vec![listener("c", None), off, listener("a", None)], ..Default::default() };
        assert_eq!(candidates(&model, None), vec![lid("c"), lid("a")]);
    }

    #[test]
    fn screen_states() {
        let (mut model, l) = model_with(ExitKind::Direct);
        model.listeners.push(l);
        let history = History::default();
        let book = DelayBook::default();
        let none = rates(&[]);
        let input = |running| Input { running, delays: &book, rates: &none, capture: None };
        assert_eq!(screen(&model, &input(false), &history, Some(&lid("l"))), Screen::NotRunning);
        assert_eq!(screen(&model, &input(true), &history, None), Screen::NoListeners);
        let Screen::Listener(view) = screen(&model, &input(true), &history, Some(&lid("l"))) else {
            panic!("expected a listener screen");
        };
        assert_eq!(view.node, "直连");
        assert_eq!(view.series.len(), 10);
        assert_eq!(screen(&model, &input(true), &history, Some(&lid("x"))), Screen::NoListeners);
    }

    #[test]
    fn screen_uses_the_current_listeners_own_rates() {
        let (mut model, l) = model_with(ExitKind::Direct);
        model.listeners.push(l);
        let book = DelayBook::default();
        let rate = rates(&[("l", 7, 9), ("other", 100, 100)]);
        let input = Input { running: true, delays: &book, rates: &rate, capture: None };
        let Screen::Listener(view) = screen(&model, &input, &History::default(), Some(&lid("l"))) else {
            panic!("expected a listener screen");
        };
        assert_eq!((view.up, view.down), (7, 9));
    }

    fn capture(mode: CaptureMode, id: &str) -> Capture {
        Capture { mode, listener: lid(id) }
    }

    #[test]
    fn per_app_capture_keeps_rotating() {
        assert_eq!(capture_of(Some((CaptureMode::Apps, lid("l")))), None);
        assert_eq!(capture_of(None), None);
        assert_eq!(capture_of(Some((CaptureMode::Tun, lid("l")))), Some(capture(CaptureMode::Tun, "l")));
    }

    #[test]
    fn a_capture_is_the_only_candidate() {
        let model = Model { listeners: vec![listener("a", None), listener("b", None)], ..Default::default() };
        let tun = capture(CaptureMode::Tun, "b");
        assert_eq!(candidates(&model, Some(&tun)), vec![lid("b")]);
        let gone = capture(CaptureMode::System, "x");
        assert_eq!(candidates(&model, Some(&gone)), vec![lid("a"), lid("b")]);
    }

    #[test]
    fn tun_traffic_stands_in_for_its_listener() {
        let raw = rates(&[("a", 1, 1), ("b", 2, 2)]);
        let tun = Rate { up: 70, down: 90 };
        let shown = effective_rates(&raw, Some(&capture(CaptureMode::Tun, "a")), &tun);
        assert_eq!((shown[&lid("a")].clone(), shown[&lid("b")].clone()), (tun.clone(), Rate { up: 2, down: 2 }));
        assert_eq!(effective_rates(&raw, Some(&capture(CaptureMode::System, "a")), &tun), raw);
        assert_eq!(effective_rates(&raw, None, &tun), raw);
    }

    #[test]
    fn screen_marks_the_captured_listener() {
        let (mut model, l) = model_with(ExitKind::Direct);
        model.listeners.push(l);
        let book = DelayBook::default();
        let none = rates(&[]);
        let system = capture(CaptureMode::System, "l");
        let input = Input { running: true, delays: &book, rates: &none, capture: Some(&system) };
        let Screen::Listener(view) = screen(&model, &input, &History::default(), Some(&lid("l"))) else {
            panic!("expected a listener screen");
        };
        assert_eq!(view.capture, Some(CaptureMode::System));
        let input = Input { capture: None, ..input };
        let Screen::Listener(view) = screen(&model, &input, &History::default(), Some(&lid("l"))) else {
            panic!("expected a listener screen");
        };
        assert_eq!(view.capture, None);
    }

    #[test]
    fn history_starts_over_when_the_capture_changes() {
        let model = Model { listeners: vec![listener("l", None)], ..Default::default() };
        let tun = capture(CaptureMode::Tun, "l");
        let mut history = History::default();
        history.record(true, &rates(&[("l", 5, 5)]), &model, None);
        history.record(true, &rates(&[("l", 9, 9)]), &model, Some(&tun));
        let series = history.series(&lid("l"));
        assert_eq!((series[8].up, series[9].up), (0, 9));
        history.record(true, &rates(&[("l", 3, 3)]), &model, Some(&tun));
        assert_eq!(history.series(&lid("l"))[8].up, 9);
    }
}

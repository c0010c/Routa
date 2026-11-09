//! Background loops. All use tokio time (tests run with a paused clock) and
//! are aborted by `Runtime::shutdown`.

use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use tokio::task::JoinHandle;
use tokio::time::{interval_at, Instant, MissedTickBehavior};

use super::{now_unix, Inner};
use crate::core::api::MihomoApi;
use crate::core::apply::ApplyOutcome;
use crate::core::process::{backoff_delay, CoreState};
use crate::model::NodeMonitorSettings;
use crate::monitor::{monitor_targets, MonitorPause, RoundOutcome, MONITOR_TIMEOUT_MS};
use crate::scheduler::delay_test_due;
use crate::stats::traffic::Rate;

const SUPERVISE_EVERY: Duration = Duration::from_secs(1);
const STABLE_AFTER: Duration = Duration::from_secs(60);
const NIC_POLL_EVERY: Duration = Duration::from_secs(3);
const PORT_RETRY_EVERY: Duration = Duration::from_secs(30);
const SUBSCRIPTION_CHECK_EVERY: Duration = Duration::from_secs(60);
const DELAY_CHECK_EVERY: Duration = Duration::from_secs(60);
const TRAFFIC_SAMPLE_EVERY: Duration = Duration::from_secs(1);
const TOTALS_SAVE_EVERY: Duration = Duration::from_secs(30);
const STREAM_RETRY_EVERY: Duration = Duration::from_secs(1);
const MONITOR_TICK: Duration = Duration::from_secs(1);
/// Wait before retrying a round voided by a core error (capped at the interval).
const MONITOR_VOID_RETRY: Duration = Duration::from_secs(10);

/// Runs `body` every `period`; the first run is after `first_delay`.
fn every<F, Fut>(inner: &Arc<Inner>, first_delay: Duration, period: Duration, body: F) -> JoinHandle<()>
where
    F: Fn(Arc<Inner>) -> Fut + Send + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    let inner = inner.clone();
    tokio::spawn(async move {
        let mut tick = interval_at(Instant::now() + first_delay, period);
        tick.set_missed_tick_behavior(MissedTickBehavior::Delay);
        loop {
            tick.tick().await;
            body(inner.clone()).await;
        }
    })
}

pub(super) fn spawn_all(inner: &Arc<Inner>) -> Vec<JoinHandle<()>> {
    vec![
        tokio::spawn(supervise(inner.clone())),
        every(inner, NIC_POLL_EVERY, NIC_POLL_EVERY, |i| async move {
            i.refresh_interfaces().await;
        }),
        every(inner, PORT_RETRY_EVERY, PORT_RETRY_EVERY, retry_failed_apply),
        every(inner, Duration::ZERO, SUBSCRIPTION_CHECK_EVERY, |i| async move {
            i.update_due_subscriptions().await
        }),
        every(inner, Duration::ZERO, DELAY_CHECK_EVERY, scheduled_delay_test),
        tokio::spawn(sample_traffic(inner.clone())),
        every(inner, TOTALS_SAVE_EVERY, TOTALS_SAVE_EVERY, |i| async move { i.save_totals() }),
        tokio::spawn(monitor_loop(inner.clone())),
    ]
}

/// Detects core exits and restarts with backoff, always from the latest model.
async fn supervise(inner: Arc<Inner>) {
    let mut attempt: u32 = 0;
    let mut restart_at: Option<Instant> = None;
    let mut tick = interval_at(Instant::now() + SUPERVISE_EVERY, SUPERVISE_EVERY);
    tick.set_missed_tick_behavior(MissedTickBehavior::Delay);
    loop {
        tick.tick().await;
        let mut eng = inner.engine.lock().await;
        if !eng.supervise {
            attempt = 0;
            restart_at = None;
            continue;
        }
        let now = Instant::now();
        if let Some(run) = eng.core.as_mut() {
            restart_at = None;
            match run.handle.try_wait() {
                None => {
                    if now.duration_since(run.started) >= STABLE_AFTER {
                        attempt = 0;
                    }
                }
                Some(exit_code) => {
                    eprintln!("runtime: mihomo exited ({exit_code:?})");
                    inner.drop_core(&mut eng).await;
                    inner.status.lock().unwrap().failed_listeners.clear();
                    inner.set_core_state(CoreState::Crashed { exit_code });
                    restart_at = Some(now + backoff_delay(attempt));
                    attempt = attempt.saturating_add(1);
                }
            }
            continue;
        }
        // Not running while supervised: a crash, a failed start, or a core
        // stopped by a failed apply.
        let at = *restart_at.get_or_insert_with(|| {
            let at = now + backoff_delay(attempt);
            attempt = attempt.saturating_add(1);
            at
        });
        if now >= at {
            restart_at = None;
            let _ = inner.start_core(&mut eng).await;
        }
    }
}

/// Re-applies the latest model while something is not in effect: a listener
/// whose port was busy (spike S5: re-PUTting the same config binds it once
/// the port is free) or a rejected/failed reload.
async fn retry_failed_apply(inner: Arc<Inner>) {
    let mut eng = inner.engine.lock().await;
    if eng.core.is_none() {
        return;
    }
    let needed = {
        let st = inner.status.lock().unwrap();
        !st.failed_listeners.is_empty()
            || matches!(
                st.last_apply,
                Some(ApplyOutcome::FailedClosed { .. } | ApplyOutcome::FailedKeptPrevious { .. })
            )
    };
    if needed {
        inner.apply(&mut eng).await;
    }
}

async fn scheduled_delay_test(inner: Arc<Inner>) {
    let (last, interval) = {
        let live = inner.live.lock().unwrap();
        if live.api.is_none() {
            return;
        }
        let interval = inner.model.lock().unwrap().settings.delay_test_interval_mins;
        (live.last_delay_run, interval)
    };
    let now = now_unix();
    if !delay_test_due(last, interval, now) {
        return;
    }
    inner.live.lock().unwrap().last_delay_run = Some(now);
    let names = inner.delay_targets();
    inner.test_delays(names).await;
}

/// Node monitor: one check per tick; a round runs inside the tick, so rounds
/// never overlap and the interval counts from the end of the previous one.
async fn monitor_loop(inner: Arc<Inner>) {
    let mut tick = interval_at(Instant::now() + MONITOR_TICK, MONITOR_TICK);
    tick.set_missed_tick_behavior(MissedTickBehavior::Delay);
    loop {
        tick.tick().await;
        monitor_tick(&inner).await;
    }
}

/// The settings whose change makes a round due right away (spec §4: turning
/// monitoring on, changing the members or the interval). Threshold, latency
/// limit and recovery notices apply from the next round.
fn monitor_key(cfg: &NodeMonitorSettings) -> NodeMonitorSettings {
    NodeMonitorSettings {
        enabled: cfg.enabled,
        members: cfg.members.clone(),
        interval_secs: cfg.interval_secs,
        ..NodeMonitorSettings::default()
    }
}

/// Sets `status.monitor.paused_reason`; true when it changed.
fn set_monitor_pause(inner: &Inner, reason: Option<MonitorPause>) -> bool {
    let mut st = inner.status.lock().unwrap();
    if st.monitor.paused_reason == reason {
        return false;
    }
    st.monitor.paused_reason = reason;
    true
}

async fn monitor_tick(inner: &Arc<Inner>) {
    let cfg = inner.model.lock().unwrap().settings.node_monitor.clone();
    if !cfg.enabled {
        let had_state = {
            let mut m = inner.monitor.lock().unwrap();
            let had = !m.book.is_empty() || m.settings_key.is_some() || m.last_run.is_some() || m.force;
            if had {
                *m = Default::default();
            }
            had
        };
        if set_monitor_pause(inner, None) | had_state {
            inner.publish_monitor();
        }
        return;
    }

    let specs = monitor_targets(&inner.model.lock().unwrap());
    let mut changed = inner.monitor.lock().unwrap().book.reconcile(&specs);

    let has_api = inner.live.lock().unwrap().api.is_some();
    let pause = match inner.status.lock().unwrap().last_apply {
        _ if !has_api => Some(MonitorPause::CoreNotRunning),
        Some(ApplyOutcome::Reloaded { .. }) => None,
        // The core still runs the previous config; results would be stale.
        Some(ApplyOutcome::FailedKeptPrevious { .. }) => Some(MonitorPause::ConfigNotApplied),
        _ => Some(MonitorPause::CoreNotRunning),
    };
    if let Some(reason) = pause {
        if set_monitor_pause(inner, Some(reason)) | changed {
            inner.publish_monitor();
        }
        return;
    }
    if set_monitor_pause(inner, None) {
        // The core (or its config) just came back: its nodes have not been
        // checked since, so the next round is due now.
        let mut m = inner.monitor.lock().unwrap();
        m.last_end = None;
        m.retry_at = None;
        changed = true;
    }

    let key = monitor_key(&cfg);
    let interval = Duration::from_secs(u64::from(cfg.interval_secs));
    let due = {
        let mut m = inner.monitor.lock().unwrap();
        let now = Instant::now();
        let due = m.retry_at.is_none_or(|at| now >= at)
            && (m.force
                || m.settings_key.as_ref() != Some(&key)
                || m.last_end.is_none_or(|end| now >= end + interval));
        if due && !specs.is_empty() {
            m.probing = true;
        }
        due
    };
    if !due || specs.is_empty() {
        if changed {
            inner.publish_monitor();
        }
        return;
    }
    inner.publish_monitor();

    let proxies = specs.into_iter().map(|t| t.proxy).collect();
    let results = inner.test_delays_with(proxies, MONITOR_TIMEOUT_MS).await;

    // Settings changed while probing (turned off, other members /
    // interval): the round belongs to a set that no longer exists. Drop it
    // without judging; the next tick starts over from the new settings.
    let current = inner.model.lock().unwrap().settings.node_monitor.clone();
    if !current.enabled || monitor_key(&current) != key {
        inner.monitor.lock().unwrap().probing = false;
        inner.publish_monitor();
        return;
    }

    // Windows' own online verdict tells a dead network from nodes all failing together.
    let nics = inner.nics.clone();
    let internet = tokio::task::spawn_blocking(move || nics.internet_access()).await.ok().flatten();
    let outcome = inner.monitor.lock().unwrap().book.apply_round_with(&results, &cfg, now_unix(), internet);
    let notices = match outcome {
        RoundOutcome::Applied(notices) => notices,
        RoundOutcome::Voided => {
            // The core failed mid-round or every result went stale: nothing
            // was judged, so keep the schedule (and any pending probe-now)
            // and retry shortly.
            let mut m = inner.monitor.lock().unwrap();
            m.retry_at = Some(Instant::now() + interval.min(MONITOR_VOID_RETRY));
            m.probing = false;
            drop(m);
            inner.publish_monitor();
            return;
        }
    };
    for n in notices {
        let _ = inner.events.send(super::RuntimeEvent::Notify(n)).await;
    }
    {
        let mut m = inner.monitor.lock().unwrap();
        m.last_run = Some(now_unix());
        m.force = false;
        m.settings_key = Some(key);
        m.last_end = Some(Instant::now());
        m.retry_at = None;
        m.probing = false;
    }
    inner.publish_monitor();
}

/// `/connections` every second → per-listener rates and totals.
async fn sample_traffic(inner: Arc<Inner>) {
    let mut tick = interval_at(Instant::now() + TRAFFIC_SAMPLE_EVERY, TRAFFIC_SAMPLE_EVERY);
    tick.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut last = Instant::now();
    loop {
        tick.tick().await;
        let (api, generation) = {
            let live = inner.live.lock().unwrap();
            (live.api.clone(), live.traffic_generation)
        };
        let Some(api) = api else {
            last = Instant::now();
            continue;
        };
        let Ok(snap) = api.connections().await else { continue };
        let now = Instant::now();
        let elapsed = now.duration_since(last);
        last = now;
        inner.record_traffic_sample(generation, &snap, elapsed);
    }
}

/// Follows mihomo's `/logs?level=info` stream for one core instance (aborted
/// with it) and feeds the Tailscale login tracker.
pub(super) async fn log_stream(inner: Arc<Inner>, api: Arc<dyn MihomoApi>, generation: u64) {
    loop {
        let mut stream = api.log_stream();
        while let Some(item) = stream.next().await {
            match item {
                Ok(line) => inner.on_core_log(generation, &line),
                Err(_) => break,
            }
        }
        tokio::time::sleep(STREAM_RETRY_EVERY).await;
    }
}

/// Follows mihomo's `/traffic` stream for one core instance (aborted with it).
pub(super) async fn traffic_stream(inner: Arc<Inner>, api: Arc<dyn MihomoApi>) {
    loop {
        let mut stream = api.traffic_stream();
        while let Some(item) = stream.next().await {
            match item {
                Ok((up, down)) => inner.live.lock().unwrap().stream_rate = Some(Rate { up, down }),
                Err(_) => break,
            }
        }
        inner.live.lock().unwrap().stream_rate = None;
        tokio::time::sleep(STREAM_RETRY_EVERY).await;
    }
}

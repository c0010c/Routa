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

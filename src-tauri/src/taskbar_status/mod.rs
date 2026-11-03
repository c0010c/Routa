//! Windows 11 taskbar status bar: traffic chart, rates, node and delay.

pub mod host;
pub mod paint;
pub mod view;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use tauri::{AppHandle, Emitter};

use crate::core::process::CoreState;
use crate::desktop::Desktop;
use crate::runtime::Runtime;
use host::{Host, Snapshot};
use view::{candidates, screen, History, Input, Screen};

const UPDATE_EVERY: Duration = Duration::from_secs(1);

pub struct TaskbarStatus {
    host: Arc<Host>,
}

impl TaskbarStatus {
    pub fn shutdown(&self) {
        self.host.shutdown();
    }
}

fn snapshot(rt: &Runtime, desktop: &Desktop, history: &mut History) -> Snapshot {
    let (model, status) = (rt.model(), rt.status());
    let running = status.core == CoreState::Running;
    let no_rates = HashMap::new();
    let rates = status.traffic.as_ref().map_or(&no_rates, |t| &t.per_listener_rate);
    history.record(running, rates, &model);
    let input = Input { running, delays: &status.delays, rates };
    let views = candidates(&model)
        .iter()
        .filter_map(|id| match screen(&model, &input, history, Some(id)) {
            Screen::Listener(view) => Some(view),
            _ => None,
        })
        .collect();
    let prefs = desktop.taskbar_status();
    Snapshot { running, views, mode: prefs.mode, traffic: prefs.traffic }
}

/// `None` where the bar is unsupported (below Windows 11).
pub fn spawn(app: &AppHandle, rt: Arc<Runtime>, desktop: Arc<Desktop>) -> Option<TaskbarStatus> {
    let (close_app, close_desktop) = (app.clone(), desktop.clone());
    let host = Arc::new(Host::spawn(
        Box::new(move || {
            let (app, desktop) = (close_app.clone(), close_desktop.clone());
            tauri::async_runtime::spawn(async move {
                match desktop.disable_taskbar_status().await {
                    Ok(_) => {
                        let _ = app.emit("ui-preferences-changed", ());
                    }
                    Err(e) => eprintln!("taskbar status: {e}"),
                }
            });
        }),
    )?);
    let feed = host.clone();
    tauri::async_runtime::spawn(async move {
        let mut history = History::default();
        let mut tick = tokio::time::interval(UPDATE_EVERY);
        loop {
            tick.tick().await;
            let enabled = desktop.taskbar_status().enabled;
            feed.set_visible(enabled);
            if enabled {
                feed.update(snapshot(&rt, &desktop, &mut history));
            } else {
                history = History::default();
            }
        }
    });
    Some(TaskbarStatus { host })
}

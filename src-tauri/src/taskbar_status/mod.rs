//! Windows 11 taskbar status bar: traffic chart, rates, node and delay.

pub mod host;
pub mod paint;
pub mod view;

use tauri::{AppHandle, Manager};

pub struct TaskbarStatus {
    host: host::Host,
}

impl TaskbarStatus {
    pub fn shutdown(&self) {
        self.host.shutdown();
    }
}

pub fn spawn(app: &AppHandle) {
    let left = app.clone();
    let host = host::Host::spawn(
        Box::new(move || crate::tray::show_main_window(&left)),
        Box::new(|| eprintln!("taskbar status: close requested")),
    );
    if let Some(host) = host {
        app.manage(TaskbarStatus { host });
    }
}

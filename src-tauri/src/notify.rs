//! Native toast for monitor notices; clicking it brings the main window back
//! and tells the frontend which node/listener to focus.

use tauri::AppHandle;

use crate::monitor::MonitorNotice;

/// Shows `notice` as a toast without blocking the caller.
#[cfg(windows)]
pub fn show(app: &AppHandle, notice: MonitorNotice) {
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || show_blocking(&app, &notice));
}

#[cfg(not(windows))]
pub fn show(_app: &AppHandle, _notice: MonitorNotice) {}

#[cfg(windows)]
fn show_blocking(app: &AppHandle, notice: &MonitorNotice) {
    use tauri::Emitter;
    use tauri_winrt_notification::Toast;

    let try_show = |app_id: &str| {
        let handle = app.clone();
        let focus = notice.focus.clone();
        Toast::new(app_id)
            .title(&notice.title)
            .text1(&notice.body)
            // Runs on a WinRT thread, only while Routa is alive.
            .on_activated(move |_| {
                crate::tray::show_main_window(&handle);
                if let Err(e) = handle.emit("monitor-focus", focus.clone()) {
                    eprintln!("emit monitor-focus failed: {e}");
                }
                Ok(())
            })
            .show()
    };

    let id = app.config().identifier.clone();
    // Dev builds have no registered AppUserModelID; PowerShell's is the fallback.
    let first = if cfg!(debug_assertions) { Toast::POWERSHELL_APP_ID } else { id.as_str() };
    if let Err(e) = try_show(first) {
        eprintln!("toast with app id {first} failed: {e}");
        if first != Toast::POWERSHELL_APP_ID {
            if let Err(e) = try_show(Toast::POWERSHELL_APP_ID) {
                eprintln!("toast fallback failed: {e}");
            }
        }
    }
}

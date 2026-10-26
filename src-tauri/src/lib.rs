pub mod commands;
pub mod config_gen;
pub mod core;
pub mod model;
pub mod netif;
pub mod runtime;
pub mod scheduler;
pub mod stats;
pub mod store;
pub mod subscription;
pub mod tray;

use std::path::PathBuf;
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::Duration;
use std::sync::Arc;

use tauri::{Emitter, Manager, RunEvent, WindowEvent};
use tauri_specta::{collect_commands, Builder};
use tokio::sync::mpsc;

use crate::core::api::{HttpMihomoApi, MihomoApi};
use crate::core::ports::WindowsPortOwners;
use crate::core::process::{CoreLauncher, ProcessLauncher};
use crate::model::Settings;
use crate::netif::WindowsInterfaces;
use crate::runtime::{ApiFor, LauncherFor, Runtime, RuntimeEvent};
use crate::store::Store;

/// The bundled mihomo: next to the exe in production (`mihomo.exe`), the
/// binaries folder of the source tree in debug builds.
fn sidecar_path() -> PathBuf {
    if let Some(p) = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(|d| d.join("mihomo.exe")))
        .filter(|p| p.exists())
    {
        return p;
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("binaries/mihomo-x86_64-pc-windows-msvc.exe")
}

fn build_runtime(app: &tauri::AppHandle) -> Result<Arc<Runtime>, Box<dyn std::error::Error>> {
    let store = Store::open(app.path().app_data_dir()?)?;
    let workdir = store.core_workdir();
    let launcher_for: LauncherFor = Arc::new(move |s: &Settings| {
        let bin = s
            .core_path
            .as_deref()
            .filter(|p| !p.trim().is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(sidecar_path);
        Arc::new(ProcessLauncher { bin, workdir: workdir.clone() }) as Arc<dyn CoreLauncher>
    });
    let api_for: ApiFor =
        Box::new(|port, secret| Arc::new(HttpMihomoApi::new(port, secret)) as Arc<dyn MihomoApi>);
    let (tx, mut rx) = mpsc::channel::<RuntimeEvent>(1024);
    let rt = Arc::new(Runtime::new(
        store,
        Arc::new(WindowsInterfaces),
        launcher_for,
        api_for,
        Arc::new(WindowsPortOwners),
        tx,
    ));
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        while let Some(ev) = rx.recv().await {
            let res = match ev {
                RuntimeEvent::Status(s) => handle.emit("runtime-status", s),
                RuntimeEvent::Traffic(t) => handle.emit("traffic", t),
            };
            if let Err(e) = res {
                eprintln!("emit event failed: {e}");
            }
        }
    });
    Ok(rt)
}

pub fn run() {
    let builder = Builder::<tauri::Wry>::new().commands(collect_commands![
        commands::get_model,
        commands::get_status,
        commands::save_listener,
        commands::delete_listener,
        commands::save_exit,
        commands::delete_exit,
        commands::save_subscription,
        commands::delete_subscription,
        commands::refresh_subscription,
        commands::select_group_member,
        commands::test_delays,
        commands::save_settings,
        commands::reset_traffic,
        commands::core_start,
        commands::core_stop,
        commands::core_restart,
        commands::open_log_dir,
    ]);

    #[cfg(debug_assertions)]
    builder
        .export(
            // i64/u64 values (timestamps, byte counters) stay far below 2^53.
            specta_typescript::Typescript::default()
                .bigint(specta_typescript::BigIntExportBehavior::Number)
                .header("// @ts-nocheck"),
            concat!(env!("CARGO_MANIFEST_DIR"), "/../src/bindings.ts"),
        )
        .expect("failed to export typescript bindings");

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
                let _ = w.unminimize();
                let _ = w.set_focus();
            }
        }))
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .invoke_handler(builder.invoke_handler())
        .setup(|app| {
            let rt = build_runtime(app.handle())?;
            app.manage(rt.clone());
            tray::setup(app.handle())?;
            tauri::async_runtime::spawn(async move {
                match rt.start().await {
                    Ok(()) => eprintln!("runtime: started"),
                    Err(e) => eprintln!("runtime: start failed: {e}"),
                }
            });
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    // Shutdown (stop mihomo, persist totals) must run exactly once on any
    // exit path and finish before the process goes away. State 0 = running,
    // 1 = shutdown in progress (every exit request is prevented), 2 = done
    // (set right before our own app.exit; requests then pass). RunEvent::Exit
    // is the fallback for paths that skip ExitRequested.
    let state = Arc::new(AtomicU8::new(0));
    app.run(move |app, event| match event {
        RunEvent::ExitRequested { api, .. } => {
            if state.load(Ordering::SeqCst) == 2 {
                return;
            }
            api.prevent_exit();
            if state.compare_exchange(0, 1, Ordering::SeqCst, Ordering::SeqCst).is_err() {
                return; // already shutting down
            }
            let app = app.clone();
            let state = state.clone();
            tauri::async_runtime::spawn(async move {
                if let Some(rt) = app.try_state::<Arc<Runtime>>() {
                    let rt = Arc::clone(&*rt);
                    // Spawned so a panic is caught; bounded so exit cannot hang.
                    let job = tauri::async_runtime::spawn(async move { rt.shutdown().await });
                    match tokio::time::timeout(Duration::from_secs(10), job).await {
                        Ok(Ok(())) => {}
                        Ok(Err(e)) => eprintln!("shutdown failed: {e}"),
                        Err(_) => eprintln!("shutdown timed out after 10s"),
                    }
                }
                state.store(2, Ordering::SeqCst);
                app.exit(0);
            });
        }
        RunEvent::Exit => {
            if state.swap(2, Ordering::SeqCst) == 0 {
                if let Some(rt) = app.try_state::<Arc<Runtime>>() {
                    tauri::async_runtime::block_on(rt.shutdown());
                }
            }
        }
        _ => {}
    });
}

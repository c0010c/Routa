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
use std::sync::atomic::{AtomicBool, Ordering};
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
        .plugin(tauri_plugin_shell::init())
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
    // exit path and finish before the process goes away. ExitRequested runs
    // it asynchronously then re-issues the exit; Exit is the fallback for
    // paths that skip ExitRequested.
    let shut_down = Arc::new(AtomicBool::new(false));
    app.run(move |app, event| match event {
        RunEvent::ExitRequested { api, .. } => {
            if shut_down.swap(true, Ordering::SeqCst) {
                return; // second request (ours): let it through
            }
            api.prevent_exit();
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                if let Some(rt) = app.try_state::<Arc<Runtime>>() {
                    rt.shutdown().await;
                }
                app.exit(0);
            });
        }
        RunEvent::Exit => {
            if !shut_down.swap(true, Ordering::SeqCst) {
                if let Some(rt) = app.try_state::<Arc<Runtime>>() {
                    tauri::async_runtime::block_on(rt.shutdown());
                }
            }
        }
        _ => {}
    });
}

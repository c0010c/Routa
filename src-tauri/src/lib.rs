pub mod commands;
pub mod config_gen;
pub mod core;
pub mod desktop;
pub mod model;
pub mod netif;
pub mod portable;
pub mod procroute;
pub mod runtime;
pub mod scheduler;
pub mod stats;
pub mod store;
pub mod subscription;
pub mod system_proxy;
pub mod tailnet;
pub mod tailscale;
pub mod tray;

use std::path::PathBuf;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use std::time::Duration;

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

/// Startup errors in a release build have no console, so show them in a box.
#[cfg(windows)]
fn fatal_dialog(msg: &str) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};
    let wide = |s: &str| s.encode_utf16().chain(Some(0)).collect::<Vec<u16>>();
    let (text, caption) = (wide(msg), wide("Routa"));
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            text.as_ptr(),
            caption.as_ptr(),
            MB_OK | MB_ICONERROR,
        )
    };
}

/// `Some(data dir)` in portable mode, after making sure it is writable.
fn portable_data_dir() -> Result<Option<PathBuf>, String> {
    let Some(dir) = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().and_then(portable::data_dir_for))
    else {
        return Ok(None);
    };
    portable::prepare(&dir).map_err(|e| {
        format!(
            "便携模式无法写入数据目录：\n{}\n\n{e}\n\n请把程序放到可写的位置，或删除 exe 旁边的 `{}` 文件改用普通模式。",
            dir.display(),
            portable::MARKER
        )
    })?;
    Ok(Some(dir))
}

/// The main window is created here rather than from tauri.conf.json so that
/// portable mode can point the WebView2 profile next to the exe.
fn create_main_window(app: &tauri::App, portable: Option<&std::path::Path>) -> tauri::Result<()> {
    let config = app
        .config()
        .app
        .windows
        .iter()
        .find(|w| w.label == "main")
        .expect("main window missing from tauri.conf.json")
        .clone();
    let mut builder = tauri::WebviewWindowBuilder::from_config(app.handle(), &config)?;
    if let Some(dir) = portable {
        builder = builder.data_directory(portable::webview_dir(dir));
    }
    builder.build()?;
    Ok(())
}

fn build_runtime(app: &tauri::AppHandle, data_dir: PathBuf) -> Result<Arc<Runtime>, Box<dyn std::error::Error>> {
    let store = Store::open(data_dir)?;
    let workdir = store.core_workdir();
    let launcher_for: LauncherFor = Arc::new(move |_s: &Settings| {
        Arc::new(ProcessLauncher {
            bin: sidecar_path(),
            workdir: workdir.clone(),
        }) as Arc<dyn CoreLauncher>
    });
    let api_for: ApiFor = Box::new(|port, secret| Arc::new(HttpMihomoApi::new(port, secret)) as Arc<dyn MihomoApi>);
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
                RuntimeEvent::Status(s) => {
                    procroute::client::runtime_changed();
                    handle.emit("runtime-status", s)
                }
                RuntimeEvent::Traffic(t) => handle.emit("traffic", t),
                RuntimeEvent::TrafficSnapshot(t) => handle.emit("traffic-snapshot", t),
            };
            if let Err(e) = res {
                eprintln!("emit event failed: {e}");
            }
        }
    });
    Ok(rt)
}

/// Commands and extra types exported to `src/bindings.ts`.
fn specta_builder() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new()
        .commands(collect_commands![
            commands::get_model,
            commands::get_status,
            commands::save_listener,
            commands::save_proxy,
            commands::delete_listener,
            commands::save_exit,
            commands::delete_exit,
            commands::save_subscription,
            commands::delete_subscription,
            commands::refresh_subscription,
            commands::select_group_member,
            commands::test_delays,
            commands::save_settings,
            commands::save_manual_node,
            commands::delete_manual_node,
            commands::save_node_group,
            commands::delete_node_group,
            commands::parse_proxy_yaml,
            commands::parse_proxy_yaml_batch,
            commands::save_manual_nodes,
            commands::group_now,
            commands::reset_traffic,
            commands::core_start,
            commands::core_stop,
            commands::core_restart,
            commands::open_log_dir,
            commands::tailnet_peers,
            commands::open_login_url,
            commands::retry_tailscale_exit_nodes,
            commands::save_process_rule,
            commands::delete_process_rule,
            commands::save_process_routing_settings,
            commands::procroute_status,
            commands::procroute_retry,
            commands::procroute_install,
            commands::procroute_uninstall,
            commands::procroute_cleanup,
            commands::list_running_processes,
            desktop::desktop_state,
            desktop::save_ui_preferences,
            desktop::bind_default_listener,
            desktop::select_simple_node,
            desktop::set_desktop_connection,
            desktop::set_system_proxy_listener,
            desktop::save_simple_app,
            desktop::set_simple_port,
            desktop::change_process_listener,
            desktop::get_connection_details,
            desktop::read_core_log,
        ])
        // Payload of the "procroute-flows" event (not returned by any command).
        .typ::<routa_procsvc_proto::Flow>()
}

/// Writes `src/bindings.ts` (debug builds do this at every start).
#[cfg(debug_assertions)]
fn export_bindings(builder: &Builder<tauri::Wry>) {
    builder
        .export(
            // i64/u64 values (timestamps, byte counters) stay far below 2^53.
            specta_typescript::Typescript::default()
                .bigint(specta_typescript::BigIntExportBehavior::Number)
                .header("// @ts-nocheck"),
            concat!(env!("CARGO_MANIFEST_DIR"), "/../src/bindings.ts"),
        )
        .expect("failed to export typescript bindings");
}

pub fn run() {
    let portable_dir = match portable_data_dir() {
        Ok(dir) => dir,
        Err(msg) => {
            eprintln!("{msg}");
            #[cfg(windows)]
            fatal_dialog(&msg);
            std::process::exit(1);
        }
    };

    let builder = specta_builder();
    #[cfg(debug_assertions)]
    {
        export_bindings(&builder);
        // `routa --export-bindings`: regenerate the bindings without starting the app.
        if std::env::args().any(|a| a == "--export-bindings") {
            return;
        }
    }

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
        .setup(move |app| {
            create_main_window(app, portable_dir.as_deref())?;
            let data_dir = match portable_dir {
                Some(dir) => dir,
                None => app.path().app_data_dir()?,
            };
            let desktop = Arc::new(desktop::Desktop::open(data_dir.clone()).map_err(std::io::Error::other)?);
            let rt = build_runtime(app.handle(), data_dir)?;
            app.manage(desktop.clone());
            app.manage(rt.clone());
            tray::setup(app.handle())?;
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = desktop.initialize(&rt).await {
                    eprintln!("default proxy initialization: {e}");
                }
                match rt.start().await {
                    Ok(()) => eprintln!("runtime: started"),
                    Err(e) => eprintln!("runtime: start failed: {e}"),
                }
                // Only now: before the first start the core reads as Stopped
                // (no reason), which would push Block to every rule. Until
                // then the service keeps its own "Routa absent" fallback.
                procroute::client::spawn(rt.clone(), handle);
                desktop.clone().spawn_monitor(rt.clone());
                let preferences = desktop.state(&rt).preferences;
                if preferences.auto_connect {
                    let result = if preferences.mode == Some(desktop::InterfaceMode::Full)
                        && preferences.capture_mode == desktop::CaptureMode::System
                    {
                        if let Some(id) = preferences.system_listener_id {
                            desktop.system_for_listener(&rt, id).await
                        } else {
                            desktop.connect(&rt, Some(preferences.capture_mode)).await
                        }
                    } else {
                        desktop.connect(&rt, Some(preferences.capture_mode)).await
                    };
                    if let Err(e) = result {
                        eprintln!("auto connect: {e}");
                    }
                }
            });
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window
                    .app_handle()
                    .try_state::<Arc<desktop::Desktop>>()
                    .is_none_or(|d| d.close_to_tray())
                {
                    api.prevent_close();
                    let _ = window.hide();
                } else {
                    api.prevent_close();
                    window.app_handle().exit(0);
                }
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
            if state
                .compare_exchange(0, 1, Ordering::SeqCst, Ordering::SeqCst)
                .is_err()
            {
                return; // already shutting down
            }
            let app = app.clone();
            let state = state.clone();
            tauri::async_runtime::spawn(async move {
                // Before the core stops: tells the service this is a normal
                // exit (on_routa_exit applies). Bounded at 1s inside.
                procroute::client::goodbye().await;
                if let Some(rt) = app.try_state::<Arc<Runtime>>() {
                    let rt = Arc::clone(&*rt);
                    if let Some(dt) = app.try_state::<Arc<desktop::Desktop>>() {
                        dt.shutdown(&rt).await;
                    }
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
                tauri::async_runtime::block_on(procroute::client::goodbye());
                if let Some(rt) = app.try_state::<Arc<Runtime>>() {
                    if let Some(dt) = app.try_state::<Arc<desktop::Desktop>>() {
                        tauri::async_runtime::block_on(dt.shutdown(&rt));
                    }
                    tauri::async_runtime::block_on(rt.shutdown());
                }
            }
        }
        _ => {}
    });
}

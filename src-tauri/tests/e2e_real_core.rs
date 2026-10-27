//! End-to-end test against the real mihomo binary.
//!
//! Run: MIHOMO_BIN=<path to mihomo exe> cargo test --test e2e_real_core -- --ignored --nocapture

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use mihomo_mapper_lib::core::api::{HttpMihomoApi, MihomoApi};
use mihomo_mapper_lib::core::ports::WindowsPortOwners;
use mihomo_mapper_lib::core::process::{CoreLauncher, CoreState, ProcessLauncher};
use mihomo_mapper_lib::model::{Exit, ExitId, ExitKind, Listener, Model, Settings};
use mihomo_mapper_lib::netif::WindowsInterfaces;
use mihomo_mapper_lib::runtime::{ApiFor, LauncherFor, Runtime};
use mihomo_mapper_lib::store::Store;
use tokio::sync::mpsc;

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
}

fn refuses(port: u16) -> bool {
    TcpStream::connect_timeout(&format!("127.0.0.1:{port}").parse().unwrap(), Duration::from_secs(2)).is_err()
}

/// Tiny HTTP server: answers every request with 204 after `delay`.
fn spawn_target(delay: Duration) -> u16 {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            std::thread::spawn(move || {
                let mut s = s;
                let mut buf = [0u8; 2048];
                let _ = s.read(&mut buf);
                std::thread::sleep(delay);
                let _ = s.write_all(b"HTTP/1.1 204 No Content\r\nConnection: close\r\n\r\n");
            });
        }
    });
    port
}

fn curl_status(proxy_port: u16, url: &str) -> String {
    let out = Command::new("curl.exe")
        .args(["-s", "-o", "NUL", "-w", "%{http_code}", "-m", "15", "-x"])
        .arg(format!("http://127.0.0.1:{proxy_port}"))
        .arg(url)
        .output()
        .expect("curl.exe");
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn ps_count(marker: &str) -> usize {
    // Matches only mihomo processes whose command line mentions our temp workdir.
    let action = "Measure-Object | ForEach-Object { $_.Count }";
    let out = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            &format!(
                "Get-CimInstance Win32_Process | Where-Object {{ $_.CommandLine -like '*{}*' -and $_.Name -like '*mihomo*' }} | {}",
                marker.replace('\'', ""),
                action
            ),
        ])
        .output()
        .expect("powershell");
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    text.parse::<usize>().unwrap_or_else(|_| {
        panic!("could not parse process count {text:?}; stderr: {}", String::from_utf8_lossy(&out.stderr))
    })
}

fn mihomo_running(marker: &str) -> bool {
    ps_count(marker) > 0
}

/// Kills the test's own mihomo (matched by temp-dir marker) even when an
/// assertion fails before `shutdown` runs.
struct KillOnDrop(String);

impl Drop for KillOnDrop {
    fn drop(&mut self) {
        let _ = Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                &format!(
                    "Get-CimInstance Win32_Process | Where-Object {{ $_.CommandLine -like '*{}*' -and $_.Name -like '*mihomo*' }} | ForEach-Object {{ Stop-Process -Id $_.ProcessId -Force }}",
                    self.0.replace('\'', "")
                ),
            ])
            .output();
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore]
async fn real_core_two_listeners_direct_exit() {
    let bin = PathBuf::from(std::env::var("MIHOMO_BIN").expect("set MIHOMO_BIN"));
    assert!(bin.exists(), "MIHOMO_BIN not found: {bin:?}");

    let tmp = tempfile::tempdir().unwrap();
    let store = Store::open(tmp.path().to_path_buf()).unwrap();
    let workdir = store.core_workdir();
    let marker = tmp.path().file_name().unwrap().to_string_lossy().to_string();
    let _guard = KillOnDrop(marker.clone());

    // Model: one Direct exit, two listeners, free ports (controller included).
    let (p1, p2, ctrl) = (free_port(), free_port(), free_port());
    let exit = Exit { id: ExitId::new(), name: "直连".into(), kind: ExitKind::Direct, interface: None };
    let mut l1 = Listener::new_default("监听一".into(), p1);
    l1.exit = Some(exit.id.clone());
    let mut l2 = Listener::new_default("监听二".into(), p2);
    l2.exit = Some(exit.id.clone());
    let l2_id = l2.id.clone();
    let model = Model {
        subscriptions: vec![],
        exits: vec![exit],
        listeners: vec![l1, l2],
        settings: Settings { controller_port: ctrl, ..Settings::default() },
        ..Model::default()
    };
    store.save_model(&model).unwrap();

    let wd = workdir.clone();
    let launcher_for: LauncherFor = Arc::new(move |_s: &Settings| {
        Arc::new(ProcessLauncher { bin: bin.clone(), workdir: wd.clone() }) as Arc<dyn CoreLauncher>
    });
    // Capture the per-run secret so the test can talk to /connections itself.
    let api_slot: Arc<Mutex<Option<Arc<HttpMihomoApi>>>> = Arc::new(Mutex::new(None));
    let slot = api_slot.clone();
    let api_for: ApiFor = Box::new(move |port, secret| {
        let api = Arc::new(HttpMihomoApi::new(port, secret));
        *slot.lock().unwrap() = Some(api.clone());
        api as Arc<dyn MihomoApi>
    });
    let (tx, mut rx) = mpsc::channel(1024);
    tokio::spawn(async move { while rx.recv().await.is_some() {} });
    let rt = Arc::new(Runtime::new(
        store,
        Arc::new(WindowsInterfaces),
        launcher_for,
        api_for,
        Arc::new(WindowsPortOwners),
        tx,
    ));
    rt.start().await.expect("runtime start");
    assert_eq!(rt.status().core, CoreState::Running);
    assert!(mihomo_running(&marker), "mihomo should be running");
    let api = api_slot.lock().unwrap().clone().expect("api created");

    // Both listeners must come up (no failed listeners reported).
    assert!(rt.status().failed_listeners.is_empty(), "{:?}", rt.status().failed_listeners);

    // 1. Traffic through listener 1: real internet if reachable, else local target.
    let real = tokio::task::spawn_blocking(move || curl_status(p1, "https://www.gstatic.com/generate_204"))
        .await
        .unwrap();
    if real == "204" {
        println!("gstatic via listener 1 -> 204 (real internet)");
    } else {
        println!("gstatic unreachable (curl status {real:?}); falling back to a local HTTP target");
        let t = spawn_target(Duration::ZERO);
        let code = tokio::task::spawn_blocking(move || curl_status(p1, &format!("http://127.0.0.1:{t}/")))
            .await
            .unwrap();
        assert_eq!(code, "204", "local target via listener 1");
    }
    // Listener 2 works too.
    let t = spawn_target(Duration::ZERO);
    let code = tokio::task::spawn_blocking(move || curl_status(p2, &format!("http://127.0.0.1:{t}/")))
        .await
        .unwrap();
    assert_eq!(code, "204", "local target via listener 2");

    // 2. /connections during a slow request shows inboundName == listener name.
    let slow = spawn_target(Duration::from_secs(4));
    let h = tokio::task::spawn_blocking(move || curl_status(p1, &format!("http://127.0.0.1:{slow}/")));
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut seen = false;
    while Instant::now() < deadline && !seen {
        tokio::time::sleep(Duration::from_millis(200)).await;
        let snap = api.connections().await.expect("connections");
        seen = snap.connections.iter().any(|c| c.metadata.inbound_name == "监听一");
    }
    assert!(seen, "no connection with inboundName 监听一 seen");
    println!("/connections showed inboundName=监听一");
    assert_eq!(h.await.unwrap(), "204");

    // 3. Disable listener 2 -> its port refuses connections; listener 1 still works.
    assert!(!refuses(p2));
    rt.mutate(|m| {
        m.listeners.iter_mut().find(|l| l.id == l2_id).unwrap().user_enabled = false;
    })
    .await
    .expect("mutate");
    assert!(refuses(p2), "disabled listener port should refuse connections");
    assert!(!refuses(p1));
    println!("listener 2 disabled -> port {p2} refuses connections");

    // 4. Shutdown -> mihomo process gone, listener 1 closed.
    rt.shutdown().await;
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(!mihomo_running(&marker), "mihomo should be gone after shutdown");
    assert!(refuses(p1));
    println!("shutdown -> mihomo process gone");
}

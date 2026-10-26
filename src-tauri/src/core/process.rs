//! Spawning and supervising the mihomo child process.

use std::io;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};

use super::api::{ApiError, MihomoApi};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreState {
    Stopped,
    Starting,
    Running,
    Crashed { exit_code: Option<i32> },
}

/// 1s, 2s, 4s ... capped at 30s.
pub fn backoff_delay(attempt: u32) -> Duration {
    Duration::from_secs(1u64 << attempt.min(5)).min(Duration::from_secs(30))
}

#[async_trait]
pub trait CoreHandleLike {
    async fn stop(&mut self) -> io::Result<()>;
    /// `Some(code)` once the process has exited (`code` is `None` if killed by
    /// a signal), `None` while it still runs.
    fn try_wait(&mut self) -> Option<Option<i32>>;
    fn pid(&self) -> u32;
}

#[async_trait]
pub trait CoreLauncher: Send + Sync {
    async fn launch(&self, config: &Path) -> io::Result<Box<dyn CoreHandleLike + Send>>;
}

#[derive(Debug)]
pub struct CoreProcess {
    child: Child,
    pid: u32,
}

/// Copies lines from a child pipe into the shared log file.
fn pipe_to_log<R: AsyncRead + Unpin + Send + 'static>(reader: R, log: PathBuf) {
    tokio::spawn(async move {
        let Ok(mut file) = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log)
            .await
        else {
            return;
        };
        let mut lines = BufReader::new(reader).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            if file
                .write_all(format!("{line}\n").as_bytes())
                .await
                .is_err()
            {
                break;
            }
        }
    });
}

impl CoreProcess {
    /// Runs `bin -d workdir -f config`. `config` must be inside `workdir`
    /// (spike S5). Does not wait for the API; see [`wait_ready`].
    pub async fn start(bin: &Path, workdir: &Path, config: &Path) -> io::Result<Self> {
        let mut cmd = Command::new(bin);
        cmd.arg("-d")
            .arg(workdir)
            .arg("-f")
            .arg(config)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        #[cfg(windows)]
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        let mut child = cmd.spawn()?;
        let pid = child
            .id()
            .ok_or_else(|| io::Error::other("mihomo exited immediately"))?;
        let log = workdir.join("mihomo.log");
        if let Some(out) = child.stdout.take() {
            pipe_to_log(out, log.clone());
        }
        if let Some(err) = child.stderr.take() {
            pipe_to_log(err, log);
        }
        Ok(Self { child, pid })
    }

    pub async fn stop(&mut self) -> io::Result<()> {
        // Killing an already-exited child is fine; wait() reaps it either way.
        let _ = self.child.start_kill();
        self.child.wait().await.map(|_| ())
    }

    pub fn try_wait(&mut self) -> Option<Option<i32>> {
        match self.child.try_wait() {
            Ok(Some(status)) => Some(status.code()),
            Ok(None) => None,
            // Cannot query: treat as gone (fail closed).
            Err(_) => Some(None),
        }
    }

    pub fn pid(&self) -> u32 {
        self.pid
    }
}

#[async_trait]
impl CoreHandleLike for CoreProcess {
    async fn stop(&mut self) -> io::Result<()> {
        CoreProcess::stop(self).await
    }
    fn try_wait(&mut self) -> Option<Option<i32>> {
        CoreProcess::try_wait(self)
    }
    fn pid(&self) -> u32 {
        self.pid
    }
}

/// Production launcher; `bin` is `settings.core_path` or the sidecar.
#[derive(Debug, Clone)]
pub struct ProcessLauncher {
    pub bin: PathBuf,
    pub workdir: PathBuf,
}

#[async_trait]
impl CoreLauncher for ProcessLauncher {
    async fn launch(&self, config: &Path) -> io::Result<Box<dyn CoreHandleLike + Send>> {
        let p = CoreProcess::start(&self.bin, &self.workdir, config).await?;
        Ok(Box::new(p))
    }
}

/// Polls `version()` every 100 ms until it answers or `timeout` passes.
/// Callers use 5 s; a timeout is a start failure.
pub async fn wait_ready(api: &dyn MihomoApi, timeout: Duration) -> Result<String, ApiError> {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        match api.version().await {
            Ok(v) => return Ok(v),
            Err(e) if tokio::time::Instant::now() >= deadline => return Err(e),
            Err(_) => tokio::time::sleep(Duration::from_millis(100)).await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::api::{ConnectionsSnapshot, HttpMihomoApi};
    use futures_util::stream::BoxStream;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[test]
    fn backoff_sequence() {
        let s = |n| backoff_delay(n).as_secs();
        assert_eq!((s(0), s(1), s(2), s(3), s(4)), (1, 2, 4, 8, 16));
        assert_eq!((s(5), s(10), s(u32::MAX)), (30, 30, 30));
    }

    struct FlakyApi(AtomicU32, u32);

    #[async_trait]
    impl MihomoApi for FlakyApi {
        async fn version(&self) -> Result<String, ApiError> {
            if self.0.fetch_add(1, Ordering::SeqCst) + 1 >= self.1 {
                Ok("v1".into())
            } else {
                Err(ApiError::Unreachable("down".into()))
            }
        }
        async fn reload(&self, _: &Path) -> Result<(), ApiError> {
            unimplemented!()
        }
        async fn connections(&self) -> Result<ConnectionsSnapshot, ApiError> {
            unimplemented!()
        }
        async fn close_connection(&self, _: &str) -> Result<(), ApiError> {
            unimplemented!()
        }
        async fn delay(&self, _: &str, _: &str, _: u32) -> Result<u32, ApiError> {
            unimplemented!()
        }
        async fn select(&self, _: &str, _: &str) -> Result<(), ApiError> {
            unimplemented!()
        }
        fn traffic_stream(&self) -> BoxStream<'static, Result<(u64, u64), ApiError>> {
            unimplemented!()
        }
    }

    #[tokio::test]
    async fn wait_ready_polls_until_ok() {
        let api = FlakyApi(AtomicU32::new(0), 3);
        let v = wait_ready(&api, Duration::from_secs(5)).await.unwrap();
        assert_eq!(v, "v1");
        assert_eq!(api.0.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn wait_ready_times_out_with_last_error() {
        let api = FlakyApi(AtomicU32::new(0), u32::MAX);
        let err = wait_ready(&api, Duration::from_millis(250))
            .await
            .unwrap_err();
        assert_eq!(err, ApiError::Unreachable("down".into()));
    }

    #[tokio::test]
    async fn start_missing_binary_is_io_error() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = dir.path().join("config.yaml");
        let r =
            CoreProcess::start(Path::new("definitely-not-a-binary.exe"), dir.path(), &cfg).await;
        assert!(r.is_err());
    }

    #[tokio::test]
    #[ignore = "needs a real mihomo binary: set MIHOMO_BIN"]
    async fn start_real_core() {
        let bin = PathBuf::from(std::env::var("MIHOMO_BIN").expect("MIHOMO_BIN"));
        let port = {
            let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            l.local_addr().unwrap().port()
        };
        let dir = tempfile::tempdir().unwrap();
        let cfg = dir.path().join("config.yaml");
        std::fs::write(&cfg, crate::config_gen::empty_config(port, "test-secret")).unwrap();

        let mut core = CoreProcess::start(&bin, dir.path(), &cfg).await.unwrap();
        let pid = core.pid();
        let api = HttpMihomoApi::new(port, "test-secret".into());
        let version = wait_ready(&api, Duration::from_secs(5)).await.unwrap();
        println!("mihomo pid={pid} version={version}");
        assert!(core.try_wait().is_none());

        core.stop().await.unwrap();
        assert!(core.try_wait().is_some());
        assert!(
            api.version().await.is_err(),
            "API still answering after stop"
        );
        // Let the log copier drain, then check the log was written.
        tokio::time::sleep(Duration::from_millis(300)).await;
        let log = std::fs::read_to_string(dir.path().join("mihomo.log")).unwrap();
        assert!(!log.is_empty());
    }
}

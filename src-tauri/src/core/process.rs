//! Spawning and supervising the mihomo child process.

use std::io;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};

use super::api::{ApiError, MihomoApi};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, specta::Type)]
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

/// Why `CoreLauncher::check_config` did not pass a config.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckError {
    /// mihomo ran and rejected the config; the text is from its output.
    Rejected(String),
    /// The check itself could not run (binary missing, timeout, ...).
    Unavailable(String),
}

#[async_trait]
pub trait CoreLauncher: Send + Sync {
    async fn launch(&self, config: &Path) -> io::Result<Box<dyn CoreHandleLike + Send>>;
    /// Validates `config` (inside the core workdir) with the real core
    /// (`mihomo -t`) without starting it.
    async fn check_config(&self, config: &Path) -> Result<(), CheckError>;
    /// The mihomo binary this launcher runs, when it is a real file.
    fn core_binary(&self) -> Option<PathBuf> {
        None
    }
}

const CHECK_TIMEOUT: Duration = Duration::from_secs(15);
const CHECK_MESSAGE_MAX_CHARS: usize = 500;

/// Replaces every `scheme://...` token (it may carry a subscription token).
fn redact_urls(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(i) = rest.find("://") {
        let start = rest[..i]
            .char_indices()
            .rev()
            .take_while(|(_, c)| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
            .last()
            .map_or(i, |(j, _)| j);
        let tail = &rest[i + 3..];
        let end = tail
            .find(|c: char| c.is_whitespace() || matches!(c, '"' | '\'' | ')' | ']' | '>' | ','))
            .unwrap_or(tail.len());
        out.push_str(&rest[..start]);
        out.push_str("<URL>");
        rest = &tail[end..];
    }
    out.push_str(rest);
    out
}

/// The useful part of a failed `mihomo -t`: the `msg` of its error lines,
/// else every other non-empty line except the "configuration file <path>
/// test failed" trailer. URLs redacted, length bounded.
fn check_failure_message(output: &str, code: Option<i32>) -> String {
    let msg_of = |line: &str| -> String {
        match line.find("msg=\"") {
            Some(i) => {
                let m = &line[i + 5..];
                m.strip_suffix('"').unwrap_or(m).replace("\\\"", "\"")
            }
            None => line.to_string(),
        }
    };
    let lines: Vec<&str> = output.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    let errors: Vec<String> = lines
        .iter()
        .filter(|l| l.contains("level=error") || l.contains("level=fatal"))
        .map(|l| msg_of(l))
        .collect();
    let picked = if errors.is_empty() {
        lines
            .iter()
            .filter(|l| !l.starts_with("configuration file") && !l.contains("level=info"))
            .map(|l| msg_of(l))
            .collect()
    } else {
        errors
    };
    let text = redact_urls(&picked.join("; "));
    if text.is_empty() {
        return match code {
            Some(c) => format!("mihomo -t 失败（退出码 {c}）"),
            None => "mihomo -t 失败".to_string(),
        };
    }
    if text.chars().count() > CHECK_MESSAGE_MAX_CHARS {
        let cut: String = text.chars().take(CHECK_MESSAGE_MAX_CHARS).collect();
        return format!("{cut}…");
    }
    text
}

/// Runs `bin -t -d workdir -f config` (no window, killed after 15 s).
pub async fn check_config_with(bin: &Path, workdir: &Path, config: &Path) -> Result<(), CheckError> {
    let mut cmd = Command::new(bin);
    cmd.arg("-t")
        .arg("-d")
        .arg(workdir)
        .arg("-f")
        .arg(config)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    let child = cmd
        .spawn()
        .map_err(|e| CheckError::Unavailable(format!("无法运行 mihomo: {e}")))?;
    // On timeout the dropped future drops the child: kill_on_drop kills it.
    match tokio::time::timeout(CHECK_TIMEOUT, child.wait_with_output()).await {
        Err(_) => Err(CheckError::Unavailable("mihomo 校验超时（15 秒）".into())),
        Ok(Err(e)) => Err(CheckError::Unavailable(format!("mihomo 校验失败: {e}"))),
        Ok(Ok(out)) if out.status.success() => Ok(()),
        Ok(Ok(out)) => {
            let text = format!(
                "{}\n{}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            );
            Err(CheckError::Rejected(check_failure_message(&text, out.status.code())))
        }
    }
}

#[derive(Debug)]
pub struct CoreProcess {
    child: Child,
    pid: u32,
}

/// `path` lies under `dir` (symlinks/`..`/case resolved for the folders).
/// The file itself is not resolved: under MSIX AppData virtualization a new
/// file resolves into the package's LocalCache while its folder does not.
fn is_inside(path: &Path, dir: &Path) -> bool {
    let in_parent = |p: &Path| match (
        p.parent().and_then(|d| d.canonicalize().ok()),
        p.file_name(),
    ) {
        (Some(d), Some(n)) => d.join(n),
        _ => p.to_path_buf(),
    };
    let dir = dir.canonicalize().unwrap_or_else(|_| in_parent(dir));
    in_parent(path).starts_with(dir)
}

/// Copies lines from a child pipe into the shared log file. If the log cannot
/// be opened the output is still drained (so the child never blocks on a full
/// pipe) but discarded, with one warning on stderr.
fn pipe_to_log<R: AsyncRead + Unpin + Send + 'static>(reader: R, log: PathBuf) {
    static WARNED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    tokio::spawn(async move {
        let mut file = match tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log)
            .await
        {
            Ok(f) => Some(f),
            Err(e) => {
                if !WARNED.swap(true, std::sync::atomic::Ordering::Relaxed) {
                    eprintln!(
                        "cannot open {}: {e}; discarding mihomo output",
                        log.display()
                    );
                }
                None
            }
        };
        let mut reader = BufReader::new(reader);
        let mut buf = Vec::new();
        loop {
            buf.clear();
            match reader.read_until(b'\n', &mut buf).await {
                Ok(0) | Err(_) => break,
                Ok(_) => {}
            }
            if let Some(f) = file.as_mut() {
                let mut line = String::from_utf8_lossy(&buf)
                    .trim_end_matches(['\r', '\n'])
                    .to_string();
                line.push('\n');
                if f.write_all(line.as_bytes()).await.is_err() {
                    file = None;
                }
            }
        }
    });
}

const LOG_ROTATE_BYTES: u64 = 5 * 1024 * 1024;

/// Before a launch: a log over `max_bytes` becomes `<log>.1` (replacing an
/// older one) so the next run starts a fresh file. Failures are logged only.
fn rotate_log(log: &Path, max_bytes: u64) {
    let Ok(meta) = std::fs::metadata(log) else { return };
    if meta.len() <= max_bytes {
        return;
    }
    let mut old = log.as_os_str().to_owned();
    old.push(".1");
    let old = PathBuf::from(old);
    let _ = std::fs::remove_file(&old);
    if let Err(e) = std::fs::rename(log, &old) {
        eprintln!("process: rotating {} failed: {e}", log.display());
    }
}

/// Windows Job Object with KILL_ON_JOB_CLOSE: when this app dies (even by a
/// hard kill) the OS closes the handle and kills every assigned mihomo.
#[cfg(windows)]
mod job {
    use std::sync::OnceLock;

    use windows_sys::Win32::Foundation::HANDLE;
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
        SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };

    struct Job(HANDLE);
    // The handle is only ever passed to Win32 calls; it lives for the process.
    unsafe impl Send for Job {}
    unsafe impl Sync for Job {}

    static JOB: OnceLock<Option<Job>> = OnceLock::new();

    fn create() -> Option<Job> {
        unsafe {
            let h = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if h.is_null() {
                return None;
            }
            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let ok = SetInformationJobObject(
                h,
                JobObjectExtendedLimitInformation,
                &info as *const _ as *const _,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            );
            (ok != 0).then_some(Job(h))
        }
    }

    /// Failures are logged only: the child still runs (kill_on_drop remains).
    pub fn assign(child: &tokio::process::Child) {
        let Some(job) = JOB.get_or_init(create) else {
            eprintln!("process: could not create job object");
            return;
        };
        let Some(proc) = child.raw_handle() else { return };
        if unsafe { AssignProcessToJobObject(job.0, proc as HANDLE) } == 0 {
            eprintln!("process: could not assign mihomo to job object");
        }
    }
}

impl CoreProcess {
    /// Runs `bin -d workdir -f config`. `config` must be inside `workdir`
    /// (spike S5). Does not wait for the API; see [`wait_ready`].
    pub async fn start(bin: &Path, workdir: &Path, config: &Path) -> io::Result<Self> {
        if !is_inside(config, workdir) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "config must be inside the mihomo workdir (spike S5)",
            ));
        }
        let log = workdir.join("mihomo.log");
        rotate_log(&log, LOG_ROTATE_BYTES);
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
        #[cfg(windows)]
        job::assign(&child);
        let pid = child
            .id()
            .ok_or_else(|| io::Error::other("mihomo exited immediately"))?;
        if let Some(out) = child.stdout.take() {
            pipe_to_log(out, log.clone());
        }
        if let Some(err) = child.stderr.take() {
            pipe_to_log(err, log);
        }
        Ok(Self { child, pid })
    }

    pub async fn stop(&mut self) -> io::Result<()> {
        if let Err(e) = self.child.start_kill() {
            // An already-exited child is fine; anything else is a real failure.
            if self.child.try_wait().ok().flatten().is_none() {
                return Err(e);
            }
        }
        match tokio::time::timeout(Duration::from_secs(5), self.child.wait()).await {
            Ok(r) => r.map(|_| ()),
            Err(_) => Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "mihomo did not exit within 5s of kill",
            )),
        }
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
    async fn check_config(&self, config: &Path) -> Result<(), CheckError> {
        check_config_with(&self.bin, &self.workdir, config).await
    }
    fn core_binary(&self) -> Option<PathBuf> {
        Some(self.bin.clone())
    }
}

/// Polls `version()` every 100 ms until it answers or `timeout` passes.
/// Callers use 5 s; a timeout is a start failure.
pub async fn wait_ready(api: &dyn MihomoApi, timeout: Duration) -> Result<String, ApiError> {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        // Bound each probe by the remaining time so a mihomo that accepts but
        // never answers cannot overrun the deadline.
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        let err = match tokio::time::timeout(remaining, api.version()).await {
            Ok(Ok(v)) => return Ok(v),
            Ok(Err(e)) => e,
            Err(_) => ApiError::Unreachable("timed out waiting for mihomo".into()),
        };
        if tokio::time::Instant::now() >= deadline {
            return Err(err);
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::api::{ConnectionsSnapshot, HttpMihomoApi};
    use futures_util::stream::BoxStream;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[test]
    fn is_inside_ignores_where_the_file_itself_resolves() {
        // MSIX AppData virtualization (e.g. launched from a packaged app)
        // makes a new file resolve elsewhere while its folder does not; a
        // file symlink reproduces that.
        let work = tempfile::tempdir().unwrap();
        let elsewhere = tempfile::tempdir().unwrap();
        let target = elsewhere.path().join("real.yaml");
        std::fs::write(&target, "").unwrap();
        let cfg = work.path().join("config.yaml");
        if std::os::windows::fs::symlink_file(&target, &cfg).is_err() {
            eprintln!("skipped: symlinks not permitted");
            return;
        }
        assert!(is_inside(&cfg, work.path()));
    }

    #[test]
    fn is_inside_rejects_dotdot_escape() {
        let work = tempfile::tempdir().unwrap();
        let sub = work.path().join("sub");
        std::fs::create_dir(&sub).unwrap();
        assert!(!is_inside(&sub.join("..").join("config.yaml"), &sub));
        assert!(is_inside(&sub.join("config.yaml"), &sub));
    }

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
        async fn proxy_now(&self, _: &str) -> Result<Option<String>, ApiError> {
            unimplemented!()
        }
        fn traffic_stream(&self) -> BoxStream<'static, Result<(u64, u64), ApiError>> {
            unimplemented!()
        }
        fn log_stream(&self) -> BoxStream<'static, Result<String, ApiError>> {
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
    async fn wait_ready_bounded_when_server_never_replies() {
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = l.local_addr().unwrap().port();
        tokio::spawn(async move {
            let mut held = Vec::new();
            while let Ok((s, _)) = l.accept().await {
                held.push(s); // accept, never answer
            }
        });
        let api = HttpMihomoApi::new(port, "x".into());
        let t = std::time::Instant::now();
        let r = wait_ready(&api, Duration::from_millis(500)).await;
        assert!(r.is_err());
        assert!(t.elapsed() < Duration::from_secs(1), "{:?}", t.elapsed());
    }

    #[tokio::test]
    async fn start_rejects_config_outside_workdir() {
        let work = tempfile::tempdir().unwrap();
        let other = tempfile::tempdir().unwrap();
        let cfg = other.path().join("config.yaml");
        let e = CoreProcess::start(Path::new("whatever.exe"), work.path(), &cfg)
            .await
            .unwrap_err();
        assert_eq!(e.kind(), io::ErrorKind::InvalidInput);
    }

    #[tokio::test]
    async fn start_missing_binary_is_io_error() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = dir.path().join("config.yaml");
        let r =
            CoreProcess::start(Path::new("definitely-not-a-binary.exe"), dir.path(), &cfg).await;
        assert!(r.is_err());
    }

    #[test]
    fn rotate_log_moves_only_oversized_logs() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("mihomo.log");
        let old = dir.path().join("mihomo.log.1");
        rotate_log(&log, 10); // missing: nothing happens
        assert!(!old.exists());

        std::fs::write(&log, "small").unwrap();
        rotate_log(&log, 10);
        assert_eq!(std::fs::read_to_string(&log).unwrap(), "small");
        assert!(!old.exists());

        std::fs::write(&old, "older").unwrap();
        std::fs::write(&log, "x".repeat(11)).unwrap();
        rotate_log(&log, 10);
        assert!(!log.exists());
        assert_eq!(std::fs::read_to_string(&old).unwrap(), "x".repeat(11));
    }

    #[test]
    fn check_message_keeps_error_msgs_only() {
        let out = "time=\"t\" level=info msg=\"Start initial configuration in progress\"\n\
                   time=\"t\" level=error msg=\"proxy 0: '' has unset fields: cipher\"\n\
                   configuration file C:\\x\\check.yaml test failed\n";
        assert_eq!(check_failure_message(out, Some(1)), "proxy 0: '' has unset fields: cipher");
    }

    #[test]
    fn check_message_redacts_urls_and_falls_back() {
        let out = "level=error msg=\"fetch https://sub.example.com/x?token=SECRET failed\"";
        let m = check_failure_message(out, Some(1));
        assert_eq!(m, "fetch <URL> failed");
        assert_eq!(check_failure_message("plain problem\n", Some(1)), "plain problem");
        assert_eq!(
            check_failure_message("configuration file x test failed", Some(2)),
            "mihomo -t 失败（退出码 2）"
        );
        let long = format!("level=error msg=\"{}\"", "x".repeat(600));
        assert_eq!(check_failure_message(&long, None).chars().count(), CHECK_MESSAGE_MAX_CHARS + 1);
    }

    #[tokio::test]
    async fn check_missing_binary_is_unavailable() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = dir.path().join("c.yaml");
        let r = check_config_with(Path::new("definitely-not-a-binary.exe"), dir.path(), &cfg).await;
        assert!(matches!(r, Err(CheckError::Unavailable(_))), "{r:?}");
    }

    #[tokio::test]
    #[ignore = "needs a real mihomo binary: set MIHOMO_BIN"]
    async fn check_real_core() {
        let bin = PathBuf::from(std::env::var("MIHOMO_BIN").expect("MIHOMO_BIN"));
        let dir = tempfile::tempdir().unwrap();
        let good = dir.path().join("good.yaml");
        std::fs::write(&good, crate::config_gen::empty_config(19999, "s")).unwrap();
        assert_eq!(check_config_with(&bin, dir.path(), &good).await, Ok(()));
        let bad = dir.path().join("bad.yaml");
        std::fs::write(
            &bad,
            "proxies:\n  - {name: a, type: ss, server: 1.1.1.1, port: 1, password: x}\nrules: [\"MATCH,REJECT\"]\n",
        )
        .unwrap();
        let r = check_config_with(&bin, dir.path(), &bad).await;
        assert!(matches!(&r, Err(CheckError::Rejected(m)) if m.contains("cipher")), "{r:?}");
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

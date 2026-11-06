//! Login state of the Tailscale nodes, derived from mihomo's log.
//!
//! mihomo v1.19.33 exposes no login state through `GET /proxies/<name>`
//! (spike/TAILSCALE.md T3), so the state comes from the info log lines tsnet
//! writes as `[Tailscale](<proxy name>) ...`. The generated config keeps
//! `log-level: warning`, so these lines are read from the controller's
//! `/logs?level=info` stream (which carries info events whatever the
//! configured level), not from stdout.

use std::collections::HashMap;

use serde::Serialize;

#[derive(Serialize, Clone, Debug, PartialEq, Eq, specta::Type)]
pub enum TailscaleState {
    Unknown,
    NeedsLogin { url: Option<String> },
    LoggedIn { ip: Option<String> },
    Failed { reason: String },
}

#[derive(Default, Debug)]
pub struct TailscaleTracker {
    states: HashMap<String, TailscaleState>,
    /// Why the running instance could not apply its `exit-node`.
    exit_errors: HashMap<String, String>,
}

impl TailscaleTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Feeds one log line (a `/logs` payload or a raw stdout line). Returns
    /// whether any state changed.
    pub fn on_log_line(&mut self, line: &str) -> bool {
        let Some((name, event)) = parse(line) else { return false };
        let event = match event {
            Event::ExitNodeFailed(reason) => {
                let old = self.exit_errors.insert(name.to_string(), reason.to_string());
                return old.as_deref() != Some(reason);
            }
            Event::Login(e) => e,
        };
        // A new instance applies its exit node afresh.
        let cleared = matches!(event, LoginEvent::Started) && self.exit_errors.remove(name).is_some();
        let old = self.states.get(name);
        let new = match (event, old) {
            // A (re)started tsnet server: a logged-in node stays logged in, a
            // pending URL is stale (every start registers anew).
            (LoginEvent::Started, Some(s @ TailscaleState::LoggedIn { .. })) => s.clone(),
            (LoginEvent::Started, Some(TailscaleState::NeedsLogin { .. })) => {
                TailscaleState::NeedsLogin { url: None }
            }
            (LoginEvent::Started, _) => TailscaleState::Unknown,
            // The NeedsLogin line precedes the URL line; never drop a URL.
            (LoginEvent::NeedsLogin, Some(s @ TailscaleState::NeedsLogin { .. })) => s.clone(),
            (LoginEvent::NeedsLogin, _) => TailscaleState::NeedsLogin { url: None },
            (LoginEvent::AuthUrl(url), _) => TailscaleState::NeedsLogin { url: Some(url.to_string()) },
            (LoginEvent::LoggedIn, _) => TailscaleState::LoggedIn { ip: None },
        };
        if old == Some(&new) {
            return cleared;
        }
        self.states.insert(name.to_string(), new);
        true
    }

    /// Proxy name (`手动/<名>`) -> state, for every node seen in the log.
    pub fn states(&self) -> HashMap<String, TailscaleState> {
        self.states.clone()
    }

    /// Proxy name -> why its exit node was not applied.
    pub fn exit_errors(&self) -> HashMap<String, String> {
        self.exit_errors.clone()
    }

    /// Forgets nodes no longer in the config. Returns whether any were dropped.
    pub fn retain(&mut self, names: &[String]) -> bool {
        let before = self.states.len() + self.exit_errors.len();
        self.states.retain(|k, _| names.contains(k));
        self.exit_errors.retain(|k, _| names.contains(k));
        self.states.len() + self.exit_errors.len() != before
    }

    /// Forgets everything (a new mihomo process logs every state afresh).
    /// Returns whether anything was dropped.
    pub fn reset(&mut self) -> bool {
        let had = !self.states.is_empty() || !self.exit_errors.is_empty();
        self.states.clear();
        self.exit_errors.clear();
        had
    }
}

enum Event<'a> {
    Login(LoginEvent<'a>),
    ExitNodeFailed(&'a str),
}

enum LoginEvent<'a> {
    Started,
    NeedsLogin,
    AuthUrl(&'a str),
    LoggedIn,
}

// Log texts verbatim from spike/TAILSCALE.md (phase A T1, phase B).
const PREFIX: &str = "[Tailscale](";
const STARTED: &str = ") tsnet running state path ";
const NEEDS_LOGIN: &str = ") LocalBackend state is NeedsLogin; running StartLoginInteractive...";
const AUTH_URL: &str = ") To start this tsnet server, restart with TS_AUTHKEY set, or go to: ";
const AUTH_LOOP: &str = ") AuthLoop: state is ";
const AUTH_LOOP_END: &str = "; done";
// warning level, e.g. `set exit node failed: no node found in netmap with IP 100.91.166.56`.
const EXIT_NODE_FAILED: &str = ") set exit node failed: ";

/// `(proxy name, event)` of a Tailscale state line. The name is everything
/// between `[Tailscale](` and the fixed text, so names containing `)` work.
fn parse(line: &str) -> Option<(&str, Event<'_>)> {
    let rest = &line[line.find(PREFIX)? + PREFIX.len()..];
    let at = |marker: &str| rest.find(marker).map(|i| (&rest[..i], &rest[i + marker.len()..]));
    if let Some((name, url)) = at(AUTH_URL) {
        let end = url.find(|c: char| c.is_whitespace() || c == '"').unwrap_or(url.len());
        let url = &url[..end];
        return url.starts_with("https://").then_some((name, Event::Login(LoginEvent::AuthUrl(url))));
    }
    if let Some((name, tail)) = at(AUTH_LOOP) {
        let state = &tail[..tail.find(AUTH_LOOP_END)?];
        // Starting | Running = logged in (spike phase B).
        return matches!(state, "Starting" | "Running").then_some((name, Event::Login(LoginEvent::LoggedIn)));
    }
    if let Some((name, reason)) = at(EXIT_NODE_FAILED) {
        // A raw stdout line ends with the closing quote of `msg="..."`.
        let reason = reason.strip_suffix('"').unwrap_or(reason).trim();
        return Some((name, Event::ExitNodeFailed(reason)));
    }
    if let Some((name, _)) = at(NEEDS_LOGIN) {
        return Some((name, Event::Login(LoginEvent::NeedsLogin)));
    }
    if let Some((name, _)) = at(STARTED) {
        return Some((name, Event::Login(LoginEvent::Started)));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    // Verbatim from spike/work/b/run1.log (first login, phase B).
    const RUN1: &[&str] = &[
        r#"time="2026-10-27T13:52:05.213053200+08:00" level=info msg="[Tailscale](ts-b) tsnet running state path C:\\E\\claudeWS\\mihomoGUI\\.claude\\worktrees\\manual-nodes-groups\\spike\\work\\b\\tailscale\\tb\\tailscaled.state""#,
        r#"time="2026-10-27T13:52:05.228545100+08:00" level=info msg="[Tailscale](ts-b) tsnet starting with hostname \"mihomo-spike\", varRoot \"C:\\\\E\\\\claudeWS\\\\mihomoGUI\\\\.claude\\\\worktrees\\\\manual-nodes-groups\\\\spike\\\\work\\\\b\\\\tailscale\\\\tb\"""#,
        r#"time="2026-10-27T13:52:05.257023400+08:00" level=info msg="[Tailscale](ts-b) LocalBackend state is NeedsLogin; running StartLoginInteractive...""#,
        r#"time="2026-10-27T13:52:08.888024000+08:00" level=info msg="[Tailscale](ts-b) To start this tsnet server, restart with TS_AUTHKEY set, or go to: https://login.tailscale.com/a/1490d0cd0192e9""#,
        r#"time="2026-10-27T13:52:10.106008100+08:00" level=warning msg="[TCP] dial ts-b 127.0.0.1:34328 --> www.gstatic.com:443 error: context deadline exceeded""#,
        r#"time="2026-10-27T13:55:39.226053600+08:00" level=info msg="[Tailscale](ts-b) AuthLoop: state is Starting; done""#,
    ];
    // spike/work/b/run2.log: restart with saved state.
    const RUN2_RUNNING: &str = r#"time="2026-10-27T13:59:50.580655100+08:00" level=info msg="[Tailscale](ts-b) AuthLoop: state is Running; done""#;
    // `/logs?level=info` payloads captured from the spike binary with
    // `log-level: warning` (spike/work/t13/logs-api.log).
    const API: &[&str] = &[
        r#"[Tailscale](手动/ts-x) tsnet running state path C:\E\claudeWS\mihomoGUI\.claude\worktrees\manual-nodes-groups\spike\work\t13\tailscale\t13\tailscaled.state"#,
        r#"[Tailscale](手动/ts-x) tsnet starting with hostname "mihomo-t13", varRoot "C:\\E\\claudeWS\\mihomoGUI\\.claude\\worktrees\\manual-nodes-groups\\spike\\work\\t13\\tailscale\\t13""#,
        r#"[Tailscale](手动/ts-x) LocalBackend state is NeedsLogin; running StartLoginInteractive..."#,
        r#"[Tailscale](手动/ts-x) To start this tsnet server, restart with TS_AUTHKEY set, or go to: https://login.tailscale.com/a/1654b85a0197ca"#,
    ];

    fn st(t: &TailscaleTracker, name: &str) -> Option<TailscaleState> {
        t.states().get(name).cloned()
    }

    fn needs(url: Option<&str>) -> TailscaleState {
        TailscaleState::NeedsLogin { url: url.map(str::to_string) }
    }

    const LOGGED_IN: TailscaleState = TailscaleState::LoggedIn { ip: None };

    #[test]
    fn first_login_sequence_from_stdout_lines() {
        let mut t = TailscaleTracker::new();
        assert!(t.on_log_line(RUN1[0]));
        assert_eq!(st(&t, "ts-b"), Some(TailscaleState::Unknown));
        t.on_log_line(RUN1[1]);
        assert_eq!(st(&t, "ts-b"), Some(TailscaleState::Unknown));
        assert!(t.on_log_line(RUN1[2]));
        assert_eq!(st(&t, "ts-b"), Some(needs(None)));
        assert!(t.on_log_line(RUN1[3]));
        assert_eq!(
            st(&t, "ts-b"),
            Some(needs(Some("https://login.tailscale.com/a/1490d0cd0192e9")))
        );
        // The dial timeout warning is not a login state.
        assert!(!t.on_log_line(RUN1[4]));
        assert!(t.on_log_line(RUN1[5]));
        assert_eq!(st(&t, "ts-b"), Some(LOGGED_IN));
        assert_eq!(t.states().len(), 1);
    }

    #[test]
    fn restart_with_saved_state_is_logged_in() {
        let mut t = TailscaleTracker::new();
        t.on_log_line(RUN1[0]);
        assert!(t.on_log_line(RUN2_RUNNING));
        assert_eq!(st(&t, "ts-b"), Some(LOGGED_IN));
    }

    #[test]
    fn api_payloads_with_manual_name() {
        let mut t = TailscaleTracker::new();
        for l in API {
            t.on_log_line(l);
        }
        assert_eq!(
            st(&t, "手动/ts-x"),
            Some(needs(Some("https://login.tailscale.com/a/1654b85a0197ca")))
        );
    }

    #[test]
    fn two_instances_are_tracked_apart() {
        // spike/work/t4.log (URL lines verbatim, in log order).
        let mut t = TailscaleTracker::new();
        t.on_log_line(r#"time="2026-10-27T12:05:01.384932600+08:00" level=info msg="[Tailscale](ts-a) LocalBackend state is NeedsLogin; running StartLoginInteractive...""#);
        t.on_log_line(r#"time="2026-10-27T12:05:03.827106200+08:00" level=info msg="[Tailscale](ts-b) To start this tsnet server, restart with TS_AUTHKEY set, or go to: https://login.tailscale.com/a/792a63012f01""#);
        assert_eq!(st(&t, "ts-a"), Some(needs(None)));
        assert_eq!(
            st(&t, "ts-b"),
            Some(needs(Some("https://login.tailscale.com/a/792a63012f01")))
        );
        t.on_log_line(r#"time="2026-10-27T12:05:04.721904100+08:00" level=info msg="[Tailscale](ts-a) To start this tsnet server, restart with TS_AUTHKEY set, or go to: https://login.tailscale.com/a/1e72955c018e69""#);
        assert_eq!(
            st(&t, "ts-a"),
            Some(needs(Some("https://login.tailscale.com/a/1e72955c018e69")))
        );
    }

    #[test]
    fn names_with_parentheses_and_spaces() {
        let mut t = TailscaleTracker::new();
        t.on_log_line("[Tailscale](手动/home (pc) x) AuthLoop: state is Running; done");
        assert_eq!(st(&t, "手动/home (pc) x"), Some(LOGGED_IN));
    }

    #[test]
    fn new_instance_drops_a_stale_url_but_keeps_logged_in() {
        // A reload recreates the tsnet server; its old URL is no longer valid
        // (spike: a fresh URL per start). A logged-in node stays logged in.
        let mut t = TailscaleTracker::new();
        for l in API {
            t.on_log_line(l);
        }
        assert!(t.on_log_line(API[0]));
        assert_eq!(st(&t, "手动/ts-x"), Some(needs(None)));
        t.on_log_line(RUN1[5]);
        assert!(!t.on_log_line(RUN1[0]));
        assert_eq!(st(&t, "ts-b"), Some(LOGGED_IN));
    }

    #[test]
    fn failure_mode_lines_do_not_change_state() {
        // spike T6 (node bound to a NIC without internet): at info/warning
        // level the instance logs NeedsLogin, then only dial timeouts; no
        // auth URL ever appears and no explicit failure line exists.
        let mut t = TailscaleTracker::new();
        t.on_log_line(r#"time="2026-10-27T12:06:43.031216500+08:00" level=info msg="[Tailscale](ts-a) LocalBackend state is NeedsLogin; running StartLoginInteractive...""#);
        assert!(!t.on_log_line(r#"time="2026-10-27T12:06:47.923911600+08:00" level=warning msg="[TCP] dial ts-a 127.0.0.1:55041 --> www.gstatic.com:443 error: context deadline exceeded""#));
        assert_eq!(st(&t, "ts-a"), Some(needs(None)));
    }

    #[test]
    fn exit_node_failure_is_tracked_until_the_next_start() {
        let mut t = TailscaleTracker::new();
        t.on_log_line(RUN2_RUNNING);
        // Verbatim from this app's mihomo.log.
        let line = r#"time="2026-10-28T00:01:20.648504300+08:00" level=warning msg="[Tailscale](手动/1) set exit node failed: no node found in netmap with IP 100.91.166.56""#;
        assert!(t.on_log_line(line));
        assert!(!t.on_log_line(line));
        assert_eq!(
            t.exit_errors().get("手动/1").map(String::as_str),
            Some("no node found in netmap with IP 100.91.166.56")
        );
        assert_eq!(st(&t, "ts-b"), Some(LOGGED_IN));
        // `/logs` payload form (no quotes).
        assert!(t.on_log_line("[Tailscale](手动/2) set exit node failed: boom"));
        assert_eq!(t.exit_errors().get("手动/2").map(String::as_str), Some("boom"));
        // A new instance clears it, and that counts as a change.
        assert!(t.on_log_line(r"[Tailscale](手动/1) tsnet running state path C:\x\tailscaled.state"));
        assert!(!t.exit_errors().contains_key("手动/1"));
        assert!(t.retain(&[]));
        assert!(t.exit_errors().is_empty());
        t.on_log_line("[Tailscale](手动/2) set exit node failed: boom");
        assert!(t.reset());
        assert!(t.exit_errors().is_empty());
    }

    #[test]
    fn unrelated_lines_are_ignored() {
        let mut t = TailscaleTracker::new();
        assert!(!t.on_log_line(r#"time="2026-10-27T13:51:56.161610000+08:00" level=info msg="Start initial configuration in progress""#));
        assert!(!t.on_log_line(""));
        assert!(t.states().is_empty());
    }

    #[test]
    fn reset_and_retain() {
        let mut t = TailscaleTracker::new();
        t.on_log_line(RUN2_RUNNING);
        t.on_log_line(API[2]);
        t.retain(&["ts-b".to_string()]);
        assert_eq!(t.states().len(), 1);
        assert!(t.reset());
        assert!(t.states().is_empty());
        assert!(!t.reset());
    }
}

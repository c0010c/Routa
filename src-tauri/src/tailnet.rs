//! Tailnet devices as seen by the system Tailscale client.
//!
//! mihomo exposes neither an instance's peers nor its own address (only
//! debug-level logs mention them), so the UI asks the system client, when one
//! is installed and logged in: `tailscale status --json`. The frontend finds
//! the instance among the peers by hostname, which also proves both are on
//! the same tailnet.

use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Clone, Debug, PartialEq, Eq, specta::Type)]
pub struct TailnetPeer {
    /// The machine's requested hostname (what a Tailscale instance's
    /// `hostname` registers as).
    pub host_name: String,
    /// MagicDNS name without the trailing dot.
    pub dns_name: String,
    pub ips: Vec<String>,
    pub online: bool,
    /// Advertises itself as an exit node and is approved for this tailnet.
    pub exit_node_option: bool,
}

#[derive(Deserialize)]
struct RawStatus {
    #[serde(rename = "BackendState", default)]
    backend_state: String,
    #[serde(rename = "Peer", default)]
    peer: Option<std::collections::BTreeMap<String, RawPeer>>,
}

#[derive(Deserialize)]
struct RawPeer {
    #[serde(rename = "HostName", default)]
    host_name: String,
    #[serde(rename = "DNSName", default)]
    dns_name: String,
    #[serde(rename = "TailscaleIPs", default)]
    ips: Option<Vec<String>>,
    #[serde(rename = "Online", default)]
    online: bool,
    #[serde(rename = "ExitNodeOption", default)]
    exit_node_option: bool,
}

/// Peers from `tailscale status --json` output, sorted by hostname.
pub fn parse_status(json: &str) -> Result<Vec<TailnetPeer>, String> {
    let raw: RawStatus = serde_json::from_str(json).map_err(|e| format!("无法解析 tailscale status 输出: {e}"))?;
    if raw.backend_state != "Running" {
        return Err(format!("本机 Tailscale 客户端未连接（{}）", raw.backend_state));
    }
    let mut peers: Vec<TailnetPeer> = raw
        .peer
        .unwrap_or_default()
        .into_values()
        .map(|p| TailnetPeer {
            host_name: p.host_name,
            dns_name: p.dns_name.trim_end_matches('.').to_string(),
            ips: p.ips.unwrap_or_default(),
            online: p.online,
            exit_node_option: p.exit_node_option,
        })
        .collect();
    peers.sort_by(|a, b| a.host_name.to_lowercase().cmp(&b.host_name.to_lowercase()));
    Ok(peers)
}

fn cli_candidates() -> Vec<PathBuf> {
    let mut v = vec![PathBuf::from("tailscale")];
    if let Some(pf) = std::env::var_os("ProgramFiles") {
        v.push(PathBuf::from(pf).join("Tailscale").join("tailscale.exe"));
    }
    v
}

/// Runs the system client's `status --json` (5 s limit).
pub async fn system_peers() -> Result<Vec<TailnetPeer>, String> {
    for exe in cli_candidates() {
        let mut cmd = tokio::process::Command::new(&exe);
        cmd.args(["status", "--json"]).kill_on_drop(true);
        #[cfg(windows)]
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        let out = match tokio::time::timeout(Duration::from_secs(5), cmd.output()).await {
            Err(_) => return Err("本机 Tailscale 客户端无响应".into()),
            Ok(Err(e)) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Ok(Err(e)) => return Err(format!("无法运行本机 Tailscale 客户端: {e}")),
            Ok(Ok(out)) => out,
        };
        // A stopped client still prints JSON (BackendState "Stopped") but
        // exits non-zero on some versions: parse whatever came out.
        return parse_status(&String::from_utf8_lossy(&out.stdout));
    }
    Err("本机没有安装 Tailscale 客户端".into())
}

/// Only Tailscale login pages may be opened from the UI.
pub fn is_login_url(url: &str) -> bool {
    url.strip_prefix("https://login.tailscale.com/")
        .is_some_and(|rest| !rest.is_empty() && rest.chars().all(|c| c.is_ascii_alphanumeric() || "/-_?=&.".contains(c)))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Shape of `tailscale status --json` (v1.9x), trimmed.
    const STATUS: &str = r#"{
      "Version": "1.96.2", "BackendState": "Running", "MagicDNSSuffix": "tailb4bf67.ts.net",
      "Self": {"HostName": "qs"},
      "Peer": {
        "nodekey:a": {"HostName": "VM-0-5-opencloudos", "DNSName": "vm-0-5-opencloudos.tailb4bf67.ts.net.",
                      "TailscaleIPs": ["100.91.166.56", "fd7a::1"], "Online": true, "ExitNodeOption": true},
        "nodekey:b": {"HostName": "mihomo-2", "DNSName": "mihomo-2.tailb4bf67.ts.net.",
                      "TailscaleIPs": ["100.121.28.62"], "Online": true, "ExitNodeOption": false},
        "nodekey:c": {"HostName": "localhost", "DNSName": "iphone181.tailb4bf67.ts.net.",
                      "TailscaleIPs": null, "Online": false}
      }
    }"#;

    #[test]
    fn parses_peers_sorted() {
        let p = parse_status(STATUS).unwrap();
        let names: Vec<_> = p.iter().map(|p| p.host_name.as_str()).collect();
        assert_eq!(names, ["localhost", "mihomo-2", "VM-0-5-opencloudos"]);
        assert_eq!(p[2].dns_name, "vm-0-5-opencloudos.tailb4bf67.ts.net");
        assert_eq!(p[2].ips[0], "100.91.166.56");
        assert!(p[2].online && p[2].exit_node_option);
        assert!(p[0].ips.is_empty() && !p[0].online && !p[0].exit_node_option);
    }

    #[test]
    fn not_running_is_an_error() {
        let e = parse_status(r#"{"BackendState": "NeedsLogin", "Peer": null}"#).unwrap_err();
        assert!(e.contains("NeedsLogin"), "{e}");
        assert!(parse_status("not json").is_err());
    }

    #[test]
    fn running_without_peers_is_empty() {
        assert_eq!(parse_status(r#"{"BackendState": "Running"}"#).unwrap(), vec![]);
    }

    #[test]
    fn login_url_check() {
        assert!(is_login_url("https://login.tailscale.com/a/1654b85a0197ca"));
        assert!(!is_login_url("https://login.tailscale.com/"));
        assert!(!is_login_url("http://login.tailscale.com/a/1"));
        assert!(!is_login_url("https://login.tailscale.com.evil.example/a/1"));
        assert!(!is_login_url("https://login.tailscale.com/a/1 & calc"));
        assert!(!is_login_url("https://evil.example/?https://login.tailscale.com/a/1"));
    }
}

#[cfg(test)]
mod live {
    /// Needs the system Tailscale client: `cargo test --lib tailnet::live -- --ignored --nocapture`.
    #[tokio::test]
    #[ignore]
    async fn system_peers_live() {
        let peers = super::system_peers().await.unwrap();
        for p in &peers {
            println!("{} {:?} online={} exit={}", p.host_name, p.ips, p.online, p.exit_node_option);
        }
        assert!(!peers.is_empty());
    }
}

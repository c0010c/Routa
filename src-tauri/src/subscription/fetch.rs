//! Subscription fetching (direct, with optional fallback through an exit) and the
//! helpers that decide how results are recorded and which loopback ports to use.

use std::collections::HashMap;
use std::fmt;
use std::net::TcpListener;
use std::time::Duration;

use crate::model::{ExitId, FetchResult, Model, Subscription};
use crate::subscription::parse::{parse_subscription, ParseError, Parsed};

#[derive(Debug, Clone, PartialEq)]
pub enum FetchError {
    Http(String),
    Parse(ParseError),
    /// The fetched nodes were rejected by `mihomo -t`.
    CoreRejected(String),
}

impl fmt::Display for FetchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FetchError::Http(msg) => write!(f, "{msg}"),
            FetchError::Parse(ParseError::NoNodes) => write!(f, "订阅中没有可用节点"),
            FetchError::Parse(ParseError::Unrecognized) => {
                write!(f, "无法识别订阅内容（可能是错误页或已过期）")
            }
            FetchError::CoreRejected(msg) => write!(f, "订阅节点未通过 mihomo 校验：{msg}"),
        }
    }
}

impl std::error::Error for FetchError {}

const MAX_BODY_BYTES: usize = 10 * 1024 * 1024;

fn build_client(via_proxy: Option<u16>) -> Result<reqwest::Client, FetchError> {
    // `no_proxy()` first: neither path may silently pick up env/system proxies.
    let mut builder = reqwest::Client::builder()
        .user_agent("clash.meta")
        .timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(10))
        .no_proxy();
    if let Some(port) = via_proxy {
        let proxy = reqwest::Proxy::all(format!("http://127.0.0.1:{port}"))
            .map_err(|e| FetchError::Http(format!("代理配置无效: {}", e.without_url())))?;
        builder = builder.proxy(proxy);
    }
    builder.build().map_err(|e| FetchError::Http(format!("创建 HTTP 客户端失败: {}", e.without_url())))
}

/// GET the raw subscription body, directly or through `127.0.0.1:<port>`.
pub async fn fetch_body(url: &str, via_proxy: Option<u16>) -> Result<String, FetchError> {
    let client = build_client(via_proxy)?;
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| FetchError::Http(format!("请求失败: {}", e.without_url())))?;
    let status = resp.status();
    if !status.is_success() {
        return Err(FetchError::Http(format!("HTTP {}", status.as_u16())));
    }
    let mut resp = resp;
    let mut buf: Vec<u8> = Vec::new();
    while let Some(chunk) = resp
        .chunk()
        .await
        .map_err(|e| FetchError::Http(format!("读取响应失败: {}", e.without_url())))?
    {
        if buf.len() + chunk.len() > MAX_BODY_BYTES {
            return Err(FetchError::Http("订阅内容过大（超过 10 MiB）".into()));
        }
        buf.extend_from_slice(&chunk);
    }
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

async fn fetch_nodes(url: &str, via_proxy: Option<u16>) -> Result<Parsed, FetchError> {
    let body = fetch_body(url, via_proxy).await?;
    parse_subscription(&body).map_err(FetchError::Parse)
}

/// Try a direct fetch; if it fails and a fallback port is given, retry through it.
pub async fn update_subscription(
    sub: &Subscription,
    fallback_port: Option<u16>,
) -> Result<Parsed, FetchError> {
    let direct_err = match fetch_nodes(&sub.url, None).await {
        Ok(nodes) => return Ok(nodes),
        Err(e) => e,
    };
    let Some(port) = fallback_port else {
        return Err(direct_err);
    };
    match fetch_nodes(&sub.url, Some(port)).await {
        Ok(nodes) => Ok(nodes),
        // `Parse` carries no text, so the combined message is reported as `Http`.
        Err(proxy_err) => Err(FetchError::Http(format!(
            "直连失败: {direct_err}; 通过出口失败: {proxy_err}"
        ))),
    }
}

/// Store a fetch outcome on the subscription. Failures keep the old nodes; both outcomes
/// stamp `last_updated` so a failing subscription is not retried immediately.
pub fn record_result(sub: &mut Subscription, result: Result<Parsed, FetchError>, now: i64) {
    match result {
        Ok(Parsed { nodes, skipped_tailscale }) => {
            sub.last_result = Some(FetchResult::Ok { node_count: nodes.len() as u32, skipped_tailscale });
            sub.nodes = nodes;
        }
        Err(e) => {
            sub.last_result = Some(FetchResult::Failed { reason: e.to_string() });
        }
    }
    sub.last_updated = Some(now);
}

/// Stable loopback port assignments for the internal fetch listeners.
#[derive(Debug, Default)]
pub struct PortAllocator {
    assigned: HashMap<ExitId, u16>,
}

impl PortAllocator {
    pub fn new() -> Self {
        Self::default()
    }
}

fn free_local_port() -> Option<u16> {
    let sock = TcpListener::bind("127.0.0.1:0").ok()?;
    sock.local_addr().ok().map(|a| a.port())
}

/// One loopback port per distinct `fallback_exit` (model order). Existing assignments are
/// kept; exits no longer referenced are released.
pub fn fetch_listener_ports(model: &Model, alloc: &mut PortAllocator) -> Vec<(ExitId, u16)> {
    let mut exits: Vec<ExitId> = Vec::new();
    for sub in &model.subscriptions {
        if let Some(id) = &sub.fallback_exit {
            if !exits.contains(id) {
                exits.push(id.clone());
            }
        }
    }
    alloc.assigned.retain(|id, _| exits.contains(id));
    let mut out = Vec::new();
    for id in exits {
        let port = match alloc.assigned.get(&id) {
            Some(p) => *p,
            None => {
                // Never hand out a port already assigned to another exit.
                let found = (0..16)
                    .filter_map(|_| free_local_port())
                    .find(|p| !alloc.assigned.values().any(|v| v == p));
                let Some(p) = found else { continue };
                alloc.assigned.insert(id.clone(), p);
                p
            }
        };
        out.push((id, port));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::SubscriptionId;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    const YAML2: &str = "proxies:\n  - {name: a, type: ss, server: 1.1.1.1, port: 1, cipher: aes-128-gcm, password: x}\n  - {name: b, type: ss, server: 2.2.2.2, port: 2, cipher: aes-128-gcm, password: x}\n";
    const YAML1: &str = "proxies:\n  - {name: c, type: ss, server: 3.3.3.3, port: 3, cipher: aes-128-gcm, password: x}\n";

    fn sub(url: &str) -> Subscription {
        Subscription {
            fallback_listener: None,
            id: SubscriptionId::new(),
            name: "s".into(),
            url: url.into(),
            update_interval_mins: Some(60),
            fallback_exit: None,
            last_updated: None,
            last_result: None,
            nodes: vec![],
        }
    }

    async fn serve(body: &str, status: u16) -> MockServer {
        let s = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(status).set_body_string(body))
            .mount(&s)
            .await;
        s
    }

    fn port_of(s: &MockServer) -> u16 {
        s.address().port()
    }

    #[tokio::test]
    async fn success_replaces_nodes() {
        let s = serve(YAML2, 200).await;
        let mut sb = sub(&format!("{}/sub", s.uri()));
        sb.nodes = parse_subscription(YAML1).unwrap().nodes;
        let r = update_subscription(&sb, None).await;
        record_result(&mut sb, r, 1000);
        assert_eq!(sb.nodes.len(), 2);
        assert_eq!(sb.last_result, Some(FetchResult::Ok { node_count: 2, skipped_tailscale: 0 }));
        assert_eq!(sb.last_updated, Some(1000));
    }

    #[test]
    fn record_result_carries_skipped_tailscale() {
        let mut sb = sub("http://x/sub");
        let p = parse_subscription("proxies:
  - {name: a, type: ss, server: a.com, port: 1}
  - {name: t, type: tailscale, server: x}
").unwrap();
        record_result(&mut sb, Ok(p), 5);
        assert_eq!(sb.last_result, Some(FetchResult::Ok { node_count: 1, skipped_tailscale: 1 }));
    }

    #[tokio::test]
    async fn failure_keeps_old_nodes() {
        let s = serve("<html><body>expired</body></html>", 200).await;
        let mut sb = sub(&format!("{}/sub", s.uri()));
        sb.nodes = parse_subscription(YAML2).unwrap().nodes;
        let before = sb.nodes.clone();
        let r = update_subscription(&sb, None).await;
        assert!(matches!(r, Err(FetchError::Parse(_))));
        record_result(&mut sb, r, 2000);
        assert_eq!(sb.nodes, before);
        assert!(matches!(sb.last_result, Some(FetchResult::Failed { .. })));
        assert_eq!(sb.last_updated, Some(2000));
    }


    #[tokio::test]
    async fn error_text_does_not_contain_url() {
        let s = serve("nope", 500).await;
        let e = fetch_body(&format!("{}/sub?token=SECRET123", s.uri()), None).await.unwrap_err();
        assert!(!e.to_string().contains("SECRET123"), "{e}");
        let dead = {
            let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            l.local_addr().unwrap().port()
        };
        let url = format!("http://127.0.0.1:{dead}/sub?token=SECRET123");
        let e = fetch_body(&url, None).await.unwrap_err();
        assert!(!e.to_string().contains("SECRET123"), "{e}");
        let sb = sub(&url);
        let e = update_subscription(&sb, Some(dead)).await.unwrap_err();
        assert!(!e.to_string().contains("SECRET123"), "{e}");
    }

    #[tokio::test]
    async fn oversized_body_rejected() {
        let big = "a".repeat(MAX_BODY_BYTES + 1);
        let s = serve(&big, 200).await;
        let r = fetch_body(&format!("{}/sub", s.uri()), None).await;
        assert_eq!(r, Err(FetchError::Http("订阅内容过大（超过 10 MiB）".into())));
    }
    #[tokio::test]
    async fn http_error_status_is_failure() {
        let s = serve("nope", 503).await;
        let r = fetch_body(&format!("{}/sub", s.uri()), None).await;
        assert_eq!(r, Err(FetchError::Http("HTTP 503".into())));
    }

    #[tokio::test]
    async fn sends_clash_meta_user_agent() {
        let s = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/sub"))
            .and(header("user-agent", "clash.meta"))
            .respond_with(ResponseTemplate::new(200).set_body_string(YAML1))
            .expect(1)
            .mount(&s)
            .await;
        fetch_body(&format!("{}/sub", s.uri()), None).await.unwrap();
    }

    #[tokio::test]
    async fn falls_back_to_proxy_when_direct_fails() {
        let direct = serve("blocked", 403).await;
        let proxy = serve(YAML2, 200).await;
        let sb = sub(&format!("{}/sub", direct.uri()));
        let nodes = update_subscription(&sb, Some(port_of(&proxy))).await.unwrap().nodes;
        assert_eq!(nodes.len(), 2);
        assert_eq!(proxy.received_requests().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn no_fallback_when_not_configured() {
        let direct = serve("blocked", 403).await;
        let proxy = serve(YAML2, 200).await;
        let sb = sub(&format!("{}/sub", direct.uri()));
        let r = update_subscription(&sb, None).await;
        assert_eq!(r, Err(FetchError::Http("HTTP 403".into())));
        assert!(proxy.received_requests().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn both_failing_reports_both_errors() {
        let direct = serve("blocked", 403).await;
        let proxy = serve("<html/>", 200).await;
        let sb = sub(&format!("{}/sub", direct.uri()));
        let Err(FetchError::Http(msg)) = update_subscription(&sb, Some(port_of(&proxy))).await
        else {
            panic!("expected Http error");
        };
        assert!(msg.contains("直连失败: HTTP 403"), "{msg}");
        assert!(msg.contains("通过出口失败"), "{msg}");
    }

    fn model_with(exits: &[Option<&ExitId>]) -> Model {
        let subscriptions = exits
            .iter()
            .map(|e| {
                let mut s = sub("http://x/");
                s.fallback_exit = e.cloned();
                s
            })
            .collect();
        Model { subscriptions, ..Default::default() }
    }

    #[test]
    fn ports_stable_across_calls() {
        let (a, b) = (ExitId::new(), ExitId::new());
        let m = model_with(&[Some(&a), None, Some(&b), Some(&a)]);
        let mut alloc = PortAllocator::new();
        let first = fetch_listener_ports(&m, &mut alloc);
        assert_eq!(first.len(), 2);
        assert_eq!(first[0].0, a);
        assert_eq!(first[1].0, b);
        assert_ne!(first[0].1, first[1].1);
        assert_eq!(fetch_listener_ports(&m, &mut alloc), first);
    }

    #[test]
    fn unreferenced_exit_released() {
        let (a, b) = (ExitId::new(), ExitId::new());
        let mut alloc = PortAllocator::new();
        let first = fetch_listener_ports(&model_with(&[Some(&a), Some(&b)]), &mut alloc);
        let second = fetch_listener_ports(&model_with(&[Some(&b)]), &mut alloc);
        assert_eq!(second, vec![first[1].clone()]);
        assert!(!alloc.assigned.contains_key(&a));
    }
}

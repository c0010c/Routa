use super::catalog::{bundled, parse_catalog, Catalog};
use super::AiApps;
use crate::updater::download::MIRRORS;
use std::sync::Arc;
use std::time::Duration;

pub const CATALOG_URL: &str = "https://raw.githubusercontent.com/c0010c/Routa/main/presets/ai-apps.json";
const FETCH_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_BODY: usize = 1024 * 1024;
const REFRESH_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

/// 镜像优先，最后直连 GitHub。
pub fn sources() -> Vec<String> {
    MIRRORS
        .iter()
        .map(|m| format!("{m}{CATALOG_URL}"))
        .chain(std::iter::once(CATALOG_URL.to_string()))
        .collect()
}

/// remote 合法 → (remote, Some(原文，用于写缓存))；否则 cached 合法 → (cached, None)；否则内置。
pub fn choose(remote: Option<&str>, cached: Option<&str>) -> (Catalog, Option<String>) {
    if let Some(r) = remote {
        if let Ok(c) = parse_catalog(r) {
            return (c, Some(r.to_string()));
        }
    }
    if let Some(c) = cached.and_then(|s| parse_catalog(s).ok()) {
        return (c, None);
    }
    (bundled(), None)
}

/// 单个来源的验收：HTTP 200 且正文通过 parse_catalog 才算数。
fn accept(status: reqwest::StatusCode, body: String) -> Option<String> {
    (status == reqwest::StatusCode::OK && parse_catalog(&body).is_ok()).then_some(body)
}

/// 单个来源：正文最多 1 MiB（有 Content-Length 先看它，再看实际读到的长度，任何状态码都一样），
/// 且须 HTTP 200、通过 parse_catalog。
async fn fetch_one(client: &reqwest::Client, url: &str) -> Result<String, String> {
    let too_large = || "rejected: body over 1 MiB".to_string();
    let mut resp = client.get(url).timeout(FETCH_TIMEOUT).send().await.map_err(|e| format!("failed: {e}"))?;
    if resp.content_length().is_some_and(|n| n > MAX_BODY as u64) {
        return Err(too_large());
    }
    let status = resp.status();
    let mut buf = Vec::new();
    while let Some(chunk) = resp.chunk().await.map_err(|e| format!("reading failed: {e}"))? {
        if buf.len() + chunk.len() > MAX_BODY {
            return Err(too_large());
        }
        buf.extend_from_slice(&chunk);
    }
    let body = String::from_utf8(buf).map_err(|_| "rejected: body is not UTF-8".to_string())?;
    accept(status, body).ok_or_else(|| format!("rejected (HTTP {status} or invalid catalog)"))
}

/// 依次尝试 sources()，每个 10 秒超时；只返回 200 且通过校验的正文。
pub async fn fetch_remote(client: &reqwest::Client) -> Option<String> {
    for url in sources() {
        match fetch_one(client, &url).await {
            Ok(t) => return Some(t),
            Err(e) => eprintln!("aiapps: {url} {e}"),
        }
    }
    None
}

/// 立即刷新一次，之后每 24 小时一次。客户端不经过代理。
pub fn spawn_refresh(state: Arc<AiApps>, on_change: impl Fn(&Catalog) + Send + Sync + 'static) {
    tauri::async_runtime::spawn(async move {
        let client = match reqwest::Client::builder().no_proxy().build() {
            Ok(c) => c,
            Err(e) => {
                eprintln!("aiapps: building http client failed: {e}");
                return;
            }
        };
        loop {
            refresh_once(&client, &state, &on_change).await;
            tokio::time::sleep(REFRESH_INTERVAL).await;
        }
    });
}

async fn refresh_once(client: &reqwest::Client, state: &AiApps, on_change: &impl Fn(&Catalog)) {
    let remote = fetch_remote(client).await;
    let cached = std::fs::read_to_string(state.cache_path()).ok();
    let (chosen, to_cache) = choose(remote.as_deref(), cached.as_deref());
    if let Some(raw) = to_cache {
        if let Err(e) = write_cache(state.cache_path(), &raw) {
            eprintln!("aiapps: writing cache failed: {e}");
        }
    }
    if chosen != state.catalog() {
        state.set(chosen.clone());
        on_change(&chosen);
    }
}

fn write_cache(path: &std::path::Path, raw: &str) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, raw)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid(name: &str) -> String {
        let mut c = bundled();
        c.apps[0].name = name.into();
        serde_json::to_string(&c).unwrap()
    }

    #[test]
    fn accept_requires_200_and_valid_catalog() {
        use reqwest::StatusCode;
        let ok = valid("A");
        assert_eq!(accept(StatusCode::OK, ok.clone()), Some(ok.clone()));
        assert!(accept(StatusCode::OK, "<html>captive</html>".into()).is_none());
        assert!(accept(StatusCode::NOT_FOUND, ok).is_none());
    }

    async fn serve(status: u16, body: String) -> wiremock::MockServer {
        use wiremock::{matchers::method, Mock, MockServer, ResponseTemplate};
        let s = MockServer::start().await;
        Mock::given(method("GET")).respond_with(ResponseTemplate::new(status).set_body_string(body)).mount(&s).await;
        s
    }

    /// Answers once without Content-Length; the body ends when the connection closes.
    async fn serve_unsized(body: String) -> String {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = l.local_addr().unwrap();
        tokio::spawn(async move {
            let (mut sock, _) = l.accept().await.unwrap();
            let mut req = [0u8; 1024];
            let _ = sock.read(&mut req).await;
            let head = "HTTP/1.1 200 OK
Connection: close

";
            let _ = sock.write_all(head.as_bytes()).await;
            let _ = sock.write_all(body.as_bytes()).await;
        });
        format!("http://{addr}/")
    }

    #[tokio::test]
    async fn fetch_one_accepts_valid_catalog_under_cap() {
        let s = serve(200, valid("A")).await;
        assert_eq!(fetch_one(&reqwest::Client::new(), &s.uri()).await, Ok(valid("A")));
    }

    #[tokio::test]
    async fn fetch_one_rejects_bodies_over_one_mib() {
        let client = reqwest::Client::new();
        let big = "a".repeat(MAX_BODY + 1);
        for status in [200, 404] {
            let s = serve(status, big.clone()).await;
            let e = fetch_one(&client, &s.uri()).await.unwrap_err();
            assert!(e.contains("1 MiB"), "{status}: {e}");
        }
        let e = fetch_one(&client, &serve_unsized(big).await).await.unwrap_err();
        assert!(e.contains("1 MiB"), "{e}");
    }

    #[test]
    fn sources_try_mirrors_then_github() {
        assert_eq!(
            sources(),
            vec![
                format!("https://gh-proxy.org/{CATALOG_URL}"),
                format!("https://ghfast.top/{CATALOG_URL}"),
                CATALOG_URL.to_string()
            ]
        );
    }

    #[test]
    fn choose_prefers_valid_remote_then_cache_then_bundled() {
        let (a, b) = (valid("A"), valid("B"));
        let (c, w) = choose(Some(&a), Some(&b));
        assert_eq!(c.apps[0].name, "A");
        assert_eq!(w.as_deref(), Some(a.as_str()));

        let (c, w) = choose(Some("garbage"), Some(&b));
        assert_eq!(c.apps[0].name, "B");
        assert!(w.is_none());

        let (c, w) = choose(Some("garbage"), Some("garbage"));
        assert_eq!(c, bundled());
        assert!(w.is_none());

        assert_eq!(choose(None, None).0, bundled());
    }
}

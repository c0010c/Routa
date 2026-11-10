use super::catalog::{bundled, parse_catalog, Catalog};
use super::AiApps;
use crate::updater::download::MIRRORS;
use std::sync::Arc;
use std::time::Duration;

pub const CATALOG_URL: &str = "https://raw.githubusercontent.com/c0010c/Routa/main/presets/ai-apps.json";
const FETCH_TIMEOUT: Duration = Duration::from_secs(10);
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

/// 依次尝试 sources()，每个 10 秒超时，HTTP 200 才返回正文。
pub async fn fetch_remote(client: &reqwest::Client) -> Option<String> {
    for url in sources() {
        let resp = match client.get(&url).timeout(FETCH_TIMEOUT).send().await {
            Ok(r) if r.status() == reqwest::StatusCode::OK => r,
            Ok(r) => {
                eprintln!("aiapps: {url} -> HTTP {}", r.status());
                continue;
            }
            Err(e) => {
                eprintln!("aiapps: {url} failed: {e}");
                continue;
            }
        };
        match resp.text().await {
            Ok(t) => return Some(t),
            Err(e) => eprintln!("aiapps: reading {url} failed: {e}"),
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

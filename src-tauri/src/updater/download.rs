//! Downloads the installer announced by the updater plugin, rotating through
//! GitHub mirrors (same scheme as Clash Verge Rev).

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use tauri_plugin_updater::Update;
use url::Url;

/// GitHub download mirrors, tried in this order before GitHub itself.
pub const MIRRORS: [&str; 2] = ["https://gh-proxy.org/", "https://ghfast.top/"];

/// Minimum bytes per window: first a throughput floor, then only a stall check
/// so a link that is slow everywhere still finishes.
const FLOORS: [u64; 2] = [1 << 20, 1];
const WINDOW: Duration = Duration::from_secs(30);

/// The given URL first, then the other mirrors, then GitHub directly.
/// Non-GitHub URLs have no alternatives.
pub fn sources(url: &Url) -> Vec<Url> {
    let origin = MIRRORS
        .iter()
        .find_map(|m| url.as_str().strip_prefix(m))
        .unwrap_or(url.as_str());
    let mut out = vec![url.clone()];
    if origin.starts_with("https://github.com/") {
        let candidates = MIRRORS
            .iter()
            .map(|m| format!("{m}{origin}"))
            .chain([origin.to_owned()]);
        for c in candidates {
            if let Ok(c) = Url::parse(&c) {
                if !out.contains(&c) {
                    out.push(c);
                }
            }
        }
    }
    out
}

/// Downloads `update` from each source in turn, two rounds (see `FLOORS`).
/// The plugin verifies the signature before returning the bytes. Progress
/// reports bytes received from the current source and its Content-Length.
pub async fn fetch(
    update: &Update,
    on_progress: impl Fn(u64, Option<u64>) + Sync,
) -> Result<Vec<u8>, String> {
    let sources = sources(&update.download_url);
    let mut last_error = None;
    for floor in FLOORS {
        for url in &sources {
            eprintln!("updater: downloading {} from {url}", update.version);
            match fetch_from(update, url, floor, &on_progress).await {
                Ok(bytes) => return Ok(bytes),
                Err(e) => {
                    eprintln!("updater: download from {url} failed: {e}");
                    last_error = Some(e);
                }
            }
        }
    }
    Err(last_error.unwrap_or_else(|| "没有可用的下载地址".to_string()))
}

async fn fetch_from(
    update: &Update,
    url: &Url,
    floor: u64,
    on_progress: &(impl Fn(u64, Option<u64>) + Sync),
) -> Result<Vec<u8>, String> {
    let mut source = update.clone();
    source.download_url = url.clone();
    let received = AtomicU64::new(0);
    on_progress(0, None);
    let download = source.download(
        |chunk, total| {
            let now = received.fetch_add(chunk as u64, Ordering::Relaxed) + chunk as u64;
            on_progress(now, total);
        },
        || {},
    );
    tokio::pin!(download);
    let mut seen = 0;
    loop {
        tokio::select! {
            result = &mut download => return result.map_err(|e| e.to_string()),
            () = tokio::time::sleep(WINDOW) => {
                let now = received.load(Ordering::Relaxed);
                if now - seen < floor {
                    return Err(format!("{WINDOW:?} 内只收到 {} 字节", now - seen));
                }
                seen = now;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sources_start_with_given_url_then_other_mirror_then_github() {
        let gh =
            "https://github.com/c0010c/Routa/releases/download/v0.4.0/routa_0.4.0_x64-setup.exe";
        let u = url::Url::parse(&format!("https://gh-proxy.org/{gh}")).unwrap();
        let got: Vec<String> = sources(&u).iter().map(|u| u.to_string()).collect();
        assert_eq!(
            got,
            vec![
                format!("https://gh-proxy.org/{gh}"),
                format!("https://ghfast.top/{gh}"),
                gh.to_string(),
            ]
        );
    }

    #[test]
    fn non_github_url_has_no_alternatives() {
        let u = url::Url::parse("https://example.com/a.exe").unwrap();
        assert_eq!(sources(&u), vec![u]);
    }
}

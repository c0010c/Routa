//! Concurrent proxy delay testing.

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use futures_util::stream::{self, StreamExt};
use serde::{Deserialize, Serialize};

use crate::core::api::{ApiError, MihomoApi};

/// Each dead node holds a slot for the whole timeout, so a wide pool keeps a
/// few dead nodes from stalling the live ones queued behind them.
pub const DEFAULT_CONCURRENCY: usize = 32;
/// Working nodes answer in well under 5 s; a longer wait only delays the
/// verdict on dead ones.
pub const DEFAULT_TIMEOUT_MS: u32 = 5000;

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, specta::Type)]
pub struct DelayResult {
    /// `None` means the test failed or timed out.
    pub ms: Option<u32>,
    /// Unix seconds at completion.
    pub tested_at: i64,
    /// Why `ms` is `None`.
    #[serde(default)]
    pub error: Option<DelayError>,
    /// The measured path/test settings changed after this result was produced.
    #[serde(default)]
    #[specta(optional)]
    pub stale: bool,
}

/// Why a delay test produced no number. mihomo's delay endpoint only tells
/// these apart by status: 504 timeout, 503 any dial/DNS/HTTP failure, 404
/// no such proxy.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, specta::Type)]
pub enum DelayError {
    Timeout,
    /// The proxy could not reach the test URL (connect, DNS or HTTP error).
    Failed,
    /// mihomo holds no proxy by that name (config not applied yet).
    NotFound,
    /// No mihomo running.
    NoCore,
    /// The controller did not answer.
    CoreUnreachable(String),
    Other(String),
}

impl From<ApiError> for DelayError {
    fn from(e: ApiError) -> Self {
        match e {
            ApiError::Rejected { status: 504 | 408, .. } => DelayError::Timeout,
            ApiError::Rejected { status: 503, .. } => DelayError::Failed,
            ApiError::Rejected { status: 404, .. } => DelayError::NotFound,
            ApiError::Rejected { status, message } => DelayError::Other(format!("{status}: {message}")),
            ApiError::Unreachable(m) => DelayError::CoreUnreachable(m),
        }
    }
}

impl DelayResult {
    pub fn failed(error: DelayError, tested_at: i64) -> Self {
        Self { ms: None, tested_at, error: Some(error), stale: false }
    }
}

/// Latest delay result per proxy name.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, specta::Type)]
pub struct DelayBook(pub HashMap<String, DelayResult>);

impl DelayBook {
    pub fn get(&self, name: &str) -> Option<&DelayResult> {
        self.0.get(name)
    }

    pub fn record(&mut self, name: String, result: DelayResult) {
        self.0.insert(name, result);
    }
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Tests `names` with at most `concurrency` requests in flight. Errors and
/// timeouts become `ms: None` with the reason. Results are in input order.
pub async fn test_many(
    api: &dyn MihomoApi,
    names: &[String],
    url: &str,
    concurrency: usize,
    timeout_ms: u32,
) -> Vec<(String, DelayResult)> {
    test_many_each(api, names, url, concurrency, timeout_ms, |_, _| {}).await
}

/// `test_many`, calling `on_result` as each test finishes (completion order),
/// so callers can show results before the slowest node answers.
pub async fn test_many_each(
    api: &dyn MihomoApi,
    names: &[String],
    url: &str,
    concurrency: usize,
    timeout_ms: u32,
    mut on_result: impl FnMut(&str, &DelayResult),
) -> Vec<(String, DelayResult)> {
    let mut done: Vec<Option<DelayResult>> = vec![None; names.len()];
    let mut tests = stream::iter(names.iter().cloned().enumerate())
        .map(|(i, name)| async move {
            let r = match api.delay(&name, url, timeout_ms).await {
                Ok(ms) => DelayResult { ms: Some(ms), tested_at: now_unix(), error: None, stale: false },
                Err(e) => DelayResult::failed(e.into(), now_unix()),
            };
            (i, r)
        })
        .buffer_unordered(concurrency.max(1));
    while let Some((i, r)) = tests.next().await {
        on_result(&names[i], &r);
        done[i] = Some(r);
    }
    names.iter().cloned().zip(done.into_iter().map(|r| r.expect("every test yields a result"))).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::fake::FakeApi;

    #[tokio::test]
    async fn test_many_marks_timeouts_as_none() {
        let api = FakeApi::new();
        api.set_delay("a", Ok(120));
        api.set_delay("b", Err(ApiError::Unreachable("timeout".into())));
        api.set_delay("c", Ok(45));
        let names: Vec<String> = ["a", "b", "c"].iter().map(|s| s.to_string()).collect();
        let out = test_many(&api, &names, "http://x", 2, 5000).await;
        let got: Vec<(&str, Option<u32>)> = out.iter().map(|(n, r)| (n.as_str(), r.ms)).collect();
        assert_eq!(got, vec![("a", Some(120)), ("b", None), ("c", Some(45))]);
        assert_eq!(out[0].1.error, None);
        assert_eq!(out[1].1.error, Some(DelayError::CoreUnreachable("timeout".into())));
        assert!(out.iter().all(|(_, r)| r.tested_at > 0));
        let calls = api.delay_calls.lock().unwrap();
        assert_eq!(calls.len(), 3);
        assert!(calls.iter().all(|c| c.1 == "http://x" && c.2 == 5000));
    }

    #[tokio::test]
    async fn test_many_empty_and_zero_concurrency() {
        let api = FakeApi::new();
        assert!(test_many(&api, &[], "u", 16, 5000).await.is_empty());
        api.set_delay("a", Ok(1));
        let out = test_many(&api, &["a".to_string()], "u", 0, 5000).await;
        assert_eq!(out[0].1.ms, Some(1));
    }

    #[tokio::test]
    async fn test_many_each_reports_every_result_once() {
        let api = FakeApi::new();
        api.set_delay("a", Ok(10));
        api.set_delay("b", Err(ApiError::Rejected { status: 504, message: "Timeout".into() }));
        let names: Vec<String> = ["a", "b"].iter().map(|s| s.to_string()).collect();
        let mut seen = Vec::new();
        let out = test_many_each(&api, &names, "u", 2, 5000, |n, r| seen.push((n.to_string(), r.ms))).await;
        seen.sort();
        assert_eq!(seen, vec![("a".to_string(), Some(10)), ("b".to_string(), None)]);
        assert_eq!(out.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>(), ["a", "b"]);
    }

    #[test]
    fn api_errors_map_to_delay_errors() {
        let rej = |status, message: &str| ApiError::Rejected { status, message: message.into() };
        assert_eq!(DelayError::from(rej(504, "Timeout")), DelayError::Timeout);
        assert_eq!(
            DelayError::from(rej(503, "An error occurred in the delay test")),
            DelayError::Failed
        );
        assert_eq!(DelayError::from(rej(404, "Resource not found")), DelayError::NotFound);
        assert_eq!(DelayError::from(rej(400, "bad")), DelayError::Other("400: bad".into()));
        assert_eq!(
            DelayError::from(ApiError::Unreachable("refused".into())),
            DelayError::CoreUnreachable("refused".into())
        );
    }

    #[test]
    fn delay_result_without_error_field_deserializes() {
        let r: DelayResult = serde_json::from_str(r#"{"ms":null,"tested_at":1}"#).unwrap();
        assert_eq!(r.error, None);
        assert!(!r.stale);
    }

    #[test]
    fn stale_delay_is_optional_in_generated_frontend_contract() {
        let generated = specta_typescript::export::<DelayResult>(
            &specta_typescript::Typescript::default().bigint(specta_typescript::BigIntExportBehavior::Number),
        ).unwrap();
        assert!(generated.contains("stale?: boolean"), "{generated}");
    }

    #[test]
    fn delay_book_records_and_gets() {
        let mut book = DelayBook::default();
        assert!(book.get("a").is_none());
        let r = DelayResult {
            ms: Some(10),
            tested_at: 5,
            error: None,
            stale: false,
        };
        book.record("a".into(), r.clone());
        assert_eq!(book.get("a"), Some(&r));
        book.record(
            "a".into(),
            DelayResult::failed(DelayError::Timeout, 6),
        );
        assert_eq!(book.get("a").unwrap().ms, None);
    }
}

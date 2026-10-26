//! Concurrent proxy delay testing.

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use futures_util::stream::{self, StreamExt};
use serde::{Deserialize, Serialize};

use crate::core::api::MihomoApi;

pub const DEFAULT_CONCURRENCY: usize = 16;
pub const DEFAULT_TIMEOUT_MS: u32 = 5000;

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, specta::Type)]
pub struct DelayResult {
    /// `None` means the test failed or timed out.
    pub ms: Option<u32>,
    /// Unix seconds at completion.
    pub tested_at: i64,
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
/// timeouts become `ms: None`. Results are in input order.
pub async fn test_many(
    api: &dyn MihomoApi,
    names: &[String],
    url: &str,
    concurrency: usize,
    timeout_ms: u32,
) -> Vec<(String, DelayResult)> {
    stream::iter(names.iter().cloned())
        .map(|name| async move {
            let ms = api.delay(&name, url, timeout_ms).await.ok();
            (
                name,
                DelayResult {
                    ms,
                    tested_at: now_unix(),
                },
            )
        })
        .buffered(concurrency.max(1))
        .collect()
        .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::api::ApiError;
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

    #[test]
    fn delay_book_records_and_gets() {
        let mut book = DelayBook::default();
        assert!(book.get("a").is_none());
        let r = DelayResult {
            ms: Some(10),
            tested_at: 5,
        };
        book.record("a".into(), r.clone());
        assert_eq!(book.get("a"), Some(&r));
        book.record(
            "a".into(),
            DelayResult {
                ms: None,
                tested_at: 6,
            },
        );
        assert_eq!(book.get("a").unwrap().ms, None);
    }
}

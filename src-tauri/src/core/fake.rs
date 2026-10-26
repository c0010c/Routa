//! Test double for `MihomoApi`, shared by apply/refresh/health tests.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use async_trait::async_trait;
use futures_util::stream::{self, BoxStream, StreamExt};

use super::api::{ApiError, ConnectionsSnapshot, MihomoApi};

#[derive(Default)]
pub struct FakeApi {
    /// Reload results consumed in call order; once exhausted every reload is Ok.
    pub reload_results: Mutex<VecDeque<Result<(), ApiError>>>,
    /// (path, file content at the moment of the call; empty if unreadable).
    pub reloads: Mutex<Vec<(PathBuf, String)>>,
    pub connections: Mutex<ConnectionsSnapshot>,
    /// When set, `connections()` fails with this error.
    pub connections_error: Mutex<Option<ApiError>>,
    pub closed: Mutex<Vec<String>>,
    /// Connection ids whose `close_connection` fails.
    pub close_fails: Mutex<HashSet<String>>,
    pub selects: Mutex<Vec<(String, String)>>,
    /// (proxy, url, timeout_ms) per delay call.
    pub delay_calls: Mutex<Vec<(String, String, u32)>>,
    /// Per-proxy delay result; unknown proxies are `Unreachable`.
    pub delay_results: Mutex<HashMap<String, Result<u32, ApiError>>>,
    /// `None` behaves as `Ok("v1.19.33")`.
    pub version_result: Mutex<Option<Result<String, ApiError>>>,
}

impl FakeApi {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_reload_results(results: Vec<Result<(), ApiError>>) -> Self {
        let api = Self::new();
        *api.reload_results.lock().unwrap() = results.into();
        api
    }

    pub fn set_connections(&self, snapshot: ConnectionsSnapshot) {
        *self.connections.lock().unwrap() = snapshot;
    }

    pub fn set_delay(&self, proxy: &str, result: Result<u32, ApiError>) {
        self.delay_results
            .lock()
            .unwrap()
            .insert(proxy.to_string(), result);
    }

    pub fn set_version(&self, result: Result<String, ApiError>) {
        *self.version_result.lock().unwrap() = Some(result);
    }
}

#[async_trait]
impl MihomoApi for FakeApi {
    async fn version(&self) -> Result<String, ApiError> {
        self.version_result
            .lock()
            .unwrap()
            .clone()
            .unwrap_or_else(|| Ok("v1.19.33".into()))
    }

    async fn reload(&self, config_path: &Path) -> Result<(), ApiError> {
        let content = std::fs::read_to_string(config_path).unwrap_or_default();
        self.reloads
            .lock()
            .unwrap()
            .push((config_path.to_path_buf(), content));
        self.reload_results
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or(Ok(()))
    }

    async fn connections(&self) -> Result<ConnectionsSnapshot, ApiError> {
        if let Some(e) = self.connections_error.lock().unwrap().clone() {
            return Err(e);
        }
        Ok(self.connections.lock().unwrap().clone())
    }

    async fn close_connection(&self, id: &str) -> Result<(), ApiError> {
        if self.close_fails.lock().unwrap().contains(id) {
            return Err(ApiError::Rejected {
                status: 404,
                message: "no such connection".into(),
            });
        }
        self.closed.lock().unwrap().push(id.to_string());
        Ok(())
    }

    async fn delay(&self, proxy: &str, url: &str, timeout_ms: u32) -> Result<u32, ApiError> {
        self.delay_calls
            .lock()
            .unwrap()
            .push((proxy.to_string(), url.to_string(), timeout_ms));
        self.delay_results
            .lock()
            .unwrap()
            .get(proxy)
            .cloned()
            .unwrap_or_else(|| Err(ApiError::Unreachable("no delay configured".into())))
    }

    async fn select(&self, group: &str, member: &str) -> Result<(), ApiError> {
        self.selects
            .lock()
            .unwrap()
            .push((group.to_string(), member.to_string()));
        Ok(())
    }

    fn traffic_stream(&self) -> BoxStream<'static, Result<(u64, u64), ApiError>> {
        stream::empty().boxed()
    }
}

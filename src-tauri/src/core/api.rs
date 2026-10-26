//! mihomo REST API client (external-controller).

use std::fmt;
use std::path::Path;

use async_trait::async_trait;
use futures_util::stream::{self, BoxStream};
use futures_util::StreamExt;
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub enum ApiError {
    /// Connection refused / timed out / bad transport.
    Unreachable(String),
    /// mihomo answered with a non-2xx status.
    Rejected { status: u16, message: String },
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ApiError::Unreachable(m) => write!(f, "mihomo unreachable: {m}"),
            ApiError::Rejected { status, message } => {
                write!(f, "mihomo rejected request ({status}): {message}")
            }
        }
    }
}

impl std::error::Error for ApiError {}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct ConnMeta {
    #[serde(rename = "inboundName", default)]
    pub inbound_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Conn {
    pub id: String,
    #[serde(default)]
    pub upload: u64,
    #[serde(default)]
    pub download: u64,
    #[serde(default)]
    pub metadata: ConnMeta,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConnectionsSnapshot {
    pub upload_total: u64,
    pub download_total: u64,
    pub connections: Vec<Conn>,
}

#[derive(Deserialize)]
struct RawSnapshot {
    #[serde(rename = "uploadTotal", default)]
    upload_total: u64,
    #[serde(rename = "downloadTotal", default)]
    download_total: u64,
    // mihomo sends `null` when there are no connections.
    #[serde(default)]
    connections: Option<Vec<Conn>>,
}

#[async_trait]
pub trait MihomoApi: Send + Sync {
    async fn version(&self) -> Result<String, ApiError>;
    /// `PUT /configs?force=true`. `Ok` only means mihomo accepted the config:
    /// per spike S5 it answers 204 even when a listener failed to bind (and
    /// the old listener on a moved port is gone). Callers MUST verify each
    /// listener's port owner afterwards.
    async fn reload(&self, config_path: &Path) -> Result<(), ApiError>;
    async fn connections(&self) -> Result<ConnectionsSnapshot, ApiError>;
    async fn close_connection(&self, id: &str) -> Result<(), ApiError>;
    async fn delay(&self, proxy: &str, url: &str, timeout_ms: u32) -> Result<u32, ApiError>;
    async fn select(&self, group: &str, member: &str) -> Result<(), ApiError>;
    /// Yields `(up, down)` bytes per second.
    fn traffic_stream(&self) -> BoxStream<'static, Result<(u64, u64), ApiError>>;
}

pub struct HttpMihomoApi {
    base: String,
    port: u16,
    secret: String,
    client: reqwest::Client,
}

fn enc(s: &str) -> String {
    utf8_percent_encode(s, NON_ALPHANUMERIC).to_string()
}

fn unreachable(e: impl fmt::Display) -> ApiError {
    ApiError::Unreachable(e.to_string())
}

impl HttpMihomoApi {
    pub fn new(port: u16, secret: String) -> Self {
        Self {
            base: format!("http://127.0.0.1:{port}"),
            port,
            secret,
            client: reqwest::Client::new(),
        }
    }

    async fn send(&self, req: reqwest::RequestBuilder) -> Result<reqwest::Response, ApiError> {
        let resp = req
            .bearer_auth(&self.secret)
            .send()
            .await
            .map_err(unreachable)?;
        let status = resp.status();
        if status.is_success() {
            return Ok(resp);
        }
        let body = resp.text().await.unwrap_or_default();
        let message = serde_json::from_str::<serde_json::Value>(&body)
            .ok()
            .and_then(|v| v.get("message").and_then(|m| m.as_str().map(str::to_owned)))
            .unwrap_or(body);
        Err(ApiError::Rejected {
            status: status.as_u16(),
            message,
        })
    }

    async fn json<T: serde::de::DeserializeOwned>(
        &self,
        req: reqwest::RequestBuilder,
    ) -> Result<T, ApiError> {
        self.send(req).await?.json().await.map_err(unreachable)
    }
}

#[async_trait]
impl MihomoApi for HttpMihomoApi {
    async fn version(&self) -> Result<String, ApiError> {
        #[derive(Deserialize)]
        struct V {
            version: String,
        }
        let v: V = self
            .json(self.client.get(format!("{}/version", self.base)))
            .await?;
        Ok(v.version)
    }

    async fn reload(&self, config_path: &Path) -> Result<(), ApiError> {
        let path = config_path.to_string_lossy().replace('\\', "/");
        self.send(
            self.client
                .put(format!("{}/configs?force=true", self.base))
                .json(&serde_json::json!({ "path": path })),
        )
        .await
        .map(|_| ())
    }

    async fn connections(&self) -> Result<ConnectionsSnapshot, ApiError> {
        let raw: RawSnapshot = self
            .json(self.client.get(format!("{}/connections", self.base)))
            .await?;
        Ok(ConnectionsSnapshot {
            upload_total: raw.upload_total,
            download_total: raw.download_total,
            connections: raw.connections.unwrap_or_default(),
        })
    }

    async fn close_connection(&self, id: &str) -> Result<(), ApiError> {
        self.send(
            self.client
                .delete(format!("{}/connections/{}", self.base, enc(id))),
        )
        .await
        .map(|_| ())
    }

    async fn delay(&self, proxy: &str, url: &str, timeout_ms: u32) -> Result<u32, ApiError> {
        #[derive(Deserialize)]
        struct D {
            delay: u32,
        }
        let d: D = self
            .json(self.client.get(format!(
                "{}/proxies/{}/delay?url={}&timeout={}",
                self.base,
                enc(proxy),
                enc(url),
                timeout_ms
            )))
            .await?;
        Ok(d.delay)
    }

    async fn select(&self, group: &str, member: &str) -> Result<(), ApiError> {
        self.send(
            self.client
                .put(format!("{}/proxies/{}", self.base, enc(group)))
                .json(&serde_json::json!({ "name": member })),
        )
        .await
        .map(|_| ())
    }

    fn traffic_stream(&self) -> BoxStream<'static, Result<(u64, u64), ApiError>> {
        use tokio_tungstenite::tungstenite::Message;
        #[derive(Deserialize)]
        struct T {
            up: u64,
            down: u64,
        }
        let url = format!(
            "ws://127.0.0.1:{}/traffic?token={}",
            self.port,
            enc(&self.secret)
        );
        let connect = async move {
            match tokio_tungstenite::connect_async(url).await {
                Ok((ws, _)) => ws
                    .filter_map(|msg| async move {
                        match msg {
                            Ok(Message::Text(t)) => Some(
                                serde_json::from_str::<T>(t.as_str())
                                    .map(|t| (t.up, t.down))
                                    .map_err(unreachable),
                            ),
                            Ok(_) => None,
                            Err(e) => Some(Err(unreachable(e))),
                        }
                    })
                    .boxed(),
                Err(e) => stream::once(async move { Err(unreachable(e)) }).boxed(),
            }
        };
        stream::once(connect).flatten().boxed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{body_json, header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn api_for(server: &MockServer) -> HttpMihomoApi {
        let port = server.address().port();
        HttpMihomoApi::new(port, "s3cret".into())
    }

    #[tokio::test]
    async fn api_parses_connections() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/connections"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "uploadTotal": 7, "downloadTotal": 9,
                "connections": [
                    {"id": "a", "upload": 1, "download": 2, "metadata": {"inboundName": "l-1", "host": "x"}},
                    {"id": "b", "upload": 3, "download": 4, "metadata": {}}
                ]
            })))
            .mount(&server)
            .await;
        let snap = api_for(&server).connections().await.unwrap();
        assert_eq!(snap.upload_total, 7);
        assert_eq!(snap.download_total, 9);
        assert_eq!(snap.connections.len(), 2);
        assert_eq!(snap.connections[0].id, "a");
        assert_eq!(snap.connections[0].metadata.inbound_name, "l-1");
        assert_eq!(snap.connections[1].download, 4);
        assert_eq!(snap.connections[1].metadata.inbound_name, "");
    }

    #[tokio::test]
    async fn null_connections_is_empty() {
        let server = MockServer::start().await;
        Mock::given(path("/connections"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!({"uploadTotal": 0, "downloadTotal": 0, "connections": null}),
            ))
            .mount(&server)
            .await;
        let snap = api_for(&server).connections().await.unwrap();
        assert!(snap.connections.is_empty());
    }

    #[tokio::test]
    async fn api_sends_bearer_and_encodes_name() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(header("authorization", "Bearer s3cret"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"delay": 42})),
            )
            .mount(&server)
            .await;
        let d = api_for(&server)
            .delay("日本/节点 1", "https://www.gstatic.com/generate_204", 5000)
            .await
            .unwrap();
        assert_eq!(d, 42);
        let reqs = server.received_requests().await.unwrap();
        let raw = reqs[0].url.as_str();
        assert!(
            raw.contains("/proxies/%E6%97%A5%E6%9C%AC%2F%E8%8A%82%E7%82%B9%201/delay?"),
            "{raw}"
        );
        let q: std::collections::HashMap<_, _> = reqs[0].url.query_pairs().into_owned().collect();
        assert_eq!(q["url"], "https://www.gstatic.com/generate_204");
        assert_eq!(q["timeout"], "5000");
    }

    #[tokio::test]
    async fn reload_sends_forward_slash_path() {
        let server = MockServer::start().await;
        Mock::given(method("PUT"))
            .and(path("/configs"))
            .and(query_param("force", "true"))
            .and(header("authorization", "Bearer s3cret"))
            .and(body_json(serde_json::json!({"path": "C:/w/config.yaml"})))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&server)
            .await;
        api_for(&server)
            .reload(Path::new(r"C:\w\config.yaml"))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn reload_rejected_maps_error() {
        let server = MockServer::start().await;
        Mock::given(method("PUT"))
            .and(path("/configs"))
            .respond_with(
                ResponseTemplate::new(400)
                    .set_body_json(serde_json::json!({"message": "path is not subpath of home"})),
            )
            .mount(&server)
            .await;
        let err = api_for(&server)
            .reload(Path::new("C:/x/config.yaml"))
            .await
            .unwrap_err();
        assert_eq!(
            err,
            ApiError::Rejected {
                status: 400,
                message: "path is not subpath of home".into()
            }
        );
    }

    #[tokio::test]
    async fn non_json_error_body_is_used_raw() {
        let server = MockServer::start().await;
        Mock::given(path("/version"))
            .respond_with(ResponseTemplate::new(401).set_body_string("Unauthorized"))
            .mount(&server)
            .await;
        let err = api_for(&server).version().await.unwrap_err();
        assert_eq!(
            err,
            ApiError::Rejected {
                status: 401,
                message: "Unauthorized".into()
            }
        );
    }

    #[tokio::test]
    async fn select_and_close_encode_and_succeed() {
        let server = MockServer::start().await;
        Mock::given(method("PUT"))
            .and(body_json(serde_json::json!({"name": "节点/1"})))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("DELETE"))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&server)
            .await;
        let api = api_for(&server);
        api.select("组 A/B", "节点/1").await.unwrap();
        api.close_connection("id-1").await.unwrap();
        let reqs = server.received_requests().await.unwrap();
        assert!(reqs[0].url.as_str().contains("/proxies/%E7%BB%84%20A%2FB"));
        assert!(reqs[1].url.path().ends_with("/connections/id%2D1"));
    }

    #[tokio::test]
    async fn unreachable_when_nothing_listens() {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        drop(l);
        let err = HttpMihomoApi::new(port, "x".into())
            .version()
            .await
            .unwrap_err();
        assert!(matches!(err, ApiError::Unreachable(_)), "{err:?}");
    }

    #[tokio::test]
    async fn traffic_stream_yields_up_down() {
        use futures_util::SinkExt;
        use tokio_tungstenite::tungstenite::Message;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            let (sock, _) = listener.accept().await.unwrap();
            let mut ws = tokio_tungstenite::accept_async(sock).await.unwrap();
            ws.send(Message::text(r#"{"up":5,"down":6}"#)).await.unwrap();
            ws.send(Message::text(r#"{"up":7,"down":8}"#)).await.unwrap();
            ws.close(None).await.unwrap();
        });
        let api = HttpMihomoApi::new(port, "tok".into());
        let got: Vec<_> = api.traffic_stream().take(2).collect().await;
        assert_eq!(got, vec![Ok((5, 6)), Ok((7, 8))]);
    }
}

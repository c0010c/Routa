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
    /// `GET /proxies/{name}`: the member a group currently uses (`now`);
    /// `None` when the proxy has no `now` (not a group).
    async fn proxy_now(&self, name: &str) -> Result<Option<String>, ApiError>;
    /// Yields `(up, down)` bytes per second.
    fn traffic_stream(&self) -> BoxStream<'static, Result<(u64, u64), ApiError>>;
    /// `/logs?level=info` websocket: each item is one log message text. It
    /// carries info events even when the config's `log-level` is higher.
    fn log_stream(&self) -> BoxStream<'static, Result<String, ApiError>>;
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

/// Connects to a controller websocket (5 s dial timeout) and yields each text
/// message parsed as `T`, mapped by `f`. Ends when the socket closes.
fn ws_json_stream<T, R>(url: String, f: fn(T) -> R) -> BoxStream<'static, Result<R, ApiError>>
where
    T: serde::de::DeserializeOwned + Send + 'static,
    R: Send + 'static,
{
    use tokio_tungstenite::tungstenite::Message;
    let connect = async move {
        // tokio-tungstenite dials the address directly (no proxy support), so
        // the secret never goes through a system proxy.
        let dial = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            tokio_tungstenite::connect_async(url),
        )
        .await
        .unwrap_or_else(|_| {
            Err(tokio_tungstenite::tungstenite::Error::Io(
                std::io::ErrorKind::TimedOut.into(),
            ))
        });
        match dial {
            Ok((ws, _)) => ws
                .filter_map(move |msg| async move {
                    match msg {
                        Ok(Message::Text(t)) => Some(
                            serde_json::from_str::<T>(t.as_str())
                                .map(f)
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

impl HttpMihomoApi {
    pub fn new(port: u16, secret: String) -> Self {
        Self {
            base: format!("http://127.0.0.1:{port}"),
            port,
            secret,
            // no_proxy: loopback controller traffic (with the Bearer secret) must
            // never go through HTTP_PROXY/system proxies.
            client: reqwest::Client::builder()
                .no_proxy()
                .timeout(std::time::Duration::from_secs(5))
                .connect_timeout(std::time::Duration::from_secs(2))
                .build()
                .expect("reqwest client"),
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
            .json(
                self.client
                    .get(format!(
                        "{}/proxies/{}/delay?url={}&timeout={}",
                        self.base,
                        enc(proxy),
                        enc(url),
                        timeout_ms
                    ))
                    .timeout(std::time::Duration::from_millis(
                        u64::from(timeout_ms) + 2000,
                    )),
            )
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

    async fn proxy_now(&self, name: &str) -> Result<Option<String>, ApiError> {
        #[derive(Deserialize)]
        struct P {
            #[serde(default)]
            now: Option<String>,
        }
        let p: P = self
            .json(self.client.get(format!("{}/proxies/{}", self.base, enc(name))))
            .await?;
        Ok(p.now.filter(|n| !n.is_empty()))
    }

    fn traffic_stream(&self) -> BoxStream<'static, Result<(u64, u64), ApiError>> {
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
        ws_json_stream(url, |t: T| (t.up, t.down))
    }

    fn log_stream(&self) -> BoxStream<'static, Result<String, ApiError>> {
        #[derive(Deserialize)]
        struct L {
            payload: String,
        }
        let url = format!(
            "ws://127.0.0.1:{}/logs?token={}&level=info",
            self.port,
            enc(&self.secret)
        );
        ws_json_stream(url, |t: L| t.payload)
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
    async fn api_proxy_now_parses_now() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(header("authorization", "Bearer s3cret"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!({"name": "组/A", "type": "Selector", "now": "g/x", "all": ["g/x"]}),
            ))
            .mount(&server)
            .await;
        let api = api_for(&server);
        assert_eq!(api.proxy_now("组/A").await.unwrap(), Some("g/x".into()));
        let reqs = server.received_requests().await.unwrap();
        assert!(reqs[0].url.path().ends_with("/proxies/%E7%BB%84%2FA"), "{}", reqs[0].url);
    }

    #[tokio::test]
    async fn api_proxy_now_absent_is_none() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"name": "n", "type": "Ss"})))
            .mount(&server)
            .await;
        assert_eq!(api_for(&server).proxy_now("n").await.unwrap(), None);
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
        let (tx, rx) = tokio::sync::oneshot::channel::<String>();
        tokio::spawn(async move {
            let (sock, _) = listener.accept().await.unwrap();
            let uri = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
            let seen = uri.clone();
            let mut ws = tokio_tungstenite::accept_hdr_async(
                sock,
                move |req: &tokio_tungstenite::tungstenite::handshake::server::Request, resp| {
                    *seen.lock().unwrap() = req.uri().to_string();
                    Ok(resp)
                },
            )
            .await
            .unwrap();
            tx.send(uri.lock().unwrap().clone()).unwrap();
            ws.send(Message::text(r#"{"up":5,"down":6}"#))
                .await
                .unwrap();
            ws.send(Message::text(r#"{"up":7,"down":8}"#))
                .await
                .unwrap();
            ws.close(None).await.unwrap();
        });
        let api = HttpMihomoApi::new(port, "tok".into());
        let got: Vec<_> = api.traffic_stream().take(2).collect().await;
        assert_eq!(got, vec![Ok((5, 6)), Ok((7, 8))]);
        assert_eq!(rx.await.unwrap(), "/traffic?token=tok");
    }

    #[tokio::test]
    async fn log_stream_yields_info_payloads() {
        use futures_util::SinkExt;
        use tokio_tungstenite::tungstenite::Message;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let (tx, rx) = tokio::sync::oneshot::channel::<String>();
        tokio::spawn(async move {
            let (sock, _) = listener.accept().await.unwrap();
            let uri = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
            let seen = uri.clone();
            let mut ws = tokio_tungstenite::accept_hdr_async(
                sock,
                move |req: &tokio_tungstenite::tungstenite::handshake::server::Request, resp| {
                    *seen.lock().unwrap() = req.uri().to_string();
                    Ok(resp)
                },
            )
            .await
            .unwrap();
            tx.send(uri.lock().unwrap().clone()).unwrap();
            // Shape captured from mihomo v1.19.33 (spike/work/t13/logs-api.log).
            ws.send(Message::text(
                r#"{"type":"info","payload":"[Tailscale](手动/ts-x) LocalBackend state is NeedsLogin; running StartLoginInteractive..."}"#,
            ))
            .await
            .unwrap();
            ws.send(Message::text(r#"{"type":"warning","payload":"say \"hi\""}"#))
                .await
                .unwrap();
            ws.close(None).await.unwrap();
        });
        let api = HttpMihomoApi::new(port, "t/k".into());
        let got: Vec<_> = api.log_stream().take(2).collect().await;
        assert_eq!(
            got,
            vec![
                Ok("[Tailscale](手动/ts-x) LocalBackend state is NeedsLogin; running StartLoginInteractive...".to_string()),
                Ok(r#"say "hi""#.to_string()),
            ]
        );
        assert_eq!(rx.await.unwrap(), "/logs?token=t%2Fk&level=info");
    }
}

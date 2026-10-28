//! Routa 与进程分流服务（RoutaProcSvc）之间命名管道协议：消息类型与分帧。

use serde::{de::DeserializeOwned, Deserialize, Serialize};

pub const PROTO_VERSION: u32 = 1;
pub const PIPE_NAME: &str = r"\\.\pipe\RoutaProcSvc";
pub const SERVICE_NAME: &str = "RoutaProcSvc";
/// 单帧 JSON 正文的最大字节数。
pub const MAX_FRAME: usize = 1 << 20;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum FaultPolicy {
    Direct,
    Block,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum ExitPolicy {
    Direct,
    Block,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum ProxyKind {
    Socks5,
    Http,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct ProxyTarget {
    pub kind: ProxyKind,
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum Action {
    Proxy(ProxyTarget),
    Direct,
    Block,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct RuleSpec {
    pub id: String,
    pub exe_path: String,
    pub action: Action,
    pub on_fault: FaultPolicy,
    pub reset_on_change: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct DesiredState {
    pub rules: Vec<RuleSpec>,
    pub on_routa_exit: ExitPolicy,
    pub routa_pid: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum Request {
    Hello { version: u32 },
    Apply(DesiredState),
    Goodbye,
    GetFlows,
    GetStatus,
    CleanupFilters,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum Response {
    Hello {
        version: u32,
        accepted: bool,
    },
    /// 请求已完整执行。
    Ok,
    /// 请求已执行（例如状态已落地），但部分规则或操作出现非致命错误（中文，可直接展示）。
    Applied {
        errors: Vec<String>,
    },
    Flows(Vec<Flow>),
    Status(SvcStatus),
    /// 请求被拒绝，什么都没改。
    Error(String),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Flow {
    pub pid: u32,
    pub rule_id: Option<String>,
    pub exe_path: Option<String>,
    pub host: String,
    pub port: u16,
    pub udp: bool,
    pub age_ms: Option<u32>,
    pub sent: u64,
    pub received: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct SvcStatus {
    pub version: u32,
    pub owner_sid: String,
    pub last_interruption: Option<Interruption>,
    pub truncated_flows: bool,
    pub conflicts: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Interruption {
    pub at_unix: i64,
    pub reason: String,
}

/// 编码为一帧：u32 小端长度 + JSON 正文。
pub fn encode<T: Serialize>(msg: &T) -> Result<Vec<u8>, String> {
    let body = serde_json::to_vec(msg).map_err(|e| e.to_string())?;
    if body.len() > MAX_FRAME {
        return Err(format!("frame too large: {} > {}", body.len(), MAX_FRAME));
    }
    let mut out = Vec::with_capacity(4 + body.len());
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(&body);
    Ok(out)
}

/// 解析帧头得到正文长度；超过 MAX_FRAME 报错。
pub fn decode_len(header: [u8; 4]) -> Result<usize, String> {
    let len = u32::from_le_bytes(header) as usize;
    if len > MAX_FRAME {
        return Err(format!("frame too large: {} > {}", len, MAX_FRAME));
    }
    Ok(len)
}

pub fn decode<T: DeserializeOwned>(body: &[u8]) -> Result<T, String> {
    serde_json::from_slice(body).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_apply() {
        let req = Request::Apply(DesiredState {
            rules: vec![RuleSpec {
                id: "r1".into(),
                exe_path: r"C:\app\a.exe".into(),
                action: Action::Proxy(ProxyTarget {
                    kind: ProxyKind::Socks5,
                    host: "127.0.0.1".into(),
                    port: 7890,
                    username: None,
                    password: Some("p".into()),
                }),
                on_fault: FaultPolicy::Block,
                reset_on_change: true,
            }],
            on_routa_exit: ExitPolicy::Direct,
            routa_pid: 42,
        });
        let buf = encode(&req).unwrap();
        let len = decode_len(buf[..4].try_into().unwrap()).unwrap();
        assert_eq!(len, buf.len() - 4);
        let back: Request = decode(&buf[4..]).unwrap();
        assert_eq!(back, req);
    }

    #[test]
    fn roundtrip_applied() {
        let resp = Response::Applied {
            errors: vec!["规则 r1 失败".into()],
        };
        let buf = encode(&resp).unwrap();
        let back: Response = decode(&buf[4..]).unwrap();
        assert_eq!(back, resp);
    }

    #[test]
    fn oversized_frame_rejected() {
        let header = ((MAX_FRAME + 1) as u32).to_le_bytes();
        assert!(decode_len(header).is_err());
        assert!(decode_len((MAX_FRAME as u32).to_le_bytes()).is_ok());
    }
}

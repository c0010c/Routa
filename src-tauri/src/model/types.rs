use serde::{Deserialize, Serialize};

macro_rules! id_type {
    ($name:ident) => {
        #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, specta::Type)]
        #[serde(transparent)]
        pub struct $name(pub String);

        impl $name {
            pub fn new() -> Self {
                Self(uuid::Uuid::new_v4().to_string())
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }
    };
}

id_type!(SubscriptionId);
id_type!(ExitId);
id_type!(ListenerId);

/// Windows 网卡标识（GUID）。
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, specta::Type)]
#[serde(transparent)]
pub struct InterfaceGuid(pub String);

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, specta::Type)]
pub struct NodeRef {
    pub subscription_id: SubscriptionId,
    pub name: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, specta::Type)]
pub struct Node {
    pub name: String,
    pub proxy_type: String,
    pub server: String,
    /// 原样的 mihomo proxy 映射
    pub raw: serde_json::Value,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, specta::Type)]
pub enum FetchResult {
    Ok { node_count: u32 },
    Failed { reason: String },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, specta::Type)]
pub struct Subscription {
    pub id: SubscriptionId,
    pub name: String,
    pub url: String,
    pub update_interval_mins: Option<u32>,
    pub fallback_exit: Option<ExitId>,
    /// unix 秒
    pub last_updated: Option<i64>,
    pub last_result: Option<FetchResult>,
    pub nodes: Vec<Node>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, specta::Type)]
pub enum GroupStrategy {
    UrlTest,
    Fallback,
    LoadBalance,
    Select,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, specta::Type)]
pub enum ExitKind {
    Node {
        node: NodeRef,
    },
    Group {
        strategy: GroupStrategy,
        members: Vec<NodeRef>,
        selected: Option<NodeRef>,
    },
    Direct,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, specta::Type)]
pub struct Exit {
    pub id: ExitId,
    pub name: String,
    pub kind: ExitKind,
    pub interface: Option<InterfaceGuid>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, specta::Type)]
pub enum ListenerType {
    Mixed,
    Http,
    Socks,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, specta::Type)]
pub struct Auth {
    pub username: String,
    pub password: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, specta::Type)]
pub struct Listener {
    pub id: ListenerId,
    pub name: String,
    pub listener_type: ListenerType,
    pub listen: String,
    pub port: u16,
    pub auth: Option<Auth>,
    pub exit: Option<ExitId>,
    pub user_enabled: bool,
}

impl Listener {
    pub fn new_default(name: String, port: u16) -> Self {
        Self {
            id: ListenerId::new(),
            name,
            listener_type: ListenerType::Mixed,
            listen: "0.0.0.0".to_string(),
            port,
            auth: None,
            exit: None,
            user_enabled: true,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, specta::Type)]
pub struct Settings {
    pub autostart: bool,
    pub core_path: Option<String>,
    pub controller_port: u16,
    pub delay_test_interval_mins: Option<u32>,
    pub delay_test_url: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            autostart: false,
            core_path: None,
            controller_port: 19097,
            delay_test_interval_mins: Some(10),
            delay_test_url: "https://www.gstatic.com/generate_204".to_string(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default, specta::Type)]
pub struct Model {
    pub subscriptions: Vec<Subscription>,
    pub exits: Vec<Exit>,
    pub listeners: Vec<Listener>,
    pub settings: Settings,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_defaults() {
        let s = Settings::default();
        assert!(!s.autostart);
        assert_eq!(s.core_path, None);
        assert_eq!(s.controller_port, 19097);
        assert_eq!(s.delay_test_interval_mins, Some(10));
        assert_eq!(s.delay_test_url, "https://www.gstatic.com/generate_204");
    }

    #[test]
    fn model_json_roundtrip() {
        let sub_id = SubscriptionId::new();
        let exit = Exit {
            id: ExitId::new(),
            name: "出口 🚀 \"q\": x".into(),
            kind: ExitKind::Group {
                strategy: GroupStrategy::UrlTest,
                members: vec![NodeRef {
                    subscription_id: sub_id.clone(),
                    name: "香港 🇭🇰 01".into(),
                }],
                selected: None,
            },
            interface: Some(InterfaceGuid("{abc}".into())),
        };
        let mut l = Listener::new_default("监听 😀".into(), 7891);
        l.exit = Some(exit.id.clone());
        l.auth = Some(Auth {
            username: "u".into(),
            password: "p".into(),
        });
        let m = Model {
            subscriptions: vec![Subscription {
                id: sub_id,
                name: "订阅 ✨".into(),
                url: "https://example.com/sub".into(),
                update_interval_mins: Some(1440),
                fallback_exit: None,
                last_updated: Some(1_700_000_000),
                last_result: Some(FetchResult::Ok { node_count: 1 }),
                nodes: vec![Node {
                    name: "香港 🇭🇰 01".into(),
                    proxy_type: "ss".into(),
                    server: "1.2.3.4".into(),
                    raw: serde_json::json!({"name": "香港 🇭🇰 01", "port": 443}),
                }],
            }],
            exits: vec![
                exit,
                Exit {
                    id: ExitId::new(),
                    name: "直连".into(),
                    kind: ExitKind::Direct,
                    interface: None,
                },
            ],
            listeners: vec![l],
            settings: Settings::default(),
        };
        let json = serde_json::to_string(&m).unwrap();
        let back: Model = serde_json::from_str(&json).unwrap();
        assert_eq!(m, back);
    }
}

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

// Schema evolution: the persisted structs are `#[serde(default)]` at struct
// level (missing fields come from `Default`), so a state.json written before
// a field existed still loads. Struct level keeps the TS fields required.

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, specta::Type)]
#[serde(default)]
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
#[serde(default)]
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
#[serde(default)]
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

impl Default for Subscription {
    fn default() -> Self {
        Self {
            id: SubscriptionId::new(),
            name: String::new(),
            url: String::new(),
            update_interval_mins: Some(1440),
            fallback_exit: None,
            last_updated: None,
            last_result: None,
            nodes: Vec::new(),
        }
    }
}

impl Default for Exit {
    fn default() -> Self {
        Self { id: ExitId::new(), name: String::new(), kind: ExitKind::Direct, interface: None }
    }
}

impl Default for Listener {
    fn default() -> Self {
        Self::new_default(String::new(), 0)
    }
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
#[serde(default)]
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
#[serde(default)]
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
    fn old_state_json_missing_fields_loads_with_defaults() {
        let json = r#"{
            "subscriptions": [{"id": "s1", "name": "订阅", "url": "https://x/sub"}],
            "exits": [{"id": "e1", "name": "直连", "kind": "Direct"}],
            "listeners": [{"id": "l1", "name": "L", "listener_type": "Mixed", "listen": "0.0.0.0", "port": 7891}],
            "settings": {"autostart": true, "core_path": null, "controller_port": 19098, "delay_test_interval_mins": 5}
        }"#;
        let m: Model = serde_json::from_str(json).unwrap();
        assert_eq!(m.settings.delay_test_url, Settings::default().delay_test_url);
        assert_eq!((m.settings.autostart, m.settings.controller_port), (true, 19098));
        let s = &m.subscriptions[0];
        assert_eq!(s.fallback_exit, None);
        assert!(s.nodes.is_empty() && s.last_result.is_none());
        assert_eq!(m.exits[0].interface, None);
        let l = &m.listeners[0];
        assert!(l.user_enabled && l.auth.is_none() && l.exit.is_none());
        // A missing top-level section is the default too.
        let m: Model = serde_json::from_str("{}").unwrap();
        assert_eq!(m, Model::default());
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

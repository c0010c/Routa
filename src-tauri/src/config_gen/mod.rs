//! Generates the mihomo YAML config from the model and its derived health.
//! Everything is built as `serde_yaml_ng` values (never string concatenation)
//! so any name round-trips; key order is fixed so output is byte-stable.

use std::collections::hash_map::DefaultHasher;
use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};

use serde_yaml_ng::{Mapping, Value};

use crate::model::health::{ExitHealth, ExitStatus, Health, ListenerStatus};
use crate::model::*;

pub struct GenInput<'a> {
    pub model: &'a Model,
    pub health: &'a Health,
    pub secret: &'a str,
    /// Internal listeners used to fetch through an exit (Task 11).
    pub fetch_listeners: &'a [(ExitId, u16)],
}

#[derive(Debug, Clone, PartialEq)]
pub struct Generated {
    pub yaml: String,
    pub plan: EffectivePlan,
    /// (group name, member copy name) for each Ok select group whose selection is effective.
    pub selects: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlannedListener {
    pub id: ListenerId,
    pub exit_name: String,
    pub exit_fingerprint: u64,
    pub port: u16,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EffectivePlan {
    /// Keyed by listener name; user listeners only, never `__fetch__`.
    pub listeners: BTreeMap<String, PlannedListener>,
}

pub fn node_proxy_name(sub: &Subscription, node: &Node) -> String {
    format!("{}/{}", sub.name, node.name)
}

pub fn member_proxy_name(exit: &Exit, sub: &Subscription, node: &Node) -> String {
    format!("{}/{}/{}", exit.name, sub.name, node.name)
}

fn s(x: &str) -> Value {
    Value::String(x.to_string())
}

fn map(entries: Vec<(&str, Value)>) -> Value {
    let mut m = Mapping::new();
    for (k, v) in entries {
        m.insert(s(k), v);
    }
    Value::Mapping(m)
}

fn json_to_yaml(v: &serde_json::Value) -> Value {
    match v {
        serde_json::Value::Null => Value::Null,
        serde_json::Value::Bool(b) => Value::Bool(*b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::from(i)
            } else if let Some(u) = n.as_u64() {
                Value::from(u)
            } else {
                Value::from(n.as_f64().unwrap_or(0.0))
            }
        }
        serde_json::Value::String(x) => Value::String(x.clone()),
        serde_json::Value::Array(a) => Value::Sequence(a.iter().map(json_to_yaml).collect()),
        serde_json::Value::Object(o) => {
            let mut m = Mapping::new();
            for (k, val) in o {
                m.insert(s(k), json_to_yaml(val));
            }
            Value::Mapping(m)
        }
    }
}

/// Copy of the node's raw proxy mapping with `name` (and optionally
/// `interface-name`) replaced.
///
/// Subscription-supplied `interface-name`, `routing-mark` and `dialer-proxy`
/// are stripped: they would bind traffic to a NIC the user did not choose, or
/// reference a proxy that has been renamed. `None` if `raw` is not an object.
fn proxy_copy(node: &Node, name: &str, interface: Option<&str>) -> Option<Value> {
    let Value::Mapping(mut m) = json_to_yaml(&node.raw) else {
        return None;
    };
    for k in ["interface-name", "routing-mark", "dialer-proxy"] {
        m.remove(k);
    }
    m.insert(s("name"), s(name));
    if let Some(i) = interface {
        m.insert(s("interface-name"), s(i));
    }
    Some(Value::Mapping(m))
}

fn find_node<'a>(model: &'a Model, r: &NodeRef) -> Option<(&'a Subscription, &'a Node)> {
    let sub = model.subscriptions.iter().find(|x| x.id == r.subscription_id)?;
    let node = sub.nodes.iter().find(|n| n.name == r.name)?;
    Some((sub, node))
}

fn strategy_type(st: &GroupStrategy) -> &'static str {
    match st {
        GroupStrategy::UrlTest => "url-test",
        GroupStrategy::Fallback => "fallback",
        GroupStrategy::LoadBalance => "load-balance",
        GroupStrategy::Select => "select",
    }
}

/// Everything generated for one Ok exit: its proxy entries and optional group entry.
struct ExitParts {
    proxies: Vec<Value>,
    group: Option<Value>,
}

/// `None` when nothing could be emitted for the exit (treated as not Ok).
fn exit_parts(model: &Model, exit: &Exit, h: &ExitHealth) -> Option<ExitParts> {
    let iface = h.interface.as_ref().map(|n| n.mihomo_name.as_str());
    let parts = match &exit.kind {
        ExitKind::Node { .. } => {
            let proxies = h
                .effective_members
                .iter()
                .filter_map(|r| find_node(model, r))
                .filter_map(|(_, n)| proxy_copy(n, &exit.name, iface))
                .collect();
            ExitParts { proxies, group: None }
        }
        ExitKind::Group { strategy, .. } => {
            let mut proxies = Vec::new();
            let mut names = Vec::new();
            for r in &h.effective_members {
                if let Some((sub, n)) = find_node(model, r) {
                    let name = member_proxy_name(exit, sub, n);
                    if let Some(p) = proxy_copy(n, &name, iface) {
                        proxies.push(p);
                        names.push(s(&name));
                    }
                }
            }
            let interval = model.settings.delay_test_interval_mins.map_or(0, |m| u64::from(m) * 60);
            let group = map(vec![
                ("name", s(&exit.name)),
                ("type", s(strategy_type(strategy))),
                ("proxies", Value::Sequence(names)),
                ("url", s(&model.settings.delay_test_url)),
                ("interval", Value::from(interval)),
            ]);
            ExitParts { proxies, group: Some(group) }
        }
        ExitKind::Direct => {
            let mut entries = vec![("name", s(&exit.name)), ("type", s("direct"))];
            if let Some(i) = iface {
                entries.push(("interface-name", s(i)));
            }
            ExitParts { proxies: vec![map(entries)], group: None }
        }
    };
    (!parts.proxies.is_empty()).then_some(parts)
}

fn fingerprint(parts: &ExitParts) -> u64 {
    let mut all = parts.proxies.clone();
    all.extend(parts.group.clone());
    let text = serde_yaml_ng::to_string(&Value::Sequence(all)).expect("yaml serialization");
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}

fn base_config(
    controller_port: u16,
    secret: &str,
    proxies: Vec<Value>,
    groups: Vec<Value>,
    listeners: Vec<Value>,
) -> String {
    let root = map(vec![
        ("mode", s("rule")),
        ("log-level", s("info")),
        ("allow-lan", Value::Bool(true)),
        ("external-controller", s(&format!("127.0.0.1:{controller_port}"))),
        ("secret", s(secret)),
        ("profile", map(vec![("store-selected", Value::Bool(false))])),
        ("proxies", Value::Sequence(proxies)),
        ("proxy-groups", Value::Sequence(groups)),
        ("listeners", Value::Sequence(listeners)),
        ("rules", Value::Sequence(vec![s("MATCH,REJECT")])),
    ]);
    serde_yaml_ng::to_string(&root).expect("yaml serialization")
}

/// Config with only the controller: no outbounds, no listeners.
pub fn empty_config(controller_port: u16, secret: &str) -> String {
    base_config(controller_port, secret, vec![], vec![], vec![])
}

pub fn generate(input: &GenInput) -> Generated {
    let model = input.model;

    // Every subscription node, for the node page's delay tests.
    let mut proxies: Vec<Value> = model
        .subscriptions
        .iter()
        .flat_map(|sub| {
            sub.nodes
                .iter()
                .filter_map(move |n| proxy_copy(n, &node_proxy_name(sub, n), None))
        })
        .collect();

    let mut groups = Vec::new();
    let mut selects = Vec::new();
    let mut fingerprints: BTreeMap<&ExitId, u64> = BTreeMap::new();

    for exit in &model.exits {
        let Some(h) = input.health.exits.get(&exit.id) else { continue };
        if h.status != ExitStatus::Ok {
            continue;
        }
        let Some(parts) = exit_parts(model, exit, h) else { continue };
        fingerprints.insert(&exit.id, fingerprint(&parts));
        proxies.extend(parts.proxies);
        groups.extend(parts.group);

        if let ExitKind::Group { strategy: GroupStrategy::Select, selected: Some(sel), .. } = &exit.kind {
            if h.effective_members.contains(sel) {
                if let Some((sub, n)) = find_node(model, sel).filter(|(_, n)| n.raw.is_object()) {
                    selects.push((exit.name.clone(), member_proxy_name(exit, sub, n)));
                }
            }
        }
    }

    let mut listeners = Vec::new();
    let mut plan = BTreeMap::new();
    for l in &model.listeners {
        if input.health.listeners.get(&l.id) != Some(&ListenerStatus::Active) {
            continue;
        }
        let Some(exit) = l.exit.as_ref().and_then(|id| model.exits.iter().find(|e| &e.id == id)) else {
            continue;
        };
        let Some(&fp) = fingerprints.get(&exit.id) else { continue };
        let ty = match l.listener_type {
            ListenerType::Mixed => "mixed",
            ListenerType::Http => "http",
            ListenerType::Socks => "socks",
        };
        let mut entries = vec![
            ("name", s(&l.name)),
            ("type", s(ty)),
            ("listen", s(&l.listen)),
            ("port", Value::from(l.port)),
            ("proxy", s(&exit.name)),
        ];
        if let Some(a) = &l.auth {
            let user = map(vec![("username", s(&a.username)), ("password", s(&a.password))]);
            entries.push(("users", Value::Sequence(vec![user])));
        }
        listeners.push(map(entries));
        plan.insert(
            l.name.clone(),
            PlannedListener {
                id: l.id.clone(),
                exit_name: exit.name.clone(),
                exit_fingerprint: fp,
                port: l.port,
            },
        );
    }

    for (exit_id, port) in input.fetch_listeners {
        if !fingerprints.contains_key(exit_id) {
            continue;
        }
        let Some(exit) = model.exits.iter().find(|e| &e.id == exit_id) else { continue };
        listeners.push(map(vec![
            ("name", s(&format!("__fetch__{}", exit_id.0))),
            ("type", s("mixed")),
            ("listen", s("127.0.0.1")),
            ("port", Value::from(*port)),
            ("proxy", s(&exit.name)),
        ]));
    }

    Generated {
        yaml: base_config(model.settings.controller_port, input.secret, proxies, groups, listeners),
        plan: EffectivePlan { listeners: plan },
        selects,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::health::{evaluate, ExitStatus};
    use crate::netif::NetInterface;
    use serde_json::json;
    use std::net::Ipv4Addr;

    fn nic(guid: &str, name: &str) -> NetInterface {
        NetInterface {
            guid: InterfaceGuid(guid.into()),
            friendly_name: name.into(),
            description: "d".into(),
            is_up: true,
            ipv4: vec![Ipv4Addr::new(10, 0, 0, 2)],
            ipv6: vec![],
            mihomo_name: name.into(),
        }
    }
    fn nics() -> Vec<NetInterface> {
        vec![nic("{G1}", "以太网"), nic("{G2}", "WLAN"), nic("{G3}", "USB")]
    }
    fn node(raw: serde_json::Value) -> Node {
        Node {
            name: raw["name"].as_str().unwrap().into(),
            proxy_type: raw["type"].as_str().unwrap().into(),
            server: raw["server"].as_str().unwrap().into(),
            raw,
        }
    }
    fn nref(n: &str) -> NodeRef {
        NodeRef { subscription_id: SubscriptionId("s1".into()), name: n.into() }
    }
    fn base() -> Model {
        let sub = Subscription {
            id: SubscriptionId("s1".into()),
            name: "机场".into(),
            url: "https://x".into(),
            update_interval_mins: Some(1440),
            fallback_exit: None,
            last_updated: None,
            last_result: None,
            nodes: vec![
                node(json!({"name":"HK 01","type":"ss","server":"1.1.1.1","port":443,"cipher":"aes-128-gcm","password":"pw"})),
                node(json!({"name":"JP 01","type":"trojan","server":"jp.example.com","port":443,"password":"pw2"})),
            ],
        };
        let exit = |id: &str, name: &str, kind, g: &str| Exit {
            id: ExitId(id.into()),
            name: name.into(),
            kind,
            interface: Some(InterfaceGuid(g.into())),
        };
        let lis = |id: &str, name: &str, t, listen: &str, port, auth, e: &str| Listener {
            id: ListenerId(id.into()),
            name: name.into(),
            listener_type: t,
            listen: listen.into(),
            port,
            auth,
            exit: Some(ExitId(e.into())),
            user_enabled: true,
        };
        Model {
            subscriptions: vec![sub],
            exits: vec![
                exit("ea", "出口A", ExitKind::Node { node: nref("HK 01") }, "{G1}"),
                exit(
                    "eb",
                    "出口B",
                    ExitKind::Group {
                        strategy: GroupStrategy::UrlTest,
                        members: vec![nref("HK 01"), nref("JP 01")],
                        selected: None,
                    },
                    "{G2}",
                ),
                exit("ec", "出口C", ExitKind::Direct, "{G3}"),
            ],
            listeners: vec![
                lis("la", "入口A", ListenerType::Mixed, "0.0.0.0", 7891, Some(Auth { username: "u".into(), password: "p".into() }), "ea"),
                lis("lb", "入口B", ListenerType::Http, "127.0.0.1", 7892, None, "eb"),
                lis("lc", "入口C", ListenerType::Socks, "0.0.0.0", 7893, None, "ec"),
            ],
            settings: Settings::default(),
        }
    }
    fn gen_with(m: &Model, nics: &[NetInterface], fetch: &[(ExitId, u16)]) -> Generated {
        let h = evaluate(m, nics);
        generate(&GenInput { model: m, health: &h, secret: "sekret", fetch_listeners: fetch })
    }
    fn gen(m: &Model) -> Generated {
        gen_with(m, &nics(), &[])
    }
    fn parse(y: &str) -> serde_yaml_ng::Value {
        serde_yaml_ng::from_str(y).unwrap()
    }
    fn names(v: &serde_yaml_ng::Value, key: &str) -> Vec<String> {
        v[key].as_sequence().unwrap().iter().map(|p| p["name"].as_str().unwrap().to_string()).collect()
    }

    #[test]
    fn basic_matches_fixture() {
        let expected = include_str!("../../tests/fixtures/gen_expected_basic.yaml").replace("\r\n", "\n");
        assert_eq!(gen(&base()).yaml, expected);
    }

    #[test]
    fn fixed_top_level() {
        let v = parse(&gen(&base()).yaml);
        assert_eq!(v["allow-lan"], true);
        assert_eq!(v["external-controller"], "127.0.0.1:19097");
        assert_eq!(v["rules"], serde_yaml_ng::to_value(["MATCH,REJECT"]).unwrap());
        for k in ["port", "socks-port", "mixed-port", "tun", "dns"] {
            assert!(v.get(k).is_none(), "{k}");
        }
    }

    #[test]
    fn broken_exit_and_listener_omitted() {
        let mut m = base();
        m.subscriptions[0].nodes.remove(0); // HK 01 gone -> 出口A NodeMissing
        let h = evaluate(&m, &nics());
        assert_eq!(h.exits[&ExitId("ea".into())].status, ExitStatus::NodeMissing);
        let g = gen(&m);
        let v = parse(&g.yaml);
        assert!(!names(&v, "proxies").contains(&"出口A".to_string()));
        assert_eq!(names(&v, "listeners"), ["入口B", "入口C"]);
        assert!(!g.plan.listeners.contains_key("入口A"));
    }

    #[test]
    fn user_disabled_listener_omitted() {
        let mut m = base();
        m.listeners[1].user_enabled = false;
        let g = gen(&m);
        assert_eq!(names(&parse(&g.yaml), "listeners"), ["入口A", "入口C"]);
        assert_eq!(g.plan.listeners.len(), 2);
    }

    #[test]
    fn group_skips_missing_members() {
        let mut m = base();
        m.subscriptions[0].nodes.remove(1); // JP 01 gone
        let v = parse(&gen(&m).yaml);
        assert_eq!(v["proxy-groups"][0]["proxies"], serde_yaml_ng::to_value(["出口B/机场/HK 01"]).unwrap());
        let ps = names(&v, "proxies");
        assert!(!ps.contains(&"出口B/机场/JP 01".to_string()));
        assert!(ps.contains(&"出口B/机场/HK 01".to_string()));
    }

    #[test]
    fn unicode_names_roundtrip() {
        let mut m = base();
        let weird = "🇭🇰 香港: \"01\"";
        m.subscriptions[0].nodes[0] = node(json!({"name":weird,"type":"ss","server":"1.1.1.1","port":443}));
        m.exits[0].kind = ExitKind::Node { node: nref(weird) };
        m.exits[1].kind = ExitKind::Group {
            strategy: GroupStrategy::Fallback,
            members: vec![nref(weird)],
            selected: None,
        };
        let mut n = nics();
        n[0].mihomo_name = "以太网 2".into();
        let v = parse(&gen_with(&m, &n, &[]).yaml);
        let ps = v["proxies"].as_sequence().unwrap();
        assert_eq!(ps[0]["name"].as_str().unwrap(), format!("机场/{weird}"));
        let a = ps.iter().find(|p| p["name"] == "出口A").unwrap();
        assert_eq!(a["interface-name"].as_str().unwrap(), "以太网 2");
        let member = format!("出口B/机场/{weird}");
        assert!(ps.iter().any(|p| p["name"].as_str() == Some(&member)));
        assert_eq!(v["proxy-groups"][0]["proxies"][0].as_str().unwrap(), member);
    }

    #[test]
    fn strips_subscription_interface_and_dialer_proxy() {
        let mut m = base();
        m.subscriptions[0].nodes[0] = node(json!({"name":"HK 01","type":"ss","server":"1.1.1.1","port":443,
            "interface-name":"X","routing-mark":1,"dialer-proxy":"foo"}));
        m.exits[2].interface = None;
        m.exits[0].interface = None; // no-NIC node exit
        let h = evaluate(&m, &nics());
        let v = parse(&generate(&GenInput { model: &m, health: &h, secret: "s", fetch_listeners: &[] }).yaml);
        let ps = v["proxies"].as_sequence().unwrap();
        let get = |n: &str| ps.iter().find(|p| p["name"].as_str() == Some(n)).unwrap();
        for n in ["机场/HK 01", "出口A"] {
            for k in ["interface-name", "routing-mark", "dialer-proxy"] {
                assert!(get(n).get(k).is_none(), "{n} {k}");
            }
        }
        let member = get("出口B/机场/HK 01");
        assert_eq!(member["interface-name"], "WLAN");
        assert!(member.get("routing-mark").is_none() && member.get("dialer-proxy").is_none());
        assert_eq!(member.as_mapping().unwrap().keys().filter(|k| *k == "interface-name").count(), 1);
    }

    #[test]
    fn non_object_raw_is_not_emitted() {
        let mut m = base();
        m.subscriptions[0].nodes[0].raw = serde_json::Value::Null;
        let g = gen(&m);
        let v = parse(&g.yaml);
        let ps = names(&v, "proxies");
        assert!(!ps.contains(&"机场/HK 01".to_string()));
        assert!(!ps.contains(&"出口A".to_string()));
        assert!(!ps.contains(&"出口B/机场/HK 01".to_string()));
        assert!(ps.contains(&"出口B/机场/JP 01".to_string()));
        assert_eq!(v["proxy-groups"][0]["proxies"], serde_yaml_ng::to_value(["出口B/机场/JP 01"]).unwrap());
        assert_eq!(names(&v, "listeners"), ["入口B", "入口C"]);
        assert!(!g.plan.listeners.contains_key("入口A"));
    }

    #[test]
    fn exit_and_listener_names_roundtrip() {
        let mut m = base();
        m.exits[0].name = "出 \"口\": 🚀".into();
        m.listeners[0].name = "入 \"口\": 🎯".into();
        let g = gen(&m);
        let v = parse(&g.yaml);
        assert!(names(&v, "proxies").contains(&"出 \"口\": 🚀".to_string()));
        let l = &v["listeners"][0];
        assert_eq!(l["name"].as_str().unwrap(), "入 \"口\": 🎯");
        assert_eq!(l["proxy"].as_str().unwrap(), "出 \"口\": 🚀");
        assert_eq!(g.plan.listeners["入 \"口\": 🎯"].exit_name, "出 \"口\": 🚀");
    }

    #[test]
    fn interval_none_gives_zero() {
        let mut m = base();
        m.settings.delay_test_interval_mins = None;
        let v = parse(&gen(&m).yaml);
        assert_eq!(v["proxy-groups"][0]["interval"].as_u64(), Some(0));
    }

    #[test]
    fn deterministic() {
        assert_eq!(gen(&base()), gen(&base()));
    }

    #[test]
    fn plan_fingerprint_changes_when_exit_interface_changes() {
        let m = base();
        let before = gen(&m);
        let mut n = nics();
        n[1].mihomo_name = "WLAN 2".into();
        let after = gen_with(&m, &n, &[]);
        let fp = |g: &Generated, l: &str| g.plan.listeners[l].exit_fingerprint;
        assert_ne!(fp(&before, "入口B"), fp(&after, "入口B"));
        assert_eq!(fp(&before, "入口A"), fp(&after, "入口A"));
        assert_eq!(fp(&before, "入口C"), fp(&after, "入口C"));
        let p = &before.plan.listeners["入口B"];
        assert_eq!((p.exit_name.as_str(), p.port, &p.id), ("出口B", 7892, &ListenerId("lb".into())));
    }

    #[test]
    fn empty_config_has_no_listeners() {
        let v = parse(&empty_config(9097, "s"));
        assert_eq!(v["listeners"].as_sequence().unwrap().len(), 0);
        assert_eq!(v["proxies"].as_sequence().unwrap().len(), 0);
        assert_eq!(v["proxy-groups"].as_sequence().unwrap().len(), 0);
        assert_eq!(v["external-controller"], "127.0.0.1:9097");
        assert_eq!(v["secret"], "s");
        assert_eq!(v["rules"], serde_yaml_ng::to_value(["MATCH,REJECT"]).unwrap());
    }

    #[test]
    fn select_groups_and_fetch_listeners() {
        let mut m = base();
        m.exits[1].kind = ExitKind::Group {
            strategy: GroupStrategy::Select,
            members: vec![nref("HK 01"), nref("JP 01")],
            selected: Some(nref("JP 01")),
        };
        let fetch = [(ExitId("eb".into()), 9100), (ExitId("zz".into()), 9101)];
        let g = gen_with(&m, &nics(), &fetch);
        assert_eq!(g.selects, vec![("出口B".to_string(), "出口B/机场/JP 01".to_string())]);
        let v = parse(&g.yaml);
        assert_eq!(v["proxy-groups"][0]["type"], "select");
        let ls = v["listeners"].as_sequence().unwrap();
        assert_eq!(ls.len(), 4);
        let last = ls.last().unwrap();
        assert_eq!(last["name"].as_str().unwrap(), "__fetch__eb");
        assert_eq!(last["listen"], "127.0.0.1");
        assert_eq!(last["proxy"], "出口B");
        assert_eq!(g.plan.listeners.len(), 3);
    }
}

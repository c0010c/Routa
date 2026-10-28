//! Generates the mihomo YAML config from the model and its derived health.
//! Everything is built as `serde_yaml_ng` values (never string concatenation)
//! so any name round-trips; key order is fixed so output is byte-stable.

use std::collections::hash_map::DefaultHasher;
use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};

use serde_yaml_ng::{Mapping, Value};

use crate::model::health::{ExitHealth, ExitStatus, Health, ListenerStatus};
use crate::model::*;
use crate::netif::NetInterface;

pub struct GenInput<'a> {
    pub model: &'a Model,
    pub health: &'a Health,
    pub secret: &'a str,
    /// Internal listeners used to fetch through an exit (Task 11).
    pub fetch_listeners: &'a [(ExitId, u16)],
    /// Same adapter list `health` was evaluated against; resolves Tailscale
    /// instance NICs.
    pub nics: &'a [NetInterface],
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
    /// Hash of the listener's own settings (type, listen, port, users): a
    /// change means existing connections were accepted under old rules.
    pub listener_fingerprint: u64,
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

/// Per-exit copy of a copyable node: `<出口>/<base>`, where base is
/// `<订阅>/<节点>` or `手动/<名>`.
pub fn member_proxy_name(exit: &Exit, base_name: &str) -> String {
    format!("{}/{}", exit.name, base_name)
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
fn proxy_copy(raw: &serde_json::Value, name: &str, interface: Option<&str>) -> Option<Value> {
    let Value::Mapping(mut m) = json_to_yaml(raw) else {
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

/// The instance's own NIC. `None`: configured but absent or unusable, so the
/// instance must not be written (fail closed, never the default NIC).
/// `Some(None)`: none configured, i.e. the system default.
fn tailscale_iface<'n>(opts: &TailscaleOpts, nics: &'n [NetInterface]) -> Option<Option<&'n str>> {
    match &opts.interface {
        None => Some(None),
        Some(g) => nics
            .iter()
            .find(|n| &n.guid == g)
            .filter(|n| n.is_usable())
            .map(|n| Some(n.mihomo_name.as_str())),
    }
}

/// The single definition of a Tailscale instance; every exit references it
/// by name and never copies it (copies would share hostname and state-dir).
fn tailscale_proxy(node: &ManualNode, opts: &TailscaleOpts, iface: Option<&str>) -> Value {
    let mut entries = vec![
        ("name", s(&manual_proxy_name(node))),
        ("type", s("tailscale")),
        ("hostname", s(&opts.hostname)),
        ("state-dir", s(&tailscale_state_dir(&node.id))),
        ("ephemeral", Value::Bool(opts.ephemeral)),
        ("udp", Value::Bool(opts.udp)),
    ];
    // Empty = no exit node: the instance only reaches tailnet devices.
    let exit_node = opts.exit_node.trim();
    if !exit_node.is_empty() {
        entries.push(("exit-node", s(exit_node)));
    }
    if let Some(k) = opts.auth_key.as_deref().filter(|k| !k.is_empty()) {
        entries.push(("auth-key", s(k)));
    }
    if let Some(i) = iface {
        entries.push(("interface-name", s(i)));
    }
    map(entries)
}

/// `None` when the instance must not be written (see `tailscale_iface`).
fn tailscale_def(node: &ManualNode, opts: &TailscaleOpts, nics: &[NetInterface]) -> Option<Value> {
    tailscale_iface(opts, nics).map(|i| tailscale_proxy(node, opts, i))
}

/// Proxy names (`手动/<名>`) of the Tailscale instances `generate` writes for
/// this model and adapter list (an instance whose NIC is unusable is skipped).
pub fn emitted_tailscale_names(model: &Model, nics: &[NetInterface]) -> Vec<String> {
    model
        .manual_nodes
        .iter()
        .filter(|n| matches!(&n.kind, ManualNodeKind::Tailscale(o) if tailscale_iface(o, nics).is_some()))
        .map(manual_proxy_name)
        .collect()
}

fn strategy_type(st: &GroupStrategy) -> &'static str {
    match st {
        GroupStrategy::UrlTest => "url-test",
        GroupStrategy::Fallback => "fallback",
        GroupStrategy::LoadBalance => "load-balance",
        GroupStrategy::Select => "select",
    }
}

/// Everything generated for one Ok exit: its own proxy entries, optional
/// group entry, and the Tailscale definitions it references. `shared` is
/// written once globally, so it only feeds the fingerprint.
struct ExitParts {
    proxies: Vec<Value>,
    group: Option<Value>,
    shared: Vec<Value>,
}

/// `None` when nothing could be emitted for the exit (treated as not Ok).
fn exit_parts(model: &Model, exit: &Exit, h: &ExitHealth, nics: &[NetInterface]) -> Option<ExitParts> {
    let iface = h.interface.as_ref().map(|n| n.mihomo_name.as_str());
    let parts = match &exit.kind {
        ExitKind::Node { .. } => match h.effective_members.first().and_then(|r| resolve(model, r)) {
            Some(Resolved::Copyable { raw, .. }) => ExitParts {
                proxies: proxy_copy(raw, &exit.name, iface).into_iter().collect(),
                group: None,
                shared: vec![],
            },
            Some(Resolved::Tailscale { node, opts }) => {
                let def = tailscale_def(node, opts, nics)?;
                let group = map(vec![
                    ("name", s(&exit.name)),
                    ("type", s("select")),
                    ("proxies", Value::Sequence(vec![s(&manual_proxy_name(node))])),
                ]);
                ExitParts { proxies: vec![], group: Some(group), shared: vec![def] }
            }
            None => return None,
        },
        ExitKind::Group { strategy, .. } => {
            let mut proxies = Vec::new();
            let mut shared = Vec::new();
            let mut names = Vec::new();
            for r in &h.effective_members {
                match resolve(model, r) {
                    Some(Resolved::Copyable { base_name, raw }) => {
                        let name = member_proxy_name(exit, &base_name);
                        if let Some(p) = proxy_copy(raw, &name, iface) {
                            proxies.push(p);
                            names.push(s(&name));
                        }
                    }
                    Some(Resolved::Tailscale { node, opts }) => {
                        if let Some(def) = tailscale_def(node, opts, nics) {
                            shared.push(def);
                            names.push(s(&manual_proxy_name(node)));
                        }
                    }
                    None => {}
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
            ExitParts { proxies, group: Some(group), shared }
        }
        ExitKind::Direct => {
            let mut entries = vec![("name", s(&exit.name)), ("type", s("direct"))];
            if let Some(i) = iface {
                entries.push(("interface-name", s(i)));
            }
            ExitParts { proxies: vec![map(entries)], group: None, shared: vec![] }
        }
    };
    (!parts.proxies.is_empty() || !parts.shared.is_empty()).then_some(parts)
}

fn hash_yaml(v: &Value) -> u64 {
    let text = serde_yaml_ng::to_string(v).expect("yaml serialization");
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}

/// What the exit routes through. The group's delay-test `url`/`interval`
/// are left out: changing them must not cut every connection of the group.
/// Referenced Tailscale definitions count: they are what the exit dials.
fn fingerprint(parts: &ExitParts) -> u64 {
    let mut all = parts.proxies.clone();
    all.extend(parts.shared.iter().cloned());
    if let Some(Value::Mapping(mut g)) = parts.group.clone() {
        g.remove("url");
        g.remove("interval");
        all.push(Value::Mapping(g));
    }
    hash_yaml(&Value::Sequence(all))
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
        ("log-level", s("warning")),
        ("unified-delay", Value::Bool(true)),
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

/// `empty_config` plus `nodes` as proxies, named and sanitized exactly like
/// `generate` emits them: what `mihomo -t` checks before fetched nodes are
/// accepted.
pub fn nodes_check_config(controller_port: u16, secret: &str, sub: &Subscription, nodes: &[Node]) -> String {
    let proxies = nodes
        .iter()
        .filter_map(|n| proxy_copy(&n.raw, &node_proxy_name(sub, n), None))
        .collect();
    base_config(controller_port, secret, proxies, vec![], vec![])
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
                .filter_map(move |n| proxy_copy(&n.raw, &node_proxy_name(sub, n), None))
        })
        .collect();
    // Every manual node; each Tailscale instance exactly once.
    proxies.extend(model.manual_nodes.iter().filter_map(|n| match &n.kind {
        ManualNodeKind::Proxy { raw } => proxy_copy(raw, &manual_proxy_name(n), None),
        ManualNodeKind::Tailscale(opts) => tailscale_def(n, opts, input.nics),
    }));

    let mut groups = Vec::new();
    let mut selects = Vec::new();
    let mut fingerprints: BTreeMap<&ExitId, u64> = BTreeMap::new();

    for exit in &model.exits {
        let Some(h) = input.health.exits.get(&exit.id) else { continue };
        if h.status != ExitStatus::Ok {
            continue;
        }
        let Some(parts) = exit_parts(model, exit, h, input.nics) else { continue };
        fingerprints.insert(&exit.id, fingerprint(&parts));
        proxies.extend(parts.proxies);
        groups.extend(parts.group);

        if let ExitKind::Group { strategy: GroupStrategy::Select, selected: Some(sel), .. } = &exit.kind {
            if h.effective_members.contains(sel) {
                let member = match resolve(model, sel) {
                    Some(Resolved::Copyable { base_name, raw }) if raw.is_object() => {
                        Some(member_proxy_name(exit, &base_name))
                    }
                    Some(Resolved::Tailscale { node, opts }) if tailscale_iface(opts, input.nics).is_some() => {
                        Some(manual_proxy_name(node))
                    }
                    _ => None,
                };
                if let Some(member) = member {
                    selects.push((exit.name.clone(), member));
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
        let mut own = vec![
            ("type", s(ty)),
            ("listen", s(&l.listen)),
            ("port", Value::from(l.port)),
        ];
        if let Some(a) = &l.auth {
            let user = map(vec![("username", s(&a.username)), ("password", s(&a.password))]);
            own.push(("users", Value::Sequence(vec![user])));
        }
        let listener_fingerprint = hash_yaml(&map(own.clone()));
        let mut entries = vec![("name", s(&l.name))];
        entries.extend(own.drain(..3));
        entries.push(("proxy", s(&exit.name)));
        entries.extend(own);
        listeners.push(map(entries));
        plan.insert(
            l.name.clone(),
            PlannedListener {
                id: l.id.clone(),
                exit_name: exit.name.clone(),
                exit_fingerprint: fp,
                listener_fingerprint,
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
        NodeRef::Subscription { subscription_id: SubscriptionId("s1".into()), name: n.into() }
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
                        members: [nref("HK 01"), nref("JP 01")].map(ExitMember::Node).into(),
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
            manual_nodes: vec![],
            node_groups: vec![],
        }
    }
    fn gen_with(m: &Model, nics: &[NetInterface], fetch: &[(ExitId, u16)]) -> Generated {
        let h = evaluate(m, nics);
        generate(&GenInput { model: m, health: &h, secret: "sekret", fetch_listeners: fetch, nics })
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
        // info logs one line per connection: the log would grow without bound.
        assert_eq!(v["log-level"], "warning");
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
            members: vec![ExitMember::Node(nref(weird))],
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
        let n = nics();
        let v = parse(&generate(&GenInput { model: &m, health: &h, secret: "s", fetch_listeners: &[], nics: &n }).yaml);
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
    fn unified_delay_enabled() {
        let v = parse(&gen(&base()).yaml);
        assert_eq!(v["unified-delay"].as_bool(), Some(true));
        let e = parse(&empty_config(19098, "s"));
        assert_eq!(e["unified-delay"].as_bool(), Some(true));
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
    fn exit_fingerprint_ignores_delay_test_settings() {
        let fp = |m: &Model| gen(m).plan.listeners["入口B"].exit_fingerprint;
        let before = fp(&base());
        let mut m = base();
        m.settings.delay_test_url = "https://cp.cloudflare.com".into();
        m.settings.delay_test_interval_mins = Some(30);
        assert_eq!(fp(&m), before);
        assert_ne!(gen(&m).yaml, gen(&base()).yaml, "the group itself still changes");
        let mut m = base();
        m.exits[1].kind = ExitKind::Group {
            strategy: GroupStrategy::Fallback,
            members: [nref("HK 01"), nref("JP 01")].map(ExitMember::Node).into(),
            selected: None,
        };
        assert_ne!(fp(&m), before, "a strategy change still counts");
    }

    #[test]
    fn listener_fingerprint_tracks_own_settings_only() {
        let lfp = |m: &Model| gen(m).plan.listeners["入口A"].listener_fingerprint;
        let before = lfp(&base());
        let changed: [fn(&mut Listener); 5] = [
            |l| l.listener_type = ListenerType::Http,
            |l| l.listen = "127.0.0.1".into(),
            |l| l.port = 7999,
            |l| l.auth.as_mut().unwrap().password = "p2".into(),
            |l| l.auth = None,
        ];
        for (i, change) in changed.iter().enumerate() {
            let mut m = base();
            change(&mut m.listeners[0]);
            assert_ne!(lfp(&m), before, "change {i}");
        }
        let mut m = base();
        m.exits[0].interface = Some(InterfaceGuid("{G2}".into())); // exit, not listener
        assert_eq!(lfp(&m), before);
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
    fn nodes_check_config_has_only_sanitized_nodes() {
        let mut m = base();
        m.subscriptions[0].nodes[0].raw["interface-name"] = json!("X");
        let sub = &m.subscriptions[0];
        let v = parse(&nodes_check_config(19097, "s", sub, &sub.nodes));
        assert_eq!(names(&v, "proxies"), ["机场/HK 01", "机场/JP 01"]);
        assert!(v["proxies"][0].get("interface-name").is_none());
        assert_eq!(v["proxies"][0]["cipher"], "aes-128-gcm");
        assert!(v["listeners"].as_sequence().unwrap().is_empty());
        assert!(v["proxy-groups"].as_sequence().unwrap().is_empty());
        assert_eq!(v["external-controller"], "127.0.0.1:19097");
    }

    #[test]
    fn select_groups_and_fetch_listeners() {
        let mut m = base();
        m.exits[1].kind = ExitKind::Group {
            strategy: GroupStrategy::Select,
            members: [nref("HK 01"), nref("JP 01")].map(ExitMember::Node).into(),
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

    fn mref(id: &str) -> NodeRef {
        NodeRef::Manual { manual_id: ManualNodeId(id.into()) }
    }
    fn proxy_manual(id: &str, name: &str, groups: &[&str]) -> ManualNode {
        ManualNode {
            id: ManualNodeId(id.into()),
            name: name.into(),
            groups: groups.iter().map(|g| NodeGroupId((*g).into())).collect(),
            kind: ManualNodeKind::Proxy {
                raw: json!({"name":"orig","type":"socks5","server":"10.0.0.9","port":1080}),
            },
        }
    }
    fn ts_manual(id: &str, name: &str, groups: &[&str], iface: Option<&str>) -> ManualNode {
        let mut opts = TailscaleOpts::defaults_for(name);
        opts.exit_node = "100.64.0.9".into();
        opts.interface = iface.map(|g| InterfaceGuid(g.into()));
        ManualNode {
            id: ManualNodeId(id.into()),
            name: name.into(),
            groups: groups.iter().map(|g| NodeGroupId((*g).into())).collect(),
            kind: ManualNodeKind::Tailscale(opts),
        }
    }
    fn custom(gid: &str) -> ExitMember {
        ExitMember::Group { group: GroupRef::Custom(NodeGroupId(gid.into())) }
    }
    fn group_kind(members: Vec<ExitMember>) -> ExitKind {
        ExitKind::Group { strategy: GroupStrategy::Fallback, members, selected: None }
    }
    fn find<'v>(v: &'v serde_yaml_ng::Value, key: &str, name: &str) -> Option<&'v serde_yaml_ng::Value> {
        v[key].as_sequence().unwrap().iter().find(|p| p["name"].as_str() == Some(name))
    }
    fn seq(xs: &[&str]) -> serde_yaml_ng::Value {
        serde_yaml_ng::to_value(xs).unwrap()
    }
    fn keys(p: &serde_yaml_ng::Value) -> Vec<String> {
        p.as_mapping().unwrap().keys().map(|k| k.as_str().unwrap().to_string()).collect()
    }
    /// 出口B (Group, G2) holds custom group g1.
    fn with_group_exit(nodes: Vec<ManualNode>) -> Model {
        let mut m = base();
        m.manual_nodes = nodes;
        m.node_groups = vec![NodeGroup { id: NodeGroupId("g1".into()), name: "组".into() }];
        m.exits[1].kind = group_kind(vec![custom("g1")]);
        m
    }

    #[test]
    fn manual_proxy_emitted_with_prefix() {
        let m = with_group_exit(vec![proxy_manual("m1", "a", &["g1"])]);
        let v = parse(&gen(&m).yaml);
        let ps = names(&v, "proxies");
        assert!(ps.contains(&"手动/a".to_string()), "{ps:?}");
        assert!(ps.contains(&"出口B/手动/a".to_string()), "{ps:?}");
        assert!(find(&v, "proxies", "手动/a").unwrap().get("interface-name").is_none());
        assert_eq!(find(&v, "proxies", "出口B/手动/a").unwrap()["interface-name"], "WLAN");
        assert_eq!(find(&v, "proxy-groups", "出口B").unwrap()["proxies"], seq(&["出口B/手动/a"]));
    }

    #[test]
    fn renamed_manual_node_follows() {
        let mut m = with_group_exit(vec![proxy_manual("m1", "a", &["g1"])]);
        m.manual_nodes[0].name = "b".into();
        let ps = names(&parse(&gen(&m).yaml), "proxies");
        assert!(ps.contains(&"手动/b".to_string()) && ps.contains(&"出口B/手动/b".to_string()), "{ps:?}");
        assert!(!ps.iter().any(|p| p.ends_with("手动/a")), "{ps:?}");
    }

    #[test]
    fn tailscale_emitted_once_and_referenced() {
        let mut m = with_group_exit(vec![ts_manual("t1", "ts", &["g1"], None)]);
        m.exits[1].interface = None;
        m.exits.push(Exit {
            id: ExitId("ed".into()),
            name: "出口D".into(),
            kind: group_kind(vec![ExitMember::Node(mref("t1"))]),
            interface: None,
        });
        let v = parse(&gen(&m).yaml);
        let ts: Vec<_> =
            v["proxies"].as_sequence().unwrap().iter().filter(|p| p["type"] == "tailscale").collect();
        assert_eq!(ts.len(), 1);
        assert_eq!(ts[0]["name"], "手动/ts");
        assert_eq!(ts[0]["state-dir"], "tailscale/t1");
        assert_eq!(ts[0]["hostname"], "mihomo-ts");
        assert_eq!(ts[0]["exit-node"], "100.64.0.9");
        assert_eq!(ts[0]["udp"], true);
        assert_eq!(ts[0]["ephemeral"], false);
        assert_eq!(keys(ts[0]), ["name", "type", "hostname", "state-dir", "ephemeral", "udp", "exit-node"]);
        for g in ["出口B", "出口D"] {
            assert_eq!(find(&v, "proxy-groups", g).unwrap()["proxies"], seq(&["手动/ts"]), "{g}");
        }
    }

    #[test]
    fn tailscale_without_exit_node_omits_the_key() {
        let mut m = base();
        let mut t = ts_manual("t1", "ts", &[], None);
        if let ManualNodeKind::Tailscale(o) = &mut t.kind {
            o.exit_node = "  ".into();
        }
        m.manual_nodes = vec![t];
        let v = parse(&gen(&m).yaml);
        let p = find(&v, "proxies", "手动/ts").unwrap();
        assert!(p.get("exit-node").is_none(), "{p:?}");
    }

    #[test]
    fn tailscale_node_exit_wraps_select() {
        let mut m = base();
        m.manual_nodes = vec![ts_manual("t1", "ts", &[], None)];
        m.exits[0].kind = ExitKind::Node { node: mref("t1") };
        m.exits[0].interface = None;
        let v = parse(&gen(&m).yaml);
        let g = find(&v, "proxy-groups", "出口A").expect("wrapper group");
        assert_eq!(g["type"], "select");
        assert_eq!(g["proxies"], seq(&["手动/ts"]));
        assert!(find(&v, "proxies", "出口A").is_none());
        assert_eq!(find(&v, "listeners", "入口A").unwrap()["proxy"], "出口A");
    }

    #[test]
    fn tailscale_interface_written() {
        let mut m = base();
        let mut t = ts_manual("t1", "ts", &[], Some("{G1}"));
        if let ManualNodeKind::Tailscale(o) = &mut t.kind {
            o.auth_key = Some("tskey-test".into());
        }
        m.manual_nodes = vec![t];
        m.exits[0].kind = ExitKind::Node { node: mref("t1") };
        let mut n = nics();
        n[0].mihomo_name = "以太网 3".into();
        let v = parse(&gen_with(&m, &n, &[]).yaml);
        let p = find(&v, "proxies", "手动/ts").unwrap();
        assert_eq!(p["interface-name"], "以太网 3");
        assert_eq!(p["auth-key"], "tskey-test");
        assert_eq!(&keys(p)[7..], ["auth-key", "interface-name"]);
    }

    #[test]
    fn tailscale_with_missing_nic_not_emitted() {
        let mut m = base();
        m.manual_nodes = vec![ts_manual("t1", "ts", &[], Some("{GONE}"))];
        m.exits[0].kind = ExitKind::Node { node: mref("t1") };
        m.exits[0].interface = Some(InterfaceGuid("{GONE}".into()));
        let h = evaluate(&m, &nics());
        assert_eq!(h.exits[&ExitId("ea".into())].status, ExitStatus::InterfaceUnavailable);
        let g = gen(&m);
        let v = parse(&g.yaml);
        assert!(!v["proxies"].as_sequence().unwrap().iter().any(|p| p["type"] == "tailscale"));
        assert!(find(&v, "proxy-groups", "出口A").is_none());
        assert!(find(&v, "listeners", "入口A").is_none());
        assert!(!g.plan.listeners.contains_key("入口A"));

        // A down NIC is not usable either.
        let mut m = base();
        m.manual_nodes = vec![ts_manual("t1", "ts", &[], Some("{G1}"))];
        let mut n = nics();
        n[0].is_up = false;
        let v = parse(&gen_with(&m, &n, &[]).yaml);
        assert!(find(&v, "proxies", "手动/ts").is_none());
    }

    #[test]
    fn tailscale_group_member_without_nic_is_skipped() {
        // Defense in depth: even when health says Ok, a group never references
        // an instance whose definition was not written.
        let mut m = with_group_exit(vec![
            ts_manual("t1", "ts", &["g1"], Some("{GONE}")),
            proxy_manual("m1", "a", &["g1"]),
        ]);
        m.exits[1].interface = None;
        let v = parse(&gen(&m).yaml);
        assert_eq!(find(&v, "proxy-groups", "出口B").unwrap()["proxies"], seq(&["出口B/手动/a"]));
        // Only the missing instance: the exit is not emitted at all.
        let mut m = with_group_exit(vec![ts_manual("t1", "ts", &["g1"], Some("{GONE}"))]);
        m.exits[1].interface = None;
        let g = gen(&m);
        let v = parse(&g.yaml);
        assert!(find(&v, "proxy-groups", "出口B").is_none());
        assert!(!g.plan.listeners.contains_key("入口B"));
    }

    #[test]
    fn group_dedupes_node_selected_twice() {
        let mut m = base();
        m.exits[1].kind = group_kind(vec![
            ExitMember::Node(nref("HK 01")),
            ExitMember::Group { group: GroupRef::Subscription(SubscriptionId("s1".into())) },
        ]);
        let v = parse(&gen(&m).yaml);
        assert_eq!(
            find(&v, "proxy-groups", "出口B").unwrap()["proxies"],
            seq(&["出口B/机场/HK 01", "出口B/机场/JP 01"])
        );
        let ps = names(&v, "proxies");
        assert_eq!(ps.iter().filter(|p| *p == "出口B/机场/HK 01").count(), 1);
    }

    #[test]
    fn fingerprint_changes_when_tailscale_changes() {
        let mk = |exit_node: &str| {
            let mut m = with_group_exit(vec![ts_manual("t1", "ts", &["g1"], None)]);
            m.exits[1].interface = None;
            if let ManualNodeKind::Tailscale(o) = &mut m.manual_nodes[0].kind {
                o.exit_node = exit_node.into();
            }
            m.manual_nodes.push(ts_manual("t2", "ts2", &[], None));
            m.exits[0].kind = ExitKind::Node { node: mref("t2") };
            m.exits[0].interface = None;
            gen(&m).plan
        };
        let (a, b) = (mk("auto:any"), mk("100.64.0.1"));
        let fp = |p: &EffectivePlan, l: &str| p.listeners[l].exit_fingerprint;
        assert_ne!(fp(&a, "入口B"), fp(&b, "入口B"));
        assert_eq!(fp(&a, "入口A"), fp(&b, "入口A"), "other instance unaffected");
    }

    #[test]
    fn select_tailscale_member_uses_shared_name() {
        let mut m = with_group_exit(vec![ts_manual("t1", "ts", &["g1"], None), proxy_manual("m1", "a", &["g1"])]);
        m.exits[1].interface = None;
        m.exits[1].kind = ExitKind::Group {
            strategy: GroupStrategy::Select,
            members: vec![custom("g1")],
            selected: Some(mref("t1")),
        };
        assert_eq!(gen(&m).selects, vec![("出口B".to_string(), "手动/ts".to_string())]);
        if let ExitKind::Group { selected, .. } = &mut m.exits[1].kind {
            *selected = Some(mref("m1"));
        }
        assert_eq!(gen(&m).selects, vec![("出口B".to_string(), "出口B/手动/a".to_string())]);
    }
}

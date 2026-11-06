//! Share-link (`ss://`, `vmess://`, ...) to mihomo proxy mapping, one function per scheme.

use std::collections::HashMap;

use base64::alphabet;
use base64::engine::general_purpose::{GeneralPurpose, GeneralPurposeConfig};
use base64::engine::DecodePaddingMode;
use base64::Engine;
use percent_encoding::percent_decode_str;
use serde_json::{json, Map, Value};
use url::Url;

type Query = HashMap<String, String>;

/// Parse one share link into a mihomo proxy mapping (including `name`).
pub fn parse_share_link(line: &str) -> Option<Value> {
    let line = line.trim();
    let (scheme, _) = line.split_once("://")?;
    match scheme.to_ascii_lowercase().as_str() {
        "ss" => parse_ss(line),
        "vmess" => parse_vmess(line),
        "vless" => parse_vless(line),
        "trojan" => parse_trojan(line),
        "hysteria2" | "hy2" => parse_hysteria2(line),
        "tuic" => parse_tuic(line),
        _ => None,
    }
}

/// Decode standard or URL-safe base64, with or without padding.
pub(crate) fn b64_decode_lenient(s: &str) -> Option<Vec<u8>> {
    let engine = GeneralPurpose::new(
        &alphabet::STANDARD,
        GeneralPurposeConfig::new()
            .with_decode_padding_mode(DecodePaddingMode::Indifferent)
            .with_decode_allow_trailing_bits(true),
    );
    let cleaned: String = s
        .chars()
        .filter(|c| !c.is_whitespace())
        .map(|c| match c {
            '-' => '+',
            '_' => '/',
            c => c,
        })
        .collect();
    engine.decode(cleaned).ok()
}

fn pct_decode(s: &str) -> String {
    percent_decode_str(s).decode_utf8_lossy().into_owned()
}

fn b64_decode_string(s: &str) -> Option<String> {
    String::from_utf8(b64_decode_lenient(s)?).ok()
}

/// Host, port, decoded fragment name and query map of a URL-shaped link.
struct Parts {
    url: Url,
    host: String,
    port: u16,
    name: String,
    query: Query,
}

fn split_url(line: &str) -> Option<Parts> {
    let url = Url::parse(line).ok()?;
    let host = url.host_str()?.trim_matches(|c| c == '[' || c == ']').to_string();
    if host.is_empty() {
        return None;
    }
    let port = url.port()?;
    let name = match url.fragment().map(pct_decode) {
        Some(n) if !n.is_empty() => n,
        _ => format!("{host}:{port}"),
    };
    let query = url.query_pairs().map(|(k, v)| (k.into_owned(), v.into_owned())).collect();
    Some(Parts { url, host, port, name, query })
}

fn base(name: &str, ty: &str, host: &str, port: u16) -> Map<String, Value> {
    let mut m = Map::new();
    m.insert("name".into(), json!(name));
    m.insert("type".into(), json!(ty));
    m.insert("server".into(), json!(host));
    m.insert("port".into(), json!(port));
    m
}

fn truthy(v: Option<&String>) -> bool {
    matches!(v.map(String::as_str), Some("1") | Some("true"))
}

fn non_empty<'a>(q: &'a Query, key: &str) -> Option<&'a str> {
    q.get(key).map(String::as_str).filter(|s| !s.is_empty())
}

fn alpn_list(s: &str) -> Value {
    json!(s.split(',').filter(|a| !a.is_empty()).collect::<Vec<_>>())
}

/// `network` plus `ws-opts` / `grpc-opts` from URL query (trojan).
fn apply_transport(m: &mut Map<String, Value>, q: &Query) {
    let Some(net) = non_empty(q, "type").filter(|n| *n != "tcp") else { return };
    m.insert("network".into(), json!(net));
    match net {
        "ws" => {
            let mut opts = Map::new();
            if let Some(p) = non_empty(q, "path") {
                opts.insert("path".into(), json!(p));
            }
            if let Some(h) = non_empty(q, "host") {
                opts.insert("headers".into(), json!({ "Host": h }));
            }
            if !opts.is_empty() {
                m.insert("ws-opts".into(), Value::Object(opts));
            }
        }
        "grpc" => {
            if let Some(s) = non_empty(q, "serviceName") {
                m.insert("grpc-opts".into(), json!({ "grpc-service-name": s }));
            }
        }
        _ => {}
    }
}

fn parse_ss(line: &str) -> Option<Value> {
    let rest = &line[line.find("://")? + 3..];
    let (rest, frag) = match rest.split_once('#') {
        Some((r, f)) => (r, Some(pct_decode(f))),
        None => (rest, None),
    };
    let (body, query_str) = match rest.split_once('?') {
        Some((b, q)) => (b, Some(q)),
        None => (rest, None),
    };
    let body = body.trim_end_matches('/');

    // Normalise both SIP002 and the legacy form to "method:password@host:port".
    let plain = if let Some((userinfo, hostport)) = body.rsplit_once('@') {
        let userinfo = pct_decode(userinfo);
        let creds = if userinfo.contains(':') { userinfo } else { b64_decode_string(&userinfo)? };
        format!("{creds}@{hostport}")
    } else {
        b64_decode_string(body)?
    };
    let (creds, hostport) = plain.rsplit_once('@')?;
    let (cipher, password) = creds.split_once(':')?;
    let (host, port) = hostport.rsplit_once(':')?;
    let host = host.trim_matches(|c| c == '[' || c == ']');
    let port: u16 = port.trim().parse().ok()?;
    if host.is_empty() || cipher.is_empty() {
        return None;
    }
    let name = frag.filter(|n| !n.is_empty()).unwrap_or_else(|| format!("{host}:{port}"));

    let mut m = base(&name, "ss", host, port);
    m.insert("cipher".into(), json!(cipher));
    m.insert("password".into(), json!(password));
    let plugin = query_str.and_then(|q| {
        url::form_urlencoded::parse(q.as_bytes()).find(|(k, _)| k == "plugin").map(|(_, v)| v.into_owned())
    });
    if let Some(plugin) = plugin {
        apply_ss_plugin(&mut m, &plugin);
    }
    Some(Value::Object(m))
}

/// SIP003 plugin string `name;k=v;flag` to mihomo `plugin` / `plugin-opts`.
fn apply_ss_plugin(m: &mut Map<String, Value>, plugin: &str) {
    let mut parts = plugin.split(';');
    let name = parts.next().unwrap_or_default();
    let kv: HashMap<&str, &str> = parts.map(|p| p.split_once('=').unwrap_or((p, ""))).collect();
    let mut opts = Map::new();
    let mut put = |opt_key: &str, plugin_key: &str| {
        if let Some(v) = kv.get(plugin_key) {
            opts.insert(opt_key.into(), json!(v));
        }
    };
    let plugin_name = match name {
        "obfs-local" | "simple-obfs" | "obfs" => {
            put("mode", "obfs");
            put("host", "obfs-host");
            "obfs"
        }
        "v2ray-plugin" => {
            put("mode", "mode");
            put("host", "host");
            put("path", "path");
            "v2ray-plugin"
        }
        _ => return,
    };
    if plugin_name == "v2ray-plugin" && kv.contains_key("tls") {
        opts.insert("tls".into(), json!(true));
    }
    m.insert("plugin".into(), json!(plugin_name));
    m.insert("plugin-opts".into(), Value::Object(opts));
}

fn json_str(v: &Value, key: &str) -> Option<String> {
    match v.get(key)? {
        Value::String(s) if !s.is_empty() => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

fn parse_vmess(line: &str) -> Option<Value> {
    let rest = &line[line.find("://")? + 3..];
    let j: Value = serde_json::from_str(&b64_decode_string(rest.trim())?).ok()?;
    let host = json_str(&j, "add")?;
    let port: u16 = json_str(&j, "port")?.parse().ok()?;
    let uuid = json_str(&j, "id")?;
    let name = json_str(&j, "ps").unwrap_or_else(|| format!("{host}:{port}"));

    let mut m = base(&name, "vmess", &host, port);
    m.insert("uuid".into(), json!(uuid));
    m.insert("alterId".into(), json!(json_str(&j, "aid").and_then(|a| a.parse::<u32>().ok()).unwrap_or(0)));
    m.insert("cipher".into(), json!(json_str(&j, "scy").unwrap_or_else(|| "auto".into())));
    let net = json_str(&j, "net").unwrap_or_else(|| "tcp".into());
    m.insert("network".into(), json!(net));
    let tls = json_str(&j, "tls").is_some_and(|t| t == "tls" || t == "true");
    m.insert("tls".into(), json!(tls));
    let req_host = json_str(&j, "host");
    if tls {
        if let Some(sn) = json_str(&j, "sni").or_else(|| req_host.clone()) {
            m.insert("servername".into(), json!(sn));
        }
    }
    match net.as_str() {
        "ws" => {
            let mut opts = Map::new();
            if let Some(p) = json_str(&j, "path") {
                opts.insert("path".into(), json!(p));
            }
            if let Some(h) = req_host {
                opts.insert("headers".into(), json!({ "Host": h }));
            }
            m.insert("ws-opts".into(), Value::Object(opts));
        }
        "grpc" => {
            if let Some(p) = json_str(&j, "path") {
                m.insert("grpc-opts".into(), json!({ "grpc-service-name": p }));
            }
        }
        _ => {}
    }
    Some(Value::Object(m))
}

fn parse_vless(line: &str) -> Option<Value> {
    let p = split_url(line)?;
    let uuid = pct_decode(p.url.username());
    if uuid.is_empty() {
        return None;
    }
    let q = &p.query;
    let mut m = base(&p.name, "vless", &p.host, p.port);
    m.insert("uuid".into(), json!(uuid));
    if let Some(flow) = non_empty(q, "flow") {
        m.insert("flow".into(), json!(flow));
    }
    let security = non_empty(q, "security").unwrap_or("none");
    if security == "tls" || security == "reality" {
        m.insert("tls".into(), json!(true));
    }
    if let Some(sni) = non_empty(q, "sni") {
        m.insert("servername".into(), json!(sni));
    }
    if let Some(fp) = non_empty(q, "fp") {
        m.insert("client-fingerprint".into(), json!(fp));
    }
    if let Some(alpn) = non_empty(q, "alpn") {
        m.insert("alpn".into(), alpn_list(alpn));
    }
    if truthy(q.get("allowInsecure")) {
        m.insert("skip-cert-verify".into(), json!(true));
    }
    if security == "reality" {
        m.insert(
            "reality-opts".into(),
            json!({
                "public-key": non_empty(q, "pbk").unwrap_or_default(),
                "short-id": non_empty(q, "sid").unwrap_or_default(),
            }),
        );
    }
    // Unlike trojan, vless always states its network ("tcp" by default).
    m.insert("network".into(), json!(non_empty(q, "type").unwrap_or("tcp")));
    apply_transport(&mut m, q);
    Some(Value::Object(m))
}

fn parse_trojan(line: &str) -> Option<Value> {
    let p = split_url(line)?;
    let password = userinfo_joined(&p.url);
    if password.is_empty() {
        return None;
    }
    let q = &p.query;
    let mut m = base(&p.name, "trojan", &p.host, p.port);
    m.insert("password".into(), json!(password));
    if let Some(sni) = non_empty(q, "sni").or_else(|| non_empty(q, "peer")) {
        m.insert("sni".into(), json!(sni));
    }
    if let Some(alpn) = non_empty(q, "alpn") {
        m.insert("alpn".into(), alpn_list(alpn));
    }
    if truthy(q.get("allowInsecure")) {
        m.insert("skip-cert-verify".into(), json!(true));
    }
    apply_transport(&mut m, q);
    Some(Value::Object(m))
}

/// `user[:pass]` of a URL, percent-decoded and rejoined (a trojan / hysteria2
/// password may contain ':').
fn userinfo_joined(url: &Url) -> String {
    let user = pct_decode(url.username());
    match url.password() {
        Some(pw) => format!("{user}:{}", pct_decode(pw)),
        None => user,
    }
}

fn parse_hysteria2(line: &str) -> Option<Value> {
    let p = split_url(line)?;
    let password = userinfo_joined(&p.url);
    if password.is_empty() {
        return None;
    }
    let q = &p.query;
    let mut m = base(&p.name, "hysteria2", &p.host, p.port);
    m.insert("password".into(), json!(password));
    if let Some(sni) = non_empty(q, "sni") {
        m.insert("sni".into(), json!(sni));
    }
    if let Some(obfs) = non_empty(q, "obfs") {
        m.insert("obfs".into(), json!(obfs));
        if let Some(op) = non_empty(q, "obfs-password") {
            m.insert("obfs-password".into(), json!(op));
        }
    }
    if truthy(q.get("insecure")) {
        m.insert("skip-cert-verify".into(), json!(true));
    }
    Some(Value::Object(m))
}

fn parse_tuic(line: &str) -> Option<Value> {
    let p = split_url(line)?;
    let uuid = pct_decode(p.url.username());
    if uuid.is_empty() {
        return None;
    }
    let q = &p.query;
    let mut m = base(&p.name, "tuic", &p.host, p.port);
    m.insert("uuid".into(), json!(uuid));
    if let Some(pw) = p.url.password() {
        m.insert("password".into(), json!(pct_decode(pw)));
    }
    if let Some(sni) = non_empty(q, "sni") {
        m.insert("sni".into(), json!(sni));
    }
    if let Some(cc) = non_empty(q, "congestion_control") {
        m.insert("congestion-controller".into(), json!(cc));
    }
    if let Some(alpn) = non_empty(q, "alpn") {
        m.insert("alpn".into(), alpn_list(alpn));
    }
    if let Some(mode) = non_empty(q, "udp_relay_mode") {
        m.insert("udp-relay-mode".into(), json!(mode));
    }
    if truthy(q.get("allow_insecure")) {
        m.insert("skip-cert-verify".into(), json!(true));
    }
    Some(Value::Object(m))
}

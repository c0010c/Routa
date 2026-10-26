use std::collections::HashSet;

use serde_json::Value;

use super::links::{b64_decode_lenient, parse_share_link};
use crate::model::Node;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// Recognised as a subscription but yielded no usable node.
    NoNodes,
    /// Body is neither Clash YAML nor share links (e.g. an HTML error page).
    Unrecognized,
}

/// Parse a subscription body: Clash YAML first, then (base64-wrapped or plain) share links.
/// Never returns `Ok` with an empty list.
pub fn parse_subscription(body: &str) -> Result<Vec<Node>, ParseError> {
    let entries = match parse_yaml_proxies(body) {
        Some(p) => p,
        None => parse_link_lines(body)?,
    };
    let nodes = build_nodes(entries);
    if nodes.is_empty() {
        Err(ParseError::NoNodes)
    } else {
        Ok(nodes)
    }
}

/// `Some(entries)` if the body is a YAML mapping with a `proxies` key (possibly empty).
fn parse_yaml_proxies(body: &str) -> Option<Vec<Value>> {
    let mut doc: serde_yaml_ng::Value = serde_yaml_ng::from_str(body).ok()?;
    // Resolve `<<: *anchor` merge keys, or merged fields would be lost and a
    // literal `<<` key would reach mihomo.
    doc.apply_merge().ok()?;
    let proxies = doc.as_mapping()?.get("proxies")?;
    let seq = proxies.as_sequence().map(Vec::as_slice).unwrap_or_default();
    Some(seq.iter().filter_map(|e| serde_json::to_value(e).ok()).collect())
}

fn looks_like_link(line: &str) -> bool {
    match line.split_once("://") {
        Some((scheme, _)) => {
            !scheme.is_empty() && scheme.chars().all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == '.')
        }
        None => false,
    }
}

/// Share links from a plain-text body, or from a base64-wrapped one.
fn parse_link_lines(body: &str) -> Result<Vec<Value>, ParseError> {
    let decoded;
    let text = if body.lines().any(|l| looks_like_link(l.trim())) {
        body
    } else {
        decoded = b64_decode_lenient(body)
            .and_then(|b| String::from_utf8(b).ok())
            .ok_or(ParseError::Unrecognized)?;
        &decoded
    };
    let candidates: Vec<&str> = text.lines().map(str::trim).filter(|l| looks_like_link(l)).collect();
    if candidates.is_empty() {
        return Err(ParseError::Unrecognized);
    }
    Ok(candidates.into_iter().filter_map(parse_share_link).collect())
}

/// Keep entries with `name`/`type`/`server`, dedupe names (` #2`, ` #3`, ...) and write the
/// final name back into `raw["name"]`.
fn build_nodes(entries: Vec<Value>) -> Vec<Node> {
    let mut used: HashSet<String> = HashSet::new();
    let mut nodes = Vec::new();
    for mut raw in entries {
        let field = |k: &str| raw.get(k).and_then(Value::as_str).filter(|s| !s.is_empty()).map(str::to_owned);
        let (Some(name), Some(proxy_type), Some(server)) = (field("name"), field("type"), field("server")) else {
            continue;
        };
        let mut unique = name.clone();
        let mut n = 2;
        while used.contains(&unique) {
            unique = format!("{name} #{n}");
            n += 1;
        }
        used.insert(unique.clone());
        raw["name"] = Value::String(unique.clone());
        nodes.push(Node { name: unique, proxy_type, server, raw });
    }
    nodes
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const CLASH: &str = include_str!("../../tests/fixtures/sub_clash.yaml");
    const B64: &str = include_str!("../../tests/fixtures/sub_base64.txt");
    const HTML: &str = include_str!("../../tests/fixtures/sub_html.html");
    const EMPTY: &str = include_str!("../../tests/fixtures/sub_empty.yaml");

    #[test]
    fn parses_clash_yaml() {
        let nodes = parse_subscription(CLASH).unwrap();
        let got: Vec<(&str, &str, &str)> = nodes
            .iter()
            .map(|n| (n.name.as_str(), n.proxy_type.as_str(), n.server.as_str()))
            .collect();
        assert_eq!(
            got,
            vec![
                ("香港01", "ss", "hk1.example.com"),
                ("美国: \"LA\" 🚀", "vmess", "us1.example.com"),
                ("日本03", "trojan", "2001:db8::1"),
            ]
        );
        // raw keeps every original field
        assert_eq!(nodes[0].raw["cipher"], json!("aes-256-gcm"));
        assert_eq!(nodes[0].raw["port"], json!(8388));
        assert_eq!(nodes[0].raw["udp"], json!(true));
        assert_eq!(nodes[1].raw["ws-opts"]["headers"]["Host"], json!("us1.example.com"));
        assert_eq!(nodes[1].raw["name"], json!("美国: \"LA\" 🚀"));
    }

    #[test]
    fn parses_base64_links() {
        let nodes = parse_subscription(B64).unwrap();
        assert_eq!(nodes.len(), 8);
        let expect = [
            ("ss", "1.1.1.1", 8388),
            ("ss", "2.2.2.2", 8389),
            ("vmess", "3.3.3.3", 443),
            ("vless", "4.4.4.4", 443),
            ("trojan", "5.5.5.5", 443),
            ("hysteria2", "6.6.6.6", 8443),
            ("hysteria2", "7.7.7.7", 8444),
            ("tuic", "8.8.8.8", 443),
        ];
        for (n, (t, s, p)) in nodes.iter().zip(expect) {
            assert_eq!(n.raw["type"], json!(t), "{}", n.name);
            assert_eq!(n.raw["server"], json!(s), "{}", n.name);
            assert_eq!(n.raw["port"], json!(p), "{}", n.name);
            assert_eq!(n.raw["name"], json!(n.name));
            assert_eq!(n.proxy_type, t);
            assert_eq!(n.server, s);
        }
        assert_eq!(nodes[0].name, "SS节点");
        assert_eq!(nodes[0].raw["cipher"], json!("aes-256-gcm"));
        assert_eq!(nodes[0].raw["password"], json!("pass123"));
        assert_eq!(nodes[1].raw["cipher"], json!("aes-128-gcm"));
        assert_eq!(nodes[1].raw["password"], json!("legacypw"));

        let vm = &nodes[2].raw;
        assert_eq!(nodes[2].name, "VM节点");
        assert_eq!(vm["uuid"], json!("22222222-2222-2222-2222-222222222222"));
        assert_eq!(vm["alterId"], json!(0));
        assert_eq!(vm["cipher"], json!("auto"));
        assert_eq!(vm["network"], json!("ws"));
        assert_eq!(vm["tls"], json!(true));
        assert_eq!(vm["servername"], json!("vm.example.com"));
        assert_eq!(vm["ws-opts"]["path"], json!("/vmws"));
        assert_eq!(vm["ws-opts"]["headers"]["Host"], json!("vm.example.com"));

        let vl = &nodes[3].raw;
        assert_eq!(nodes[3].name, "VLESS Reality");
        assert_eq!(vl["flow"], json!("xtls-rprx-vision"));
        assert_eq!(vl["tls"], json!(true));
        assert_eq!(vl["reality-opts"]["public-key"], json!("PUBKEY123"));
        assert_eq!(vl["reality-opts"]["short-id"], json!("ab12"));
        assert_eq!(vl["client-fingerprint"], json!("chrome"));

        let tj = &nodes[4].raw;
        assert_eq!(tj["password"], json!("trojanpw"));
        assert_eq!(tj["sni"], json!("tj.example.com"));
        assert_eq!(tj["skip-cert-verify"], json!(true));
        assert_eq!(tj["network"], json!("ws"));
        assert_eq!(tj["ws-opts"]["path"], json!("/tj"));

        let hy = &nodes[5].raw;
        assert_eq!(hy["password"], json!("hy2pw"));
        assert_eq!(hy["obfs"], json!("salamander"));
        assert_eq!(hy["obfs-password"], json!("obfspw"));
        assert_eq!(hy["skip-cert-verify"], json!(true));

        let tu = &nodes[7].raw;
        assert_eq!(tu["uuid"], json!("44444444-4444-4444-4444-444444444444"));
        assert_eq!(tu["password"], json!("tuicpw"));
        assert_eq!(tu["congestion-controller"], json!("bbr"));
        assert_eq!(tu["alpn"], json!(["h3", "h2"]));
    }

    #[test]
    fn resolves_yaml_merge_keys() {
        let body = "base: &base
  type: ss
  cipher: aes-128-gcm
  password: pw
  port: 443
proxies:
  - <<: *base
    name: m1
    server: 1.1.1.1
  - <<: *base
    name: m2
    server: 2.2.2.2
    port: 8443
";
        let nodes = parse_subscription(body).unwrap();
        assert_eq!(nodes.len(), 2);
        assert_eq!(nodes[0].proxy_type, "ss");
        assert_eq!(nodes[0].raw["cipher"], json!("aes-128-gcm"));
        assert_eq!(nodes[0].raw["port"], json!(443));
        assert_eq!(nodes[1].raw["port"], json!(8443));
        for n in &nodes {
            assert!(n.raw.get("<<").is_none(), "{:?}", n.raw);
        }
    }

    #[test]
    fn dedupes_names_within_subscription() {
        let body = "proxies:\n  - {name: 香港01, type: ss, server: a.com, port: 1}\n  - {name: 香港01, type: ss, server: b.com, port: 2}\n  - {name: 香港01, type: ss, server: c.com, port: 3}\n";
        let nodes = parse_subscription(body).unwrap();
        let names: Vec<&str> = nodes.iter().map(|n| n.name.as_str()).collect();
        assert_eq!(names, vec!["香港01", "香港01 #2", "香港01 #3"]);
        assert_eq!(nodes[1].raw["name"], json!("香港01 #2"));
    }

    #[test]
    fn html_page_is_no_nodes() {
        let r = parse_subscription(HTML);
        assert!(matches!(r, Err(ParseError::NoNodes) | Err(ParseError::Unrecognized)), "{r:?}");
    }

    #[test]
    fn empty_proxies_is_no_nodes() {
        assert_eq!(parse_subscription(EMPTY), Err(ParseError::NoNodes));
    }

    #[test]
    fn empty_body_is_never_ok() {
        assert!(parse_subscription("").is_err());
        assert!(parse_subscription("  \n").is_err());
    }

    #[test]
    fn unknown_link_lines_are_skipped() {
        let plain = "foo://x\ntrojan://pw@5.5.5.5:443#T1\n\nnot a link\n";
        let nodes = parse_subscription(plain).unwrap();
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].name, "T1");
        // base64-wrapped, same result
        use base64::Engine;
        let enc = base64::engine::general_purpose::STANDARD.encode(plain);
        assert_eq!(parse_subscription(&enc).unwrap().len(), 1);
    }

    #[test]
    fn trojan_password_with_colon_is_kept_whole() {
        let nodes = parse_subscription("trojan://pa:ss:word@5.5.5.5:443#T
").unwrap();
        assert_eq!(nodes[0].raw["password"], json!("pa:ss:word"));
        let nodes = parse_subscription("trojan://p%3Aw@5.5.5.5:443#T
").unwrap();
        assert_eq!(nodes[0].raw["password"], json!("p:w"));
    }

    #[test]
    fn only_unparseable_links_is_no_nodes() {
        assert_eq!(parse_subscription("foo://x\nbar://y"), Err(ParseError::NoNodes));
    }
}

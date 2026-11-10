use routa_procsvc_proto::FaultPolicy;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

const SCHEMA_VERSION: u32 = 1;
const MAX_ICON_LEN: usize = 64 * 1024;
pub(crate) const EXE_ROOTS: [&str; 4] = ["%LOCALAPPDATA%", "%PROGRAMFILES%", "%PROGRAMFILES(X86)%", "%APPDATA%"];

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, specta::Type)]
pub struct Catalog {
    pub schema_version: u32,
    pub apps: Vec<CatalogApp>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, specta::Type)]
pub struct CatalogApp {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub download_url: String,
    pub test_url: String,
    #[serde(default = "block")]
    pub on_fault: FaultPolicy,
    pub install: Vec<InstallSource>,
    pub processes: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, specta::Type)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum InstallSource {
    Store { family: String, app_id: String },
    Exe { path: String },
}

fn block() -> FaultPolicy {
    FaultPolicy::Block
}

/// 任一校验失败 → Err，整份作废。
pub fn parse_catalog(json: &str) -> Result<Catalog, String> {
    let c: Catalog = serde_json::from_str(json).map_err(|e| format!("解析失败: {e}"))?;
    validate(&c)?;
    Ok(c)
}

pub fn bundled() -> Catalog {
    parse_catalog(include_str!("../../../presets/ai-apps.json")).expect("内置 ai-apps.json 非法")
}

fn validate(c: &Catalog) -> Result<(), String> {
    if c.schema_version != SCHEMA_VERSION {
        return Err(format!("不支持的 schema_version: {}", c.schema_version));
    }
    let mut ids = HashSet::new();
    for a in &c.apps {
        validate_app(a).map_err(|e| format!("应用 {:?}: {e}", a.id))?;
        if !ids.insert(a.id.as_str()) {
            return Err(format!("重复的应用 id: {}", a.id));
        }
    }
    Ok(())
}

fn validate_app(a: &CatalogApp) -> Result<(), String> {
    let id_ok = (1..=32).contains(&a.id.len())
        && a.id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
    if !id_ok {
        return Err("id 须匹配 ^[a-z0-9-]{1,32}$".into());
    }
    if a.name.trim().is_empty() {
        return Err("name 不能为空".into());
    }
    if !a.icon.starts_with("data:image/") || a.icon.len() > MAX_ICON_LEN {
        return Err("icon 须为 data:image/ URI 且不超过 64 KiB".into());
    }
    if !is_https_url(&a.download_url) || !is_https_url(&a.test_url) {
        return Err("download_url / test_url 须为 https://，且不含 , \" ' ^ \\ ` 或空白".into());
    }
    if a.install.is_empty() {
        return Err("install 不能为空".into());
    }
    for s in &a.install {
        validate_install(s)?;
    }
    if a.processes.is_empty() {
        return Err("processes 不能为空".into());
    }
    let mut seen = HashSet::new();
    for p in &a.processes {
        if p.is_empty() || p.trim() != p || !routa_procsvc_proto::is_process_name(p) {
            return Err(format!("非法进程名: {p}"));
        }
        // 与模型校验一致，否则规则写入时会被拒绝。
        if let Some(reason) = routa_procsvc_proto::process_name_problem(p) {
            return Err(format!("非法进程名 {p}: {reason}"));
        }
        if !p.to_ascii_lowercase().ends_with(".exe") {
            return Err(format!("进程名须以 .exe 结尾: {p}"));
        }
        if !seen.insert(p.to_lowercase()) {
            return Err(format!("重复的进程名: {p}"));
        }
    }
    Ok(())
}

/// 下载页经 explorer.exe 打开：它按逗号拆参数、可能把引号或转义后的内容当作程序路径，
/// 所以除 https 外还拒绝 `, " ' ^ \ `` ` `` 和空白。
fn is_https_url(s: &str) -> bool {
    !s.chars().any(|c| c.is_whitespace() || matches!(c, ',' | '"' | '\'' | '^' | '\\' | '`'))
        && url::Url::parse(s).is_ok_and(|u| u.scheme() == "https" && u.host_str().is_some_and(|h| !h.is_empty()))
}

fn validate_install(s: &InstallSource) -> Result<(), String> {
    match s {
        InstallSource::Store { family, app_id } => {
            let bad = |v: &str| v.is_empty() || v.contains(['!', '\\', '/']);
            if bad(family) || bad(app_id) {
                return Err("store 的 family / app_id 非法".into());
            }
        }
        InstallSource::Exe { path } => {
            let rooted = EXE_ROOTS
                .iter()
                .any(|r| {
                    path.len() > r.len() + 1
                        && path.is_char_boundary(r.len())
                        && path[..r.len()].eq_ignore_ascii_case(r)
                        && path.as_bytes()[r.len()] == b'\\'
                });
            if !rooted || !path.to_ascii_lowercase().ends_with(".exe") || path.contains("..") {
                return Err(format!("exe 路径非法: {path}"));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app_json(over: &[(&str, &str)]) -> String {
        let mut fields: Vec<(&str, String)> = vec![
            ("id", r#""a""#.into()),
            ("name", r#""A""#.into()),
            ("icon", r#""data:image/svg+xml;base64,AAAA""#.into()),
            ("download_url", r#""https://x.com/d""#.into()),
            ("test_url", r#""https://x.com""#.into()),
            ("install", r#"[{"type":"exe","path":"%LOCALAPPDATA%\\a\\a.exe"}]"#.into()),
            ("processes", r#"["a.exe"]"#.into()),
        ];
        for (k, v) in over {
            fields.retain(|(fk, _)| fk != k);
            fields.push((k, v.to_string()));
        }
        let body: Vec<String> = fields.iter().map(|(k, v)| format!("\"{k}\":{v}")).collect();
        format!("{{\"schema_version\":1,\"apps\":[{{{}}}]}}", body.join(","))
    }

    #[test]
    fn bundled_catalog_is_valid() {
        let c = bundled();
        assert_eq!(c.schema_version, 1);
        assert_eq!(c.apps.iter().map(|a| a.id.as_str()).collect::<Vec<_>>(), ["chatgpt", "claude", "cursor"]);
        assert!(c.apps.iter().all(|a| a.on_fault == FaultPolicy::Block));
    }

    #[test]
    fn accepts_minimal_app() {
        assert!(parse_catalog(&app_json(&[])).is_ok());
    }

    #[test]
    fn rejects_wrong_schema_version() {
        assert!(parse_catalog(r#"{"schema_version":2,"apps":[]}"#).is_err());
    }

    #[test]
    fn rejects_non_data_icon() {
        assert!(parse_catalog(&app_json(&[("icon", r#""https://x/y.png""#)])).is_err());
    }

    #[test]
    fn rejects_http_urls() {
        assert!(parse_catalog(&app_json(&[("download_url", r#""http://x.com""#)])).is_err());
        assert!(parse_catalog(&app_json(&[("test_url", r#""http://x.com""#)])).is_err());
        assert!(parse_catalog(&app_json(&[("test_url", r#""https://""#)])).is_err());
        assert!(parse_catalog(&app_json(&[("download_url", r#""https://:443/p""#)])).is_err());
        assert!(parse_catalog(&app_json(&[("test_url", r#""https://""#)])).is_err());
        assert!(parse_catalog(&app_json(&[("download_url", r#""https://:443/p""#)])).is_err());
    }

    #[test]
    fn rejects_url_chars_that_explorer_could_misread() {
        // JSON-escaped: \" is a quote, \\ a backslash, \t a tab.
        for c in [",", "\\\"", "'", "^", "\\\\", "`", " ", "\\t"] {
            for field in ["download_url", "test_url"] {
                let v = format!("\"https://x.com/a{c}b\"");
                assert!(parse_catalog(&app_json(&[(field, &v)])).is_err(), "{field} with {c:?}");
            }
        }
    }

    #[test]
    fn rejects_process_names_the_model_rejects() {
        for p in ["*.exe", "a?.exe", "a;b.exe", ".exe"] {
            let v = format!("[\"{p}\"]");
            assert!(parse_catalog(&app_json(&[("processes", &v)])).is_err(), "{p}");
        }
    }

    #[test]
    fn rejects_process_with_path() {
        assert!(parse_catalog(&app_json(&[("processes", r#"["C:\\a\\b.exe"]"#)])).is_err());
        assert!(parse_catalog(&app_json(&[("processes", r#"["a"]"#)])).is_err());
        assert!(parse_catalog(&app_json(&[("processes", "[]")])).is_err());
        assert!(parse_catalog(&app_json(&[("processes", r#"["A.exe","a.EXE"]"#)])).is_err());
    }

    #[test]
    fn rejects_exe_outside_known_roots() {
        let bad = |p: &str| {
            let install = format!(r#"[{{"type":"exe","path":"{p}"}}]"#);
            parse_catalog(&app_json(&[("install", &install)])).is_err()
        };
        assert!(bad(r"C:\\Windows\\cmd.exe"));
        assert!(bad(r"%LOCALAPPDATA%\\..\\x.exe"));
        assert!(bad(r"%LOCALAPPDATA%\\x.dll"));
        assert!(bad(r"%LOCALAPPDATA%x.exe"));
        assert!(bad(r"%PROGRAMFILES%Foo\\x.exe"));
    }

    #[test]
    fn rejects_bad_store_and_empty_install() {
        let install = r#"[{"type":"store","family":"a!b","app_id":"App"}]"#;
        assert!(parse_catalog(&app_json(&[("install", install)])).is_err());
        assert!(parse_catalog(&app_json(&[("install", "[]")])).is_err());
    }

    #[test]
    fn rejects_bad_id() {
        assert!(parse_catalog(&app_json(&[("id", r#""Bad_Id""#)])).is_err());
        assert!(parse_catalog(&app_json(&[("id", r#""""#)])).is_err());
    }

    #[test]
    fn rejects_duplicate_ids() {
        let one = app_json(&[]);
        let app = one.split_once("\"apps\":[").unwrap().1.trim_end_matches("]}");
        let json = format!("{{\"schema_version\":1,\"apps\":[{app},{app}]}}");
        assert!(parse_catalog(&json).is_err());
    }

    #[test]
    fn on_fault_defaults_to_block_and_accepts_direct() {
        assert_eq!(parse_catalog(&app_json(&[])).unwrap().apps[0].on_fault, FaultPolicy::Block);
        let c = parse_catalog(&app_json(&[("on_fault", r#""Direct""#)])).unwrap();
        assert_eq!(c.apps[0].on_fault, FaultPolicy::Direct);
    }
}

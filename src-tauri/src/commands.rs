//! Tauri command layer. The logic lives in `ops` (plain functions over
//! `&Runtime`, testable without a Tauri runtime); the `#[tauri::command]`
//! wrappers below are thin.

use std::collections::HashMap;
use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, State};
use tauri_plugin_autostart::ManagerExt;

use crate::core::apply::ApplyOutcome;
use crate::model::*;
use crate::runtime::{Runtime, RuntimeStatus};
use crate::stats::delay::DelayResult;

type Rt<'a> = State<'a, Arc<Runtime>>;
pub type CmdResult = Result<ApplyOutcome, Vec<ValidationError>>;

fn upsert<T>(items: &mut Vec<T>, item: T, same: impl Fn(&T) -> bool) {
    match items.iter_mut().find(|x| same(x)) {
        Some(slot) => *slot = item,
        None => items.push(item),
    }
}

/// Plain functions over the runtime (no Tauri types).
pub mod ops {
    use super::*;

    pub async fn save_listener(rt: &Runtime, l: Listener) -> CmdResult {
        rt.mutate(|m| {
            let id = l.id.clone();
            upsert(&mut m.listeners, l, |x| x.id == id)
        })
        .await
    }

    pub async fn delete_listener(rt: &Runtime, id: ListenerId) -> CmdResult {
        rt.mutate(|m| m.listeners.retain(|l| l.id != id)).await
    }

    pub async fn save_exit(rt: &Runtime, e: Exit) -> CmdResult {
        rt.mutate(|m| {
            let id = e.id.clone();
            upsert(&mut m.exits, e, |x| x.id == id)
        })
        .await
    }

    /// Rejected with `DanglingRef` while a listener or subscription fallback
    /// still references the exit (validation fails, nothing changes).
    pub async fn delete_exit(rt: &Runtime, id: ExitId) -> CmdResult {
        rt.mutate(|m| m.exits.retain(|e| e.id != id)).await
    }

    /// Upsert. Returns the outcome and whether the subscription should be
    /// fetched now (new, or its URL changed). An existing subscription keeps
    /// its server-side nodes and fetch state: the caller's copy may be stale.
    /// The merge runs inside the mutation, against the model being edited.
    pub async fn save_subscription(
        rt: &Runtime,
        s: Subscription,
    ) -> Result<(ApplyOutcome, bool), Vec<ValidationError>> {
        rt.mutate_with(|m| {
            let mut s = s;
            let mut fetch_now = true;
            if let Some(old) = m.subscriptions.iter().find(|x| x.id == s.id) {
                fetch_now = old.url != s.url;
                s.nodes = old.nodes.clone();
                s.last_updated = old.last_updated;
                s.last_result = old.last_result.clone();
            }
            let id = s.id.clone();
            upsert(&mut m.subscriptions, s, |x| x.id == id);
            Ok(fetch_now)
        })
        .await
    }

    pub async fn delete_subscription(rt: &Runtime, id: SubscriptionId) -> CmdResult {
        rt.mutate(|m| m.subscriptions.retain(|s| s.id != id)).await
    }

    /// Errors (`DanglingRef`) when the exit is missing, not a group, or
    /// `member` is not one of its expanded member nodes.
    pub async fn select_group_member(rt: &Runtime, exit: ExitId, member: NodeRef) -> CmdResult {
        rt.mutate_with(|m| {
            let bad = |what: &str| Err(vec![ValidationError::DanglingRef { what: what.to_string() }]);
            let Some(i) = m.exits.iter().position(|e| e.id == exit) else {
                return bad("出口不存在");
            };
            let is_member = match &m.exits[i].kind {
                ExitKind::Group { members, .. } => expand(m, members).nodes.contains(&member),
                _ => false,
            };
            match &mut m.exits[i].kind {
                ExitKind::Group { selected, .. } if is_member => {
                    *selected = Some(member);
                    Ok(())
                }
                ExitKind::Group { .. } => bad("该节点不是此出口组的成员"),
                _ => bad("该出口不是出口组"),
            }
        })
        .await
        .map(|(outcome, ())| outcome)
    }

    pub async fn save_settings(rt: &Runtime, s: Settings) -> CmdResult {
        rt.mutate(|m| m.settings = s).await
    }

    pub async fn save_manual_node(rt: &Runtime, n: ManualNode) -> CmdResult {
        rt.mutate(|m| {
            let id = n.id.clone();
            upsert(&mut m.manual_nodes, n, |x| x.id == id)
        })
        .await
    }

    /// Rejected with `DanglingRef` while an exit references the node. A
    /// deleted Tailscale instance also loses its state dir
    /// (`<core_workdir>/tailscale/<id>`); failing to remove it is only logged.
    pub async fn delete_manual_node(rt: &Runtime, id: ManualNodeId) -> CmdResult {
        let (outcome, was_tailscale) = rt
            .mutate_with(|m| {
                let ts = m
                    .manual_nodes
                    .iter()
                    .any(|n| n.id == id && matches!(n.kind, ManualNodeKind::Tailscale(_)));
                m.manual_nodes.retain(|n| n.id != id);
                Ok(ts)
            })
            .await?;
        if was_tailscale {
            remove_tailscale_state(rt, &id);
        }
        Ok(outcome)
    }

    fn remove_tailscale_state(rt: &Runtime, id: &ManualNodeId) {
        // The id names a directory: never let it step outside tailscale/.
        let plain = !id.0.is_empty() && id.0 != "." && id.0 != ".." && !id.0.contains(['/', '\\', ':']);
        if !plain {
            eprintln!("delete_manual_node: refusing to remove state dir for id {:?}", id.0);
            return;
        }
        let dir = rt.core_workdir().join(tailscale_state_dir(id));
        match std::fs::remove_dir_all(&dir) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => eprintln!("delete_manual_node: removing {} failed: {e}", dir.display()),
        }
    }

    pub async fn save_node_group(rt: &Runtime, g: NodeGroup) -> CmdResult {
        rt.mutate(|m| {
            let id = g.id.clone();
            upsert(&mut m.node_groups, g, |x| x.id == id)
        })
        .await
    }

    /// Also drops the group from every manual node's `groups`. Rejected with
    /// `DanglingRef` while an exit uses the group as a member.
    pub async fn delete_node_group(rt: &Runtime, id: NodeGroupId) -> CmdResult {
        rt.mutate(|m| {
            m.node_groups.retain(|g| g.id != id);
            for n in &mut m.manual_nodes {
                n.groups.retain(|g| *g != id);
            }
        })
        .await
    }

    /// Keys a pasted proxy must not carry: the exit decides the interface,
    /// and chaining through other proxies is not supported.
    const STRIPPED_KEYS: [&str; 3] = ["interface-name", "routing-mark", "dialer-proxy"];

    /// Parses one pasted mihomo proxy: a mapping, or a one-element sequence
    /// (`- {...}`). `type: tailscale` is returned as-is (the UI converts it).
    pub fn parse_proxy_yaml(text: String) -> Result<ParsedProxy, String> {
        use serde_json::Value;
        let v: Value = serde_yaml_ng::from_str(&text).map_err(|e| format!("YAML 解析失败：{e}"))?;
        let v = match v {
            Value::Array(mut items) => match items.len() {
                1 => items.remove(0),
                0 => return Err("没有找到节点".into()),
                n => return Err(format!("一次只能添加一个节点，这里有 {n} 个")),
            },
            other => other,
        };
        let Value::Object(mut map) = v else {
            return Err("节点必须是一个映射（如 name: …、type: …）".into());
        };
        for key in ["name", "type"] {
            if map.get(key).and_then(Value::as_str).is_none_or(|s| s.trim().is_empty()) {
                return Err(format!("节点缺少 {key}"));
            }
        }
        let stripped = STRIPPED_KEYS
            .iter()
            .filter(|k| map.remove(**k).is_some())
            .map(|k| k.to_string())
            .collect();
        Ok(ParsedProxy { raw: Value::Object(map), stripped })
    }
}

/// A pasted proxy, ready to become `ManualNodeKind::Proxy { raw }`.
#[derive(Serialize, Clone, Debug, PartialEq, specta::Type)]
pub struct ParsedProxy {
    pub raw: serde_json::Value,
    /// Keys removed from the paste, a subset of
    /// `["interface-name", "routing-mark", "dialer-proxy"]` in that order.
    pub stripped: Vec<String>,
}

#[tauri::command]
#[specta::specta]
pub fn get_model(rt: Rt<'_>) -> Model {
    rt.model()
}

#[tauri::command]
#[specta::specta]
pub fn get_status(rt: Rt<'_>) -> RuntimeStatus {
    rt.status()
}

#[tauri::command]
#[specta::specta]
pub async fn save_listener(rt: Rt<'_>, l: Listener) -> CmdResult {
    ops::save_listener(&rt, l).await
}

#[tauri::command]
#[specta::specta]
pub async fn delete_listener(rt: Rt<'_>, id: ListenerId) -> CmdResult {
    ops::delete_listener(&rt, id).await
}

#[tauri::command]
#[specta::specta]
pub async fn save_exit(rt: Rt<'_>, e: Exit) -> CmdResult {
    ops::save_exit(&rt, e).await
}

#[tauri::command]
#[specta::specta]
pub async fn delete_exit(rt: Rt<'_>, id: ExitId) -> CmdResult {
    ops::delete_exit(&rt, id).await
}

#[tauri::command]
#[specta::specta]
pub async fn save_subscription(rt: Rt<'_>, s: Subscription) -> CmdResult {
    let id = s.id.clone();
    let (outcome, fetch_now) = ops::save_subscription(&rt, s).await?;
    if fetch_now {
        // Do not hold the command open for the network fetch.
        let rt = Arc::clone(&rt);
        tauri::async_runtime::spawn(async move {
            rt.refresh_subscription(id).await;
        });
    }
    Ok(outcome)
}

#[tauri::command]
#[specta::specta]
pub async fn delete_subscription(rt: Rt<'_>, id: SubscriptionId) -> CmdResult {
    ops::delete_subscription(&rt, id).await
}

#[tauri::command]
#[specta::specta]
pub async fn refresh_subscription(rt: Rt<'_>, id: SubscriptionId) -> Result<FetchResult, ()> {
    Ok(rt.refresh_subscription(id).await)
}

#[tauri::command]
#[specta::specta]
pub async fn select_group_member(rt: Rt<'_>, exit: ExitId, member: NodeRef) -> CmdResult {
    ops::select_group_member(&rt, exit, member).await
}

#[tauri::command]
#[specta::specta]
pub async fn test_delays(rt: Rt<'_>, proxy_names: Vec<String>) -> Result<Vec<(String, DelayResult)>, ()> {
    Ok(rt.test_delays(proxy_names).await)
}

#[tauri::command]
#[specta::specta]
pub async fn save_settings(app: AppHandle, rt: Rt<'_>, s: Settings) -> CmdResult {
    let want_autostart = s.autostart;
    let outcome = ops::save_settings(&rt, s).await?;
    // The setting is saved either way; a failing autostart toggle is only logged.
    let mgr = app.autolaunch();
    let res = match mgr.is_enabled() {
        Ok(on) if on == want_autostart => Ok(()),
        _ if want_autostart => mgr.enable(),
        _ => mgr.disable(),
    };
    if let Err(e) = res {
        eprintln!("autostart: {e}");
    }
    Ok(outcome)
}

#[tauri::command]
#[specta::specta]
pub async fn save_manual_node(rt: Rt<'_>, n: ManualNode) -> CmdResult {
    ops::save_manual_node(&rt, n).await
}

#[tauri::command]
#[specta::specta]
pub async fn delete_manual_node(rt: Rt<'_>, id: ManualNodeId) -> CmdResult {
    ops::delete_manual_node(&rt, id).await
}

#[tauri::command]
#[specta::specta]
pub async fn save_node_group(rt: Rt<'_>, g: NodeGroup) -> CmdResult {
    ops::save_node_group(&rt, g).await
}

#[tauri::command]
#[specta::specta]
pub async fn delete_node_group(rt: Rt<'_>, id: NodeGroupId) -> CmdResult {
    ops::delete_node_group(&rt, id).await
}

#[tauri::command]
#[specta::specta]
pub fn parse_proxy_yaml(text: String) -> Result<ParsedProxy, String> {
    ops::parse_proxy_yaml(text)
}

/// Exit name -> member mihomo currently uses; unknown names are omitted.
#[tauri::command]
#[specta::specta]
pub async fn group_now(rt: Rt<'_>, names: Vec<String>) -> Result<HashMap<String, String>, ()> {
    Ok(rt.group_now(names).await)
}

#[tauri::command]
#[specta::specta]
pub async fn reset_traffic(rt: Rt<'_>) -> Result<(), ()> {
    rt.reset_traffic().await;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn core_start(rt: Rt<'_>) -> Result<(), ()> {
    rt.restart_core().await;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn core_stop(rt: Rt<'_>) -> Result<(), ()> {
    rt.stop_core().await;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn core_restart(rt: Rt<'_>) -> Result<(), ()> {
    rt.restart_core().await;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn open_log_dir(rt: Rt<'_>) -> Result<(), String> {
    std::process::Command::new("explorer.exe")
        .arg(rt.core_workdir())
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("无法打开日志目录: {e}"))
}

#[cfg(test)]
mod tests {
    use std::io;
    use std::path::Path;
    use std::sync::{Arc, Mutex};

    use async_trait::async_trait;
    use tokio::sync::mpsc;

    use super::ops;
    use crate::core::api::MihomoApi;
    use crate::core::fake::FakeApi;
    use crate::core::ports::FakePortOwners;
    use crate::core::process::{CheckError, CoreHandleLike, CoreLauncher};
    use crate::model::*;
    use crate::netif::FakeInterfaces;
    use crate::runtime::{ApiFor, LauncherFor, Runtime};
    use crate::store::Store;

    /// Launching always fails: these tests only exercise the model layer.
    struct NoLauncher;

    #[async_trait]
    impl CoreLauncher for NoLauncher {
        async fn launch(&self, _config: &Path) -> io::Result<Box<dyn CoreHandleLike + Send>> {
            Err(io::Error::other("no core in tests"))
        }
        async fn check_config(&self, _config: &Path) -> Result<(), CheckError> {
            Err(CheckError::Unavailable("no core in tests".into()))
        }
    }

    fn runtime(td: &tempfile::TempDir) -> Runtime {
        let store = Store::open(td.path().join("data")).unwrap();
        let launcher_for: LauncherFor = Arc::new(|_s: &Settings| Arc::new(NoLauncher) as Arc<dyn CoreLauncher>);
        let api_for: ApiFor = Box::new(|_, _| Arc::new(FakeApi::new()) as Arc<dyn MihomoApi>);
        let (tx, _rx) = mpsc::channel(16);
        Runtime::new(
            store,
            Arc::new(FakeInterfaces(Mutex::new(vec![]))),
            launcher_for,
            api_for,
            Arc::new(FakePortOwners(Mutex::new(Default::default()))),
            tx,
        )
    }

    fn direct_exit(name: &str) -> Exit {
        Exit { id: ExitId::new(), name: name.into(), kind: ExitKind::Direct, interface: None }
    }

    #[tokio::test]
    async fn delete_exit_in_use_is_rejected() {
        let td = tempfile::tempdir().unwrap();
        let rt = runtime(&td);
        let exit = direct_exit("直连");
        let mut l = Listener::new_default("L1".into(), 7891);
        l.exit = Some(exit.id.clone());
        ops::save_exit(&rt, exit.clone()).await.unwrap();
        ops::save_listener(&rt, l).await.unwrap();

        let errs = ops::delete_exit(&rt, exit.id.clone()).await.unwrap_err();
        assert!(errs.iter().any(|e| matches!(e, ValidationError::DanglingRef { .. })));
        assert_eq!(rt.model().exits.len(), 1, "nothing changed");
    }

    #[tokio::test]
    async fn delete_unused_exit_and_upsert_listener() {
        let td = tempfile::tempdir().unwrap();
        let rt = runtime(&td);
        let exit = direct_exit("直连");
        ops::save_exit(&rt, exit.clone()).await.unwrap();
        let mut l = Listener::new_default("L1".into(), 7891);
        ops::save_listener(&rt, l.clone()).await.unwrap();
        l.port = 7892;
        ops::save_listener(&rt, l.clone()).await.unwrap();
        assert_eq!(rt.model().listeners.len(), 1);
        assert_eq!(rt.model().listeners[0].port, 7892);
        ops::delete_exit(&rt, exit.id).await.unwrap();
        assert!(rt.model().exits.is_empty());
        ops::delete_listener(&rt, l.id).await.unwrap();
        assert!(rt.model().listeners.is_empty());
    }

    #[tokio::test]
    async fn select_group_member_sets_selected() {
        let td = tempfile::tempdir().unwrap();
        let rt = runtime(&td);
        let sub_id = SubscriptionId::new();
        let node = NodeRef::Subscription { subscription_id: sub_id.clone(), name: "n1".into() };
        let sub = Subscription {
            id: sub_id,
            name: "s".into(),
            url: "http://127.0.0.1:1/x".into(),
            update_interval_mins: None,
            fallback_exit: None,
            last_updated: None,
            last_result: None,
            nodes: vec![Node {
                name: "n1".into(),
                proxy_type: "ss".into(),
                server: "1.2.3.4".into(),
                raw: serde_json::json!({"name": "n1"}),
            }],
        };
        // Insert via the model directly: save_subscription would fetch.
        rt.mutate(|m| m.subscriptions.push(sub)).await.unwrap();
        let group = Exit {
            id: ExitId::new(),
            name: "组".into(),
            kind: ExitKind::Group {
                strategy: GroupStrategy::Select,
                members: vec![ExitMember::Node(node.clone())],
                selected: None,
            },
            interface: None,
        };
        ops::save_exit(&rt, group.clone()).await.unwrap();
        ops::select_group_member(&rt, group.id.clone(), node.clone()).await.unwrap();
        let m = rt.model();
        let ExitKind::Group { selected, .. } = &m.exits[0].kind else { panic!() };
        assert_eq!(selected.as_ref(), Some(&node));
    }

    fn sub(url: &str) -> Subscription {
        Subscription {
            id: SubscriptionId::new(),
            name: "s".into(),
            url: url.into(),
            update_interval_mins: None,
            fallback_exit: None,
            last_updated: Some(5),
            last_result: None,
            nodes: vec![],
        }
    }

    #[tokio::test]
    async fn save_subscription_fetch_now_rules() {
        let td = tempfile::tempdir().unwrap();
        let rt = runtime(&td);
        let mut s = sub("http://a/x");
        let (_, fetch) = ops::save_subscription(&rt, s.clone()).await.unwrap();
        assert!(fetch, "new -> fetch");

        // Server-side nodes exist; the caller's copy is stale (no nodes).
        let node = Node {
            name: "n".into(),
            proxy_type: "ss".into(),
            server: "1.1.1.1".into(),
            raw: serde_json::json!({"name": "n"}),
        };
        let id = s.id.clone();
        rt.mutate(|m| m.subscriptions.iter_mut().find(|x| x.id == id).unwrap().nodes = vec![node]).await.unwrap();

        s.name = "renamed".into();
        s.last_updated = None;
        let (_, fetch) = ops::save_subscription(&rt, s.clone()).await.unwrap();
        assert!(!fetch, "url unchanged -> no fetch");
        let saved = rt.model().subscriptions[0].clone();
        assert_eq!(saved.name, "renamed");
        assert_eq!(saved.nodes.len(), 1, "server-side nodes kept");
        assert_eq!(saved.last_updated, Some(5), "fetch state kept");

        s.url = "http://b/y".into();
        let (_, fetch) = ops::save_subscription(&rt, s).await.unwrap();
        assert!(fetch, "url changed -> fetch");
    }

    #[tokio::test]
    async fn select_group_member_rejects_bad_targets() {
        let td = tempfile::tempdir().unwrap();
        let rt = runtime(&td);
        let direct = direct_exit("直连");
        ops::save_exit(&rt, direct.clone()).await.unwrap();
        let node = NodeRef::Subscription { subscription_id: SubscriptionId::new(), name: "n".into() };
        let missing = ops::select_group_member(&rt, ExitId::new(), node.clone()).await;
        assert!(matches!(missing.unwrap_err()[0], ValidationError::DanglingRef { .. }));
        let not_group = ops::select_group_member(&rt, direct.id, node.clone()).await;
        assert!(matches!(not_group.unwrap_err()[0], ValidationError::DanglingRef { .. }));
        let group = Exit {
            id: ExitId::new(),
            name: "组".into(),
            kind: ExitKind::Group { strategy: GroupStrategy::Select, members: vec![], selected: None },
            interface: None,
        };
        ops::save_exit(&rt, group.clone()).await.unwrap();
        let not_member = ops::select_group_member(&rt, group.id, node).await;
        assert!(matches!(not_member.unwrap_err()[0], ValidationError::DanglingRef { .. }));
    }

    // ---- manual nodes / groups ----

    fn ts_node(name: &str, iface: Option<InterfaceGuid>) -> ManualNode {
        let mut o = TailscaleOpts::defaults_for(name);
        o.interface = iface;
        ManualNode { id: ManualNodeId::new(), name: name.into(), groups: vec![], kind: ManualNodeKind::Tailscale(o) }
    }

    fn proxy_node(name: &str, groups: Vec<NodeGroupId>) -> ManualNode {
        ManualNode {
            id: ManualNodeId::new(),
            name: name.into(),
            groups,
            kind: ManualNodeKind::Proxy {
                raw: serde_json::json!({"name": name, "type": "socks5", "server": "1.2.3.4", "port": 1080}),
            },
        }
    }

    fn mref(n: &ManualNode) -> NodeRef {
        NodeRef::Manual { manual_id: n.id.clone() }
    }

    #[test]
    fn parse_proxy_yaml_accepts_dash_and_reports_stripped() {
        let p = ops::parse_proxy_yaml("- {name: a, type: vless, server: x, port: 1, dialer-proxy: y}".into()).unwrap();
        assert_eq!(p.stripped, vec!["dialer-proxy".to_string()]);
        assert_eq!(p.raw["name"], "a");
        assert_eq!(p.raw["type"], "vless");
        assert!(p.raw.get("dialer-proxy").is_none());

        let p = ops::parse_proxy_yaml(
            "name: b\ntype: ss\nserver: x\nport: 2\ninterface-name: eth\nrouting-mark: 3\n".into(),
        )
        .unwrap();
        assert_eq!(p.stripped, vec!["interface-name".to_string(), "routing-mark".to_string()]);
        assert_eq!(p.raw["port"], 2);
        let ts = ops::parse_proxy_yaml("{name: t, type: tailscale}".into()).unwrap();
        assert_eq!(ts.raw["type"], "tailscale");
    }

    #[test]
    fn parse_proxy_yaml_rejects_two_and_missing_type() {
        assert!(ops::parse_proxy_yaml("- {name: a, type: ss}\n- {name: b, type: ss}".into()).is_err());
        assert!(ops::parse_proxy_yaml("{name: a, server: x}".into()).is_err());
        assert!(ops::parse_proxy_yaml("{type: ss}".into()).is_err());
        assert!(ops::parse_proxy_yaml("just a string".into()).is_err());
        assert!(ops::parse_proxy_yaml("[1, 2".into()).is_err());
        assert!(ops::parse_proxy_yaml("- 5".into()).is_err());
        assert!(ops::parse_proxy_yaml("[]".into()).is_err());
    }

    #[tokio::test]
    async fn delete_referenced_manual_node_rejected() {
        let td = tempfile::tempdir().unwrap();
        let rt = runtime(&td);
        let n = proxy_node("m1", vec![]);
        ops::save_manual_node(&rt, n.clone()).await.unwrap();
        let exit = Exit { id: ExitId::new(), name: "e".into(), kind: ExitKind::Node { node: mref(&n) }, interface: None };
        ops::save_exit(&rt, exit).await.unwrap();
        let before = rt.model();

        let errs = ops::delete_manual_node(&rt, n.id.clone()).await.unwrap_err();
        assert!(errs.iter().any(|e| matches!(e, ValidationError::DanglingRef { .. })), "{errs:?}");
        assert_eq!(rt.model(), before, "nothing changed");
    }

    #[tokio::test]
    async fn delete_group_removes_membership() {
        let td = tempfile::tempdir().unwrap();
        let rt = runtime(&td);
        let g = NodeGroup { id: NodeGroupId::new(), name: "家".into() };
        let keep = NodeGroup { id: NodeGroupId::new(), name: "留".into() };
        ops::save_node_group(&rt, g.clone()).await.unwrap();
        ops::save_node_group(&rt, keep.clone()).await.unwrap();
        ops::save_manual_node(&rt, proxy_node("a", vec![g.id.clone(), keep.id.clone()])).await.unwrap();
        ops::save_manual_node(&rt, proxy_node("b", vec![g.id.clone()])).await.unwrap();

        ops::delete_node_group(&rt, g.id.clone()).await.unwrap();
        let m = rt.model();
        assert_eq!(m.node_groups, vec![keep.clone()]);
        assert!(m.manual_nodes.iter().all(|n| !n.groups.contains(&g.id)));
        assert_eq!(m.manual_nodes[0].groups, vec![keep.id]);
    }

    #[tokio::test]
    async fn delete_referenced_group_rejected() {
        let td = tempfile::tempdir().unwrap();
        let rt = runtime(&td);
        let g = NodeGroup { id: NodeGroupId::new(), name: "家".into() };
        ops::save_node_group(&rt, g.clone()).await.unwrap();
        let exit = Exit {
            id: ExitId::new(),
            name: "e".into(),
            kind: ExitKind::Group {
                strategy: GroupStrategy::UrlTest,
                members: vec![ExitMember::Group { group: GroupRef::Custom(g.id.clone()) }],
                selected: None,
            },
            interface: None,
        };
        ops::save_exit(&rt, exit).await.unwrap();
        let errs = ops::delete_node_group(&rt, g.id).await.unwrap_err();
        assert!(errs.iter().any(|e| matches!(e, ValidationError::DanglingRef { .. })), "{errs:?}");
        assert_eq!(rt.model().node_groups.len(), 1);
    }

    #[tokio::test]
    async fn delete_tailscale_removes_state_dir() {
        let td = tempfile::tempdir().unwrap();
        let rt = runtime(&td);
        let t = ts_node("家里", None);
        ops::save_manual_node(&rt, t.clone()).await.unwrap();
        let dir = rt.core_workdir().join("tailscale").join(&t.id.0);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("tailscaled.state"), "x").unwrap();

        ops::delete_manual_node(&rt, t.id.clone()).await.unwrap();
        assert!(rt.model().manual_nodes.is_empty());
        assert!(!dir.exists(), "state dir removed");
        // Deleting an instance whose dir never existed still succeeds.
        let t2 = ts_node("b", None);
        ops::save_manual_node(&rt, t2.clone()).await.unwrap();
        ops::delete_manual_node(&rt, t2.id).await.unwrap();
    }

    #[tokio::test]
    async fn mutate_normalizes_tailscale_exit_interface() {
        let td = tempfile::tempdir().unwrap();
        let rt = runtime(&td);
        let t = ts_node("t", None);
        let p = proxy_node("p", vec![]);
        ops::save_manual_node(&rt, t.clone()).await.unwrap();
        ops::save_manual_node(&rt, p.clone()).await.unwrap();
        let only_ts =
            Exit { id: ExitId::new(), name: "ts".into(), kind: ExitKind::Node { node: mref(&t) }, interface: None };
        let other =
            Exit { id: ExitId::new(), name: "p".into(), kind: ExitKind::Node { node: mref(&p) }, interface: None };
        ops::save_exit(&rt, only_ts.clone()).await.unwrap();
        ops::save_exit(&rt, other.clone()).await.unwrap();

        let guid = Some(InterfaceGuid("{B}".into()));
        let mut t2 = t.clone();
        if let ManualNodeKind::Tailscale(o) = &mut t2.kind {
            o.interface = guid.clone();
        }
        ops::save_manual_node(&rt, t2).await.unwrap();
        let m = rt.model();
        assert_eq!(m.exits.iter().find(|e| e.id == only_ts.id).unwrap().interface, guid);
        assert_eq!(m.exits.iter().find(|e| e.id == other.id).unwrap().interface, None);
    }

    #[tokio::test]
    async fn select_member_from_group_expansion_ok() {
        let td = tempfile::tempdir().unwrap();
        let rt = runtime(&td);
        let mut s = sub("http://127.0.0.1:1/x");
        let n = |name: &str| Node {
            name: name.into(),
            proxy_type: "ss".into(),
            server: "1.2.3.4".into(),
            raw: serde_json::json!({"name": name}),
        };
        s.nodes = vec![n("HK"), n("JP")];
        let sid = s.id.clone();
        rt.mutate(|m| m.subscriptions.push(s)).await.unwrap();
        let group = Exit {
            id: ExitId::new(),
            name: "组".into(),
            kind: ExitKind::Group {
                strategy: GroupStrategy::Select,
                members: vec![ExitMember::Group { group: GroupRef::Subscription(sid.clone()) }],
                selected: None,
            },
            interface: None,
        };
        ops::save_exit(&rt, group.clone()).await.unwrap();
        let jp = NodeRef::Subscription { subscription_id: sid.clone(), name: "JP".into() };
        ops::select_group_member(&rt, group.id.clone(), jp.clone()).await.unwrap();
        let m = rt.model();
        let ExitKind::Group { selected, .. } = &m.exits[0].kind else { panic!() };
        assert_eq!(selected.as_ref(), Some(&jp));
        let gone = NodeRef::Subscription { subscription_id: sid, name: "XX".into() };
        assert!(ops::select_group_member(&rt, group.id, gone).await.is_err());
    }
}

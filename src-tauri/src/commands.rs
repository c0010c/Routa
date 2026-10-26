//! Tauri command layer. The logic lives in `ops` (plain functions over
//! `&Runtime`, testable without a Tauri runtime); the `#[tauri::command]`
//! wrappers below are thin.

use std::sync::Arc;

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
    /// `member` is not one of its members.
    pub async fn select_group_member(rt: &Runtime, exit: ExitId, member: NodeRef) -> CmdResult {
        rt.mutate_with(|m| {
            let bad = |what: &str| Err(vec![ValidationError::DanglingRef { what: what.to_string() }]);
            let Some(e) = m.exits.iter_mut().find(|e| e.id == exit) else {
                return bad("出口不存在");
            };
            match &mut e.kind {
                ExitKind::Group { members, selected, .. } if members.contains(&member) => {
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
    use crate::core::process::{CoreHandleLike, CoreLauncher};
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
        let node = NodeRef { subscription_id: sub_id.clone(), name: "n1".into() };
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
                members: vec![node.clone()],
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
        let node = NodeRef { subscription_id: SubscriptionId::new(), name: "n".into() };
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
}

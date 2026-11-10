//! AI 应用页的命令：列表视图、冲突预览、按应用测延迟、启动、打开下载页，以及规则与列表同步。

use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use super::catalog::{Catalog, CatalogApp};
use super::launch::LaunchOutcome;
use super::rules::{self, AiAppConflict};
use super::{detect, AiApps};
use crate::desktop::Desktop;
use crate::model::{Model, NodeRef, ProcessRuleId};
use crate::runtime::Runtime;
use crate::stats::delay::DelayResult;

type Apps<'a> = State<'a, Arc<AiApps>>;
type Dt<'a> = State<'a, Arc<Desktop>>;
type Rt<'a> = State<'a, Arc<Runtime>>;

#[derive(Serialize, Clone, Debug, PartialEq, specta::Type)]
pub struct AiAppView {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub download_url: String,
    pub installed: bool,
    pub running: bool,
    pub rule_id: Option<ProcessRuleId>,
    pub enabled: bool,
    pub node: Option<NodeRef>,
}

/// 每个列表应用一项，按列表顺序；`enabled` 只看 AI 规则自身的开关（总开关由"启动"打开）。
pub(crate) fn build_views(
    catalog: &Catalog,
    model: &Model,
    installed: impl Fn(&CatalogApp) -> bool,
    running: impl Fn(&CatalogApp) -> bool,
) -> Vec<AiAppView> {
    catalog
        .apps
        .iter()
        .map(|a| {
            let rule = rules::ai_rule(&model.process_routing, &a.id);
            AiAppView {
                id: a.id.clone(),
                name: a.name.clone(),
                icon: a.icon.clone(),
                download_url: a.download_url.clone(),
                installed: installed(a),
                running: running(a),
                rule_id: rule.map(|r| r.id.clone()),
                enabled: rule.is_some_and(|r| r.user_enabled),
                node: rule.and_then(|r| r.node.clone()),
            }
        })
        .collect()
}

pub(crate) fn find_app(apps: &AiApps, app_id: &str) -> Result<CatalogApp, String> {
    apps.catalog()
        .apps
        .into_iter()
        .find(|a| a.id == app_id)
        .ok_or_else(|| "AI 应用不存在".to_string())
}

/// 让 AI 规则跟上列表。先在副本上试跑，有改动才写入模型。
pub async fn sync_rules(rt: &Runtime, catalog: &Catalog) {
    let mut pr = rt.model().process_routing;
    if !rules::sync_with_catalog(&mut pr, catalog) {
        return;
    }
    if let Err(e) = rt
        .mutate_process_routing(|pr| {
            rules::sync_with_catalog(pr, catalog);
        })
        .await
    {
        eprintln!("aiapps: syncing rules with catalog failed: {}", crate::desktop::validation(e));
    }
}

/// 状态参数取自 `app`，命令不借用参数，返回值不必包一层 Result。
#[tauri::command]
#[specta::specta]
pub async fn ai_apps_list(app: AppHandle) -> Vec<AiAppView> {
    let catalog = app.state::<Arc<AiApps>>().catalog();
    let model = app.state::<Arc<Runtime>>().model();
    // 商店包查询与进程枚举都会阻塞。
    tauri::async_runtime::spawn_blocking(move || {
        let procs = crate::procroute::processes::running_processes();
        build_views(&catalog, &model, |a| detect::detect(a).is_some(), |a| detect::running(a, &procs))
    })
    .await
    .unwrap_or_else(|e| {
        eprintln!("aiapps: listing apps failed: {e}");
        Vec::new()
    })
}

#[tauri::command]
#[specta::specta]
pub fn ai_app_conflicts(apps: Apps<'_>, rt: Rt<'_>, app_id: String) -> Result<Vec<AiAppConflict>, String> {
    let app = find_app(&apps, &app_id)?;
    Ok(rules::conflicts(&rt.model().process_routing, &app))
}

/// 用该应用列表里的 test_url 测 `proxy_name` 的延迟（不记入延迟表）。
#[tauri::command]
#[specta::specta]
pub async fn test_ai_app_delay(
    apps: Apps<'_>,
    rt: Rt<'_>,
    app_id: String,
    proxy_name: String,
) -> Result<DelayResult, String> {
    let app = find_app(&apps, &app_id)?;
    Ok(rt.test_delay_at(proxy_name, app.test_url).await)
}

#[tauri::command]
#[specta::specta]
pub async fn launch_ai_app(apps: Apps<'_>, dt: Dt<'_>, rt: Rt<'_>, app_id: String) -> Result<LaunchOutcome, String> {
    let app = find_app(&apps, &app_id)?;
    let probe = app.clone();
    let target = tauri::async_runtime::spawn_blocking(move || detect::detect(&probe))
        .await
        .map_err(|e| format!("检测安装失败：{e}"))?
        .ok_or("未安装")?;
    dt.prepare_ai_app(&rt, &app_id).await?;
    tauri::async_runtime::spawn_blocking(move || super::launch::focus_or_launch(&app, &target))
        .await
        .map_err(|e| format!("启动失败：{e}"))?
}

/// 打开列表里该应用的下载页；不接受前端传入的 URL。
#[tauri::command]
#[specta::specta]
pub fn open_ai_app_download(apps: Apps<'_>, app_id: String) -> Result<(), String> {
    let app = find_app(&apps, &app_id)?;
    std::process::Command::new("explorer.exe")
        .arg(&app.download_url)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("无法打开浏览器: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ManualNodeId, ProcessEntry, ProcessRule};
    use routa_procsvc_proto::FaultPolicy;

    fn app(id: &str) -> CatalogApp {
        CatalogApp {
            id: id.into(),
            name: id.to_uppercase(),
            icon: "data:image/png;base64,".into(),
            download_url: format!("https://{id}.example/download"),
            test_url: format!("https://{id}.example"),
            on_fault: FaultPolicy::Block,
            install: vec![],
            processes: vec![format!("{id}.exe")],
        }
    }

    fn ai_rule(id: &str, app_id: &str, enabled: bool) -> ProcessRule {
        ProcessRule {
            id: ProcessRuleId(id.into()),
            ai_app: Some(app_id.into()),
            processes: vec![ProcessEntry::name(format!("{app_id}.exe"))],
            node: Some(NodeRef::Manual { manual_id: ManualNodeId("n".into()) }),
            user_enabled: enabled,
            ..Default::default()
        }
    }

    #[test]
    fn view_marks_enabled_only_when_rule_enabled() {
        let catalog = Catalog { schema_version: 1, apps: vec![app("on"), app("off"), app("none")] };
        let mut model = Model::default();
        model.process_routing.rules = vec![ai_rule("r-on", "on", true), ai_rule("r-off", "off", false)];
        let views = build_views(&catalog, &model, |a| a.id != "none", |a| a.id == "on");
        assert_eq!(views.len(), 3);
        let (on, off, none) = (&views[0], &views[1], &views[2]);
        assert_eq!(on.id, "on");
        assert_eq!(on.name, "ON");
        assert_eq!(on.download_url, "https://on.example/download");
        assert!(on.enabled && on.installed && on.running);
        assert_eq!(on.rule_id, Some(ProcessRuleId("r-on".into())));
        assert!(on.node.is_some());
        assert!(!off.enabled && off.installed && !off.running);
        assert_eq!(off.rule_id, Some(ProcessRuleId("r-off".into())));
        assert!(off.node.is_some());
        assert!(!none.enabled && !none.installed);
        assert_eq!(none.rule_id, None);
        assert_eq!(none.node, None);
    }

    #[test]
    fn find_app_rejects_unknown_ids() {
        let apps = crate::aiapps::AiApps::new(std::env::temp_dir().join("routa-aiapps-cmd-missing.json"));
        assert_eq!(find_app(&apps, "chatgpt").unwrap().id, "chatgpt");
        assert!(find_app(&apps, "nope").is_err());
    }
}

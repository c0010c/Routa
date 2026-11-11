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
    /// 本机已安装时为该应用自己的图标，否则为列表里的占位图。
    pub icon: String,
    pub download_url: String,
    pub installed: bool,
    pub running: bool,
    pub rule_id: Option<ProcessRuleId>,
    pub enabled: bool,
    pub node: Option<NodeRef>,
    /// 开关开着时，列表里这个应用的进程中仍由手动规则管着的那些（列表更新新增、同步时跳过的）。
    pub held: Vec<AiAppConflict>,
}

/// 每个列表应用一项，按列表顺序；`enabled` 只看 AI 规则自身的开关（总开关由"启动"打开）。
pub(crate) fn build_views(
    catalog: &Catalog,
    model: &Model,
    installed: impl Fn(&CatalogApp) -> bool,
    running: impl Fn(&CatalogApp) -> bool,
    local_icon: impl Fn(&CatalogApp) -> Option<String>,
) -> Vec<AiAppView> {
    catalog
        .apps
        .iter()
        .map(|a| {
            let rule = rules::ai_rule(&model.process_routing, &a.id);
            let enabled = rule.is_some_and(|r| r.user_enabled);
            AiAppView {
                id: a.id.clone(),
                name: a.name.clone(),
                icon: local_icon(a).unwrap_or_else(|| a.icon.clone()),
                download_url: a.download_url.clone(),
                installed: installed(a),
                running: running(a),
                rule_id: rule.map(|r| r.id.clone()),
                enabled,
                node: rule.and_then(|r| r.node.clone()),
                held: if enabled { rules::conflicts(&model.process_routing, a) } else { vec![] },
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
    // 商店包查询、进程枚举与取图标都会阻塞。
    tauri::async_runtime::spawn_blocking(move || {
        let procs = crate::procroute::processes::running_processes();
        let targets: std::collections::HashMap<&str, detect::LaunchTarget> =
            catalog.apps.iter().filter_map(|a| Some((a.id.as_str(), detect::detect(a)?))).collect();
        build_views(
            &catalog,
            &model,
            |a| targets.contains_key(a.id.as_str()),
            |a| detect::running(a, &procs),
            |a| targets.get(a.id.as_str()).and_then(super::icon::local_icon),
        )
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

/// "运行中"且代理开关关着时只置前（不启动内核 / 组件，也不要求选好节点）；其余情况先 prepare。
pub(crate) fn needs_prepare(running: bool, ai_enabled: bool) -> bool {
    !running || ai_enabled
}

#[tauri::command]
#[specta::specta]
pub async fn launch_ai_app(apps: Apps<'_>, dt: Dt<'_>, rt: Rt<'_>, app_id: String) -> Result<LaunchOutcome, String> {
    let app = find_app(&apps, &app_id)?;
    let probe = app.clone();
    let (target, running) = tauri::async_runtime::spawn_blocking(move || {
        let procs = crate::procroute::processes::running_processes();
        (detect::detect(&probe), detect::running(&probe, &procs))
    })
    .await
    .map_err(|e| format!("检测安装失败：{e}"))?;
    let target = target.ok_or("未安装")?;
    let ai_enabled = rules::ai_rule(&rt.model().process_routing, &app_id).is_some_and(|r| r.user_enabled);
    if needs_prepare(running, ai_enabled) {
        dt.prepare_ai_app(&rt, &app_id).await?;
    }
    tauri::async_runtime::spawn_blocking(move || super::launch::focus_or_launch(&app, &target))
        .await
        .map_err(|e| format!("启动失败：{e}"))?
}

/// 打开列表里该应用的下载页；不接受前端传入的 URL。
#[tauri::command]
#[specta::specta]
pub fn open_ai_app_download(apps: Apps<'_>, app_id: String) -> Result<(), String> {
    let app = find_app(&apps, &app_id)?;
    // 列表校验已拒绝会被 explorer 误读的字符；这里再交出规范化后的 URL。
    let url = url::Url::parse(&app.download_url).map_err(|e| format!("下载地址无效: {e}"))?;
    std::process::Command::new("explorer.exe")
        .arg(url.as_str())
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
        let views = build_views(&catalog, &model, |a| a.id != "none", |a| a.id == "on", |a| (a.id == "on").then(|| "data:image/png;base64,LOCAL".to_string()));
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
        // 本机图标优先，取不到用列表占位图。
        assert_eq!(on.icon, "data:image/png;base64,LOCAL");
        assert_eq!(off.icon, "data:image/png;base64,");
        assert_eq!(none.node, None);
    }

    #[test]
    fn view_lists_processes_held_by_manual_rules_only_when_enabled() {
        let mut on = app("on");
        on.processes.push("extra.exe".into());
        let mut off = app("off");
        off.processes.push("extra2.exe".into());
        let catalog = Catalog { schema_version: 1, apps: vec![on, off] };
        let mut model = Model::default();
        let manual = |id: &str, name: &str, procs: &[&str]| ProcessRule {
            id: ProcessRuleId(id.into()),
            name: name.into(),
            processes: procs.iter().map(|p| ProcessEntry::name(*p)).collect(),
            ..Default::default()
        };
        model.process_routing.rules = vec![
            ai_rule("r-on", "on", true),
            ai_rule("r-off", "off", false),
            manual("m1", "工作软件", &["EXTRA.exe", "notepad.exe"]),
            manual("m2", "", &["extra2.exe"]),
        ];
        let views = build_views(&catalog, &model, |_| true, |_| false, |_| None);
        assert_eq!(
            views[0].held,
            vec![AiAppConflict {
                rule_id: ProcessRuleId("m1".into()),
                rule_name: "工作软件".into(),
                process: "EXTRA.exe".into(),
                removes_rule: false,
            }]
        );
        // 开关关着：开启时会走冲突确认，不算"被占用"
        assert!(views[1].held.is_empty());
    }

    #[test]
    fn running_app_with_switch_off_is_only_focused() {
        assert!(!needs_prepare(true, false));
        assert!(needs_prepare(true, true));
        assert!(needs_prepare(false, false));
        assert!(needs_prepare(false, true));
    }

    #[test]
    fn find_app_rejects_unknown_ids() {
        let apps = crate::aiapps::AiApps::new(std::env::temp_dir().join("routa-aiapps-cmd-missing.json"));
        assert_eq!(find_app(&apps, "chatgpt").unwrap().id, "chatgpt");
        assert!(find_app(&apps, "nope").is_err());
    }
}

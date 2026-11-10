//! 本机检测：商店包 / exe 文件是否存在，以及应用当前是否在运行。

use std::path::{Path, PathBuf};

use super::catalog::{CatalogApp, InstallSource, EXE_ROOTS};
use crate::procroute::processes::RunningProcess;

#[derive(Clone, Debug, PartialEq)]
pub enum LaunchTarget {
    /// "<family>!<app_id>"
    Aumid(String),
    Exe(PathBuf),
}

/// 路径根占位符 → 环境变量名。
fn env_name(root: &str) -> &'static str {
    match root {
        "%LOCALAPPDATA%" => "LOCALAPPDATA",
        "%PROGRAMFILES%" => "ProgramFiles",
        "%PROGRAMFILES(X86)%" => "ProgramFiles(x86)",
        _ => "APPDATA",
    }
}

fn expand(path: &str, env: &impl Fn(&str) -> Option<String>) -> Option<PathBuf> {
    let root = EXE_ROOTS.iter().find(|r| {
        path.len() > r.len() && path.is_char_boundary(r.len()) && path[..r.len()].eq_ignore_ascii_case(r)
    })?;
    let base = env(env_name(root))?;
    Some(PathBuf::from(format!("{}{}", base.trim_end_matches('\\'), &path[root.len()..])))
}

/// 按 install 顺序取第一个命中的来源。
pub fn resolve(
    sources: &[InstallSource],
    has_package: impl Fn(&str) -> bool,
    exists: impl Fn(&Path) -> bool,
    env: impl Fn(&str) -> Option<String>,
) -> Option<LaunchTarget> {
    sources.iter().find_map(|s| match s {
        InstallSource::Store { family, app_id } => {
            has_package(family).then(|| LaunchTarget::Aumid(format!("{family}!{app_id}")))
        }
        InstallSource::Exe { path } => expand(path, &env).filter(|p| exists(p)).map(LaunchTarget::Exe),
    })
}

#[cfg(windows)]
fn has_store_package(family: &str) -> bool {
    use windows::core::HSTRING;
    use windows::Management::Deployment::PackageManager;
    let Ok(pm) = PackageManager::new() else { return false };
    pm.FindPackagesByUserSecurityIdPackageFamilyName(&HSTRING::new(), &HSTRING::from(family))
        .ok()
        .and_then(|it| it.First().ok())
        .is_some_and(|it| it.HasCurrent().unwrap_or(false))
}

#[cfg(not(windows))]
fn has_store_package(_family: &str) -> bool {
    false
}

pub fn detect(app: &CatalogApp) -> Option<LaunchTarget> {
    resolve(&app.install, has_store_package, |p| p.is_file(), |k| std::env::var(k).ok())
}

pub fn running(app: &CatalogApp, procs: &[RunningProcess]) -> bool {
    procs.iter().any(|p| app.processes.iter().any(|n| n.eq_ignore_ascii_case(&p.name)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aiapps::bundled;

    fn app(id: &str) -> CatalogApp {
        bundled().apps.into_iter().find(|a| a.id == id).unwrap()
    }

    fn env(k: &str) -> Option<String> {
        (k == "LOCALAPPDATA").then(|| "C:\\U\\AppData\\Local".to_string())
    }

    #[test]
    fn resolve_prefers_first_matching_source() {
        let a = app("claude");
        let t = resolve(&a.install, |f| f == "Claude_pzs8sxrjxfjjc", |_| true, env);
        assert_eq!(t, Some(LaunchTarget::Aumid("Claude_pzs8sxrjxfjjc!Claude".into())));
    }

    #[test]
    fn resolve_falls_back_to_exe() {
        let a = app("cursor");
        let t = resolve(&a.install, |_| false, |_| true, env);
        assert!(matches!(t, Some(LaunchTarget::Exe(_))));
    }

    #[test]
    fn resolve_expands_only_known_roots() {
        let src = [InstallSource::Exe { path: "%LOCALAPPDATA%\\Programs\\cursor\\Cursor.exe".into() }];
        assert_eq!(
            resolve(&src, |_| false, |_| true, env),
            Some(LaunchTarget::Exe(PathBuf::from("C:\\U\\AppData\\Local\\Programs\\cursor\\Cursor.exe")))
        );
        let unknown = [InstallSource::Exe { path: "%TEMP%\\x.exe".into() }];
        assert_eq!(resolve(&unknown, |_| false, |_| true, |_| Some("C:\\T".into())), None);
    }

    #[test]
    fn resolve_none_when_nothing_found() {
        let a = app("claude");
        assert_eq!(resolve(&a.install, |_| false, |_| false, env), None);
        assert_eq!(resolve(&[], |_| true, |_| true, env), None);
    }

    #[test]
    fn running_matches_any_process_case_insensitively() {
        let mut a = app("cursor");
        a.processes = vec!["Cursor.exe".into()];
        let p = |name: &str| RunningProcess { pid: 1, name: name.into(), exe_path: format!("C:\\{name}") };
        assert!(running(&a, &[p("notepad.exe"), p("CURSOR.EXE")]));
        assert!(!running(&a, &[p("notepad.exe")]));
    }

    #[cfg(windows)]
    #[test]
    #[ignore]
    fn detect_real_machine() {
        for id in ["chatgpt", "claude"] {
            println!("{id}: {:?}", detect(&app(id)));
        }
    }
}

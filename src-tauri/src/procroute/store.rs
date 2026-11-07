//! 商店应用（MSIX）更新后换了安装目录（`WindowsApps\<包全名>`）：监听包目录事件，
//! 把已知位置映射到新版本的目录。

use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use windows::core::HSTRING;
use windows::ApplicationModel::{
    PackageCatalog, PackageInstallingEventArgs, PackageUpdatingEventArgs,
};
use windows::Foundation::TypedEventHandler;

use super::learn::exe_key;
use crate::model::normalize::normalize_exe_path;

/// 包全名 `Name_Version_Arch_ResourceId_PublisherId` → （小写名称，小写发布者 id）。
/// 名称去掉 `.v<数字和->` 后缀（如 `OpenAI.CodexPrimaryRuntime.v26-1029`，每个版本名称都不同）。
pub fn family_key(full_name: &str) -> Option<(String, String)> {
    let parts: Vec<&str> = full_name.split('_').collect();
    let [name, version, arch, _resource, publisher] = parts[..] else { return None };
    if name.is_empty() || version.is_empty() || arch.is_empty() || publisher.is_empty() {
        return None;
    }
    let name = name.to_lowercase();
    let name = match name.rfind(".v") {
        Some(i) if i > 0 && {
            let tail = &name[i + 2..];
            tail.starts_with(|c: char| c.is_ascii_digit()) && tail.chars().all(|c| c.is_ascii_digit() || c == '-')
        } => name[..i].to_string(),
        _ => name,
    };
    Some((name, publisher.to_lowercase()))
}

/// `path` 位于 `\WindowsApps\<包全名>\<相对路径>` 时返回（包全名，相对路径）。
fn split_windows_apps(path: &str) -> Option<(&str, &str)> {
    const SEG: &str = r"\windowsapps\";
    // 只转 ASCII 小写：字节位置与原串一致。
    let i = path.to_ascii_lowercase().find(SEG)?;
    let (pkg, rel) = path[i + SEG.len()..].split_once('\\')?;
    (!pkg.is_empty() && !rel.is_empty()).then_some((pkg, rel))
}

/// 把 `learned` 中与新包同族的 `WindowsApps` 路径映射到新包的安装目录 `new_root`；
/// 排除已在 `learned` 的，去重。
pub fn siblings(learned: &[String], new_full_name: &str, new_root: &str) -> Vec<String> {
    let Some(family) = family_key(new_full_name) else { return vec![] };
    let root = normalize_exe_path(new_root);
    let root = root.trim_end_matches('\\');
    let mut seen: HashSet<String> = learned.iter().map(|p| exe_key(p)).collect();
    let mut out = Vec::new();
    for p in learned {
        let p = normalize_exe_path(p);
        let Some((pkg, rel)) = split_windows_apps(&p) else { continue };
        if family_key(pkg).as_ref() != Some(&family) {
            continue;
        }
        let mapped = format!(r"{root}\{rel}");
        if seen.insert(exe_key(&mapped)) {
            out.push(mapped);
        }
    }
    out
}

/// 持有订阅；析构时注销两个事件。
pub struct WatchGuard {
    catalog: PackageCatalog,
    installing: i64,
    updating: i64,
}

impl Drop for WatchGuard {
    fn drop(&mut self) {
        let _ = self.catalog.RemovePackageInstalling(self.installing);
        let _ = self.catalog.RemovePackageUpdating(self.updating);
    }
}

/// 订阅当前用户的包安装 / 更新事件；完成时以（包全名，安装目录）回调。
/// WinRT 未初始化时 `windows` crate 会自行加入 MTA，这里不另外初始化。
pub fn watch<F: Fn(String, String) + Send + 'static>(on_package: F) -> windows::core::Result<WatchGuard> {
    let catalog = PackageCatalog::OpenForCurrentUser()?;
    // 事件可能在任意线程回调：包一层锁让处理器满足 `Send`，回调不必是 `Sync`。
    let on_package = Arc::new(Mutex::new(on_package));
    fn call<F: Fn(String, String)>(cb: &Mutex<F>, name: String, root: String) {
        (cb.lock().unwrap_or_else(|e| e.into_inner()))(name, root)
    }
    let report = |pkg: windows::core::Result<windows::ApplicationModel::Package>| -> windows::core::Result<(String, String)> {
        let pkg = pkg?;
        let name: HSTRING = pkg.Id()?.FullName()?;
        let root: HSTRING = pkg.InstalledPath()?;
        Ok((name.to_string_lossy(), root.to_string_lossy()))
    };
    let cb = on_package.clone();
    let installing = catalog.PackageInstalling(&TypedEventHandler::<PackageCatalog, PackageInstallingEventArgs>::new(
        move |_, args| {
            if let Some(args) = args.as_ref() {
                if args.IsComplete()? {
                    let (name, root) = report(args.Package())?;
                    call(&cb, name, root);
                }
            }
            Ok(())
        },
    ))?;
    let cb = on_package;
    let updating = catalog.PackageUpdating(&TypedEventHandler::<PackageCatalog, PackageUpdatingEventArgs>::new(
        move |_, args| {
            if let Some(args) = args.as_ref() {
                if args.IsComplete()? {
                    let (name, root) = report(args.TargetPackage())?;
                    call(&cb, name, root);
                }
            }
            Ok(())
        },
    ));
    let updating = match updating {
        Ok(t) => t,
        Err(e) => {
            let _ = catalog.RemovePackageInstalling(installing);
            return Err(e);
        }
    };
    Ok(WatchGuard { catalog, installing, updating })
}

#[cfg(test)]
mod tests {
    use super::*;

    const OLD: &str = r"C:\Program Files\WindowsApps\OpenAI.Codex_26.1029.7803.0_x64__2p2nqsd0c76g0";
    const NEW_NAME: &str = "OpenAI.Codex_26.1105.8012.0_x64__2p2nqsd0c76g0";
    const NEW_ROOT: &str = r"C:\Program Files\WindowsApps\OpenAI.Codex_26.1105.8012.0_x64__2p2nqsd0c76g0";

    #[test]
    fn family_key_strips_version_suffix() {
        assert_eq!(
            family_key("OpenAI.Codex_26.1029.7803.0_x64__2p2nqsd0c76g0"),
            Some(("openai.codex".to_string(), "2p2nqsd0c76g0".to_string()))
        );
        let a = family_key("OpenAI.CodexPrimaryRuntime.v26-1029_26.1029.7803.0_x64__2p2nqsd0c76g0");
        let b = family_key("OpenAI.CodexPrimaryRuntime.v26-1105_26.1105.8012.0_x64__2p2nqsd0c76g0");
        assert_eq!(a, Some(("openai.codexprimaryruntime".to_string(), "2p2nqsd0c76g0".to_string())));
        assert_eq!(a, b);
        assert_eq!(family_key("not-a-package"), None);
    }

    #[test]
    fn siblings_map_relative_path_into_new_version() {
        let learned = vec![format!(r"{OLD}\app\ChatGPT.exe"), r"C:\Apps\ChatGPT.exe".to_string()];
        assert_eq!(siblings(&learned, NEW_NAME, NEW_ROOT), vec![format!(r"{NEW_ROOT}\app\ChatGPT.exe")]);
        // 已记下的不再产出；其他包不产出。
        let mut known = learned.clone();
        known.push(format!(r"{NEW_ROOT}\app\ChatGPT.exe"));
        assert!(siblings(&known, NEW_NAME, NEW_ROOT).is_empty());
        assert!(siblings(&learned, "Other.App_1.0.0.0_x64__2p2nqsd0c76g0", r"C:\X").is_empty());
        assert!(siblings(&[r"C:\Apps\ChatGPT.exe".to_string()], NEW_NAME, NEW_ROOT).is_empty());
    }

    #[test]
    #[ignore = "needs the real WinRT package catalog"]
    fn catalog_opens_in_unpackaged_process() {
        let guard = watch(|_, _| {});
        assert!(guard.is_ok(), "{:?}", guard.err());
    }
}

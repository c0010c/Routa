pub mod cache;
pub mod download;
pub mod service;

use std::sync::Arc;

use tauri::{AppHandle, State};

pub use service::{UpdateState, UpdateView, Updater};

/// Only release builds installed by NSIS update themselves (the installer
/// puts `uninstall.exe` next to `routa.exe`).
pub fn is_nsis_install() -> bool {
    !cfg!(debug_assertions)
        && std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|d| d.join("uninstall.exe")))
            .is_some_and(|p| p.is_file())
}

/// The minisign public key from `plugins.updater.pubkey` in tauri.conf.json,
/// so the cache checks against the same key as the plugin.
pub fn updater_pubkey(app: &AppHandle) -> Option<String> {
    app.config()
        .plugins
        .0
        .get("updater")?
        .get("pubkey")?
        .as_str()
        .map(str::to_owned)
}

#[tauri::command]
#[specta::specta]
pub fn update_status(state: State<'_, Arc<Updater>>) -> UpdateView {
    state.view()
}

#[tauri::command]
#[specta::specta]
pub async fn update_check(state: State<'_, Arc<Updater>>) -> Result<UpdateView, ()> {
    Ok(state.check_now().await)
}

#[cfg(test)]
mod tests {
    #[test]
    fn plugin_config_parses_with_expected_endpoints() {
        let conf: serde_json::Value =
            serde_json::from_str(include_str!("../../tauri.conf.json")).unwrap();
        let raw = conf["plugins"]["updater"].clone();
        let config: tauri_plugin_updater::Config = serde_json::from_value(raw).unwrap();
        let base = "https://github.com/c0010c/Routa/releases/download/updater";
        let got: Vec<String> = config.endpoints.iter().map(|u| u.to_string()).collect();
        assert_eq!(got, vec![
            format!("https://update.hwdns.net/{base}/latest-proxy.json"),
            format!("https://gh-proxy.org/{base}/latest-proxy.json"),
            format!("{base}/latest.json"),
        ]);
        assert!(!config.pubkey.is_empty());
    }
}

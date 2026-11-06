#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if std::env::args_os().nth(1).as_deref() == Some(std::ffi::OsStr::new("--build-info")) {
        println!("{}", serde_json::json!({
            "format": "ROUTA_BUILD_INFO_V1",
            "protocolVersion": routa_procsvc_proto::PROTO_VERSION,
            "serviceSha256": option_env!("ROUTA_SERVICE_SHA256"),
            "frontendSha256": option_env!("ROUTA_FRONTEND_SHA256"),
            "customProtocol": !tauri::is_dev(),
            "sourceSha256": option_env!("ROUTA_GUI_SOURCE_SHA256"),
        }));
        return;
    }
    routa_lib::run()
}

//! 按进程分流：根据 Routa 的模型和运行状态，算出下发给助手服务的规则动作，并与服务保持同步。

pub mod client;
pub mod decide;
pub mod elevate;
pub mod processes;

pub use decide::decide;

/// 独立任务持有维护锁直到提权助手退出，即使调用方不再等待，也不能提前恢复自动拉起。
pub async fn service_command(cmd: elevate::ElevatedCmd) -> Result<(), String> {
    tauri::async_runtime::spawn(async move {
        let mut operation = client::service_operation()?;
        operation.pause().await?;
        elevate::run_elevated(cmd).await
    }).await.map_err(|e| format!("分流服务操作没有完成：{e}"))?
}

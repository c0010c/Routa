//! 按进程分流：根据 Routa 的模型和运行状态，算出下发给助手服务的规则动作，并与服务保持同步。

pub mod client;
pub mod decide;
pub mod elevate;
pub mod processes;

pub use decide::decide;

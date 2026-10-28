//! 按进程分流：根据 Routa 的模型和运行状态，算出下发给助手服务的规则动作。

pub mod decide;

pub use decide::decide;

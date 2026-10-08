//! TOML 配置管理框架（详见 README）。
//!
//! 根目录解析的三种方式：
//! 1. `Config::ROOT` 常量覆写（推荐，任何特征下可用）
//! 2. `*_at` 方法族传任意根目录（任何特征下可用）
//! 3. 进程级全局根目录（需要 `global-root` 特征，历史设计）

pub mod error;
pub mod config_path;
mod config;

pub use config::Config;

#[cfg(feature = "global-root")]
pub use config_path::init_config_path;
#[cfg(feature = "global-root")]
pub use config_path::config_path;
pub use config_path::config_doc_path;

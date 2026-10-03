pub mod error;
mod config_path;
mod config;
pub use config::Config;
/// 必须调用！！！
pub use config_path::init_config_path;
pub use config_path::config_path;



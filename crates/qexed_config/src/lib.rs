pub mod error;
mod config_path;
mod config;
pub const PROTOCOL_VERSION: i32 = 777;
pub const MC_VERSION: &'static str = "26.3";

pub use config::Config;
/// 必须调用！！！
pub use config_path::init_config_path;
pub use config_path::config_path;
pub use config_path::config_doc_path;



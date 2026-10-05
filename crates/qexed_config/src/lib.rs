pub mod error;
mod config_path;
mod config;
pub const PROTOCOL_VERSION: i32 = 775;
pub const MC_VERSION: &'static str = "26.1.2";

pub use config::Config;
/// 必须调用！！！
pub use config_path::init_config_path;
pub use config_path::config_path;
pub use config_path::config_doc_path;



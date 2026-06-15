extern crate self as qexed_config;
pub const PROTOCOL_VERSION: i32 = 775;
pub const MC_VERSION: &'static str = "26.1.2";
pub mod app;
pub mod public;
pub mod tool;
pub static CONFIG_PATH: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();
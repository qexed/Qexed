extern crate self as qexed_config;

rust_i18n::i18n!("./locales");
pub const PROTOCOL_VERSION: i32 = 775; // 数据包协议版本
pub const MC_VERSION: &'static str = "26.1.2"; // Minecraft游戏版本
include!(concat!(env!("OUT_DIR"), "/build_info.rs"));
pub mod app;
pub mod public;
pub mod tool;

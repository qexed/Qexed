//! qexed_server：连接层与最小可玩服务端状态机。
//!
//! 职责：异步包分帧（长度前缀 + zlib 压缩）、LOGIN 握手、
//! CONFIGURATION 协商、PLAY 最小流程（join + keepalive + 空世界区块）。
//! qexed 主程序保持轻量，只做装配。

pub mod transport;
pub mod auth;
pub mod login;
pub mod configuration;
pub mod play;
pub mod registry_sync;
pub mod error;
pub mod server;

pub use error::ServerError;
pub use server::{ServerConfig, ServerHandle, serve};
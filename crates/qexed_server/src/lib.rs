//! Qexed 服务端连接处理。
//!
//! 模块划分：
//! - [`handle`]        — `ServerHandle` / `ServerContext`
//! - [`server`]        — `init()` 启动 & 单连接分发
//! - [`status`]        — Status 阶段
//! - [`login`]         — Login 阶段（含正版认证）
//! - [`configuration`] — Configuration 阶段（含 KeepAlive 心跳）
//! - [`codec`]         — 收发包辅助
//! - [`consts`]        — 各阶段超时/参数常量

mod codec;
mod configuration;
mod consts;
mod handle;
mod login;
mod server;
mod status;

pub mod config;
pub mod error;

pub use handle::{ServerContext, ServerHandle};
pub use server::init;

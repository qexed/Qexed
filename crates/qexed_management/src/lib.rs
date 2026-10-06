//! Minecraft Server Management Protocol (MCSMP) 服务器端实现。
//!
//! JSON-RPC 2.0 over WebSocket，对齐官方 26.x 规范：
//! - 认证：Bearer 或 Sec-WebSocket-Protocol("minecraft-v1,<secret>")
//! - 方法命名空间：minecraft:{allowlist,bans,ip_bans,players,operators,
//!   server,serversettings,gamerules}/<path>
//! - 通知命名空间：minecraft:notification/<domain>/<event>
//!
//! 架构：ManagementServer 持有 TcpListener + 会话集；每个 WebSocket 连接
//! 是一个 Session（认证 → JSON-RPC 分发 → 通知订阅广播）。
//! 服务器状态经 ManagementBackend trait 注入（qexed 组装层实现真实逻辑）。

pub mod config;
pub mod error;
pub mod model;
pub mod protocol;
pub mod server;
pub mod backend;

pub use backend::ManagementBackend;
pub use error::{ManagementError, Result};
pub use model::{
    AllowlistEntry, Difficulty, GameMode, IpBan, KickRequest, Message, OperatorEntry, PlayerRef,
    ServerState, SystemMessage, UserBan, VersionInfo,
};
pub use server::{ManagementServer, Notification};

#[cfg(test)]
mod tests;

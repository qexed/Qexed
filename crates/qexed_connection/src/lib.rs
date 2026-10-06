//! qexed_connection：连接域（v4 connection/ + auth + secure_chat +
//! proxy_forwarding + qexed_tcp_connect crate 的全量迁移）。
//!
//! 模块结构：
//! - transport —— TCP 帧读写/压缩/加密/绑定（原 qexed_tcp_connect crate）
//! - codec —— 包负载编解码辅助
//! - auth —— Mojang 在线认证（RSA 加密握手 + 会话验证 + 离线 UUID）
//! - secure_chat —— 安全聊天签名校验
//! - proxy_forwarding —— BungeeCord / Velocity 转发解析
//! - code_of_conduct —— 行为守则多语言文本
//! - status —— 服务器列表 ping 响应
//! - connection —— 握手/登录/配置阶段处理

pub mod auth;
pub mod code_of_conduct;
pub mod config;
pub mod connection;
pub mod error;
pub mod proxy_forwarding;
pub mod secure_chat;
pub mod status;
pub mod transport;

//! qexed_server：服务器域（server/bootstrap/console/commands/status/services/
//! cluster/warden/content_filter/code_of_conduct/audit/placeholders/l10n）。
//!
//! v4 → v6 迁移要点：
//! - 错误统一 crate::error::ServerError（禁止 anyhow）。
//! - 配置用 app_config 宏在本 crate 定义（v4 的 qexed_config::app::qexed::* 不存在）。
//! - 用户可见文案走 qexed_language::t，键同步写入 qexed_language locales。
//! - 依赖 qexed_player/qexed_world/qexed_entities/qexed_plugins 的能力以 trait 注入
//!   （这些 crate 当前是空壳，trait 见 context.rs / cluster_entities.rs）。

pub mod audit;
pub mod bootstrap;
pub mod cluster_entities;
pub mod cluster_rpc;
pub mod cluster_shard;
pub mod code_of_conduct;
pub mod commands;
pub mod config;
pub mod console;
pub mod content_filter;
pub mod context;
pub mod error;
pub mod l10n;
pub mod placeholders;
pub mod server;
pub mod services;
pub mod status;
pub mod warden;
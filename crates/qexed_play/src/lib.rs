//! qexed_play：play 会话域。
//!
//! 两个迁移任务的协作基座（play-core + play-gameplay）：
//! - play-core（已适配 v6）：bootstrap / session / chunks / drops / events / lobby /
//!   menus / util / session_core（play.rs 会话核心）+ context（边界 trait）+ config + error
//! - play-gameplay（进行中）：chat / gameplay/ / geyser / mining / recipes / scoreboard /
//!   survival / tests / pathfinding / structures / inventory / runtime（其 460K runtime.rs
//!   为 v4 play.rs 拷贝，待按本 crate 基座改造；改造完成后在下方取消注释）
//!
//! v4 → v6 迁移要点：
//! - 错误统一 crate::error::PlayError（禁止 anyhow）
//! - 配置（Lobby/Menus/World/PlayerMessages）用 app_config 宏在本 crate 定义
//! - 26.3 协议差异：Position -> PlayerPosition、GameStateChange -> GameEvent、
//!   MapChunk -> LevelChunkWithLight、move_player_pos -> pos 等（详见各模块注释）
//! - world/集群能力以 trait 注入（context.rs）；gameplay 子系统经
//!   session_core::GameplayHooks 回调
//! - i18n：用户可见文案走 qexed_language::t，键在 qexed_language locales

pub mod config;
pub mod context;
pub mod error;

mod bootstrap;
mod chunks;
mod drops;
mod events;
mod lobby;
mod menus;
mod session;
mod session_core;
mod util;

// play-gameplay 任务落地的模块（已适配 v6：本 crate config/error + qexed_player/
// qexed_entities/qexed_plugins/qexed_world(规则)/qexed_mojang_data + world_access trait）：
mod gameplay;
pub(crate) mod l10n;
mod mining;
mod pathfinding;
pub(crate) mod plugin_bridge;
mod recipes;
mod structures;
mod survival;
mod world_access;
pub mod inventory;

// play-gameplay 的命令支持面（权限 trait + 命令名/帮助文案；chat.rs 与装配层用）：
pub(crate) mod chat_support;

// TODO(play-gameplay)：chat / geyser / scoreboard / tests 仍引用 play-core 未稳定的
// LobbyRuntime/MenuRuntime/GeyserRuntime/ChunkSendState 接线（play-core 任务进行中）。
// chat.rs 的命令逻辑（teleport/gamemode/give/time/gamerule/entity/npc/structure/
// 插件 action）已完成 v6 适配（协议路径/错误类型/world_access/chat_support），
// 待 play-core 基座定稿后在 handle_chat_command/apply_plugin_action 的会话参数上
// 接线并取消注释：
// mod chat;
// mod geyser;
// mod scoreboard;
// mod tests;
// mod runtime;

pub use bootstrap::{ServerDisplay, SessionInventory, SurvivalSnapshot, entities_position};
pub use chunks::{
    ChunkLoadResult, ChunkSendState, FluidSeed, SharedWorld, chunk_load_parallelism_limit,
};
pub use config::{
    ChestMenu, ForwardingMode, GameMode, Lobby, LobbyAction, LobbyActionKind, LobbyMenuItem,
    MenuAction, MenuActionKind, MenuHotbarItem, MenuItem, Menus, PlayConfig, PlayerMessages,
    ServerProxyConfig, Spawn, WorldConfig,
};
pub use context::{
    AllowAllPermissions, ClusterEntityView, DimensionRules, NoPlaceholders, NoPlayerAudit,
    NoPluginEvents, PermissionLookup, PlaceholderContext, PlaceholderRenderer, PlayerAuditLog,
    PluginEventSink, WorldChunkSource, WorldRulesSource, WorldSessionGuard,
};
pub use drops::ItemRegistry;
pub use error::{PlayError, Result};
pub use lobby::{LobbyRuntime, LobbyServerStatus, LobbyStatusSnapshot, NoProxyTransfer, ProxyTransfer};
pub use menus::{HotbarSync, MenuRuntime};
pub use session_core::{
    ChatFilter, ChatRateLimit, FilterAction, GameplayHooks, NoChatFilter, NoGameplay,
    NoSecureChat, PlaySessionDeps, SecureChatHook, SessionTickContext, FluidRuntime, initialize,
};
pub use session::player_payload;

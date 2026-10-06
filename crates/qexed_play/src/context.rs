//! play 域与外部域（world / entities / players / plugins / permissions / 审计 / 集群）的交互边界。
//!
//! v4 的 play.rs 直接依赖 `crate::world::WorldManager`、`crate::players::PlayerManager`、
//! `crate::entities::EntityManager` 等具体类型；v6 按域拆分 crate 且 qexed_world
//! 的 manager 仍在迁移中，因此把 play 会话循环需要的最小面收敛为 trait：
//! - [`WorldChunkSource`]：区块负载构建/缓存/放置方块回放（v4 WorldManager 的 chunk 子集）
//! - [`WorldRulesSource`]：维度规则与时间（v4 WorldRulesManager）
//! - [`PermissionLookup`]：命令权限查询（v4 PermissionManager 最小面）
//! - [`PlayerAuditLog`]：玩家审计日志（v4 PlayerAuditLogger 最小面）
//!
//! qexed 本体组装时提供实现；单测用本文件的空实现。

use std::collections::HashMap;

use bytes::Bytes;
use qexed_packet::net_types::Position as BlockPosition;
use qexed_protocol::to_client::play::level_chunk_with_light::LevelChunkWithLight;

use crate::error::Result;

/// 插件事件面（v4 PluginManager::emit_chunk_load/emit_chunk_unload 等）。
///
/// qexed 本体组装时用 PluginManager 实现它；单测用 [`NoPluginEvents`]。
pub trait PluginEventSink: Send + Sync {
    fn emit_chunk_load(&self, dimension: &str, chunk_x: i32, chunk_z: i32);
    fn emit_chunk_unload(&self, dimension: &str, chunk_x: i32, chunk_z: i32);
}

/// 空插件事件汇：丢弃。
#[derive(Debug, Default)]
pub struct NoPluginEvents;

impl PluginEventSink for NoPluginEvents {
    fn emit_chunk_load(&self, _dimension: &str, _chunk_x: i32, _chunk_z: i32) {}
    fn emit_chunk_unload(&self, _dimension: &str, _chunk_x: i32, _chunk_z: i32) {}
}

/// qexed_plugins::PluginManager 的插件事件汇适配。
impl PluginEventSink for qexed_plugins::PluginManager {
    fn emit_chunk_load(&self, dimension: &str, chunk_x: i32, chunk_z: i32) {
        qexed_plugins::PluginManager::emit_chunk_load(self, dimension, chunk_x, chunk_z);
    }

    fn emit_chunk_unload(&self, dimension: &str, chunk_x: i32, chunk_z: i32) {
        qexed_plugins::PluginManager::emit_chunk_unload(self, dimension, chunk_x, chunk_z);
    }
}

/// 方块更新（v4 world::BlockUpdate 的 play 视图）。
#[derive(Debug, Clone)]
pub struct PlacedBlockUpdate {
    pub location: BlockPosition,
    pub block_state: i32,
}

/// 世界会话句柄：drop 时结束会话（v4 world::WorldSession）。
pub struct WorldSessionGuard<'a> {
    source: &'a dyn WorldChunkSource,
    active: bool,
}

impl<'a> WorldSessionGuard<'a> {
    pub fn new(source: &'a dyn WorldChunkSource) -> Self {
        source.begin_session();
        Self { source, active: true }
    }

    fn end(&mut self) {
        if self.active {
            self.source.end_session();
            self.active = false;
        }
    }
}

impl Drop for WorldSessionGuard<'_> {
    fn drop(&mut self) {
        self.end();
    }
}

/// 区块负载源（v4 WorldManager 的 chunk 会话子集）。
pub trait WorldChunkSource: Send + Sync {
    /// 缓存纪元：世界写操作后递增，旧纪元的区块任务作废。
    fn cache_epoch(&self) -> u64;

    /// 会话开始/结束（引用计数，v4 begin_session/end_session）。
    fn begin_session(&self);
    fn end_session(&self);

    /// 预编译区块帧缓存命中（含压缩阈值键）。
    fn precompiled_chunk_packet(
        &self,
        dimension: &str,
        chunk_x: i32,
        chunk_z: i32,
        cache_epoch: u64,
        compression_threshold: Option<i32>,
    ) -> Result<Option<Bytes>>;

    /// 已保存区块的网络包形式；None 表示需要生成。
    fn saved_network_chunk_for_session(
        &self,
        dimension: &str,
        chunk_x: i32,
        chunk_z: i32,
        cache_epoch: u64,
    ) -> Result<Option<LevelChunkWithLight>>;

    /// 生成区块的网络包形式。
    fn generated_network_chunk_for_session(
        &self,
        dimension: &str,
        chunk_x: i32,
        chunk_z: i32,
        cache_epoch: u64,
    ) -> Result<LevelChunkWithLight>;

    /// 区块内流体种子（位置, 方块状态）。
    fn fluid_positions_in_chunk(
        &self,
        dimension: &str,
        chunk_x: i32,
        chunk_z: i32,
    ) -> Result<Vec<(BlockPosition, i32)>>;

    /// 已放置方块的补发包（进入区块时回放）。
    fn placed_block_updates(
        &self,
        dimension: &str,
        chunk_x: i32,
        chunk_z: i32,
    ) -> Vec<PlacedBlockUpdate>;

    /// 预编译区块包是否启用。
    fn precompiled_chunk_packets_enabled(&self) -> bool;

    /// 预编译帧是否含光照（决定缓存拆分方式）。
    fn precompiled_chunk_payload_includes_light(&self) -> bool;

    /// 记住完整预编译帧。
    fn remember_precompiled_chunk_frame(
        &self,
        dimension: &str,
        chunk_x: i32,
        chunk_z: i32,
        frame: Bytes,
        compression_threshold: Option<i32>,
    );

    /// 记住不含光照的预编译包前缀。
    fn remember_precompiled_chunk_packet_without_light(
        &self,
        dimension: &str,
        chunk_x: i32,
        chunk_z: i32,
        prefix: Bytes,
        compression_threshold: Option<i32>,
    );

    /// 按钮状态的查询/放置等 world 写路径（play 会话交互回调用）。
    fn block_state_at(&self, dimension: &str, position: &BlockPosition) -> Option<i32>;
}

/// 维度规则快照（v4 WorldRulesManager::snapshot 的 play 子集）。
#[derive(Debug, Clone)]
pub struct DimensionRules {
    /// 维度类型标识（minecraft:overworld 等）。
    pub dimension_type: String,
}

/// 世界规则源（v4 WorldRulesManager 最小面）。
pub trait WorldRulesSource: Send + Sync {
    /// 确保维度规则已加载。
    fn ensure_loaded(&self, dimension: &str) -> Result<()>;

    /// 维度规则快照。
    fn snapshot(&self, dimension: &str) -> DimensionRules;

    /// 维度当前游戏时间（tick）。
    fn current_time(&self, dimension: &str) -> i64;

    /// 推进维度时间（世界时间 tick 用）。
    fn tick_dimension_time(&self, dimension: &str, default_day_ticks: i64) -> i64;
}

/// 权限查询面（v4 PermissionManager 的 play 子集）。
pub trait PermissionLookup: Send + Sync {
    /// 玩家是否可执行命令。
    fn can_run_command(&self, profile_id: uuid::Uuid, command: &str) -> bool;

    /// 玩家命令树可见的根命令（含插件命令）。
    fn command_grants(&self, profile_id: uuid::Uuid) -> Vec<String>;
}

/// 空权限表：全放行（单测用）。
#[derive(Debug, Default)]
pub struct AllowAllPermissions;

impl PermissionLookup for AllowAllPermissions {
    fn can_run_command(&self, _profile_id: uuid::Uuid, _command: &str) -> bool {
        true
    }

    fn command_grants(&self, _profile_id: uuid::Uuid) -> Vec<String> {
        Vec::new()
    }
}

/// 玩家审计日志面（v4 PlayerAuditLogger 的 play 子集）。
pub trait PlayerAuditLog: Send + Sync {
    /// 记录一条玩家审计事件。
    fn record(&self, profile_id: uuid::Uuid, event: &str, detail: &str);
}

/// 空审计：丢弃（单测用）。
#[derive(Debug, Default)]
pub struct NoPlayerAudit;

impl PlayerAuditLog for NoPlayerAudit {
    fn record(&self, _profile_id: uuid::Uuid, _event: &str, _detail: &str) {}
}

/// 占位符上下文（v4 placeholders::PlaceholderContext 的菜单渲染子集）。
#[derive(Debug, Clone, Default)]
pub struct PlaceholderContext {
    pub online_players: i32,
    pub max_players: i32,
    pub lobby_online_servers: usize,
    pub lobby_total_servers: usize,
    pub lobby_servers: String,
}

/// 占位符渲染面（v4 placeholders::format_placeholders；由 server/plugins 域注入）。
pub trait PlaceholderRenderer: Send + Sync {
    /// 渲染文本占位符（%player_name% 等）。
    fn render(
        &self,
        enabled: bool,
        player: Option<&qexed_player::OnlinePlayer>,
        text: &str,
        context: &PlaceholderContext,
    ) -> String;
}

/// 无占位符渲染：原样返回。
#[derive(Debug, Default)]
pub struct NoPlaceholders;

impl PlaceholderRenderer for NoPlaceholders {
    fn render(
        &self,
        _enabled: bool,
        _player: Option<&qexed_player::OnlinePlayer>,
        text: &str,
        _context: &PlaceholderContext,
    ) -> String {
        text.to_string()
    }
}

/// 导航物品点击等菜单回调所需的占位符数据便捷构造。
pub fn lobby_placeholder_context(
    online_players: i32,
    max_players: i32,
    online_servers: usize,
    total_servers: usize,
    labels: &[String],
) -> PlaceholderContext {
    PlaceholderContext {
        online_players,
        max_players,
        lobby_online_servers: online_servers,
        lobby_total_servers: total_servers,
        lobby_servers: if labels.is_empty() {
            "none".to_string()
        } else {
            labels.join(", ")
        },
    }
}

/// v4 HashMap 别名重导出（模块内部使用）。
pub type StringMap = HashMap<String, String>;

/// 集群实体视图面（v4 ClusterEntityController::spawn_view_for_player）。
///
/// server 域 cluster_entities 迁移后由装配层实现；None 跳过集群视图。
pub trait ClusterEntityView: Send + Sync {
    /// 玩家视角的集群实体生成包。
    fn spawn_view_for_player(
        &self,
        player: &qexed_player::OnlinePlayer,
        rendering: &qexed_entities::EntityRendering,
        simulation_distance: i32,
    ) -> Vec<bytes::Bytes>;
}

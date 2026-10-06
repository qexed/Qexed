//! 实体系统与外部域（players / world / plugins）的交互边界。
//!
//! v4 的 EntityManager 直接依赖 `crate::players::PlayerManager`、
//! `crate::world::WorldManager`、`crate::plugins::PluginManager`。
//! v6 按域拆分 crate（play/entities/player/connection/plugins → world），且这些
//! 邻域 crate 尚是空壳，因此这里把 manager 需要的最小面收敛为 trait：
//! - [`ViewerSource`]：在线玩家快照 + 定向/广播发包 + 伤害/效果施加
//! - [`WorldAccess`]：方块状态查询 / 放置 / 纪元（缓存失效）
//! - [`WorldRulesAccess`]：维度规则快照（block_updates、时间、维度类型）
//! - [`EntityAiHost`]：插件 AI tick 查询
//! - [`BlockShapeSource`]：方块碰撞箱与名称（v4 在 inventory 域）
//!
//! server 域组装时提供实现；单测用本 crate 的 test_support 空实现。

use std::collections::BTreeMap;

use bytes::Bytes;

use crate::position::EntityPosition;

/// 玩家伤害类型（v4 `crate::players::PlayerDamageKind`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerDamageKind {
    MobAttack,
    Projectile,
    Explosion,
    Magic,
    Fire,
    Fall,
    Drown,
    Void,
    Command,
}

/// 在线玩家快照（v4 `crate::players::OnlinePlayer` 的实体域子集）。
#[derive(Debug, Clone)]
pub struct OnlinePlayerView {
    pub profile_id: uuid::Uuid,
    pub username: String,
    pub entity_id: i32,
    pub game_mode: i32,
    pub position: EntityPosition,
    pub dimension: String,
}

impl OnlinePlayerView {
    /// 是否可被实体攻击（非创造/旁观）。
    pub fn can_be_attacked(&self) -> bool {
        !matches!(self.game_mode, 1 | 3)
    }
}

/// 弹射物命中玩家事件（v4 `crate::players::ProjectileHitPlayerEvent`）。
#[derive(Debug, Clone)]
pub struct ProjectileHitPlayerEvent {
    pub shooter_profile_id: uuid::Uuid,
    pub target_profile_id: uuid::Uuid,
    pub projectile_entity_id: i32,
    pub projectile_kind: String,
    pub dimension: String,
    pub position: EntityPosition,
    pub configured_event: String,
    pub tag: String,
}

/// 方块碰撞箱（v4 `crate::inventory::BlockCollisionShape`）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BlockCollisionShape {
    pub min_x: f64,
    pub min_y: f64,
    pub min_z: f64,
    pub max_x: f64,
    pub max_y: f64,
    pub max_z: f64,
}

/// 方块放置更新结果。
#[derive(Debug, Clone)]
pub struct BlockUpdate {
    pub position: qexed_packet::net_types::Position,
    pub block_state: i32,
}

/// 玩家交互面。
pub trait ViewerSource: Send + Sync {
    /// 除指定玩家外的在线快照（v4 list_except；nil = 全部）。
    fn list_except(&self, excluded: uuid::Uuid) -> Vec<OnlinePlayerView>;

    /// 向指定玩家发送已编码数据包。
    fn send_packets_to(&self, profile_id: uuid::Uuid, packets: Vec<Bytes>);

    /// 广播数据包。
    fn broadcast_packets(&self, packets: Vec<Bytes>);

    /// 除指定玩家外广播。
    fn broadcast_packets_except(&self, excluded: uuid::Uuid, packets: Vec<Bytes>);

    /// 广播方块变更。
    fn broadcast_block_changed(
        &self,
        actor: uuid::Uuid,
        dimension: &str,
        position: qexed_packet::net_types::Position,
        block_state: i32,
    );

    /// 对玩家施加伤害。
    fn damage_player(
        &self,
        target: uuid::Uuid,
        amount: f32,
        kind: PlayerDamageKind,
        source_entity_id: i32,
        source_position: EntityPosition,
        knockback: f32,
    );

    /// 对玩家施加药水效果。
    fn apply_potion_effect(
        &self,
        target: uuid::Uuid,
        effect: &str,
        amplifier: i32,
        duration_ticks: i32,
        source_entity_id: i32,
        source_position: EntityPosition,
        knockback: f32,
    );

    /// 通知插件域弹射物命中玩家。
    fn emit_projectile_hit_player(&self, event: ProjectileHitPlayerEvent);
}

/// 世界访问面。
pub trait WorldAccess: Send + Sync {
    /// 方块状态查询（未加载返回 None）。
    fn block_state_at(
        &self,
        dimension: &str,
        position: &qexed_packet::net_types::Position,
    ) -> Option<i32>;

    /// 缓存纪元：变更后实体碰撞缓存整体失效。
    fn cache_epoch(&self) -> u64;

    /// 批量放置方块，返回实际更新。
    fn place_blocks(
        &self,
        dimension: &str,
        blocks: Vec<(qexed_packet::net_types::Position, i32)>,
    ) -> Result<Vec<BlockUpdate>, crate::error::EntitiesError>;
}

/// 世界规则快照（v4 `WorldRulesManager::snapshot` 的实体域子集）。
#[derive(Debug, Clone)]
pub struct WorldRulesSnapshot {
    pub dimension_type: String,
    pub time_value: i64,
    pub block_updates: bool,
}

/// 世界规则面。
pub trait WorldRulesAccess: Send + Sync {
    fn snapshot(&self, dimension: &str) -> WorldRulesSnapshot;
}

/// 方块形状/名称源（v4 inventory 域）。
pub trait BlockShapeSource: Send + Sync {
    fn block_collision_shape(&self, block_state: i32) -> Option<BlockCollisionShape>;
    fn block_name_for_state(&self, block_state: i32) -> Option<String>;
    fn is_air_block_state(&self, block_state: i32) -> bool;
    /// 空气状态 id（爆炸清块用）。
    fn air_block_state(&self) -> i32;
}

/// 插件实体 AI 面（v4 `PluginManager::handle_entity_ai_tick`）。
pub trait EntityAiHost: Send + Sync {
    fn handle_entity_ai_tick(&self, query: EntityAiTickQuery) -> Vec<EntityAiOperation>;
}

/// 插件自定义实体定义（v4 `qexed_plugin_api::CustomEntityDefinition`）。
#[derive(Debug, Clone, Default)]
pub struct CustomEntityDefinition {
    pub id: String,
    pub entity_type: String,
    pub shell_entity_type: String,
    pub registry_id: Option<i32>,
    pub display_name: String,
    pub ai: String,
    pub ai_params: BTreeMap<String, serde_json::Value>,
}

/// 插件 AI tick 查询载荷（v4 `qexed_plugin_api::EntityAiTickQuery`，位置字段简化为实体位置）。
#[derive(Debug, Clone)]
pub struct EntityAiTickQuery {
    pub entity: EntityAiEntityPayload,
    pub nearby_players: Vec<EntityAiPlayerPayload>,
    pub tick_ms: u64,
}

#[derive(Debug, Clone)]
pub struct EntityAiEntityPayload {
    pub key: String,
    pub entity_id: i32,
    pub entity_type: String,
    pub custom_type: String,
    pub ai: String,
    pub spawn_rule: String,
    pub ai_params: BTreeMap<String, serde_json::Value>,
    pub dimension: String,
    pub position: EntityPosition,
}

#[derive(Debug, Clone)]
pub struct EntityAiPlayerPayload {
    pub entity_id: i32,
    pub position: EntityPosition,
}

/// 插件 AI 操作（v4 `qexed_plugin_api::EntityAiOperation`）。
#[derive(Debug, Clone)]
pub enum EntityAiOperation {
    MoveDelta {
        x: f64,
        y: f64,
        z: f64,
        yaw: Option<f32>,
        pitch: Option<f32>,
    },
    LookAt {
        x: f64,
        y: f64,
        z: f64,
    },
    Remove,
}

/// 无插件宿主：AI 恒返回空操作。
#[derive(Debug, Default)]
pub struct NoEntityAiHost;

impl EntityAiHost for NoEntityAiHost {
    fn handle_entity_ai_tick(&self, _query: EntityAiTickQuery) -> Vec<EntityAiOperation> {
        Vec::new()
    }
}

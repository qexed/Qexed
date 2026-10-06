//! v4 qexed_plugin_api 转换辅助 + 实体域 ViewerSource 桥（play-gameplay 任务）。
//!
//! v6 中 qexed_plugin_api 并入 qexed_plugins::api，payload 类型由该 crate 提供；
//! OnlinePlayer -> payload 的构造函数（v4 的 player_payload_owned /
//! player_position_payload）在 v6 无家可归（api 层不依赖 qexed_player），
//! 因此在 play 域提供这些桥接（session.rs 为主实现，此处再导出方便 gameplay
//! 模块以 v4 同名路径调用）。
//!
//! 另：qexed_entities 的伤害/掉落接口以 `ViewerSource` trait 观察玩家面，
//! 而 qexed_player::PlayerManager 尚未实现该 trait（跨 crate 接线归装配层），
//! 这里提供 `PlayerViewerBridge` 包装，combat 等玩法模块直接可用。

use bytes::Bytes;
use qexed_entities::context::{
    OnlinePlayerView, PlayerDamageKind as EntitiesDamageKind,
    ProjectileHitPlayerEvent as EntitiesProjectileEvent, ViewerSource,
};
use qexed_player::{OnlinePlayer, PlayerDamageKind, PlayerManager};

pub(crate) use crate::session::{player_payload_owned, player_position_payload};
pub(crate) use qexed_plugins::api::PlayerAction;

/// qexed_protocol 与 qexed_entities 各自定义了同构的 EntityPosition（26.3 协议
/// crate 不再提供 add_entity::EntityPosition 载体），play 域在两域边界做字段级转换。
pub fn entities_position(position: qexed_protocol::types::EntityPosition) -> qexed_entities::EntityPosition {
    qexed_entities::EntityPosition {
        x: position.x,
        y: position.y,
        z: position.z,
        yaw: position.yaw,
        pitch: position.pitch,
        on_ground: position.on_ground,
    }
}

/// 反向转换（实体域 -> 协议域）。
pub fn protocol_position(position: qexed_entities::EntityPosition) -> qexed_protocol::types::EntityPosition {
    qexed_protocol::types::EntityPosition {
        x: position.x,
        y: position.y,
        z: position.z,
        yaw: position.yaw,
        pitch: position.pitch,
        on_ground: position.on_ground,
    }
}

fn online_player_view(player: &OnlinePlayer) -> OnlinePlayerView {
    OnlinePlayerView {
        profile_id: player.profile.uuid,
        username: player.profile.username.clone(),
        entity_id: player.entity_id,
        game_mode: player.game_mode,
        position: entities_position(player.position),
        dimension: player.dimension.clone(),
    }
}

/// PlayerManager 的实体域视图（v4 中 EntityManager 直接持有 PlayerManager）。
pub struct PlayerViewerBridge<'a> {
    players: &'a PlayerManager,
}

impl<'a> PlayerViewerBridge<'a> {
    pub fn new(players: &'a PlayerManager) -> Self {
        Self { players }
    }
}

impl ViewerSource for PlayerViewerBridge<'_> {
    fn list_except(&self, excluded: uuid::Uuid) -> Vec<OnlinePlayerView> {
        self.players
            .list_except(excluded)
            .iter()
            .map(online_player_view)
            .collect()
    }

    fn send_packets_to(&self, profile_id: uuid::Uuid, packets: Vec<Bytes>) {
        self.players.send_packets_to(profile_id, packets);
    }

    fn broadcast_packets(&self, packets: Vec<Bytes>) {
        self.players.broadcast_packets(packets);
    }

    fn broadcast_packets_except(&self, excluded: uuid::Uuid, packets: Vec<Bytes>) {
        self.players.broadcast_packets_except(excluded, packets);
    }

    fn broadcast_block_changed(
        &self,
        actor: uuid::Uuid,
        dimension: &str,
        position: qexed_packet::net_types::Position,
        block_state: i32,
    ) {
        self.players
            .broadcast_block_changed(actor, dimension, position, block_state, None);
    }

    fn damage_player(
        &self,
        target: uuid::Uuid,
        amount: f32,
        kind: EntitiesDamageKind,
        source_entity_id: i32,
        source_position: qexed_entities::EntityPosition,
        knockback: f32,
    ) {
        // 实体域枚举成员比玩家域多（Fire/Fall/Drown/Void/Command），玩家域统一折叠。
        let player_kind = match kind {
            EntitiesDamageKind::MobAttack => PlayerDamageKind::MobAttack,
            EntitiesDamageKind::Projectile => PlayerDamageKind::Projectile,
            EntitiesDamageKind::Explosion => PlayerDamageKind::Explosion,
            EntitiesDamageKind::Magic => PlayerDamageKind::Magic,
            _ => PlayerDamageKind::Generic,
        };
        self.players.damage_player(
            target,
            amount,
            player_kind,
            source_entity_id,
            protocol_position(source_position),
            knockback,
        );
    }

    fn apply_potion_effect(
        &self,
        target: uuid::Uuid,
        effect: &str,
        amplifier: i32,
        duration_ticks: i32,
        source_entity_id: i32,
        source_position: qexed_entities::EntityPosition,
        knockback: f32,
    ) {
        self.players.apply_potion_effect(
            target,
            effect,
            amplifier,
            duration_ticks,
            source_entity_id,
            protocol_position(source_position),
            knockback,
        );
    }

    fn emit_projectile_hit_player(&self, event: EntitiesProjectileEvent) {
        // qexed_player 的事件枚举与实体域事件结构不同构，这里直接构造玩家事件。
        let _ = self
            .players
            .emit_projectile_hit_player(qexed_player::ProjectileHitPlayerEvent {
                shooter_profile_id: event.shooter_profile_id,
                target_profile_id: event.target_profile_id,
                projectile_entity_id: event.projectile_entity_id,
                projectile_kind: event.projectile_kind,
                dimension: event.dimension,
                position: protocol_position(event.position),
                configured_event: event.configured_event,
                tag: event.tag,
            });
    }
}

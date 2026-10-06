//! 玩家攻击实体（v4 play/gameplay/combat.rs 迁移）。
//!
//! v6 适配：
//! - 集群实体伤害（v4 ClusterEntityController::damage_entity）经本文件定义的
//!   `ClusterDamageSink` trait 注入（server 域 cluster_entities 落地后实现）。
//! - 实体渲染参数用 qexed_entities::EntityRendering（v4 在 server 配置域）。
//! - DamageEvent/SetEntityMotion 按 26.3 v6 协议字段；实体伤害接口走
//!   plugin_bridge::PlayerViewerBridge（qexed_player 尚未实现实体域 ViewerSource）。

use qexed_packet::net_types::VarInt;
use qexed_protocol::to_client::play::{
    damage_event::DamageEvent, entity_event::EntityEvent, hurt_animation::HurtAnimation,
    set_entity_motion::SetEntityMotion,
};
use qexed_protocol::types::EntityPosition;

use super::items;
use crate::error::Result;
use crate::plugin_bridge::PlayerViewerBridge;

/// 集群实体伤害面（v4 ClusterEntityController::damage_entity 的 play 子集）。
pub trait ClusterDamageSink: Send + Sync {
    /// 对集群实体施加伤害；None 表示目标不在集群视图。返回是否击杀。
    fn damage_entity(
        &self,
        players: &qexed_player::PlayerManager,
        player: &qexed_player::OnlinePlayer,
        rendering: &qexed_entities::EntityRendering,
        simulation_distance: i32,
        target_entity_id: i32,
        damage: f32,
    ) -> Result<Option<ClusterDamageResult>>;
}

/// 集群实体伤害结果（v4 ClusterEntityController 的返回子集）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClusterDamageResult {
    pub killed: bool,
}

/// 无集群：damage_entity 恒返回 None（单机/未启用集群时使用）。
#[derive(Debug, Default)]
pub struct NoClusterDamage;

impl ClusterDamageSink for NoClusterDamage {
    fn damage_entity(
        &self,
        _players: &qexed_player::PlayerManager,
        _player: &qexed_player::OnlinePlayer,
        _rendering: &qexed_entities::EntityRendering,
        _simulation_distance: i32,
        _target_entity_id: i32,
        _damage: f32,
    ) -> Result<Option<ClusterDamageResult>> {
        Ok(None)
    }
}

#[derive(Debug, Default)]
pub(in crate::gameplay) struct CombatOutcome {
    pub(in crate::gameplay) handled: bool,
    pub(in crate::gameplay) killed: bool,
    pub(in crate::gameplay) damaged_held_item: bool,
    pub(in crate::gameplay) actions: Vec<qexed_plugins::api::PlayerAction>,
}

#[allow(clippy::too_many_arguments)]
pub(in crate::gameplay) async fn attack_entity<W>(
    sink: &mut qexed_connection::transport::PacketSink<W>,
    players: &qexed_player::PlayerManager,
    cluster_entities: Option<&dyn ClusterDamageSink>,
    entities: &qexed_entities::EntityManager,
    player: &qexed_player::OnlinePlayer,
    inventory: &mut crate::inventory::PlayerInventory,
    plugins: &qexed_plugins::PluginManager,
    config: &crate::config::GameplayConfig,
    effects: &super::effects::EffectRuntime,
    target_entity_id: i32,
    rendering: &qexed_entities::EntityRendering,
    simulation_distance: i32,
) -> Result<CombatOutcome>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    if !config.combat {
        return Ok(CombatOutcome::default());
    }

    let held = inventory.held_item().clone();
    let mut damage = base_attack_damage(&held);
    let mut knockback = base_knockback(&held);
    let mut fire_ticks = 0;
    if config.enchantments {
        damage += combat_enchantment_damage(&held);
        knockback += items::enchantment_level(&held, "minecraft:knockback") as f32 * 0.35;
        fire_ticks = items::enchantment_level(&held, "minecraft:fire_aspect").max(0) * 80;
    }
    if config.potion_effects {
        if let Some(strength) = effects.amplifier("minecraft:strength") {
            damage += 3.0 * (strength + 1) as f32;
        }
        if let Some(weakness) = effects.amplifier("minecraft:weakness") {
            damage = (damage - 4.0 * (weakness + 1) as f32).max(0.0);
        }
    }

    let target = entities.entity_by_runtime_id(target_entity_id);
    let target_type = target
        .as_ref()
        .map(|entity| entity.entity_type.clone())
        .unwrap_or_default();
    let target_player = players.player_by_entity_id(target_entity_id);
    let should_broadcast_animation = target.is_some() || target_player.is_some();
    let response = plugins.apply_player_attack(qexed_plugins::api::PlayerAttackQuery {
        player: crate::plugin_bridge::player_payload_owned(player),
        dimension: player.dimension.clone(),
        position: crate::plugin_bridge::player_position_payload(player.position),
        target_entity_id,
        target_uuid: target
            .as_ref()
            .map(|entity| *entity.uuid.as_bytes())
            .or_else(|| {
                target_player
                    .as_ref()
                    .map(|player| *player.profile.uuid.as_bytes())
            })
            .unwrap_or_default(),
        target_player: target_player
            .as_ref()
            .map(crate::plugin_bridge::player_payload_owned),
        target_type,
        weapon: items::item_stack_payload(&held),
        damage,
        knockback,
        fire_ticks,
    });
    if should_broadcast_animation {
        players.broadcast_animation(player.profile.uuid, 0);
    }
    if response.cancel {
        return Ok(CombatOutcome {
            handled: !response.actions.is_empty(),
            actions: response.actions,
            ..CombatOutcome::default()
        });
    }
    if let Some(plugin_damage) = response.damage {
        damage = plugin_damage.max(0.0);
    }
    if let Some(plugin_knockback) = response.knockback {
        knockback = plugin_knockback.max(0.0);
    }
    if let Some(plugin_fire_ticks) = response.fire_ticks {
        fire_ticks = plugin_fire_ticks.max(0);
    }
    if damage <= 0.0 {
        return Ok(CombatOutcome::default());
    }

    let mut outcome = CombatOutcome {
        handled: true,
        actions: response.actions,
        ..CombatOutcome::default()
    };

    let viewers = PlayerViewerBridge::new(players);
    if let Some(result) =
        entities.damage_managed_entity(&viewers, rendering, target_entity_id, damage)?
    {
        send_damage_feedback(
            sink,
            players,
            player,
            crate::plugin_bridge::protocol_position(result.entity.position),
            target_entity_id,
        )
        .await?;
        if knockback > 0.0 {
            send_knockback(
                sink,
                players,
                player.position,
                crate::plugin_bridge::protocol_position(result.entity.position),
                target_entity_id,
                knockback,
            )
            .await?;
        }
        if fire_ticks > 0 {
            let _ =
                entities.ignite_managed_entity(&viewers, rendering, target_entity_id, fire_ticks)?;
        }
        if result.killed {
            send_death_animation(sink, players, player, target_entity_id).await?;
            drop_entity_death_loot(
                sink,
                players,
                entities,
                player,
                &result.entity,
                rendering,
            )
            .await?;
        }
        outcome.killed = result.killed;
        outcome.damaged_held_item = true;
    } else if target_player.is_some() && !outcome.actions.is_empty() {
        outcome.damaged_held_item = true;
    } else if let Some(cluster_entities) = cluster_entities
        && let Some(result) = cluster_entities.damage_entity(
            players,
            player,
            rendering,
            simulation_distance,
            target_entity_id,
            damage,
        )?
    {
        if !should_broadcast_animation {
            players.broadcast_animation(player.profile.uuid, 0);
        }
        outcome.killed = result.killed;
        outcome.damaged_held_item = true;
    }
    Ok(outcome)
}

async fn drop_entity_death_loot<W>(
    sink: &mut qexed_connection::transport::PacketSink<W>,
    players: &qexed_player::PlayerManager,
    entities: &qexed_entities::EntityManager,
    player: &qexed_player::OnlinePlayer,
    entity: &qexed_entities::ManagedEntity,
    rendering: &qexed_entities::EntityRendering,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    for (item_name, count) in entity_death_drops(&entity.entity_type) {
        let Some(item_id) = crate::inventory::item_id_for_name(item_name) else {
            continue;
        };
        let item = crate::inventory::simple_item(item_id, *count);
        let viewers = PlayerViewerBridge::new(players);
        let updates = entities.drop_item_with_rendering(
            &viewers,
            player.profile.uuid,
            &entity.dimension,
            entity.position,
            item,
            rendering,
        )?;
        for update in updates {
            send_dropped_item_update(sink, update).await?;
        }
    }
    Ok(())
}

fn entity_death_drops(entity_type: &str) -> &'static [(&'static str, i32)] {
    match entity_type {
        "minecraft:pig" => &[("minecraft:porkchop", 2)],
        "minecraft:cow" | "minecraft:mooshroom" => {
            &[("minecraft:beef", 2), ("minecraft:leather", 1)]
        }
        "minecraft:sheep" => &[("minecraft:mutton", 1), ("minecraft:white_wool", 1)],
        "minecraft:chicken" => &[("minecraft:chicken", 1), ("minecraft:feather", 1)],
        "minecraft:rabbit" => &[("minecraft:rabbit", 1), ("minecraft:rabbit_hide", 1)],
        _ => &[],
    }
}

async fn send_dropped_item_update<W>(
    sink: &mut qexed_connection::transport::PacketSink<W>,
    update: qexed_entities::DroppedItemUpdate,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let packets = match update {
        qexed_entities::DroppedItemUpdate::Spawned(entity) => entity.spawn_packets(
            qexed_entities::registry::entity_type_id("minecraft:item")?,
        )?,
        qexed_entities::DroppedItemUpdate::Merged(entity) => entity.metadata_packets()?,
    };
    for packet in packets {
        sink.send_raw(packet).await?;
    }
    Ok(())
}

async fn send_death_animation<W>(
    sink: &mut qexed_connection::transport::PacketSink<W>,
    players: &qexed_player::PlayerManager,
    player: &qexed_player::OnlinePlayer,
    target_entity_id: i32,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let packet = EntityEvent {
        entity_id: target_entity_id,
        event_id: 3,
    };
    let bytes = qexed_player::packet_bytes(packet.clone())?;
    sink.send(packet).await?;
    players.broadcast_packets_except(player.profile.uuid, vec![bytes]);
    Ok(())
}

async fn send_damage_feedback<W>(
    sink: &mut qexed_connection::transport::PacketSink<W>,
    players: &qexed_player::PlayerManager,
    player: &qexed_player::OnlinePlayer,
    target_position: EntityPosition,
    target_entity_id: i32,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    // source_type 0 = minecraft:generic（damage_type 注册表 holder 占位）。
    let damage_event = DamageEvent {
        entity_id: VarInt(target_entity_id),
        source_type: VarInt(0),
        source_cause_id: VarInt(player.entity_id),
        source_direct_id: VarInt(player.entity_id),
        source_position: None,
    };
    let hurt = HurtAnimation {
        entity_id: VarInt(target_entity_id),
        yaw: target_position.yaw,
    };
    let damage_bytes = qexed_player::packet_bytes(damage_event.clone())?;
    let hurt_bytes = qexed_player::packet_bytes(hurt.clone())?;
    sink.send(damage_event).await?;
    sink.send(hurt).await?;
    players.broadcast_packets_except(player.profile.uuid, vec![damage_bytes, hurt_bytes]);
    Ok(())
}

async fn send_knockback<W>(
    sink: &mut qexed_connection::transport::PacketSink<W>,
    players: &qexed_player::PlayerManager,
    attacker: EntityPosition,
    target: EntityPosition,
    target_entity_id: i32,
    strength: f32,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let dx = target.x - attacker.x;
    let dz = target.z - attacker.z;
    let length = (dx * dx + dz * dz).sqrt().max(0.0001);
    let packet = SetEntityMotion {
        id: VarInt(target_entity_id),
        movement_x: dx / length * f64::from(strength),
        movement_y: 0.35,
        movement_z: dz / length * f64::from(strength),
    };
    let bytes = qexed_player::packet_bytes(packet.clone())?;
    sink.send(packet).await?;
    players.broadcast_packets_except(uuid::Uuid::nil(), vec![bytes]);
    Ok(())
}


// ── gameplay hooks 装配层辅助（外部伤害反馈） ──

/// 受击朝向：从伤害来源反推受击动画 yaw（v4 damage_yaw_from_source）。
pub fn damage_yaw_from_source(
    position: EntityPosition,
    source: EntityPosition,
) -> f32 {
    let dx = source.x - position.x;
    let dz = source.z - position.z;
    if dx.abs() <= f64::EPSILON && dz.abs() <= f64::EPSILON {
        return position.yaw;
    }
    (dz.atan2(dx).to_degrees() as f32) - 90.0
}

/// 外部伤害击退速度（v4 external_damage_knockback）。
pub fn external_damage_knockback_packet(
    entity_id: i32,
    position: EntityPosition,
    source: EntityPosition,
    strength: f32,
) -> SetEntityMotion {
    let dx = position.x - source.x;
    let dz = position.z - source.z;
    let length = (dx * dx + dz * dz).sqrt().max(0.0001);
    SetEntityMotion {
        id: VarInt(entity_id),
        movement_x: dx / length * f64::from(strength),
        movement_y: 0.35,
        movement_z: dz / length * f64::from(strength),
    }
}

fn base_attack_damage(slot: &qexed_protocol::types::Slot) -> f32 {
    match items::item_name(slot).as_str() {
        name if name.ends_with("_sword") => material_damage(name, 4.0, 5.0, 6.0, 7.0, 8.0),
        name if name.ends_with("_axe") => material_damage(name, 7.0, 9.0, 9.0, 9.0, 10.0),
        name if name.ends_with("_pickaxe") => material_damage(name, 2.0, 3.0, 4.0, 5.0, 6.0),
        name if name.ends_with("_shovel") => material_damage(name, 2.5, 3.5, 4.5, 5.5, 6.5),
        name if name.ends_with("_hoe") => 1.0,
        "minecraft:trident" => 9.0,
        "minecraft:mace" => 6.0,
        _ => 1.0,
    }
}

fn material_damage(
    name: &str,
    wooden: f32,
    stone: f32,
    iron: f32,
    diamond: f32,
    netherite: f32,
) -> f32 {
    if name.contains("wooden_") || name.contains("golden_") {
        wooden
    } else if name.contains("stone_") {
        stone
    } else if name.contains("iron_") {
        iron
    } else if name.contains("diamond_") {
        diamond
    } else if name.contains("netherite_") {
        netherite
    } else {
        1.0
    }
}

fn combat_enchantment_damage(slot: &qexed_protocol::types::Slot) -> f32 {
    let sharpness = items::enchantment_level(slot, "minecraft:sharpness").max(0);
    let smite = items::enchantment_level(slot, "minecraft:smite").max(0);
    let bane = items::enchantment_level(slot, "minecraft:bane_of_arthropods").max(0);
    let density = items::enchantment_level(slot, "minecraft:density").max(0);
    let breach = items::enchantment_level(slot, "minecraft:breach").max(0);
    let impaling = items::enchantment_level(slot, "minecraft:impaling").max(0);
    let mut bonus = 0.0;
    if sharpness > 0 {
        bonus += 0.5 * sharpness as f32 + 0.5;
    }
    bonus + (smite + bane + density + breach + impaling) as f32 * 2.5
}

fn base_knockback(slot: &qexed_protocol::types::Slot) -> f32 {
    match items::item_name(slot).as_str() {
        name if name.ends_with("_sword") => 0.35,
        name if name.ends_with("_axe") => 0.45,
        "minecraft:mace" => 0.65,
        _ => 0.25,
    }
}

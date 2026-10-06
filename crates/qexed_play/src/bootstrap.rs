//! play 会话初始化状态包（v4 play/bootstrap.rs 迁移）。
//!
//! v6 适配（26.3 协议）：
//! - GameStateChange{reason:13} -> GameEvent{event:13}（level_chunks_load_start）
//! - UpdateViewDistance -> SetChunkCacheRadius；UpdateViewPosition -> SetChunkCacheCenter
//! - SetHeldSlot.slot 改 VarInt
//! - 世界规则经 [`WorldRulesSource`]（crate::context）注入
//! - 背包/装备包（v4 crate::inventory）经 [`SessionInventory`] 注入
//! - 命令树包（v4 permissions/plugins 构造）经 `command_tree: &[Bytes]` 注入

// play-gameplay 接线前部分 API 暂未被本 crate 引用（供其改造 runtime.rs 时使用）。
#![allow(dead_code)]


use qexed_connection::transport::PacketSink;
use qexed_packet::net_types::VarInt;
use qexed_protocol::to_client::play::{
    change_difficulty::ChangeDifficulty,
    game_event::GameEvent,
    initialize_border::InitializeBorder,
    player_abilities::PlayerAbilities,
    player_info_update::{PlayerInfoActions, PlayerInfoEntry, PlayerInfoUpdate},
    server_data::ServerData,
    set_chunk_cache_center::SetChunkCacheCenter,
    set_chunk_cache_radius::SetChunkCacheRadius,
    set_default_spawn_position::SetDefaultSpawnPosition,
    set_experience::SetExperience,
    set_health::SetHealth,
    set_held_slot::SetHeldSlot,
    set_simulation_distance::SetSimulationDistance,
    set_time::SetTime,
    ticking_state::TickingState,
};

use qexed_player::OnlinePlayer;

use crate::config::WorldConfig;
use crate::context::WorldRulesSource;
use crate::error::Result;
use crate::util::{chunk_coord, favicon_bytes, player_ability_flags, text_component};

/// 会话背包面（v4 crate::inventory::PlayerInventory 的 bootstrap 子集）。
///
/// TODO(play-gameplay)：inventory 域落地后由装配层桥接（selected_slot /
/// set_player_inventory_packets / visible_equipment / equipment_packet）。
pub trait SessionInventory: Send + Sync {
    /// 选中的快捷栏槽。
    fn selected_slot(&self) -> usize;
    /// 初始背包同步包（SetPlayerInventory 系列）。
    fn set_player_inventory_packets(&self) -> Vec<bytes::Bytes>;
    /// 可见装备（登录/刷新发给其他玩家用）。
    fn visible_equipment(&self) -> Vec<qexed_protocol::to_client::play::set_equipment::EquipmentEntry>;
    /// 装备包（SetEquipment）。
    fn equipment_packet(
        &self,
        entity_id: i32,
        equipment: Vec<qexed_protocol::to_client::play::set_equipment::EquipmentEntry>,
    ) -> Result<Option<qexed_protocol::to_client::play::set_equipment::SetEquipment>>;
}

/// 生存快照（v4 StoredSurvival 的 play 子集；字段同构）。
#[derive(Debug, Clone, Copy, Default)]
pub struct SurvivalSnapshot {
    pub health: f32,
    pub food: i32,
    pub saturation: f32,
}

impl SurvivalSnapshot {
    /// 从 PlayerData 存档生存字段构造（qexed_player 未导出 StoredSurvival，经字段拷贝）。
    pub fn from_stored(health: f32, food: i32, saturation: f32) -> Self {
        Self {
            health,
            food,
            saturation,
        }
    }
}

/// 服务器展示信息（motd/favicon；v4 config.server 的展示子集）。
#[derive(Debug, Clone, Default)]
pub struct ServerDisplay {
    pub motd: Vec<String>,
    pub favicon: String,
}

/// 初始玩家状态（v4 send_initial_player_state）。
#[allow(clippy::too_many_arguments)]
pub async fn send_initial_player_state<W>(
    sink: &mut PacketSink<W>,
    world_config: &WorldConfig,
    world_rules: &dyn WorldRulesSource,
    display: &ServerDisplay,
    play_dimension: &str,
    player: &OnlinePlayer,
    inventory: &dyn SessionInventory,
    survival: SurvivalSnapshot,
    command_tree: bytes::Bytes,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let view_distance = world_config.view_distance.max(1);
    let simulation_distance = world_config.simulation_distance.max(1);
    let game_time = world_rules.current_time(play_dimension);
    sink.send(ChangeDifficulty {
        difficulty: 2,
        locked: false,
    })
    .await?;

    sink.send(PlayerAbilities {
        flags: player_ability_flags(world_config.game_mode, world_config.allow_flight),
        flying_speed: 0.05,
        walking_speed: 0.1,
    })
    .await?;

    sink.send(SetHeldSlot {
        slot: VarInt(inventory.selected_slot() as i32),
    })
    .await?;
    for packet in inventory.set_player_inventory_packets() {
        sink.send_raw(packet).await?;
    }
    if let Some(equipment) = inventory.equipment_packet(player.entity_id, inventory.visible_equipment())? {
        sink.send(equipment).await?;
    }
    sink.send(SetExperience {
        experience_progress: 0.0,
        experience_level: VarInt(0),
        total_experience: VarInt(0),
    })
    .await?;

    // v4 super::recipes::send_initial_recipe_book(sink)：
    // 配方书初始同步归 play-gameplay（recipes 模块），此处跳过。

    sink.send(ServerData {
        motd: text_component(display.motd.join("\n")),
        icon_bytes: favicon_bytes(&display.favicon),
    })
    .await?;

    sink.send(PlayerInfoUpdate {
        actions: PlayerInfoActions::player_initializing(),
        entries: vec![PlayerInfoEntry::from_profile(
            &player.profile,
            world_config.game_mode.protocol_id(),
        )],
    })
    .await?;

    // v4 在此发送 lobby 服务器专属命令树；v6 由调用方注入（commands 包）。
    sink.send_raw(command_tree).await?;

    sink.send(InitializeBorder::default()).await?;
    sink.send(SetTime {
        game_time,
        clock_updates: Vec::new(),
    })
    .await?;

    sink.send(SetDefaultSpawnPosition {
        dimension: play_dimension.to_string(),
        position: qexed_packet::net_types::Position {
            x: player.position.x.floor() as i32,
            y: player.position.y.floor() as i32,
            z: player.position.z.floor() as i32,
        },
        yaw: player.position.yaw,
        pitch: player.position.pitch,
    })
    .await?;

    // v4 GameStateChange{reason: 13}；v6 GameEvent{event: 13}（level_chunks_load_start）。
    sink.send(GameEvent {
        event: 13,
        param: 0.0,
    })
    .await?;

    sink.send(TickingState::default()).await?;

    sink.send(SetHealth {
        health: survival.health,
        food: VarInt(survival.food),
        saturation: survival.saturation,
    })
    .await?;

    sink.send(SetSimulationDistance {
        simulation_distance: VarInt(simulation_distance),
    })
    .await?;
    sink.send(SetChunkCacheRadius {
        radius: VarInt(view_distance),
    })
    .await?;
    sink.send(SetChunkCacheCenter {
        x: VarInt(chunk_coord(player.position.x)),
        z: VarInt(chunk_coord(player.position.z)),
    })
    .await?;

    Ok(())
}

/// 重生后的玩家状态（v4 send_respawn_player_state）。
pub async fn send_respawn_player_state<W>(
    sink: &mut PacketSink<W>,
    world_config: &WorldConfig,
    world_rules: &dyn WorldRulesSource,
    play_dimension: &str,
    position: qexed_protocol::types::EntityPosition,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let view_distance = world_config.view_distance.max(1);
    let simulation_distance = world_config.simulation_distance.max(1);
    let game_time = world_rules.current_time(play_dimension);

    sink.send(ChangeDifficulty {
        difficulty: 2,
        locked: false,
    })
    .await?;
    sink.send(SetExperience {
        experience_progress: 0.0,
        experience_level: VarInt(0),
        total_experience: VarInt(0),
    })
    .await?;
    sink.send(InitializeBorder::default()).await?;
    sink.send(SetTime {
        game_time,
        clock_updates: Vec::new(),
    })
    .await?;
    sink.send(SetDefaultSpawnPosition {
        dimension: play_dimension.to_string(),
        position: qexed_packet::net_types::Position {
            x: position.x.floor() as i32,
            y: position.y.floor() as i32,
            z: position.z.floor() as i32,
        },
        yaw: position.yaw,
        pitch: position.pitch,
    })
    .await?;
    sink.send(GameEvent {
        event: 13,
        param: 0.0,
    })
    .await?;
    sink.send(TickingState::default()).await?;
    sink.send(SetSimulationDistance {
        simulation_distance: VarInt(simulation_distance),
    })
    .await?;
    sink.send(SetChunkCacheRadius {
        radius: VarInt(view_distance),
    })
    .await?;
    sink.send(SetChunkCacheCenter {
        x: VarInt(chunk_coord(position.x)),
        z: VarInt(chunk_coord(position.z)),
    })
    .await?;

    Ok(())
}

/// 已在线玩家初始可见（v4 send_existing_players；玩家事件包由 qexed_player 提供）。
pub async fn send_existing_players<W>(
    sink: &mut PacketSink<W>,
    players: &qexed_player::PlayerManager,
    profile_id: uuid::Uuid,
    player_entity_type: i32,
    play_dimension: &str,
    viewer_position: qexed_protocol::types::EntityPosition,
    render_distance: f64,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    for player in players.list_except(profile_id) {
        if !within_render_distance(viewer_position, player.position, render_distance) {
            continue;
        }
        let event = qexed_player::PlayerEvent::Joined(player);
        for packet in event.packets(player_entity_type, play_dimension)? {
            sink.send_raw(packet).await?;
        }
    }
    Ok(())
}

/// 已存在实体初始可见（v4 send_existing_entities；渲染参数用 qexed_entities 的 EntityRendering）。
pub async fn send_existing_entities<W>(
    sink: &mut PacketSink<W>,
    entities: &qexed_entities::EntityManager,
    dimension: &str,
    viewer_position: qexed_protocol::types::EntityPosition,
    rendering: &qexed_entities::EntityRendering,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    for packet in entities.spawn_packets_for_view(
        dimension,
        entities_position(viewer_position),
        rendering,
    )? {
        sink.send_raw(packet).await?;
    }
    Ok(())
}

fn within_render_distance(
    left: qexed_protocol::types::EntityPosition,
    right: qexed_protocol::types::EntityPosition,
    distance: f64,
) -> bool {
    if distance <= 0.0 {
        return false;
    }
    let dx = left.x - right.x;
    let dz = left.z - right.z;
    (dx * dx + dz * dz) <= distance * distance
}

/// protocol::types::EntityPosition -> qexed_entities::EntityPosition（同构拷贝）。
pub fn entities_position(
    position: qexed_protocol::types::EntityPosition,
) -> qexed_entities::EntityPosition {
    qexed_entities::EntityPosition {
        x: position.x,
        y: position.y,
        z: position.z,
        yaw: position.yaw,
        pitch: position.pitch,
        on_ground: position.on_ground,
    }
}

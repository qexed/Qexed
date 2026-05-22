use anyhow::{Context, Result};
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::OnceLock,
};

use qexed_packet::{Packet, net_types::VarInt};
use qexed_protocol::to_client::play::{
    change_difficulty::ChangeDifficulty,
    chunk_batch_finished::ChunkBatchFinished,
    chunk_batch_start::ChunkBatchStart,
    forget_level_chunk::ForgetLevelChunk,
    game_state_change::GameStateChange,
    initialize_border::InitializeBorder,
    keep_alive::KeepAlive as ClientboundKeepAlive,
    login::Login,
    player_abilities::PlayerAbilities,
    player_chat::{PackedMessageSignature, PlayerChat},
    player_info_update::{PlayerInfoActions, PlayerInfoEntry, PlayerInfoUpdate},
    position::Position,
    server_data::ServerData,
    set_default_spawn_position::SetDefaultSpawnPosition,
    set_experience::SetExperience,
    set_health::SetHealth,
    set_held_slot::SetHeldSlot,
    set_simulation_distance::SetSimulationDistance,
    set_time::SetTime,
    system_chat::SystemChat,
    ticking_state::TickingState,
    update_view_distance::UpdateViewDistance,
    update_view_position::UpdateViewPosition,
};
use qexed_protocol::to_server::play::{
    accept_teleportation::AcceptTeleportation, chat_ack::ChatAck, chat_command::ChatCommand,
    chat_message::ChatMessage, chat_session_update::ChatSessionUpdate,
    chunk_batch_received::ChunkBatchReceived, keep_alive::KeepAlive as ServerboundKeepAlive,
    move_player_pos::MovePlayerPos, move_player_pos_rot::MovePlayerPosRot,
    move_player_rot::MovePlayerRot, move_player_status_only::MovePlayerStatusOnly,
};

use crate::players::{OnlinePlayer, PlayerEvent, PlayerManager, PlayerSession};
use crate::world::WorldManager;

const TELEPORT_ID: i32 = 1;
const KEEP_ALIVE_INTERVAL: std::time::Duration = std::time::Duration::from_secs(15);
const REGISTRIES_REPORT: &str = "assets/reports/registries.json";

pub async fn initialize<R, W>(
    packets: &mut qexed_tcp_connect::PacketStream<R>,
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    config: &qexed_config::app::qexed::Qexed,
    authenticator: &crate::auth::Authenticator,
    world: &WorldManager,
    players: &PlayerManager,
    profile: &qexed_packet::net_types::GameProfile,
) -> Result<()>
where
    R: tokio::io::AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
{
    let world_config = &config.server.world;
    let spawn = &world_config.spawn;
    let view_distance = world_config.view_distance.max(1);
    let simulation_distance = world_config.simulation_distance.max(1);
    let spawn_chunk_x = chunk_coord(spawn.x);
    let spawn_chunk_z = chunk_coord(spawn.z);
    let position = qexed_protocol::to_client::play::add_entity::EntityPosition {
        x: spawn.x,
        y: spawn.y,
        z: spawn.z,
        yaw: spawn.yaw,
        pitch: spawn.pitch,
        on_ground: true,
    };
    let session = players.join(profile.clone(), position);
    let leave_guard = PlayerLeaveGuard::new(players, profile.uuid);
    let player_entity_type = entity_type_id("minecraft:player")?;

    log::debug!(
        "初始化 Play 态: dimension={}, spawn=({}, {}, {}), yaw={}, pitch={}",
        world_config.dimension,
        spawn.x,
        spawn.y,
        spawn.z,
        spawn.yaw,
        spawn.pitch
    );

    sink.send(Login {
        entity_id: session.player.entity_id,
        is_hardcore: false,
        dimension_names: vec![world_config.dimension.clone()],
        max_player: VarInt(config.server.max_player.max(0)),
        view_distance: VarInt(view_distance),
        simulation_distance: VarInt(simulation_distance),
        reduced_debug_info: false,
        enable_respawn_screen: true,
        do_limited_crafting: false,
        dimension_type: VarInt(dimension_type_holder_id(&world_config.dimension_type)),
        dimension_name: world_config.dimension.clone(),
        hashed_seed: 0,
        game_mode: 1,
        previous_game_mode: -1,
        is_debug: false,
        is_flat: true,
        has_death_location: false,
        death_dimension_name: None,
        death_position: None,
        portal_cooldown: VarInt(0),
        sea_level: VarInt(63),
        enforces_secure_chat: config.server.online_mode,
    })
    .await?;

    send_initial_player_state(sink, config, world_config, &session.player).await?;
    send_existing_players(sink, players, profile.uuid, player_entity_type).await?;

    sink.send(Position {
        teleport_id: VarInt(TELEPORT_ID),
        x: spawn.x,
        y: spawn.y,
        z: spawn.z,
        dx: 0.0,
        dy: 0.0,
        dz: 0.0,
        yaw: spawn.yaw,
        pitch: spawn.pitch,
        flags: 0,
    })
    .await?;

    sink.send(SystemChat {
        content: text_component("Qexed: 正在载入空世界"),
        overlay: false,
    })
    .await?;

    let mut chunk_state = ChunkSendState::new(
        world_config.dimension.clone(),
        spawn_chunk_x,
        spawn_chunk_z,
        view_distance,
    );
    chunk_state.send_missing_chunks(sink, world).await?;
    sink.flush().await?;

    let result = wait_for_play_packets(
        packets,
        sink,
        config,
        authenticator,
        world,
        players,
        session,
        player_entity_type,
        profile,
        chunk_state,
    )
    .await;
    leave_guard.leave();
    result
}

async fn send_initial_player_state<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    config: &qexed_config::app::qexed::Qexed,
    world_config: &qexed_config::app::qexed::server::World,
    player: &OnlinePlayer,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let view_distance = world_config.view_distance.max(1);
    let simulation_distance = world_config.simulation_distance.max(1);
    let spawn_chunk_x = chunk_coord(world_config.spawn.x);
    let spawn_chunk_z = chunk_coord(world_config.spawn.z);

    sink.send(ChangeDifficulty {
        difficulty: 2,
        locked: false,
    })
    .await?;

    sink.send(PlayerAbilities {
        flags: PlayerAbilities::INVULNERABLE
            | PlayerAbilities::CAN_FLY
            | PlayerAbilities::INSTABUILD,
        flying_speed: 0.05,
        walking_speed: 0.1,
    })
    .await?;

    sink.send(SetHeldSlot { slot: VarInt(0) }).await?;
    sink.send(SetExperience {
        experience_progress: 0.0,
        experience_level: VarInt(0),
        total_experience: VarInt(0),
    })
    .await?;

    sink.send(ServerData {
        motd: text_component(config.server.motd.join("\n")),
        icon_bytes: favicon_bytes(&config.server.favicon),
    })
    .await?;

    sink.send(PlayerInfoUpdate {
        actions: PlayerInfoActions::player_initializing(),
        entries: vec![PlayerInfoEntry::from_profile(&player.profile, 1)],
    })
    .await?;

    sink.send(InitializeBorder::default()).await?;
    sink.send(SetTime {
        game_time: 0,
        clock_updates: Vec::new(),
    })
    .await?;

    sink.send(SetDefaultSpawnPosition {
        dimension: world_config.dimension.clone(),
        position: qexed_packet::net_types::Position {
            x: world_config.spawn.x.floor() as i32,
            y: world_config.spawn.y.floor() as i32,
            z: world_config.spawn.z.floor() as i32,
        },
        yaw: world_config.spawn.yaw,
        pitch: world_config.spawn.pitch,
    })
    .await?;

    sink.send(GameStateChange {
        reason: 13,
        game_mode: 0.0,
    })
    .await?;
    sink.send(TickingState::default()).await?;

    sink.send(SetHealth {
        health: 20.0,
        food: VarInt(20),
        saturation: 5.0,
    })
    .await?;

    sink.send(SetSimulationDistance {
        simulation_distance: VarInt(simulation_distance),
    })
    .await?;
    sink.send(UpdateViewDistance {
        view_distance: VarInt(view_distance),
    })
    .await?;
    sink.send(UpdateViewPosition {
        chunk_x: VarInt(spawn_chunk_x),
        chunk_z: VarInt(spawn_chunk_z),
    })
    .await?;

    Ok(())
}

async fn send_existing_players<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    players: &PlayerManager,
    profile_id: uuid::Uuid,
    player_entity_type: i32,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    for player in players.list_except(profile_id) {
        for packet in crate::players::spawn_player_packets(&player, player_entity_type)? {
            sink.send_raw(packet).await?;
        }
    }
    Ok(())
}

struct ChunkSendState {
    dimension: String,
    center_x: i32,
    center_z: i32,
    view_distance: i32,
    visible_chunks: HashSet<(i32, i32)>,
}

impl ChunkSendState {
    fn new(dimension: String, center_x: i32, center_z: i32, view_distance: i32) -> Self {
        Self {
            dimension,
            center_x,
            center_z,
            view_distance: view_distance.max(1),
            visible_chunks: HashSet::new(),
        }
    }

    async fn update_center<W>(
        &mut self,
        sink: &mut qexed_tcp_connect::PacketSink<W>,
        world: &WorldManager,
        x: f64,
        z: f64,
    ) -> Result<()>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        let chunk_x = chunk_coord(x);
        let chunk_z = chunk_coord(z);
        if chunk_x == self.center_x && chunk_z == self.center_z {
            return Ok(());
        }

        let previous = self.visible_chunks.clone();
        let next = visible_chunk_set(chunk_x, chunk_z, self.view_distance);
        sink.send(UpdateViewPosition {
            chunk_x: VarInt(chunk_x),
            chunk_z: VarInt(chunk_z),
        })
        .await?;

        for (chunk_x, chunk_z) in previous.difference(&next) {
            sink.send(ForgetLevelChunk {
                chunk_x: *chunk_x,
                chunk_z: *chunk_z,
            })
            .await?;
        }

        self.center_x = chunk_x;
        self.center_z = chunk_z;
        self.visible_chunks = previous.intersection(&next).copied().collect();
        self.send_missing_chunks(sink, world).await?;
        sink.flush().await?;
        log::debug!("玩家移动到新区块，已补发视距区块: center=({chunk_x}, {chunk_z})");
        Ok(())
    }

    async fn send_missing_chunks<W>(
        &mut self,
        sink: &mut qexed_tcp_connect::PacketSink<W>,
        world: &WorldManager,
    ) -> Result<usize>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        let chunks = self.missing_chunks();
        if chunks.is_empty() {
            return Ok(0);
        }

        sink.send(ChunkBatchStart {}).await?;
        for (chunk_x, chunk_z) in &chunks {
            sink.send(world.network_chunk(&self.dimension, *chunk_x, *chunk_z)?)
                .await?;
            self.visible_chunks.insert((*chunk_x, *chunk_z));
        }
        sink.send(ChunkBatchFinished {
            batch_size: VarInt(chunks.len() as i32),
        })
        .await?;
        Ok(chunks.len())
    }

    fn missing_chunks(&self) -> Vec<(i32, i32)> {
        visible_chunks(self.center_x, self.center_z, self.view_distance)
            .into_iter()
            .filter(|chunk| !self.visible_chunks.contains(chunk))
            .collect()
    }
}

fn visible_chunk_set(center_x: i32, center_z: i32, view_distance: i32) -> HashSet<(i32, i32)> {
    visible_chunks(center_x, center_z, view_distance)
        .into_iter()
        .collect()
}

fn visible_chunks(center_x: i32, center_z: i32, view_distance: i32) -> Vec<(i32, i32)> {
    let view_distance = view_distance.max(1);
    let mut chunks = Vec::new();
    for chunk_z in center_z - view_distance..=center_z + view_distance {
        for chunk_x in center_x - view_distance..=center_x + view_distance {
            chunks.push((chunk_x, chunk_z));
        }
    }
    chunks
}

#[allow(dead_code)]
async fn send_spawn_chunks<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    world: &WorldManager,
    dimension: &str,
    center_x: i32,
    center_z: i32,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let mut chunk_state = ChunkSendState::new(dimension.to_string(), center_x, center_z, 1);
    chunk_state.send_missing_chunks(sink, world).await?;
    Ok(())
}

async fn wait_for_play_packets<R, W>(
    packets: &mut qexed_tcp_connect::PacketStream<R>,
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    config: &qexed_config::app::qexed::Qexed,
    authenticator: &crate::auth::Authenticator,
    world: &WorldManager,
    players: &PlayerManager,
    mut session: PlayerSession,
    player_entity_type: i32,
    profile: &qexed_packet::net_types::GameProfile,
    mut chunk_state: ChunkSendState,
) -> Result<()>
where
    R: tokio::io::AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
{
    let mut keep_alive = tokio::time::interval(KEEP_ALIVE_INTERVAL);
    keep_alive.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    keep_alive.tick().await;
    let mut pending_keep_alive = None;
    let mut chat_session: Option<crate::secure_chat::SecureChatSession> = None;
    let mut next_chat_global_index = 0;
    let enforce_secure_chat = config.server.online_mode;
    let mut position = session.player.position;

    loop {
        tokio::select! {
            event = session.receiver.recv() => {
                let Some(event) = event else {
                    continue;
                };
                if event_is_self(&event, profile.uuid) {
                    continue;
                }
                for packet in event.packets(player_entity_type)? {
                    sink.send_raw(packet).await?;
                }
                sink.flush().await?;
            }
            packet = packets.read_packet() => {
                let Some(mut payload) = packet? else {
                    return Ok(());
                };

                let packet_id = crate::connection::read_packet_id(&mut payload)?;
                if packet_id == AcceptTeleportation::ID {
                    let teleport = crate::connection::decode_payload::<AcceptTeleportation>(&mut payload)?;
                    log::debug!("客户端已确认传送: teleport_id={}", teleport.teleport_id.0);
                    continue;
                }

                if packet_id == ServerboundKeepAlive::ID {
                    let keep_alive = crate::connection::decode_payload::<ServerboundKeepAlive>(&mut payload)?;
                    match pending_keep_alive {
                        Some(expected) if keep_alive.keep_alive_id == expected => {
                            log::debug!("客户端已响应 KeepAlive: id={expected}");
                            pending_keep_alive = None;
                        }
                        Some(expected) => {
                            anyhow::bail!(
                                "客户端 KeepAlive 响应不匹配: 期望 {}, 实际 {}",
                                expected,
                                keep_alive.keep_alive_id
                            );
                        }
                        None => {
                            log::trace!("跳过未请求的客户端 KeepAlive 响应: id={}", keep_alive.keep_alive_id);
                        }
                    }
                    continue;
                }

                if packet_id == ChunkBatchReceived::ID {
                    let batch = crate::connection::decode_payload::<ChunkBatchReceived>(&mut payload)?;
                    log::debug!(
                        "客户端已确认区块批次，期望区块速率: {} chunks/tick",
                        batch.desired_chunks_per_tick
                    );
                    continue;
                }

                if packet_id == MovePlayerPos::ID {
                    let movement = crate::connection::decode_payload::<MovePlayerPos>(&mut payload)?;
                    position.x = movement.x;
                    position.y = movement.y;
                    position.z = movement.z;
                    position.on_ground = movement.flags & 0x01 != 0;
                    chunk_state.update_center(sink, world, movement.x, movement.z).await?;
                    players.update_position(profile.uuid, position);
                    continue;
                }

                if packet_id == MovePlayerPosRot::ID {
                    let movement = crate::connection::decode_payload::<MovePlayerPosRot>(&mut payload)?;
                    position.x = movement.x;
                    position.y = movement.y;
                    position.z = movement.z;
                    position.yaw = movement.yaw;
                    position.pitch = movement.pitch;
                    position.on_ground = movement.flags & 0x01 != 0;
                    chunk_state.update_center(sink, world, movement.x, movement.z).await?;
                    players.update_position(profile.uuid, position);
                    continue;
                }

                if packet_id == MovePlayerRot::ID {
                    let movement = crate::connection::decode_payload::<MovePlayerRot>(&mut payload)?;
                    position.yaw = movement.yaw;
                    position.pitch = movement.pitch;
                    position.on_ground = movement.flags & 0x01 != 0;
                    players.update_position(profile.uuid, position);
                    continue;
                }

                if packet_id == MovePlayerStatusOnly::ID {
                    let movement = crate::connection::decode_payload::<MovePlayerStatusOnly>(&mut payload)?;
                    position.on_ground = movement.flags & 0x01 != 0;
                    players.update_position(profile.uuid, position);
                    continue;
                }

                if packet_id == ChatAck::ID {
                    let ack = crate::connection::decode_payload::<ChatAck>(&mut payload)?;
                    if let Some(chat_session) = chat_session.as_mut() {
                        chat_session.apply_offset(ack.offset)?;
                    }
                    continue;
                }

                if packet_id == ChatMessage::ID {
                    let chat = crate::connection::decode_payload::<ChatMessage>(&mut payload)?;
                    log::debug!("收到聊天消息: {}", chat.message);
                    if let Some(chat_session) = chat_session.as_mut() {
                        let verified = chat_session.verify_message(profile.uuid, &chat)?;
                        sink.send(PlayerChat::pass_through(
                            next_chat_global_index,
                            profile.uuid,
                            verified.index,
                            verified.signature.clone(),
                            verified
                                .last_seen
                                .into_iter()
                                .map(PackedMessageSignature::full)
                                .collect(),
                            chat.message,
                            chat.timestamp,
                            chat.salt,
                            text_component(&profile.username),
                        )).await?;
                        chat_session.add_pending_signature(&verified.signature);
                        next_chat_global_index += 1;
                    } else if enforce_secure_chat {
                        anyhow::bail!("客户端未初始化 Mojang secure chat 会话");
                    } else {
                        sink.send(SystemChat {
                            content: text_component(format!("<{}> {}", profile.username, chat.message)),
                            overlay: false,
                        }).await?;
                    }
                    sink.flush().await?;
                    continue;
                }

                if packet_id == ChatSessionUpdate::ID {
                    let update = crate::connection::decode_payload::<ChatSessionUpdate>(&mut payload)?;
                    log::debug!("收到聊天会话更新: session_id={}", update.chat_session.session_id);
                    if enforce_secure_chat {
                        authenticator.verify_chat_session(profile.uuid, &update.chat_session).await?;
                    }
                    chat_session = Some(crate::secure_chat::SecureChatSession::new(&update.chat_session)?);
                    sink.send(PlayerInfoUpdate {
                        actions: PlayerInfoActions(PlayerInfoActions::INITIALIZE_CHAT),
                        entries: vec![PlayerInfoEntry {
                            profile_id: profile.uuid,
                            chat_session: Some(update.chat_session),
                            ..PlayerInfoEntry::default()
                        }],
                    }).await?;
                    sink.flush().await?;
                    continue;
                }

                if packet_id == ChatCommand::ID {
                    let command = crate::connection::decode_payload::<ChatCommand>(&mut payload)?;
                    log::debug!("收到聊天命令: /{}", command.command);
                    sink.send(SystemChat {
                        content: text_component(format!("未知命令: /{}", command.command)),
                        overlay: false,
                    }).await?;
                    sink.flush().await?;
                    continue;
                }

                log::trace!("跳过 Play 态服务端暂未处理的数据包 ID: {packet_id}");
            }
            _ = keep_alive.tick() => {
                if let Some(expected) = pending_keep_alive {
                    anyhow::bail!("客户端 KeepAlive 响应超时: id={expected}");
                }

                let keep_alive_id = keep_alive_id();
                sink.send(ClientboundKeepAlive { keep_alive_id }).await?;
                sink.flush().await?;
                pending_keep_alive = Some(keep_alive_id);
                log::debug!("发送 Play 态 KeepAlive: id={keep_alive_id}");
            }
        }
    }
}

fn event_is_self(event: &PlayerEvent, profile_id: uuid::Uuid) -> bool {
    match event {
        PlayerEvent::Joined(player) => player.profile.uuid == profile_id,
        PlayerEvent::Left {
            profile_id: left_id,
            entity_id: _,
        } => *left_id == profile_id,
        PlayerEvent::Moved {
            profile_id: moved_id,
            entity_id: _,
            position: _,
        } => *moved_id == profile_id,
    }
}

struct PlayerLeaveGuard<'a> {
    players: &'a PlayerManager,
    profile_id: uuid::Uuid,
    active: bool,
}

impl<'a> PlayerLeaveGuard<'a> {
    fn new(players: &'a PlayerManager, profile_id: uuid::Uuid) -> Self {
        Self {
            players,
            profile_id,
            active: true,
        }
    }

    fn leave(mut self) {
        self.players.leave(self.profile_id);
        self.active = false;
    }
}

impl Drop for PlayerLeaveGuard<'_> {
    fn drop(&mut self) {
        if self.active {
            self.players.leave(self.profile_id);
        }
    }
}

fn text_component(text: impl Into<String>) -> qexed_protocol::types::TextComponent {
    let mut map = std::collections::HashMap::new();
    map.insert(
        "text".to_string(),
        qexed_nbt::Tag::String(std::sync::Arc::from(text.into())),
    );
    qexed_nbt::Tag::Compound(std::sync::Arc::new(map))
}

fn favicon_bytes(favicon: &str) -> Option<Vec<u8>> {
    let payload = favicon.strip_prefix("data:image/png;base64,")?;
    base64::Engine::decode(&base64::engine::general_purpose::STANDARD, payload).ok()
}

fn keep_alive_id() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(i64::MAX as u128) as i64)
        .unwrap_or_default()
}

fn chunk_coord(value: f64) -> i32 {
    (value.floor() as i32).div_euclid(16)
}

fn dimension_type_holder_id(dimension_type: &str) -> i32 {
    match dimension_type {
        "minecraft:overworld" => 1,
        "minecraft:overworld_caves" => 2,
        "minecraft:the_end" => 3,
        "minecraft:the_nether" => 4,
        _ => 1,
    }
}

fn entity_type_id(name: &str) -> Result<i32> {
    entity_type_registry()
        .get(name)
        .copied()
        .with_context(|| format!("missing entity type registry id: {name}"))
}

fn entity_type_registry() -> &'static HashMap<String, i32> {
    static REGISTRY: OnceLock<HashMap<String, i32>> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        load_registry_id_map("minecraft:entity_type").unwrap_or_else(|err| {
            log::warn!("failed to load entity type registry ids: {err:#}");
            HashMap::from([("minecraft:player".to_string(), 155)])
        })
    })
}

fn load_registry_id_map(registry_id: &str) -> Result<HashMap<String, i32>> {
    let path = workspace_root().join(REGISTRIES_REPORT);
    let content =
        std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    let value: serde_json::Value =
        serde_json::from_str(&content).with_context(|| format!("parse {}", path.display()))?;
    let entries = value
        .get(registry_id)
        .and_then(|registry| registry.get("entries"))
        .and_then(serde_json::Value::as_object)
        .with_context(|| format!("registry not found in {}: {registry_id}", path.display()))?;

    let mut ids = HashMap::new();
    for (name, value) in entries {
        let Some(id) = value
            .get("protocol_id")
            .and_then(serde_json::Value::as_i64)
            .and_then(|id| i32::try_from(id).ok())
        else {
            continue;
        };
        ids.insert(name.clone(), id);
    }

    Ok(ids)
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

#[cfg(test)]
mod tests {
    use super::{chunk_coord, dimension_type_holder_id, entity_type_id, keep_alive_id};

    #[test]
    fn chunk_coord_uses_floor_division() {
        assert_eq!(chunk_coord(0.0), 0);
        assert_eq!(chunk_coord(15.9), 0);
        assert_eq!(chunk_coord(16.0), 1);
        assert_eq!(chunk_coord(-0.1), -1);
        assert_eq!(chunk_coord(-16.0), -1);
    }

    #[test]
    fn dimension_type_holder_id_matches_registry_order_plus_one() {
        assert_eq!(dimension_type_holder_id("minecraft:overworld"), 1);
        assert_eq!(dimension_type_holder_id("minecraft:the_nether"), 4);
    }

    #[test]
    fn keep_alive_id_is_non_negative() {
        assert!(keep_alive_id() >= 0);
    }

    #[test]
    fn player_entity_type_id_is_loaded_from_current_report() {
        assert_eq!(entity_type_id("minecraft:player").unwrap(), 155);
    }
}

use anyhow::{Context, Result};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    path::{Path, PathBuf},
    sync::OnceLock,
    time::{Duration, Instant},
};

use qexed_config::app::qexed::server::{GameMode, Spawn};
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
    pick_item_from_block::PickItemFromBlock, set_carried_item::SetCarriedItem,
    set_creative_mode_slot::SetCreativeModeSlot, use_item::UseItem, use_item_on::UseItemOn,
};

use crate::player_data::{PlayerData, PlayerDataManager};
use crate::players::{OnlinePlayer, PlayerEvent, PlayerManager, PlayerSession};
use crate::world::WorldManager;

const TELEPORT_ID: i32 = 1;
const KEEP_ALIVE_INTERVAL: std::time::Duration = std::time::Duration::from_secs(15);
const CHUNK_UNLOAD_DELAY: Duration = Duration::from_secs(4);
const CHUNK_UNLOAD_SWEEP_INTERVAL: Duration = Duration::from_millis(500);
const DEFAULT_CHUNK_LOAD_PARALLELISM: usize = 4;
const MAX_CHUNK_LOAD_PARALLELISM: usize = 64;
const SLOW_CHUNK_PAYLOAD_LOG_THRESHOLD: Duration = Duration::from_millis(250);
const REGISTRIES_REPORT: &str = "assets/reports/registries.json";

pub async fn initialize<R, W>(
    packets: &mut qexed_tcp_connect::PacketStream<R>,
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    config: &qexed_config::app::qexed::Qexed,
    authenticator: &crate::auth::Authenticator,
    world: &WorldManager,
    players: &PlayerManager,
    player_data: &PlayerDataManager,
    permissions: &crate::permissions::PermissionManager,
    plugins: &crate::plugins::PluginManager,
    content_filter: &crate::content_filter::ContentFilter,
    profile: &qexed_packet::net_types::GameProfile,
) -> Result<()>
where
    R: tokio::io::AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
{
    let world_config = &config.server.world;
    let mut saved_player = player_data
        .load_or_default(profile, &world_config.dimension, &world_config.spawn)
        .await;
    let play_dimension = if saved_player.dimension.is_empty() {
        world_config.dimension.clone()
    } else {
        saved_player.dimension.clone()
    };
    let player_position = saved_player.entity_position();
    let view_distance = world_config.view_distance.max(1);
    let chunk_load_parallelism = chunk_load_parallelism_limit(world_config.chunk_load_parallelism);
    let simulation_distance = world_config.simulation_distance.max(1);
    let spawn_chunk_x = chunk_coord(player_position.x);
    let spawn_chunk_z = chunk_coord(player_position.z);
    let inventory = saved_player.inventory();
    let session = players.join(
        profile.clone(),
        player_position,
        inventory.visible_equipment(),
    );
    let world_session = world.begin_session();
    plugins.emit_player_join(&session.player);
    let leave_guard = PlayerLeaveGuard::new(players, plugins, session.player.clone());
    let player_entity_type = entity_type_id("minecraft:player")?;

    log::debug!(
        "初始化 Play 态: dimension={}, spawn=({}, {}, {}), yaw={}, pitch={}",
        play_dimension,
        player_position.x,
        player_position.y,
        player_position.z,
        player_position.yaw,
        player_position.pitch
    );

    sink.send(Login {
        entity_id: session.player.entity_id,
        is_hardcore: false,
        dimension_names: vec![play_dimension.clone()],
        max_player: VarInt(config.server.max_player.max(0)),
        view_distance: VarInt(view_distance),
        simulation_distance: VarInt(simulation_distance),
        reduced_debug_info: false,
        enable_respawn_screen: true,
        do_limited_crafting: false,
        dimension_type: VarInt(dimension_type_holder_id(&world_config.dimension_type)),
        dimension_name: play_dimension.clone(),
        hashed_seed: 0,
        game_mode: world_config.game_mode.protocol_id(),
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

    send_initial_player_state(
        sink,
        config,
        world_config,
        &play_dimension,
        &session.player,
        &inventory,
        permissions,
    )
    .await?;
    send_existing_players(sink, players, profile.uuid, player_entity_type).await?;

    sink.send(Position {
        teleport_id: VarInt(TELEPORT_ID),
        x: player_position.x,
        y: player_position.y,
        z: player_position.z,
        dx: 0.0,
        dy: 0.0,
        dz: 0.0,
        yaw: player_position.yaw,
        pitch: player_position.pitch,
        flags: 0,
    })
    .await?;

    sink.send(SystemChat {
        content: text_component("Qexed: 正在载入空世界"),
        overlay: false,
    })
    .await?;

    let chunk_state = ChunkSendState::new(
        play_dimension.clone(),
        spawn_chunk_x,
        spawn_chunk_z,
        view_distance,
        chunk_load_parallelism,
    );
    sink.flush().await?;

    let result = wait_for_play_packets(
        packets,
        sink,
        config,
        authenticator,
        world,
        players,
        player_data,
        permissions,
        plugins,
        content_filter,
        session,
        player_entity_type,
        profile,
        &mut saved_player,
        play_dimension,
        chunk_state,
        inventory,
        world_session,
    )
    .await;
    leave_guard.leave();
    result
}

async fn send_initial_player_state<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    config: &qexed_config::app::qexed::Qexed,
    world_config: &qexed_config::app::qexed::server::World,
    play_dimension: &str,
    player: &OnlinePlayer,
    inventory: &crate::inventory::PlayerInventory,
    permissions: &crate::permissions::PermissionManager,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let view_distance = world_config.view_distance.max(1);
    let simulation_distance = world_config.simulation_distance.max(1);
    sink.send(ChangeDifficulty {
        difficulty: 2,
        locked: false,
    })
    .await?;

    sink.send(PlayerAbilities {
        flags: player_ability_flags(world_config.game_mode),
        flying_speed: 0.05,
        walking_speed: 0.1,
    })
    .await?;

    sink.send(SetHeldSlot {
        slot: VarInt(inventory.selected_slot() as i32),
    })
    .await?;
    for packet in inventory.set_player_inventory_packets() {
        sink.send(packet).await?;
    }
    sink.send(crate::inventory::equipment_packet(
        player.entity_id,
        inventory.visible_equipment(),
    ))
    .await?;
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
        entries: vec![PlayerInfoEntry::from_profile(
            &player.profile,
            world_config.game_mode.protocol_id() as i32,
        )],
    })
    .await?;

    let visible_commands = crate::commands::visible_commands(permissions, &player.profile).await?;
    let command_tree = if visible_commands.as_slice() == ["help", "list"] {
        crate::commands::command_tree()
    } else {
        crate::commands::command_tree_for(&visible_commands)
    };
    sink.send(command_tree).await?;

    sink.send(InitializeBorder::default()).await?;
    sink.send(SetTime {
        game_time: 0,
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

    sink.send(GameStateChange {
        reason: 13,
        game_mode: world_config.game_mode.protocol_id() as f32,
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
        chunk_x: VarInt(chunk_coord(player.position.x)),
        chunk_z: VarInt(chunk_coord(player.position.z)),
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
    load_parallelism: usize,
    visible_chunks: HashSet<(i32, i32)>,
    pending_chunks: VecDeque<(i32, i32)>,
    pending_unloads: HashMap<(i32, i32), Instant>,
    loading_chunks: HashSet<(i32, i32)>,
}

struct ChunkLoadResult {
    chunk_x: i32,
    chunk_z: i32,
    payload: Result<bytes::Bytes>,
}

impl ChunkSendState {
    fn new(
        dimension: String,
        center_x: i32,
        center_z: i32,
        view_distance: i32,
        load_parallelism: usize,
    ) -> Self {
        Self {
            dimension,
            center_x,
            center_z,
            view_distance: view_distance.max(1),
            load_parallelism: chunk_load_parallelism_limit(load_parallelism),
            visible_chunks: HashSet::new(),
            pending_chunks: VecDeque::new(),
            pending_unloads: HashMap::new(),
            loading_chunks: HashSet::new(),
        }
    }

    async fn update_center<W>(
        &mut self,
        sink: &mut qexed_tcp_connect::PacketSink<W>,
        chunk_sender: &tokio::sync::mpsc::UnboundedSender<ChunkLoadResult>,
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

        let next = visible_chunk_set(chunk_x, chunk_z, self.view_distance);
        sink.send(UpdateViewPosition {
            chunk_x: VarInt(chunk_x),
            chunk_z: VarInt(chunk_z),
        })
        .await?;

        self.center_x = chunk_x;
        self.center_z = chunk_z;
        self.mark_delayed_unloads(next, Instant::now() + CHUNK_UNLOAD_DELAY);
        self.refresh_pending_chunks();
        self.start_next_chunk_load(world, Some(chunk_sender));
        sink.flush().await?;
        log::debug!("玩家移动到新区块，已补发视距区块: center=({chunk_x}, {chunk_z})");
        Ok(())
    }

    async fn send_missing_chunks<W>(
        &mut self,
        sink: &mut qexed_tcp_connect::PacketSink<W>,
        world: &WorldManager,
        cache_epoch: u64,
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
            let world_for_task = world.clone();
            let dimension = self.dimension.clone();
            let chunk_x = *chunk_x;
            let chunk_z = *chunk_z;
            let chunk_payload = tokio::task::spawn_blocking(move || {
                build_chunk_payload_sync(world_for_task, dimension, chunk_x, chunk_z, cache_epoch)
            })
            .await
            .context("join chunk packet build task")??;
            sink.send_raw(chunk_payload).await?;
            for update in world.placed_block_updates(&self.dimension, chunk_x, chunk_z) {
                sink.send(update).await?;
            }
            self.visible_chunks.insert((chunk_x, chunk_z));
        }
        sink.send(ChunkBatchFinished {
            batch_size: VarInt(chunks.len() as i32),
        })
        .await?;
        Ok(chunks.len())
    }

    fn missing_chunks(&self) -> Vec<(i32, i32)> {
        let mut chunks = self
            .target_chunks()
            .into_iter()
            .filter(|chunk| {
                !self.visible_chunks.contains(chunk) && !self.loading_chunks.contains(chunk)
            })
            .collect::<Vec<_>>();
        chunks.sort_by_key(|(chunk_x, chunk_z)| {
            let dx = *chunk_x - self.center_x;
            let dz = *chunk_z - self.center_z;
            (
                dx * dx + dz * dz,
                dx.abs().max(dz.abs()),
                *chunk_z,
                *chunk_x,
            )
        });
        chunks
    }

    fn target_chunks(&self) -> HashSet<(i32, i32)> {
        visible_chunk_set(self.center_x, self.center_z, self.view_distance)
    }

    fn refresh_pending_chunks(&mut self) {
        self.pending_chunks = self.missing_chunks().into();
    }

    fn mark_delayed_unloads(&mut self, target_chunks: HashSet<(i32, i32)>, unload_at: Instant) {
        self.pending_unloads.retain(|chunk, _| {
            self.visible_chunks.contains(chunk) && !target_chunks.contains(chunk)
        });

        for chunk in &self.visible_chunks {
            if target_chunks.contains(chunk) {
                self.pending_unloads.remove(chunk);
            } else {
                self.pending_unloads.entry(*chunk).or_insert(unload_at);
            }
        }
    }

    async fn unload_expired_chunks<W>(
        &mut self,
        sink: &mut qexed_tcp_connect::PacketSink<W>,
        plugins: &crate::plugins::PluginManager,
        now: Instant,
    ) -> Result<usize>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        if self.pending_unloads.is_empty() {
            return Ok(0);
        }

        let target_chunks = self.target_chunks();
        let mut expired = self
            .pending_unloads
            .iter()
            .filter_map(|(chunk, unload_at)| {
                (*unload_at <= now
                    && self.visible_chunks.contains(chunk)
                    && !target_chunks.contains(chunk))
                .then_some(*chunk)
            })
            .collect::<Vec<_>>();

        expired.sort_unstable();
        for (chunk_x, chunk_z) in &expired {
            self.pending_unloads.remove(&(*chunk_x, *chunk_z));
            if self.visible_chunks.remove(&(*chunk_x, *chunk_z)) {
                sink.send(ForgetLevelChunk {
                    chunk_x: *chunk_x,
                    chunk_z: *chunk_z,
                })
                .await?;
                plugins.emit_chunk_unload(&self.dimension, *chunk_x, *chunk_z);
            }
        }

        if !expired.is_empty() {
            sink.flush().await?;
        }
        Ok(expired.len())
    }

    fn start_next_chunk_load(
        &mut self,
        world: &WorldManager,
        sender: Option<&tokio::sync::mpsc::UnboundedSender<ChunkLoadResult>>,
    ) {
        let Some(sender) = sender else {
            return;
        };

        while self.loading_chunks.len() < self.load_parallelism {
            let Some((chunk_x, chunk_z)) = self.pending_chunks.pop_front() else {
                break;
            };
            let chunk = (chunk_x, chunk_z);
            if self.visible_chunks.contains(&chunk) || self.loading_chunks.contains(&chunk) {
                continue;
            }

            let world = world.clone();
            let dimension = self.dimension.clone();
            let sender = sender.clone();
            let cache_epoch = world.cache_epoch();
            tokio::task::spawn_blocking(move || {
                let payload =
                    build_chunk_payload_sync(world, dimension, chunk_x, chunk_z, cache_epoch);
                let _ = sender.send(ChunkLoadResult {
                    chunk_x,
                    chunk_z,
                    payload,
                });
            });
            self.loading_chunks.insert(chunk);
        }
    }

    fn has_chunk_work(&self) -> bool {
        !self.loading_chunks.is_empty() || !self.pending_chunks.is_empty()
    }

    fn has_pending_unloads(&self) -> bool {
        !self.pending_unloads.is_empty()
    }

    async fn send_loaded_chunk<W>(
        &mut self,
        sink: &mut qexed_tcp_connect::PacketSink<W>,
        world: &WorldManager,
        plugins: &crate::plugins::PluginManager,
        loaded: ChunkLoadResult,
        sender: &tokio::sync::mpsc::UnboundedSender<ChunkLoadResult>,
    ) -> Result<()>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        let chunk_x = loaded.chunk_x;
        let chunk_z = loaded.chunk_z;
        self.loading_chunks.remove(&(chunk_x, chunk_z));

        if self.visible_chunks.contains(&(chunk_x, chunk_z))
            || !visible_chunks(self.center_x, self.center_z, self.view_distance)
                .contains(&(chunk_x, chunk_z))
        {
            self.start_next_chunk_load(world, Some(sender));
            return Ok(());
        }

        let chunk_payload = loaded.payload?;

        sink.send(ChunkBatchStart {}).await?;
        sink.send_raw(chunk_payload).await?;
        for update in world.placed_block_updates(&self.dimension, chunk_x, chunk_z) {
            sink.send(update).await?;
        }
        self.visible_chunks.insert((chunk_x, chunk_z));
        plugins.emit_chunk_load(&self.dimension, chunk_x, chunk_z);
        sink.send(ChunkBatchFinished {
            batch_size: VarInt(1),
        })
        .await?;
        sink.flush().await?;

        self.start_next_chunk_load(world, Some(sender));
        Ok(())
    }
}

fn build_chunk_payload_sync(
    world: WorldManager,
    dimension: String,
    chunk_x: i32,
    chunk_z: i32,
    cache_epoch: u64,
) -> Result<bytes::Bytes> {
    let total_start = Instant::now();
    let chunk_start = Instant::now();
    let chunk = world.network_chunk_for_session(&dimension, chunk_x, chunk_z, cache_epoch)?;
    let chunk_elapsed = chunk_start.elapsed();

    let encode_start = Instant::now();
    let payload = qexed_tcp_connect::PacketSink::<tokio::io::Sink>::build_send_packet(chunk)
        .context("encode chunk packet")?;
    let encode_elapsed = encode_start.elapsed();
    let total_elapsed = total_start.elapsed();

    if total_elapsed >= SLOW_CHUNK_PAYLOAD_LOG_THRESHOLD && log::log_enabled!(log::Level::Debug) {
        log::debug!(
            "区块 payload 构建耗时: dimension={dimension}, chunk=({chunk_x}, {chunk_z}), total_ms={:.2}, world_ms={:.2}, encode_ms={:.2}, bytes={}",
            duration_ms(total_elapsed),
            duration_ms(chunk_elapsed),
            duration_ms(encode_elapsed),
            payload.len()
        );
    }

    Ok(payload)
}

fn duration_ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

fn chunk_load_parallelism_limit(value: usize) -> usize {
    let value = if value == 0 {
        DEFAULT_CHUNK_LOAD_PARALLELISM
    } else {
        value
    };
    value.clamp(1, MAX_CHUNK_LOAD_PARALLELISM)
}

fn player_ability_flags(game_mode: GameMode) -> u8 {
    match game_mode {
        GameMode::Survival | GameMode::Adventure => 0,
        GameMode::Creative => PlayerAbilities::CAN_FLY | PlayerAbilities::INSTABUILD,
        GameMode::Spectator => {
            PlayerAbilities::INVULNERABLE | PlayerAbilities::FLYING | PlayerAbilities::CAN_FLY
        }
    }
}

fn can_modify_world(
    world_config: &qexed_config::app::qexed::server::World,
    position: &qexed_packet::net_types::Position,
) -> bool {
    matches!(
        world_config.game_mode,
        GameMode::Survival | GameMode::Creative
    ) && !world_config.read_only
        && !is_spawn_protected(
            &world_config.spawn,
            world_config.spawn_protection_radius,
            position,
        )
}

fn is_spawn_protected(
    spawn: &Spawn,
    radius: i32,
    position: &qexed_packet::net_types::Position,
) -> bool {
    let radius = radius.max(0);
    radius > 0
        && (position.x - spawn.x.floor() as i32).abs() <= radius
        && (position.z - spawn.z.floor() as i32).abs() <= radius
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
    let mut chunk_state = ChunkSendState::new(dimension.to_string(), center_x, center_z, 1, 1);
    chunk_state
        .send_missing_chunks(sink, world, world.cache_epoch())
        .await?;
    Ok(())
}

async fn wait_for_play_packets<R, W>(
    packets: &mut qexed_tcp_connect::PacketStream<R>,
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    config: &qexed_config::app::qexed::Qexed,
    authenticator: &crate::auth::Authenticator,
    world: &WorldManager,
    players: &PlayerManager,
    player_data: &PlayerDataManager,
    permissions: &crate::permissions::PermissionManager,
    plugins: &crate::plugins::PluginManager,
    content_filter: &crate::content_filter::ContentFilter,
    mut session: PlayerSession,
    player_entity_type: i32,
    profile: &qexed_packet::net_types::GameProfile,
    saved_player: &mut PlayerData,
    play_dimension: String,
    mut chunk_state: ChunkSendState,
    mut inventory: crate::inventory::PlayerInventory,
    _world_session: crate::world::WorldSession,
) -> Result<()>
where
    R: tokio::io::AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
{
    let mut keep_alive = tokio::time::interval(KEEP_ALIVE_INTERVAL);
    keep_alive.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    keep_alive.tick().await;
    let mut chunk_unload_sweep = tokio::time::interval(CHUNK_UNLOAD_SWEEP_INTERVAL);
    chunk_unload_sweep.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    chunk_unload_sweep.tick().await;
    let mut pending_keep_alive = None;
    let mut chat_session: Option<crate::secure_chat::SecureChatSession> = None;
    let mut next_chat_global_index = 0;
    let enforce_secure_chat = config.server.online_mode;
    let world_config = &config.server.world;
    let mut position = session.player.position;
    let (chunk_sender, mut chunk_receiver) = tokio::sync::mpsc::unbounded_channel();
    chunk_state.refresh_pending_chunks();
    chunk_state.start_next_chunk_load(world, Some(&chunk_sender));

    let result: Result<()> = async {
        loop {
        tokio::select! {
            loaded_chunk = chunk_receiver.recv(), if chunk_state.has_chunk_work() => {
                let Some(loaded_chunk) = loaded_chunk else {
                    anyhow::bail!("区块加载任务通道已关闭");
                };
                chunk_state
                    .send_loaded_chunk(sink, world, plugins, loaded_chunk, &chunk_sender)
                    .await?;
            }
            _ = chunk_unload_sweep.tick(), if chunk_state.has_pending_unloads() => {
                let unloaded = chunk_state
                    .unload_expired_chunks(sink, plugins, Instant::now())
                    .await?;
                if unloaded > 0 {
                    log::debug!("延迟卸载区块完成: count={unloaded}");
                }
            }
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
                if let Some(message) = player_event_message(config, &event) {
                    sink.send(SystemChat {
                        content: text_component(message),
                        overlay: false,
                    })
                    .await?;
                }
                sink.flush().await?;
            }
            packet = packets.read_packet() => {
                let Some(mut payload) = packet? else {
                    break Ok(());
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
                        "客户端已确认区块批次，建议区块发送速率: {} chunks/tick",
                        batch.desired_chunks_per_tick
                    );
                    continue;
                }

                if packet_id == SetCarriedItem::ID {
                    let carried = crate::connection::decode_payload::<SetCarriedItem>(&mut payload)?;
                    if let Some(main_hand) = inventory.set_selected(carried.slot) {
                        sink.send(SetHeldSlot {
                            slot: VarInt(carried.slot as i32),
                        })
                        .await?;
                        players.update_equipment(
                            profile.uuid,
                            vec![qexed_protocol::to_client::play::set_equipment::Equipment::mainhand(
                                main_hand,
                            )],
                        );
                        sink.flush().await?;
                    } else {
                        log::warn!("瀹㈡埛绔皾璇曢€夋嫨鏃犳晥鐑爮: {}", carried.slot);
                    }
                    continue;
                }

                if packet_id == SetCreativeModeSlot::ID {
                    let slot = crate::connection::decode_payload::<SetCreativeModeSlot>(&mut payload)?;
                    if world_config.game_mode != GameMode::Creative {
                        log::debug!(
                            "ignored creative slot update outside creative mode: uuid={}",
                            profile.uuid
                        );
                        continue;
                    }
                    if let Some(change) = inventory.set_creative_slot(slot.slot_num, slot.item_stack.clone()) {
                        match change {
                            crate::inventory::InventorySlotChange::Hotbar { slot: inventory_slot, item } => {
                                sink.send(crate::inventory::set_player_inventory_packet(
                                    inventory_slot,
                                    item.clone(),
                                )).await?;
                                if inventory_slot == inventory.selected_slot() {
                                    players.update_equipment(
                                        profile.uuid,
                                        vec![qexed_protocol::to_client::play::set_equipment::Equipment::mainhand(item)],
                                    );
                                }
                            }
                            crate::inventory::InventorySlotChange::Equipment { slot, item } => {
                                players.update_equipment(
                                    profile.uuid,
                                    vec![qexed_protocol::to_client::play::set_equipment::Equipment::new(slot, item)],
                                );
                            }
                        }
                        sink.flush().await?;
                    }
                    continue;
                }

                if packet_id == PickItemFromBlock::ID {
                    let pick = crate::connection::decode_payload::<PickItemFromBlock>(&mut payload)?;
                    let item_id = world
                        .block_state_at(&play_dimension, &pick.position)
                        .and_then(|block_state| (block_state == crate::inventory::STONE_BLOCK_STATE_ID).then_some(1))
                        .unwrap_or(1);
                    let slot = inventory.pick_block(item_id);
                    let held = inventory.held_item().clone();
                    sink.send(crate::inventory::set_player_inventory_packet(slot, held.clone())).await?;
                    sink.send(SetHeldSlot { slot: VarInt(slot as i32) }).await?;
                    players.update_equipment(
                        profile.uuid,
                        vec![qexed_protocol::to_client::play::set_equipment::Equipment::mainhand(held)],
                    );
                    sink.flush().await?;
                    continue;
                }

                if packet_id == UseItemOn::ID {
                    let use_item_on = crate::connection::decode_payload::<UseItemOn>(&mut payload)?;
                    sink.send(crate::inventory::acknowledge_block_change(use_item_on.sequence).packet()).await?;
                    if use_item_on.hand.0 == 0 {
                        if let Some(block_state) = crate::inventory::placed_block_state_for_item(inventory.held_item()) {
                            let placed = crate::inventory::placement_position(
                                &use_item_on.block_hit.position,
                                use_item_on.block_hit.face.0,
                            );
                            if !can_modify_world(world_config, &placed) {
                                log::debug!(
                                    "blocked world edit: read_only={}, spawn_protection_radius={}, position=({}, {}, {})",
                                    world_config.read_only,
                                    world_config.spawn_protection_radius,
                                    placed.x,
                                    placed.y,
                                    placed.z
                                );
                                sink.send(crate::inventory::block_update(
                                    placed.clone(),
                                    world
                                        .block_state_at(&play_dimension, &placed)
                                        .unwrap_or_default(),
                                )).await?;
                                sink.flush().await?;
                                continue;
                            }
                            world.place_block(&play_dimension, placed.clone(), block_state);
                            sink.send(crate::inventory::block_update(placed.clone(), block_state)).await?;
                            let light_update = if world.dynamic_light_enabled() {
                                let update = world.light_update(
                                    &play_dimension,
                                    placed.x.div_euclid(16),
                                    placed.z.div_euclid(16),
                                );
                                sink.send(update.clone()).await?;
                                Some(qexed_tcp_connect::PacketSink::<tokio::io::Sink>::build_send_packet(update)?)
                            } else {
                                None
                            };
                            players.broadcast_block_changed(profile.uuid, placed, block_state, light_update);
                        }
                    }
                    sink.flush().await?;
                    continue;
                }

                if packet_id == UseItem::ID {
                    let use_item = crate::connection::decode_payload::<UseItem>(&mut payload)?;
                    sink.send(crate::inventory::acknowledge_block_change(use_item.sequence).packet()).await?;
                    sink.flush().await?;
                    continue;
                }

                if packet_id == MovePlayerPos::ID {
                    let movement = crate::connection::decode_payload::<MovePlayerPos>(&mut payload)?;
                    position.x = movement.x;
                    position.y = movement.y;
                    position.z = movement.z;
                    position.on_ground = movement.flags & 0x01 != 0;
                    chunk_state
                        .update_center(sink, &chunk_sender, world, movement.x, movement.z)
                        .await?;
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
                    chunk_state
                        .update_center(sink, &chunk_sender, world, movement.x, movement.z)
                        .await?;
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
                    let filtered_message = match content_filter.check_chat(&chat.message).await? {
                        crate::content_filter::FilterAction::Allow(message) => message,
                        crate::content_filter::FilterAction::Block { reason } => {
                            let reason = if reason.trim().is_empty() {
                                content_filter.block_message()
                            } else {
                                &reason
                            };
                            sink.send(SystemChat {
                                content: text_component(reason),
                                overlay: false,
                            })
                            .await?;
                            sink.flush().await?;
                            continue;
                        }
                    };
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
                            filtered_message,
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
                            content: text_component(format!("<{}> {}", profile.username, filtered_message)),
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
                    handle_chat_command(sink, config, players, permissions, profile, &command.command).await?;
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
    .await;

    saved_player.update_runtime(&play_dimension, position, &inventory);
    if let Err(err) = player_data.save(saved_player).await {
        log::warn!("保存玩家存档失败: uuid={}, error={err:#}", profile.uuid);
    }

    result
}

async fn handle_chat_command<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    config: &qexed_config::app::qexed::Qexed,
    players: &PlayerManager,
    permissions: &crate::permissions::PermissionManager,
    profile: &qexed_packet::net_types::GameProfile,
    command: &str,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let command = command.trim();
    if !permissions.can_run_command(profile, command).await? {
        sink.send(SystemChat {
            content: text_component(permissions.denied_message()),
            overlay: false,
        })
        .await?;
        return Ok(());
    }

    let messages = crate::commands::messages();
    let content = match command {
        "help" => messages.render_help(),
        "list" => {
            let mut names = players.online_names();
            names.sort();
            messages.render_list(players.online_count(), config.server.max_player, &names)
        }
        _ => messages.render_unknown(command),
    };

    sink.send(SystemChat {
        content,
        overlay: false,
    })
    .await?;
    Ok(())
}

fn event_is_self(event: &PlayerEvent, profile_id: uuid::Uuid) -> bool {
    match event {
        PlayerEvent::Joined(player) => player.profile.uuid == profile_id,
        PlayerEvent::Left {
            profile_id: left_id,
            entity_id: _,
            username: _,
        } => *left_id == profile_id,
        PlayerEvent::Moved {
            profile_id: moved_id,
            entity_id: _,
            position: _,
        } => *moved_id == profile_id,
        PlayerEvent::EquipmentChanged {
            profile_id: changed_id,
            entity_id: _,
            slots: _,
        } => *changed_id == profile_id,
        PlayerEvent::BlockChanged {
            profile_id: changed_id,
            position: _,
            block_state: _,
            light_update: _,
        } => *changed_id == profile_id,
    }
}

fn player_event_message(
    config: &qexed_config::app::qexed::Qexed,
    event: &PlayerEvent,
) -> Option<String> {
    let messages = &config.server.player_messages;
    if !messages.enable {
        return None;
    }

    match event {
        PlayerEvent::Joined(player) => Some(render_player_message(
            &messages.join,
            &player.profile.username,
        )),
        PlayerEvent::Left { username, .. } => {
            Some(render_player_message(&messages.leave, username))
        }
        _ => None,
    }
}

fn render_player_message(template: &str, username: &str) -> String {
    template.replace("{player}", username)
}

struct PlayerLeaveGuard<'a> {
    players: &'a PlayerManager,
    plugins: &'a crate::plugins::PluginManager,
    player: OnlinePlayer,
    active: bool,
}

impl<'a> PlayerLeaveGuard<'a> {
    fn new(
        players: &'a PlayerManager,
        plugins: &'a crate::plugins::PluginManager,
        player: OnlinePlayer,
    ) -> Self {
        Self {
            players,
            plugins,
            player,
            active: true,
        }
    }

    fn leave(mut self) {
        self.leave_inner();
        self.active = false;
    }

    fn leave_inner(&self) {
        self.plugins.emit_player_leave(&self.player);
        self.players.leave(self.player.profile.uuid);
    }
}

impl Drop for PlayerLeaveGuard<'_> {
    fn drop(&mut self) {
        if self.active {
            self.leave_inner();
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
    use super::{
        ChunkSendState, can_modify_world, chunk_coord, chunk_load_parallelism_limit,
        dimension_type_holder_id, entity_type_id, keep_alive_id, player_ability_flags,
    };
    use qexed_config::app::qexed::server::GameMode;
    use qexed_protocol::to_client::play::player_abilities::PlayerAbilities;
    use std::time::{Duration, Instant};

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
    fn chunk_load_parallelism_is_bounded() {
        assert_eq!(
            chunk_load_parallelism_limit(0),
            super::DEFAULT_CHUNK_LOAD_PARALLELISM
        );
        assert_eq!(chunk_load_parallelism_limit(1), 1);
        assert_eq!(
            chunk_load_parallelism_limit(usize::MAX),
            super::MAX_CHUNK_LOAD_PARALLELISM
        );
    }

    #[test]
    fn player_abilities_follow_game_mode() {
        assert_eq!(player_ability_flags(GameMode::Survival), 0);
        assert_eq!(
            player_ability_flags(GameMode::Creative),
            PlayerAbilities::CAN_FLY | PlayerAbilities::INSTABUILD
        );
        assert_eq!(
            player_ability_flags(GameMode::Spectator),
            PlayerAbilities::INVULNERABLE | PlayerAbilities::FLYING | PlayerAbilities::CAN_FLY
        );
    }

    #[test]
    fn world_edit_rules_apply_read_only_and_spawn_protection() {
        let mut world = qexed_config::app::qexed::server::World::default();
        let spawn = qexed_packet::net_types::Position { x: 1, y: 64, z: 1 };
        let outside_spawn = qexed_packet::net_types::Position {
            x: 100,
            y: 64,
            z: 100,
        };

        assert!(!can_modify_world(&world, &spawn));
        assert!(can_modify_world(&world, &outside_spawn));

        world.spawn_protection_radius = 0;
        assert!(can_modify_world(&world, &spawn));

        world.read_only = true;
        assert!(!can_modify_world(&world, &outside_spawn));

        world.read_only = false;
        world.game_mode = GameMode::Adventure;
        assert!(!can_modify_world(&world, &outside_spawn));

        world.game_mode = GameMode::Spectator;
        assert!(!can_modify_world(&world, &outside_spawn));
    }

    #[test]
    fn missing_chunks_skips_in_flight_chunks() {
        let mut state = ChunkSendState::new("minecraft:overworld".to_string(), 0, 0, 1, 4);
        state.loading_chunks.insert((0, 0));
        state.visible_chunks.insert((1, 0));

        let missing = state.missing_chunks();

        assert!(!missing.contains(&(0, 0)));
        assert!(!missing.contains(&(1, 0)));
        assert_eq!(missing.len(), 7);
    }

    #[test]
    fn delayed_unload_keeps_recently_left_chunks_visible() {
        let mut state = ChunkSendState::new("minecraft:overworld".to_string(), 0, 0, 1, 1);
        state.visible_chunks.insert((-1, 0));
        state.visible_chunks.insert((0, 0));

        state.center_x = 2;
        let target = state.target_chunks();
        state.mark_delayed_unloads(target, Instant::now() + Duration::from_secs(4));

        assert!(state.visible_chunks.contains(&(-1, 0)));
        assert!(state.pending_unloads.contains_key(&(-1, 0)));
    }

    #[test]
    fn delayed_unload_is_cancelled_when_chunk_returns_to_view() {
        let mut state = ChunkSendState::new("minecraft:overworld".to_string(), 0, 0, 1, 1);
        state.visible_chunks.insert((-1, 0));

        state.center_x = 2;
        state.mark_delayed_unloads(
            state.target_chunks(),
            Instant::now() + Duration::from_secs(4),
        );
        state.center_x = 0;
        state.mark_delayed_unloads(
            state.target_chunks(),
            Instant::now() + Duration::from_secs(4),
        );

        assert!(!state.pending_unloads.contains_key(&(-1, 0)));
        assert!(state.visible_chunks.contains(&(-1, 0)));
    }

    #[test]
    fn player_entity_type_id_is_loaded_from_current_report() {
        assert_eq!(entity_type_id("minecraft:player").unwrap(), 155);
    }
}

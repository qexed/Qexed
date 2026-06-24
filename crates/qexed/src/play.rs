mod bootstrap;
mod chat;
mod chunks;
mod drops;
mod events;
mod gameplay;
mod geyser;
mod lobby;
mod menus;
mod mining;
pub(crate) mod pathfinding;
mod recipes;
mod scoreboard;
mod session;
mod survival;
mod util;

use anyhow::{Context, Result};
use std::{
    collections::{BTreeMap, HashMap, HashSet, VecDeque},
    sync::{Mutex, mpsc},
    time::{Duration, Instant},
};

use qexed_config::app::qexed::server::GameMode;
use qexed_packet::{
    Packet,
    net_types::{Position as BlockPosition, VarInt, VarLong},
};
use qexed_protocol::to_client::play::{
    add_entity::EntityPosition,
    command_suggestions::{CommandSuggestions, Matches},
    container_set_slot,
    damage_event::{DamageEvent, DamageSourcePosition},
    game_state_change::GameStateChange,
    hurt_animation::HurtAnimation,
    keep_alive::KeepAlive as ClientboundKeepAlive,
    login::Login,
    player_abilities::PlayerAbilities as ClientboundPlayerAbilities,
    player_chat::{PackedMessageSignature, PlayerChat},
    player_info_update::{PlayerInfoActions, PlayerInfoEntry, PlayerInfoUpdate},
    position::Position,
    respawn::{KEEP_NO_DATA, Respawn},
    set_entity_motion::SetEntityMotion,
    set_held_slot::SetHeldSlot,
    set_time::SetTime,
    system_chat::SystemChat,
    take_item_entity::TakeItemEntity,
};
use qexed_protocol::to_server::play::{
    accept_teleportation::AcceptTeleportation, attack::Attack, chat_ack::ChatAck,
    chat_command::ChatCommand, chat_message::ChatMessage, chat_session_update::ChatSessionUpdate,
    chunk_batch_received::ChunkBatchReceived, client_command::ClientCommand,
    command_suggestion::CommandSuggestion, container_button_click::ContainerButtonClick,
    container_click::ContainerClick, container_close::ContainerClose,
    custom_payload::CustomPayload as ServerboundCustomPayload, interact::Interact,
    keep_alive::KeepAlive as ServerboundKeepAlive, move_player_pos::MovePlayerPos,
    move_player_pos_rot::MovePlayerPosRot, move_player_rot::MovePlayerRot,
    move_player_status_only::MovePlayerStatusOnly, pick_item_from_block::PickItemFromBlock,
    player_abilities::PlayerAbilities as ServerboundPlayerAbilities, player_action::PlayerAction,
    player_input::PlayerInput, player_loaded::PlayerLoaded, set_carried_item::SetCarriedItem,
    set_creative_mode_slot::SetCreativeModeSlot, use_item::UseItem, use_item_on::UseItemOn,
};

use crate::player_data::{PlayerData, PlayerDataLockGuard, PlayerDataManager};
use crate::players::{PlayerDamageKind, PlayerManager, PlayerSession};
use crate::world::WorldManager;

use bootstrap::{
    send_existing_entities, send_existing_players, send_initial_player_state,
    send_respawn_player_state,
};
use chat::handle_chat_command;
use chunks::{ChunkSendState, chunk_load_parallelism_limit};
#[cfg(test)]
use chunks::{
    DEFAULT_PARALLELISM_FOR_TESTS as DEFAULT_CHUNK_LOAD_PARALLELISM,
    MAX_PARALLELISM_FOR_TESTS as MAX_CHUNK_LOAD_PARALLELISM,
};
use events::{event_is_self, player_event_message};
use session::PlayerLeaveGuard;
use survival::{
    DeathMessage, FallContext, FallLanding, FallLocation, SurvivalState, spawn_position,
};
use util::{
    acknowledged_player_ability_flags, can_attempt_world_edit_for_game_mode,
    can_modify_world_for_game_mode, chunk_coord, dimension_type_holder_id, keep_alive_id,
    player_ability_flags, text_component, translatable_component,
};

const KEEP_ALIVE_INTERVAL: std::time::Duration = std::time::Duration::from_secs(15);
const CHUNK_SEND_TICK_INTERVAL: Duration = Duration::from_millis(50);
const CHUNK_UNLOAD_SWEEP_INTERVAL: Duration = Duration::from_millis(500);
const GAMEPLAY_TICK_INTERVAL: Duration = Duration::from_millis(50);
const SURVIVAL_TICK_INTERVAL: Duration = Duration::from_secs(1);
const WORLD_TIME_TICK_INTERVAL: Duration = Duration::from_secs(1);
const SIDEBAR_REFRESH_INTERVAL: Duration = Duration::from_secs(1);
const MIN_PLAYER_DATA_AUTOSAVE_INTERVAL: Duration = Duration::from_secs(10);
const MINING_EXHAUSTION_PER_BLOCK: f32 = 0.005;
const PLAYER_ACTION_START_DESTROY_BLOCK: i32 = 0;
const PLAYER_ACTION_CANCEL_DESTROY_BLOCK: i32 = 1;
const PLAYER_ACTION_STOP_DESTROY_BLOCK: i32 = 2;
const PLAYER_ACTION_DROP_ITEM_STACK: i32 = 3;
const PLAYER_ACTION_DROP_ITEM: i32 = 4;
const PLAYER_ACTION_RELEASE_USE_ITEM: i32 = 5;
const ADVENTURE_BREAK_CHEAT_BAN_REASON: &str = "开第三方客户端";

// ── Fluid system (removed) ──

type FluidSeed = (BlockPosition, i32);

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct FluidQueueEntry {
    dimension: String,
    x: i32,
    y: i32,
    z: i32,
}

#[derive(Clone, Debug, PartialEq)]
struct FluidBlockChange {
    dimension: String,
    position: BlockPosition,
    block_state: i32,
}

struct FluidTickJob;
struct FluidTickResult;

#[derive(Debug)]
pub(crate) struct FluidRuntime;

impl FluidRuntime {
    pub(crate) fn new() -> Self {
        Self
    }
    pub(crate) fn with_world_rules(_: crate::world::WorldRulesManager) -> Self {
        Self
    }
    pub(crate) fn enqueue_block_change(&self, _: &str, _: &BlockPosition) {}
    fn enqueue_fluid_seeds(&self, _: &str, _: impl IntoIterator<Item = FluidSeed>) {}
    fn enqueue_fluid_continuation(&self, _: &str, _: &BlockPosition, _: i32) {}
    fn try_start_tick_job(
        &self,
        _: WorldManager,
        _: &mut Option<FluidTickJob>,
        _: &tokio::sync::mpsc::UnboundedSender<()>,
    ) -> bool {
        false
    }
    fn poll_completed_tick_job(&self, _: &mut Option<FluidTickJob>) -> Vec<FluidBlockChange> {
        Vec::new()
    }
    #[cfg(test)]
    fn force_tick_due(&self) {}
    #[cfg(test)]
    fn force_current_tick(&self, _tick: u64) {}
}

impl Default for FluidRuntime {
    fn default() -> Self {
        Self
    }
}

fn compute_fluid_tick_changes(_: &WorldManager, _: Vec<FluidQueueEntry>) -> FluidTickResult {
    FluidTickResult
}
struct ChatRateLimit {
    window: Duration,
    max_messages: usize,
    max_length: usize,
    sent_at: VecDeque<Instant>,
}

impl ChatRateLimit {
    fn new(config: &qexed_config::app::qexed::server::PlayerMessages) -> Self {
        Self {
            window: Duration::from_secs(config.chat_rate_limit_window_secs.max(1)),
            max_messages: config.chat_rate_limit_max_messages.max(1) as usize,
            max_length: config.chat_max_length.max(1),
            sent_at: VecDeque::new(),
        }
    }

    fn check(&mut self, message: &str, now: Instant) -> ChatLimitResult {
        if message.chars().count() > self.max_length {
            return ChatLimitResult::TooLong {
                max_length: self.max_length,
            };
        }

        while self
            .sent_at
            .front()
            .is_some_and(|sent_at| now.duration_since(*sent_at) >= self.window)
        {
            self.sent_at.pop_front();
        }

        if self.sent_at.len() >= self.max_messages {
            return ChatLimitResult::RateLimited;
        }

        self.sent_at.push_back(now);
        ChatLimitResult::Allowed
    }
}

enum ChatLimitResult {
    Allowed,
    RateLimited,
    TooLong { max_length: usize },
}

fn validate_player_chat_message(rate_limit: &mut ChatRateLimit, message: &str) -> Option<String> {
    match rate_limit.check(message, Instant::now()) {
        ChatLimitResult::Allowed => None,
        ChatLimitResult::RateLimited => {
            Some("You are sending chat messages too quickly.".to_string())
        }
        ChatLimitResult::TooLong { max_length } => Some(format!(
            "Chat message is too long. Maximum length is {max_length}."
        )),
    }
}

fn effective_online_mode(server: &qexed_config::app::qexed::server::Server) -> bool {
    if server.proxy
        && matches!(
            server.proxy_protocol,
            qexed_config::app::qexed::server::ForwardingMode::Victory
        )
    {
        server.proxy_online_mode
    } else {
        server.online_mode
    }
}

fn player_data_autosave_interval(
    config: &qexed_config::app::qexed::server::PlayerData,
) -> Duration {
    Duration::from_secs(config.autosave_interval_secs).max(MIN_PLAYER_DATA_AUTOSAVE_INTERVAL)
}

fn pending_chunk_center_update_sleep(chunk_state: &ChunkSendState) -> tokio::time::Sleep {
    let deadline = chunk_state
        .pending_center_update_deadline()
        .unwrap_or_else(Instant::now);
    tokio::time::sleep_until(tokio::time::Instant::from_std(deadline))
}

async fn send_initial_cluster_entity_view_after_chunks<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    cluster_entities: Option<&crate::cluster_entities::ClusterEntityController>,
    sent: &mut bool,
    player: &mut crate::players::OnlinePlayer,
    dimension: &str,
    position: qexed_protocol::to_client::play::add_entity::EntityPosition,
    rendering: &qexed_config::app::qexed::server::EntityRendering,
    simulation_distance: i32,
    chunk_state: &ChunkSendState,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    if *sent || !chunk_state.initial_view_complete() {
        return Ok(());
    }
    *sent = true;
    let Some(cluster_entities) = cluster_entities else {
        return Ok(());
    };

    player.position = position;
    player.dimension = dimension.to_string();
    let packets = cluster_entities.spawn_view_for_player(player, rendering, simulation_distance);
    for packet in packets.iter().cloned() {
        sink.send_raw(packet).await?;
    }
    if !packets.is_empty() {
        log::debug!(
            "initial cluster entity view sent: player={}, packets={}",
            player.profile.username,
            packets.len()
        );
        sink.flush().await?;
    }
    Ok(())
}

async fn refresh_cluster_entity_view<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    cluster_entities: Option<&crate::cluster_entities::ClusterEntityController>,
    player: &crate::players::OnlinePlayer,
    rendering: &qexed_config::app::qexed::server::EntityRendering,
    simulation_distance: i32,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let Some(cluster_entities) = cluster_entities else {
        return Ok(());
    };
    let packets = cluster_entities.spawn_view_for_player(player, rendering, simulation_distance);
    for packet in packets.iter().cloned() {
        sink.send_raw(packet).await?;
    }
    if !packets.is_empty() {
        sink.flush().await?;
    }
    Ok(())
}

const PLAYER_HEIGHT_BLOCKS: f64 = 1.8;

pub async fn initialize<R, W>(
    packets: &mut qexed_tcp_connect::PacketStream<R>,
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    config: &crate::config::RuntimeConfig,
    authenticator: &crate::auth::Authenticator,
    world: &WorldManager,
    world_rules: &crate::world::WorldRulesManager,
    ore_pits: &crate::world::OrePitManager,
    cluster_entities: Option<&crate::cluster_entities::ClusterEntityController>,
    players: &PlayerManager,
    fluid: &FluidRuntime,
    entities: &crate::entities::EntityManager,
    player_data: &PlayerDataManager,
    permissions: &crate::permissions::PermissionManager,
    plugins: &crate::plugins::PluginManager,
    player_audit: &crate::audit::PlayerAuditLogger,
    content_filter: &crate::content_filter::ContentFilter,
    warden: &crate::warden::WardenManager,
    profile: &qexed_packet::net_types::GameProfile,
    client_language: Option<String>,
    displayed_skin_parts: u8,
    shutdown: tokio::sync::watch::Receiver<bool>,
) -> Result<()>
where
    R: tokio::io::AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
{
    let world_config = &config.world;
    let default_dimension = world_config.default_play_dimension();
    let player_data_lock = player_data
        .lock_player(profile.uuid)
        .await
        .with_context(|| {
            format!(
                "acquire player data lock for {} ({})",
                profile.username, profile.uuid
            )
        })?;
    let mut saved_player = player_data
        .locked_load_or_default(
            &player_data_lock,
            profile,
            &default_dimension,
            &world_config.spawn,
        )
        .await;
    let play_dimension = if saved_player.dimension.is_empty() {
        default_dimension
    } else {
        saved_player.dimension.clone()
    };
    world_rules.ensure_loaded(&play_dimension)?;
    let play_rule = world_rules.snapshot(&play_dimension);
    let view_distance = world_config.view_distance.max(1);
    let chunk_load_parallelism = chunk_load_parallelism_limit(world_config.chunk_load_parallelism);
    let simulation_distance = world_config.simulation_distance.max(1);
    let mut inventory = saved_player.inventory();
    let initial_lobby = lobby::LobbyRuntime::new(&config.server.lobby);
    let initial_menus = menus::MenuRuntime::new(&config.server.menus);
    if initial_menus.reset_inventory_on_join() {
        inventory = crate::inventory::PlayerInventory::empty();
    }
    let _ = lobby::sync_navigator_item(&mut inventory, &initial_lobby);
    let _ = initial_menus.sync_hotbar_items(&mut inventory);
    let stored_was_dead = saved_player.survival.health <= 0.0;
    let initial_survival =
        SurvivalState::from_stored(saved_player.survival, world_config.game_mode);
    saved_player.survival = initial_survival.to_stored();
    let player_position = if stored_was_dead {
        spawn_position(&world_config.spawn)
    } else {
        saved_player.entity_position()
    };
    let spawn_chunk_x = chunk_coord(player_position.x);
    let spawn_chunk_z = chunk_coord(player_position.z);
    let mut session = players.join_with_skin_parts(
        profile.clone(),
        player_position,
        play_dimension.clone(),
        inventory.visible_equipment(),
        client_language.unwrap_or_else(|| config.language.clone()),
        displayed_skin_parts,
        world_config.game_mode.protocol_id() as i32,
    );
    let world_session = world.begin_session();
    plugins.emit_player_join(&session.player);
    let leave_guard = PlayerLeaveGuard::new(players, plugins, session.player.clone());
    let player_entity_type = crate::entities::entity_type_id("minecraft:player")?;

    log::debug!(
        "initializing Play state: dimension={}, spawn=({}, {}, {}), yaw={}, pitch={}",
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
        dimension_names: login_dimension_names(world_config, &play_dimension),
        max_player: VarInt(config.server.max_player.max(0)),
        view_distance: VarInt(view_distance),
        simulation_distance: VarInt(simulation_distance),
        reduced_debug_info: false,
        enable_respawn_screen: true,
        do_limited_crafting: false,
        dimension_type: VarInt(dimension_type_holder_id(&play_rule.dimension_type)),
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
        enforces_secure_chat: effective_online_mode(&config.server),
    })
    .await?;

    let mut next_teleport_id = 1;
    sink.send(Position {
        teleport_id: VarInt(next_teleport_id),
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
    next_teleport_id += 1;

    send_initial_player_state(
        sink,
        config,
        world_config,
        world_rules,
        &play_dimension,
        &session.player,
        &inventory,
        saved_player.survival,
        permissions,
        plugins,
    )
    .await?;
    send_existing_players(
        sink,
        players,
        profile.uuid,
        player_entity_type,
        &play_dimension,
        player_position,
        config.server.entity_rendering.player_distance,
    )
    .await?;
    send_existing_entities(
        sink,
        entities,
        &play_dimension,
        player_position,
        &config.server.entity_rendering,
    )
    .await?;

    // sink.send(SystemChat {
    //     content: text_component("Qexed: loading world"),
    //     overlay: false,
    // })
    // .await?;

    let mut chunk_state = ChunkSendState::new(
        play_dimension.clone(),
        spawn_chunk_x,
        spawn_chunk_z,
        view_distance,
        chunk_load_parallelism,
    );
    chunk_state.set_center_update_delay(Duration::from_millis(world_config.chunk_update_delay_ms));
    sink.flush().await?;

    let result = wait_for_play_packets(
        packets,
        sink,
        config,
        authenticator,
        world,
        world_rules,
        ore_pits,
        cluster_entities,
        players,
        fluid,
        player_data,
        entities,
        permissions,
        plugins,
        player_audit,
        content_filter,
        warden,
        session,
        player_entity_type,
        profile,
        &mut saved_player,
        &player_data_lock,
        play_dimension,
        chunk_state,
        inventory,
        next_teleport_id,
        world_session,
        shutdown,
    )
    .await;
    leave_guard.leave();
    result
}

async fn wait_for_play_packets<R, W>(
    packets: &mut qexed_tcp_connect::PacketStream<R>,
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    config: &crate::config::RuntimeConfig,
    authenticator: &crate::auth::Authenticator,
    world: &WorldManager,
    world_rules: &crate::world::WorldRulesManager,
    ore_pits: &crate::world::OrePitManager,
    cluster_entities: Option<&crate::cluster_entities::ClusterEntityController>,
    players: &PlayerManager,
    fluid: &FluidRuntime,
    player_data: &PlayerDataManager,
    entities: &crate::entities::EntityManager,
    permissions: &crate::permissions::PermissionManager,
    plugins: &crate::plugins::PluginManager,
    player_audit: &crate::audit::PlayerAuditLogger,
    content_filter: &crate::content_filter::ContentFilter,
    warden: &crate::warden::WardenManager,
    mut session: PlayerSession,
    player_entity_type: i32,
    profile: &qexed_packet::net_types::GameProfile,
    saved_player: &mut PlayerData,
    player_data_lock: &PlayerDataLockGuard,
    mut play_dimension: String,
    mut chunk_state: ChunkSendState,
    mut inventory: crate::inventory::PlayerInventory,
    mut next_teleport_id: i32,
    _world_session: crate::world::WorldSession,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
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
    let mut chunk_send_tick = tokio::time::interval(CHUNK_SEND_TICK_INTERVAL);
    chunk_send_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    chunk_send_tick.tick().await;
    let mut gameplay_tick = tokio::time::interval(GAMEPLAY_TICK_INTERVAL);
    gameplay_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    gameplay_tick.tick().await;
    let mut survival_tick = tokio::time::interval(SURVIVAL_TICK_INTERVAL);
    survival_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    survival_tick.tick().await;
    let mut world_time_tick = tokio::time::interval(WORLD_TIME_TICK_INTERVAL);
    world_time_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    world_time_tick.tick().await;
    let mut sidebar_refresh_tick = tokio::time::interval(SIDEBAR_REFRESH_INTERVAL);
    sidebar_refresh_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    sidebar_refresh_tick.tick().await;
    let mut player_data_autosave =
        tokio::time::interval(player_data_autosave_interval(&config.server.player_data));
    player_data_autosave.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    player_data_autosave.tick().await;
    let mut pending_keep_alive = None;
    let mut chat_session: Option<crate::secure_chat::SecureChatSession> = None;
    let mut next_chat_global_index = 0;
    let enforce_secure_chat = effective_online_mode(&config.server);
    let mut chat_rate_limit = ChatRateLimit::new(&config.server.player_messages);
    let world_config = &config.world;
    let mut current_game_mode = world_config.game_mode;
    let simulation_distance = world_config.simulation_distance.max(1);
    let block_update_distance = block_update_distance(world_config.view_distance.max(1));
    let mut position = session.player.position;
    let mut initial_cluster_entity_view_sent = false;
    let mut last_stepped_block: Option<BlockPosition> = None;
    let mut last_input_flags = 0u8;
    let mut geyser_runtime = geyser::GeyserRuntime::default();
    let mut movement_observation_logs = 0u8;
    let mut survival = SurvivalState::from_stored(saved_player.survival, current_game_mode);
    let mut pending_dig: Option<mining::PendingDig> = None;
    let mut gameplay_runtime = gameplay::GameplayRuntime::new(&config.server.gameplay);
    gameplay_runtime
        .advancements
        .send_initial(sink, &session.player, plugins, &config.server.gameplay)
        .await?;
    let mut click_tracker = ClickTracker::new(&config.server.click_detection);
    let lobby = lobby::LobbyRuntime::new(&config.server.lobby);
    let mut lobby_status = lobby.refresh_status().await;
    let mut lobby_status_refresh = lobby.status_refresh_interval().map(tokio::time::interval);
    if let Some(interval) = lobby_status_refresh.as_mut() {
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        interval.tick().await;
    }
    let mut lobby_broadcast = lobby.broadcast_interval().map(tokio::time::interval);
    if let Some(interval) = lobby_broadcast.as_mut() {
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        interval.tick().await;
    }
    let mut lobby_broadcast_index = 0usize;
    let mut lobby_menu_open = false;
    let menus = menus::MenuRuntime::new(&config.server.menus);
    let mut active_config_menu: Option<String> = None;
    let mut players_hidden = false;
    let mut visible_player_entities = players
        .list_except(profile.uuid)
        .into_iter()
        .filter(|player| player.dimension == play_dimension)
        .filter(|player| {
            within_horizontal_distance(
                position,
                player.position,
                config.server.entity_rendering.player_distance,
            )
        })
        .map(|player| player.profile.uuid)
        .collect::<HashSet<_>>();
    let navigator_changes = lobby::sync_navigator_item(&mut inventory, &lobby);
    let menu_hotbar_changes = menus.sync_hotbar_items(&mut inventory);
    lobby.show_boss_bar(sink).await?;
    lobby.update_boss_bar_status(sink, &lobby_status).await?;
    if let Some(request) = lobby.proxy_server_list_request(&config.server) {
        sink.send(request).await?;
    }
    for packet in scoreboard::lobby_sidebar_packets(
        &config.server.scoreboard,
        &lobby,
        &lobby_status,
        config.server.placeholders.enable,
        plugins,
        &session.player,
        players.online_count(),
        config.server.max_player,
    )? {
        sink.send_raw(packet).await?;
    }
    let initial_inventory_changes = navigator_changes
        .into_iter()
        .chain(menu_hotbar_changes)
        .collect::<Vec<_>>();
    if !initial_inventory_changes.is_empty() {
        sync_inventory_changes(
            sink,
            players,
            profile.uuid,
            session.player.entity_id,
            inventory.selected_slot(),
            initial_inventory_changes,
        )
        .await?;
        sink.flush().await?;
    }
    let (chunk_sender, mut chunk_receiver) = tokio::sync::mpsc::unbounded_channel();
    let (fluid_wake_sender, mut fluid_wake_receiver) = tokio::sync::mpsc::unbounded_channel();
    let mut fluid_tick_job = None;
    chunk_state.refresh_pending_chunks();
    let fluid_seeds = chunk_state
        .send_center_chunk_first(sink, world, plugins)
        .await?;
    fluid.enqueue_fluid_seeds(&play_dimension, fluid_seeds);
    let fluid_seeds = chunk_state
        .send_remaining_initial_chunks(sink, world, plugins)
        .await?;
    fluid.enqueue_fluid_seeds(&play_dimension, fluid_seeds);
    chunk_state.start_next_chunk_load(world, Some(&chunk_sender), sink.compression_threshold());

    let result: Result<()> = async {
        loop {
            tokio::select! {
            changed = shutdown.changed() => {
                if changed.is_ok() && *shutdown.borrow() {
                    log::debug!("closing play session because server shutdown was requested");
                    break Ok(());
                }
            }
            loaded_chunk = chunk_receiver.recv(), if chunk_state.has_loading_chunks() => {
                let _span = crate::profile_span!("world:chunk_load");
                let Some(loaded_chunk) = loaded_chunk else {
                    anyhow::bail!("chunk load task channel closed");
                };
                chunk_state
                    .queue_loaded_chunk(loaded_chunk);
                chunk_state.start_next_chunk_load(world, Some(&chunk_sender), sink.compression_threshold());
                let fluid_seeds = chunk_state
                    .send_ready_chunks(sink, world, plugins, &chunk_sender, false)
                    .await?;
                fluid.enqueue_fluid_seeds(&play_dimension, fluid_seeds);
                send_initial_cluster_entity_view_after_chunks(
                    sink,
                    cluster_entities,
                    &mut initial_cluster_entity_view_sent,
                    &mut session.player,
                    &play_dimension,
                    position,
                    &config.server.entity_rendering,
                    simulation_distance,
                    &chunk_state,
                )
                .await?;
            }
            _ = chunk_send_tick.tick(), if chunk_state.has_ready_chunks() => {
                let _span = crate::profile_span!("net:chunk_send");
                let fluid_seeds = chunk_state
                    .send_ready_chunks(sink, world, plugins, &chunk_sender, true)
                    .await?;
                fluid.enqueue_fluid_seeds(&play_dimension, fluid_seeds);
                send_initial_cluster_entity_view_after_chunks(
                    sink,
                    cluster_entities,
                    &mut initial_cluster_entity_view_sent,
                    &mut session.player,
                    &play_dimension,
                    position,
                    &config.server.entity_rendering,
                    simulation_distance,
                    &chunk_state,
                )
                .await?;
            }
            _ = chunk_unload_sweep.tick(), if chunk_state.has_pending_unloads() => {
                let _span = crate::profile_span!("world:chunk_unload");
                let unloaded = chunk_state
                    .unload_expired_chunks(sink, plugins, Instant::now())
                    .await?;
                if unloaded > 0 {
                    log::debug!("delayed chunk unload completed: count={unloaded}");
                }
            }
            _ = pending_chunk_center_update_sleep(&chunk_state), if chunk_state.has_pending_center_update() => {
                if chunk_state
                    .apply_due_center_update(sink, &chunk_sender, world, position.x, position.z)
                    .await?
                {
                    log::debug!("delayed chunk center update applied");
                }
            }
            _ = survival_tick.tick() => {
                let _span = crate::profile_span!("tick:survival");
                if apply_survival_tick(
                    sink,
                    world,
                    world_rules,
                    players,
                    plugins,
                    fluid,
                    entities,
                    world_config,
                    current_game_mode,
                    &mut play_dimension,
                    profile.uuid,
                    session.player.entity_id,
                    &chunk_sender,
                    &mut chunk_state,
                    &mut position,
                    &mut survival,
                    &mut inventory,
                    &mut next_teleport_id,
                    config.server.gameplay.drop_inventory_on_death,
                    &config.server.entity_rendering,
                ).await? {
                    pending_dig = None;
                    session.player.position = position;
                    session.player.dimension = play_dimension.clone();
                }
                let effect_damage = gameplay_runtime
                    .effects
                    .tick(
                        sink,
                        &session.player,
                        plugins,
                        &config.server.gameplay,
                        SURVIVAL_TICK_INTERVAL,
                        &mut survival,
                    )
                    .await?;
                if let Some(message) = effect_damage.death_message() {
                    handle_player_death(
                        sink,
                        world,
                        world_rules,
                        players,
                        plugins,
                        fluid,
                        entities,
                        world_config,
                        current_game_mode,
                        profile.uuid,
                        &mut play_dimension,
                        session.player.entity_id,
                        &chunk_sender,
                        &mut chunk_state,
                        &mut position,
                        &mut inventory,
                        &mut survival,
                        &mut next_teleport_id,
                        message,
                        config.server.gameplay.drop_inventory_on_death,
                        &config.server.entity_rendering,
                        0,
                    )
                    .await?;
                    pending_dig = None;
                    session.player.position = position;
                    session.player.dimension = play_dimension.clone();
                }
            }
            _ = gameplay_tick.tick() => {
                let _span = crate::profile_span!("tick:gameplay");
                let mut tick_player = session.player.clone();
                tick_player.position = position;
                tick_player.dimension = play_dimension.clone();
                let response = plugins.handle_player_tick(
                    &tick_player,
                    GAMEPLAY_TICK_INTERVAL.as_millis() as u64,
                );
                let handled = handle_plugin_response_actions(
                    sink,
                    world,
                    world_rules,
                    &config.server,
                    world_config,
                    entities,
                    players,
                    plugins,
                    &tick_player,
                    response,
                    &chunk_sender,
                    &mut chunk_state,
                    &mut position,
                    &mut next_teleport_id,
                    &mut play_dimension,
                    &menus,
                    &mut active_config_menu,
                    &mut players_hidden,
                    &mut visible_player_entities,
                    config.server.entity_rendering.player_distance,
                    &mut inventory,
                    &mut geyser_runtime,
                )
                .await?;
                if handled {
                    if let Some(ref pending) = pending_dig {
                        send_block_destruction_stage(
                            sink,
                            session.player.entity_id,
                            pending.position.clone(),
                            -1,
                        )
                        .await?;
                    }
                    pending_dig = None;
                }

                // Send block destruction stage updates for active mining.
                // Advance one game tick first so the destruction stage
                // progresses at a steady 20 TPS rate regardless of real-time
                // tick jitter.
                if let Some(ref mut pending) = pending_dig {
                    pending.tick();
                    if let Some(stage) = pending.destroy_stage() {
                        send_block_destruction_stage(
                            sink,
                            session.player.entity_id,
                            pending.position.clone(),
                            stage,
                        )
                        .await?;
                    }
                }

                if config.server.gameplay.redstone
                    && gameplay_runtime.should_tick_redstone(&config.server.gameplay)
                {
                    let _span = crate::profile_span!("tick:redstone");
                    let updates =
                        gameplay_runtime
                            .redstone
                            .tick(world, world_rules, players, &config.server.gameplay);
                    apply_and_propagate_redstone_updates(
                        sink,
                        world,
                        world_rules,
                        players,
                        fluid,
                        &config.server.gameplay,
                        &mut gameplay_runtime.redstone,
                        &play_dimension,
                        profile.uuid,
                        updates,
                    )
                    .await?;
                }

                if config.server.gameplay.block_updates {
                    let _fluid_span = crate::profile_span!("tick:fluid_update");
                    poll_apply_and_restart_fluid_tick(
                        sink,
                        world,
                        world_rules,
                        players,
                        fluid,
                        &play_dimension,
                        profile.uuid,
                        position,
                        block_update_distance,
                        &mut fluid_tick_job,
                        &fluid_wake_sender,
                    )
                    .await?;
                }

                if config.server.gameplay.block_updates
                    && gameplay_runtime.should_tick_farmland(&config.server.gameplay)
                {
                    let _span = crate::profile_span!("tick:environment");
                    let updates = environment_tick_updates(world, &play_dimension, position);
                    apply_environment_block_state_updates(
                        sink,
                        world,
                        world_rules,
                        players,
                        fluid,
                        &play_dimension,
                        profile.uuid,
                        updates,
                    )
                    .await?;
                }

                if gameplay_runtime.should_tick_furnace(&config.server.gameplay) {
                    let _span = crate::profile_span!("tick:furnace");
                    let mut outcome = gameplay_runtime
                        .furnace
                        .tick(
                            sink,
                            &session.player,
                            plugins,
                            &config.server.gameplay,
                        )
                        .await?;
                    handle_gameplay_outcome(
                        sink,
                        players,
                        profile.uuid,
                        session.player.entity_id,
                        &mut inventory,
                        &mut gameplay_runtime,
                        plugins,
                        &session.player,
                        &config.server.gameplay,
                        &mut outcome,
                    )
                    .await?;
                }
                if gameplay_runtime.should_tick_oxygen(&config.server.gameplay) {
                    let oxygen = gameplay_runtime
                        .oxygen
                        .tick(
                            sink,
                            &session.player,
                            plugins,
                            &config.server.gameplay,
                            false,
                            &mut inventory,
                            &gameplay_runtime.effects,
                            &mut survival,
                            current_game_mode,
                        )
                        .await?;
                    if let Some(message) = oxygen.death_message() {
                        handle_player_death(
                            sink,
                            world,
                            world_rules,
                            players,
                            plugins,
                            fluid,
                            entities,
                            world_config,
                            current_game_mode,
                            profile.uuid,
                            &mut play_dimension,
                            session.player.entity_id,
                            &chunk_sender,
                            &mut chunk_state,
                            &mut position,
                            &mut inventory,
                            &mut survival,
                            &mut next_teleport_id,
                            message,
                            config.server.gameplay.drop_inventory_on_death,
                            &config.server.entity_rendering,
                            0,
                        )
                        .await?;
                        pending_dig = None;
                        session.player.position = position;
                        session.player.dimension = play_dimension.clone();
                    }
                }
            }
            _ = fluid_wake_receiver.recv(), if config.server.gameplay.block_updates => {
                let _span = crate::profile_span!("tick:fluid_apply");
                poll_apply_and_restart_fluid_tick(
                    sink,
                    world,
                    world_rules,
                    players,
                    fluid,
                    &play_dimension,
                    profile.uuid,
                    position,
                    block_update_distance,
                    &mut fluid_tick_job,
                    &fluid_wake_sender,
                )
                .await?;
            }
            _ = world_time_tick.tick() => {
                let _span = crate::profile_span!("tick:world_time");
                let game_time = world_rules.tick_dimension_time(&play_dimension);
                sink.send(SetTime {
                    game_time,
                    clock_updates: Vec::new(),
                })
                .await?;
                sink.flush().await?;
            }
            _ = sidebar_refresh_tick.tick() => {
                let _span = crate::profile_span!("ui:sidebar");
                let mut sidebar_player = session.player.clone();
                sidebar_player.position = position;
                sidebar_player.dimension = play_dimension.clone();
                for packet in scoreboard::refresh_lobby_sidebar_packets(
                    &config.server.scoreboard,
                    &lobby,
                    &lobby_status,
                    config.server.placeholders.enable,
                    plugins,
                    &sidebar_player,
                    players.online_count(),
                    config.server.max_player,
                )? {
                    sink.send_raw(packet).await?;
                }
                sink.flush().await?;
            }
            _ = player_data_autosave.tick() => {
                let _span = crate::profile_span!("io:player_autosave");
                save_player_runtime(
                    player_data,
                    player_data_lock,
                    saved_player,
                    &play_dimension,
                    position,
                    &inventory,
                    survival,
                    profile.uuid,
                    "autosave",
                )
                .await;
                world.flush_block_writes();
            }
            event = session.receiver.recv() => {
                let _span = crate::profile_span!("event:player");
                let Some(event) = event else {
                    continue;
                };
                if let crate::players::PlayerEvent::Damage {
                    profile_id: target_id,
                    amount,
                    kind,
                    source_entity_id,
                    source_position,
                    knockback,
                } = event
                {
                    if target_id == profile.uuid {
                        if apply_external_player_damage(
                            sink,
                            world,
                            world_rules,
                            players,
                            plugins,
                            fluid,
                            entities,
                            world_config,
                            current_game_mode,
                            profile.uuid,
                            &mut play_dimension,
                            session.player.entity_id,
                            &chunk_sender,
                            &mut chunk_state,
                            &mut position,
                            &mut inventory,
                            &mut survival,
                            &mut next_teleport_id,
                            amount,
                            kind,
                            source_entity_id,
                            source_position,
                            knockback,
                            config.server.gameplay.drop_inventory_on_death,
                            &config.server.entity_rendering,
                        )
                        .await?
                        {
                            pending_dig = None;
                            session.player.position = position;
                            session.player.dimension = play_dimension.clone();
                        }
                    }
                    continue;
                }
                if let crate::players::PlayerEvent::GameModeChanged {
                    profile_id: target_id,
                    username: _,
                    game_mode,
                } = event
                {
                    if target_id == profile.uuid {
                        if let Some(next_mode) = game_mode_from_protocol_id(game_mode) {
                            current_game_mode = next_mode;
                            session.player.game_mode = game_mode;
                            sink.send(GameStateChange {
                                reason: 3,
                                game_mode: game_mode as f32,
                            })
                            .await?;
                            sink.send(ClientboundPlayerAbilities {
                                flags: player_ability_flags(
                                    current_game_mode,
                                    world_config.allow_flight,
                                ),
                                flying_speed: 0.05,
                                walking_speed: 0.1,
                            })
                            .await?;
                            sink.send(PlayerInfoUpdate {
                                actions: PlayerInfoActions(PlayerInfoActions::UPDATE_GAME_MODE),
                                entries: vec![PlayerInfoEntry {
                                    profile_id: profile.uuid,
                                    game_mode: VarInt(game_mode),
                                    ..PlayerInfoEntry::default()
                                }],
                            })
                            .await?;
                            pending_dig = None;
                            sink.flush().await?;
                        }
                    }
                    continue;
                }
                if let crate::players::PlayerEvent::GiveItem {
                    profile_id: target_id,
                    item,
                    item_name,
                } = event
                {
                    if target_id == profile.uuid {
                        let requested = item.item_count.0.max(0);
                        let (changes, given) = inventory.add_item_stack_partial(&item);
                        if !changes.is_empty() {
                            sync_inventory_changes(
                                sink,
                                players,
                                profile.uuid,
                                session.player.entity_id,
                                inventory.selected_slot(),
                                changes,
                            )
                            .await?;
                        }
                        if given > 0 {
                            sink.send(SystemChat {
                                content: text_component(format!(
                                    "收到 {given} 个 {item_name}"
                                )),
                                overlay: false,
                            })
                            .await?;
                        }
                        if given < requested {
                            sink.send(SystemChat {
                                content: text_component(format!(
                                    "背包空间不足，剩余 {} 个 {item_name} 未放入",
                                    requested - given
                                )),
                                overlay: false,
                            })
                            .await?;
                        }
                        sink.flush().await?;
                    }
                    continue;
                }
                if let crate::players::PlayerEvent::PotionEffect {
                    profile_id: target_id,
                    effect,
                    amplifier,
                    duration_ticks,
                    source_entity_id,
                    source_position,
                    knockback,
                } = event
                {
                    if target_id == profile.uuid
                        && apply_external_player_potion_effect(
                            sink,
                            world,
                            world_rules,
                            players,
                            plugins,
                            fluid,
                            entities,
                            world_config,
                            current_game_mode,
                            profile.uuid,
                            &mut play_dimension,
                            session.player.entity_id,
                            &chunk_sender,
                            &mut chunk_state,
                            &mut position,
                            &mut inventory,
                            &mut survival,
                            &mut next_teleport_id,
                            &mut gameplay_runtime.effects,
                            &config.server.gameplay,
                            &effect,
                            amplifier,
                            duration_ticks,
                            source_entity_id,
                            source_position,
                            knockback,
                            &config.server.entity_rendering,
                        )
                        .await?
                    {
                        pending_dig = None;
                        session.player.position = position;
                        session.player.dimension = play_dimension.clone();
                    }
                    continue;
                }
                if let crate::players::PlayerEvent::Teleport {
                    profile_id: target_id,
                    dimension: target_dimension,
                    position: target_position,
                } = event
                {
                    if target_id == profile.uuid {
                        pending_dig = None;
                        let changed_dimension = play_dimension != target_dimension;
                        position = target_position;
                        if changed_dimension {
                            world_rules.ensure_loaded(&target_dimension)?;
                            let dimension_rule = world_rules.snapshot(&target_dimension);
                            sink.send(Respawn {
                                dimension_type: VarInt(dimension_type_holder_id(
                                    &dimension_rule.dimension_type,
                                )),
                                dimension_name: target_dimension.clone(),
                                hashed_seed: 0,
                                game_mode: current_game_mode.protocol_id(),
                                previous_game_mode: -1,
                                is_debug: false,
                                is_flat: true,
                                has_death_location: false,
                                death_dimension_name: None,
                                death_position: None,
                                portal_cooldown: VarInt(0),
                                sea_level: VarInt(63),
                                data_to_keep: KEEP_NO_DATA,
                            })
                            .await?;
                            play_dimension = target_dimension.clone();
                            send_respawn_player_state(
                                sink,
                                world_config,
                                world_rules,
                                &play_dimension,
                                position,
                            )
                            .await?;
                        }
                        let teleport_id = next_teleport_id;
                        next_teleport_id = next_teleport_id.saturating_add(1);
                        sink.send(Position {
                            teleport_id: VarInt(teleport_id),
                            x: position.x,
                            y: position.y,
                            z: position.z,
                            dx: 0.0,
                            dy: 0.0,
                            dz: 0.0,
                            yaw: position.yaw,
                            pitch: position.pitch,
                            flags: 0,
                        })
                        .await?;
                        if changed_dimension {
                            let fluid_seeds = chunk_state
                            .reset_dimension_after_respawn(
                                sink,
                                &chunk_sender,
                                world,
                                plugins,
                                play_dimension.clone(),
                                position.x,
                                position.z,
                            )
                            .await?;
                            fluid.enqueue_fluid_seeds(&play_dimension, fluid_seeds);
                            resync_inventory_state(
                                sink,
                                players,
                                profile.uuid,
                                session.player.entity_id,
                                &inventory,
                            )
                            .await?;
                        } else {
                            let fluid_seeds = chunk_state
                                .reset_after_respawn(
                                    sink,
                                    &chunk_sender,
                                    world,
                                    plugins,
                                    position.x,
                                    position.z,
                                )
                                .await?;
                            fluid.enqueue_fluid_seeds(&play_dimension, fluid_seeds);
                        }
                        session.player.position = position;
                        session.player.dimension = play_dimension.clone();
                        visible_player_entities.clear();
                        if !players_hidden {
                            refresh_visible_players(
                                sink,
                                players,
                                profile.uuid,
                                player_entity_type,
                                &play_dimension,
                                position,
                                config.server.entity_rendering.player_distance,
                                &mut visible_player_entities,
                            )
                            .await?;
                        }
                        sink.flush().await?;
                    }
                    continue;
                }
                if let crate::players::PlayerEvent::ProjectileHitPlayer(hit) = event {
                    if hit.shooter_profile_id == profile.uuid {
                        if let (Some(shooter), Some(target)) = (
                            players.player_by_uuid(hit.shooter_profile_id),
                            players.player_by_uuid(hit.target_profile_id),
                        ) {
                            let response = plugins.handle_projectile_hit_player(
                                crate::plugins::ProjectileHitPlayerPayload {
                                    shooter: qexed_plugin_api::player_payload_owned(&shooter),
                                    target: qexed_plugin_api::player_payload_owned(&target),
                                    dimension: hit.dimension,
                                    position: qexed_plugin_api::player_position_payload(
                                        hit.position,
                                    ),
                                    projectile_entity_id: hit.projectile_entity_id,
                                    projectile_kind: hit.projectile_kind,
                                    configured_event: hit.configured_event,
                                    tag: hit.tag,
                                },
                            );
                            let mut player = shooter;
                            player.position = position;
                            player.dimension = play_dimension.clone();
                            let handled = handle_plugin_response_actions(
                                sink,
                                world,
                                world_rules,
                                &config.server,
                                world_config,
                                entities,
                                players,
                                plugins,
                                &player,
                                response,
                                &chunk_sender,
                                &mut chunk_state,
                                &mut position,
                                &mut next_teleport_id,
                                &mut play_dimension,
                                &menus,
                                &mut active_config_menu,
                                &mut players_hidden,
                                &mut visible_player_entities,
                                config.server.entity_rendering.player_distance,
                                &mut inventory,
                                &mut geyser_runtime,
                            )
                            .await?;
                            if handled {
                                pending_dig = None;
                            }
                        }
                    }
                    continue;
                }
                if event_is_self(&event, profile.uuid) {
                    continue;
                }
                if players_hidden && event_affects_player_entity(&event) {
                    continue;
                }
                let event_packets = filtered_player_event_packets(
                    &event,
                    players,
                    player_entity_type,
                    &play_dimension,
                    position,
                    config.server.entity_rendering.player_distance,
                    block_update_distance,
                    &mut visible_player_entities,
                )?;
                for packet in event_packets {
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
                let _span = crate::profile_span!("net:packet_process");
                let Some(mut payload) = packet? else {
                    break Ok(());
                };

                let packet_id = crate::connection::read_packet_id(&mut payload)?;
                if packet_id == AcceptTeleportation::ID {
                    let teleport = crate::connection::decode_payload::<AcceptTeleportation>(&mut payload)?;
                    log::debug!("client accepted teleport: teleport_id={}", teleport.teleport_id.0);
                    continue;
                }

                if packet_id == ServerboundKeepAlive::ID {
                    let keep_alive = crate::connection::decode_payload::<ServerboundKeepAlive>(&mut payload)?;
                    match pending_keep_alive {
                        Some(expected) if keep_alive.keep_alive_id == expected => {
                            log::debug!("client responded KeepAlive: id={expected}");
                            pending_keep_alive = None;
                        }
                        Some(expected) => {
                            anyhow::bail!(
                                "client KeepAlive mismatch: expected {}, actual {}",
                                expected,
                                keep_alive.keep_alive_id
                            );
                        }
                        None => {
                            log::trace!("ignored unsolicited client KeepAlive: id={}", keep_alive.keep_alive_id);
                        }
                    }
                    continue;
                }

                if packet_id == ChunkBatchReceived::ID {
                    let batch = crate::connection::decode_payload::<ChunkBatchReceived>(&mut payload)?;
                    chunk_state.on_chunk_batch_received(batch.desired_chunks_per_tick);
                    let fluid_seeds = chunk_state
                        .send_ready_chunks(sink, world, plugins, &chunk_sender, false)
                        .await?;
                    fluid.enqueue_fluid_seeds(&play_dimension, fluid_seeds);
                    send_initial_cluster_entity_view_after_chunks(
                        sink,
                        cluster_entities,
                        &mut initial_cluster_entity_view_sent,
                        &mut session.player,
                        &play_dimension,
                        position,
                        &config.server.entity_rendering,
                        simulation_distance,
                        &chunk_state,
                    )
                    .await?;
                    log::debug!(
                        "client acknowledged chunk batch, desired rate: {} chunks/tick",
                        batch.desired_chunks_per_tick
                    );
                    continue;
                }

                if packet_id == PlayerLoaded::ID {
                    let _loaded = crate::connection::decode_payload::<PlayerLoaded>(&mut payload)?;
                    let fluid_seeds = chunk_state
                        .send_ready_chunks(sink, world, plugins, &chunk_sender, false)
                        .await?;
                    fluid.enqueue_fluid_seeds(&play_dimension, fluid_seeds);
                    send_initial_cluster_entity_view_after_chunks(
                        sink,
                        cluster_entities,
                        &mut initial_cluster_entity_view_sent,
                        &mut session.player,
                        &play_dimension,
                        position,
                        &config.server.entity_rendering,
                        simulation_distance,
                        &chunk_state,
                    )
                    .await?;
                    log::debug!("client reported player loaded: player={}", profile.username);
                    continue;
                }

                if packet_id == CommandSuggestion::ID {
                    let suggestion = crate::connection::decode_payload::<CommandSuggestion>(&mut payload)?;
                    let matches = command_suggestion_matches(
                        &suggestion.text,
                        players,
                        &lobby,
                        &lobby_status,
                    );
                    sink.send(CommandSuggestions {
                        id: suggestion.id,
                        start: VarInt(matches.start as i32),
                        length: VarInt(matches.length as i32),
                        matches: matches
                            .values
                            .into_iter()
                            .map(|value| Matches {
                                r#match: value,
                                tooltip: None,
                            })
                            .collect(),
                    })
                    .await?;
                    sink.flush().await?;
                    continue;
                }

                if packet_id == ServerboundCustomPayload::ID {
                    let custom_payload = crate::connection::decode_payload::<ServerboundCustomPayload>(&mut payload)?;
                    let geyser_outcome = geyser_runtime.apply_custom_payload(&custom_payload);
                    if geyser_outcome.recognized {
                        plugins.upsert_geyser_player_info(geyser_runtime.player_info(&session.player));
                    }
                    if let Some(response) = geyser_outcome.form_response {
                        if let Some((_menu_id, action)) = menus.action_for_bedrock_form_response(
                            &response.plugin_form_id,
                            &response.response,
                        ) {
                            let outcome = run_menu_action(
                                sink,
                                &menus,
                                players,
                                &mut players_hidden,
                                &mut visible_player_entities,
                                profile.uuid,
                                session.player.entity_id,
                                &play_dimension,
                                position,
                                config.server.entity_rendering.player_distance,
                                Some(&config.server),
                                plugins,
                                &lobby,
                                &lobby_status,
                                Some(&mut inventory),
                                Some(&mut geyser_runtime),
                                action,
                            )
                            .await?;
                            if outcome.opened_menu {
                                active_config_menu = outcome.opened_menu_id;
                            } else {
                                active_config_menu = None;
                            }
                            let deferred_actions = outcome.deferred_actions;
                            let ran_deferred_actions = !deferred_actions.is_empty();
                            apply_deferred_menu_actions(
                                sink,
                                world,
                                world_rules,
                                &config.server,
                                world_config,
                                entities,
                                players,
                                plugins,
                                profile.uuid,
                                &chunk_sender,
                                &mut chunk_state,
                                &mut position,
                                &mut next_teleport_id,
                                &mut play_dimension,
                                &menus,
                                &mut active_config_menu,
                                &mut players_hidden,
                                &mut visible_player_entities,
                                config.server.entity_rendering.player_distance,
                                &mut inventory,
                                &mut geyser_runtime,
                                session.player.entity_id,
                                deferred_actions,
                            )
                            .await?;
                            if ran_deferred_actions {
                                session.player.position = position;
                                session.player.dimension = play_dimension.clone();
                            }
                            pending_dig = None;
                            sink.flush().await?;
                            continue;
                        }
                        if menus.is_bedrock_menu_form_id(&response.plugin_form_id) {
                            active_config_menu = None;
                            sink.flush().await?;
                            continue;
                        }
                        plugins.emit_bedrock_form_response(&qexed_plugin_api::BedrockFormResponsePayload {
                            player: qexed_plugin_api::player_payload_owned(&session.player),
                            form_id: response.form_id,
                            plugin_form_id: response.plugin_form_id,
                            response: response.response,
                        });
                        sink.flush().await?;
                        continue;
                    }
                    if lobby.apply_proxy_server_list(&mut lobby_status, &custom_payload) {
                        lobby.update_boss_bar_status(sink, &lobby_status).await?;
                        for packet in scoreboard::refresh_lobby_sidebar_packets(
                            &config.server.scoreboard,
                            &lobby,
                            &lobby_status,
                            config.server.placeholders.enable,
                            plugins,
                            &session.player,
                            players.online_count(),
                            config.server.max_player,
                        )? {
                            sink.send_raw(packet).await?;
                        }
                        if lobby_menu_open {
                            lobby.refresh_open_menu(sink, &lobby_status).await?;
                        }
                        refresh_command_tree(
                            sink,
                            permissions,
                            plugins,
                            profile,
                            &lobby,
                            &lobby_status,
                        )
                        .await?;
                        sink.flush().await?;
                    }
                    continue;
                }

                if packet_id == SetCarriedItem::ID {
                    let carried = crate::connection::decode_payload::<SetCarriedItem>(&mut payload)?;
                    if let Some(main_hand) = inventory.set_selected(carried.slot) {
                        pending_dig = None;
                        players.update_equipment(
                            profile.uuid,
                            vec![qexed_protocol::to_client::play::set_equipment::Equipment::mainhand(
                                main_hand,
                            )],
                        );
                        player_audit.log_item_switch(
                            profile,
                            &play_dimension,
                            carried.slot,
                            inventory.held_item().item_id.as_ref().map(|id| id.0),
                            inventory.held_item().item_count.0,
                        );
                        sink.flush().await?;
                    } else {
                        log::warn!("ignored invalid carried item slot: {}", carried.slot);
                    }
                    continue;
                }

                if packet_id == SetCreativeModeSlot::ID {
                    let slot = crate::connection::decode_payload::<SetCreativeModeSlot>(&mut payload)?;
                    if survival.is_dead() {
                        log::debug!(
                            "ignored creative slot update from dead player: uuid={}",
                            profile.uuid
                        );
                        continue;
                    }
                    if current_game_mode != GameMode::Creative {
                        log::debug!(
                            "ignored creative slot update outside creative mode: uuid={}",
                            profile.uuid
                        );
                        continue;
                    }
                    let creative_hotbar_slot = creative_mode_hotbar_slot(slot.slot_num);
                    if lobby.protect_world() {
                        pending_dig = None;
                        resync_inventory_state(
                            sink,
                            players,
                            profile.uuid,
                            session.player.entity_id,
                            &mut inventory,
                        )
                        .await?;
                        sink.flush().await?;
                        continue;
                    }
                    if creative_hotbar_slot.is_some_and(|hotbar_slot| {
                        inventory
                            .hotbar_item(hotbar_slot)
                            .is_some_and(|item| lobby.navigator_item_matches(hotbar_slot, item))
                    }) {
                        pending_dig = None;
                        resync_inventory_state(
                            sink,
                            players,
                            profile.uuid,
                            session.player.entity_id,
                            &mut inventory,
                        ).await?;
                        sink.flush().await?;
                        continue;
                    }
                    if creative_hotbar_slot.is_some_and(|hotbar_slot| {
                        menus.fixed_hotbar_slot(hotbar_slot)
                            && inventory
                                .hotbar_item(hotbar_slot)
                                .is_some_and(|item| menus.hotbar_item_matches(hotbar_slot, item))
                    }) {
                        pending_dig = None;
                        resync_inventory_state(
                            sink,
                            players,
                            profile.uuid,
                            session.player.entity_id,
                            &mut inventory,
                        )
                        .await?;
                        sink.flush().await?;
                        continue;
                    }
                    if let Some(change) = inventory.set_creative_slot(slot.slot_num, slot.item_stack.clone()) {
                        pending_dig = None;
                        sync_inventory_changes(
                            sink,
                            players,
                            profile.uuid,
                            session.player.entity_id,
                            inventory.selected_slot(),
                            vec![change],
                        ).await?;
                        sink.flush().await?;
                    }
                    continue;
                }

                if packet_id == PickItemFromBlock::ID {
                    let pick = crate::connection::decode_payload::<PickItemFromBlock>(&mut payload)?;
                    if survival.is_dead() {
                        continue;
                    }
                    if lobby.protect_world() {
                        pending_dig = None;
                        resync_inventory_state(
                            sink,
                            players,
                            profile.uuid,
                            session.player.entity_id,
                            &mut inventory,
                        )
                        .await?;
                        sink.flush().await?;
                        continue;
                    }
                    let item_id = world
                        .block_state_at(&play_dimension, &pick.position)
                        .and_then(crate::inventory::picked_item_for_block_state)
                        .unwrap_or(1);
                    let slot = inventory.pick_block(item_id);
                    pending_dig = None;
                    let held = inventory.held_item().clone();
                    sink.send(crate::inventory::set_player_inventory_packet(slot, held.clone())).await?;
                    sink.send(SetHeldSlot { slot: VarInt(slot as i32) }).await?;
                    players.update_equipment(
                        profile.uuid,
                        vec![qexed_protocol::to_client::play::set_equipment::Equipment::mainhand(held)],
                    );
                    player_audit.log_item_switch(
                        profile,
                        &play_dimension,
                        slot as i16,
                        inventory.held_item().item_id.as_ref().map(|id| id.0),
                        inventory.held_item().item_count.0,
                    );
                    sink.flush().await?;
                    continue;
                }

                if packet_id == UseItemOn::ID {
                    let use_item_on = crate::connection::decode_payload::<UseItemOn>(&mut payload)?;
                    let sequence = use_item_on.sequence.clone();
                    if survival.is_dead() {
                        send_block_change_ack(sink, sequence).await?;
                        sink.flush().await?;
                        continue;
                    }
                    pending_dig = None;
                    if lobby.protect_world() {
                        send_block_rollback(sink, world, &play_dimension, use_item_on.block_hit.position.clone()).await?;
                        send_block_change_ack(sink, sequence).await?;
                        sink.flush().await?;
                        continue;
                    }
                    if handle_plugin_player_block_interact(
                        sink,
                        world,
                        world_rules,
                        &config.server,
                        world_config,
                        entities,
                        players,
                        plugins,
                        &session.player,
                        &use_item_on,
                        &chunk_sender,
                        &mut chunk_state,
                        &mut position,
                        &mut next_teleport_id,
                        &mut play_dimension,
                        &menus,
                        &mut active_config_menu,
                        &mut players_hidden,
                        &mut visible_player_entities,
                        config.server.entity_rendering.player_distance,
                        &mut inventory,
                        &mut geyser_runtime,
                        last_input_flags,
                    )
                    .await?
                    {
                        session.player.position = position;
                        session.player.dimension = play_dimension.clone();
                        send_block_change_ack(sink, sequence).await?;
                        sink.flush().await?;
                        continue;
                    }
                    if use_item_on.hand.0 == 0 {
                        if let Some(block_name) =
                            block_name_at(world, &play_dimension, &use_item_on.block_hit.position)
                        {
                            if handle_redstone_interaction(
                                sink,
                                world,
                                world_rules,
                                players,
                                fluid,
                                &config.server.gameplay,
                                &mut gameplay_runtime.redstone,
                                &play_dimension,
                                profile.uuid,
                                use_item_on.block_hit.position.clone(),
                                &block_name,
                            )
                            .await?
                            {
                                send_block_change_ack(sink, sequence).await?;
                                sink.flush().await?;
                                continue;
                            }
                            if handle_vanilla_block_interaction(
                                sink,
                                world,
                                world_rules,
                                players,
                                fluid,
                                world_config,
                                current_game_mode,
                                &play_dimension,
                                profile.uuid,
                                session.player.entity_id,
                                &config.server.gameplay,
                                &mut gameplay_runtime.redstone,
                                &mut inventory,
                                &mut survival,
                                use_item_on.block_hit.position.clone(),
                                use_item_on.block_hit.face.0,
                                &block_name,
                            )
                            .await?
                            {
                                send_block_change_ack(sink, sequence).await?;
                                sink.flush().await?;
                                continue;
                            }
                            if config.server.gameplay.crafting_table
                                && block_name == "minecraft:crafting_table"
                            {
                                gameplay_runtime.crafting.open(sink).await?;
                                send_block_change_ack(sink, sequence).await?;
                                sink.flush().await?;
                                continue;
                            }
                            if config.server.gameplay.furnace
                                && configured_gameplay_block_matches(
                                    &block_name,
                                    &config.server.gameplay.furnace_blocks,
                                )
                            {
                                gameplay_runtime.furnace.open(sink).await?;
                                send_block_change_ack(sink, sequence).await?;
                                sink.flush().await?;
                                continue;
                            }
                            if handle_cauldron_interaction(
                                sink,
                                world,
                                world_rules,
                                players,
                                fluid,
                                world_config,
                                current_game_mode,
                                &play_dimension,
                                profile.uuid,
                                session.player.entity_id,
                                &config.server.gameplay,
                                &mut inventory,
                                use_item_on.block_hit.position.clone(),
                                &block_name,
                            )
                            .await?
                            {
                                send_block_change_ack(sink, sequence).await?;
                                sink.flush().await?;
                                continue;
                            }
                            if config.enchanting.enable && block_name == "minecraft:enchanting_table" {
                                gameplay_runtime
                                    .enchanting
                                    .open(
                                        sink,
                                        config,
                                        &session.player,
                                        plugins,
                                        world,
                                        &play_dimension,
                                        &inventory,
                                        use_item_on.block_hit.position.clone(),
                                    )
                                    .await?;
                                send_block_change_ack(sink, sequence).await?;
                                sink.flush().await?;
                                continue;
                            }
                        }
                        if let Some(block_state) = crate::inventory::placed_block_state_for_item(inventory.held_item()) {
                            let held_item_id = inventory.held_item().item_id.as_ref().map(|id| id.0);
                            let placed = place_held_block(
                                sink,
                                world,
                                world_rules,
                                players,
                                fluid,
                                world_config,
                                current_game_mode,
                                &play_dimension,
                                profile.uuid,
                                &position,
                                &use_item_on,
                                block_state,
                                config.server.gameplay.block_updates,
                            )
                            .await?;
                            if !placed.is_empty() {
                                for changed in &placed {
                                    player_audit.log_block_place(
                                        profile,
                                        &play_dimension,
                                        &changed.position,
                                        changed.block_state,
                                        held_item_id,
                                    );
                                }
                                sync_held_item_after_world_edit(
                                    sink,
                                    players,
                                    profile.uuid,
                                    current_game_mode,
                                    &mut inventory,
                                )
                                .await?;
                                gameplay::sounds::play_at(
                                    sink,
                                    players,
                                    plugins,
                                    Some(&session.player),
                                    &play_dimension,
                                    position,
                                    "minecraft:block.stone.place",
                                    "block",
                                    config.server.gameplay.sounds,
                                )
                                .await?;
                                propagate_redstone_from_block_positions(
                                    sink,
                                    world,
                                    world_rules,
                                    players,
                                    fluid,
                                    &config.server.gameplay,
                                    &mut gameplay_runtime.redstone,
                                    &play_dimension,
                                    profile.uuid,
                                    placed
                                        .iter()
                                        .map(|changed| changed.position.clone())
                                        .collect(),
                                )
                                .await?;
                            }
                        }
                    }
                    send_block_change_ack(sink, sequence).await?;
                    sink.flush().await?;
                    continue;
                }

                if packet_id == PlayerAction::ID {
                    let action = crate::connection::decode_payload::<PlayerAction>(&mut payload)?;
                    let sequence = action.sequence.clone();
                    if click_tracker.record_player_action(
                        plugins,
                        &session.player,
                        action.status.0,
                    ) {
                        pending_dig = None;
                        if config.server.click_detection.cancel_actions {
                            if player_action_changes_block(action.status.0) {
                                send_block_rollback(sink, world, &play_dimension, action.location)
                                    .await?;
                            } else {
                                resync_inventory_state(
                                    sink,
                                    players,
                                    profile.uuid,
                                    session.player.entity_id,
                                    &inventory,
                                )
                                .await?;
                            }
                            send_block_change_ack(sink, sequence).await?;
                            sink.flush().await?;
                            continue;
                        }
                    }
                    if player_action_drops_item(action.status.0)
                        && (menus.hotbar_item_matches(inventory.selected_slot(), inventory.held_item())
                            || lobby.navigator_item_matches(
                                inventory.selected_slot(),
                                inventory.held_item(),
                            ))
                    {
                        pending_dig = None;
                        resync_inventory_state(
                            sink,
                            players,
                            profile.uuid,
                            session.player.entity_id,
                            &mut inventory,
                        )
                        .await?;
                        send_block_change_ack(sink, sequence).await?;
                        sink.flush().await?;
                        continue;
                    }
                    if player_action_drops_item(action.status.0) {
                        drop_player_item(
                            sink,
                            world,
                            players,
                            cluster_entities,
                            entities,
                            &play_dimension,
                            profile.uuid,
                            session.player.entity_id,
                            position,
                            &mut inventory,
                            action.status.0,
                            &config.server.entity_rendering,
                            simulation_distance,
                        )
                        .await?;
                        pending_dig = None;
                        send_block_change_ack(sink, sequence).await?;
                        sink.flush().await?;
                        continue;
                    }
                    if action.status.0 == PLAYER_ACTION_RELEASE_USE_ITEM {
                        if !survival.is_dead()
                            && handle_plugin_player_use_item(
                                sink,
                                world,
                                world_rules,
                                &config.server,
                                world_config,
                                entities,
                                players,
                                plugins,
                                &session.player,
                                "release_use_item",
                                "main_hand",
                                action.sequence.0,
                                position.yaw,
                                position.pitch,
                                last_input_flags,
                                &mut inventory,
                                &chunk_sender,
                                &mut chunk_state,
                                &mut position,
                                &mut next_teleport_id,
                                &mut play_dimension,
                                &menus,
                                &mut active_config_menu,
                                &mut players_hidden,
                                &mut visible_player_entities,
                                config.server.entity_rendering.player_distance,
                                &mut geyser_runtime,
                            )
                            .await?
                        {
                            pending_dig = None;
                        }
                        send_block_change_ack(sink, sequence).await?;
                        sink.flush().await?;
                        continue;
                    }
                    if survival.is_dead() {
                        pending_dig = None;
                        send_block_change_ack(sink, sequence).await?;
                        sink.flush().await?;
                        continue;
                    }
                    if adventure_destroy_packet_violates_can_break(
                        world,
                        current_game_mode,
                        &play_dimension,
                        &action.location,
                        action.status.0,
                        inventory.held_item(),
                    ) {
                        pending_dig = None;
                        let ban = warden.permanently_ban(
                            profile,
                            ADVENTURE_BREAK_CHEAT_BAN_REASON,
                        )?;
                        log::warn!(
                            "warden permanently banned player for illegal adventure block break: player={}, uuid={}, dimension={}, block=({}, {}, {}), reason={}",
                            profile.username,
                            profile.uuid,
                            play_dimension,
                            action.location.x,
                            action.location.y,
                            action.location.z,
                            ban.reason,
                        );
                        disconnect_play(sink, &ban.reason).await?;
                        sink.flush().await?;
                        return Ok(());
                    }
                    if lobby.protect_world() {
                        pending_dig = None;
                        if player_action_changes_block(action.status.0) {
                            send_block_rollback(sink, world, &play_dimension, action.location)
                                .await?;
                        } else {
                            resync_inventory_state(
                                sink,
                                players,
                                profile.uuid,
                                session.player.entity_id,
                                &mut inventory,
                            )
                            .await?;
                        }
                        send_block_change_ack(sink, sequence).await?;
                        sink.flush().await?;
                        continue;
                    }
                    match action.status.0 {
                        PLAYER_ACTION_START_DESTROY_BLOCK => {
                            if should_destroy_block(current_game_mode, PLAYER_ACTION_START_DESTROY_BLOCK) {
                                if let Some(destroyed) = destroy_block(
                                    sink,
                                    world,
                                    world_rules,
                                    ore_pits,
                                    players,
                                    fluid,
                                    entities,
                                    plugins,
                                    world_config,
                                    current_game_mode,
                                    &play_dimension,
                                    profile.uuid,
                                    inventory.held_item(),
                                    action.location,
                                    &config.server.entity_rendering,
                                    config.server.gameplay.block_updates,
                                )
                                .await? {
                                    player_audit.log_block_break(
                                        profile,
                                        &play_dimension,
                                        &destroyed.position,
                                        destroyed.previous_state,
                                        inventory.held_item().item_id.as_ref().map(|id| id.0),
                                    );
                                    propagate_redstone_from_block_positions(
                                        sink,
                                        world,
                                        world_rules,
                                        players,
                                        fluid,
                                        &config.server.gameplay,
                                        &mut gameplay_runtime.redstone,
                                        &play_dimension,
                                        profile.uuid,
                                        vec![destroyed.position],
                                    )
                                    .await?;
                                }
                            } else {
                                pending_dig = begin_destroy_block(
                                    sink,
                                    world,
                                    ore_pits,
                                    plugins,
                                    current_game_mode,
                                    &play_dimension,
                                    &action.location,
                                    inventory.held_item(),
                                )
                                .await?;
                                // Send stage 0 immediately so the client doesn't jump to a high stage
                                // on the first gameplay_tick (which may be up to 50ms later).
                                if let Some(ref pending) = pending_dig {
                                    send_block_destruction_stage(
                                        sink,
                                        session.player.entity_id,
                                        pending.position.clone(),
                                        0,
                                    )
                                    .await?;
                                }
                            }
                        }
                        PLAYER_ACTION_CANCEL_DESTROY_BLOCK => {
                            if let Some(ref pending) = pending_dig {
                                send_block_destruction_stage(
                                    sink,
                                    session.player.entity_id,
                                    pending.position.clone(),
                                    -1,
                                )
                                .await?;
                            }
                            pending_dig = None;
                        }
                        status if should_destroy_block(current_game_mode, status) => {
                            let can_destroy = can_finish_destroy_block(
                                sink,
                                world,
                                ore_pits,
                                current_game_mode,
                                &play_dimension,
                                &action.location,
                                inventory.held_item(),
                                &pending_dig,
                            )
                            .await?;
                            // Send clear destruction stage before clearing pending_dig
                            if let Some(ref pending) = pending_dig {
                                send_block_destruction_stage(
                                    sink,
                                    session.player.entity_id,
                                    pending.position.clone(),
                                    -1,
                                )
                                .await?;
                            }
                            pending_dig = None;
                            if can_destroy {
                                let destroyed = destroy_block(
                                    sink,
                                    world,
                                    world_rules,
                                    ore_pits,
                                    players,
                                    fluid,
                                    entities,
                                    plugins,
                                    world_config,
                                    current_game_mode,
                                    &play_dimension,
                                    profile.uuid,
                                    inventory.held_item(),
                                    action.location,
                                    &config.server.entity_rendering,
                                    config.server.gameplay.block_updates,
                                )
                                .await?;
                                let was_destroyed = destroyed.is_some();
                                if let Some(destroyed) = destroyed {
                                    player_audit.log_block_break(
                                        profile,
                                        &play_dimension,
                                        &destroyed.position,
                                        destroyed.previous_state,
                                        inventory.held_item().item_id.as_ref().map(|id| id.0),
                                    );
                                    propagate_redstone_from_block_positions(
                                        sink,
                                        world,
                                        world_rules,
                                        players,
                                        fluid,
                                        &config.server.gameplay,
                                        &mut gameplay_runtime.redstone,
                                        &play_dimension,
                                        profile.uuid,
                                        vec![destroyed.position.clone()],
                                    )
                                    .await?;
                                    survival.apply_exhaustion(MINING_EXHAUSTION_PER_BLOCK);
                                    if current_game_mode == GameMode::Survival {
                                        let damaged = gameplay::durability::damage_item(
                                            inventory.held_item_mut(),
                                            &session.player,
                                            plugins,
                                            &config.server.gameplay,
                                            "mine",
                                            1,
                                        );
                                        if damaged {
                                            sync_inventory_changes(
                                                sink,
                                                players,
                                                profile.uuid,
                                                session.player.entity_id,
                                                inventory.selected_slot(),
                                                vec![inventory.selected_hotbar_change()],
                                            )
                                            .await?;
                                        }
                                    }
                                    gameplay::sounds::play_at(
                                        sink,
                                        players,
                                        plugins,
                                        Some(&session.player),
                                        &play_dimension,
                                        position,
                                        "minecraft:block.stone.break",
                                        "block",
                                        config.server.gameplay.sounds,
                                    )
                                    .await?;
                                    let mut outcome = gameplay::GameplayActionOutcome::default();
                                    outcome.grant_triggers.push(
                                        qexed_config::app::qexed::server::CustomAdvancementTrigger::Mine,
                                    );
                                    handle_gameplay_outcome(
                                        sink,
                                        players,
                                        profile.uuid,
                                        session.player.entity_id,
                                        &mut inventory,
                                        &mut gameplay_runtime,
                                        plugins,
                                        &session.player,
                                        &config.server.gameplay,
                                        &mut outcome,
                                    )
                                    .await?;
                                }
                                if !survival.is_dead() && was_destroyed {
                                    collect_nearby_drops(
                                        sink,
                                        world,
                                        world_rules,
                                        &config.server,
                                        world_config,
                                        &lobby,
                                        &lobby_status,
                                        players,
                                        plugins,
                                        cluster_entities,
                                        entities,
                                        profile.uuid,
                                        session.player.entity_id,
                                        &chunk_sender,
                                        &mut chunk_state,
                                        &mut position,
                                        &mut next_teleport_id,
                                        &mut play_dimension,
                                        &menus,
                                        &mut active_config_menu,
                                        &mut players_hidden,
                                        &mut visible_player_entities,
                                        &mut inventory,
                                        config.server.entity_rendering.player_distance,
                                        &mut geyser_runtime,
                                        simulation_distance,
                                    )
                                    .await?;
                                }
                            }
                        }
                        _ => {}
                    }
                    send_block_change_ack(sink, sequence).await?;
                    sink.flush().await?;
                    continue;
                }

                if packet_id == UseItem::ID {
                    let use_item = crate::connection::decode_payload::<UseItem>(&mut payload)?;
                    let sequence = use_item.sequence.clone();
                    if !survival.is_dead()
                        && handle_plugin_player_use_item(
                            sink,
                            world,
                            world_rules,
                            &config.server,
                            world_config,
                            entities,
                            players,
                            plugins,
                            &session.player,
                            "use_item",
                            if use_item.hand.0 == 1 {
                                "off_hand"
                            } else {
                                "main_hand"
                            },
                            use_item.sequence.0,
                            use_item.yaw,
                            use_item.pitch,
                            last_input_flags,
                            &mut inventory,
                            &chunk_sender,
                            &mut chunk_state,
                            &mut position,
                            &mut next_teleport_id,
                            &mut play_dimension,
                            &menus,
                            &mut active_config_menu,
                            &mut players_hidden,
                            &mut visible_player_entities,
                            config.server.entity_rendering.player_distance,
                            &mut geyser_runtime,
                        )
                        .await?
                    {
                        pending_dig = None;
                        send_block_change_ack(sink, sequence).await?;
                        sink.flush().await?;
                        continue;
                    }
                    if !survival.is_dead()
                        && use_item.hand.0 == 0
                        && let Some(action) = menus
                            .action_for_hotbar_item(inventory.selected_slot(), inventory.held_item())
                    {
                        let outcome = run_menu_action(
                            sink,
                            &menus,
                            players,
                            &mut players_hidden,
                            &mut visible_player_entities,
                            profile.uuid,
                            session.player.entity_id,
                            &play_dimension,
                            position,
                            config.server.entity_rendering.player_distance,
                            Some(&config.server),
                            plugins,
                            &lobby,
                            &lobby_status,
                            Some(&mut inventory),
                            Some(&mut geyser_runtime),
                            action,
                        )
                        .await?;
                        if outcome.opened_menu {
                            active_config_menu = outcome.opened_menu_id;
                        }
                        let deferred_actions = outcome.deferred_actions;
                        let ran_deferred_actions = !deferred_actions.is_empty();
                        apply_deferred_menu_actions(
                            sink,
                            world,
                            world_rules,
                            &config.server,
                            world_config,
                            entities,
                            players,
                            plugins,
                            profile.uuid,
                            &chunk_sender,
                            &mut chunk_state,
                            &mut position,
                            &mut next_teleport_id,
                            &mut play_dimension,
                            &menus,
                            &mut active_config_menu,
                            &mut players_hidden,
                            &mut visible_player_entities,
                            config.server.entity_rendering.player_distance,
                            &mut inventory,
                            &mut geyser_runtime,
                            session.player.entity_id,
                            deferred_actions,
                        )
                        .await?;
                        if ran_deferred_actions {
                            session.player.position = position;
                            session.player.dimension = play_dimension.clone();
                        }
                        pending_dig = None;
                        send_block_change_ack(sink, sequence).await?;
                        sink.flush().await?;
                        continue;
                    }
                    if !survival.is_dead()
                        && use_item.hand.0 == 0
                        && lobby
                            .handle_use_item(
                                sink,
                                inventory.selected_slot(),
                                inventory.held_item(),
                                &lobby_status,
                            )
                            .await?
                    {
                        lobby_menu_open = true;
                        pending_dig = None;
                    }
                    if !survival.is_dead()
                        && use_item.hand.0 == 0
                        && handle_gameplay_use_item(
                            sink,
                            world,
                            world_rules,
                            players,
                            plugins,
                            fluid,
                            entities,
                            world_config,
                            &session.player,
                            &config.server.gameplay,
                            current_game_mode,
                            &mut play_dimension,
                            &chunk_sender,
                            &mut chunk_state,
                            &mut position,
                            &mut inventory,
                            &mut survival,
                            &mut next_teleport_id,
                            &mut gameplay_runtime,
                            config.server.gameplay.drop_inventory_on_death,
                            &config.server.entity_rendering,
                        )
                        .await?
                    {
                        pending_dig = None;
                        session.player.position = position;
                        session.player.dimension = play_dimension.clone();
                    }
                    send_block_change_ack(sink, sequence).await?;
                    sink.flush().await?;
                    continue;
                }

                if packet_id == Interact::ID {
                    let interact = crate::connection::decode_payload::<Interact>(&mut payload)?;
                    if click_tracker.record_interact(plugins, &session.player, &interact)
                        && config.server.click_detection.cancel_actions
                    {
                        pending_dig = None;
                        sink.flush().await?;
                        continue;
                    }
                    if !survival.is_dead() {
                        let hand = npc_interact_hand(&interact);
                        let plugin_outcome = {
                            let viewer_position = position;
                            handle_plugin_npc_interact(
                                sink,
                                world,
                                world_rules,
                                &config.server,
                                world_config,
                                players,
                                plugins,
                                entities,
                                &session.player,
                                interact.entity_id.0,
                                hand.action,
                                hand.name,
                                &chunk_sender,
                                &mut chunk_state,
                                &mut position,
                                &mut next_teleport_id,
                                &mut play_dimension,
                                &menus,
                                &mut active_config_menu,
                                &mut players_hidden,
                                &mut visible_player_entities,
                                &mut inventory,
                                viewer_position,
                                config.server.entity_rendering.player_distance,
                                &mut geyser_runtime,
                            )
                            .await?
                        };
                        let mut config_outcome = NpcConfigActionOutcome::default();
                        if !plugin_outcome.handled {
                            config_outcome = run_config_npc_action(
                                sink,
                                config,
                                entities,
                                interact.entity_id.0,
                                hand.action,
                                &menus,
                                players,
                                &mut players_hidden,
                                &mut visible_player_entities,
                                profile.uuid,
                                &play_dimension,
                                position,
                                config.server.entity_rendering.player_distance,
                                Some(&config.server),
                            plugins,
                            &lobby,
                            &lobby_status,
                            Some(&mut geyser_runtime),
                        )
                        .await?;
                        }
                        if config_outcome.opened_menu {
                            active_config_menu = config_outcome.opened_menu_id;
                        }
                        if plugin_outcome.handled || config_outcome.handled {
                            pending_dig = None;
                            session.player.position = position;
                            session.player.dimension = play_dimension.clone();
                            visible_player_entities.clear();
                            if !players_hidden {
                                refresh_visible_players(
                                    sink,
                                    players,
                                    profile.uuid,
                                    player_entity_type,
                                    &play_dimension,
                                    position,
                                    config.server.entity_rendering.player_distance,
                                    &mut visible_player_entities,
                                )
                                .await?;
                            }
                            bootstrap::send_existing_entities(
                                sink,
                                entities,
                                &play_dimension,
                                position,
                                &config.server.entity_rendering,
                            )
                            .await?;
                            for packet in scoreboard::refresh_lobby_sidebar_packets(
                                &config.server.scoreboard,
                                &lobby,
                                &lobby_status,
                                config.server.placeholders.enable,
                                plugins,
                                &session.player,
                                players.online_count(),
                                config.server.max_player,
                            )? {
                                sink.send_raw(packet).await?;
                            }
                            sink.flush().await?;
                        }
                    }
                    if survival.is_dead() {
                        pending_dig = None;
                    }
                    continue;
                }

                if packet_id == ContainerButtonClick::ID {
                    let button = crate::connection::decode_payload::<ContainerButtonClick>(&mut payload)?;
                    let mut gameplay_outcome = gameplay::GameplayActionOutcome::default();
                    if let Some(outcome) = gameplay_runtime
                        .enchanting
                        .handle_button_click(
                            sink,
                            &button,
                            &mut inventory,
                            &session.player,
                            plugins,
                            config,
                            world,
                            &play_dimension,
                        )
                        .await?
                    {
                        gameplay_outcome.merge(outcome);
                    }
                    if gameplay_outcome.handled {
                        handle_gameplay_outcome(
                            sink,
                            players,
                            profile.uuid,
                            session.player.entity_id,
                            &mut inventory,
                            &mut gameplay_runtime,
                            plugins,
                            &session.player,
                            &config.server.gameplay,
                            &mut gameplay_outcome,
                        )
                        .await?;
                        pending_dig = None;
                        sink.flush().await?;
                        continue;
                    }
                }

                if packet_id == Attack::ID {
                    let attack = crate::connection::decode_payload::<Attack>(&mut payload)?;
                    if click_tracker.record_attack(plugins, &session.player, attack.entity_id.0)
                        && config.server.click_detection.cancel_actions
                    {
                        pending_dig = None;
                        sink.flush().await?;
                        continue;
                    }
                    if !survival.is_dead() {
                        let viewer_position = position;
                        let plugin_outcome = handle_plugin_npc_interact(
                            sink,
                            world,
                            world_rules,
                            &config.server,
                            world_config,
                            players,
                            plugins,
                            entities,
                            &session.player,
                            attack.entity_id.0,
                            "attack",
                            "attack",
                            &chunk_sender,
                            &mut chunk_state,
                            &mut position,
                            &mut next_teleport_id,
                            &mut play_dimension,
                            &menus,
                            &mut active_config_menu,
                            &mut players_hidden,
                            &mut visible_player_entities,
                            &mut inventory,
                            viewer_position,
                            config.server.entity_rendering.player_distance,
                            &mut geyser_runtime,
                        )
                        .await?;
                        if plugin_outcome.handled {
                            pending_dig = None;
                            session.player.position = position;
                            session.player.dimension = play_dimension.clone();
                            visible_player_entities.clear();
                            if !players_hidden {
                                refresh_visible_players(
                                    sink,
                                    players,
                                    profile.uuid,
                                    player_entity_type,
                                    &play_dimension,
                                    position,
                                    config.server.entity_rendering.player_distance,
                                    &mut visible_player_entities,
                                )
                                .await?;
                            }
                            bootstrap::send_existing_entities(
                                sink,
                                entities,
                                &play_dimension,
                                position,
                                &config.server.entity_rendering,
                            )
                            .await?;
                            for packet in scoreboard::refresh_lobby_sidebar_packets(
                                &config.server.scoreboard,
                                &lobby,
                                &lobby_status,
                                config.server.placeholders.enable,
                                plugins,
                                &session.player,
                                players.online_count(),
                                config.server.max_player,
                            )? {
                                sink.send_raw(packet).await?;
                            }
                            sink.flush().await?;
                            continue;
                        } else {
                            let config_outcome = run_config_npc_action(
                                sink,
                                config,
                                entities,
                                attack.entity_id.0,
                                "attack",
                                &menus,
                                players,
                                &mut players_hidden,
                                &mut visible_player_entities,
                                profile.uuid,
                                &play_dimension,
                                position,
                                config.server.entity_rendering.player_distance,
                                Some(&config.server),
                                plugins,
                                &lobby,
                                &lobby_status,
                                Some(&mut geyser_runtime),
                            )
                            .await?;
                            if config_outcome.opened_menu {
                                active_config_menu = config_outcome.opened_menu_id;
                            }
                            if config_outcome.handled {
                                pending_dig = None;
                                sink.flush().await?;
                                continue;
                            }
                        }
                        let combat_outcome = gameplay::combat::attack_entity(
                            sink,
                            players,
                            cluster_entities,
                            entities,
                            &session.player,
                            &mut inventory,
                            plugins,
                            &config.server.gameplay,
                            &gameplay_runtime.effects,
                            attack.entity_id.0,
                            &config.server.entity_rendering,
                            simulation_distance,
                        )
                        .await?;
                        if combat_outcome.handled {
                            for action in combat_outcome.actions {
                                let viewer_position = position;
                                let before_dimension = play_dimension.clone();
                                let _ = chat::apply_plugin_action(
                                    sink,
                                    Some(&config.server),
                                    world,
                                    world_rules,
                                    world_config,
                                    entities,
                                    players,
                                    plugins,
                                    profile.uuid,
                                    &chunk_sender,
                                    &mut chunk_state,
                                    &mut position,
                                    &mut next_teleport_id,
                                    &mut play_dimension,
                                    &menus,
                                    &mut active_config_menu,
                                    &mut players_hidden,
                                    &mut visible_player_entities,
                                    viewer_position,
                                    config.server.entity_rendering.player_distance,
                                    Some(&mut inventory),
                                    Some(&mut geyser_runtime),
                                    action,
                                )
                                .await?;
                                if before_dimension != play_dimension {
                                    resync_inventory_state(
                                        sink,
                                        players,
                                        profile.uuid,
                                        session.player.entity_id,
                                        &inventory,
                                    )
                                    .await?;
                                }
                            }
                            if combat_outcome.damaged_held_item {
                                gameplay::durability::damage_item(
                                    inventory.held_item_mut(),
                                    &session.player,
                                    plugins,
                                    &config.server.gameplay,
                                    "attack",
                                    1,
                                );
                                sync_inventory_changes(
                                    sink,
                                    players,
                                    profile.uuid,
                                    session.player.entity_id,
                                    inventory.selected_slot(),
                                    vec![inventory.selected_hotbar_change()],
                                )
                                .await?;
                            }
                            gameplay::sounds::play_at(
                                sink,
                                players,
                                plugins,
                                Some(&session.player),
                                &play_dimension,
                                position,
                                "minecraft:entity.player.attack.strong",
                                "player",
                                config.server.gameplay.sounds,
                            )
                            .await?;
                            let mut outcome = gameplay::GameplayActionOutcome::default();
                            outcome.grant_triggers.push(
                                qexed_config::app::qexed::server::CustomAdvancementTrigger::Attack,
                            );
                            if combat_outcome.killed {
                                outcome.grant_triggers.push(
                                    qexed_config::app::qexed::server::CustomAdvancementTrigger::Kill,
                                );
                            }
                            handle_gameplay_outcome(
                                sink,
                                players,
                                profile.uuid,
                                session.player.entity_id,
                                &mut inventory,
                                &mut gameplay_runtime,
                                plugins,
                                &session.player,
                                &config.server.gameplay,
                                &mut outcome,
                            )
                            .await?;
                            pending_dig = None;
                            sink.flush().await?;
                        }
                    }
                    if survival.is_dead() {
                        pending_dig = None;
                    }
                    continue;
                }

                if packet_id == ContainerClick::ID {
                    let click = crate::connection::decode_payload::<ContainerClick>(&mut payload)?;
                    let mut gameplay_outcome = gameplay::GameplayActionOutcome::default();
                    if let Some(outcome) = gameplay_runtime
                        .crafting
                        .handle_click(
                            sink,
                            &click,
                            &mut inventory,
                            &session.player,
                            plugins,
                            &config.server.gameplay,
                        )
                        .await?
                    {
                        gameplay_outcome.merge(outcome);
                    }
                    if let Some(outcome) = gameplay_runtime
                        .furnace
                        .handle_click(sink, &click, &mut inventory)
                        .await?
                    {
                        gameplay_outcome.merge(outcome);
                    }
                    if let Some(outcome) = gameplay_runtime
                        .enchanting
                        .handle_click(
                            sink,
                            &click,
                            &mut inventory,
                            &session.player,
                        plugins,
                        config,
                        world,
                        &play_dimension,
                    )
                        .await?
                    {
                        gameplay_outcome.merge(outcome);
                    }
                    if gameplay_outcome.handled {
                        handle_gameplay_outcome(
                            sink,
                            players,
                            profile.uuid,
                            session.player.entity_id,
                            &mut inventory,
                            &mut gameplay_runtime,
                            plugins,
                            &session.player,
                            &config.server.gameplay,
                            &mut gameplay_outcome,
                        )
                        .await?;
                        pending_dig = None;
                        sink.flush().await?;
                        continue;
                    }
                    let affects_fixed_menu_slot =
                        container_click_affects_fixed_menu_slot(&click, &menus, &inventory);
                    let affects_navigator_slot =
                        container_click_affects_navigator_slot(&click, &lobby, &inventory);
                    let menu_render_context = menus::MenuRenderContext::from_lobby(
                        Some(&config.server),
                        plugins,
                        players,
                        profile.uuid,
                        &lobby,
                        &lobby_status,
                    );
                    if let Some(action) = menus
                        .handle_container_click(
                            sink,
                            active_config_menu.as_deref(),
                            click.clone(),
                            Some(&menu_render_context),
                        )
                        .await?
                    {
                        let outcome = run_menu_action(
                            sink,
                            &menus,
                            players,
                            &mut players_hidden,
                            &mut visible_player_entities,
                            profile.uuid,
                            session.player.entity_id,
                            &play_dimension,
                            position,
                            config.server.entity_rendering.player_distance,
                            Some(&config.server),
                            plugins,
                            &lobby,
                            &lobby_status,
                            Some(&mut inventory),
                            Some(&mut geyser_runtime),
                            action,
                        )
                        .await?;
                        if outcome.opened_menu {
                            active_config_menu = outcome.opened_menu_id;
                        } else {
                            active_config_menu = None;
                            menus.close_menu(sink).await?;
                        }
                        let deferred_actions = outcome.deferred_actions;
                        let ran_deferred_actions = !deferred_actions.is_empty();
                        apply_deferred_menu_actions(
                            sink,
                            world,
                            world_rules,
                            &config.server,
                            world_config,
                            entities,
                            players,
                            plugins,
                            profile.uuid,
                            &chunk_sender,
                            &mut chunk_state,
                            &mut position,
                            &mut next_teleport_id,
                            &mut play_dimension,
                            &menus,
                            &mut active_config_menu,
                            &mut players_hidden,
                            &mut visible_player_entities,
                            config.server.entity_rendering.player_distance,
                            &mut inventory,
                            &mut geyser_runtime,
                            session.player.entity_id,
                            deferred_actions,
                        )
                        .await?;
                        if ran_deferred_actions {
                            session.player.position = position;
                            session.player.dimension = play_dimension.clone();
                        }
                        pending_dig = None;
                        resync_inventory_state(
                            sink,
                            players,
                            profile.uuid,
                            session.player.entity_id,
                            &mut inventory,
                        )
                        .await?;
                        sink.flush().await?;
                    } else {
                        let proxy_context = lobby::ProxyConnectContext {
                            server_config: &config.server,
                            plugins,
                            players,
                            actor: profile.uuid,
                        };
                        if lobby
                            .handle_container_click(
                                sink,
                                click,
                                &lobby_status,
                                Some(&proxy_context),
                            )
                            .await?
                        {
                            lobby_menu_open = true;
                            pending_dig = None;
                            sink.flush().await?;
                        } else if lobby.protect_world()
                            || affects_navigator_slot
                            || affects_fixed_menu_slot
                        {
                            pending_dig = None;
                            clear_carried_item(sink).await?;
                            resync_inventory_state(
                                sink,
                                players,
                                profile.uuid,
                                session.player.entity_id,
                                &inventory,
                            )
                            .await?;
                            sink.flush().await?;
                        }
                    }
                    continue;
                }

                if packet_id == ContainerClose::ID {
                    let close = crate::connection::decode_payload::<ContainerClose>(&mut payload)?;
                    log::debug!(
                        "client closed container: player={}, window_id={}",
                        profile.username,
                        close.window_id.0
                    );
                    let mut gameplay_outcome = gameplay::GameplayActionOutcome::default();
                    if close.window_id.0 == gameplay::crafting::CRAFTING_WINDOW_ID {
                        gameplay_outcome.merge(
                            gameplay_runtime
                                .crafting
                                .close(sink, &mut inventory)
                                .await?,
                        );
                    }
                    if close.window_id.0 == gameplay::furnace::FURNACE_WINDOW_ID {
                        gameplay_outcome.merge(
                            gameplay_runtime
                                .furnace
                                .close(sink, &mut inventory)
                                .await?,
                        );
                    }
                    if close.window_id.0 == gameplay::enchanting::ENCHANTING_WINDOW_ID {
                        gameplay_outcome.merge(
                            gameplay_runtime
                                .enchanting
                                .close(sink, &mut inventory)
                                .await?,
                        );
                    }
                    if gameplay_outcome.handled {
                        handle_gameplay_outcome(
                            sink,
                            players,
                            profile.uuid,
                            session.player.entity_id,
                            &mut inventory,
                            &mut gameplay_runtime,
                            plugins,
                            &session.player,
                            &config.server.gameplay,
                            &mut gameplay_outcome,
                        )
                        .await?;
                        pending_dig = None;
                        sink.flush().await?;
                        continue;
                    }
                    if close.window_id.0 == lobby::MENU_WINDOW_ID {
                        lobby_menu_open = false;
                    }
                    if close.window_id.0 == menus::MENU_WINDOW_ID {
                        active_config_menu = None;
                    }
                    continue;
                }

                if packet_id == ClientCommand::ID {
                    let command = crate::connection::decode_payload::<ClientCommand>(&mut payload)?;
                    if command.action.0
                        == qexed_protocol::to_server::play::client_command::PERFORM_RESPAWN
                    {
                        respawn_player(
                            sink,
                            world,
                            world_rules,
                            players,
                            plugins,
                            fluid,
                            world_config,
                            current_game_mode,
                            &mut play_dimension,
                            profile.uuid,
                            session.player.entity_id,
                            &chunk_sender,
                            &mut chunk_state,
                            &mut position,
                            &mut survival,
                            &mut inventory,
                            &mut next_teleport_id,
                        )
                        .await?;
                        session.player.position = position;
                        session.player.dimension = play_dimension.clone();
                    }
                    continue;
                }

                if packet_id == ServerboundPlayerAbilities::ID {
                    let abilities =
                        crate::connection::decode_payload::<ServerboundPlayerAbilities>(
                            &mut payload,
                        )?;
                    sink.send(ClientboundPlayerAbilities {
                        flags: acknowledged_player_ability_flags(
                            current_game_mode,
                            world_config.allow_flight,
                            abilities.flags,
                        ),
                        flying_speed: 0.05,
                        walking_speed: 0.1,
                    })
                    .await?;
                    continue;
                }

                if packet_id == MovePlayerPos::ID {
                    let movement = crate::connection::decode_payload::<MovePlayerPos>(&mut payload)?;
                    let previous = position;
                    position.x = movement.x;
                    position.y = movement.y;
                    position.z = movement.z;
                    position.on_ground = movement.flags & 0x01 != 0;
                    log_player_movement_observation(
                        &profile.username,
                        "pos",
                        &mut movement_observation_logs,
                        previous,
                        position,
                        None,
                    );
                    apply_survival_movement(
                        sink,
                        world,
                        world_rules,
                        players,
                        plugins,
                        fluid,
                        entities,
                        world_config,
                        &mut survival,
                        current_game_mode,
                        world_config.allow_flight,
                        &mut play_dimension,
                        profile.uuid,
                        session.player.entity_id,
                        &chunk_sender,
                        &mut chunk_state,
                        previous,
                        &mut position,
                        &mut inventory,
                        &mut next_teleport_id,
                        config.server.gameplay.drop_inventory_on_death,
                        &config.server.entity_rendering,
                    )
                    .await?;
                    if survival.is_dead() {
                        pending_dig = None;
                    } else if handle_plugin_player_block_step(
                        sink,
                        world,
                        world_rules,
                        &config.server,
                        world_config,
                        entities,
                        players,
                        plugins,
                        &session.player,
                        &chunk_sender,
                        &mut chunk_state,
                        &mut position,
                        &mut next_teleport_id,
                        &mut play_dimension,
                        &menus,
                        &mut active_config_menu,
                        &mut players_hidden,
                        &mut visible_player_entities,
                        config.server.entity_rendering.player_distance,
                        &mut last_stepped_block,
                        &mut inventory,
                        &mut geyser_runtime,
                    )
                    .await?
                    {
                        pending_dig = None;
                    }
                    if !survival.is_dead()
                        && handle_plugin_player_move(
                            sink,
                            world,
                            world_rules,
                            &config.server,
                            world_config,
                            entities,
                            players,
                            plugins,
                            &session.player,
                            previous,
                            &chunk_sender,
                            &mut chunk_state,
                            &mut position,
                            &mut next_teleport_id,
                            &mut play_dimension,
                            &menus,
                            &mut active_config_menu,
                            &mut players_hidden,
                            &mut visible_player_entities,
                            config.server.entity_rendering.player_distance,
                            &mut inventory,
                            &mut geyser_runtime,
                        )
                        .await?
                    {
                        pending_dig = None;
                    }
                    chunk_state
                        .update_center(sink, &chunk_sender, world, position.x, position.z)
                        .await?;
                    players.update_position(profile.uuid, position);
                    session.player.position = position;
                    session.player.dimension = play_dimension.clone();
                    refresh_cluster_entity_view(
                        sink,
                        cluster_entities,
                        &session.player,
                        &config.server.entity_rendering,
                        simulation_distance,
                    )
                    .await?;
                    entities.update_look_at_npcs(players, &config.server.entity_rendering)?;
                    if !players_hidden {
                        refresh_visible_players(
                            sink,
                            players,
                            profile.uuid,
                            player_entity_type,
                            &play_dimension,
                            position,
                            config.server.entity_rendering.player_distance,
                            &mut visible_player_entities,
                        )
                        .await?;
                    }
                    if !survival.is_dead() {
                        collect_nearby_drops(
                            sink,
                            world,
                            world_rules,
                            &config.server,
                            world_config,
                            &lobby,
                            &lobby_status,
                            players,
                            plugins,
                            cluster_entities,
                            entities,
                            profile.uuid,
                            session.player.entity_id,
                            &chunk_sender,
                            &mut chunk_state,
                            &mut position,
                            &mut next_teleport_id,
                            &mut play_dimension,
                            &menus,
                            &mut active_config_menu,
                            &mut players_hidden,
                            &mut visible_player_entities,
                            &mut inventory,
                            config.server.entity_rendering.player_distance,
                            &mut geyser_runtime,
                            simulation_distance,
                        )
                        .await?;
                    }
                    continue;
                }

                if packet_id == MovePlayerPosRot::ID {
                    let movement = crate::connection::decode_payload::<MovePlayerPosRot>(&mut payload)?;
                    let previous = position;
                    position.x = movement.x;
                    position.y = movement.y;
                    position.z = movement.z;
                    position.yaw = movement.yaw;
                    position.pitch = movement.pitch;
                    position.on_ground = movement.flags & 0x01 != 0;
                    log_player_movement_observation(
                        &profile.username,
                        "pos_rot",
                        &mut movement_observation_logs,
                        previous,
                        position,
                        None,
                    );
                    apply_survival_movement(
                        sink,
                        world,
                        world_rules,
                        players,
                        plugins,
                        fluid,
                        entities,
                        world_config,
                        &mut survival,
                        current_game_mode,
                        world_config.allow_flight,
                        &mut play_dimension,
                        profile.uuid,
                        session.player.entity_id,
                        &chunk_sender,
                        &mut chunk_state,
                        previous,
                        &mut position,
                        &mut inventory,
                        &mut next_teleport_id,
                        config.server.gameplay.drop_inventory_on_death,
                        &config.server.entity_rendering,
                    )
                    .await?;
                    if survival.is_dead() {
                        pending_dig = None;
                    } else if handle_plugin_player_block_step(
                        sink,
                        world,
                        world_rules,
                        &config.server,
                        world_config,
                        entities,
                        players,
                        plugins,
                        &session.player,
                        &chunk_sender,
                        &mut chunk_state,
                        &mut position,
                        &mut next_teleport_id,
                        &mut play_dimension,
                        &menus,
                        &mut active_config_menu,
                        &mut players_hidden,
                        &mut visible_player_entities,
                        config.server.entity_rendering.player_distance,
                        &mut last_stepped_block,
                        &mut inventory,
                        &mut geyser_runtime,
                    )
                    .await?
                    {
                        pending_dig = None;
                    }
                    if !survival.is_dead()
                        && handle_plugin_player_move(
                            sink,
                            world,
                            world_rules,
                            &config.server,
                            world_config,
                            entities,
                            players,
                            plugins,
                            &session.player,
                            previous,
                            &chunk_sender,
                            &mut chunk_state,
                            &mut position,
                            &mut next_teleport_id,
                            &mut play_dimension,
                            &menus,
                            &mut active_config_menu,
                            &mut players_hidden,
                            &mut visible_player_entities,
                            config.server.entity_rendering.player_distance,
                            &mut inventory,
                            &mut geyser_runtime,
                        )
                        .await?
                    {
                        pending_dig = None;
                    }
                    chunk_state
                        .update_center(sink, &chunk_sender, world, position.x, position.z)
                        .await?;
                    players.update_position(profile.uuid, position);
                    session.player.position = position;
                    session.player.dimension = play_dimension.clone();
                    refresh_cluster_entity_view(
                        sink,
                        cluster_entities,
                        &session.player,
                        &config.server.entity_rendering,
                        simulation_distance,
                    )
                    .await?;
                    entities.update_look_at_npcs(players, &config.server.entity_rendering)?;
                    if !players_hidden {
                        refresh_visible_players(
                            sink,
                            players,
                            profile.uuid,
                            player_entity_type,
                            &play_dimension,
                            position,
                            config.server.entity_rendering.player_distance,
                            &mut visible_player_entities,
                        )
                        .await?;
                    }
                    if !survival.is_dead() {
                        collect_nearby_drops(
                            sink,
                            world,
                            world_rules,
                            &config.server,
                            world_config,
                            &lobby,
                            &lobby_status,
                            players,
                            plugins,
                            cluster_entities,
                            entities,
                            profile.uuid,
                            session.player.entity_id,
                            &chunk_sender,
                            &mut chunk_state,
                            &mut position,
                            &mut next_teleport_id,
                            &mut play_dimension,
                            &menus,
                            &mut active_config_menu,
                            &mut players_hidden,
                            &mut visible_player_entities,
                            &mut inventory,
                            config.server.entity_rendering.player_distance,
                            &mut geyser_runtime,
                            simulation_distance,
                        )
                        .await?;
                    }
                    continue;
                }

                if packet_id == MovePlayerRot::ID {
                    let movement = crate::connection::decode_payload::<MovePlayerRot>(&mut payload)?;
                    let previous = position;
                    position.yaw = movement.yaw;
                    position.pitch = movement.pitch;
                    position.on_ground = movement.flags & 0x01 != 0;
                    log_player_movement_observation(
                        &profile.username,
                        "rot",
                        &mut movement_observation_logs,
                        previous,
                        position,
                        None,
                    );
                    apply_survival_movement(
                        sink,
                        world,
                        world_rules,
                        players,
                        plugins,
                        fluid,
                        entities,
                        world_config,
                        &mut survival,
                        current_game_mode,
                        world_config.allow_flight,
                        &mut play_dimension,
                        profile.uuid,
                        session.player.entity_id,
                        &chunk_sender,
                        &mut chunk_state,
                        previous,
                        &mut position,
                        &mut inventory,
                        &mut next_teleport_id,
                        config.server.gameplay.drop_inventory_on_death,
                        &config.server.entity_rendering,
                    )
                    .await?;
                    if survival.is_dead() {
                        pending_dig = None;
                    } else if handle_plugin_player_block_step(
                        sink,
                        world,
                        world_rules,
                        &config.server,
                        world_config,
                        entities,
                        players,
                        plugins,
                        &session.player,
                        &chunk_sender,
                        &mut chunk_state,
                        &mut position,
                        &mut next_teleport_id,
                        &mut play_dimension,
                        &menus,
                        &mut active_config_menu,
                        &mut players_hidden,
                        &mut visible_player_entities,
                        config.server.entity_rendering.player_distance,
                        &mut last_stepped_block,
                        &mut inventory,
                        &mut geyser_runtime,
                    )
                    .await?
                    {
                        pending_dig = None;
                    }
                    if !survival.is_dead()
                        && handle_plugin_player_move(
                            sink,
                            world,
                            world_rules,
                            &config.server,
                            world_config,
                            entities,
                            players,
                            plugins,
                            &session.player,
                            previous,
                            &chunk_sender,
                            &mut chunk_state,
                            &mut position,
                            &mut next_teleport_id,
                            &mut play_dimension,
                            &menus,
                            &mut active_config_menu,
                            &mut players_hidden,
                            &mut visible_player_entities,
                            config.server.entity_rendering.player_distance,
                            &mut inventory,
                            &mut geyser_runtime,
                        )
                        .await?
                    {
                        pending_dig = None;
                    }
                    players.update_position(profile.uuid, position);
                    session.player.position = position;
                    session.player.dimension = play_dimension.clone();
                    refresh_cluster_entity_view(
                        sink,
                        cluster_entities,
                        &session.player,
                        &config.server.entity_rendering,
                        simulation_distance,
                    )
                    .await?;
                    entities.update_look_at_npcs(players, &config.server.entity_rendering)?;
                    if !survival.is_dead() {
                        collect_nearby_drops(
                            sink,
                            world,
                            world_rules,
                            &config.server,
                            world_config,
                            &lobby,
                            &lobby_status,
                            players,
                            plugins,
                            cluster_entities,
                            entities,
                            profile.uuid,
                            session.player.entity_id,
                            &chunk_sender,
                            &mut chunk_state,
                            &mut position,
                            &mut next_teleport_id,
                            &mut play_dimension,
                            &menus,
                            &mut active_config_menu,
                            &mut players_hidden,
                            &mut visible_player_entities,
                            &mut inventory,
                            config.server.entity_rendering.player_distance,
                            &mut geyser_runtime,
                            simulation_distance,
                        )
                        .await?;
                    }
                    continue;
                }

                if packet_id == MovePlayerStatusOnly::ID {
                    let movement = crate::connection::decode_payload::<MovePlayerStatusOnly>(&mut payload)?;
                    let previous = position;
                    position.on_ground = movement.flags & 0x01 != 0;
                    log_player_movement_observation(
                        &profile.username,
                        "status",
                        &mut movement_observation_logs,
                        previous,
                        position,
                        None,
                    );
                    apply_survival_movement(
                        sink,
                        world,
                        world_rules,
                        players,
                        plugins,
                        fluid,
                        entities,
                        world_config,
                        &mut survival,
                        current_game_mode,
                        world_config.allow_flight,
                        &mut play_dimension,
                        profile.uuid,
                        session.player.entity_id,
                        &chunk_sender,
                        &mut chunk_state,
                        previous,
                        &mut position,
                        &mut inventory,
                        &mut next_teleport_id,
                        config.server.gameplay.drop_inventory_on_death,
                        &config.server.entity_rendering,
                    )
                    .await?;
                    if survival.is_dead() {
                        pending_dig = None;
                    } else if handle_plugin_player_block_step(
                        sink,
                        world,
                        world_rules,
                        &config.server,
                        world_config,
                        entities,
                        players,
                        plugins,
                        &session.player,
                        &chunk_sender,
                        &mut chunk_state,
                        &mut position,
                        &mut next_teleport_id,
                        &mut play_dimension,
                        &menus,
                        &mut active_config_menu,
                        &mut players_hidden,
                        &mut visible_player_entities,
                        config.server.entity_rendering.player_distance,
                        &mut last_stepped_block,
                        &mut inventory,
                        &mut geyser_runtime,
                    )
                    .await?
                    {
                        pending_dig = None;
                    }
                    if !survival.is_dead()
                        && handle_plugin_player_move(
                            sink,
                            world,
                            world_rules,
                            &config.server,
                            world_config,
                            entities,
                            players,
                            plugins,
                            &session.player,
                            previous,
                            &chunk_sender,
                            &mut chunk_state,
                            &mut position,
                            &mut next_teleport_id,
                            &mut play_dimension,
                            &menus,
                            &mut active_config_menu,
                            &mut players_hidden,
                            &mut visible_player_entities,
                            config.server.entity_rendering.player_distance,
                            &mut inventory,
                            &mut geyser_runtime,
                        )
                        .await?
                    {
                        pending_dig = None;
                    }
                    players.update_position(profile.uuid, position);
                    session.player.position = position;
                    session.player.dimension = play_dimension.clone();
                    refresh_cluster_entity_view(
                        sink,
                        cluster_entities,
                        &session.player,
                        &config.server.entity_rendering,
                        simulation_distance,
                    )
                    .await?;
                    entities.update_look_at_npcs(players, &config.server.entity_rendering)?;
                    if !survival.is_dead() {
                        collect_nearby_drops(
                            sink,
                            world,
                            world_rules,
                            &config.server,
                            world_config,
                            &lobby,
                            &lobby_status,
                            players,
                            plugins,
                            cluster_entities,
                            entities,
                            profile.uuid,
                            session.player.entity_id,
                            &chunk_sender,
                            &mut chunk_state,
                            &mut position,
                            &mut next_teleport_id,
                            &mut play_dimension,
                            &menus,
                            &mut active_config_menu,
                            &mut players_hidden,
                            &mut visible_player_entities,
                            &mut inventory,
                            config.server.entity_rendering.player_distance,
                            &mut geyser_runtime,
                            simulation_distance,
                        )
                        .await?;
                    }
                    continue;
                }

                if packet_id == PlayerInput::ID {
                    let input = crate::connection::decode_payload::<PlayerInput>(&mut payload)?;
                    log_player_movement_observation(
                        &profile.username,
                        "input",
                        &mut movement_observation_logs,
                        position,
                        position,
                        Some(input.flags),
                    );
                    if !survival.is_dead() {
                        let previous_flags = last_input_flags;
                        last_input_flags = input.flags;
                        if handle_plugin_player_input(
                            sink,
                            world,
                            world_rules,
                            &config.server,
                            world_config,
                            entities,
                            players,
                            plugins,
                            &session.player,
                            previous_flags,
                            input.flags,
                            &chunk_sender,
                            &mut chunk_state,
                            &mut position,
                            &mut next_teleport_id,
                            &mut play_dimension,
                            &menus,
                            &mut active_config_menu,
                            &mut players_hidden,
                            &mut visible_player_entities,
                            config.server.entity_rendering.player_distance,
                            &mut inventory,
                            &mut geyser_runtime,
                        )
                        .await?
                        {
                            pending_dig = None;
                        }
                    }
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
                    if let Some(message) =
                        validate_player_chat_message(&mut chat_rate_limit, &chat.message)
                    {
                        sink.send(SystemChat {
                            content: text_component(message),
                            overlay: false,
                        })
                        .await?;
                        sink.flush().await?;
                        continue;
                    }
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
                        let packet = PlayerChat::pass_through(
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
                        );
                        players.broadcast_packets_except(
                            profile.uuid,
                            vec![crate::players::packet_bytes(packet.clone())?],
                        );
                        sink.send(packet).await?;
                        chat_session.add_pending_signature(&verified.signature);
                        next_chat_global_index += 1;
                    } else if enforce_secure_chat {
                        anyhow::bail!("client did not initialize Mojang secure chat session");
                    } else {
                        let packet = SystemChat {
                            content: text_component(format!("<{}> {}", profile.username, filtered_message)),
                            overlay: false,
                        };
                        players.broadcast_packets_except(
                            profile.uuid,
                            vec![crate::players::packet_bytes(packet.clone())?],
                        );
                        sink.send(packet).await?;
                    }
                    sink.flush().await?;
                    continue;
                }

                if packet_id == ChatSessionUpdate::ID {
                    let update = crate::connection::decode_payload::<ChatSessionUpdate>(&mut payload)?;
                    log::debug!("received chat session update: session_id={}", update.chat_session.session_id);
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
                    log::debug!("received chat command: /{}", command.command);
                    player_audit.log_command(
                        profile,
                        &play_dimension,
                        &format!("/{}", command.command),
                    );
                    let outcome = handle_chat_command(
                        sink,
                        config,
                        world,
                        world_rules,
                        players,
                        fluid,
                        entities,
                        permissions,
                        plugins,
                        profile,
                        session.player.entity_id,
                        &command.command,
                        &lobby,
                        &mut lobby_status,
                        &menus,
                        &mut active_config_menu,
                        &mut players_hidden,
                        &mut visible_player_entities,
                        position,
                        config.server.entity_rendering.player_distance,
                        &chunk_sender,
                        &mut chunk_state,
                        &mut position,
                        &mut next_teleport_id,
                        &mut play_dimension,
                        &mut geyser_runtime,
                        &mut inventory,
                    ).await?;
                    if outcome.opened_lobby_menu {
                        lobby_menu_open = true;
                    }
                    if outcome.opened_config_menu {
                        active_config_menu = outcome.opened_config_menu_id;
                    }
                    if outcome.teleported {
                        lobby_menu_open = false;
                        pending_dig = None;
                        session.player.position = position;
                        session.player.dimension = play_dimension.clone();
                        visible_player_entities.clear();
                        if !players_hidden {
                            refresh_visible_players(
                                sink,
                                players,
                                profile.uuid,
                                player_entity_type,
                                &play_dimension,
                                position,
                                config.server.entity_rendering.player_distance,
                                &mut visible_player_entities,
                            )
                            .await?;
                        }
                    }
                    sink.flush().await?;
                    continue;
                }

                log::trace!("skipped unhandled Play serverbound packet ID: {packet_id}");
            }
            _ = keep_alive.tick() => {
                let _span = crate::profile_span!("net:keep_alive");
                if let Some(expected) = pending_keep_alive {
                    anyhow::bail!("client KeepAlive response timed out: id={expected}");
                }

                let keep_alive_id = keep_alive_id();
                sink.send(ClientboundKeepAlive { keep_alive_id }).await?;
                sink.flush().await?;
                pending_keep_alive = Some(keep_alive_id);
                log::debug!("sent Play KeepAlive: id={keep_alive_id}");
            }
            _ = async {
                if let Some(interval) = lobby_status_refresh.as_mut() {
                    interval.tick().await;
                }
            }, if lobby_status_refresh.is_some() => {
                let refreshed = lobby.refresh_status().await;
                for (server, status) in refreshed.changed_servers_since(&lobby_status) {
                    log::info!("lobby backend status changed: server={server}, status={status:?}");
                }
                lobby_status = refreshed;
                lobby.update_boss_bar_status(sink, &lobby_status).await?;
                for packet in scoreboard::refresh_lobby_sidebar_packets(
                    &config.server.scoreboard,
                    &lobby,
                    &lobby_status,
                    config.server.placeholders.enable,
                    plugins,
                    &session.player,
                    players.online_count(),
                    config.server.max_player,
                )? {
                    sink.send_raw(packet).await?;
                }
                if lobby_menu_open {
                    lobby.refresh_open_menu(sink, &lobby_status).await?;
                }
                sink.flush().await?;
            }
            _ = async {
                if let Some(interval) = lobby_broadcast.as_mut() {
                    interval.tick().await;
                }
            }, if lobby_broadcast.is_some() => {
                if let Some(message) = lobby.broadcast_message(lobby_broadcast_index, &lobby_status) {
                    let progress = lobby_broadcast_progress(
                        lobby_broadcast_index,
                        config.server.lobby.broadcast.messages.len(),
                    );
                    lobby.update_boss_bar(sink, &message, progress).await?;
                    sink.send(SystemChat {
                        content: text_component(message),
                        overlay: false,
                    }).await?;
                    sink.flush().await?;
                    lobby_broadcast_index = lobby_broadcast_index.wrapping_add(1);
                }
            }
            }
        }
    }
    .await;

    if let Err(err) = lobby.remove_boss_bar(sink).await {
        log::debug!("failed to remove lobby boss bar before disconnect: {err:#}");
    }
    save_player_runtime(
        player_data,
        player_data_lock,
        saved_player,
        &play_dimension,
        position,
        &inventory,
        survival,
        profile.uuid,
        "disconnect",
    )
    .await;
    world.flush_block_writes();

    result
}

async fn save_player_runtime(
    player_data: &PlayerDataManager,
    player_data_lock: &PlayerDataLockGuard,
    saved_player: &mut PlayerData,
    play_dimension: &str,
    position: EntityPosition,
    inventory: &crate::inventory::PlayerInventory,
    survival: SurvivalState,
    profile_id: uuid::Uuid,
    reason: &str,
) {
    saved_player.update_runtime(play_dimension, position, inventory, survival.to_stored());
    if let Err(err) = player_data
        .locked_save(player_data_lock, saved_player)
        .await
    {
        log::warn!("failed to save player data: uuid={profile_id}, reason={reason}, error={err:#}");
    } else {
        log::debug!("saved player data: uuid={profile_id}, reason={reason}");
    }
}

fn lobby_broadcast_progress(index: usize, message_count: usize) -> f32 {
    if message_count == 0 {
        return 1.0;
    }
    ((index % message_count) + 1) as f32 / message_count as f32
}

struct CommandSuggestionMatches {
    start: usize,
    length: usize,
    values: Vec<String>,
}

fn command_suggestion_matches(
    text: &str,
    players: &PlayerManager,
    lobby: &lobby::LobbyRuntime,
    lobby_status: &lobby::LobbyStatusSnapshot,
) -> CommandSuggestionMatches {
    let raw_token_start = text
        .char_indices()
        .rev()
        .find_map(|(index, ch)| ch.is_whitespace().then_some(index + ch.len_utf8()))
        .unwrap_or(0);
    let token_start = if raw_token_start == 0 && text.starts_with('/') {
        1
    } else {
        raw_token_start
    };
    let prefix = &text[token_start..];
    let lower_prefix = prefix.trim_start_matches('/').to_ascii_lowercase();
    let mut values = command_suggestion_candidates(text, players, lobby, lobby_status)
        .into_iter()
        .filter(|candidate| {
            candidate
                .trim_start_matches('/')
                .to_ascii_lowercase()
                .starts_with(&lower_prefix)
        })
        .collect::<Vec<_>>();
    values.sort();
    values.dedup();
    CommandSuggestionMatches {
        start: token_start,
        length: text.len().saturating_sub(token_start),
        values,
    }
}

fn command_suggestion_candidates(
    text: &str,
    players: &PlayerManager,
    lobby: &lobby::LobbyRuntime,
    lobby_status: &lobby::LobbyStatusSnapshot,
) -> Vec<String> {
    let trimmed = text.trim_start_matches('/').trim_start();
    let has_argument_separator = trimmed.chars().any(char::is_whitespace);
    let mut parts = trimmed.split_whitespace();
    if trimmed.is_empty() || !has_argument_separator {
        return crate::commands::command_names_for_suggestions()
            .into_iter()
            .map(ToString::to_string)
            .collect();
    }
    match parts
        .next()
        .map(crate::commands::normalize_command_name)
        .as_deref()
    {
        None | Some("") => Vec::new(),
        Some("teleport") => {
            let mut values = default_dimension_suggestions();
            values.extend(players.online_names());
            values
        }
        Some("gamemode") => {
            let mut values = vec![
                "survival".to_string(),
                "creative".to_string(),
                "adventure".to_string(),
                "spectator".to_string(),
            ];
            values.extend(players.online_names());
            values
        }
        Some("give") => {
            let mut values = players.online_names();
            values.extend(crate::inventory::item_id_name_map().into_values());
            values
        }
        Some("server") => lobby.server_targets_from_status(lobby_status),
        Some("entity") => ["list", "spawn", "move", "remove", "npc", "hologram"]
            .into_iter()
            .map(ToString::to_string)
            .collect(),
        Some("scoreboard") => [
            "objectives",
            "players",
            "sidebar",
            "list",
            "add",
            "remove",
            "setdisplay",
            "set",
            "reset",
            "on",
            "off",
            "reload",
        ]
        .into_iter()
        .map(ToString::to_string)
        .collect(),
        Some("time") | Some("gamerule") => default_dimension_suggestions(),
        _ => Vec::new(),
    }
}

async fn refresh_command_tree<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    permissions: &crate::permissions::PermissionManager,
    plugins: &crate::plugins::PluginManager,
    profile: &qexed_packet::net_types::GameProfile,
    lobby: &lobby::LobbyRuntime,
    lobby_status: &lobby::LobbyStatusSnapshot,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let lobby_server_ids = lobby.server_targets_from_status(lobby_status);
    let packet =
        command_tree_packet_for_lobby_servers(permissions, plugins, profile, &lobby_server_ids)
            .await?;
    sink.send_raw(packet).await?;
    Ok(())
}

pub(crate) async fn command_tree_packet_for_player(
    config: &crate::config::RuntimeConfig,
    permissions: &crate::permissions::PermissionManager,
    plugins: &crate::plugins::PluginManager,
    profile: &qexed_packet::net_types::GameProfile,
) -> Result<bytes::Bytes> {
    let lobby = lobby::LobbyRuntime::new(&config.server.lobby);
    let lobby_server_ids = lobby.server_targets();
    command_tree_packet_for_lobby_servers(permissions, plugins, profile, &lobby_server_ids).await
}

pub(super) async fn command_tree_packet_for_lobby_servers(
    permissions: &crate::permissions::PermissionManager,
    plugins: &crate::plugins::PluginManager,
    profile: &qexed_packet::net_types::GameProfile,
    lobby_server_ids: &[String],
) -> Result<bytes::Bytes> {
    let visible_commands = crate::commands::visible_commands(permissions, profile).await?;
    let mut plugin_commands = Vec::new();
    for command in plugins.plugin_commands() {
        if permissions.can_run_command(profile, &command.name).await? {
            plugin_commands.push(command.name);
        }
    }
    let command_tree = crate::commands::command_tree_for_lobby_with_extra(
        &visible_commands,
        lobby_server_ids,
        &plugin_commands,
    );
    Ok(qexed_tcp_connect::PacketSink::<tokio::io::Sink>::build_send_packet(command_tree)?)
}

fn default_dimension_suggestions() -> Vec<String> {
    vec![
        "minecraft:overworld".to_string(),
        "minecraft:the_nether".to_string(),
        "minecraft:the_end".to_string(),
    ]
}

fn game_mode_from_protocol_id(game_mode: i32) -> Option<GameMode> {
    match game_mode {
        0 => Some(GameMode::Survival),
        1 => Some(GameMode::Creative),
        2 => Some(GameMode::Adventure),
        3 => Some(GameMode::Spectator),
        _ => None,
    }
}

fn login_dimension_names(
    world_config: &qexed_config::app::qexed::server::World,
    primary: &str,
) -> Vec<String> {
    let mut dimensions = world_config.configured_dimension_names();
    if !dimensions.iter().any(|dimension| dimension == primary) {
        dimensions.push(primary.to_string());
    }
    dimensions.sort();
    dimensions.dedup();
    dimensions
}

#[allow(clippy::too_many_arguments)]
async fn apply_external_player_damage<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    world: &WorldManager,
    world_rules: &crate::world::WorldRulesManager,
    players: &PlayerManager,
    plugins: &crate::plugins::PluginManager,
    fluid: &FluidRuntime,
    entities: &crate::entities::EntityManager,
    world_config: &qexed_config::app::qexed::server::World,
    game_mode: GameMode,
    actor: uuid::Uuid,
    play_dimension: &mut String,
    collector_entity_id: i32,
    chunk_sender: &tokio::sync::mpsc::UnboundedSender<chunks::ChunkLoadResult>,
    chunk_state: &mut ChunkSendState,
    position: &mut EntityPosition,
    inventory: &mut crate::inventory::PlayerInventory,
    survival: &mut SurvivalState,
    next_teleport_id: &mut i32,
    amount: f32,
    kind: PlayerDamageKind,
    source_entity_id: i32,
    source_position: EntityPosition,
    knockback: f32,
    drop_inventory_on_death: bool,
    rendering: &qexed_config::app::qexed::server::EntityRendering,
) -> Result<bool>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    if game_mode != GameMode::Survival || survival.is_dead() || amount <= 0.0 || !amount.is_finite()
    {
        return Ok(false);
    }

    let message = external_damage_death_message(kind);
    let outcome = survival.apply_damage(amount, message);
    if !outcome.changed() {
        return Ok(false);
    }

    send_external_damage_feedback(
        sink,
        players,
        actor,
        collector_entity_id,
        *position,
        source_entity_id,
        source_position,
        knockback,
    )
    .await?;
    if outcome.death_message().is_some() {
        handle_external_player_death(
            sink,
            world,
            world_rules,
            players,
            plugins,
            fluid,
            entities,
            world_config,
            game_mode,
            actor,
            play_dimension,
            collector_entity_id,
            chunk_sender,
            chunk_state,
            position,
            inventory,
            survival,
            next_teleport_id,
            kind,
            source_entity_id,
            drop_inventory_on_death,
            rendering,
        )
        .await?;
    } else {
        sink.send(survival.health_packet()).await?;
    }
    sink.flush().await?;
    Ok(survival.is_dead())
}

#[allow(clippy::too_many_arguments)]
async fn apply_external_player_potion_effect<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    world: &WorldManager,
    world_rules: &crate::world::WorldRulesManager,
    players: &PlayerManager,
    plugins: &crate::plugins::PluginManager,
    fluid: &FluidRuntime,
    entities: &crate::entities::EntityManager,
    world_config: &qexed_config::app::qexed::server::World,
    game_mode: GameMode,
    actor: uuid::Uuid,
    play_dimension: &mut String,
    collector_entity_id: i32,
    chunk_sender: &tokio::sync::mpsc::UnboundedSender<chunks::ChunkLoadResult>,
    chunk_state: &mut ChunkSendState,
    position: &mut EntityPosition,
    inventory: &mut crate::inventory::PlayerInventory,
    survival: &mut SurvivalState,
    next_teleport_id: &mut i32,
    effects: &mut gameplay::effects::EffectRuntime,
    config: &qexed_config::app::qexed::server::Gameplay,
    effect: &str,
    amplifier: i32,
    duration_ticks: i32,
    source_entity_id: i32,
    source_position: EntityPosition,
    knockback: f32,
    rendering: &qexed_config::app::qexed::server::EntityRendering,
) -> Result<bool>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    if !config.potion_effects
        || game_mode != GameMode::Survival
        || survival.is_dead()
        || effect.trim().is_empty()
        || duration_ticks <= 0
    {
        return Ok(false);
    }

    if is_instant_potion_effect(effect) {
        let outcome = effects
            .apply_instant_effect(sink, collector_entity_id, effect, amplifier, survival)
            .await?;
        if outcome.changed() {
            send_external_damage_feedback(
                sink,
                players,
                actor,
                collector_entity_id,
                *position,
                source_entity_id,
                source_position,
                knockback,
            )
            .await?;
        }
        if outcome.death_message().is_some() {
            handle_external_player_death(
                sink,
                world,
                world_rules,
                players,
                plugins,
                fluid,
                entities,
                world_config,
                game_mode,
                actor,
                play_dimension,
                collector_entity_id,
                chunk_sender,
                chunk_state,
                position,
                inventory,
                survival,
                next_teleport_id,
                PlayerDamageKind::Magic,
                source_entity_id,
                config.drop_inventory_on_death,
                rendering,
            )
            .await?;
        }
    } else {
        effects
            .add_effect(sink, collector_entity_id, effect, amplifier, duration_ticks)
            .await?;
    }

    sink.flush().await?;
    Ok(survival.is_dead())
}

fn is_instant_potion_effect(effect: &str) -> bool {
    matches!(
        normalize_resource_key(effect).as_str(),
        "minecraft:instant_damage" | "minecraft:instant_health"
    )
}

#[allow(clippy::too_many_arguments)]
async fn handle_external_player_death<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    world: &WorldManager,
    world_rules: &crate::world::WorldRulesManager,
    players: &PlayerManager,
    plugins: &crate::plugins::PluginManager,
    fluid: &FluidRuntime,
    entities: &crate::entities::EntityManager,
    world_config: &qexed_config::app::qexed::server::World,
    game_mode: GameMode,
    actor: uuid::Uuid,
    play_dimension: &mut String,
    collector_entity_id: i32,
    chunk_sender: &tokio::sync::mpsc::UnboundedSender<chunks::ChunkLoadResult>,
    chunk_state: &mut ChunkSendState,
    position: &mut EntityPosition,
    inventory: &mut crate::inventory::PlayerInventory,
    survival: &mut SurvivalState,
    next_teleport_id: &mut i32,
    kind: PlayerDamageKind,
    source_entity_id: i32,
    drop_inventory_on_death: bool,
    rendering: &qexed_config::app::qexed::server::EntityRendering,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let message = external_damage_death_message(kind);
    let mut player =
        players
            .player_by_uuid(actor)
            .unwrap_or_else(|| crate::players::OnlinePlayer {
                profile: qexed_packet::net_types::GameProfile {
                    uuid: actor,
                    username: players.display_name(actor),
                    properties: Vec::new(),
                },
                entity_id: collector_entity_id,
                game_mode: game_mode.protocol_id() as i32,
                position: *position,
                dimension: play_dimension.clone(),
                equipment: Vec::new(),
                language: String::new(),
                displayed_skin_parts: crate::players::DEFAULT_DISPLAYED_SKIN_PARTS,
            });
    player.position = *position;
    player.dimension = play_dimension.clone();
    let response = plugins.handle_player_death(
        &player,
        message.translation_key().to_string(),
        source_entity_id,
    );
    if response.cancel {
        if !response.message.trim().is_empty() {
            sink.send(SystemChat {
                content: text_component(response.message),
                overlay: response.overlay,
            })
            .await?;
        }
        respawn_player(
            sink,
            world,
            world_rules,
            players,
            plugins,
            fluid,
            world_config,
            game_mode,
            play_dimension,
            actor,
            collector_entity_id,
            chunk_sender,
            chunk_state,
            position,
            survival,
            inventory,
            next_teleport_id,
        )
        .await?;
        return Ok(());
    }

    sink.send(survival.health_packet()).await?;
    broadcast_external_death_message(
        sink,
        players,
        entities,
        kind,
        source_entity_id,
        actor,
        collector_entity_id,
    )
    .await?;
    drop_player_inventory_on_death(
        sink,
        players,
        entities,
        game_mode,
        actor,
        play_dimension,
        *position,
        inventory,
        drop_inventory_on_death,
        rendering,
        collector_entity_id,
    )
    .await
}

async fn send_external_damage_feedback<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    players: &PlayerManager,
    actor: uuid::Uuid,
    collector_entity_id: i32,
    position: EntityPosition,
    source_entity_id: i32,
    source_position: EntityPosition,
    knockback: f32,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let damage_event = DamageEvent {
        entity_id: VarInt(collector_entity_id),
        source_type_id: VarInt(0),
        source_cause_id: VarInt(source_entity_id),
        source_direct_id: VarInt(source_entity_id),
        has_source_position: true,
        source_position: Some(DamageSourcePosition {
            x: source_position.x,
            y: source_position.y,
            z: source_position.z,
        }),
    };
    let hurt = HurtAnimation {
        entity_id: VarInt(collector_entity_id),
        yaw: damage_yaw_from_source(position, source_position),
    };
    let mut broadcast = vec![
        crate::players::packet_bytes(damage_event.clone())?,
        crate::players::packet_bytes(hurt.clone())?,
    ];
    sink.send(damage_event).await?;
    sink.send(hurt).await?;

    if knockback > 0.0 {
        let motion =
            external_damage_knockback(collector_entity_id, position, source_position, knockback);
        broadcast.push(crate::players::packet_bytes(motion.clone())?);
        sink.send(motion).await?;
    }

    players.broadcast_packets_except(actor, broadcast);
    Ok(())
}

fn external_damage_death_message(kind: PlayerDamageKind) -> DeathMessage {
    match kind {
        PlayerDamageKind::Explosion => DeathMessage::Explosion,
        PlayerDamageKind::Magic => DeathMessage::Magic,
        PlayerDamageKind::Generic | PlayerDamageKind::MobAttack | PlayerDamageKind::Projectile => {
            DeathMessage::Generic
        }
    }
}

fn damage_yaw_from_source(position: EntityPosition, source: EntityPosition) -> f32 {
    let dx = source.x - position.x;
    let dz = source.z - position.z;
    if dx.abs() <= f64::EPSILON && dz.abs() <= f64::EPSILON {
        return position.yaw;
    }
    (dz.atan2(dx).to_degrees() as f32) - 90.0
}

fn external_damage_knockback(
    entity_id: i32,
    position: EntityPosition,
    source: EntityPosition,
    strength: f32,
) -> SetEntityMotion {
    let dx = position.x - source.x;
    let dz = position.z - source.z;
    let length = (dx * dx + dz * dz).sqrt().max(0.0001);
    SetEntityMotion::from_velocity(
        entity_id,
        dx / length * f64::from(strength),
        0.35,
        dz / length * f64::from(strength),
    )
}

async fn apply_survival_movement<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    world: &WorldManager,
    world_rules: &crate::world::WorldRulesManager,
    players: &PlayerManager,
    plugins: &crate::plugins::PluginManager,
    fluid: &FluidRuntime,
    entities: &crate::entities::EntityManager,
    world_config: &qexed_config::app::qexed::server::World,
    survival: &mut SurvivalState,
    game_mode: GameMode,
    allow_flight: bool,
    play_dimension: &mut String,
    actor: uuid::Uuid,
    collector_entity_id: i32,
    chunk_sender: &tokio::sync::mpsc::UnboundedSender<chunks::ChunkLoadResult>,
    chunk_state: &mut ChunkSendState,
    previous: EntityPosition,
    current: &mut EntityPosition,
    inventory: &mut crate::inventory::PlayerInventory,
    next_teleport_id: &mut i32,
    drop_inventory_on_death: bool,
    rendering: &qexed_config::app::qexed::server::EntityRendering,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let fall_context = fall_context_at(world, play_dimension, *current);
    let outcome = if allow_flight {
        survival.apply_movement_with_flight(game_mode, true, previous, *current, fall_context)
    } else {
        survival.apply_movement(game_mode, previous, *current, fall_context)
    };
    if outcome.changed() {
        if let Some(message) = outcome.death_message() {
            handle_player_death(
                sink,
                world,
                world_rules,
                players,
                plugins,
                fluid,
                entities,
                world_config,
                game_mode,
                actor,
                play_dimension,
                collector_entity_id,
                chunk_sender,
                chunk_state,
                current,
                inventory,
                survival,
                next_teleport_id,
                message,
                drop_inventory_on_death,
                rendering,
                0,
            )
            .await?;
        } else {
            sink.send(survival.health_packet()).await?;
        }
        sink.flush().await?;
    }
    apply_fall_block_side_effects(
        sink,
        world,
        players,
        play_dimension,
        actor,
        previous,
        *current,
        fall_context,
    )
    .await?;
    Ok(())
}

async fn apply_survival_tick<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    world: &WorldManager,
    world_rules: &crate::world::WorldRulesManager,
    players: &PlayerManager,
    plugins: &crate::plugins::PluginManager,
    fluid: &FluidRuntime,
    entities: &crate::entities::EntityManager,
    world_config: &qexed_config::app::qexed::server::World,
    game_mode: GameMode,
    play_dimension: &mut String,
    actor: uuid::Uuid,
    collector_entity_id: i32,
    chunk_sender: &tokio::sync::mpsc::UnboundedSender<chunks::ChunkLoadResult>,
    chunk_state: &mut ChunkSendState,
    position: &mut EntityPosition,
    survival: &mut SurvivalState,
    inventory: &mut crate::inventory::PlayerInventory,
    next_teleport_id: &mut i32,
    drop_inventory_on_death: bool,
    rendering: &qexed_config::app::qexed::server::EntityRendering,
) -> Result<bool>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let outcome = survival.tick(game_mode, SURVIVAL_TICK_INTERVAL);
    if !outcome.changed() {
        return Ok(false);
    }

    if let Some(message) = outcome.death_message() {
        handle_player_death(
            sink,
            world,
            world_rules,
            players,
            plugins,
            fluid,
            entities,
            world_config,
            game_mode,
            actor,
            play_dimension,
            collector_entity_id,
            chunk_sender,
            chunk_state,
            position,
            inventory,
            survival,
            next_teleport_id,
            message,
            drop_inventory_on_death,
            rendering,
            0,
        )
        .await?;
    } else {
        sink.send(survival.health_packet()).await?;
    }
    sink.flush().await?;
    Ok(survival.is_dead())
}

async fn handle_player_death<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    world: &WorldManager,
    world_rules: &crate::world::WorldRulesManager,
    players: &PlayerManager,
    plugins: &crate::plugins::PluginManager,
    fluid: &FluidRuntime,
    entities: &crate::entities::EntityManager,
    world_config: &qexed_config::app::qexed::server::World,
    game_mode: GameMode,
    actor: uuid::Uuid,
    play_dimension: &mut String,
    collector_entity_id: i32,
    chunk_sender: &tokio::sync::mpsc::UnboundedSender<chunks::ChunkLoadResult>,
    chunk_state: &mut ChunkSendState,
    position: &mut EntityPosition,
    inventory: &mut crate::inventory::PlayerInventory,
    survival: &mut SurvivalState,
    next_teleport_id: &mut i32,
    message: DeathMessage,
    drop_inventory_on_death: bool,
    rendering: &qexed_config::app::qexed::server::EntityRendering,
    source_entity_id: i32,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let mut player =
        players
            .player_by_uuid(actor)
            .unwrap_or_else(|| crate::players::OnlinePlayer {
                profile: qexed_packet::net_types::GameProfile {
                    uuid: actor,
                    username: players.display_name(actor),
                    properties: Vec::new(),
                },
                entity_id: collector_entity_id,
                game_mode: game_mode.protocol_id() as i32,
                position: *position,
                dimension: play_dimension.clone(),
                equipment: Vec::new(),
                language: String::new(),
                displayed_skin_parts: crate::players::DEFAULT_DISPLAYED_SKIN_PARTS,
            });
    player.position = *position;
    player.dimension = play_dimension.clone();
    let response = plugins.handle_player_death(
        &player,
        message.translation_key().to_string(),
        source_entity_id,
    );
    if response.cancel {
        if !response.message.trim().is_empty() {
            sink.send(SystemChat {
                content: text_component(response.message),
                overlay: response.overlay,
            })
            .await?;
        }
        respawn_player(
            sink,
            world,
            world_rules,
            players,
            plugins,
            fluid,
            world_config,
            game_mode,
            play_dimension,
            actor,
            collector_entity_id,
            chunk_sender,
            chunk_state,
            position,
            survival,
            inventory,
            next_teleport_id,
        )
        .await?;
        return Ok(());
    }

    sink.send(survival.health_packet()).await?;
    broadcast_death_message(sink, players, message, actor, collector_entity_id).await?;
    drop_player_inventory_on_death(
        sink,
        players,
        entities,
        game_mode,
        actor,
        play_dimension,
        *position,
        inventory,
        drop_inventory_on_death,
        rendering,
        collector_entity_id,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn drop_player_inventory_on_death<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    players: &PlayerManager,
    entities: &crate::entities::EntityManager,
    game_mode: GameMode,
    actor: uuid::Uuid,
    dimension: &str,
    position: EntityPosition,
    inventory: &mut crate::inventory::PlayerInventory,
    drop_inventory_on_death: bool,
    rendering: &qexed_config::app::qexed::server::EntityRendering,
    collector_entity_id: i32,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    if game_mode != GameMode::Survival || !drop_inventory_on_death {
        return Ok(());
    }

    let (drops, changes) = inventory.drain_droppable_items();
    sync_inventory_changes(
        sink,
        players,
        actor,
        collector_entity_id,
        inventory.selected_slot(),
        changes,
    )
    .await?;
    for item in drops {
        for update in entities
            .drop_item_with_rendering(players, actor, dimension, position, item, rendering)?
        {
            send_dropped_item_update(sink, update).await?;
        }
    }
    Ok(())
}

async fn handle_gameplay_outcome<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    players: &PlayerManager,
    actor: uuid::Uuid,
    entity_id: i32,
    inventory: &mut crate::inventory::PlayerInventory,
    gameplay_runtime: &mut gameplay::GameplayRuntime,
    plugins: &crate::plugins::PluginManager,
    player: &crate::players::OnlinePlayer,
    config: &qexed_config::app::qexed::server::Gameplay,
    outcome: &mut gameplay::GameplayActionOutcome,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    if !outcome.inventory_changes.is_empty() {
        let changes = std::mem::take(&mut outcome.inventory_changes);
        sync_inventory_changes(
            sink,
            players,
            actor,
            entity_id,
            inventory.selected_slot(),
            changes,
        )
        .await?;
    }
    if !outcome.grant_triggers.is_empty() {
        let triggers = std::mem::take(&mut outcome.grant_triggers);
        gameplay_runtime
            .advancements
            .grant_triggers(sink, player, plugins, config, triggers)
            .await?;
    }
    Ok(())
}

async fn handle_gameplay_use_item<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    world: &WorldManager,
    world_rules: &crate::world::WorldRulesManager,
    players: &PlayerManager,
    plugins: &crate::plugins::PluginManager,
    fluid: &FluidRuntime,
    entities: &crate::entities::EntityManager,
    world_config: &qexed_config::app::qexed::server::World,
    player: &crate::players::OnlinePlayer,
    config: &qexed_config::app::qexed::server::Gameplay,
    game_mode: GameMode,
    play_dimension: &mut String,
    chunk_sender: &tokio::sync::mpsc::UnboundedSender<chunks::ChunkLoadResult>,
    chunk_state: &mut ChunkSendState,
    position: &mut EntityPosition,
    inventory: &mut crate::inventory::PlayerInventory,
    survival: &mut SurvivalState,
    next_teleport_id: &mut i32,
    gameplay_runtime: &mut gameplay::GameplayRuntime,
    drop_inventory_on_death: bool,
    rendering: &qexed_config::app::qexed::server::EntityRendering,
) -> Result<bool>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    if game_mode != GameMode::Survival {
        return Ok(false);
    }
    let held = inventory.held_item().clone();
    if held.item_count.0 <= 0 {
        return Ok(false);
    }
    let profile = consumable_profile(&held);
    let mut handled = false;
    let mut death_message = None;
    if let Some(food) = profile.food
        && survival.can_eat(food.can_always_eat)
        && survival.eat(food.nutrition, food.saturation_modifier)
    {
        handled = true;
    }
    if config.potion_effects {
        for effect in profile.effects {
            if matches!(
                effect.id.as_str(),
                "minecraft:instant_health" | "minecraft:instant_damage"
            ) {
                let damage = gameplay_runtime
                    .effects
                    .apply_instant_effect(
                        sink,
                        player.entity_id,
                        &effect.id,
                        effect.amplifier,
                        survival,
                    )
                    .await?;
                death_message = death_message.or_else(|| damage.death_message());
            } else {
                gameplay_runtime
                    .effects
                    .add_effect(
                        sink,
                        player.entity_id,
                        &effect.id,
                        effect.amplifier,
                        effect.duration_ticks,
                    )
                    .await?;
            }
            handled = true;
        }
    }
    if !handled {
        return Ok(false);
    }
    if let Some(change) = inventory.decrement_hotbar_slot(inventory.selected_slot(), 1) {
        sync_inventory_changes(
            sink,
            players,
            player.profile.uuid,
            player.entity_id,
            inventory.selected_slot(),
            vec![change],
        )
        .await?;
    }
    if let Some(message) = death_message {
        handle_player_death(
            sink,
            world,
            world_rules,
            players,
            plugins,
            fluid,
            entities,
            world_config,
            game_mode,
            player.profile.uuid,
            play_dimension,
            player.entity_id,
            chunk_sender,
            chunk_state,
            position,
            inventory,
            survival,
            next_teleport_id,
            message,
            drop_inventory_on_death,
            rendering,
            0,
        )
        .await?;
    } else {
        sink.send(survival.health_packet()).await?;
    }
    gameplay::sounds::play_at(
        sink,
        players,
        plugins,
        Some(player),
        &player.dimension,
        player.position,
        "minecraft:entity.generic.eat",
        "player",
        config.sounds,
    )
    .await?;
    Ok(true)
}

#[derive(Debug, Default)]
struct ConsumableProfile {
    food: Option<ConsumableFood>,
    effects: Vec<ConsumableEffect>,
}

#[derive(Debug, Clone, Copy)]
struct ConsumableFood {
    nutrition: i32,
    saturation_modifier: f32,
    can_always_eat: bool,
}

#[derive(Debug, Clone)]
struct ConsumableEffect {
    id: String,
    amplifier: i32,
    duration_ticks: i32,
}

fn consumable_profile(slot: &qexed_protocol::types::Slot) -> ConsumableProfile {
    let item_name = slot
        .item_id
        .as_ref()
        .and_then(|id| crate::inventory::item_name_for_id(id.0))
        .unwrap_or_default();
    let mut profile = fallback_consumable_profile(&item_name);
    if let Some(components) = slot.components_to_add.as_ref() {
        for component in components {
            match component {
                qexed_protocol::types::ComponentsToAdd::MinecraftFood(food) => {
                    profile.food = Some(ConsumableFood {
                        nutrition: food.nutrition.0,
                        saturation_modifier: food.saturation_modifier,
                        can_always_eat: food.can_always_eat,
                    });
                }
                qexed_protocol::types::ComponentsToAdd::MinecraftPotionContents(potion) => {
                    if let Some(potion_id) = &potion.potion_id {
                        profile.effects.extend(potion_effects_for_id(potion_id.0));
                    }
                }
                qexed_protocol::types::ComponentsToAdd::MinecraftSuspiciousStewEffects(stew) => {
                    profile
                        .effects
                        .extend(stew.effects.iter().filter_map(|effect| {
                            effect_name_for_id(effect.type_id.0).map(|id| ConsumableEffect {
                                id,
                                amplifier: 0,
                                duration_ticks: effect.duration.0.max(1),
                            })
                        }));
                }
                _ => {}
            }
        }
    }
    profile
}

fn fallback_consumable_profile(item_name: &str) -> ConsumableProfile {
    let mut profile = ConsumableProfile::default();
    if let Some(food) = fallback_food(item_name) {
        profile.food = Some(food);
    }
    match item_name {
        "minecraft:potion" => profile.effects.push(ConsumableEffect {
            id: "minecraft:water_breathing".to_string(),
            amplifier: 0,
            duration_ticks: 20 * 60 * 3,
        }),
        "minecraft:golden_apple" => {
            profile.effects.push(ConsumableEffect {
                id: "minecraft:regeneration".to_string(),
                amplifier: 1,
                duration_ticks: 20 * 5,
            });
            profile.effects.push(ConsumableEffect {
                id: "minecraft:absorption".to_string(),
                amplifier: 0,
                duration_ticks: 20 * 60 * 2,
            });
        }
        "minecraft:enchanted_golden_apple" => {
            profile.effects.push(ConsumableEffect {
                id: "minecraft:regeneration".to_string(),
                amplifier: 1,
                duration_ticks: 20 * 20,
            });
            profile.effects.push(ConsumableEffect {
                id: "minecraft:resistance".to_string(),
                amplifier: 0,
                duration_ticks: 20 * 60 * 5,
            });
            profile.effects.push(ConsumableEffect {
                id: "minecraft:fire_resistance".to_string(),
                amplifier: 0,
                duration_ticks: 20 * 60 * 5,
            });
            profile.effects.push(ConsumableEffect {
                id: "minecraft:absorption".to_string(),
                amplifier: 3,
                duration_ticks: 20 * 60 * 2,
            });
        }
        _ => {}
    }
    profile
}

fn fallback_food(item_name: &str) -> Option<ConsumableFood> {
    let (nutrition, saturation_modifier) = match item_name {
        "minecraft:apple" => (4, 0.3),
        "minecraft:bread" => (5, 0.6),
        "minecraft:cooked_beef" | "minecraft:cooked_porkchop" => (8, 0.8),
        "minecraft:cooked_chicken" | "minecraft:cooked_mutton" => (6, 0.6),
        "minecraft:cooked_cod" | "minecraft:cooked_salmon" => (5, 0.6),
        "minecraft:carrot" => (3, 0.6),
        "minecraft:potato" => (1, 0.3),
        "minecraft:baked_potato" => (5, 0.6),
        "minecraft:beetroot" => (1, 0.6),
        "minecraft:melon_slice" => (2, 0.3),
        "minecraft:pumpkin_pie" => (8, 0.3),
        "minecraft:cookie" => (2, 0.1),
        "minecraft:golden_apple" | "minecraft:enchanted_golden_apple" => (4, 1.2),
        "minecraft:golden_carrot" => (6, 1.2),
        "minecraft:raw_beef" | "minecraft:raw_porkchop" => (3, 0.3),
        "minecraft:raw_chicken" | "minecraft:raw_mutton" => (2, 0.3),
        "minecraft:rotten_flesh" => (4, 0.1),
        "minecraft:spider_eye" => (2, 0.8),
        _ => return None,
    };
    Some(ConsumableFood {
        nutrition,
        saturation_modifier,
        can_always_eat: matches!(
            item_name,
            "minecraft:golden_apple" | "minecraft:enchanted_golden_apple"
        ),
    })
}

fn potion_effects_for_id(potion_id: i32) -> Vec<ConsumableEffect> {
    let effect = match potion_id {
        1 => ("minecraft:regeneration", 0, 20 * 45),
        2 => ("minecraft:speed", 0, 20 * 180),
        3 => ("minecraft:fire_resistance", 0, 20 * 180),
        4 => ("minecraft:poison", 0, 20 * 45),
        5 => ("minecraft:instant_health", 0, 1),
        6 => ("minecraft:night_vision", 0, 20 * 180),
        7 => ("minecraft:weakness", 0, 20 * 90),
        8 => ("minecraft:strength", 0, 20 * 180),
        9 => ("minecraft:slowness", 0, 20 * 90),
        10 => ("minecraft:jump_boost", 0, 20 * 180),
        11 => ("minecraft:water_breathing", 0, 20 * 180),
        12 => ("minecraft:invisibility", 0, 20 * 180),
        13 => ("minecraft:instant_damage", 0, 1),
        14 => ("minecraft:slow_falling", 0, 20 * 90),
        _ => return Vec::new(),
    };
    vec![ConsumableEffect {
        id: effect.0.to_string(),
        amplifier: effect.1,
        duration_ticks: effect.2,
    }]
}

fn effect_name_for_id(effect_id: i32) -> Option<String> {
    Some(
        match effect_id {
            1 => "minecraft:speed",
            2 => "minecraft:slowness",
            3 => "minecraft:haste",
            4 => "minecraft:mining_fatigue",
            5 => "minecraft:strength",
            6 => "minecraft:instant_health",
            7 => "minecraft:instant_damage",
            8 => "minecraft:jump_boost",
            9 => "minecraft:nausea",
            10 => "minecraft:regeneration",
            11 => "minecraft:resistance",
            12 => "minecraft:fire_resistance",
            13 => "minecraft:water_breathing",
            14 => "minecraft:invisibility",
            15 => "minecraft:blindness",
            16 => "minecraft:night_vision",
            17 => "minecraft:hunger",
            18 => "minecraft:weakness",
            19 => "minecraft:poison",
            20 => "minecraft:wither",
            21 => "minecraft:health_boost",
            22 => "minecraft:absorption",
            23 => "minecraft:saturation",
            24 => "minecraft:glowing",
            25 => "minecraft:levitation",
            26 => "minecraft:luck",
            27 => "minecraft:unluck",
            28 => "minecraft:slow_falling",
            29 => "minecraft:conduit_power",
            30 => "minecraft:dolphins_grace",
            31 => "minecraft:bad_omen",
            32 => "minecraft:hero_of_the_village",
            33 => "minecraft:darkness",
            _ => return None,
        }
        .to_string(),
    )
}

fn fall_context_at(world: &WorldManager, dimension: &str, position: EntityPosition) -> FallContext {
    let landing_position = BlockPosition {
        x: position.x.floor() as i32,
        y: (position.y - 0.2).floor() as i32,
        z: position.z.floor() as i32,
    };
    let body_position = BlockPosition {
        x: position.x.floor() as i32,
        y: position.y.floor() as i32,
        z: position.z.floor() as i32,
    };
    let head_position = BlockPosition {
        x: position.x.floor() as i32,
        y: (position.y + PLAYER_HEIGHT_BLOCKS - 0.01).floor() as i32,
        z: position.z.floor() as i32,
    };

    let landing_state = world
        .block_state_at(dimension, &landing_position)
        .unwrap_or_else(crate::inventory::air_block_state);
    let body_state = world
        .block_state_at(dimension, &body_position)
        .unwrap_or_else(crate::inventory::air_block_state);
    let head_state = world
        .block_state_at(dimension, &head_position)
        .unwrap_or_else(crate::inventory::air_block_state);

    let landing_name = block_name(landing_state);
    let body_name = block_name(body_state);
    let head_name = block_name(head_state);
    let in_lava = is_lava_block(&body_name) || is_lava_block(&head_name);

    FallContext {
        in_lava,
        landing: fall_landing(landing_state, &landing_name),
        climbable: fall_location_for_climbable(&body_name)
            .or_else(|| fall_location_for_climbable(&head_name)),
    }
}

async fn apply_fall_block_side_effects<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    world: &WorldManager,
    players: &PlayerManager,
    dimension: &str,
    actor: uuid::Uuid,
    previous: EntityPosition,
    position: EntityPosition,
    fall_context: FallContext,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let fall_distance = (previous.y - position.y).max(0.0);
    if previous.on_ground || !position.on_ground || fall_distance <= 0.5 {
        return Ok(());
    }
    if fall_context.in_lava {
        return Ok(());
    }

    let landing_position = BlockPosition {
        x: position.x.floor() as i32,
        y: (position.y - 0.2).floor() as i32,
        z: position.z.floor() as i32,
    };
    let landing_state = world
        .block_state_at(dimension, &landing_position)
        .unwrap_or_else(crate::inventory::air_block_state);
    if block_name(landing_state) != "minecraft:farmland" {
        return Ok(());
    }

    let dirt = crate::world::chunk_nbt::default_block_state("minecraft:dirt").id;
    world.place_block(dimension, landing_position.clone(), dirt);
    sink.send(crate::inventory::block_update(
        landing_position.clone(),
        dirt,
    ))
    .await?;
    players.broadcast_block_changed(actor, dimension, landing_position, dirt, None);
    Ok(())
}

fn fall_landing(block_state: i32, name: &str) -> FallLanding {
    match name {
        "minecraft:bed" => FallLanding::Bed,
        "minecraft:hay_block" => FallLanding::Hay,
        "minecraft:honey_block" => FallLanding::Honey,
        "minecraft:slime_block" => FallLanding::Slime,
        "minecraft:powder_snow" => FallLanding::PowderSnow,
        "minecraft:pointed_dripstone"
            if block_state_property(block_state, "vertical_direction").as_deref() == Some("up")
                && block_state_property(block_state, "thickness").as_deref() == Some("tip") =>
        {
            FallLanding::PointedDripstoneTip
        }
        _ => FallLanding::Generic,
    }
}

fn fall_location_for_climbable(name: &str) -> Option<FallLocation> {
    match name {
        "minecraft:ladder" => Some(FallLocation::Ladder),
        "minecraft:vine" => Some(FallLocation::Vines),
        "minecraft:weeping_vines" | "minecraft:weeping_vines_plant" => {
            Some(FallLocation::WeepingVines)
        }
        "minecraft:twisting_vines" | "minecraft:twisting_vines_plant" => {
            Some(FallLocation::TwistingVines)
        }
        "minecraft:scaffolding" => Some(FallLocation::Scaffolding),
        name if name.ends_with("_trapdoor") => Some(FallLocation::Ladder),
        name if is_climbable_block(name) => Some(FallLocation::OtherClimbable),
        _ => None,
    }
}

fn is_climbable_block(name: &str) -> bool {
    matches!(
        name,
        "minecraft:cave_vines"
            | "minecraft:cave_vines_plant"
            | "minecraft:nether_vines"
            | "minecraft:chain"
    )
}

fn is_lava_block(name: &str) -> bool {
    name == "minecraft:lava"
}

fn block_name(block_state: i32) -> String {
    crate::inventory::block_name_for_state(block_state)
        .unwrap_or_else(|| crate::world::chunk_nbt::block_state_entry(block_state).name)
}

fn block_name_at(
    world: &WorldManager,
    dimension: &str,
    position: &BlockPosition,
) -> Option<String> {
    world.block_state_at(dimension, position).map(block_name)
}

fn block_state_property(block_state: i32, key: &str) -> Option<String> {
    let entry = crate::world::chunk_nbt::block_state_entry(block_state);
    entry
        .properties
        .iter()
        .find_map(|(name, value)| (name == key).then_some(value.clone()))
}

fn block_state_with_property(block_state: i32, key: &str, value: &str) -> Option<i32> {
    let entry = crate::world::chunk_nbt::block_state_entry(block_state);
    let mut properties = entry.properties;
    let mut found = false;
    for (property_key, property_value) in &mut properties {
        if property_key == key {
            *property_value = value.to_string();
            found = true;
            break;
        }
    }
    found.then(|| crate::world::chunk_nbt::block_state(&entry.name, &properties).id)
}

fn block_state_with_name_and_overlapping_properties(
    block_state: i32,
    target_name: &str,
) -> Option<i32> {
    let source = crate::world::chunk_nbt::block_state_entry(block_state);
    let mut target = crate::world::chunk_nbt::default_block_state(target_name).properties;
    for (target_key, target_value) in &mut target {
        if let Some((_, source_value)) = source
            .properties
            .iter()
            .find(|(source_key, _)| source_key == target_key)
        {
            *target_value = source_value.clone();
        }
    }
    block_state_with_exact_properties(target_name, &target)
}

fn block_state_with_exact_properties(
    block_name: &str,
    properties: &[(String, String)],
) -> Option<i32> {
    let state = crate::world::chunk_nbt::block_state(block_name, properties).id;
    let entry = crate::world::chunk_nbt::block_state_entry(state);
    (entry.name == normalize_resource_key(block_name) && entry.properties == properties)
        .then_some(state)
}

fn block_state_bool_property_exists(block_state: i32, key: &str) -> bool {
    block_state_property(block_state, key).is_some()
}

fn block_state_u8_property(block_state: i32, key: &str) -> Option<u8> {
    block_state_property(block_state, key).and_then(|value| value.parse().ok())
}

fn bool_value(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}

fn block_name_for_manual_state(block_state: i32) -> String {
    block_name(block_state)
}

async fn broadcast_death_message<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    players: &PlayerManager,
    message: DeathMessage,
    actor: uuid::Uuid,
    collector_entity_id: i32,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let packet = SystemChat {
        content: translatable_component(
            message.translation_key(),
            vec![text_component(players.display_name(actor))],
        ),
        overlay: false,
    };
    let bytes = crate::players::packet_bytes(packet.clone())?;
    sink.send(packet).await?;
    players.broadcast_packets_except(actor, vec![bytes]);
    log::debug!(
        "player died: entity_id={}, message={}",
        collector_entity_id,
        message.translation_key()
    );
    Ok(())
}

async fn broadcast_external_death_message<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    players: &PlayerManager,
    entities: &crate::entities::EntityManager,
    kind: PlayerDamageKind,
    source_entity_id: i32,
    actor: uuid::Uuid,
    collector_entity_id: i32,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let actor_name = text_component(players.display_name(actor));
    let source_name = external_damage_source_name(entities, source_entity_id);
    let (key, with) = match (kind, source_name) {
        (PlayerDamageKind::MobAttack, Some(source)) => {
            ("death.attack.mob", vec![actor_name, text_component(source)])
        }
        (PlayerDamageKind::Projectile, Some(source)) => (
            "death.attack.arrow",
            vec![actor_name, text_component(source)],
        ),
        (PlayerDamageKind::Explosion, Some(source)) => (
            "death.attack.explosion.player",
            vec![actor_name, text_component(source)],
        ),
        (PlayerDamageKind::Explosion, None) => ("death.attack.explosion", vec![actor_name]),
        (PlayerDamageKind::Magic, _) => ("death.attack.magic", vec![actor_name]),
        _ => ("death.attack.generic", vec![actor_name]),
    };
    let packet = SystemChat {
        content: translatable_component(key, with),
        overlay: false,
    };
    let bytes = crate::players::packet_bytes(packet.clone())?;
    sink.send(packet).await?;
    players.broadcast_packets_except(actor, vec![bytes]);
    log::debug!("player died: entity_id={collector_entity_id}, message={key}");
    Ok(())
}

fn external_damage_source_name(
    entities: &crate::entities::EntityManager,
    source_entity_id: i32,
) -> Option<String> {
    let entity = entities.entity_by_runtime_id(source_entity_id)?;
    if !entity.display_name.trim().is_empty() {
        return Some(entity.display_name);
    }
    if !entity.name.trim().is_empty() {
        return Some(entity.name);
    }
    Some(
        entity
            .entity_type
            .trim_start_matches("minecraft:")
            .to_string(),
    )
}

async fn respawn_player<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    world: &WorldManager,
    world_rules: &crate::world::WorldRulesManager,
    players: &PlayerManager,
    plugins: &crate::plugins::PluginManager,
    fluid: &FluidRuntime,
    world_config: &qexed_config::app::qexed::server::World,
    game_mode: GameMode,
    play_dimension: &mut String,
    actor: uuid::Uuid,
    entity_id: i32,
    chunk_sender: &tokio::sync::mpsc::UnboundedSender<chunks::ChunkLoadResult>,
    chunk_state: &mut ChunkSendState,
    position: &mut EntityPosition,
    survival: &mut SurvivalState,
    inventory: &mut crate::inventory::PlayerInventory,
    next_teleport_id: &mut i32,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    if !survival.is_dead() {
        return Ok(());
    }

    survival.respawn();
    *position = spawn_position(&world_config.spawn);
    let respawn_dimension = world_config.default_play_dimension();
    world_rules.ensure_loaded(&respawn_dimension)?;
    let dimension_rule = world_rules.snapshot(&respawn_dimension);
    sink.send(Respawn {
        dimension_type: VarInt(dimension_type_holder_id(&dimension_rule.dimension_type)),
        dimension_name: respawn_dimension.clone(),
        hashed_seed: 0,
        game_mode: game_mode.protocol_id(),
        previous_game_mode: -1,
        is_debug: false,
        is_flat: true,
        has_death_location: false,
        death_dimension_name: None,
        death_position: None,
        portal_cooldown: VarInt(0),
        sea_level: VarInt(63),
        data_to_keep: KEEP_NO_DATA,
    })
    .await?;
    let teleport_id = *next_teleport_id;
    *next_teleport_id = next_teleport_id.saturating_add(1);
    sink.send(Position {
        teleport_id: VarInt(teleport_id),
        x: position.x,
        y: position.y,
        z: position.z,
        dx: 0.0,
        dy: 0.0,
        dz: 0.0,
        yaw: position.yaw,
        pitch: position.pitch,
        flags: 0,
    })
    .await?;
    *play_dimension = respawn_dimension.clone();
    send_respawn_player_state(sink, world_config, world_rules, play_dimension, *position).await?;
    sink.send(survival.health_packet()).await?;
    let fluid_seeds = chunk_state
        .reset_dimension_after_respawn(
            sink,
            chunk_sender,
            world,
            plugins,
            play_dimension.clone(),
            position.x,
            position.z,
        )
        .await?;
    fluid.enqueue_fluid_seeds(play_dimension, fluid_seeds);
    resync_inventory_state(sink, players, actor, entity_id, inventory).await?;
    players.update_position_and_dimension(actor, play_dimension.clone(), *position);
    sink.flush().await?;
    Ok(())
}

async fn teleport_to_spawn<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    world: &WorldManager,
    world_rules: &crate::world::WorldRulesManager,
    players: &PlayerManager,
    plugins: &crate::plugins::PluginManager,
    fluid: &FluidRuntime,
    world_config: &qexed_config::app::qexed::server::World,
    dimension: &str,
    actor: uuid::Uuid,
    chunk_sender: &tokio::sync::mpsc::UnboundedSender<chunks::ChunkLoadResult>,
    chunk_state: &mut ChunkSendState,
    position: &mut EntityPosition,
    next_teleport_id: &mut i32,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    *position = spawn_position(&world_config.spawn);
    let teleport_id = *next_teleport_id;
    *next_teleport_id = next_teleport_id.saturating_add(1);
    sink.send(Position {
        teleport_id: VarInt(teleport_id),
        x: position.x,
        y: position.y,
        z: position.z,
        dx: 0.0,
        dy: 0.0,
        dz: 0.0,
        yaw: position.yaw,
        pitch: position.pitch,
        flags: 0,
    })
    .await?;
    send_respawn_player_state(sink, world_config, world_rules, dimension, *position).await?;
    let fluid_seeds = chunk_state
        .reset_after_respawn(sink, chunk_sender, world, plugins, position.x, position.z)
        .await?;
    fluid.enqueue_fluid_seeds(dimension, fluid_seeds);
    players.update_position(actor, *position);
    Ok(())
}

#[derive(Debug, Default, Clone, Copy)]
struct PluginNpcInteractOutcome {
    handled: bool,
}

struct NpcInteractHand {
    name: &'static str,
    action: &'static str,
}

struct ClickTracker {
    enable: bool,
    window: Duration,
    max_clicks: u32,
    clicks: std::collections::VecDeque<Instant>,
}

impl ClickTracker {
    fn new(config: &qexed_config::app::qexed::server::ClickDetection) -> Self {
        Self {
            enable: config.enable,
            window: Duration::from_millis(config.window_ms.max(1)),
            max_clicks: config.max_clicks.max(1),
            clicks: std::collections::VecDeque::new(),
        }
    }

    fn record_player_action(
        &mut self,
        plugins: &crate::plugins::PluginManager,
        player: &crate::players::OnlinePlayer,
        status: i32,
    ) -> bool {
        if !matches!(
            status,
            PLAYER_ACTION_START_DESTROY_BLOCK
                | PLAYER_ACTION_DROP_ITEM
                | PLAYER_ACTION_DROP_ITEM_STACK
        ) {
            return false;
        }
        self.record(plugins, player, player_action_label(status))
    }

    fn record_interact(
        &mut self,
        plugins: &crate::plugins::PluginManager,
        player: &crate::players::OnlinePlayer,
        interact: &Interact,
    ) -> bool {
        self.record(plugins, player, npc_interact_hand(interact).action)
    }

    fn record_attack(
        &mut self,
        plugins: &crate::plugins::PluginManager,
        player: &crate::players::OnlinePlayer,
        _entity_id: i32,
    ) -> bool {
        self.record(plugins, player, "attack")
    }

    fn record(
        &mut self,
        plugins: &crate::plugins::PluginManager,
        player: &crate::players::OnlinePlayer,
        action: &'static str,
    ) -> bool {
        if !self.enable {
            return false;
        }
        let now = Instant::now();
        self.clicks.push_back(now);
        while self
            .clicks
            .front()
            .is_some_and(|time| now.duration_since(*time) > self.window)
        {
            self.clicks.pop_front();
        }
        let clicks = u32::try_from(self.clicks.len()).unwrap_or(u32::MAX);
        if clicks <= self.max_clicks {
            return false;
        }
        plugins.emit_click_detected(
            player,
            action.to_string(),
            clicks,
            self.window.as_millis() as u64,
        );
        true
    }
}

#[derive(Debug, Default)]
struct MenuActionOutcome {
    opened_menu: bool,
    opened_menu_id: Option<String>,
    deferred_actions: Vec<crate::plugins::PlayerAction>,
}

#[derive(Debug, Default)]
struct NpcConfigActionOutcome {
    handled: bool,
    opened_menu: bool,
    opened_menu_id: Option<String>,
}

async fn run_config_npc_action<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    config: &crate::config::RuntimeConfig,
    entities: &crate::entities::EntityManager,
    entity_id: i32,
    action_name: &str,
    menus: &menus::MenuRuntime,
    players: &PlayerManager,
    players_hidden: &mut bool,
    visible_player_entities: &mut HashSet<uuid::Uuid>,
    actor: uuid::Uuid,
    dimension: &str,
    viewer_position: EntityPosition,
    render_distance: f64,
    server_config: Option<&qexed_config::app::qexed::server::Server>,
    plugins: &crate::plugins::PluginManager,
    lobby: &lobby::LobbyRuntime,
    lobby_status: &lobby::LobbyStatusSnapshot,
    geyser_runtime: Option<&mut geyser::GeyserRuntime>,
) -> Result<NpcConfigActionOutcome>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let Some(entity) = entities.entity_by_runtime_id(entity_id) else {
        return Ok(NpcConfigActionOutcome::default());
    };
    if entity.kind != crate::entities::ManagedEntityKind::Npc {
        return Ok(NpcConfigActionOutcome::default());
    }
    let Some(npc) = config
        .npcs
        .list
        .iter()
        .find(|npc| npc.id.trim() == entity.key)
    else {
        return Ok(NpcConfigActionOutcome::default());
    };
    let action = match action_name {
        "interact" => npc.actions.main_hand.clone(),
        "interact_off_hand" => npc.actions.off_hand.clone(),
        "attack" => npc.actions.attack.clone(),
        _ => qexed_config::app::qexed::server::MenuAction::default(),
    };
    if action.kind == qexed_config::app::qexed::server::MenuActionKind::None {
        return Ok(NpcConfigActionOutcome::default());
    }

    let outcome = run_menu_action(
        sink,
        menus,
        players,
        players_hidden,
        visible_player_entities,
        actor,
        0,
        dimension,
        viewer_position,
        render_distance,
        server_config,
        plugins,
        lobby,
        lobby_status,
        None,
        geyser_runtime,
        action,
    )
    .await?;
    Ok(NpcConfigActionOutcome {
        handled: true,
        opened_menu: outcome.opened_menu,
        opened_menu_id: outcome.opened_menu_id,
    })
}

async fn run_menu_action<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    menus: &menus::MenuRuntime,
    players: &PlayerManager,
    players_hidden: &mut bool,
    visible_player_entities: &mut HashSet<uuid::Uuid>,
    actor: uuid::Uuid,
    actor_entity_id: i32,
    dimension: &str,
    viewer_position: EntityPosition,
    render_distance: f64,
    server_config: Option<&qexed_config::app::qexed::server::Server>,
    plugins: &crate::plugins::PluginManager,
    lobby: &lobby::LobbyRuntime,
    lobby_status: &lobby::LobbyStatusSnapshot,
    mut inventory: Option<&mut crate::inventory::PlayerInventory>,
    mut geyser_runtime: Option<&mut geyser::GeyserRuntime>,
    action: qexed_config::app::qexed::server::MenuAction,
) -> Result<MenuActionOutcome>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    match action.kind {
        qexed_config::app::qexed::server::MenuActionKind::None => Ok(MenuActionOutcome::default()),
        qexed_config::app::qexed::server::MenuActionKind::OpenMenu => {
            let render_context = menus::MenuRenderContext::from_lobby(
                server_config,
                plugins,
                players,
                actor,
                lobby,
                lobby_status,
            );
            let username = players
                .player_by_uuid(actor)
                .map(|player| player.profile.username)
                .unwrap_or_default();
            let opened = menus
                .open_menu_for_client(
                    sink,
                    &action.target,
                    Some(&render_context),
                    &username,
                    geyser_runtime.as_deref_mut(),
                )
                .await?;
            Ok(MenuActionOutcome {
                opened_menu: opened.is_some(),
                opened_menu_id: opened,
                ..Default::default()
            })
        }
        qexed_config::app::qexed::server::MenuActionKind::Transfer => {
            let target = action.target.trim();
            if target.is_empty() {
                return Ok(MenuActionOutcome::default());
            }
            if lobby.enabled() && lobby.has_server(target) {
                let proxy_context = server_config.map(|server_config| lobby::ProxyConnectContext {
                    server_config,
                    plugins,
                    players,
                    actor,
                });
                lobby
                    .transfer_to_server_with_message(
                        sink,
                        target,
                        &action.message,
                        lobby_status,
                        proxy_context.as_ref(),
                    )
                    .await?;
            } else {
                chat::apply_proxy_connect_action(
                    sink,
                    server_config,
                    plugins,
                    players,
                    actor,
                    target,
                    &action.message,
                )
                .await?;
            }
            Ok(MenuActionOutcome::default())
        }
        qexed_config::app::qexed::server::MenuActionKind::Command => {
            let command_line = action.target.trim().trim_start_matches('/');
            let mut parts = command_line.split_whitespace();
            let command = crate::commands::normalize_command_name(parts.next().unwrap_or_default());
            let argument = parts.collect::<Vec<_>>().join(" ");
            if command.is_empty() {
                return Ok(MenuActionOutcome::default());
            }
            let Some(player) = players.player_by_uuid(actor) else {
                return Ok(MenuActionOutcome::default());
            };
            let response = plugins.execute_command(&player, &command, &argument);
            let mut outcome = MenuActionOutcome::default();
            if inventory.is_some() {
                outcome.deferred_actions = response.actions;
                return Ok(outcome);
            }
            for action in response.actions {
                match action {
                    crate::plugins::PlayerAction::SystemMessage {
                        text,
                        translate,
                        with,
                        overlay,
                    } => {
                        let content = if !translate.trim().is_empty() {
                            translatable_component(
                                &translate,
                                with.into_iter().map(text_component).collect(),
                            )
                        } else {
                            text_component(text)
                        };
                        sink.send(SystemChat { content, overlay }).await?;
                    }
                    crate::plugins::PlayerAction::OpenMenu { menu } => {
                        let render_context = menus::MenuRenderContext::from_lobby(
                            server_config,
                            plugins,
                            players,
                            actor,
                            lobby,
                            lobby_status,
                        );
                        let username = players
                            .player_by_uuid(actor)
                            .map(|player| player.profile.username)
                            .unwrap_or_default();
                        let opened = menus
                            .open_menu_for_client(
                                sink,
                                &menu,
                                Some(&render_context),
                                &username,
                                geyser_runtime.as_deref_mut(),
                            )
                            .await?;
                        outcome.opened_menu = opened.is_some();
                        outcome.opened_menu_id = opened;
                    }
                    crate::plugins::PlayerAction::GiveItem {
                        item,
                        count,
                        name,
                        lore,
                        enchantments,
                        plugin_enchantments,
                    } => {
                        if let Some(inventory) = inventory.as_deref_mut()
                            && let Some(item) = plugin_action_item_stack(
                                &item,
                                count,
                                &name,
                                &lore,
                                &enchantments,
                                &plugin_enchantments,
                            )
                        {
                            if let Some(changes) = inventory.add_item_stack(&item) {
                                sync_inventory_changes(
                                    sink,
                                    players,
                                    actor,
                                    actor_entity_id,
                                    inventory.selected_slot(),
                                    changes,
                                )
                                .await?;
                            } else {
                                sink.send(SystemChat {
                                    content: text_component("背包空间不足"),
                                    overlay: false,
                                })
                                .await?;
                            }
                        }
                    }
                    other => {
                        outcome.deferred_actions.push(other);
                    }
                }
            }
            Ok(outcome)
        }
        qexed_config::app::qexed::server::MenuActionKind::Message => {
            if !action.message.trim().is_empty() {
                sink.send(SystemChat {
                    content: text_component(action.message),
                    overlay: false,
                })
                .await?;
            }
            Ok(MenuActionOutcome::default())
        }
        qexed_config::app::qexed::server::MenuActionKind::HidePlayers => {
            set_other_players_visible(
                sink,
                players,
                actor,
                dimension,
                viewer_position,
                render_distance,
                visible_player_entities,
                false,
            )
            .await?;
            *players_hidden = true;
            Ok(MenuActionOutcome::default())
        }
        qexed_config::app::qexed::server::MenuActionKind::ShowPlayers => {
            set_other_players_visible(
                sink,
                players,
                actor,
                dimension,
                viewer_position,
                render_distance,
                visible_player_entities,
                true,
            )
            .await?;
            *players_hidden = false;
            Ok(MenuActionOutcome::default())
        }
        qexed_config::app::qexed::server::MenuActionKind::TogglePlayers => {
            let visible = *players_hidden;
            set_other_players_visible(
                sink,
                players,
                actor,
                dimension,
                viewer_position,
                render_distance,
                visible_player_entities,
                visible,
            )
            .await?;
            *players_hidden = !visible;
            Ok(MenuActionOutcome::default())
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn apply_deferred_menu_actions<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    world: &WorldManager,
    world_rules: &crate::world::WorldRulesManager,
    server_config: &qexed_config::app::qexed::server::Server,
    world_config: &qexed_config::app::qexed::server::World,
    entities: &crate::entities::EntityManager,
    players: &PlayerManager,
    plugins: &crate::plugins::PluginManager,
    actor: uuid::Uuid,
    chunk_sender: &tokio::sync::mpsc::UnboundedSender<chunks::ChunkLoadResult>,
    chunk_state: &mut ChunkSendState,
    position: &mut EntityPosition,
    next_teleport_id: &mut i32,
    play_dimension: &mut String,
    menus: &menus::MenuRuntime,
    active_config_menu: &mut Option<String>,
    players_hidden: &mut bool,
    visible_player_entities: &mut HashSet<uuid::Uuid>,
    render_distance: f64,
    inventory: &mut crate::inventory::PlayerInventory,
    geyser_runtime: &mut geyser::GeyserRuntime,
    actor_entity_id: i32,
    actions: Vec<crate::plugins::PlayerAction>,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    for action in actions {
        let viewer_position = *position;
        let before_dimension = play_dimension.clone();
        chat::apply_plugin_action(
            sink,
            Some(server_config),
            world,
            world_rules,
            world_config,
            entities,
            players,
            plugins,
            actor,
            chunk_sender,
            chunk_state,
            position,
            next_teleport_id,
            play_dimension,
            menus,
            active_config_menu,
            players_hidden,
            visible_player_entities,
            viewer_position,
            render_distance,
            Some(&mut *inventory),
            Some(geyser_runtime),
            action,
        )
        .await?;
        if before_dimension != *play_dimension {
            resync_inventory_state(sink, players, actor, actor_entity_id, inventory).await?;
        }
    }
    Ok(())
}

fn plugin_action_item_stack(
    item_name: &str,
    count: i32,
    display_name: &str,
    lore: &[String],
    enchantments: &[crate::plugins::ItemEnchantment],
    plugin_enchantments: &[crate::plugins::PluginEnchantment],
) -> Option<qexed_protocol::types::Slot> {
    let item_id = crate::inventory::item_id_for_name(&normalize_resource_key(item_name))?;
    let mut item = crate::inventory::simple_item(item_id, count.clamp(1, 64));
    let mut components = Vec::new();

    let vanilla_enchantments = enchantments
        .iter()
        .filter(|enchantment| enchantment.level > 0)
        .filter_map(|enchantment| {
            vanilla_enchantment_id(&enchantment.id).map(|id| {
                qexed_protocol::types::minecraft::Enchantment {
                    enchantment: VarInt(id),
                    level: VarInt(enchantment.level),
                }
            })
        })
        .collect::<Vec<_>>();
    if !vanilla_enchantments.is_empty() {
        components.push(
            qexed_protocol::types::ComponentsToAdd::MinecraftEnchantments(
                qexed_protocol::types::minecraft::Enchantments {
                    enchantments: vanilla_enchantments,
                },
            ),
        );
    }

    if !plugin_enchantments.is_empty() {
        let enchantments = plugin_enchantments
            .iter()
            .filter(|enchantment| enchantment.level > 0 && !enchantment.id.trim().is_empty())
            .map(|enchantment| {
                (
                    enchantment.id.trim().to_string(),
                    qexed_nbt::Tag::Int(enchantment.level),
                )
            })
            .collect::<std::collections::HashMap<_, _>>();
        if !enchantments.is_empty() {
            let mut root = std::collections::HashMap::new();
            root.insert(
                "qexed:enchantments".to_string(),
                qexed_nbt::Tag::Compound(std::sync::Arc::new(enchantments)),
            );
            components.push(qexed_protocol::types::ComponentsToAdd::MinecraftCustomData(
                qexed_protocol::types::minecraft::CustomData {
                    data: qexed_nbt::Tag::Compound(std::sync::Arc::new(root)),
                },
            ));
        }
    }

    let display_name = display_name.trim();
    if !display_name.is_empty() {
        components.push(qexed_protocol::types::ComponentsToAdd::MinecraftItemName(
            qexed_protocol::types::minecraft::ItemName {
                name: text_component(display_name),
            },
        ));
    }
    if !lore.is_empty() {
        components.push(qexed_protocol::types::ComponentsToAdd::MinecraftLore(
            qexed_protocol::types::minecraft::Lore {
                lines: lore.iter().map(|line| text_component(line)).collect(),
            },
        ));
    }

    if !components.is_empty() {
        item.number_of_components_to_add = Some(VarInt(components.len() as i32));
        item.components_to_add = Some(components);
    }
    Some(item)
}

fn normalize_resource_key(value: &str) -> String {
    let value = value.trim();
    if value.contains(':') {
        value.to_string()
    } else {
        format!("minecraft:{value}")
    }
}

fn vanilla_enchantment_id(value: &str) -> Option<i32> {
    let key = normalize_resource_key(value);
    static IDS: std::sync::OnceLock<std::collections::HashMap<String, i32>> =
        std::sync::OnceLock::new();
    IDS.get_or_init(|| {
        crate::registry_sync::load_registry_id_map("minecraft:enchantment").unwrap_or_default()
    })
    .get(&key)
    .copied()
    .or_else(|| match key.as_str() {
        "minecraft:efficiency" => Some(8),
        "minecraft:fortune" => Some(13),
        "minecraft:mending" => Some(23),
        "minecraft:silk_touch" => Some(34),
        "minecraft:unbreaking" => Some(40),
        _ => None,
    })
}

fn filtered_player_event_packets(
    event: &crate::players::PlayerEvent,
    players: &PlayerManager,
    player_entity_type: i32,
    viewer_dimension: &str,
    viewer_position: EntityPosition,
    render_distance: f64,
    block_update_distance: f64,
    visible_player_entities: &mut HashSet<uuid::Uuid>,
) -> Result<Vec<bytes::Bytes>> {
    match event {
        crate::players::PlayerEvent::Joined(player) => {
            if player.dimension != viewer_dimension {
                return Ok(vec![crate::players::packet_bytes(
                    qexed_protocol::to_client::play::player_info_update::PlayerInfoUpdate {
                        actions: qexed_protocol::to_client::play::player_info_update::PlayerInfoActions::player_initializing(),
                        entries: vec![
                            qexed_protocol::to_client::play::player_info_update::PlayerInfoEntry::from_profile(
                                &player.profile,
                                1,
                            ),
                        ],
                    },
                )?]);
            }
            if !within_horizontal_distance(viewer_position, player.position, render_distance) {
                visible_player_entities.remove(&player.profile.uuid);
                return Ok(Vec::new());
            }
            visible_player_entities.insert(player.profile.uuid);
            crate::players::spawn_player_packets(player, player_entity_type)
        }
        crate::players::PlayerEvent::Left {
            profile_id,
            entity_id,
            dimension,
            ..
        } => {
            let mut packets = Vec::new();
            if dimension == viewer_dimension && visible_player_entities.remove(profile_id) {
                packets.push(crate::players::packet_bytes(
                    qexed_protocol::to_client::play::add_entity::RemoveEntities::one(*entity_id),
                )?);
            }
            packets.push(crate::players::packet_bytes(
                qexed_protocol::to_client::play::add_entity::PlayerInfoRemove::one(*profile_id),
            )?);
            Ok(packets)
        }
        crate::players::PlayerEvent::Moved {
            profile_id,
            entity_id,
            dimension,
            position,
        } => {
            if dimension != viewer_dimension {
                if visible_player_entities.remove(profile_id) {
                    return Ok(vec![crate::players::packet_bytes(
                        qexed_protocol::to_client::play::add_entity::RemoveEntities::one(
                            *entity_id,
                        ),
                    )?]);
                }
                return Ok(Vec::new());
            }

            let in_range = within_horizontal_distance(viewer_position, *position, render_distance);
            let was_visible = visible_player_entities.contains(profile_id);
            if in_range && !was_visible {
                if let Some(player) = players.player_by_uuid(*profile_id) {
                    visible_player_entities.insert(*profile_id);
                    return crate::players::spawn_player_packets(&player, player_entity_type);
                }
            }
            if in_range {
                visible_player_entities.insert(*profile_id);
                return event.packets(player_entity_type, viewer_dimension);
            }
            if was_visible {
                visible_player_entities.remove(profile_id);
                return Ok(vec![crate::players::packet_bytes(
                    qexed_protocol::to_client::play::add_entity::RemoveEntities::one(*entity_id),
                )?]);
            }
            Ok(Vec::new())
        }
        crate::players::PlayerEvent::DimensionChanged {
            profile_id,
            entity_id,
            old_dimension,
            player,
        } => {
            let mut packets = Vec::new();
            if old_dimension == viewer_dimension && visible_player_entities.remove(profile_id) {
                packets.push(crate::players::packet_bytes(
                    qexed_protocol::to_client::play::add_entity::RemoveEntities::one(*entity_id),
                )?);
            }
            if player.dimension == viewer_dimension
                && within_horizontal_distance(viewer_position, player.position, render_distance)
            {
                visible_player_entities.insert(*profile_id);
                packets.extend(crate::players::spawn_player_packets(
                    player,
                    player_entity_type,
                )?);
            }
            Ok(packets)
        }
        crate::players::PlayerEvent::EquipmentChanged {
            profile_id,
            dimension,
            ..
        } => {
            if dimension != viewer_dimension || !visible_player_entities.contains(profile_id) {
                return Ok(Vec::new());
            }
            event.packets(player_entity_type, viewer_dimension)
        }
        crate::players::PlayerEvent::Animation {
            profile_id,
            dimension,
            ..
        } => {
            if dimension != viewer_dimension || !visible_player_entities.contains(profile_id) {
                return Ok(Vec::new());
            }
            event.packets(player_entity_type, viewer_dimension)
        }
        crate::players::PlayerEvent::BlockChanged {
            dimension,
            position,
            ..
        } => {
            if dimension != viewer_dimension
                || !block_position_within_horizontal_distance(
                    viewer_position,
                    position,
                    block_update_distance,
                )
            {
                return Ok(Vec::new());
            }
            event.packets(player_entity_type, viewer_dimension)
        }
        crate::players::PlayerEvent::BlockChanges {
            dimension, changes, ..
        } => {
            if dimension != viewer_dimension {
                return Ok(Vec::new());
            }
            let visible_changes = changes
                .iter()
                .filter(|change| {
                    block_position_within_horizontal_distance(
                        viewer_position,
                        &change.position,
                        block_update_distance,
                    )
                })
                .map(|change| (change.position.clone(), change.block_state))
                .collect::<Vec<_>>();
            block_update_packets(visible_changes)
        }
        _ => event.packets(player_entity_type, viewer_dimension),
    }
}

async fn refresh_visible_players<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    players: &PlayerManager,
    actor: uuid::Uuid,
    player_entity_type: i32,
    dimension: &str,
    viewer_position: EntityPosition,
    render_distance: f64,
    visible_player_entities: &mut HashSet<uuid::Uuid>,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let mut still_visible = HashSet::new();
    for player in players.list_except(actor) {
        if player.dimension != dimension {
            continue;
        }
        if !within_horizontal_distance(viewer_position, player.position, render_distance) {
            continue;
        }
        still_visible.insert(player.profile.uuid);
        if visible_player_entities.contains(&player.profile.uuid) {
            continue;
        }
        for packet in crate::players::spawn_player_packets(&player, player_entity_type)? {
            sink.send_raw(packet).await?;
        }
    }

    let to_remove = visible_player_entities
        .difference(&still_visible)
        .copied()
        .collect::<Vec<_>>();
    for profile_id in to_remove {
        visible_player_entities.remove(&profile_id);
        if let Some(player) = players.player_by_uuid(profile_id) {
            sink.send(
                qexed_protocol::to_client::play::add_entity::RemoveEntities::one(player.entity_id),
            )
            .await?;
        }
    }
    Ok(())
}

fn within_horizontal_distance(left: EntityPosition, right: EntityPosition, distance: f64) -> bool {
    if distance <= 0.0 {
        return false;
    }
    let dx = left.x - right.x;
    let dz = left.z - right.z;
    (dx * dx + dz * dz) <= distance * distance
}

fn block_update_distance(view_distance_chunks: i32) -> f64 {
    f64::from(view_distance_chunks.max(1) * 16 + 16)
}

fn block_position_within_horizontal_distance(
    viewer: EntityPosition,
    block: &BlockPosition,
    distance: f64,
) -> bool {
    if distance <= 0.0 {
        return false;
    }
    let dx = viewer.x - (f64::from(block.x) + 0.5);
    let dz = viewer.z - (f64::from(block.z) + 0.5);
    (dx * dx + dz * dz) <= distance * distance
}

fn block_update_packets(
    changes: impl IntoIterator<Item = (BlockPosition, i32)>,
) -> Result<Vec<bytes::Bytes>> {
    let mut by_section: BTreeMap<(i32, i32, i32), Vec<(BlockPosition, i32)>> = BTreeMap::new();
    for (position, block_state) in changes {
        by_section
            .entry((
                position.x.div_euclid(16),
                position.y.div_euclid(16),
                position.z.div_euclid(16),
            ))
            .or_default()
            .push((position, block_state));
    }

    let mut packets = Vec::new();
    for ((section_x, section_y, section_z), mut section_changes) in by_section {
        if section_changes.len() == 1 {
            let (position, block_state) = section_changes.pop().expect("one block change");
            packets.push(crate::players::packet_bytes(
                qexed_protocol::to_client::play::block_update::BlockUpdate {
                    location: position,
                    block_state: VarInt(block_state),
                },
            )?);
            continue;
        }
        section_changes.sort_by_key(|(position, _)| {
            (
                position.y.rem_euclid(16),
                position.z.rem_euclid(16),
                position.x.rem_euclid(16),
            )
        });
        let blocks = section_changes
            .into_iter()
            .map(|(position, block_state)| {
                VarLong((i64::from(block_state) << 12) | i64::from(section_relative_pos(&position)))
            })
            .collect();
        packets.push(crate::players::packet_bytes(
            qexed_protocol::to_client::play::section_blocks_update::SectionBlocksUpdate {
                section_position: section_position(section_x, section_y, section_z),
                blocks,
            },
        )?);
    }
    Ok(packets)
}

fn section_relative_pos(position: &BlockPosition) -> i32 {
    (position.x.rem_euclid(16) << 8) | (position.z.rem_euclid(16) << 4) | position.y.rem_euclid(16)
}

fn section_position(section_x: i32, section_y: i32, section_z: i32) -> i64 {
    ((i64::from(section_x) & 0x3f_ffff) << 42)
        | ((i64::from(section_z) & 0x3f_ffff) << 20)
        | (i64::from(section_y) & 0x0f_ffff)
}

fn player_action_drops_item(status: i32) -> bool {
    status == PLAYER_ACTION_DROP_ITEM_STACK || status == PLAYER_ACTION_DROP_ITEM
}

fn container_click_affects_fixed_menu_slot(
    click: &ContainerClick,
    menus: &menus::MenuRuntime,
    inventory: &crate::inventory::PlayerInventory,
) -> bool {
    container_click_affects_hotbar_slot(click, |slot| {
        menus.fixed_hotbar_slot(slot)
            && inventory
                .hotbar_item(slot)
                .is_some_and(|item| menus.hotbar_item_matches(slot, item))
    })
}

fn container_click_affects_navigator_slot(
    click: &ContainerClick,
    lobby: &lobby::LobbyRuntime,
    inventory: &crate::inventory::PlayerInventory,
) -> bool {
    container_click_affects_hotbar_slot(click, |slot| {
        inventory
            .hotbar_item(slot)
            .is_some_and(|item| lobby.navigator_item_matches(slot, item))
    })
}

fn container_click_affects_hotbar_slot(
    click: &ContainerClick,
    mut matches_slot: impl FnMut(usize) -> bool,
) -> bool {
    if let Some(slot) = inventory_window_hotbar_slot(click.slot)
        && matches_slot(slot)
    {
        return true;
    }
    click
        .changed_slots
        .0
        .keys()
        .any(|slot| inventory_window_hotbar_slot(*slot).is_some_and(|slot| matches_slot(slot)))
}

fn inventory_window_hotbar_slot(slot: i16) -> Option<usize> {
    (36..=44)
        .contains(&slot)
        .then(|| usize::try_from(slot - 36).ok())
        .flatten()
}

fn creative_mode_hotbar_slot(slot: i16) -> Option<usize> {
    inventory_window_hotbar_slot(slot)
}

pub(super) async fn set_other_players_visible<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    players: &PlayerManager,
    actor: uuid::Uuid,
    dimension: &str,
    viewer_position: EntityPosition,
    render_distance: f64,
    visible_player_entities: &mut HashSet<uuid::Uuid>,
    visible: bool,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    if visible {
        visible_player_entities.clear();
        for player in players.list_except(actor) {
            if player.dimension != dimension {
                continue;
            }
            if !within_horizontal_distance(viewer_position, player.position, render_distance) {
                continue;
            }
            for packet in crate::players::spawn_player_packets(
                &player,
                crate::entities::entity_type_id("minecraft:player")?,
            )? {
                sink.send_raw(packet).await?;
            }
            visible_player_entities.insert(player.profile.uuid);
        }
        return Ok(());
    }

    let ids = players
        .list_except(actor)
        .into_iter()
        .filter(|player| player.dimension == dimension)
        .map(|player| player.entity_id)
        .collect::<Vec<_>>();
    visible_player_entities.clear();
    if !ids.is_empty() {
        sink.send(
            qexed_protocol::to_client::play::add_entity::RemoveEntities {
                entity_ids: ids.into_iter().map(VarInt).collect(),
            },
        )
        .await?;
    }
    Ok(())
}

fn event_affects_player_entity(event: &crate::players::PlayerEvent) -> bool {
    matches!(
        event,
        crate::players::PlayerEvent::Joined(_)
            | crate::players::PlayerEvent::Left { .. }
            | crate::players::PlayerEvent::Moved { .. }
            | crate::players::PlayerEvent::DimensionChanged { .. }
            | crate::players::PlayerEvent::EquipmentChanged { .. }
    )
}

#[allow(clippy::too_many_arguments)]
async fn handle_plugin_player_block_step<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    world: &WorldManager,
    world_rules: &crate::world::WorldRulesManager,
    server_config: &qexed_config::app::qexed::server::Server,
    world_config: &qexed_config::app::qexed::server::World,
    entities: &crate::entities::EntityManager,
    players: &PlayerManager,
    plugins: &crate::plugins::PluginManager,
    player: &crate::players::OnlinePlayer,
    chunk_sender: &tokio::sync::mpsc::UnboundedSender<chunks::ChunkLoadResult>,
    chunk_state: &mut ChunkSendState,
    position: &mut EntityPosition,
    next_teleport_id: &mut i32,
    play_dimension: &mut String,
    menus: &menus::MenuRuntime,
    active_config_menu: &mut Option<String>,
    players_hidden: &mut bool,
    visible_player_entities: &mut HashSet<uuid::Uuid>,
    render_distance: f64,
    last_stepped_block: &mut Option<BlockPosition>,
    inventory: &mut crate::inventory::PlayerInventory,
    geyser_runtime: &mut geyser::GeyserRuntime,
) -> Result<bool>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let Some((block_position, block_state, block_name)) =
        stepped_plugin_block(world, play_dimension, *position)
    else {
        *last_stepped_block = None;
        return Ok(false);
    };
    if last_stepped_block.as_ref() == Some(&block_position) {
        return Ok(false);
    }
    *last_stepped_block = Some(block_position.clone());

    let mut player = player.clone();
    player.position = *position;
    player.dimension = play_dimension.clone();
    let response = plugins.handle_player_block_step(
        &player,
        block_state,
        block_name,
        crate::plugins::BlockStepPosition {
            x: block_position.x,
            y: block_position.y,
            z: block_position.z,
        },
    );

    let mut handled = response.handled || !response.actions.is_empty();
    for action in response.actions {
        let before_dimension = play_dimension.clone();
        handled |= chat::apply_plugin_action(
            sink,
            Some(server_config),
            world,
            world_rules,
            world_config,
            entities,
            players,
            plugins,
            player.profile.uuid,
            chunk_sender,
            chunk_state,
            position,
            next_teleport_id,
            play_dimension,
            menus,
            active_config_menu,
            players_hidden,
            visible_player_entities,
            player.position,
            render_distance,
            Some(&mut *inventory),
            Some(geyser_runtime),
            action,
        )
        .await?;
        if before_dimension != *play_dimension {
            resync_inventory_state(
                sink,
                players,
                player.profile.uuid,
                player.entity_id,
                inventory,
            )
            .await?;
        }
    }
    Ok(handled)
}

#[allow(clippy::too_many_arguments)]
async fn handle_plugin_player_block_interact<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    world: &WorldManager,
    world_rules: &crate::world::WorldRulesManager,
    server_config: &qexed_config::app::qexed::server::Server,
    world_config: &qexed_config::app::qexed::server::World,
    entities: &crate::entities::EntityManager,
    players: &PlayerManager,
    plugins: &crate::plugins::PluginManager,
    player: &crate::players::OnlinePlayer,
    use_item_on: &UseItemOn,
    chunk_sender: &tokio::sync::mpsc::UnboundedSender<chunks::ChunkLoadResult>,
    chunk_state: &mut ChunkSendState,
    position: &mut EntityPosition,
    next_teleport_id: &mut i32,
    play_dimension: &mut String,
    menus: &menus::MenuRuntime,
    active_config_menu: &mut Option<String>,
    players_hidden: &mut bool,
    visible_player_entities: &mut HashSet<uuid::Uuid>,
    render_distance: f64,
    inventory: &mut crate::inventory::PlayerInventory,
    geyser_runtime: &mut geyser::GeyserRuntime,
    last_input_flags: u8,
) -> Result<bool>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let Some(block_state) = world.block_state_at(play_dimension, &use_item_on.block_hit.position)
    else {
        return Ok(false);
    };
    let block_name = block_name(block_state);
    let mut player = player.clone();
    player.position = *position;
    player.dimension = play_dimension.clone();
    let hand = match use_item_on.hand.0 {
        0 => "main_hand",
        1 => "off_hand",
        _ => "unknown",
    };
    let response = plugins.handle_player_block_interact(
        &player,
        block_state,
        block_name,
        crate::plugins::BlockDropPosition {
            x: use_item_on.block_hit.position.x,
            y: use_item_on.block_hit.position.y,
            z: use_item_on.block_hit.position.z,
        },
        hand.to_string(),
        block_hit_payload(use_item_on),
        qexed_plugin_api::player_input_state(last_input_flags),
        geyser_runtime.client_payload(&player.profile.username),
    );

    handle_plugin_response_actions(
        sink,
        world,
        world_rules,
        server_config,
        world_config,
        entities,
        players,
        plugins,
        &player,
        response,
        chunk_sender,
        chunk_state,
        position,
        next_teleport_id,
        play_dimension,
        menus,
        active_config_menu,
        players_hidden,
        visible_player_entities,
        render_distance,
        inventory,
        geyser_runtime,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn handle_plugin_player_move<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    world: &WorldManager,
    world_rules: &crate::world::WorldRulesManager,
    server_config: &qexed_config::app::qexed::server::Server,
    world_config: &qexed_config::app::qexed::server::World,
    entities: &crate::entities::EntityManager,
    players: &PlayerManager,
    plugins: &crate::plugins::PluginManager,
    player: &crate::players::OnlinePlayer,
    previous: EntityPosition,
    chunk_sender: &tokio::sync::mpsc::UnboundedSender<chunks::ChunkLoadResult>,
    chunk_state: &mut ChunkSendState,
    position: &mut EntityPosition,
    next_teleport_id: &mut i32,
    play_dimension: &mut String,
    menus: &menus::MenuRuntime,
    active_config_menu: &mut Option<String>,
    players_hidden: &mut bool,
    visible_player_entities: &mut HashSet<uuid::Uuid>,
    render_distance: f64,
    inventory: &mut crate::inventory::PlayerInventory,
    geyser_runtime: &mut geyser::GeyserRuntime,
) -> Result<bool>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let mut player = player.clone();
    player.position = *position;
    player.dimension = play_dimension.clone();
    let response = plugins.handle_player_move(&player, previous);
    let mut handled = response.handled || !response.actions.is_empty();
    for action in response.actions {
        let before_dimension = play_dimension.clone();
        handled |= chat::apply_plugin_action(
            sink,
            Some(server_config),
            world,
            world_rules,
            world_config,
            entities,
            players,
            plugins,
            player.profile.uuid,
            chunk_sender,
            chunk_state,
            position,
            next_teleport_id,
            play_dimension,
            menus,
            active_config_menu,
            players_hidden,
            visible_player_entities,
            player.position,
            render_distance,
            Some(&mut *inventory),
            Some(geyser_runtime),
            action,
        )
        .await?;
        if before_dimension != *play_dimension {
            resync_inventory_state(
                sink,
                players,
                player.profile.uuid,
                player.entity_id,
                inventory,
            )
            .await?;
        }
    }
    Ok(handled)
}

fn block_hit_payload(use_item_on: &UseItemOn) -> crate::plugins::PlayerBlockHitPayload {
    let face_id = use_item_on.block_hit.face.0;
    crate::plugins::PlayerBlockHitPayload {
        sequence: use_item_on.sequence.0,
        face: block_face_name(face_id).to_string(),
        face_id,
        cursor_x: use_item_on.block_hit.cursor_x,
        cursor_y: use_item_on.block_hit.cursor_y,
        cursor_z: use_item_on.block_hit.cursor_z,
        inside_block: use_item_on.block_hit.inside_block,
        world_border_hit: use_item_on.block_hit.world_border_hit,
    }
}

fn block_face_name(face_id: i32) -> &'static str {
    match face_id {
        0 => "down",
        1 => "up",
        2 => "north",
        3 => "south",
        4 => "west",
        5 => "east",
        _ => "unknown",
    }
}

#[allow(clippy::too_many_arguments)]
async fn handle_plugin_response_actions<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    world: &WorldManager,
    world_rules: &crate::world::WorldRulesManager,
    server_config: &qexed_config::app::qexed::server::Server,
    world_config: &qexed_config::app::qexed::server::World,
    entities: &crate::entities::EntityManager,
    players: &PlayerManager,
    plugins: &crate::plugins::PluginManager,
    player: &crate::players::OnlinePlayer,
    response: crate::plugins::PluginCommandResponse,
    chunk_sender: &tokio::sync::mpsc::UnboundedSender<chunks::ChunkLoadResult>,
    chunk_state: &mut ChunkSendState,
    position: &mut EntityPosition,
    next_teleport_id: &mut i32,
    play_dimension: &mut String,
    menus: &menus::MenuRuntime,
    active_config_menu: &mut Option<String>,
    players_hidden: &mut bool,
    visible_player_entities: &mut HashSet<uuid::Uuid>,
    render_distance: f64,
    inventory: &mut crate::inventory::PlayerInventory,
    geyser_runtime: &mut geyser::GeyserRuntime,
) -> Result<bool>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let mut handled = response.handled || !response.actions.is_empty();
    for action in response.actions {
        let before_dimension = play_dimension.clone();
        handled |= chat::apply_plugin_action(
            sink,
            Some(server_config),
            world,
            world_rules,
            world_config,
            entities,
            players,
            plugins,
            player.profile.uuid,
            chunk_sender,
            chunk_state,
            position,
            next_teleport_id,
            play_dimension,
            menus,
            active_config_menu,
            players_hidden,
            visible_player_entities,
            player.position,
            render_distance,
            Some(&mut *inventory),
            Some(geyser_runtime),
            action,
        )
        .await?;
        if before_dimension != *play_dimension {
            resync_inventory_state(
                sink,
                players,
                player.profile.uuid,
                player.entity_id,
                inventory,
            )
            .await?;
        }
    }
    Ok(handled)
}

#[allow(clippy::too_many_arguments)]
async fn handle_plugin_player_input<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    world: &WorldManager,
    world_rules: &crate::world::WorldRulesManager,
    server_config: &qexed_config::app::qexed::server::Server,
    world_config: &qexed_config::app::qexed::server::World,
    entities: &crate::entities::EntityManager,
    players: &PlayerManager,
    plugins: &crate::plugins::PluginManager,
    player: &crate::players::OnlinePlayer,
    previous_flags: u8,
    flags: u8,
    chunk_sender: &tokio::sync::mpsc::UnboundedSender<chunks::ChunkLoadResult>,
    chunk_state: &mut ChunkSendState,
    position: &mut EntityPosition,
    next_teleport_id: &mut i32,
    play_dimension: &mut String,
    menus: &menus::MenuRuntime,
    active_config_menu: &mut Option<String>,
    players_hidden: &mut bool,
    visible_player_entities: &mut HashSet<uuid::Uuid>,
    render_distance: f64,
    inventory: &mut crate::inventory::PlayerInventory,
    geyser_runtime: &mut geyser::GeyserRuntime,
) -> Result<bool>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let mut player = player.clone();
    player.position = *position;
    player.dimension = play_dimension.clone();
    let response = plugins.handle_player_input(
        &player,
        previous_flags,
        flags,
        geyser_runtime.client_payload(&player.profile.username),
    );
    let mut handled = response.handled || !response.actions.is_empty();
    for action in response.actions {
        let before_dimension = play_dimension.clone();
        handled |= chat::apply_plugin_action(
            sink,
            Some(server_config),
            world,
            world_rules,
            world_config,
            entities,
            players,
            plugins,
            player.profile.uuid,
            chunk_sender,
            chunk_state,
            position,
            next_teleport_id,
            play_dimension,
            menus,
            active_config_menu,
            players_hidden,
            visible_player_entities,
            player.position,
            render_distance,
            Some(&mut *inventory),
            Some(geyser_runtime),
            action,
        )
        .await?;
        if before_dimension != *play_dimension {
            resync_inventory_state(
                sink,
                players,
                player.profile.uuid,
                player.entity_id,
                inventory,
            )
            .await?;
        }
    }
    Ok(handled)
}

#[allow(clippy::too_many_arguments)]
async fn handle_plugin_player_use_item<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    world: &WorldManager,
    world_rules: &crate::world::WorldRulesManager,
    server_config: &qexed_config::app::qexed::server::Server,
    world_config: &qexed_config::app::qexed::server::World,
    entities: &crate::entities::EntityManager,
    players: &PlayerManager,
    plugins: &crate::plugins::PluginManager,
    player: &crate::players::OnlinePlayer,
    action: &str,
    hand: &str,
    sequence: i32,
    yaw: f32,
    pitch: f32,
    last_input_flags: u8,
    inventory: &mut crate::inventory::PlayerInventory,
    chunk_sender: &tokio::sync::mpsc::UnboundedSender<chunks::ChunkLoadResult>,
    chunk_state: &mut ChunkSendState,
    position: &mut EntityPosition,
    next_teleport_id: &mut i32,
    play_dimension: &mut String,
    menus: &menus::MenuRuntime,
    active_config_menu: &mut Option<String>,
    players_hidden: &mut bool,
    visible_player_entities: &mut HashSet<uuid::Uuid>,
    render_distance: f64,
    geyser_runtime: &mut geyser::GeyserRuntime,
) -> Result<bool>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let mut player = player.clone();
    player.position = *position;
    player.dimension = play_dimension.clone();
    let response = plugins.handle_player_use_item(
        &player,
        hand.to_string(),
        action.to_string(),
        gameplay::items::item_stack_payload(inventory.held_item()),
        sequence,
        yaw,
        pitch,
        qexed_plugin_api::player_input_state(last_input_flags),
        geyser_runtime.client_payload(&player.profile.username),
    );
    handle_plugin_response_actions(
        sink,
        world,
        world_rules,
        server_config,
        world_config,
        entities,
        players,
        plugins,
        &player,
        response,
        chunk_sender,
        chunk_state,
        position,
        next_teleport_id,
        play_dimension,
        menus,
        active_config_menu,
        players_hidden,
        visible_player_entities,
        render_distance,
        inventory,
        geyser_runtime,
    )
    .await
}

async fn handle_plugin_npc_interact<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    world: &WorldManager,
    world_rules: &crate::world::WorldRulesManager,
    server_config: &qexed_config::app::qexed::server::Server,
    world_config: &qexed_config::app::qexed::server::World,
    players: &PlayerManager,
    plugins: &crate::plugins::PluginManager,
    entities: &crate::entities::EntityManager,
    player: &crate::players::OnlinePlayer,
    entity_id: i32,
    action_name: &'static str,
    hand_name: &'static str,
    chunk_sender: &tokio::sync::mpsc::UnboundedSender<chunks::ChunkLoadResult>,
    chunk_state: &mut ChunkSendState,
    position: &mut EntityPosition,
    next_teleport_id: &mut i32,
    play_dimension: &mut String,
    menus: &menus::MenuRuntime,
    active_config_menu: &mut Option<String>,
    players_hidden: &mut bool,
    visible_player_entities: &mut HashSet<uuid::Uuid>,
    inventory: &mut crate::inventory::PlayerInventory,
    viewer_position: EntityPosition,
    render_distance: f64,
    geyser_runtime: &mut geyser::GeyserRuntime,
) -> Result<PluginNpcInteractOutcome>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let Some(entity) = entities.entity_by_runtime_id(entity_id) else {
        log::debug!(
            "ignored entity interact for unmanaged entity: entity_id={}, action={}",
            entity_id,
            action_name
        );
        return Ok(PluginNpcInteractOutcome::default());
    };
    if entity.kind == crate::entities::ManagedEntityKind::Hologram {
        return Ok(PluginNpcInteractOutcome::default());
    }

    let configured_event = match action_name {
        "interact" => entity.main_hand_event.clone(),
        "interact_off_hand" => entity.off_hand_event.clone(),
        "attack" => entity.attack_event.clone(),
        _ => action_name.to_string(),
    };

    log::debug!(
        "dispatching plugin entity interact: player={}, entity_key={}, entity_id={}, action={}",
        player.profile.username,
        entity.key,
        entity.entity_id,
        configured_event
    );

    let response = plugins.handle_npc_interact(
        player,
        crate::plugins::NpcEntityPayload {
            key: entity.key,
            entity_id: entity.entity_id,
            dimension: entity.dimension,
            x: entity.position.x,
            y: entity.position.y,
            z: entity.position.z,
            yaw: entity.position.yaw,
            pitch: entity.position.pitch,
        },
        &configured_event,
        hand_name,
        &configured_event,
    );
    let mut handled = response.handled || !response.actions.is_empty();
    let mut resync_inventory = false;
    for action in response.actions {
        let before_dimension = play_dimension.clone();
        handled |= chat::apply_plugin_action(
            sink,
            Some(server_config),
            world,
            world_rules,
            world_config,
            entities,
            players,
            plugins,
            player.profile.uuid,
            chunk_sender,
            chunk_state,
            position,
            next_teleport_id,
            play_dimension,
            menus,
            active_config_menu,
            players_hidden,
            visible_player_entities,
            viewer_position,
            render_distance,
            Some(&mut *inventory),
            Some(geyser_runtime),
            action,
        )
        .await?;
        if before_dimension != *play_dimension {
            resync_inventory = true;
        }
    }
    if resync_inventory {
        resync_inventory_state(
            sink,
            players,
            player.profile.uuid,
            player.entity_id,
            inventory,
        )
        .await?;
    }

    Ok(PluginNpcInteractOutcome { handled })
}

fn npc_interact_hand(interact: &Interact) -> NpcInteractHand {
    if interact.hand.0 == 1 {
        NpcInteractHand {
            name: "off_hand",
            action: "interact_off_hand",
        }
    } else {
        NpcInteractHand {
            name: "main_hand",
            action: "interact",
        }
    }
}

fn player_action_label(status: i32) -> &'static str {
    match status {
        PLAYER_ACTION_START_DESTROY_BLOCK => "start_destroy_block",
        PLAYER_ACTION_DROP_ITEM => "drop_item",
        PLAYER_ACTION_DROP_ITEM_STACK => "drop_item_stack",
        _ => "player_action",
    }
}

#[allow(clippy::too_many_arguments)]
async fn handle_vanilla_block_interaction<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    world: &WorldManager,
    world_rules: &crate::world::WorldRulesManager,
    players: &PlayerManager,
    fluid: &FluidRuntime,
    world_config: &qexed_config::app::qexed::server::World,
    game_mode: GameMode,
    play_dimension: &str,
    actor: uuid::Uuid,
    entity_id: i32,
    gameplay: &qexed_config::app::qexed::server::Gameplay,
    runtime: &mut gameplay::redstone::RedstoneRuntime,
    inventory: &mut crate::inventory::PlayerInventory,
    survival: &mut SurvivalState,
    position: BlockPosition,
    face: i32,
    block_name: &str,
) -> Result<bool>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    if !gameplay.block_updates {
        return Ok(false);
    }
    let Some(block_state) = world.block_state_at(play_dimension, &position) else {
        return Ok(false);
    };

    if let Some(updates) = manual_openable_block_updates(
        world,
        play_dimension,
        position.clone(),
        block_state,
        block_name,
    ) {
        let changed = apply_vanilla_block_state_updates(
            sink,
            world,
            world_rules,
            players,
            fluid,
            world_config,
            game_mode,
            play_dimension,
            actor,
            updates,
            gameplay.block_updates,
        )
        .await?;
        if !changed.is_empty() {
            propagate_redstone_from_block_positions(
                sink,
                world,
                world_rules,
                players,
                fluid,
                gameplay,
                runtime,
                play_dimension,
                actor,
                changed,
            )
            .await?;
        }
        return Ok(true);
    }

    let held_item = held_item_name(inventory);
    if let Some(interaction) = bucket_fluid_interaction(
        world,
        play_dimension,
        &position,
        block_state,
        block_name,
        held_item.as_deref(),
        face,
    ) {
        if game_mode != GameMode::Creative
            && !can_exchange_held_item(inventory, interaction.replacement_item)
        {
            return Ok(true);
        }
        let changed = apply_vanilla_block_state_updates(
            sink,
            world,
            world_rules,
            players,
            fluid,
            world_config,
            game_mode,
            play_dimension,
            actor,
            vec![(interaction.position.clone(), interaction.target_block_state)],
            gameplay.block_updates,
        )
        .await?;
        if !changed.is_empty() && game_mode != GameMode::Creative {
            if let Some(changes) = exchange_held_item(inventory, interaction.replacement_item) {
                sync_inventory_changes(
                    sink,
                    players,
                    actor,
                    entity_id,
                    inventory.selected_slot(),
                    changes,
                )
                .await?;
            }
        }
        if !changed.is_empty() {
            propagate_redstone_from_block_positions(
                sink,
                world,
                world_rules,
                players,
                fluid,
                gameplay,
                runtime,
                play_dimension,
                actor,
                changed,
            )
            .await?;
        }
        return Ok(true);
    }

    // Sapling bone meal: grow tree instantly
    if is_sapling(block_name)
        && held_item.as_deref().map(normalize_resource_key).as_deref()
            == Some("minecraft:bone_meal")
    {
        if let Some(tree_blocks) =
            sapling_bone_meal_blocks(world, play_dimension, &position, block_name)
        {
            if can_apply_vanilla_inventory_action(
                inventory,
                game_mode,
                VanillaInventoryAction::ConsumeHeld,
            ) {
                let changed = apply_vanilla_block_state_updates(
                    sink,
                    world,
                    world_rules,
                    players,
                    fluid,
                    world_config,
                    game_mode,
                    play_dimension,
                    actor,
                    tree_blocks,
                    gameplay.block_updates,
                )
                .await?;
                if !changed.is_empty() {
                    let changes = apply_vanilla_inventory_action(
                        inventory,
                        game_mode,
                        VanillaInventoryAction::ConsumeHeld,
                    );
                    if !changes.is_empty() {
                        sync_inventory_changes(
                            sink,
                            players,
                            actor,
                            entity_id,
                            inventory.selected_slot(),
                            changes,
                        )
                        .await?;
                    }
                }
            }
            send_bone_meal_particles(sink, position.clone()).await?;
            return Ok(true);
        }
        // Sapling can't grow (no space, blocked, etc.) — fall through to bone meal
        // on the ground which may grow flowers/grass.
    }

    // Grass block bone meal: if a sapling is on top, grow tree instead of flowers
    if is_grass_block(block_name)
        && held_item.as_deref().map(normalize_resource_key).as_deref()
            == Some("minecraft:bone_meal")
    {
        // Check if there's a sapling directly above — bone meal the sapling instead
        let above = offset_position(&position, 0, 1, 0);
        let above_name = world
            .block_state_at(play_dimension, &above)
            .and_then(|s| crate::inventory::block_name_for_state(s))
            .unwrap_or_default();
        if is_sapling(&above_name) {
            if let Some(tree_blocks) =
                sapling_bone_meal_blocks(world, play_dimension, &above, &above_name)
            {
                if can_apply_vanilla_inventory_action(
                    inventory,
                    game_mode,
                    VanillaInventoryAction::ConsumeHeld,
                ) {
                    let changed = apply_vanilla_block_state_updates(
                        sink,
                        world,
                        world_rules,
                        players,
                        fluid,
                        world_config,
                        game_mode,
                        play_dimension,
                        actor,
                        tree_blocks,
                        gameplay.block_updates,
                    )
                    .await?;
                    if !changed.is_empty() {
                        let changes = apply_vanilla_inventory_action(
                            inventory,
                            game_mode,
                            VanillaInventoryAction::ConsumeHeld,
                        );
                        if !changes.is_empty() {
                            sync_inventory_changes(
                                sink,
                                players,
                                actor,
                                entity_id,
                                inventory.selected_slot(),
                                changes,
                            )
                            .await?;
                        }
                    }
                }
                send_bone_meal_particles(sink, above.clone()).await?;
                return Ok(true);
            }
        }
        // No sapling above — grow flowers/grass
        if let Some(growth) = grass_bone_meal_growth(world, play_dimension, &position) {
            if can_apply_vanilla_inventory_action(
                inventory,
                game_mode,
                VanillaInventoryAction::ConsumeHeld,
            ) {
                let changed = apply_vanilla_block_state_updates(
                    sink,
                    world,
                    world_rules,
                    players,
                    fluid,
                    world_config,
                    game_mode,
                    play_dimension,
                    actor,
                    growth,
                    gameplay.block_updates,
                )
                .await?;
                if !changed.is_empty() {
                    let changes = apply_vanilla_inventory_action(
                        inventory,
                        game_mode,
                        VanillaInventoryAction::ConsumeHeld,
                    );
                    if !changes.is_empty() {
                        sync_inventory_changes(
                            sink,
                            players,
                            actor,
                            entity_id,
                            inventory.selected_slot(),
                            changes,
                        )
                        .await?;
                    }
                }
            }
            send_bone_meal_particles(sink, position.clone()).await?;
            return Ok(true);
        }
    }

    if let Some(interaction) = vanilla_state_interaction(
        world,
        play_dimension,
        &position,
        block_state,
        block_name,
        held_item.as_deref(),
        face,
    ) {
        if !can_apply_vanilla_inventory_action(inventory, game_mode, interaction.inventory) {
            return Ok(true);
        }
        let changed = apply_vanilla_block_state_updates(
            sink,
            world,
            world_rules,
            players,
            fluid,
            world_config,
            game_mode,
            play_dimension,
            actor,
            vec![(position.clone(), interaction.target_block_state)],
            gameplay.block_updates,
        )
        .await?;
        if !changed.is_empty() {
            let inventory_changes =
                apply_vanilla_inventory_action(inventory, game_mode, interaction.inventory);
            if !inventory_changes.is_empty() {
                sync_inventory_changes(
                    sink,
                    players,
                    actor,
                    entity_id,
                    inventory.selected_slot(),
                    inventory_changes,
                )
                .await?;
            }
        }
        if !changed.is_empty() {
            propagate_redstone_from_block_positions(
                sink,
                world,
                world_rules,
                players,
                fluid,
                gameplay,
                runtime,
                play_dimension,
                actor,
                changed,
            )
            .await?;
        }
        return Ok(true);
    }

    if let Some(next_state) = cake_interaction(block_state, block_name, game_mode, survival) {
        sink.send(survival.health_packet()).await?;
        let changed = apply_vanilla_block_state_updates(
            sink,
            world,
            world_rules,
            players,
            fluid,
            world_config,
            game_mode,
            play_dimension,
            actor,
            vec![(position.clone(), next_state)],
            gameplay.block_updates,
        )
        .await?;
        if !changed.is_empty() {
            propagate_redstone_from_block_positions(
                sink,
                world,
                world_rules,
                players,
                fluid,
                gameplay,
                runtime,
                play_dimension,
                actor,
                changed,
            )
            .await?;
        }
        return Ok(true);
    }

    if let Some(interaction) =
        composter_interaction(block_state, block_name, held_item.as_deref(), &position)
    {
        let mut inventory_changes = Vec::new();
        match interaction.inventory {
            ComposterInventoryAction::ConsumeHeld if game_mode != GameMode::Creative => {
                if inventory.held_item().item_count.0 <= 0 {
                    return Ok(false);
                }
                if let Some(change) = inventory.decrement_hotbar_slot(inventory.selected_slot(), 1)
                {
                    inventory_changes.push(change);
                }
            }
            ComposterInventoryAction::ConsumeHeld => {}
            ComposterInventoryAction::Give(item_name) if game_mode != GameMode::Creative => {
                let Some(item_id) = crate::inventory::item_id_for_name(item_name) else {
                    return Ok(true);
                };
                let item = crate::inventory::simple_item(item_id, 1);
                let Some(mut changes) = inventory.add_item_stack(&item) else {
                    return Ok(true);
                };
                inventory_changes.append(&mut changes);
            }
            ComposterInventoryAction::Give(_) => {}
        }

        let changed = if let Some(next_state) = interaction.target_block_state {
            apply_vanilla_block_state_updates(
                sink,
                world,
                world_rules,
                players,
                fluid,
                world_config,
                game_mode,
                play_dimension,
                actor,
                vec![(position.clone(), next_state)],
                gameplay.block_updates,
            )
            .await?
        } else {
            Vec::new()
        };
        if !inventory_changes.is_empty() {
            sync_inventory_changes(
                sink,
                players,
                actor,
                entity_id,
                inventory.selected_slot(),
                inventory_changes,
            )
            .await?;
        }
        if !changed.is_empty() {
            propagate_redstone_from_block_positions(
                sink,
                world,
                world_rules,
                players,
                fluid,
                gameplay,
                runtime,
                play_dimension,
                actor,
                changed,
            )
            .await?;
        }
        return Ok(true);
    }

    Ok(false)
}

#[allow(clippy::too_many_arguments)]
async fn apply_vanilla_block_state_updates<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    world: &WorldManager,
    world_rules: &crate::world::WorldRulesManager,
    players: &PlayerManager,
    fluid: &FluidRuntime,
    world_config: &qexed_config::app::qexed::server::World,
    game_mode: GameMode,
    dimension: &str,
    actor: uuid::Uuid,
    updates: Vec<(BlockPosition, i32)>,
    gameplay_block_updates: bool,
) -> Result<Vec<BlockPosition>>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    if world.read_only() {
        return Ok(Vec::new());
    }

    let mut changed = Vec::new();
    for (position, block_state) in updates {
        let edit_kind = if crate::inventory::is_air_block_state(block_state) {
            WorldEditKind::Break
        } else {
            WorldEditKind::Place
        };
        if apply_block_change(
            sink,
            world,
            world_rules,
            players,
            fluid,
            world_config,
            game_mode,
            dimension,
            actor,
            position.clone(),
            block_state,
            edit_kind,
            gameplay_block_updates,
        )
        .await?
        {
            changed.push(position);
        }
    }
    Ok(changed)
}

#[allow(clippy::too_many_arguments)]
async fn poll_apply_and_restart_fluid_tick<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    world: &WorldManager,
    world_rules: &crate::world::WorldRulesManager,
    players: &PlayerManager,
    fluid: &FluidRuntime,
    play_dimension: &str,
    actor: uuid::Uuid,
    viewer_position: EntityPosition,
    block_update_distance: f64,
    fluid_tick_job: &mut Option<FluidTickJob>,
    fluid_wake_sender: &tokio::sync::mpsc::UnboundedSender<()>,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let fluid_updates = fluid.poll_completed_tick_job(fluid_tick_job);
    if !fluid_updates.is_empty() {
        let applied_fluid_updates = apply_fluid_updates(
            sink,
            world,
            world_rules,
            players,
            fluid,
            play_dimension,
            actor,
            viewer_position,
            block_update_distance,
            fluid_updates,
        )
        .await?;
        if !applied_fluid_updates.is_empty() {
            sink.flush().await?;
        }
    }
    fluid.try_start_tick_job(world.clone(), fluid_tick_job, fluid_wake_sender);
    Ok(())
}

async fn apply_fluid_updates<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    world: &WorldManager,
    world_rules: &crate::world::WorldRulesManager,
    players: &PlayerManager,
    fluid: &FluidRuntime,
    play_dimension: &str,
    actor: uuid::Uuid,
    viewer_position: EntityPosition,
    block_update_distance: f64,
    updates: Vec<FluidBlockChange>,
) -> Result<Vec<BlockPosition>>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let mut changed = Vec::new();
    let mut viewer_changes = Vec::new();
    let mut broadcast_by_dimension: HashMap<String, Vec<crate::players::BlockChange>> =
        HashMap::new();
    let mut persist_by_dimension: HashMap<String, Vec<(BlockPosition, i32)>> = HashMap::new();
    for update in updates {
        if !world_rules.snapshot(&update.dimension).block_updates {
            continue;
        }
        if FluidKind::from_block_state(update.block_state).is_some() {
            fluid.enqueue_fluid_continuation(
                &update.dimension,
                &update.position,
                update.block_state,
            );
        }
        fluid.enqueue_block_change(&update.dimension, &update.position);
        if update.dimension == play_dimension
            && block_position_within_horizontal_distance(
                viewer_position,
                &update.position,
                block_update_distance,
            )
        {
            viewer_changes.push((update.position.clone(), update.block_state));
        }
        broadcast_by_dimension
            .entry(update.dimension.clone())
            .or_default()
            .push(crate::players::BlockChange {
                position: update.position.clone(),
                block_state: update.block_state,
            });
        persist_by_dimension
            .entry(update.dimension.clone())
            .or_default()
            .push((update.position.clone(), update.block_state));
        changed.push(update.position);
    }
    for (dimension, blocks) in persist_by_dimension {
        world.place_blocks_deferred(&dimension, blocks)?;
    }
    for packet in block_update_packets(viewer_changes)? {
        sink.send_raw(packet).await?;
    }
    for (dimension, changes) in broadcast_by_dimension {
        players.broadcast_block_changes(actor, &dimension, changes);
    }
    Ok(changed)
}

async fn apply_environment_block_state_updates<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    world: &WorldManager,
    world_rules: &crate::world::WorldRulesManager,
    players: &PlayerManager,
    fluid: &FluidRuntime,
    play_dimension: &str,
    actor: uuid::Uuid,
    updates: Vec<(BlockPosition, i32)>,
) -> Result<Vec<BlockPosition>>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let mut changed = Vec::new();
    for (position, block_state) in updates {
        if !world_rules.snapshot(play_dimension).block_updates {
            continue;
        }
        let current = world
            .block_state_at(play_dimension, &position)
            .unwrap_or_else(crate::inventory::air_block_state);
        if current == block_state {
            continue;
        }
        world.place_block(play_dimension, position.clone(), block_state);
        fluid.enqueue_block_change(play_dimension, &position);
        sink.send(crate::inventory::block_update(
            position.clone(),
            block_state,
        ))
        .await?;
        let light_update = if world.dynamic_light_enabled()
            && matches!(
                world_rules.snapshot(play_dimension).light,
                qexed_config::app::qexed::server::LightMode::Dynamic
            ) {
            let light = world.light_update(
                play_dimension,
                position.x.div_euclid(16),
                position.z.div_euclid(16),
            );
            sink.send(light.clone()).await?;
            Some(qexed_tcp_connect::PacketSink::<tokio::io::Sink>::build_send_packet(light)?)
        } else {
            None
        };
        players.broadcast_block_changed(
            actor,
            play_dimension,
            position.clone(),
            block_state,
            light_update,
        );
        changed.push(position);
    }
    Ok(changed)
}

fn environment_tick_updates(
    world: &WorldManager,
    dimension: &str,
    center: EntityPosition,
) -> Vec<(BlockPosition, i32)> {
    const HORIZONTAL_RADIUS: i32 = 4;
    const MIN_Y_OFFSET: i32 = -3;
    const MAX_Y_OFFSET: i32 = 2;
    const MAX_UPDATES: usize = 64;

    let center_block = BlockPosition {
        x: center.x.floor() as i32,
        y: center.y.floor() as i32,
        z: center.z.floor() as i32,
    };
    let mut updates = Vec::new();
    let mut updated_positions = HashSet::new();
    for y in (center_block.y + MIN_Y_OFFSET)..=(center_block.y + MAX_Y_OFFSET) {
        for x in (center_block.x - HORIZONTAL_RADIUS)..=(center_block.x + HORIZONTAL_RADIUS) {
            for z in (center_block.z - HORIZONTAL_RADIUS)..=(center_block.z + HORIZONTAL_RADIUS) {
                let position = BlockPosition { x, y, z };
                let Some(block_state) = world.block_state_at(dimension, &position) else {
                    continue;
                };
                let name = block_name(block_state);
                if is_fluid_block(&name) {
                    continue;
                }
                collect_environment_tick_updates(
                    world,
                    dimension,
                    &position,
                    block_state,
                    &mut updates,
                    &mut updated_positions,
                );
                if updates.len() >= MAX_UPDATES {
                    break;
                }
            }
        }
    }
    updates
}

fn collect_environment_tick_updates(
    world: &WorldManager,
    dimension: &str,
    position: &BlockPosition,
    block_state: i32,
    updates: &mut Vec<(BlockPosition, i32)>,
    updated_positions: &mut HashSet<(i32, i32, i32)>,
) {
    if let Some(next_state) = environment_next_state(world, dimension, position, block_state) {
        push_environment_update(position.clone(), next_state, updates, updated_positions);
    }
    if let Some((spread_position, spread_state)) =
        surface_spread_update(world, dimension, position, block_state)
    {
        push_environment_update(spread_position, spread_state, updates, updated_positions);
    }
    for (growth_position, growth_state) in
        column_plant_tick_updates(world, dimension, position, block_state)
    {
        push_environment_update(growth_position, growth_state, updates, updated_positions);
    }
    for (fire_position, fire_state) in fire_tick_updates(world, dimension, position, block_state) {
        push_environment_update(fire_position, fire_state, updates, updated_positions);
    }
    if let Some(next_state) = crop_random_tick(world, dimension, position, block_state) {
        push_environment_update(position.clone(), next_state, updates, updated_positions);
    }
    // Sapling growth: natural tree growth
    let name = block_name(block_state);
    if is_sapling(&name) {
        for (tree_pos, tree_state) in
            sapling_random_tick(world, dimension, position, block_state, &name)
        {
            push_environment_update(tree_pos, tree_state, updates, updated_positions);
        }
    }
}

fn crop_random_tick(
    world: &WorldManager,
    dimension: &str,
    position: &BlockPosition,
    block_state: i32,
) -> Option<i32> {
    // Only grow crops with ~14% chance per tick (approximates vanilla random tick rate)
    if rand::Rng::gen_range(&mut rand::thread_rng(), 0u32..100) < 14 {
        return None;
    }
    let name = block_name(block_state);
    let max_age = crop_max_age(&name)?;
    let age = block_state_u8_property(block_state, "age")?;
    if age >= max_age {
        return None;
    }
    // Check if crop is on farmland (required for most crops)
    if crop_needs_farmland(&name) && !is_on_farmland(world, dimension, position) {
        return None;
    }
    // Check light level (crops need light to grow)
    if !position_has_sky_light(world, dimension, position) {
        return None;
    }
    let new_age = (age + 1).min(max_age);
    block_state_with_property(block_state, "age", &new_age.to_string())
}

fn crop_max_age(name: &str) -> Option<u8> {
    match name {
        "minecraft:wheat"
        | "minecraft:carrots"
        | "minecraft:potatoes"
        | "minecraft:melon_stem"
        | "minecraft:pumpkin_stem" => Some(7),
        "minecraft:beetroots" | "minecraft:sweet_berry_bush" => Some(3),
        "minecraft:cocoa" => Some(2),
        "minecraft:torchflower_crop" => Some(1),
        "minecraft:pitcher_crop" => Some(4),
        "minecraft:nether_wart" => Some(3),
        _ => None,
    }
}

fn crop_needs_farmland(name: &str) -> bool {
    matches!(
        name,
        "minecraft:wheat"
            | "minecraft:carrots"
            | "minecraft:potatoes"
            | "minecraft:beetroots"
            | "minecraft:melon_stem"
            | "minecraft:pumpkin_stem"
            | "minecraft:torchflower_crop"
            | "minecraft:pitcher_crop"
    )
}

fn is_on_farmland(world: &WorldManager, dimension: &str, position: &BlockPosition) -> bool {
    let below = offset_position(position, 0, -1, 0);
    world
        .block_state_at(dimension, &below)
        .map(|state| block_name(state) == "minecraft:farmland")
        .unwrap_or(false)
}

fn position_has_sky_light(world: &WorldManager, dimension: &str, position: &BlockPosition) -> bool {
    // Check that there's no solid block above the crop
    let mut check = offset_position(position, 0, 1, 0);
    // Check up to 16 blocks above for sky access
    for _ in 0..16 {
        let state = world
            .block_state_at(dimension, &check)
            .unwrap_or_else(crate::inventory::air_block_state);
        if !crate::inventory::is_air_block_state(state)
            && !crate::inventory::can_replace_block_state(state)
        {
            return false;
        }
        check = offset_position(&check, 0, 1, 0);
    }
    true
}

fn push_environment_update(
    position: BlockPosition,
    block_state: i32,
    updates: &mut Vec<(BlockPosition, i32)>,
    updated_positions: &mut HashSet<(i32, i32, i32)>,
) {
    if !updated_positions.insert((position.x, position.y, position.z)) {
        return;
    }
    updates.push((position, block_state));
}

fn environment_next_state(
    world: &WorldManager,
    dimension: &str,
    position: &BlockPosition,
    block_state: i32,
) -> Option<i32> {
    farmland_next_state(world, dimension, position, block_state)
        .or_else(|| covered_surface_next_state(world, dimension, position, block_state))
}

fn farmland_next_state(
    world: &WorldManager,
    dimension: &str,
    position: &BlockPosition,
    block_state: i32,
) -> Option<i32> {
    if block_name(block_state) != "minecraft:farmland" {
        return None;
    }
    let moisture = block_state_u8_property(block_state, "moisture")
        .unwrap_or(0)
        .min(7);
    if farmland_has_water_nearby(world, dimension, position) {
        return (moisture < 7).then(|| farmland_state_for_moisture(7));
    }
    if moisture > 0 {
        return Some(farmland_state_for_moisture(moisture - 1));
    }
    (!farmland_has_crop_above(world, dimension, position))
        .then(|| crate::world::chunk_nbt::default_block_state_id("minecraft:dirt"))
}

fn farmland_has_water_nearby(
    _world: &WorldManager,
    _dimension: &str,
    _position: &BlockPosition,
) -> bool {
    false
}

fn farmland_has_crop_above(
    world: &WorldManager,
    dimension: &str,
    position: &BlockPosition,
) -> bool {
    let above = offset_position(position, 0, 1, 0);
    world
        .block_state_at(dimension, &above)
        .map(block_name)
        .is_some_and(|name| {
            matches!(
                name.as_str(),
                "minecraft:wheat"
                    | "minecraft:carrots"
                    | "minecraft:potatoes"
                    | "minecraft:beetroots"
                    | "minecraft:pumpkin_stem"
                    | "minecraft:melon_stem"
                    | "minecraft:attached_pumpkin_stem"
                    | "minecraft:attached_melon_stem"
                    | "minecraft:torchflower_crop"
                    | "minecraft:pitcher_crop"
            )
        })
}

fn farmland_state_for_moisture(moisture: u8) -> i32 {
    crate::world::chunk_nbt::block_state(
        "minecraft:farmland",
        &[("moisture".to_string(), moisture.min(7).to_string())],
    )
    .id
}

fn covered_surface_next_state(
    world: &WorldManager,
    dimension: &str,
    position: &BlockPosition,
    block_state: i32,
) -> Option<i32> {
    let name = block_name(block_state);
    match name.as_str() {
        "minecraft:grass_block" | "minecraft:mycelium" => {
            (!surface_above_allows_survival(world, dimension, position))
                .then(|| crate::world::chunk_nbt::default_block_state_id("minecraft:dirt"))
        }
        "minecraft:dirt_path" => {
            (!block_above_allows_surface_transform(world, dimension, position))
                .then(|| crate::world::chunk_nbt::default_block_state_id("minecraft:dirt"))
        }
        _ => None,
    }
}

fn surface_spread_update(
    world: &WorldManager,
    dimension: &str,
    position: &BlockPosition,
    block_state: i32,
) -> Option<(BlockPosition, i32)> {
    let source_name = block_name(block_state);
    if !matches!(
        source_name.as_str(),
        "minecraft:grass_block" | "minecraft:mycelium"
    ) || !surface_above_allows_survival(world, dimension, position)
    {
        return None;
    }

    const OFFSETS: [(i32, i32, i32); 26] = [
        (-1, -1, -1),
        (-1, -1, 0),
        (-1, -1, 1),
        (0, -1, -1),
        (0, -1, 0),
        (0, -1, 1),
        (1, -1, -1),
        (1, -1, 0),
        (1, -1, 1),
        (-1, 0, -1),
        (-1, 0, 0),
        (-1, 0, 1),
        (0, 0, -1),
        (0, 0, 1),
        (1, 0, -1),
        (1, 0, 0),
        (1, 0, 1),
        (-1, 1, -1),
        (-1, 1, 0),
        (-1, 1, 1),
        (0, 1, -1),
        (0, 1, 0),
        (0, 1, 1),
        (1, 1, -1),
        (1, 1, 0),
        (1, 1, 1),
    ];
    let start = surface_spread_start(position, block_state) % OFFSETS.len();
    for index in 0..OFFSETS.len() {
        let (dx, dy, dz) = OFFSETS[(start + index) % OFFSETS.len()];
        let target = offset_position(position, dx, dy, dz);
        let Some(target_state) = world.block_state_at(dimension, &target) else {
            continue;
        };
        if block_name(target_state) != "minecraft:dirt"
            || !surface_above_allows_survival(world, dimension, &target)
        {
            continue;
        }
        let spread_state = surface_spread_state(
            &source_name,
            surface_has_snow_above(world, dimension, &target),
        )?;
        return Some((target, spread_state));
    }
    None
}

fn surface_spread_start(position: &BlockPosition, block_state: i32) -> usize {
    let mut hash = 0xcbf29ce484222325u64;
    for value in [position.x, position.y, position.z, block_state] {
        hash ^= value as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash as usize
}

fn surface_spread_state(block_name: &str, snowy: bool) -> Option<i32> {
    match block_name {
        "minecraft:grass_block" | "minecraft:mycelium" => block_state_with_exact_properties(
            block_name,
            &[("snowy".to_string(), bool_value(snowy).to_string())],
        )
        .or_else(|| Some(crate::world::chunk_nbt::default_block_state_id(block_name))),
        _ => None,
    }
}

fn surface_above_allows_survival(
    world: &WorldManager,
    dimension: &str,
    position: &BlockPosition,
) -> bool {
    let above = offset_position(position, 0, 1, 0);
    let above_state = world
        .block_state_at(dimension, &above)
        .unwrap_or_else(crate::inventory::air_block_state);
    if crate::inventory::can_replace_block_state(above_state) {
        return true;
    }
    let above_name = block_name(above_state);
    is_snow_cover_block(&above_name) || !crate::inventory::block_has_collision(above_state)
}

fn surface_has_snow_above(world: &WorldManager, dimension: &str, position: &BlockPosition) -> bool {
    let above = offset_position(position, 0, 1, 0);
    world
        .block_state_at(dimension, &above)
        .map(block_name)
        .is_some_and(|name| is_snow_cover_block(&name))
}

fn is_snow_cover_block(block_name: &str) -> bool {
    matches!(block_name, "minecraft:snow" | "minecraft:snow_block")
}

fn column_plant_tick_updates(
    world: &WorldManager,
    dimension: &str,
    position: &BlockPosition,
    block_state: i32,
) -> Vec<(BlockPosition, i32)> {
    let name = block_name(block_state);
    if !matches!(name.as_str(), "minecraft:sugar_cane" | "minecraft:cactus")
        || column_plant_has_same_block_above(world, dimension, position, &name)
    {
        return Vec::new();
    }

    let age = block_state_u8_property(block_state, "age")
        .unwrap_or(0)
        .min(15);
    if age < 15 {
        return block_state_with_property(block_state, "age", &(age + 1).to_string())
            .filter(|next| *next != block_state)
            .map(|next| vec![(position.clone(), next)])
            .unwrap_or_default();
    }

    if !column_plant_can_grow(world, dimension, position, &name) {
        return Vec::new();
    }

    let Some(reset_state) = block_state_with_property(block_state, "age", "0") else {
        return Vec::new();
    };
    let above = offset_position(position, 0, 1, 0);
    let new_state = column_plant_new_block_state(&name);
    vec![(position.clone(), reset_state), (above, new_state)]
}

fn column_plant_has_same_block_above(
    world: &WorldManager,
    dimension: &str,
    position: &BlockPosition,
    block_name: &str,
) -> bool {
    let above = offset_position(position, 0, 1, 0);
    world
        .block_state_at(dimension, &above)
        .map(block_name_for_manual_state)
        .is_some_and(|name| name == block_name)
}

fn column_plant_can_grow(
    world: &WorldManager,
    dimension: &str,
    position: &BlockPosition,
    block_name: &str,
) -> bool {
    let above = offset_position(position, 0, 1, 0);
    let above_state = world
        .block_state_at(dimension, &above)
        .unwrap_or_else(crate::inventory::air_block_state);
    if !crate::inventory::can_replace_block_state(above_state) {
        return false;
    }
    let height = column_plant_height(world, dimension, position, block_name);
    match block_name {
        "minecraft:sugar_cane" => {
            height < 3 && sugar_cane_has_valid_support_and_water(world, dimension, position, height)
        }
        "minecraft:cactus" => {
            height < 3
                && cactus_has_valid_support(world, dimension, position, height)
                && cactus_growth_space_clear(world, dimension, &above)
        }
        _ => false,
    }
}

fn column_plant_height(
    world: &WorldManager,
    dimension: &str,
    position: &BlockPosition,
    block_name: &str,
) -> usize {
    let mut height = 1usize;
    let mut below = offset_position(position, 0, -1, 0);
    while block_name_at(world, dimension, &below).as_deref() == Some(block_name) {
        height += 1;
        below = offset_position(&below, 0, -1, 0);
        if height >= 32 {
            break;
        }
    }
    height
}

fn column_plant_base_position(position: &BlockPosition, height: usize) -> BlockPosition {
    offset_position(position, 0, -((height as i32) - 1), 0)
}

fn sugar_cane_has_valid_support_and_water(
    world: &WorldManager,
    dimension: &str,
    position: &BlockPosition,
    height: usize,
) -> bool {
    let base = column_plant_base_position(position, height);
    let support = offset_position(&base, 0, -1, 0);
    let Some(support_name) = block_name_at(world, dimension, &support) else {
        return false;
    };
    is_sugar_cane_support_block(&support_name)
}

fn is_sugar_cane_support_block(block_name: &str) -> bool {
    matches!(
        block_name,
        "minecraft:grass_block"
            | "minecraft:dirt"
            | "minecraft:coarse_dirt"
            | "minecraft:rooted_dirt"
            | "minecraft:podzol"
            | "minecraft:mycelium"
            | "minecraft:sand"
            | "minecraft:red_sand"
            | "minecraft:moss_block"
            | "minecraft:mud"
    )
}

fn cactus_has_valid_support(
    world: &WorldManager,
    dimension: &str,
    position: &BlockPosition,
    height: usize,
) -> bool {
    let base = column_plant_base_position(position, height);
    let support = offset_position(&base, 0, -1, 0);
    block_name_at(world, dimension, &support)
        .is_some_and(|name| matches!(name.as_str(), "minecraft:sand" | "minecraft:red_sand"))
}

fn cactus_growth_space_clear(
    world: &WorldManager,
    dimension: &str,
    position: &BlockPosition,
) -> bool {
    for (dx, dz) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
        let side = offset_position(position, dx, 0, dz);
        let side_state = world
            .block_state_at(dimension, &side)
            .unwrap_or_else(crate::inventory::air_block_state);
        if crate::inventory::block_has_collision(side_state)
            && !crate::inventory::can_replace_block_state(side_state)
        {
            return false;
        }
    }
    true
}

fn column_plant_new_block_state(block_name: &str) -> i32 {
    let default_state = crate::world::chunk_nbt::default_block_state_id(block_name);
    block_state_with_property(default_state, "age", "0").unwrap_or(default_state)
}

fn fire_tick_updates(
    world: &WorldManager,
    dimension: &str,
    position: &BlockPosition,
    block_state: i32,
) -> Vec<(BlockPosition, i32)> {
    let name = block_name(block_state);
    match name.as_str() {
        "minecraft:soul_fire" => {
            if fire_block_below(world, dimension, position)
                .is_some_and(|below| is_soul_fire_base(&below))
            {
                Vec::new()
            } else {
                vec![(position.clone(), crate::inventory::air_block_state())]
            }
        }
        "minecraft:fire" => {
            fire_tick_updates_for_normal_fire(world, dimension, position, block_state)
        }
        _ => Vec::new(),
    }
}

fn fire_tick_updates_for_normal_fire(
    world: &WorldManager,
    dimension: &str,
    position: &BlockPosition,
    block_state: i32,
) -> Vec<(BlockPosition, i32)> {
    let below = fire_block_below(world, dimension, position);
    let eternal_base = below.as_deref().is_some_and(is_eternal_fire_base);
    let has_fuel = fire_has_flammable_neighbor(world, dimension, position);
    if !eternal_base && !has_fuel {
        return vec![(position.clone(), crate::inventory::air_block_state())];
    }

    let mut updates = Vec::new();
    let age = block_state_u8_property(block_state, "age")
        .unwrap_or(0)
        .min(15);
    if age < 15 {
        if let Some(next_state) =
            block_state_with_property(block_state, "age", &(age + 1).to_string())
        {
            updates.push((position.clone(), next_state));
        }
    }
    if let Some(spread_position) = fire_spread_position(world, dimension, position, block_state) {
        updates.push((spread_position, fire_state_for_age(0)));
    }
    updates
}

fn fire_block_below(
    world: &WorldManager,
    dimension: &str,
    position: &BlockPosition,
) -> Option<String> {
    let below = offset_position(position, 0, -1, 0);
    block_name_at(world, dimension, &below)
}

fn fire_has_flammable_neighbor(
    world: &WorldManager,
    dimension: &str,
    position: &BlockPosition,
) -> bool {
    fire_neighbor_offsets().iter().any(|(dx, dy, dz)| {
        let neighbor = offset_position(position, *dx, *dy, *dz);
        block_name_at(world, dimension, &neighbor)
            .as_deref()
            .is_some_and(is_flammable_block)
    })
}

fn fire_spread_position(
    world: &WorldManager,
    dimension: &str,
    position: &BlockPosition,
    block_state: i32,
) -> Option<BlockPosition> {
    let offsets = fire_neighbor_offsets();
    let start = surface_spread_start(position, block_state) % offsets.len();
    for index in 0..offsets.len() {
        let (dx, dy, dz) = offsets[(start + index) % offsets.len()];
        let target = offset_position(position, dx, dy, dz);
        let target_state = world
            .block_state_at(dimension, &target)
            .unwrap_or_else(crate::inventory::air_block_state);
        if !crate::inventory::can_replace_block_state(target_state)
            || !fire_has_flammable_neighbor(world, dimension, &target)
        {
            continue;
        }
        return Some(target);
    }
    None
}

fn fire_neighbor_offsets() -> &'static [(i32, i32, i32); 6] {
    &[
        (-1, 0, 0),
        (1, 0, 0),
        (0, -1, 0),
        (0, 1, 0),
        (0, 0, -1),
        (0, 0, 1),
    ]
}

fn fire_state_for_age(age: u8) -> i32 {
    let default_state = crate::world::chunk_nbt::default_block_state_id("minecraft:fire");
    block_state_with_property(default_state, "age", &age.min(15).to_string())
        .unwrap_or(default_state)
}

fn is_eternal_fire_base(block_name: &str) -> bool {
    matches!(block_name, "minecraft:netherrack" | "minecraft:magma_block")
}

fn is_soul_fire_base(block_name: &str) -> bool {
    matches!(block_name, "minecraft:soul_sand" | "minecraft:soul_soil")
}

fn is_flammable_block(block_name: &str) -> bool {
    is_flammable_wood_family_block(block_name)
        || block_name.ends_with("_leaves")
        || block_name.ends_with("_wool")
        || block_name.ends_with("_carpet")
        || matches!(
            block_name,
            "minecraft:bookshelf"
                | "minecraft:chiseled_bookshelf"
                | "minecraft:lectern"
                | "minecraft:crafting_table"
                | "minecraft:chest"
                | "minecraft:trapped_chest"
                | "minecraft:barrel"
                | "minecraft:beehive"
                | "minecraft:bee_nest"
                | "minecraft:hay_block"
                | "minecraft:bamboo"
                | "minecraft:scaffolding"
                | "minecraft:dead_bush"
                | "minecraft:grass"
                | "minecraft:tall_grass"
                | "minecraft:fern"
                | "minecraft:large_fern"
                | "minecraft:azalea"
                | "minecraft:flowering_azalea"
                | "minecraft:dry_grass"
        )
}

fn is_flammable_wood_family_block(block_name: &str) -> bool {
    let mut local = block_name.strip_prefix("minecraft:").unwrap_or(block_name);
    if let Some(stripped) = local.strip_prefix("stripped_") {
        local = stripped;
    }
    const WOOD_FAMILIES: [&str; 10] = [
        "oak", "spruce", "birch", "jungle", "acacia", "dark_oak", "mangrove", "cherry", "pale_oak",
        "bamboo",
    ];
    let Some(family) = WOOD_FAMILIES
        .iter()
        .find(|family| local == **family || local.starts_with(&format!("{family}_")))
    else {
        return false;
    };
    let suffix = local.strip_prefix(family).unwrap_or(local);
    matches!(
        suffix,
        "_planks"
            | "_log"
            | "_wood"
            | "_stem"
            | "_hyphae"
            | "_block"
            | "_mosaic"
            | "_slab"
            | "_mosaic_slab"
            | "_stairs"
            | "_mosaic_stairs"
            | "_fence"
            | "_fence_gate"
            | "_door"
            | "_trapdoor"
            | "_sign"
            | "_wall_sign"
            | "_hanging_sign"
            | "_wall_hanging_sign"
            | "_button"
            | "_pressure_plate"
    )
}

// ── Sapling growth ──

/// Returns the tree blocks to place when a sapling grows naturally.
/// The sapling position itself is replaced with air (the caller handles this).
fn sapling_random_tick(
    world: &WorldManager,
    dimension: &str,
    position: &BlockPosition,
    _block_state: i32,
    block_name: &str,
) -> Vec<(BlockPosition, i32)> {
    if !is_sapling(block_name) {
        return Vec::new();
    }
    // ~5% chance per tick
    if rand::Rng::gen_range(&mut rand::thread_rng(), 0u32..100) >= 5 {
        return Vec::new();
    }
    // Check light — at least sky access
    if !position_has_sky_light(world, dimension, position) {
        return Vec::new();
    }
    // Check there's enough space above for the tree
    if !sapling_has_growth_space(world, dimension, position, block_name) {
        return Vec::new();
    }
    // Place tree blocks: logs + leaves, sapling replaced by air via environment tick
    let mut tree_blocks = grow_tree_blocks(position, block_name);
    // Replace sapling itself with air (first log will be at sapling position)
    tree_blocks.insert(0, (position.clone(), crate::inventory::air_block_state()));
    tree_blocks
}

pub(super) fn is_sapling(block_name: &str) -> bool {
    matches!(
        block_name,
        "minecraft:oak_sapling"
            | "minecraft:birch_sapling"
            | "minecraft:spruce_sapling"
            | "minecraft:jungle_sapling"
            | "minecraft:acacia_sapling"
            | "minecraft:dark_oak_sapling"
            | "minecraft:cherry_sapling"
            | "minecraft:mangrove_propagule"
    )
}

/// Check if there's enough air space above the sapling for a tree to grow
fn sapling_has_growth_space(
    world: &WorldManager,
    dimension: &str,
    position: &BlockPosition,
    sapling_name: &str,
) -> bool {
    let min_height = sapling_min_log_height(sapling_name) + 2;
    // Start at y+1 — the sapling itself is at position.y and will be replaced
    for y_off in 1..=min_height {
        let check = BlockPosition {
            x: position.x,
            y: position.y + y_off,
            z: position.z,
        };
        let state = world
            .block_state_at(dimension, &check)
            .unwrap_or_else(crate::inventory::air_block_state);
        if !crate::inventory::can_replace_block_state(state)
            && !crate::inventory::is_air_block_state(state)
        {
            return false;
        }
    }
    true
}

fn sapling_min_log_height(sapling_name: &str) -> i32 {
    match sapling_name {
        "minecraft:spruce_sapling" => 6,
        "minecraft:jungle_sapling" => 7,
        "minecraft:dark_oak_sapling" => 5,
        _ => 4,
    }
}

fn sapling_log_name(sapling_name: &str) -> &str {
    match sapling_name {
        "minecraft:birch_sapling" => "minecraft:birch_log",
        "minecraft:spruce_sapling" => "minecraft:spruce_log",
        "minecraft:jungle_sapling" => "minecraft:jungle_log",
        "minecraft:acacia_sapling" => "minecraft:acacia_log",
        "minecraft:dark_oak_sapling" => "minecraft:dark_oak_log",
        "minecraft:cherry_sapling" => "minecraft:cherry_log",
        "minecraft:mangrove_propagule" => "minecraft:mangrove_log",
        _ => "minecraft:oak_log",
    }
}

fn sapling_leaves_name(sapling_name: &str) -> &str {
    match sapling_name {
        "minecraft:birch_sapling" => "minecraft:birch_leaves",
        "minecraft:spruce_sapling" => "minecraft:spruce_leaves",
        "minecraft:jungle_sapling" => "minecraft:jungle_leaves",
        "minecraft:acacia_sapling" => "minecraft:acacia_leaves",
        "minecraft:dark_oak_sapling" => "minecraft:dark_oak_leaves",
        "minecraft:cherry_sapling" => "minecraft:cherry_leaves",
        "minecraft:mangrove_propagule" => "minecraft:mangrove_leaves",
        _ => "minecraft:oak_leaves",
    }
}

/// Generate the list of (position, block_id) for a tree grown from a sapling.
/// Tree shapes are simplified approximations of vanilla trees.
/// The sapling is at `position`; first log block replaces it.
pub(super) fn grow_tree_blocks(
    sapling_pos: &BlockPosition,
    sapling_name: &str,
) -> Vec<(BlockPosition, i32)> {
    let log_name = sapling_log_name(sapling_name);
    let leaves_name = sapling_leaves_name(sapling_name);
    let log = crate::world::chunk_nbt::default_block_state(log_name).id;
    let leaves = crate::world::chunk_nbt::default_block_state(leaves_name).id;

    let trunk_height = match sapling_name {
        "minecraft:spruce_sapling" => {
            6 + (rand::Rng::gen_range(&mut rand::thread_rng(), 0u32..5) as i32)
        }
        "minecraft:jungle_sapling" => {
            7 + (rand::Rng::gen_range(&mut rand::thread_rng(), 0u32..5) as i32)
        }
        "minecraft:dark_oak_sapling" => {
            5 + (rand::Rng::gen_range(&mut rand::thread_rng(), 0u32..2) as i32)
        }
        "minecraft:birch_sapling" => {
            5 + (rand::Rng::gen_range(&mut rand::thread_rng(), 0u32..3) as i32)
        }
        "minecraft:acacia_sapling" => {
            5 + (rand::Rng::gen_range(&mut rand::thread_rng(), 0u32..4) as i32)
        }
        _ => 4 + (rand::Rng::gen_range(&mut rand::thread_rng(), 0u32..2) as i32), // oak, cherry
    };

    let mut blocks = Vec::new();

    // Place trunk logs
    for y_off in 0..=trunk_height {
        blocks.push((
            BlockPosition {
                x: sapling_pos.x,
                y: sapling_pos.y + y_off,
                z: sapling_pos.z,
            },
            log,
        ));
    }

    // Place leaves canopy
    let canopy_bottom = trunk_height - 2;
    let canopy_top = trunk_height + 1;
    let canopy_radius = if sapling_name == "minecraft:spruce_sapling" {
        2 // narrower
    } else if sapling_name == "minecraft:acacia_sapling" {
        3 // wider flat
    } else {
        2
    };

    for y_off in canopy_bottom..=canopy_top {
        let y = sapling_pos.y + y_off;
        let radius = if sapling_name == "minecraft:spruce_sapling" {
            // Spruce: pyramid shape, narrower at top
            ((canopy_top - y_off) as i32 + 1).min(2)
        } else if sapling_name == "minecraft:acacia_sapling" {
            // Acacia: flat top
            if y_off >= canopy_top - 1 { 3 } else { 2 }
        } else {
            if y_off == canopy_top {
                1
            } else if y_off <= canopy_bottom + 1 {
                2
            } else {
                2
            }
        };

        for dx in -radius..=radius {
            for dz in -radius..=radius {
                // Skip corners for a more rounded shape
                if radius > 1 && (dx.abs() == radius && dz.abs() == radius) {
                    if rand::Rng::gen_range(&mut rand::thread_rng(), 0u32..3) == 0 {
                        continue;
                    }
                }
                // Don't overwrite log blocks
                if dx == 0 && dz == 0 && y_off <= trunk_height {
                    continue;
                }
                blocks.push((
                    BlockPosition {
                        x: sapling_pos.x + dx,
                        y,
                        z: sapling_pos.z + dz,
                    },
                    leaves,
                ));
            }
        }
    }

    // Top leaves
    let top_y = sapling_pos.y + trunk_height + 1;
    if sapling_name != "minecraft:spruce_sapling" {
        blocks.push((
            BlockPosition {
                x: sapling_pos.x,
                y: top_y,
                z: sapling_pos.z,
            },
            leaves,
        ));
        for dx in -1..=1 {
            for dz in -1..=1 {
                if dx == 0 && dz == 0 {
                    continue;
                }
                blocks.push((
                    BlockPosition {
                        x: sapling_pos.x + dx,
                        y: top_y,
                        z: sapling_pos.z + dz,
                    },
                    leaves,
                ));
            }
        }
    }

    blocks
}

/// Handle bone meal on a sapling — returns the blocks to place for a full tree.
pub(super) fn sapling_bone_meal_blocks(
    world: &WorldManager,
    dimension: &str,
    position: &BlockPosition,
    sapling_name: &str,
) -> Option<Vec<(BlockPosition, i32)>> {
    if !is_sapling(sapling_name) {
        return None;
    }
    if !sapling_has_growth_space(world, dimension, position, sapling_name) {
        return None;
    }
    let mut tree_blocks = grow_tree_blocks(position, sapling_name);
    // Replace sapling with trunk base
    tree_blocks.insert(0, (position.clone(), crate::inventory::air_block_state()));
    Some(tree_blocks)
}

fn manual_openable_block_updates(
    world: &WorldManager,
    dimension: &str,
    position: BlockPosition,
    block_state: i32,
    block_name: &str,
) -> Option<Vec<(BlockPosition, i32)>> {
    if !manual_openable_block(block_name) || !block_state_bool_property_exists(block_state, "open")
    {
        return None;
    }
    let open = block_state_bool_property(block_state, "open");
    let next = block_state_with_property(block_state, "open", bool_value(!open))?;
    let mut updates = vec![(position.clone(), next)];

    if is_door_block(block_name) {
        let other = if block_state_property(block_state, "half").as_deref() == Some("upper") {
            offset_position(&position, 0, -1, 0)
        } else {
            offset_position(&position, 0, 1, 0)
        };
        if let Some(other_state) = world.block_state_at(dimension, &other)
            && block_name_for_manual_state(other_state) == block_name
            && block_state_bool_property_exists(other_state, "open")
            && let Some(next_other) =
                block_state_with_property(other_state, "open", bool_value(!open))
        {
            updates.push((other, next_other));
        }
    }

    Some(updates)
}

fn manual_openable_block(block_name: &str) -> bool {
    (is_door_block(block_name) && block_name != "minecraft:iron_door")
        || (block_name.ends_with("_trapdoor") && block_name != "minecraft:iron_trapdoor")
        || block_name.ends_with("_fence_gate")
}

fn is_door_block(block_name: &str) -> bool {
    block_name.ends_with("_door")
}

#[derive(Debug, Clone, PartialEq)]
struct BucketFluidInteraction {
    position: BlockPosition,
    target_block_state: i32,
    replacement_item: &'static str,
}

fn bucket_fluid_interaction(
    world: &WorldManager,
    dimension: &str,
    clicked_position: &BlockPosition,
    clicked_state: i32,
    clicked_block_name: &str,
    held_item: Option<&str>,
    face: i32,
) -> Option<BucketFluidInteraction> {
    if is_cauldron_block(clicked_block_name) {
        return None;
    }
    let held_item = normalize_resource_key(held_item?).to_ascii_lowercase();
    match held_item.as_str() {
        "minecraft:bucket" => {
            bucket_fill_interaction(clicked_position, clicked_state, clicked_block_name)
        }
        "minecraft:lava_bucket" => bucket_empty_interaction(
            world,
            dimension,
            clicked_position,
            clicked_state,
            face,
            "minecraft:lava",
            "minecraft:bucket",
        ),
        "minecraft:powder_snow_bucket" => bucket_empty_interaction(
            world,
            dimension,
            clicked_position,
            clicked_state,
            face,
            "minecraft:powder_snow",
            "minecraft:bucket",
        ),
        _ => None,
    }
}

fn bucket_fill_interaction(
    clicked_position: &BlockPosition,
    clicked_state: i32,
    clicked_block_name: &str,
) -> Option<BucketFluidInteraction> {
    let replacement_item = match clicked_block_name {
        "minecraft:lava" if block_state_u8_property(clicked_state, "level") == Some(0) => {
            "minecraft:lava_bucket"
        }
        "minecraft:powder_snow" => "minecraft:powder_snow_bucket",
        _ => return None,
    };
    Some(BucketFluidInteraction {
        position: clicked_position.clone(),
        target_block_state: crate::inventory::air_block_state(),
        replacement_item,
    })
}

fn bucket_empty_interaction(
    world: &WorldManager,
    dimension: &str,
    clicked_position: &BlockPosition,
    clicked_state: i32,
    face: i32,
    fluid_block_name: &'static str,
    replacement_item: &'static str,
) -> Option<BucketFluidInteraction> {
    let target = if crate::inventory::can_replace_block_state(clicked_state) {
        clicked_position.clone()
    } else {
        crate::inventory::placement_position(clicked_position, face)
    };
    let target_state = world
        .block_state_at(dimension, &target)
        .unwrap_or_else(crate::inventory::air_block_state);
    if !crate::inventory::can_replace_block_state(target_state) {
        return None;
    }
    Some(BucketFluidInteraction {
        position: target,
        target_block_state: crate::world::chunk_nbt::default_block_state_id(fluid_block_name),
        replacement_item,
    })
}

fn is_cauldron_block(block_name: &str) -> bool {
    matches!(
        block_name,
        "minecraft:cauldron" | "minecraft:lava_cauldron" | "minecraft:powder_snow_cauldron"
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct VanillaStateInteraction {
    target_block_state: i32,
    inventory: VanillaInventoryAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VanillaInventoryAction {
    None,
    ConsumeHeld,
    Give(&'static str, i32),
    ExchangeHeld(&'static str),
}

fn vanilla_state_interaction(
    world: &WorldManager,
    dimension: &str,
    position: &BlockPosition,
    block_state: i32,
    block_name: &str,
    held_item: Option<&str>,
    face: i32,
) -> Option<VanillaStateInteraction> {
    if let Some(interaction) = candle_placement_interaction(block_state, block_name, held_item) {
        return Some(interaction);
    }

    if let Some(interaction) = beehive_interaction(block_state, block_name, held_item) {
        return Some(interaction);
    }

    if let Some(interaction) = pumpkin_shear_interaction(block_state, block_name, held_item, face) {
        return Some(interaction);
    }

    if let Some(target_block_state) = note_block_interaction(block_state, block_name) {
        return Some(VanillaStateInteraction {
            target_block_state,
            inventory: VanillaInventoryAction::None,
        });
    }

    if let Some(interaction) = bone_meal_interaction(block_state, block_name, held_item) {
        return Some(interaction);
    }

    if let Some(interaction) = harvestable_block_interaction(block_state, block_name) {
        return Some(interaction);
    }

    if let Some(interaction) = lit_block_interaction(block_state, block_name, held_item) {
        return Some(interaction);
    }

    let held_item = held_item?;
    tool_block_interaction(
        world,
        dimension,
        position,
        block_state,
        block_name,
        held_item,
        face,
    )
    .map(|target_block_state| VanillaStateInteraction {
        target_block_state,
        inventory: VanillaInventoryAction::None,
    })
}

fn can_apply_vanilla_inventory_action(
    inventory: &crate::inventory::PlayerInventory,
    game_mode: GameMode,
    action: VanillaInventoryAction,
) -> bool {
    match action {
        VanillaInventoryAction::None => true,
        VanillaInventoryAction::ConsumeHeld => {
            game_mode == GameMode::Creative || inventory.held_item().item_count.0 > 0
        }
        VanillaInventoryAction::Give(item_name, count) => {
            game_mode == GameMode::Creative
                || crate::inventory::item_id_for_name(item_name).is_some_and(|item_id| {
                    inventory.can_accept_item_stack(&crate::inventory::simple_item(
                        item_id,
                        count.max(0),
                    ))
                })
        }
        VanillaInventoryAction::ExchangeHeld(replacement_item) => {
            game_mode == GameMode::Creative || can_exchange_held_item(inventory, replacement_item)
        }
    }
}

fn apply_vanilla_inventory_action(
    inventory: &mut crate::inventory::PlayerInventory,
    game_mode: GameMode,
    action: VanillaInventoryAction,
) -> Vec<crate::inventory::InventorySlotChange> {
    if game_mode == GameMode::Creative {
        return Vec::new();
    }
    match action {
        VanillaInventoryAction::None => Vec::new(),
        VanillaInventoryAction::ConsumeHeld => inventory
            .decrement_hotbar_slot(inventory.selected_slot(), 1)
            .into_iter()
            .collect(),
        VanillaInventoryAction::Give(item_name, count) => {
            crate::inventory::item_id_for_name(item_name)
                .and_then(|item_id| {
                    inventory.add_item_stack(&crate::inventory::simple_item(item_id, count.max(0)))
                })
                .unwrap_or_default()
        }
        VanillaInventoryAction::ExchangeHeld(replacement_item) => {
            exchange_held_item(inventory, replacement_item).unwrap_or_default()
        }
    }
}

fn harvestable_block_interaction(
    block_state: i32,
    block_name: &str,
) -> Option<VanillaStateInteraction> {
    match block_name {
        "minecraft:sweet_berry_bush" => {
            let age = block_state_u8_property(block_state, "age")?;
            if age < 2 {
                return None;
            }
            Some(VanillaStateInteraction {
                target_block_state: block_state_with_property(block_state, "age", "1")?,
                inventory: VanillaInventoryAction::Give(
                    "minecraft:sweet_berries",
                    if age >= 3 { 2 } else { 1 },
                ),
            })
        }
        "minecraft:cave_vines" | "minecraft:cave_vines_plant"
            if block_state_bool_property(block_state, "berries") =>
        {
            Some(VanillaStateInteraction {
                target_block_state: block_state_with_property(block_state, "berries", "false")?,
                inventory: VanillaInventoryAction::Give("minecraft:glow_berries", 1),
            })
        }
        _ => None,
    }
}

fn candle_placement_interaction(
    block_state: i32,
    block_name: &str,
    held_item: Option<&str>,
) -> Option<VanillaStateInteraction> {
    let held_item = normalize_resource_key(held_item?).to_ascii_lowercase();
    if block_name == "minecraft:cake"
        && block_state_u8_property(block_state, "bites").unwrap_or(0) == 0
    {
        return Some(VanillaStateInteraction {
            target_block_state: crate::world::chunk_nbt::default_block_state_id(
                candle_cake_block_for_item(&held_item)?,
            ),
            inventory: VanillaInventoryAction::ConsumeHeld,
        });
    }

    if block_name == held_item && is_standalone_candle_block(block_name) {
        let candles = block_state_u8_property(block_state, "candles")?;
        if candles >= 4 {
            return None;
        }
        return Some(VanillaStateInteraction {
            target_block_state: block_state_with_property(
                block_state,
                "candles",
                &(candles + 1).to_string(),
            )?,
            inventory: VanillaInventoryAction::ConsumeHeld,
        });
    }

    None
}

fn candle_cake_block_for_item(item_name: &str) -> Option<&'static str> {
    match item_name {
        "minecraft:candle" => Some("minecraft:candle_cake"),
        "minecraft:white_candle" => Some("minecraft:white_candle_cake"),
        "minecraft:orange_candle" => Some("minecraft:orange_candle_cake"),
        "minecraft:magenta_candle" => Some("minecraft:magenta_candle_cake"),
        "minecraft:light_blue_candle" => Some("minecraft:light_blue_candle_cake"),
        "minecraft:yellow_candle" => Some("minecraft:yellow_candle_cake"),
        "minecraft:lime_candle" => Some("minecraft:lime_candle_cake"),
        "minecraft:pink_candle" => Some("minecraft:pink_candle_cake"),
        "minecraft:gray_candle" => Some("minecraft:gray_candle_cake"),
        "minecraft:light_gray_candle" => Some("minecraft:light_gray_candle_cake"),
        "minecraft:cyan_candle" => Some("minecraft:cyan_candle_cake"),
        "minecraft:purple_candle" => Some("minecraft:purple_candle_cake"),
        "minecraft:blue_candle" => Some("minecraft:blue_candle_cake"),
        "minecraft:brown_candle" => Some("minecraft:brown_candle_cake"),
        "minecraft:green_candle" => Some("minecraft:green_candle_cake"),
        "minecraft:red_candle" => Some("minecraft:red_candle_cake"),
        "minecraft:black_candle" => Some("minecraft:black_candle_cake"),
        _ => None,
    }
}

fn is_standalone_candle_block(block_name: &str) -> bool {
    (block_name == "minecraft:candle" || block_name.ends_with("_candle"))
        && !block_name.ends_with("_candle_cake")
}

fn beehive_interaction(
    block_state: i32,
    block_name: &str,
    held_item: Option<&str>,
) -> Option<VanillaStateInteraction> {
    if !matches!(block_name, "minecraft:bee_nest" | "minecraft:beehive")
        || block_state_u8_property(block_state, "honey_level")? < 5
    {
        return None;
    }
    let held_item = normalize_resource_key(held_item?).to_ascii_lowercase();
    match held_item.as_str() {
        "minecraft:shears" => Some(VanillaStateInteraction {
            target_block_state: block_state_with_property(block_state, "honey_level", "0")?,
            inventory: VanillaInventoryAction::Give("minecraft:honeycomb", 3),
        }),
        "minecraft:glass_bottle" => Some(VanillaStateInteraction {
            target_block_state: block_state_with_property(block_state, "honey_level", "0")?,
            inventory: VanillaInventoryAction::ExchangeHeld("minecraft:honey_bottle"),
        }),
        _ => None,
    }
}

fn pumpkin_shear_interaction(
    _block_state: i32,
    block_name: &str,
    held_item: Option<&str>,
    face: i32,
) -> Option<VanillaStateInteraction> {
    if block_name != "minecraft:pumpkin"
        || held_item.map(normalize_resource_key).as_deref() != Some("minecraft:shears")
    {
        return None;
    }
    let facing = horizontal_facing_for_clicked_face(face);
    Some(VanillaStateInteraction {
        target_block_state: crate::world::chunk_nbt::block_state(
            "minecraft:carved_pumpkin",
            &[("facing".to_string(), facing.to_string())],
        )
        .id,
        inventory: VanillaInventoryAction::Give("minecraft:pumpkin_seeds", 4),
    })
}

fn horizontal_facing_for_clicked_face(face: i32) -> &'static str {
    match face {
        2 => "north",
        3 => "south",
        4 => "west",
        5 => "east",
        _ => "north",
    }
}

fn note_block_interaction(block_state: i32, block_name: &str) -> Option<i32> {
    if block_name != "minecraft:note_block" {
        return None;
    }
    let note = block_state_u8_property(block_state, "note").unwrap_or(0);
    block_state_with_property(block_state, "note", &((note + 1) % 25).to_string())
}

fn bone_meal_interaction(
    block_state: i32,
    block_name: &str,
    held_item: Option<&str>,
) -> Option<VanillaStateInteraction> {
    if held_item.map(normalize_resource_key).as_deref() != Some("minecraft:bone_meal") {
        return None;
    }
    Some(VanillaStateInteraction {
        target_block_state: bone_meal_growth_state(block_state, block_name)?,
        inventory: VanillaInventoryAction::ConsumeHeld,
    })
}

fn bone_meal_growth_state(block_state: i32, block_name: &str) -> Option<i32> {
    if let Some(max_age) = bone_meal_age_max(block_name) {
        let age = block_state_u8_property(block_state, "age")?;
        if age >= max_age {
            return None;
        }
        let growth = bone_meal_growth_step(block_name);
        return block_state_with_property(
            block_state,
            "age",
            &(age + growth).min(max_age).to_string(),
        );
    }

    if is_berrying_vine_block(block_name)
        && block_state_bool_property_exists(block_state, "berries")
        && !block_state_bool_property(block_state, "berries")
    {
        return block_state_with_property(block_state, "berries", "true");
    }

    None
}

fn bone_meal_age_max(block_name: &str) -> Option<u8> {
    match block_name {
        "minecraft:wheat"
        | "minecraft:carrots"
        | "minecraft:potatoes"
        | "minecraft:melon_stem"
        | "minecraft:pumpkin_stem" => Some(7),
        "minecraft:beetroots" | "minecraft:sweet_berry_bush" => Some(3),
        "minecraft:cocoa" => Some(2),
        "minecraft:torchflower_crop" => Some(1),
        "minecraft:pitcher_crop" => Some(4),
        _ => None,
    }
}

fn bone_meal_growth_step(block_name: &str) -> u8 {
    match block_name {
        "minecraft:beetroots" | "minecraft:torchflower_crop" => 1,
        "minecraft:sweet_berry_bush" | "minecraft:cocoa" => 1,
        _ => 2,
    }
}

fn is_berrying_vine_block(block_name: &str) -> bool {
    matches!(
        block_name,
        "minecraft:cave_vines" | "minecraft:cave_vines_plant"
    )
}

fn is_grass_block(block_name: &str) -> bool {
    matches!(
        block_name,
        "minecraft:grass_block" | "minecraft:dirt" | "minecraft:podzol" | "minecraft:mycelium"
    )
}

/// Bone meal on grass/dirt: grow tall grass and flowers on top.
fn grass_bone_meal_growth(
    world: &WorldManager,
    dimension: &str,
    position: &BlockPosition,
) -> Option<Vec<(BlockPosition, i32)>> {
    let above = offset_position(position, 0, 1, 0);
    let above_state = world
        .block_state_at(dimension, &above)
        .unwrap_or_else(crate::inventory::air_block_state);
    // Only grow on air blocks — don't overwrite saplings or other plants
    if !crate::inventory::is_air_block_state(above_state) {
        return None;
    }
    // Pick a random plant: short_grass (60%), dandelion (15%), poppy (15%), oxeye_daisy (5%), cornflower (5%)
    let r = rand::Rng::gen_range(&mut rand::thread_rng(), 0u32..100);
    let plant = if r < 60 {
        "minecraft:short_grass"
    } else if r < 75 {
        "minecraft:dandelion"
    } else if r < 90 {
        "minecraft:poppy"
    } else if r < 95 {
        "minecraft:oxeye_daisy"
    } else {
        "minecraft:cornflower"
    };
    let plant_state = crate::world::chunk_nbt::default_block_state(plant).id;
    Some(vec![(above, plant_state)])
}

fn lit_block_interaction(
    block_state: i32,
    block_name: &str,
    held_item: Option<&str>,
) -> Option<VanillaStateInteraction> {
    if !block_state_bool_property_exists(block_state, "lit") {
        return None;
    }
    let lit = block_state_bool_property(block_state, "lit");
    let held_item = held_item.map(|item| normalize_resource_key(item).to_ascii_lowercase());
    let held_item = held_item.as_deref();

    if lit && held_item.is_none() && is_candle_block(block_name) {
        return Some(VanillaStateInteraction {
            target_block_state: block_state_with_property(block_state, "lit", "false")?,
            inventory: VanillaInventoryAction::None,
        });
    }

    if lit && is_campfire_block(block_name) && held_item.is_some_and(is_shovel_item) {
        return Some(VanillaStateInteraction {
            target_block_state: block_state_with_property(block_state, "lit", "false")?,
            inventory: VanillaInventoryAction::None,
        });
    }

    if !lit && is_lightable_block(block_name) && held_item.is_some_and(is_igniter_item) {
        return Some(VanillaStateInteraction {
            target_block_state: block_state_with_property(block_state, "lit", "true")?,
            inventory: if held_item == Some("minecraft:fire_charge") {
                VanillaInventoryAction::ConsumeHeld
            } else {
                VanillaInventoryAction::None
            },
        });
    }

    None
}

fn tool_block_interaction(
    world: &WorldManager,
    dimension: &str,
    position: &BlockPosition,
    block_state: i32,
    block_name: &str,
    held_item: &str,
    face: i32,
) -> Option<i32> {
    let held_item = normalize_resource_key(held_item).to_ascii_lowercase();
    if is_axe_item(&held_item) {
        return axe_block_transform(block_state, block_name);
    }
    if is_shovel_item(&held_item) {
        return shovel_block_transform(world, dimension, position, block_name, face);
    }
    if is_hoe_item(&held_item) {
        return hoe_block_transform(world, dimension, position, block_name, face);
    }
    None
}

fn axe_block_transform(block_state: i32, block_name: &str) -> Option<i32> {
    let block_name = normalize_resource_key(block_name).to_ascii_lowercase();
    if let Some(target_name) = stripped_wood_name(&block_name) {
        return block_state_with_name_and_overlapping_properties(block_state, target_name);
    }
    if let Some(unwaxed) = block_name.strip_prefix("minecraft:waxed_") {
        let target_name = format!("minecraft:{unwaxed}");
        return block_state_with_name_and_overlapping_properties(block_state, &target_name);
    }
    if let Some(weathered) = block_name.strip_prefix("minecraft:oxidized_") {
        let target_name = format!("minecraft:weathered_{weathered}");
        return block_state_with_name_and_overlapping_properties(block_state, &target_name);
    }
    if let Some(exposed) = block_name.strip_prefix("minecraft:weathered_") {
        let target_name = format!("minecraft:exposed_{exposed}");
        return block_state_with_name_and_overlapping_properties(block_state, &target_name);
    }
    if let Some(copper) = block_name.strip_prefix("minecraft:exposed_") {
        let target_name = format!("minecraft:{copper}");
        return block_state_with_name_and_overlapping_properties(block_state, &target_name);
    }
    None
}

fn shovel_block_transform(
    world: &WorldManager,
    dimension: &str,
    position: &BlockPosition,
    block_name: &str,
    face: i32,
) -> Option<i32> {
    if face == 0 || !block_above_allows_surface_transform(world, dimension, position) {
        return None;
    }
    matches!(
        block_name,
        "minecraft:grass_block"
            | "minecraft:dirt"
            | "minecraft:podzol"
            | "minecraft:mycelium"
            | "minecraft:coarse_dirt"
            | "minecraft:rooted_dirt"
    )
    .then(|| crate::world::chunk_nbt::default_block_state_id("minecraft:dirt_path"))
}

fn hoe_block_transform(
    world: &WorldManager,
    dimension: &str,
    position: &BlockPosition,
    block_name: &str,
    face: i32,
) -> Option<i32> {
    if face == 0 || !block_above_allows_surface_transform(world, dimension, position) {
        return None;
    }
    match block_name {
        "minecraft:grass_block" | "minecraft:dirt" | "minecraft:dirt_path" => Some(
            crate::world::chunk_nbt::default_block_state_id("minecraft:farmland"),
        ),
        "minecraft:coarse_dirt" | "minecraft:rooted_dirt" => Some(
            crate::world::chunk_nbt::default_block_state_id("minecraft:dirt"),
        ),
        _ => None,
    }
}

fn block_above_allows_surface_transform(
    world: &WorldManager,
    dimension: &str,
    position: &BlockPosition,
) -> bool {
    let above = offset_position(position, 0, 1, 0);
    let above_state = world
        .block_state_at(dimension, &above)
        .unwrap_or_else(crate::inventory::air_block_state);
    crate::inventory::can_replace_block_state(above_state)
}

fn stripped_wood_name(block_name: &str) -> Option<&'static str> {
    match block_name {
        "minecraft:oak_log" => Some("minecraft:stripped_oak_log"),
        "minecraft:oak_wood" => Some("minecraft:stripped_oak_wood"),
        "minecraft:spruce_log" => Some("minecraft:stripped_spruce_log"),
        "minecraft:spruce_wood" => Some("minecraft:stripped_spruce_wood"),
        "minecraft:birch_log" => Some("minecraft:stripped_birch_log"),
        "minecraft:birch_wood" => Some("minecraft:stripped_birch_wood"),
        "minecraft:jungle_log" => Some("minecraft:stripped_jungle_log"),
        "minecraft:jungle_wood" => Some("minecraft:stripped_jungle_wood"),
        "minecraft:acacia_log" => Some("minecraft:stripped_acacia_log"),
        "minecraft:acacia_wood" => Some("minecraft:stripped_acacia_wood"),
        "minecraft:dark_oak_log" => Some("minecraft:stripped_dark_oak_log"),
        "minecraft:dark_oak_wood" => Some("minecraft:stripped_dark_oak_wood"),
        "minecraft:mangrove_log" => Some("minecraft:stripped_mangrove_log"),
        "minecraft:mangrove_wood" => Some("minecraft:stripped_mangrove_wood"),
        "minecraft:cherry_log" => Some("minecraft:stripped_cherry_log"),
        "minecraft:cherry_wood" => Some("minecraft:stripped_cherry_wood"),
        "minecraft:pale_oak_log" => Some("minecraft:stripped_pale_oak_log"),
        "minecraft:pale_oak_wood" => Some("minecraft:stripped_pale_oak_wood"),
        "minecraft:crimson_stem" => Some("minecraft:stripped_crimson_stem"),
        "minecraft:crimson_hyphae" => Some("minecraft:stripped_crimson_hyphae"),
        "minecraft:warped_stem" => Some("minecraft:stripped_warped_stem"),
        "minecraft:warped_hyphae" => Some("minecraft:stripped_warped_hyphae"),
        "minecraft:bamboo_block" => Some("minecraft:stripped_bamboo_block"),
        _ => None,
    }
}

fn is_lightable_block(block_name: &str) -> bool {
    is_campfire_block(block_name) || is_candle_block(block_name)
}

fn is_campfire_block(block_name: &str) -> bool {
    matches!(block_name, "minecraft:campfire" | "minecraft:soul_campfire")
}

fn is_candle_block(block_name: &str) -> bool {
    block_name == "minecraft:candle"
        || block_name.ends_with("_candle")
        || block_name == "minecraft:candle_cake"
        || block_name.ends_with("_candle_cake")
}

fn is_igniter_item(item_name: &str) -> bool {
    matches!(
        item_name,
        "minecraft:flint_and_steel" | "minecraft:fire_charge"
    )
}

fn is_axe_item(item_name: &str) -> bool {
    item_name.ends_with("_axe")
}

fn is_shovel_item(item_name: &str) -> bool {
    item_name.ends_with("_shovel")
}

fn is_hoe_item(item_name: &str) -> bool {
    item_name.ends_with("_hoe")
}

fn cake_interaction(
    block_state: i32,
    block_name: &str,
    game_mode: GameMode,
    survival: &mut SurvivalState,
) -> Option<i32> {
    if block_name != "minecraft:cake" || game_mode != GameMode::Survival {
        return None;
    }
    if !survival.can_eat(false) || !survival.eat(2, 0.1) {
        return None;
    }
    let bites = block_state_u8_property(block_state, "bites").unwrap_or(0);
    if bites >= 6 {
        Some(crate::inventory::air_block_state())
    } else {
        block_state_with_property(block_state, "bites", &(bites + 1).to_string())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ComposterInteraction {
    target_block_state: Option<i32>,
    inventory: ComposterInventoryAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ComposterInventoryAction {
    ConsumeHeld,
    Give(&'static str),
}

fn composter_interaction(
    block_state: i32,
    block_name: &str,
    held_item: Option<&str>,
    position: &BlockPosition,
) -> Option<ComposterInteraction> {
    if block_name != "minecraft:composter" {
        return None;
    }
    let level = block_state_u8_property(block_state, "level")
        .unwrap_or(0)
        .min(8);
    if level >= 8 {
        return Some(ComposterInteraction {
            target_block_state: Some(composter_state(0)),
            inventory: ComposterInventoryAction::Give("minecraft:bone_meal"),
        });
    }

    let held_item = held_item?;
    let chance = compostable_chance(held_item)?;
    let accepted = composter_accepts_item(held_item, position, level, chance);
    Some(ComposterInteraction {
        target_block_state: accepted.then(|| composter_state(level.saturating_add(1).min(8))),
        inventory: ComposterInventoryAction::ConsumeHeld,
    })
}

fn composter_state(level: u8) -> i32 {
    crate::world::chunk_nbt::block_state(
        "minecraft:composter",
        &[("level".to_string(), level.min(8).to_string())],
    )
    .id
}

fn composter_accepts_item(
    item_name: &str,
    position: &BlockPosition,
    level: u8,
    chance_percent: u8,
) -> bool {
    if chance_percent >= 100 {
        return true;
    }
    let mut hash = 0xcbf29ce484222325u64;
    for byte in item_name.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    for value in [position.x, position.y, position.z, i32::from(level)] {
        hash ^= value as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash % 100 < u64::from(chance_percent)
}

fn compostable_chance(item_name: &str) -> Option<u8> {
    let item_name = normalize_resource_key(item_name).to_ascii_lowercase();
    match item_name.as_str() {
        "minecraft:cake" | "minecraft:pumpkin_pie" => Some(100),
        "minecraft:baked_potato"
        | "minecraft:bread"
        | "minecraft:cookie"
        | "minecraft:hay_block"
        | "minecraft:mushroom_stew"
        | "minecraft:beetroot_soup"
        | "minecraft:suspicious_stew"
        | "minecraft:nether_wart_block"
        | "minecraft:warped_wart_block"
        | "minecraft:flowering_azalea_leaves" => Some(85),
        "minecraft:apple"
        | "minecraft:beetroot"
        | "minecraft:carrot"
        | "minecraft:cocoa_beans"
        | "minecraft:fern"
        | "minecraft:lily_pad"
        | "minecraft:melon"
        | "minecraft:potato"
        | "minecraft:pumpkin"
        | "minecraft:sea_pickle"
        | "minecraft:wheat"
        | "minecraft:brown_mushroom"
        | "minecraft:red_mushroom"
        | "minecraft:crimson_fungus"
        | "minecraft:warped_fungus"
        | "minecraft:azalea"
        | "minecraft:flowering_azalea" => Some(65),
        name if name.ends_with("_sapling")
            || name.ends_with("_leaves")
            || name.ends_with("_flowers")
            || name.ends_with("_tulip")
            || matches!(
                name,
                "minecraft:dandelion"
                    | "minecraft:poppy"
                    | "minecraft:blue_orchid"
                    | "minecraft:allium"
                    | "minecraft:azure_bluet"
                    | "minecraft:oxeye_daisy"
                    | "minecraft:cornflower"
                    | "minecraft:lily_of_the_valley"
                    | "minecraft:wither_rose"
                    | "minecraft:sunflower"
                    | "minecraft:lilac"
                    | "minecraft:rose_bush"
                    | "minecraft:peony"
            ) =>
        {
            Some(65)
        }
        "minecraft:cactus"
        | "minecraft:dried_kelp_block"
        | "minecraft:melon_slice"
        | "minecraft:sugar_cane"
        | "minecraft:tall_grass"
        | "minecraft:vine"
        | "minecraft:weeping_vines"
        | "minecraft:twisting_vines"
        | "minecraft:nether_sprouts"
        | "minecraft:crimson_roots"
        | "minecraft:warped_roots"
        | "minecraft:moss_block"
        | "minecraft:big_dripleaf" => Some(50),
        "minecraft:beetroot_seeds"
        | "minecraft:dried_kelp"
        | "minecraft:grass"
        | "minecraft:kelp"
        | "minecraft:melon_seeds"
        | "minecraft:pumpkin_seeds"
        | "minecraft:seagrass"
        | "minecraft:sweet_berries"
        | "minecraft:glow_berries"
        | "minecraft:wheat_seeds"
        | "minecraft:moss_carpet"
        | "minecraft:small_dripleaf"
        | "minecraft:hanging_roots"
        | "minecraft:mangrove_roots"
        | "minecraft:pink_petals"
        | "minecraft:torchflower_seeds"
        | "minecraft:pitcher_pod" => Some(30),
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
async fn handle_redstone_interaction<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    world: &WorldManager,
    world_rules: &crate::world::WorldRulesManager,
    players: &PlayerManager,
    fluid: &FluidRuntime,
    gameplay: &qexed_config::app::qexed::server::Gameplay,
    runtime: &mut gameplay::redstone::RedstoneRuntime,
    play_dimension: &str,
    actor: uuid::Uuid,
    position: BlockPosition,
    block_name: &str,
) -> Result<bool>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    if !gameplay.redstone || !gameplay.block_updates {
        return Ok(false);
    }
    let Some(block_state) = world.block_state_at(play_dimension, &position) else {
        return Ok(false);
    };
    let Some(interaction) =
        gameplay::redstone::interaction_for_block_state(block_state, block_name)
    else {
        return Ok(false);
    };
    let changes = vec![gameplay::redstone::RedstoneBlockChange {
        dimension: play_dimension.to_string(),
        position: position.clone(),
        block_state: interaction.block_state,
    }];
    let applied = apply_redstone_updates(
        sink,
        world,
        world_rules,
        players,
        fluid,
        runtime,
        play_dimension,
        actor,
        changes,
    )
    .await?;
    if applied.is_empty() {
        return Ok(true);
    }
    if let Some(delay_ms) = interaction.button_release_ms {
        runtime.schedule_button_release(play_dimension.to_string(), position, delay_ms);
    }
    propagate_redstone_from_positions(
        sink,
        world,
        world_rules,
        players,
        fluid,
        gameplay,
        runtime,
        play_dimension,
        actor,
        applied,
    )
    .await?;
    Ok(true)
}

#[allow(clippy::too_many_arguments)]
async fn apply_and_propagate_redstone_updates<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    world: &WorldManager,
    world_rules: &crate::world::WorldRulesManager,
    players: &PlayerManager,
    fluid: &FluidRuntime,
    gameplay: &qexed_config::app::qexed::server::Gameplay,
    runtime: &mut gameplay::redstone::RedstoneRuntime,
    play_dimension: &str,
    actor: uuid::Uuid,
    updates: Vec<gameplay::redstone::RedstoneBlockChange>,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    if !gameplay.redstone || !gameplay.block_updates {
        return Ok(());
    }
    let applied = apply_redstone_updates(
        sink,
        world,
        world_rules,
        players,
        fluid,
        runtime,
        play_dimension,
        actor,
        updates,
    )
    .await?;
    let observer_applied = apply_observer_updates_after_block_changes(
        sink,
        world,
        world_rules,
        players,
        fluid,
        runtime,
        play_dimension,
        actor,
        &applied,
    )
    .await?;
    let mut origins = applied;
    origins.extend(observer_applied);
    propagate_redstone_from_positions(
        sink,
        world,
        world_rules,
        players,
        fluid,
        gameplay,
        runtime,
        play_dimension,
        actor,
        origins,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn propagate_redstone_from_positions<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    world: &WorldManager,
    world_rules: &crate::world::WorldRulesManager,
    players: &PlayerManager,
    fluid: &FluidRuntime,
    gameplay: &qexed_config::app::qexed::server::Gameplay,
    runtime: &mut gameplay::redstone::RedstoneRuntime,
    play_dimension: &str,
    actor: uuid::Uuid,
    origins: Vec<gameplay::redstone::RedstoneBlockChange>,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    if !gameplay.redstone || !gameplay.block_updates || origins.is_empty() {
        return Ok(());
    }
    let mut by_dimension = std::collections::HashMap::<String, Vec<BlockPosition>>::new();
    for origin in origins {
        by_dimension
            .entry(origin.dimension)
            .or_default()
            .push(origin.position);
    }
    for (dimension, positions) in by_dimension {
        let observer_applied = apply_observer_updates_after_block_positions(
            sink,
            world,
            world_rules,
            players,
            fluid,
            runtime,
            play_dimension,
            actor,
            &dimension,
            &positions,
        )
        .await?;
        let mut positions = positions;
        positions.extend(
            observer_applied
                .iter()
                .map(|changed| changed.position.clone()),
        );
        let updates = gameplay::redstone::updates_after_block_changes(
            world,
            &dimension,
            &positions,
            gameplay.redstone_max_distance,
        );
        let applied_updates = apply_redstone_updates(
            sink,
            world,
            world_rules,
            players,
            fluid,
            runtime,
            play_dimension,
            actor,
            updates,
        )
        .await?;
        let network_observer_applied = apply_observer_updates_after_block_changes(
            sink,
            world,
            world_rules,
            players,
            fluid,
            runtime,
            play_dimension,
            actor,
            &applied_updates,
        )
        .await?;
        if !network_observer_applied.is_empty() {
            let observer_positions = network_observer_applied
                .iter()
                .map(|changed| changed.position.clone())
                .collect::<Vec<_>>();
            let updates = gameplay::redstone::updates_after_block_changes(
                world,
                &dimension,
                &observer_positions,
                gameplay.redstone_max_distance,
            );
            apply_redstone_updates(
                sink,
                world,
                world_rules,
                players,
                fluid,
                runtime,
                play_dimension,
                actor,
                updates,
            )
            .await?;
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn propagate_redstone_from_block_positions<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    world: &WorldManager,
    world_rules: &crate::world::WorldRulesManager,
    players: &PlayerManager,
    fluid: &FluidRuntime,
    gameplay: &qexed_config::app::qexed::server::Gameplay,
    runtime: &mut gameplay::redstone::RedstoneRuntime,
    play_dimension: &str,
    actor: uuid::Uuid,
    positions: Vec<BlockPosition>,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    if positions.is_empty() {
        return Ok(());
    }
    let origins = positions
        .into_iter()
        .map(|position| gameplay::redstone::RedstoneBlockChange {
            dimension: play_dimension.to_string(),
            position,
            block_state: crate::inventory::air_block_state(),
        })
        .collect();
    propagate_redstone_from_positions(
        sink,
        world,
        world_rules,
        players,
        fluid,
        gameplay,
        runtime,
        play_dimension,
        actor,
        origins,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn apply_observer_updates_after_block_changes<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    world: &WorldManager,
    world_rules: &crate::world::WorldRulesManager,
    players: &PlayerManager,
    fluid: &FluidRuntime,
    runtime: &mut gameplay::redstone::RedstoneRuntime,
    play_dimension: &str,
    actor: uuid::Uuid,
    changes: &[gameplay::redstone::RedstoneBlockChange],
) -> Result<Vec<gameplay::redstone::RedstoneBlockChange>>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let mut by_dimension = std::collections::HashMap::<String, Vec<BlockPosition>>::new();
    for change in changes {
        by_dimension
            .entry(change.dimension.clone())
            .or_default()
            .push(change.position.clone());
    }

    let mut applied = Vec::new();
    for (dimension, positions) in by_dimension {
        applied.extend(
            apply_observer_updates_after_block_positions(
                sink,
                world,
                world_rules,
                players,
                fluid,
                runtime,
                play_dimension,
                actor,
                &dimension,
                &positions,
            )
            .await?,
        );
    }
    Ok(applied)
}

#[allow(clippy::too_many_arguments)]
async fn apply_observer_updates_after_block_positions<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    world: &WorldManager,
    world_rules: &crate::world::WorldRulesManager,
    players: &PlayerManager,
    fluid: &FluidRuntime,
    runtime: &mut gameplay::redstone::RedstoneRuntime,
    play_dimension: &str,
    actor: uuid::Uuid,
    dimension: &str,
    positions: &[BlockPosition],
) -> Result<Vec<gameplay::redstone::RedstoneBlockChange>>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let updates = runtime.observer_updates_after_block_changes(world, dimension, positions);
    apply_redstone_updates(
        sink,
        world,
        world_rules,
        players,
        fluid,
        runtime,
        play_dimension,
        actor,
        updates,
    )
    .await
}

fn piston_side_effect_updates(
    world: &WorldManager,
    dimension: &str,
    position: &BlockPosition,
    current_state: i32,
    next_state: i32,
) -> Option<Vec<gameplay::redstone::RedstoneBlockChange>> {
    let current_name = block_name(current_state);
    let next_name = block_name(next_state);
    if !is_piston_base_block(&current_name) || !is_piston_base_block(&next_name) {
        return Some(Vec::new());
    }

    let was_extended = block_state_bool_property(current_state, "extended");
    let will_extend = block_state_bool_property(next_state, "extended");
    if was_extended == will_extend {
        return Some(Vec::new());
    }

    let (dx, dy, dz, facing) = piston_facing_offset(current_state)?;
    let sticky = current_name == "minecraft:sticky_piston";
    if will_extend {
        piston_extension_updates(world, dimension, position, dx, dy, dz, &facing, sticky)
    } else {
        Some(piston_retraction_updates(
            world, dimension, position, dx, dy, dz, sticky,
        ))
    }
}

#[allow(clippy::too_many_arguments)]
fn piston_extension_updates(
    world: &WorldManager,
    dimension: &str,
    position: &BlockPosition,
    dx: i32,
    dy: i32,
    dz: i32,
    facing: &str,
    sticky: bool,
) -> Option<Vec<gameplay::redstone::RedstoneBlockChange>> {
    let front = offset_position(position, dx, dy, dz);
    let head_state = piston_head_state(facing, sticky);
    let mut updates = Vec::new();
    let mut movable = Vec::<(BlockPosition, i32)>::new();
    let mut cursor = front.clone();

    loop {
        let state = world
            .block_state_at(dimension, &cursor)
            .unwrap_or_else(crate::inventory::air_block_state);
        if crate::inventory::is_air_block_state(state)
            || crate::inventory::can_replace_block_state(state)
        {
            break;
        }
        if movable.len() >= 12 || !piston_can_move_block(state) {
            return None;
        }
        movable.push((cursor.clone(), state));
        cursor = offset_position(&cursor, dx, dy, dz);
    }

    for (source, state) in movable.iter().rev() {
        updates.push(gameplay::redstone::RedstoneBlockChange {
            dimension: dimension.to_string(),
            position: offset_position(source, dx, dy, dz),
            block_state: *state,
        });
    }
    updates.push(gameplay::redstone::RedstoneBlockChange {
        dimension: dimension.to_string(),
        position: front,
        block_state: head_state,
    });
    Some(updates)
}

fn piston_retraction_updates(
    world: &WorldManager,
    dimension: &str,
    position: &BlockPosition,
    dx: i32,
    dy: i32,
    dz: i32,
    sticky: bool,
) -> Vec<gameplay::redstone::RedstoneBlockChange> {
    let front = offset_position(position, dx, dy, dz);
    let mut updates = Vec::new();
    let front_state = world
        .block_state_at(dimension, &front)
        .unwrap_or_else(crate::inventory::air_block_state);
    if block_name(front_state) == "minecraft:piston_head" {
        updates.push(gameplay::redstone::RedstoneBlockChange {
            dimension: dimension.to_string(),
            position: front.clone(),
            block_state: crate::inventory::air_block_state(),
        });
    }

    if sticky {
        let pull = offset_position(&front, dx, dy, dz);
        let pull_state = world
            .block_state_at(dimension, &pull)
            .unwrap_or_else(crate::inventory::air_block_state);
        if piston_can_move_block(pull_state) {
            updates.push(gameplay::redstone::RedstoneBlockChange {
                dimension: dimension.to_string(),
                position: front,
                block_state: pull_state,
            });
            updates.push(gameplay::redstone::RedstoneBlockChange {
                dimension: dimension.to_string(),
                position: pull,
                block_state: crate::inventory::air_block_state(),
            });
        }
    }

    updates
}

fn piston_head_state(facing: &str, sticky: bool) -> i32 {
    crate::world::chunk_nbt::block_state(
        "minecraft:piston_head",
        &[
            ("facing".to_string(), facing.to_string()),
            ("short".to_string(), "false".to_string()),
            (
                "type".to_string(),
                if sticky { "sticky" } else { "normal" }.to_string(),
            ),
        ],
    )
    .id
}

fn piston_can_move_block(block_state: i32) -> bool {
    if crate::inventory::is_air_block_state(block_state)
        || crate::inventory::can_replace_block_state(block_state)
    {
        return false;
    }
    !matches!(
        block_name(block_state).as_str(),
        "minecraft:bedrock"
            | "minecraft:obsidian"
            | "minecraft:crying_obsidian"
            | "minecraft:reinforced_deepslate"
            | "minecraft:end_portal_frame"
            | "minecraft:piston"
            | "minecraft:sticky_piston"
            | "minecraft:piston_head"
            | "minecraft:moving_piston"
    )
}

fn piston_facing_offset(block_state: i32) -> Option<(i32, i32, i32, String)> {
    let facing = block_state_property(block_state, "facing")?;
    let (dx, dy, dz) = match facing.as_str() {
        "east" => (1, 0, 0),
        "west" => (-1, 0, 0),
        "up" => (0, 1, 0),
        "down" => (0, -1, 0),
        "south" => (0, 0, 1),
        "north" => (0, 0, -1),
        _ => return None,
    };
    Some((dx, dy, dz, facing))
}

fn is_piston_base_block(block_name: &str) -> bool {
    matches!(block_name, "minecraft:piston" | "minecraft:sticky_piston")
}

fn block_state_bool_property(block_state: i32, key: &str) -> bool {
    block_state_property(block_state, key).is_some_and(|value| value == "true")
}

async fn apply_redstone_updates<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    world: &WorldManager,
    world_rules: &crate::world::WorldRulesManager,
    players: &PlayerManager,
    fluid: &FluidRuntime,
    runtime: &mut gameplay::redstone::RedstoneRuntime,
    play_dimension: &str,
    actor: uuid::Uuid,
    updates: Vec<gameplay::redstone::RedstoneBlockChange>,
) -> Result<Vec<gameplay::redstone::RedstoneBlockChange>>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let mut applied = Vec::new();
    let mut expanded = Vec::new();
    for update in updates {
        let current = world
            .block_state_at(&update.dimension, &update.position)
            .unwrap_or_else(crate::inventory::air_block_state);
        if let Some(piston_updates) = piston_side_effect_updates(
            world,
            &update.dimension,
            &update.position,
            current,
            update.block_state,
        ) {
            expanded.extend(piston_updates);
        } else {
            continue;
        }
        expanded.push(update);
    }

    for update in expanded {
        if !world_rules.snapshot(&update.dimension).block_updates {
            continue;
        }
        let current = world
            .block_state_at(&update.dimension, &update.position)
            .unwrap_or_else(crate::inventory::air_block_state);
        if current == update.block_state {
            continue;
        }
        if runtime.should_delay_redstone_update(current, &update) {
            continue;
        }
        if block_name(current) == "minecraft:tnt"
            && crate::inventory::is_air_block_state(update.block_state)
        {
            runtime.schedule_tnt_explosion(
                update.dimension.clone(),
                update.position.clone(),
                4_000,
            );
        }
        world.place_block(
            &update.dimension,
            update.position.clone(),
            update.block_state,
        );
        fluid.enqueue_block_change(&update.dimension, &update.position);
        if update.dimension == play_dimension {
            sink.send(crate::inventory::block_update(
                update.position.clone(),
                update.block_state,
            ))
            .await?;
        }
        let light_update = if world.dynamic_light_enabled()
            && matches!(
                world_rules.snapshot(&update.dimension).light,
                qexed_config::app::qexed::server::LightMode::Dynamic
            ) {
            let light = world.light_update(
                &update.dimension,
                update.position.x.div_euclid(16),
                update.position.z.div_euclid(16),
            );
            if update.dimension == play_dimension {
                sink.send(light.clone()).await?;
            }
            Some(qexed_tcp_connect::PacketSink::<tokio::io::Sink>::build_send_packet(light)?)
        } else {
            None
        };
        players.broadcast_block_changed(
            actor,
            &update.dimension,
            update.position.clone(),
            update.block_state,
            light_update,
        );
        applied.push(update);
    }
    Ok(applied)
}

#[allow(clippy::too_many_arguments)]
async fn handle_cauldron_interaction<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    world: &WorldManager,
    world_rules: &crate::world::WorldRulesManager,
    players: &PlayerManager,
    fluid: &FluidRuntime,
    world_config: &qexed_config::app::qexed::server::World,
    game_mode: GameMode,
    dimension: &str,
    actor: uuid::Uuid,
    entity_id: i32,
    gameplay: &qexed_config::app::qexed::server::Gameplay,
    inventory: &mut crate::inventory::PlayerInventory,
    position: BlockPosition,
    block_name: &str,
) -> Result<bool>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    if !gameplay.cauldron
        || !configured_gameplay_block_matches(block_name, &gameplay.cauldron_blocks)
    {
        return Ok(false);
    }

    let Some(block_state) = world.block_state_at(dimension, &position) else {
        return Ok(false);
    };
    let Some(held_item) = held_item_name(inventory) else {
        return Ok(false);
    };
    let Some(interaction) = cauldron_interaction(block_state, block_name, &held_item) else {
        return Ok(false);
    };
    if game_mode != GameMode::Creative
        && !can_exchange_held_item(inventory, interaction.replacement_item)
    {
        return Ok(true);
    }

    let changed = apply_block_change(
        sink,
        world,
        world_rules,
        players,
        fluid,
        world_config,
        game_mode,
        dimension,
        actor,
        position,
        interaction.target_block_state,
        WorldEditKind::Place,
        gameplay.block_updates,
    )
    .await?;
    if !changed {
        return Ok(true);
    }
    if game_mode == GameMode::Creative {
        return Ok(true);
    }

    let Some(changes) = exchange_held_item(inventory, interaction.replacement_item) else {
        return Ok(true);
    };
    sync_inventory_changes(
        sink,
        players,
        actor,
        entity_id,
        inventory.selected_slot(),
        changes,
    )
    .await?;
    Ok(true)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CauldronInteraction {
    target_block_state: i32,
    replacement_item: &'static str,
}

fn cauldron_interaction(
    block_state: i32,
    block_name: &str,
    held_item: &str,
) -> Option<CauldronInteraction> {
    let block_name = normalize_resource_key(block_name).to_ascii_lowercase();
    let held_item = normalize_resource_key(held_item).to_ascii_lowercase();
    match (block_name.as_str(), held_item.as_str()) {
        ("minecraft:cauldron", "minecraft:lava_bucket") => Some(CauldronInteraction {
            target_block_state: crate::world::chunk_nbt::default_block_state_id(
                "minecraft:lava_cauldron",
            ),
            replacement_item: "minecraft:bucket",
        }),
        ("minecraft:cauldron", "minecraft:powder_snow_bucket") => Some(CauldronInteraction {
            target_block_state: leveled_cauldron_state("minecraft:powder_snow_cauldron", 3),
            replacement_item: "minecraft:bucket",
        }),
        ("minecraft:lava_cauldron", "minecraft:bucket") => Some(CauldronInteraction {
            target_block_state: crate::world::chunk_nbt::default_block_state_id(
                "minecraft:cauldron",
            ),
            replacement_item: "minecraft:lava_bucket",
        }),
        ("minecraft:powder_snow_cauldron", "minecraft:bucket")
            if cauldron_level(block_state) >= 3 =>
        {
            Some(CauldronInteraction {
                target_block_state: crate::world::chunk_nbt::default_block_state_id(
                    "minecraft:cauldron",
                ),
                replacement_item: "minecraft:powder_snow_bucket",
            })
        }
        _ => None,
    }
}

fn leveled_cauldron_state(block_name: &str, level: u8) -> i32 {
    crate::world::chunk_nbt::block_state(
        block_name,
        &[("level".to_string(), level.clamp(1, 3).to_string())],
    )
    .id
}

fn cauldron_level(block_state: i32) -> u8 {
    crate::inventory::block_properties_for_state(block_state)
        .and_then(|properties| properties.get("level").and_then(|value| value.parse().ok()))
        .unwrap_or(0)
}

fn configured_gameplay_block_matches(block_name: &str, configured_blocks: &[String]) -> bool {
    let block_name = normalize_resource_key(block_name).to_ascii_lowercase();
    configured_blocks
        .iter()
        .any(|configured| normalize_resource_key(configured).to_ascii_lowercase() == block_name)
}

fn held_item_name(inventory: &crate::inventory::PlayerInventory) -> Option<String> {
    inventory
        .held_item()
        .item_id
        .as_ref()
        .and_then(|id| crate::inventory::item_name_for_id(id.0))
}

fn can_exchange_held_item(
    inventory: &crate::inventory::PlayerInventory,
    replacement_item: &str,
) -> bool {
    let Some(item_id) = crate::inventory::item_id_for_name(replacement_item) else {
        return false;
    };
    if inventory.held_item().item_count.0 <= 0 {
        return false;
    }
    inventory.held_item().item_count.0 == 1
        || inventory.can_accept_item_stack(&crate::inventory::simple_item(item_id, 1))
}

fn exchange_held_item(
    inventory: &mut crate::inventory::PlayerInventory,
    replacement_item: &str,
) -> Option<Vec<crate::inventory::InventorySlotChange>> {
    let item_id = crate::inventory::item_id_for_name(replacement_item)?;
    let replacement = crate::inventory::simple_item(item_id, 1);
    let selected_slot = inventory.selected_slot();
    if inventory.held_item().item_count.0 == 1 {
        return inventory
            .set_hotbar_slot(selected_slot, replacement)
            .map(|change| vec![change]);
    }

    let mut changes = Vec::new();
    if let Some(change) = inventory.decrement_hotbar_slot(selected_slot, 1) {
        changes.push(change);
    }
    changes.extend(inventory.add_item_stack(&replacement)?);
    Some(changes)
}

async fn place_held_block<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    world: &WorldManager,
    world_rules: &crate::world::WorldRulesManager,
    players: &PlayerManager,
    fluid: &FluidRuntime,
    world_config: &qexed_config::app::qexed::server::World,
    game_mode: GameMode,
    dimension: &str,
    actor: uuid::Uuid,
    player_position: &EntityPosition,
    use_item_on: &UseItemOn,
    block_state: i32,
    gameplay_block_updates: bool,
) -> Result<Vec<PlacedBlockChange>>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let block_state = crate::inventory::block_state_for_placement(
        block_state,
        crate::inventory::PlacementContext {
            face: use_item_on.block_hit.face.0,
            cursor_y: use_item_on.block_hit.cursor_y,
            player_yaw: player_position.yaw,
        },
    );
    let clicked = &use_item_on.block_hit.position;
    let clicked_state = world
        .block_state_at(dimension, clicked)
        .unwrap_or_else(crate::inventory::air_block_state);
    let target = if crate::inventory::can_replace_block_state(clicked_state) {
        clicked.clone()
    } else {
        crate::inventory::placement_position(clicked, use_item_on.block_hit.face.0)
    };
    let target_state = world
        .block_state_at(dimension, &target)
        .unwrap_or_else(crate::inventory::air_block_state);

    if !crate::inventory::can_replace_block_state(target_state)
        || (crate::inventory::block_has_collision(block_state)
            && player_intersects_block(player_position, &target))
    {
        send_block_rollback(sink, world, dimension, target).await?;
        return Ok(Vec::new());
    }

    if let Some(upper_state) = crate::inventory::upper_half_block_state(block_state) {
        let upper = offset_position(&target, 0, 1, 0);
        let upper_state_at_target = world
            .block_state_at(dimension, &upper)
            .unwrap_or_else(crate::inventory::air_block_state);
        if !crate::inventory::can_replace_block_state(upper_state_at_target)
            || (crate::inventory::block_has_collision(upper_state)
                && player_intersects_block(player_position, &upper))
        {
            send_block_rollback(sink, world, dimension, target).await?;
            send_block_rollback(sink, world, dimension, upper).await?;
            return Ok(Vec::new());
        }

        let placed_lower = apply_block_change(
            sink,
            world,
            world_rules,
            players,
            fluid,
            world_config,
            game_mode,
            dimension,
            actor,
            target.clone(),
            block_state,
            WorldEditKind::Place,
            gameplay_block_updates,
        )
        .await?;
        if !placed_lower {
            return Ok(Vec::new());
        }
        let placed_upper = apply_block_change(
            sink,
            world,
            world_rules,
            players,
            fluid,
            world_config,
            game_mode,
            dimension,
            actor,
            upper.clone(),
            upper_state,
            WorldEditKind::Place,
            gameplay_block_updates,
        )
        .await?;
        let mut placed = vec![PlacedBlockChange {
            position: target,
            block_state,
        }];
        if placed_upper {
            placed.push(PlacedBlockChange {
                position: upper,
                block_state: upper_state,
            });
        }
        return Ok(placed);
    }

    let placed = apply_block_change(
        sink,
        world,
        world_rules,
        players,
        fluid,
        world_config,
        game_mode,
        dimension,
        actor,
        target.clone(),
        block_state,
        WorldEditKind::Place,
        gameplay_block_updates,
    )
    .await?;
    if !placed {
        return Ok(Vec::new());
    }
    Ok(vec![PlacedBlockChange {
        position: target,
        block_state,
    }])
}

async fn destroy_block<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    world: &WorldManager,
    world_rules: &crate::world::WorldRulesManager,
    ore_pits: &crate::world::OrePitManager,
    players: &PlayerManager,
    fluid: &FluidRuntime,
    entities: &crate::entities::EntityManager,
    plugins: &crate::plugins::PluginManager,
    world_config: &qexed_config::app::qexed::server::World,
    game_mode: GameMode,
    dimension: &str,
    actor: uuid::Uuid,
    held_item: &qexed_protocol::types::Slot,
    position: BlockPosition,
    rendering: &qexed_config::app::qexed::server::EntityRendering,
    gameplay_block_updates: bool,
) -> Result<Option<DestroyedBlockChange>>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let current = world
        .block_state_at(dimension, &position)
        .unwrap_or_else(crate::inventory::air_block_state);
    if crate::inventory::is_air_block_state(current) {
        log_block_break_rejected("air_block", dimension, &position, Some(current));
        send_block_rollback(sink, world, dimension, position).await?;
        return Ok(None);
    }
    if !ore_pits.permits_player_break(dimension, &position, current) {
        log_block_break_rejected("ore_pit_restriction", dimension, &position, Some(current));
        send_block_rollback(sink, world, dimension, position).await?;
        return Ok(None);
    }

    let paired_position = if crate::inventory::upper_half_block_state(current).is_some() {
        Some(offset_position(&position, 0, 1, 0))
    } else if crate::inventory::lower_half_block_state(current).is_some() {
        Some(offset_position(&position, 0, -1, 0))
    } else {
        None
    };

    let destroyed = apply_block_change(
        sink,
        world,
        world_rules,
        players,
        fluid,
        world_config,
        game_mode,
        dimension,
        actor,
        position.clone(),
        crate::inventory::air_block_state(),
        WorldEditKind::Break,
        gameplay_block_updates,
    )
    .await?;
    if destroyed {
        if let Some(paired_position) = paired_position {
            apply_block_change(
                sink,
                world,
                world_rules,
                players,
                fluid,
                world_config,
                game_mode,
                dimension,
                actor,
                paired_position,
                crate::inventory::air_block_state(),
                WorldEditKind::Break,
                gameplay_block_updates,
            )
            .await?;
        }
        drop_broken_block(
            sink,
            world,
            world_rules,
            ore_pits,
            players,
            fluid,
            entities,
            plugins,
            world_config,
            game_mode,
            actor,
            dimension,
            current,
            held_item,
            &position,
            gameplay_block_updates,
            rendering,
        )
        .await?;
        return Ok(Some(DestroyedBlockChange {
            position,
            previous_state: current,
        }));
    }
    Ok(None)
}

async fn drop_broken_block<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    world: &WorldManager,
    world_rules: &crate::world::WorldRulesManager,
    ore_pits: &crate::world::OrePitManager,
    players: &PlayerManager,
    fluid: &FluidRuntime,
    entities: &crate::entities::EntityManager,
    plugins: &crate::plugins::PluginManager,
    world_config: &qexed_config::app::qexed::server::World,
    game_mode: GameMode,
    actor: uuid::Uuid,
    dimension: &str,
    block_state: i32,
    held_item: &qexed_protocol::types::Slot,
    position: &BlockPosition,
    gameplay_block_updates: bool,
    rendering: &qexed_config::app::qexed::server::EntityRendering,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    if game_mode != GameMode::Survival {
        return Ok(());
    }

    let player = players.player_by_uuid(actor);
    let player_payload = player.as_ref().map(qexed_plugin_api::player_payload_owned);
    let player_position = player
        .as_ref()
        .map(|player| qexed_plugin_api::player_position_payload(player.position));
    let outcome = mining::block_drop_outcome(
        block_state,
        position,
        held_item,
        plugins,
        player_payload.clone(),
        player_position,
    );
    drop_items_for_broken_block(
        sink,
        world,
        players,
        entities,
        actor,
        dimension,
        position,
        outcome.drops,
        rendering,
    )
    .await?;

    const MAX_PLUGIN_EXTRA_BREAKS: usize = 512;
    for extra_position in outcome
        .break_positions
        .into_iter()
        .filter(|extra| extra != position)
        .take(MAX_PLUGIN_EXTRA_BREAKS)
    {
        let current = world
            .block_state_at(dimension, &extra_position)
            .unwrap_or_else(crate::inventory::air_block_state);
        if crate::inventory::is_air_block_state(current)
            || !ore_pits.permits_player_break(dimension, &extra_position, current)
        {
            continue;
        }
        let destroyed = apply_block_change(
            sink,
            world,
            world_rules,
            players,
            fluid,
            world_config,
            game_mode,
            dimension,
            actor,
            extra_position.clone(),
            crate::inventory::air_block_state(),
            WorldEditKind::Break,
            gameplay_block_updates,
        )
        .await?;
        if !destroyed {
            continue;
        }
        let extra_outcome = mining::block_drop_outcome(
            current,
            &extra_position,
            held_item,
            plugins,
            player_payload.clone(),
            player
                .as_ref()
                .map(|player| qexed_plugin_api::player_position_payload(player.position)),
        );
        drop_items_for_broken_block(
            sink,
            world,
            players,
            entities,
            actor,
            dimension,
            &extra_position,
            extra_outcome.drops,
            rendering,
        )
        .await?;
    }
    Ok(())
}

async fn drop_items_for_broken_block<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    world: &WorldManager,
    players: &PlayerManager,
    entities: &crate::entities::EntityManager,
    actor: uuid::Uuid,
    dimension: &str,
    position: &BlockPosition,
    drops: Vec<qexed_protocol::types::Slot>,
    rendering: &qexed_config::app::qexed::server::EntityRendering,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let drop_position =
        settle_drop_position_on_ground(world, dimension, mining::drop_position(position));
    for item in drops {
        for update in entities.drop_item_with_rendering(
            players,
            actor,
            dimension,
            drop_position,
            item,
            rendering,
        )? {
            send_dropped_item_update(sink, update).await?;
        }
    }
    Ok(())
}

async fn collect_nearby_drops<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    world: &WorldManager,
    world_rules: &crate::world::WorldRulesManager,
    server_config: &qexed_config::app::qexed::server::Server,
    world_config: &qexed_config::app::qexed::server::World,
    lobby: &lobby::LobbyRuntime,
    lobby_status: &lobby::LobbyStatusSnapshot,
    players: &PlayerManager,
    plugins: &crate::plugins::PluginManager,
    cluster_entities: Option<&crate::cluster_entities::ClusterEntityController>,
    entities: &crate::entities::EntityManager,
    actor: uuid::Uuid,
    collector_entity_id: i32,
    chunk_sender: &tokio::sync::mpsc::UnboundedSender<chunks::ChunkLoadResult>,
    chunk_state: &mut ChunkSendState,
    position: &mut EntityPosition,
    next_teleport_id: &mut i32,
    play_dimension: &mut String,
    menus: &menus::MenuRuntime,
    active_config_menu: &mut Option<String>,
    players_hidden: &mut bool,
    visible_player_entities: &mut HashSet<uuid::Uuid>,
    inventory: &mut crate::inventory::PlayerInventory,
    render_distance: f64,
    geyser_runtime: &mut geyser::GeyserRuntime,
    simulation_distance: i32,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let collection_dimension = play_dimension.clone();
    entities.settle_collectable_dropped_items(&collection_dimension, *position, |item_position| {
        settle_drop_position_on_ground(world, &collection_dimension, item_position)
    });
    let mut remote_slots = Vec::new();
    if let Some(cluster_entities) = cluster_entities
        && let Some(mut collector) = players.player_by_uuid(actor)
    {
        collector.position = *position;
        collector.dimension = play_dimension.clone();
        remote_slots = cluster_entities.collect_items(players, &collector, simulation_distance)?;
    }
    let items = drops::collect_dropped_items(entities, &collection_dimension, *position)?;
    if items.is_empty() && remote_slots.is_empty() {
        return Ok(());
    }

    let mut picked = Vec::new();
    let mut changes = Vec::new();
    let mut refresh_sidebar = false;
    let mut player = match players.player_by_uuid(actor) {
        Some(player) => player,
        None => return Ok(()),
    };
    player.position = *position;
    player.dimension = play_dimension.clone();

    for item in items {
        let response = plugins.handle_player_item_pickup(&player, &item);
        let mut cancelled = response.cancel;
        let consumed = response.consume;
        refresh_sidebar |= response.cancel || response.consume || !response.actions.is_empty();
        for action in response.actions {
            let viewer_position = *position;
            let handled = chat::apply_plugin_action(
                sink,
                Some(server_config),
                world,
                world_rules,
                world_config,
                entities,
                players,
                plugins,
                actor,
                chunk_sender,
                chunk_state,
                position,
                next_teleport_id,
                play_dimension,
                menus,
                active_config_menu,
                players_hidden,
                visible_player_entities,
                viewer_position,
                render_distance,
                Some(&mut *inventory),
                Some(geyser_runtime),
                action,
            )
            .await?;
            cancelled |= handled;
        }
        if consumed && !cancelled {
            picked.push((item.clone(), item.item.item_count.0.max(1)));
            continue;
        }
        if cancelled {
            entities.restore_dropped_item(item);
            continue;
        }
        let original_count = item.item.item_count.0;
        let (mut item_changes, picked_count) = inventory.add_item_stack_partial(&item.item);
        if picked_count > 0 {
            changes.append(&mut item_changes);
            picked.push((item.clone(), picked_count));
            if picked_count < original_count {
                let mut remaining = item;
                remaining.item.item_count.0 = original_count - picked_count;
                entities.restore_dropped_item(remaining);
            }
        } else {
            entities.restore_dropped_item(item);
        }
    }

    for slot in remote_slots {
        let (mut item_changes, picked_count) = inventory.add_item_stack_partial(&slot);
        if picked_count > 0 {
            changes.append(&mut item_changes);
        }
    }

    if picked.is_empty() {
        return Ok(());
    }

    sync_inventory_changes(
        sink,
        players,
        actor,
        collector_entity_id,
        inventory.selected_slot(),
        changes,
    )
    .await?;

    for (item, amount) in picked {
        let packets = item_pickup_packets(&item, collector_entity_id, amount)?;
        for packet in &packets {
            sink.send_raw(packet.clone()).await?;
        }
        players.broadcast_packets_except(actor, packets);
    }
    if refresh_sidebar {
        let sidebar_player = players.player_by_uuid(actor).unwrap_or_else(|| {
            let mut player = player.clone();
            player.position = *position;
            player.dimension = play_dimension.clone();
            player
        });
        for packet in scoreboard::refresh_lobby_sidebar_packets(
            &server_config.scoreboard,
            lobby,
            lobby_status,
            server_config.placeholders.enable,
            plugins,
            &sidebar_player,
            players.online_count(),
            server_config.max_player,
        )? {
            sink.send_raw(packet).await?;
        }
    }
    Ok(())
}

fn item_pickup_packets(
    item: &crate::entities::DroppedItemEntity,
    collector_entity_id: i32,
    amount: i32,
) -> Result<Vec<bytes::Bytes>> {
    if amount >= item.item.item_count.0 {
        return item.pickup_packets(collector_entity_id);
    }

    let mut packets = vec![crate::players::packet_bytes(TakeItemEntity {
        item_id: VarInt(item.entity_id),
        player_id: VarInt(collector_entity_id),
        amount: VarInt(amount.max(1)),
    })?];
    let mut remaining = item.clone();
    remaining.item.item_count.0 -= amount.max(0);
    packets.extend(remaining.metadata_packets()?);
    Ok(packets)
}

async fn drop_player_item<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    world: &WorldManager,
    players: &PlayerManager,
    cluster_entities: Option<&crate::cluster_entities::ClusterEntityController>,
    entities: &crate::entities::EntityManager,
    dimension: &str,
    actor: uuid::Uuid,
    entity_id: i32,
    position: EntityPosition,
    inventory: &mut crate::inventory::PlayerInventory,
    status: i32,
    rendering: &qexed_config::app::qexed::server::EntityRendering,
    simulation_distance: i32,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let Some((slot, held)) = inventory.drop_selected(player_action_drop_all(status)) else {
        return Ok(());
    };
    let drop_position =
        settle_drop_position_on_ground(world, dimension, dropped_item_position(position));
    let mut updates = Vec::new();
    let handled_by_cluster = if let Some(cluster_entities) = cluster_entities {
        cluster_entities.drop_item(
            players,
            actor,
            dimension,
            drop_position,
            rendering,
            simulation_distance,
            &held,
        )?
    } else {
        false
    };
    if !handled_by_cluster {
        updates = entities.drop_item_with_rendering(
            players,
            actor,
            dimension,
            drop_position,
            held,
            rendering,
        )?;
    }
    sync_inventory_changes(
        sink,
        players,
        actor,
        entity_id,
        inventory.selected_slot(),
        vec![crate::inventory::InventorySlotChange::Hotbar {
            slot,
            item: inventory.held_item().clone(),
        }],
    )
    .await?;
    for update in updates {
        send_dropped_item_update(sink, update).await?;
    }
    Ok(())
}

async fn send_dropped_item_update<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    update: crate::entities::DroppedItemUpdate,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let packets = match update {
        crate::entities::DroppedItemUpdate::Spawned(entity) => {
            entity.spawn_packets(crate::entities::entity_type_id("minecraft:item")?)?
        }
        crate::entities::DroppedItemUpdate::Merged(entity) => entity.metadata_packets()?,
    };
    for packet in packets {
        sink.send_raw(packet).await?;
    }
    Ok(())
}

fn player_action_drop_all(status: i32) -> bool {
    status == PLAYER_ACTION_DROP_ITEM_STACK
}

fn dropped_item_position(position: EntityPosition) -> EntityPosition {
    let yaw = f64::from(position.yaw).to_radians();
    EntityPosition {
        x: position.x - yaw.sin() * 0.4,
        y: position.y + 1.3,
        z: position.z + yaw.cos() * 0.4,
        yaw: position.yaw,
        pitch: position.pitch,
        on_ground: false,
    }
}

fn log_player_movement_observation(
    name: &str,
    packet: &str,
    logged: &mut u8,
    previous: EntityPosition,
    current: EntityPosition,
    input_flags: Option<u8>,
) {
    if *logged >= 8 {
        return;
    }
    *logged += 1;
    if let Some(flags) = input_flags {
        log::info!(
            "client movement observed: player={name}, packet={packet}, input_flags={flags:#04x}, pos=({:.3},{:.3},{:.3})",
            current.x,
            current.y,
            current.z
        );
        return;
    }
    log::info!(
        "client movement observed: player={name}, packet={packet}, from=({:.3},{:.3},{:.3}), to=({:.3},{:.3},{:.3}), on_ground={}",
        previous.x,
        previous.y,
        previous.z,
        current.x,
        current.y,
        current.z,
        current.on_ground
    );
}

fn lift_drop_position_out_of_blocks(
    world: &WorldManager,
    dimension: &str,
    mut position: EntityPosition,
) -> EntityPosition {
    for _ in 0..8 {
        if !drop_item_intersects_blocks(world, dimension, position) {
            return position;
        }
        position.y = position.y.floor() + 1.05;
        position.on_ground = false;
    }
    position
}

fn settle_drop_position_on_ground(
    world: &WorldManager,
    dimension: &str,
    position: EntityPosition,
) -> EntityPosition {
    const FALL_STEP: f64 = 0.25;
    const MAX_FALL_STEPS: usize = 384;

    let mut current = lift_drop_position_out_of_blocks(world, dimension, position);
    for _ in 0..MAX_FALL_STEPS {
        if current.y <= f64::from(crate::world::WORLD_MIN_Y) {
            break;
        }
        let mut probe = current;
        probe.y -= FALL_STEP;
        if drop_item_intersects_blocks(world, dimension, probe) {
            return current;
        }
        current = probe;
    }
    current
}

fn drop_item_intersects_blocks(
    world: &WorldManager,
    dimension: &str,
    position: EntityPosition,
) -> bool {
    const ITEM_HALF_WIDTH: f64 = 0.125;
    const ITEM_HEIGHT: f64 = 0.25;

    let min_x = position.x - ITEM_HALF_WIDTH;
    let max_x = position.x + ITEM_HALF_WIDTH;
    let min_y = position.y;
    let max_y = position.y + ITEM_HEIGHT;
    let min_z = position.z - ITEM_HALF_WIDTH;
    let max_z = position.z + ITEM_HALF_WIDTH;

    for block_x in min_x.floor() as i32..=max_x.floor() as i32 {
        for block_y in min_y.floor() as i32..=max_y.floor() as i32 {
            for block_z in min_z.floor() as i32..=max_z.floor() as i32 {
                let block = BlockPosition {
                    x: block_x,
                    y: block_y,
                    z: block_z,
                };
                let state = world
                    .block_state_at(dimension, &block)
                    .unwrap_or_else(crate::inventory::air_block_state);
                let Some(shape) = crate::inventory::block_collision_shape(state) else {
                    continue;
                };
                if item_aabb_intersects_block_shape(
                    (min_x, max_x, min_y, max_y, min_z, max_z),
                    &block,
                    shape,
                ) {
                    return true;
                }
            }
        }
    }
    false
}

fn item_aabb_intersects_block_shape(
    item: (f64, f64, f64, f64, f64, f64),
    block: &BlockPosition,
    shape: crate::inventory::BlockCollisionShape,
) -> bool {
    let (min_x, max_x, min_y, max_y, min_z, max_z) = item;
    let block_min_x = block.x as f64 + shape.min_x;
    let block_max_x = block.x as f64 + shape.max_x;
    let block_min_y = block.y as f64 + shape.min_y;
    let block_max_y = block.y as f64 + shape.max_y;
    let block_min_z = block.z as f64 + shape.min_z;
    let block_max_z = block.z as f64 + shape.max_z;

    max_x > block_min_x
        && min_x < block_max_x
        && max_y > block_min_y
        && min_y < block_max_y
        && max_z > block_min_z
        && min_z < block_max_z
}

fn log_block_break_rejected(
    reason: &'static str,
    dimension: &str,
    position: &BlockPosition,
    block_state: Option<i32>,
) {
    let block_name = block_state
        .and_then(crate::inventory::block_name_for_state)
        .unwrap_or_else(|| "unknown".to_string());
    log::debug!(
        "block break rejected: reason={reason}, dimension={dimension}, position=({}, {}, {}), block_state={:?}, block={block_name}",
        position.x,
        position.y,
        position.z,
        block_state
    );
}

async fn begin_destroy_block<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    world: &WorldManager,
    ore_pits: &crate::world::OrePitManager,
    plugins: &crate::plugins::PluginManager,
    game_mode: GameMode,
    dimension: &str,
    position: &BlockPosition,
    held_item: &qexed_protocol::types::Slot,
) -> Result<Option<mining::PendingDig>>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    if game_mode == GameMode::Creative {
        return Ok(None);
    }
    if game_mode != GameMode::Survival {
        log_block_break_rejected("non_survival_start", dimension, position, None);
        send_block_rollback(sink, world, dimension, position.clone()).await?;
        return Ok(None);
    }

    let block_state = world
        .block_state_at(dimension, position)
        .unwrap_or_else(crate::inventory::air_block_state);
    if crate::inventory::is_air_block_state(block_state) {
        log_block_break_rejected("air_block_start", dimension, position, Some(block_state));
        send_block_rollback(sink, world, dimension, position.clone()).await?;
        return Ok(None);
    }
    if !ore_pits.permits_player_break(dimension, position, block_state) {
        log_block_break_rejected(
            "ore_pit_restriction_start",
            dimension,
            position,
            Some(block_state),
        );
        send_block_rollback(sink, world, dimension, position.clone()).await?;
        return Ok(None);
    }

    let required = mining::required_break_duration(block_state, held_item, plugins);
    Ok(Some(mining::PendingDig::new(
        position.clone(),
        block_state,
        held_item,
        required,
    )))
}

async fn can_finish_destroy_block<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    world: &WorldManager,
    ore_pits: &crate::world::OrePitManager,
    game_mode: GameMode,
    dimension: &str,
    position: &BlockPosition,
    held_item: &qexed_protocol::types::Slot,
    pending: &Option<mining::PendingDig>,
) -> Result<bool>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    if game_mode == GameMode::Creative {
        return Ok(true);
    }
    if game_mode != GameMode::Survival {
        log_block_break_rejected("non_survival_finish", dimension, position, None);
        send_block_rollback(sink, world, dimension, position.clone()).await?;
        return Ok(false);
    }

    let block_state = world
        .block_state_at(dimension, position)
        .unwrap_or_else(crate::inventory::air_block_state);
    if !ore_pits.permits_player_break(dimension, position, block_state) {
        log_block_break_rejected(
            "ore_pit_restriction_finish",
            dimension,
            position,
            Some(block_state),
        );
        send_block_rollback(sink, world, dimension, position.clone()).await?;
        return Ok(false);
    }
    let Some(pending) = pending else {
        log_block_break_rejected(
            "missing_pending_dig",
            dimension,
            position,
            Some(block_state),
        );
        send_block_rollback(sink, world, dimension, position.clone()).await?;
        return Ok(false);
    };
    if !pending.matches(position, block_state, held_item) {
        log_block_break_rejected(
            "pending_dig_mismatch",
            dimension,
            position,
            Some(block_state),
        );
        send_block_rollback(sink, world, dimension, position.clone()).await?;
        return Ok(false);
    }

    if !pending.is_complete() {
        log_block_break_rejected(
            "pending_dig_incomplete",
            dimension,
            position,
            Some(block_state),
        );
        send_block_rollback(sink, world, dimension, position.clone()).await?;
        return Ok(false);
    }

    Ok(true)
}

async fn sync_inventory_changes<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    players: &PlayerManager,
    actor: uuid::Uuid,
    entity_id: i32,
    selected_slot: usize,
    changes: Vec<crate::inventory::InventorySlotChange>,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let mut equipment = Vec::new();
    for change in changes {
        match change {
            crate::inventory::InventorySlotChange::Hotbar { slot, item } => {
                sink.send(crate::inventory::set_player_inventory_packet(
                    slot,
                    item.clone(),
                ))
                .await?;
                if slot == selected_slot {
                    equipment.push(
                        qexed_protocol::to_client::play::set_equipment::Equipment::mainhand(item),
                    );
                }
            }
            crate::inventory::InventorySlotChange::Main { slot, item } => {
                sink.send(crate::inventory::set_player_main_inventory_packet(
                    slot, item,
                ))
                .await?;
            }
            crate::inventory::InventorySlotChange::Equipment { slot, item } => {
                equipment.push(
                    qexed_protocol::to_client::play::set_equipment::Equipment::new(slot, item),
                );
            }
        }
    }

    if !equipment.is_empty() {
        sink.send(crate::inventory::equipment_packet(
            entity_id,
            equipment.clone(),
        ))
        .await?;
        players.update_equipment(actor, equipment);
    }
    Ok(())
}

async fn resync_inventory_state<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    players: &PlayerManager,
    actor: uuid::Uuid,
    entity_id: i32,
    inventory: &crate::inventory::PlayerInventory,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    sink.send(SetHeldSlot {
        slot: VarInt(inventory.selected_slot() as i32),
    })
    .await?;
    for packet in inventory.set_player_inventory_packets() {
        sink.send(packet).await?;
    }
    let equipment = inventory.visible_equipment();
    sink.send(crate::inventory::equipment_packet(
        entity_id,
        equipment.clone(),
    ))
    .await?;
    players.update_equipment(actor, equipment);
    Ok(())
}

async fn clear_carried_item<W>(sink: &mut qexed_tcp_connect::PacketSink<W>) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    sink.send(container_set_slot::ContainerSetContent {
        window_id: VarInt(-1),
        state_id: VarInt(0),
        slot: -1,
        slot_data: crate::inventory::empty_slot(),
    })
    .await?;
    Ok(())
}

async fn apply_block_change<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    world: &WorldManager,
    world_rules: &crate::world::WorldRulesManager,
    players: &PlayerManager,
    fluid: &FluidRuntime,
    world_config: &qexed_config::app::qexed::server::World,
    game_mode: GameMode,
    dimension: &str,
    actor: uuid::Uuid,
    position: BlockPosition,
    block_state: i32,
    edit_kind: WorldEditKind,
    gameplay_block_updates: bool,
) -> Result<bool>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let write_mode = world_write_mode(
        world,
        world_config,
        game_mode,
        world_rules,
        dimension,
        &position,
        edit_kind,
        gameplay_block_updates,
    );
    if !write_mode.allowed {
        log::debug!(
            "blocked world edit: reason={}, kind={:?}, dimension={}, read_only={}, spawn_protection_radius={}, position=({}, {}, {})",
            write_mode.reason,
            edit_kind,
            dimension,
            world_config.read_only,
            world_config.spawn_protection_radius,
            position.x,
            position.y,
            position.z
        );
        send_block_rollback(sink, world, dimension, position).await?;
        return Ok(false);
    }

    if write_mode.runtime_only {
        world.set_runtime_block(dimension, position.clone(), block_state);
    } else {
        world.place_block(dimension, position.clone(), block_state);
    }
    fluid.enqueue_block_change(dimension, &position);
    sink.send(crate::inventory::block_update(
        position.clone(),
        block_state,
    ))
    .await?;
    let dimension_rule = world_rules.snapshot(dimension);
    let light_update = if world.dynamic_light_enabled()
        && matches!(
            dimension_rule.light,
            qexed_config::app::qexed::server::LightMode::Dynamic
        ) {
        let update = world.light_update(
            dimension,
            position.x.div_euclid(16),
            position.z.div_euclid(16),
        );
        sink.send(update.clone()).await?;
        Some(qexed_tcp_connect::PacketSink::<tokio::io::Sink>::build_send_packet(update)?)
    } else {
        None
    };
    players.broadcast_block_changed(actor, dimension, position, block_state, light_update);
    Ok(true)
}

#[derive(Debug, Clone, Copy)]
enum WorldEditKind {
    Break,
    Place,
}

#[derive(Debug, Clone, Copy)]
struct WorldWriteMode {
    allowed: bool,
    runtime_only: bool,
    reason: &'static str,
}

fn world_write_mode(
    world: &WorldManager,
    world_config: &qexed_config::app::qexed::server::World,
    game_mode: GameMode,
    world_rules: &crate::world::WorldRulesManager,
    dimension: &str,
    position: &BlockPosition,
    edit_kind: WorldEditKind,
    gameplay_block_updates: bool,
) -> WorldWriteMode {
    if !gameplay_block_updates {
        return WorldWriteMode {
            allowed: false,
            runtime_only: false,
            reason: "gameplay_block_updates_disabled",
        };
    }

    if !can_attempt_world_edit_for_game_mode(game_mode, world_config) {
        return WorldWriteMode {
            allowed: false,
            runtime_only: false,
            reason: "world_edit_not_attemptable",
        };
    }

    let rule = world_rules.snapshot(dimension);
    if !rule.block_updates {
        return WorldWriteMode {
            allowed: false,
            runtime_only: false,
            reason: "dimension_block_updates_disabled",
        };
    }

    if can_modify_world_for_game_mode(game_mode, world_config, position) && !rule.read_only {
        return WorldWriteMode {
            allowed: true,
            runtime_only: false,
            reason: "normal_world_edit",
        };
    }

    let region = match edit_kind {
        WorldEditKind::Break => world.editable_region_for_player_break(dimension, position),
        WorldEditKind::Place => world.editable_region_for_player_place(dimension, position),
    };
    let Some(region) = region else {
        return WorldWriteMode {
            allowed: false,
            runtime_only: false,
            reason: "no_matching_edit_region",
        };
    };

    WorldWriteMode {
        allowed: true,
        runtime_only: region.runtime_only || world_config.read_only || rule.read_only,
        reason: "runtime_edit_region",
    }
}

async fn send_block_rollback<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    world: &WorldManager,
    dimension: &str,
    position: BlockPosition,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    sink.send(crate::inventory::block_update(
        position.clone(),
        world
            .block_state_at(dimension, &position)
            .unwrap_or_else(crate::inventory::air_block_state),
    ))
    .await?;
    Ok(())
}

async fn send_block_change_ack<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    sequence: VarInt,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    sink.send(crate::inventory::acknowledge_block_change(sequence).packet())
        .await?;
    Ok(())
}

async fn send_block_destruction_stage<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    entity_id: i32,
    position: BlockPosition,
    stage: i8,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    use qexed_packet::net_types::VarInt;
    use qexed_protocol::to_client::play::block_destruction::BlockDestruction;
    sink.send(BlockDestruction {
        entity_id: VarInt(entity_id),
        location: position,
        destroy_stage: stage,
    })
    .await?;
    Ok(())
}

const BONE_MEAL_PARTICLE_ID: i32 = 2005;

async fn send_bone_meal_particles<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    position: BlockPosition,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    use qexed_protocol::to_client::play::level_event::LevelEvent;
    sink.send(LevelEvent {
        event_id: BONE_MEAL_PARTICLE_ID,
        position,
        data: 0,
        global_event: false,
    })
    .await?;
    Ok(())
}

async fn disconnect_play<W>(sink: &mut qexed_tcp_connect::PacketSink<W>, reason: &str) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    sink.send(qexed_protocol::to_client::play::disconnect::Disconnect {
        reason: text_component(reason),
    })
    .await?;
    Ok(())
}

async fn sync_held_item_after_world_edit<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    players: &PlayerManager,
    actor: uuid::Uuid,
    game_mode: GameMode,
    inventory: &mut crate::inventory::PlayerInventory,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    if game_mode == GameMode::Creative {
        return Ok(());
    }

    let Some((slot, held)) = inventory.consume_selected_one() else {
        return Ok(());
    };
    sink.send(crate::inventory::set_player_inventory_packet(
        slot,
        held.clone(),
    ))
    .await?;
    players.update_equipment(
        actor,
        vec![qexed_protocol::to_client::play::set_equipment::Equipment::mainhand(held)],
    );
    Ok(())
}

fn should_destroy_block(game_mode: GameMode, action_status: i32) -> bool {
    match game_mode {
        GameMode::Creative => action_status == PLAYER_ACTION_START_DESTROY_BLOCK,
        GameMode::Survival => action_status == PLAYER_ACTION_STOP_DESTROY_BLOCK,
        GameMode::Adventure | GameMode::Spectator => false,
    }
}

fn adventure_destroy_packet_violates_can_break(
    world: &WorldManager,
    game_mode: GameMode,
    dimension: &str,
    position: &BlockPosition,
    action_status: i32,
    held_item: &qexed_protocol::types::Slot,
) -> bool {
    if game_mode != GameMode::Adventure || !player_action_attempts_destroy_block(action_status) {
        return false;
    }

    let block_state = world
        .block_state_at(dimension, position)
        .unwrap_or_else(crate::inventory::air_block_state);
    !crate::inventory::is_air_block_state(block_state)
        && !mining::held_item_allows_adventure_break(held_item, block_state)
}

fn player_action_attempts_destroy_block(action_status: i32) -> bool {
    matches!(
        action_status,
        PLAYER_ACTION_START_DESTROY_BLOCK | PLAYER_ACTION_STOP_DESTROY_BLOCK
    )
}

fn player_action_changes_block(action_status: i32) -> bool {
    matches!(
        action_status,
        PLAYER_ACTION_START_DESTROY_BLOCK
            | PLAYER_ACTION_CANCEL_DESTROY_BLOCK
            | PLAYER_ACTION_STOP_DESTROY_BLOCK
    )
}

fn player_intersects_block(player: &EntityPosition, block: &BlockPosition) -> bool {
    let player_min_x = player.x - 0.3;
    let player_max_x = player.x + 0.3;
    let player_min_y = player.y;
    let player_max_y = player.y + PLAYER_HEIGHT_BLOCKS;
    let player_min_z = player.z - 0.3;
    let player_max_z = player.z + 0.3;

    let block_min_x = block.x as f64;
    let block_max_x = block_min_x + 1.0;
    let block_min_y = block.y as f64;
    let block_max_y = block_min_y + 1.0;
    let block_min_z = block.z as f64;
    let block_max_z = block_min_z + 1.0;

    player_min_x < block_max_x
        && player_max_x > block_min_x
        && player_min_y < block_max_y
        && player_max_y > block_min_y
        && player_min_z < block_max_z
        && player_max_z > block_min_z
}

fn offset_position(position: &BlockPosition, dx: i32, dy: i32, dz: i32) -> BlockPosition {
    BlockPosition {
        x: position.x + dx,
        y: position.y + dy,
        z: position.z + dz,
    }
}

fn stepped_block_position(position: EntityPosition) -> Option<BlockPosition> {
    if !position.on_ground {
        return None;
    }
    Some(BlockPosition {
        x: position.x.floor() as i32,
        y: (position.y - 0.0001).floor() as i32,
        z: position.z.floor() as i32,
    })
}

fn stepped_plugin_block(
    world: &WorldManager,
    dimension: &str,
    position: EntityPosition,
) -> Option<(BlockPosition, i32, String)> {
    let base = stepped_block_position(position)?;
    let surface = BlockPosition {
        x: base.x,
        y: position.y.floor() as i32,
        z: base.z,
    };
    if surface.y != base.y {
        if let Some((state, name)) = named_non_air_block(world, dimension, &surface) {
            if is_pressure_plate_block(&name) {
                return Some((surface, state, name));
            }
        }
    }

    let (state, name) = named_non_air_block(world, dimension, &base)?;
    Some((base, state, name))
}

fn named_non_air_block(
    world: &WorldManager,
    dimension: &str,
    position: &BlockPosition,
) -> Option<(i32, String)> {
    let state = world.block_state_at(dimension, position)?;
    if crate::inventory::is_air_block_state(state) {
        return None;
    }
    let name = crate::inventory::block_name_for_state(state)
        .unwrap_or_else(|| format!("minecraft:unknown_block_state_{state}"));
    Some((state, name))
}

fn is_pressure_plate_block(block_name: &str) -> bool {
    block_name == "minecraft:pressure_plate" || block_name.ends_with("_pressure_plate")
}

#[derive(Debug, Clone)]
struct PlacedBlockChange {
    position: BlockPosition,
    block_state: i32,
}

#[derive(Debug, Clone)]
struct DestroyedBlockChange {
    position: BlockPosition,
    previous_state: i32,
}

#[cfg(test)]
mod tests;
fn is_fluid_block(_name: &str) -> bool {
    false
}
fn is_fluid_source(_name: &str) -> bool {
    false
}
struct FluidKind;
impl FluidKind {
    fn from_block_state(_: i32) -> Option<Self> {
        None
    }
}

mod bootstrap;
mod chat;
mod chunks;
mod drops;
mod events;
mod gameplay;
mod lobby;
mod menus;
mod mining;
pub(crate) mod pathfinding;
mod recipes;
mod scoreboard;
mod session;
mod survival;
mod util;

use anyhow::Result;
use std::{
    collections::{HashSet, VecDeque},
    time::{Duration, Instant},
};

use qexed_config::app::qexed::server::GameMode;
use qexed_packet::{
    Packet,
    net_types::{Position as BlockPosition, VarInt},
};
use qexed_protocol::to_client::play::{
    add_entity::EntityPosition,
    command_suggestions::{CommandSuggestions, Matches},
    container_set_slot,
    damage_event::{DamageEvent, DamageSourcePosition},
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

use crate::player_data::{PlayerData, PlayerDataManager};
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
#[cfg(test)]
use util::player_ability_flags;
use util::{
    acknowledged_player_ability_flags, can_attempt_world_edit, can_modify_world, chunk_coord,
    dimension_type_holder_id, keep_alive_id, text_component, translatable_component,
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
const ADVENTURE_BREAK_CHEAT_BAN_REASON: &str = "开第三方客户端";

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
    let mut saved_player = player_data
        .load_or_default(profile, &default_dimension, &world_config.spawn)
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
    let _ = lobby::sync_navigator_item(&mut inventory, &initial_lobby);
    let initial_menus = menus::MenuRuntime::new(&config.server.menus);
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
    let session = players.join_with_skin_parts(
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
    let simulation_distance = world_config.simulation_distance.max(1);
    let mut position = session.player.position;
    let mut initial_cluster_entity_view_sent = false;
    let mut last_stepped_block: Option<BlockPosition> = None;
    let mut last_input_flags = 0u8;
    let mut movement_observation_logs = 0u8;
    let mut survival = SurvivalState::from_stored(saved_player.survival, world_config.game_mode);
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
    chunk_state.refresh_pending_chunks();
    chunk_state
        .send_center_chunk_first(sink, world, plugins)
        .await?;
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
                let Some(loaded_chunk) = loaded_chunk else {
                    anyhow::bail!("chunk load task channel closed");
                };
                chunk_state
                    .queue_loaded_chunk(loaded_chunk);
                chunk_state.start_next_chunk_load(world, Some(&chunk_sender), sink.compression_threshold());
                chunk_state
                    .send_ready_chunks(sink, world, plugins, &chunk_sender, false)
                    .await?;
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
                chunk_state
                    .send_ready_chunks(sink, world, plugins, &chunk_sender, true)
                    .await?;
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
                if apply_survival_tick(
                    sink,
                    players,
                    entities,
                    world_config.game_mode,
                    &play_dimension,
                    profile.uuid,
                    session.player.entity_id,
                    position,
                    &mut survival,
                    &mut inventory,
                    config.server.gameplay.drop_inventory_on_death,
                    &config.server.entity_rendering,
                ).await? {
                    pending_dig = None;
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
                        players,
                        entities,
                        world_config.game_mode,
                        profile.uuid,
                        &play_dimension,
                        session.player.entity_id,
                        position,
                        &mut inventory,
                        message,
                        config.server.gameplay.drop_inventory_on_death,
                        &config.server.entity_rendering,
                    )
                    .await?;
                    pending_dig = None;
                }
            }
            _ = gameplay_tick.tick() => {
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
                    &inventory,
                )
                .await?;
                if handled {
                    pending_dig = None;
                }

                if gameplay_runtime.should_tick_furnace(&config.server.gameplay) {
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
                    let underwater = fall_context_at(world, &play_dimension, position).in_water;
                    let oxygen = gameplay_runtime
                        .oxygen
                        .tick(
                            sink,
                            &session.player,
                            plugins,
                            &config.server.gameplay,
                            underwater,
                            &inventory,
                            &gameplay_runtime.effects,
                            &mut survival,
                            world_config.game_mode,
                        )
                        .await?;
                    if let Some(message) = oxygen.death_message() {
                        handle_player_death(
                            sink,
                            players,
                            entities,
                            world_config.game_mode,
                            profile.uuid,
                            &play_dimension,
                            session.player.entity_id,
                            position,
                            &mut inventory,
                            message,
                            config.server.gameplay.drop_inventory_on_death,
                            &config.server.entity_rendering,
                        )
                        .await?;
                        pending_dig = None;
                    }
                    if underwater {
                        let mut outcome = gameplay::GameplayActionOutcome::default();
                        outcome.grant_triggers.push(
                            qexed_config::app::qexed::server::CustomAdvancementTrigger::EnterWater,
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
                }
            }
            _ = world_time_tick.tick() => {
                let game_time = world_rules.tick_dimension_time(&play_dimension);
                sink.send(SetTime {
                    game_time,
                    clock_updates: Vec::new(),
                })
                .await?;
                sink.flush().await?;
            }
            _ = sidebar_refresh_tick.tick() => {
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
                save_player_runtime(
                    player_data,
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
                            players,
                            entities,
                            world_config.game_mode,
                            profile.uuid,
                            &play_dimension,
                            session.player.entity_id,
                            position,
                            &mut inventory,
                            &mut survival,
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
                        }
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
                            players,
                            entities,
                            world_config.game_mode,
                            profile.uuid,
                            &play_dimension,
                            session.player.entity_id,
                            position,
                            &mut inventory,
                            &mut survival,
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
                                game_mode: world_config.game_mode.protocol_id(),
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
                            chunk_state
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
                            resync_inventory_state(
                                sink,
                                players,
                                profile.uuid,
                                session.player.entity_id,
                                &inventory,
                            )
                            .await?;
                        } else {
                            chunk_state
                                .reset_after_respawn(
                                    sink,
                                    &chunk_sender,
                                    world,
                                    plugins,
                                    position.x,
                                    position.z,
                                )
                                .await?;
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
                    chunk_state
                        .send_ready_chunks(sink, world, plugins, &chunk_sender, false)
                        .await?;
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
                    chunk_state
                        .send_ready_chunks(sink, world, plugins, &chunk_sender, false)
                        .await?;
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
                    if world_config.game_mode != GameMode::Creative {
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
                            &inventory,
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
                            &inventory,
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
                            &inventory,
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
                            &inventory,
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
                        &inventory,
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
                            if config.server.gameplay.crafting_table
                                && block_name == "minecraft:crafting_table"
                            {
                                gameplay_runtime.crafting.open(sink).await?;
                                send_block_change_ack(sink, sequence).await?;
                                sink.flush().await?;
                                continue;
                            }
                            if config.server.gameplay.furnace && block_name == "minecraft:furnace" {
                                gameplay_runtime.furnace.open(sink).await?;
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
                                world_config,
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
                                    world_config.game_mode,
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
                            &inventory,
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
                    if survival.is_dead() {
                        pending_dig = None;
                        send_block_change_ack(sink, sequence).await?;
                        sink.flush().await?;
                        continue;
                    }
                    if adventure_destroy_packet_violates_can_break(
                        world,
                        world_config.game_mode,
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
                                &inventory,
                            )
                            .await?;
                        }
                        send_block_change_ack(sink, sequence).await?;
                        sink.flush().await?;
                        continue;
                    }
                    match action.status.0 {
                        PLAYER_ACTION_START_DESTROY_BLOCK => {
                            if should_destroy_block(world_config.game_mode, PLAYER_ACTION_START_DESTROY_BLOCK) {
                                if let Some(destroyed) = destroy_block(
                                    sink,
                                    world,
                                    world_rules,
                                    ore_pits,
                                    players,
                                    entities,
                                    plugins,
                                    world_config,
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
                                }
                            } else {
                                pending_dig = begin_destroy_block(
                                    sink,
                                    world,
                                    ore_pits,
                                    plugins,
                                    world_config.game_mode,
                                    &play_dimension,
                                    &action.location,
                                    inventory.held_item(),
                                )
                                .await?;
                            }
                        }
                        PLAYER_ACTION_CANCEL_DESTROY_BLOCK => {
                            pending_dig = None;
                        }
                        status if should_destroy_block(world_config.game_mode, status) => {
                            let can_destroy = can_finish_destroy_block(
                                sink,
                                world,
                                ore_pits,
                                world_config.game_mode,
                                &play_dimension,
                                &action.location,
                                inventory.held_item(),
                                &pending_dig,
                            )
                            .await?;
                            pending_dig = None;
                            if can_destroy {
                                let destroyed = destroy_block(
                                    sink,
                                    world,
                                    world_rules,
                                    ore_pits,
                                    players,
                                    entities,
                                    plugins,
                                    world_config,
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
                                    survival.apply_exhaustion(MINING_EXHAUSTION_PER_BLOCK);
                                    if world_config.game_mode == GameMode::Survival {
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
                            &inventory,
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
                            players,
                            plugins,
                            &session.player,
                            &config.server.gameplay,
                            world_config.game_mode,
                            &mut inventory,
                            &mut survival,
                            &mut gameplay_runtime,
                        )
                        .await?
                    {
                        pending_dig = None;
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
                                &inventory,
                                viewer_position,
                                config.server.entity_rendering.player_distance,
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
                            &inventory,
                            viewer_position,
                            config.server.entity_rendering.player_distance,
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
                            &inventory,
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
                            &inventory,
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
                            world_config,
                            &mut play_dimension,
                            profile.uuid,
                            session.player.entity_id,
                            &chunk_sender,
                            &mut chunk_state,
                            &mut position,
                            &mut survival,
                            &inventory,
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
                            world_config.game_mode,
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
                        players,
                        entities,
                        &mut survival,
                        world_config.game_mode,
                        world_config.allow_flight,
                        &play_dimension,
                        profile.uuid,
                        session.player.entity_id,
                        previous,
                        position,
                        &mut inventory,
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
                        &inventory,
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
                            &inventory,
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
                        players,
                        entities,
                        &mut survival,
                        world_config.game_mode,
                        world_config.allow_flight,
                        &play_dimension,
                        profile.uuid,
                        session.player.entity_id,
                        previous,
                        position,
                        &mut inventory,
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
                        &inventory,
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
                            &inventory,
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
                        players,
                        entities,
                        &mut survival,
                        world_config.game_mode,
                        world_config.allow_flight,
                        &play_dimension,
                        profile.uuid,
                        session.player.entity_id,
                        previous,
                        position,
                        &mut inventory,
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
                        &inventory,
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
                            &inventory,
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
                        players,
                        entities,
                        &mut survival,
                        world_config.game_mode,
                        world_config.allow_flight,
                        &play_dimension,
                        profile.uuid,
                        session.player.entity_id,
                        previous,
                        position,
                        &mut inventory,
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
                        &inventory,
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
                            &inventory,
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
                            &inventory,
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
    saved_player: &mut PlayerData,
    play_dimension: &str,
    position: EntityPosition,
    inventory: &crate::inventory::PlayerInventory,
    survival: SurvivalState,
    profile_id: uuid::Uuid,
    reason: &str,
) {
    saved_player.update_runtime(play_dimension, position, inventory, survival.to_stored());
    if let Err(err) = player_data.save(saved_player).await {
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
    players: &PlayerManager,
    entities: &crate::entities::EntityManager,
    game_mode: GameMode,
    actor: uuid::Uuid,
    dimension: &str,
    collector_entity_id: i32,
    position: EntityPosition,
    inventory: &mut crate::inventory::PlayerInventory,
    survival: &mut SurvivalState,
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
        position,
        source_entity_id,
        source_position,
        knockback,
    )
    .await?;
    sink.send(survival.health_packet()).await?;
    if outcome.death_message().is_some() {
        handle_external_player_death(
            sink,
            players,
            entities,
            game_mode,
            actor,
            dimension,
            collector_entity_id,
            position,
            inventory,
            kind,
            source_entity_id,
            drop_inventory_on_death,
            rendering,
        )
        .await?;
    }
    sink.flush().await?;
    Ok(survival.is_dead())
}

#[allow(clippy::too_many_arguments)]
async fn apply_external_player_potion_effect<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    players: &PlayerManager,
    entities: &crate::entities::EntityManager,
    game_mode: GameMode,
    actor: uuid::Uuid,
    dimension: &str,
    collector_entity_id: i32,
    position: EntityPosition,
    inventory: &mut crate::inventory::PlayerInventory,
    survival: &mut SurvivalState,
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
                position,
                source_entity_id,
                source_position,
                knockback,
            )
            .await?;
        }
        if outcome.death_message().is_some() {
            handle_external_player_death(
                sink,
                players,
                entities,
                game_mode,
                actor,
                dimension,
                collector_entity_id,
                position,
                inventory,
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
    players: &PlayerManager,
    entities: &crate::entities::EntityManager,
    game_mode: GameMode,
    actor: uuid::Uuid,
    dimension: &str,
    collector_entity_id: i32,
    position: EntityPosition,
    inventory: &mut crate::inventory::PlayerInventory,
    kind: PlayerDamageKind,
    source_entity_id: i32,
    drop_inventory_on_death: bool,
    rendering: &qexed_config::app::qexed::server::EntityRendering,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
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
        dimension,
        position,
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
    players: &PlayerManager,
    entities: &crate::entities::EntityManager,
    survival: &mut SurvivalState,
    game_mode: GameMode,
    allow_flight: bool,
    dimension: &str,
    actor: uuid::Uuid,
    collector_entity_id: i32,
    previous: EntityPosition,
    current: EntityPosition,
    inventory: &mut crate::inventory::PlayerInventory,
    drop_inventory_on_death: bool,
    rendering: &qexed_config::app::qexed::server::EntityRendering,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let fall_context = fall_context_at(world, dimension, current);
    let outcome = if allow_flight {
        survival.apply_movement_with_flight(game_mode, true, previous, current, fall_context)
    } else {
        survival.apply_movement(game_mode, previous, current, fall_context)
    };
    if outcome.changed() {
        sink.send(survival.health_packet()).await?;
        if let Some(message) = outcome.death_message() {
            handle_player_death(
                sink,
                players,
                entities,
                game_mode,
                actor,
                dimension,
                collector_entity_id,
                current,
                inventory,
                message,
                drop_inventory_on_death,
                rendering,
            )
            .await?;
        }
        sink.flush().await?;
    }
    apply_fall_block_side_effects(
        sink,
        world,
        players,
        dimension,
        actor,
        previous,
        current,
        fall_context,
    )
    .await?;
    Ok(())
}

async fn apply_survival_tick<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    players: &PlayerManager,
    entities: &crate::entities::EntityManager,
    game_mode: GameMode,
    dimension: &str,
    actor: uuid::Uuid,
    collector_entity_id: i32,
    position: EntityPosition,
    survival: &mut SurvivalState,
    inventory: &mut crate::inventory::PlayerInventory,
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

    sink.send(survival.health_packet()).await?;
    if let Some(message) = outcome.death_message() {
        handle_player_death(
            sink,
            players,
            entities,
            game_mode,
            actor,
            dimension,
            collector_entity_id,
            position,
            inventory,
            message,
            drop_inventory_on_death,
            rendering,
        )
        .await?;
    }
    sink.flush().await?;
    Ok(survival.is_dead())
}

async fn handle_player_death<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    players: &PlayerManager,
    entities: &crate::entities::EntityManager,
    game_mode: GameMode,
    actor: uuid::Uuid,
    dimension: &str,
    collector_entity_id: i32,
    position: EntityPosition,
    inventory: &mut crate::inventory::PlayerInventory,
    message: DeathMessage,
    drop_inventory_on_death: bool,
    rendering: &qexed_config::app::qexed::server::EntityRendering,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    broadcast_death_message(sink, players, message, actor, collector_entity_id).await?;
    drop_player_inventory_on_death(
        sink,
        players,
        entities,
        game_mode,
        actor,
        dimension,
        position,
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
    players: &PlayerManager,
    plugins: &crate::plugins::PluginManager,
    player: &crate::players::OnlinePlayer,
    config: &qexed_config::app::qexed::server::Gameplay,
    game_mode: GameMode,
    inventory: &mut crate::inventory::PlayerInventory,
    survival: &mut SurvivalState,
    gameplay_runtime: &mut gameplay::GameplayRuntime,
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
                if let Some(message) = damage.death_message() {
                    log::debug!(
                        "player died from consumable effect: entity_id={}, message={}",
                        player.entity_id,
                        message.translation_key()
                    );
                }
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
    sink.send(survival.health_packet()).await?;
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
    let in_water = is_water_block(&body_name) || is_water_block(&head_name);
    let in_lava = is_lava_block(&body_name) || is_lava_block(&head_name);

    FallContext {
        in_water,
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
    if fall_context.in_water || fall_context.in_lava {
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

fn is_water_block(name: &str) -> bool {
    name == "minecraft:water"
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
    world_config: &qexed_config::app::qexed::server::World,
    play_dimension: &mut String,
    actor: uuid::Uuid,
    entity_id: i32,
    chunk_sender: &tokio::sync::mpsc::UnboundedSender<chunks::ChunkLoadResult>,
    chunk_state: &mut ChunkSendState,
    position: &mut EntityPosition,
    survival: &mut SurvivalState,
    inventory: &crate::inventory::PlayerInventory,
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
        game_mode: world_config.game_mode.protocol_id(),
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
    chunk_state
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
    chunk_state
        .reset_after_respawn(sink, chunk_sender, world, plugins, position.x, position.z)
        .await?;
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
            let opened = menus
                .open_menu(sink, &action.target, Some(&render_context))
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
                        let opened = menus.open_menu(sink, &menu, Some(&render_context)).await?;
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
    inventory: &crate::inventory::PlayerInventory,
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
    inventory: &crate::inventory::PlayerInventory,
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
    inventory: &crate::inventory::PlayerInventory,
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
    );

    handle_plugin_response_actions(
        sink,
        world,
        world_rules,
        server_config,
        world_config,
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
    inventory: &crate::inventory::PlayerInventory,
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
async fn handle_plugin_response_actions<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    world: &WorldManager,
    world_rules: &crate::world::WorldRulesManager,
    server_config: &qexed_config::app::qexed::server::Server,
    world_config: &qexed_config::app::qexed::server::World,
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
    inventory: &crate::inventory::PlayerInventory,
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
    inventory: &crate::inventory::PlayerInventory,
) -> Result<bool>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let mut player = player.clone();
    player.position = *position;
    player.dimension = play_dimension.clone();
    let response = plugins.handle_player_input(&player, previous_flags, flags);
    let mut handled = response.handled || !response.actions.is_empty();
    for action in response.actions {
        let before_dimension = play_dimension.clone();
        handled |= chat::apply_plugin_action(
            sink,
            Some(server_config),
            world,
            world_rules,
            world_config,
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
    inventory: &crate::inventory::PlayerInventory,
    viewer_position: EntityPosition,
    render_distance: f64,
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

async fn place_held_block<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    world: &WorldManager,
    world_rules: &crate::world::WorldRulesManager,
    players: &PlayerManager,
    world_config: &qexed_config::app::qexed::server::World,
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
            world_config,
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
            world_config,
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
        world_config,
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
    entities: &crate::entities::EntityManager,
    plugins: &crate::plugins::PluginManager,
    world_config: &qexed_config::app::qexed::server::World,
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
        world_config,
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
                world_config,
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
            entities,
            plugins,
            world_config,
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
    entities: &crate::entities::EntityManager,
    plugins: &crate::plugins::PluginManager,
    world_config: &qexed_config::app::qexed::server::World,
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
    if world_config.game_mode != GameMode::Survival {
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
            world_config,
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

    if !pending.is_complete(Instant::now()) {
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
    world_config: &qexed_config::app::qexed::server::World,
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

    if !can_attempt_world_edit(world_config) {
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

    if can_modify_world(world_config, position) && !rule.read_only {
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

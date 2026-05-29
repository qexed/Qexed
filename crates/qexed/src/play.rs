mod bootstrap;
mod chat;
mod chunks;
mod drops;
mod events;
mod lobby;
mod menus;
mod mining;
mod recipes;
mod scoreboard;
mod session;
mod survival;
mod util;

use anyhow::Result;
use std::{
    collections::HashSet,
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
    keep_alive::KeepAlive as ClientboundKeepAlive,
    login::Login,
    player_chat::{PackedMessageSignature, PlayerChat},
    player_info_update::{PlayerInfoActions, PlayerInfoEntry, PlayerInfoUpdate},
    position::Position,
    respawn::{KEEP_NO_DATA, Respawn},
    set_held_slot::SetHeldSlot,
    set_time::SetTime,
    system_chat::SystemChat,
};
use qexed_protocol::to_server::play::{
    accept_teleportation::AcceptTeleportation, attack::Attack, chat_ack::ChatAck,
    chat_command::ChatCommand, chat_message::ChatMessage, chat_session_update::ChatSessionUpdate,
    chunk_batch_received::ChunkBatchReceived, client_command::ClientCommand,
    command_suggestion::CommandSuggestion, container_click::ContainerClick,
    container_close::ContainerClose, interact::Interact,
    keep_alive::KeepAlive as ServerboundKeepAlive, move_player_pos::MovePlayerPos,
    move_player_pos_rot::MovePlayerPosRot, move_player_rot::MovePlayerRot,
    move_player_status_only::MovePlayerStatusOnly, pick_item_from_block::PickItemFromBlock,
    player_action::PlayerAction, player_input::PlayerInput, set_carried_item::SetCarriedItem,
    set_creative_mode_slot::SetCreativeModeSlot, use_item::UseItem, use_item_on::UseItemOn,
};

use crate::player_data::{PlayerData, PlayerDataManager};
use crate::players::{PlayerManager, PlayerSession};
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
    can_modify_world, chunk_coord, dimension_type_holder_id, keep_alive_id, text_component,
    translatable_component,
};

const KEEP_ALIVE_INTERVAL: std::time::Duration = std::time::Duration::from_secs(15);
const CHUNK_SEND_TICK_INTERVAL: Duration = Duration::from_millis(50);
const CHUNK_UNLOAD_SWEEP_INTERVAL: Duration = Duration::from_millis(500);
const SURVIVAL_TICK_INTERVAL: Duration = Duration::from_secs(1);
const WORLD_TIME_TICK_INTERVAL: Duration = Duration::from_secs(1);
const MINING_EXHAUSTION_PER_BLOCK: f32 = 0.005;
const PLAYER_ACTION_START_DESTROY_BLOCK: i32 = 0;
const PLAYER_ACTION_CANCEL_DESTROY_BLOCK: i32 = 1;
const PLAYER_ACTION_STOP_DESTROY_BLOCK: i32 = 2;
const PLAYER_ACTION_DROP_ITEM_STACK: i32 = 3;
const PLAYER_ACTION_DROP_ITEM: i32 = 4;
const PLAYER_HEIGHT_BLOCKS: f64 = 1.8;

pub async fn initialize<R, W>(
    packets: &mut qexed_tcp_connect::PacketStream<R>,
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    config: &crate::config::RuntimeConfig,
    authenticator: &crate::auth::Authenticator,
    world: &WorldManager,
    world_rules: &crate::world::WorldRulesManager,
    players: &PlayerManager,
    entities: &crate::entities::EntityManager,
    player_data: &PlayerDataManager,
    permissions: &crate::permissions::PermissionManager,
    plugins: &crate::plugins::PluginManager,
    player_audit: &crate::audit::PlayerAuditLogger,
    content_filter: &crate::content_filter::ContentFilter,
    profile: &qexed_packet::net_types::GameProfile,
    client_language: Option<String>,
) -> Result<()>
where
    R: tokio::io::AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
{
    let world_config = &config.world;
    let mut saved_player = player_data
        .load_or_default(profile, &world_config.dimension, &world_config.spawn)
        .await;
    let play_dimension = if saved_player.dimension.is_empty() {
        world_config.dimension.clone()
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
    let session = players.join(
        profile.clone(),
        player_position,
        play_dimension.clone(),
        inventory.visible_equipment(),
        client_language.unwrap_or_else(|| config.language.clone()),
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
        dimension_names: login_dimension_names(&play_dimension),
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
        enforces_secure_chat: config.server.online_mode,
    })
    .await?;

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

    sink.send(SystemChat {
        content: text_component("Qexed: loading world"),
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
        world_rules,
        players,
        player_data,
        entities,
        permissions,
        plugins,
        player_audit,
        content_filter,
        session,
        player_entity_type,
        profile,
        &mut saved_player,
        play_dimension,
        chunk_state,
        inventory,
        next_teleport_id,
        world_session,
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
    players: &PlayerManager,
    player_data: &PlayerDataManager,
    entities: &crate::entities::EntityManager,
    permissions: &crate::permissions::PermissionManager,
    plugins: &crate::plugins::PluginManager,
    player_audit: &crate::audit::PlayerAuditLogger,
    content_filter: &crate::content_filter::ContentFilter,
    mut session: PlayerSession,
    player_entity_type: i32,
    profile: &qexed_packet::net_types::GameProfile,
    saved_player: &mut PlayerData,
    mut play_dimension: String,
    mut chunk_state: ChunkSendState,
    mut inventory: crate::inventory::PlayerInventory,
    mut next_teleport_id: i32,
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
    let mut chunk_send_tick = tokio::time::interval(CHUNK_SEND_TICK_INTERVAL);
    chunk_send_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    chunk_send_tick.tick().await;
    let mut survival_tick = tokio::time::interval(SURVIVAL_TICK_INTERVAL);
    survival_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    survival_tick.tick().await;
    let mut world_time_tick = tokio::time::interval(WORLD_TIME_TICK_INTERVAL);
    world_time_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    world_time_tick.tick().await;
    let mut pending_keep_alive = None;
    let mut chat_session: Option<crate::secure_chat::SecureChatSession> = None;
    let mut next_chat_global_index = 0;
    let enforce_secure_chat = config.server.online_mode;
    let world_config = &config.world;
    let mut position = session.player.position;
    let mut last_stepped_block: Option<BlockPosition> = None;
    let mut last_input_flags = 0u8;
    let mut survival = SurvivalState::from_stored(saved_player.survival, world_config.game_mode);
    let mut pending_dig: Option<mining::PendingDig> = None;
    let lobby = lobby::LobbyRuntime::new(&config.server.lobby);
    let mut lobby_status = if lobby.status_refresh_interval().is_some() {
        lobby.refresh_status().await
    } else {
        lobby::LobbyStatusSnapshot::default()
    };
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
    chunk_state.start_next_chunk_load(world, Some(&chunk_sender));

    let result: Result<()> = async {
        loop {
            tokio::select! {
            loaded_chunk = chunk_receiver.recv(), if chunk_state.has_loading_chunks() => {
                let Some(loaded_chunk) = loaded_chunk else {
                    anyhow::bail!("chunk load task channel closed");
                };
                chunk_state
                    .queue_loaded_chunk(loaded_chunk);
                chunk_state.start_next_chunk_load(world, Some(&chunk_sender));
                chunk_state
                    .send_ready_chunks(sink, world, plugins, &chunk_sender, false)
                    .await?;
            }
            _ = chunk_send_tick.tick(), if chunk_state.has_ready_chunks() => {
                chunk_state
                    .send_ready_chunks(sink, world, plugins, &chunk_sender, true)
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
                    &config.server.entity_rendering,
                ).await? {
                    pending_dig = None;
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
            event = session.receiver.recv() => {
                let Some(event) = event else {
                    continue;
                };
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
                    log::debug!(
                        "client acknowledged chunk batch, desired rate: {} chunks/tick",
                        batch.desired_chunks_per_tick
                    );
                    continue;
                }

                if packet_id == CommandSuggestion::ID {
                    let suggestion = crate::connection::decode_payload::<CommandSuggestion>(&mut payload)?;
                    let matches = command_suggestion_matches(
                        &suggestion.text,
                        players,
                        &config.server.lobby,
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

                if packet_id == SetCarriedItem::ID {
                    let carried = crate::connection::decode_payload::<SetCarriedItem>(&mut payload)?;
                    if let Some(main_hand) = inventory.set_selected(carried.slot) {
                        pending_dig = None;
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
                    if lobby.protect_world() {
                        let _ = lobby::sync_navigator_item(&mut inventory, &lobby);
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
                    if lobby.navigator_slot().is_some_and(|navigator| usize::try_from(slot.slot_num).ok() == Some(navigator)) {
                        let changes = lobby::sync_navigator_item(&mut inventory, &lobby);
                        sync_inventory_changes(
                            sink,
                            players,
                            profile.uuid,
                            session.player.entity_id,
                            inventory.selected_slot(),
                            changes,
                        ).await?;
                        sink.flush().await?;
                        continue;
                    }
                    if menus.fixed_hotbar_slot(
                        usize::try_from(slot.slot_num - 36).unwrap_or(usize::MAX),
                    ) {
                        let changes = menus.sync_hotbar_items(&mut inventory);
                        sync_inventory_changes(
                            sink,
                            players,
                            profile.uuid,
                            session.player.entity_id,
                            inventory.selected_slot(),
                            changes,
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
                        let _ = lobby::sync_navigator_item(&mut inventory, &lobby);
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
                    if use_item_on.hand.0 == 0 {
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
                    if player_action_drops_item(action.status.0)
                        && menus.fixed_hotbar_slot(inventory.selected_slot())
                    {
                        pending_dig = None;
                        let changes = menus.sync_hotbar_items(&mut inventory);
                        sync_inventory_changes(
                            sink,
                            players,
                            profile.uuid,
                            session.player.entity_id,
                            inventory.selected_slot(),
                            changes,
                        )
                        .await?;
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
                                    players,
                                    entities,
                                    plugins,
                                    world_config,
                                    &play_dimension,
                                    profile.uuid,
                                    inventory.held_item(),
                                    action.location,
                                    &config.server.entity_rendering,
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
                                    players,
                                    entities,
                                    plugins,
                                    world_config,
                                    &play_dimension,
                                    profile.uuid,
                                    inventory.held_item(),
                                    action.location,
                                    &config.server.entity_rendering,
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
                                }
                                if !survival.is_dead() && was_destroyed {
                                    collect_nearby_drops(
                                        sink,
                                        players,
                                        entities,
                                        &play_dimension,
                                        profile.uuid,
                                        session.player.entity_id,
                                        position,
                                        &mut inventory,
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
                        && let Some(action) = menus.action_for_hotbar_slot(inventory.selected_slot())
                    {
                        let outcome = run_menu_action(
                            sink,
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
                            action,
                        )
                        .await?;
                        if outcome.opened_menu {
                            active_config_menu = outcome.opened_menu_id;
                        }
                        pending_dig = None;
                        send_block_change_ack(sink, sequence).await?;
                        sink.flush().await?;
                        continue;
                    }
                    if !survival.is_dead()
                        && use_item.hand.0 == 0
                        && lobby
                            .handle_use_item(sink, inventory.selected_slot(), &lobby_status)
                            .await?
                    {
                        lobby_menu_open = true;
                        pending_dig = None;
                    }
                    send_block_change_ack(sink, sequence).await?;
                    sink.flush().await?;
                    continue;
                }

                if packet_id == Interact::ID {
                    let interact = crate::connection::decode_payload::<Interact>(&mut payload)?;
                    if !survival.is_dead() {
                        let plugin_outcome = if is_primary_interact(&interact) {
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
                                "interact",
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
                            )
                            .await?
                        } else {
                            PluginNpcInteractOutcome::default()
                        };
                        let outcome = if plugin_outcome.handled {
                            lobby::LobbyInteractionOutcome::default()
                        } else {
                            lobby
                                .handle_entity_interact(sink, entities, interact, &lobby_status)
                                .await?
                        };
                        if outcome.opened_menu {
                            lobby_menu_open = true;
                        }
                        if plugin_outcome.handled || outcome.handled {
                            pending_dig = None;
                            sink.flush().await?;
                        }
                    }
                    if survival.is_dead() {
                        pending_dig = None;
                    }
                    continue;
                }

                if packet_id == Attack::ID {
                    let attack = crate::connection::decode_payload::<Attack>(&mut payload)?;
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
                        )
                        .await?;
                        if plugin_outcome.handled {
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
                    let affects_fixed_menu_slot =
                        container_click_affects_fixed_menu_slot(&click, &menus);
                    if let Some(action) = menus
                        .handle_container_click(sink, active_config_menu.as_deref(), click.clone())
                        .await?
                    {
                        let outcome = run_menu_action(
                            sink,
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
                            action,
                        )
                        .await?;
                        if outcome.opened_menu {
                            active_config_menu = outcome.opened_menu_id;
                        } else {
                            active_config_menu = None;
                            menus.close_menu(sink).await?;
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
                    } else if lobby.handle_container_click(sink, click, &lobby_status).await? {
                        lobby_menu_open = true;
                        pending_dig = None;
                        sink.flush().await?;
                    } else if lobby.protect_world() || lobby.navigator_slot().is_some() {
                        let _ = lobby::sync_navigator_item(&mut inventory, &lobby);
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
                    } else if affects_fixed_menu_slot {
                        let _ = menus.sync_hotbar_items(&mut inventory);
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
                    }
                    continue;
                }

                if packet_id == ContainerClose::ID {
                    let close = crate::connection::decode_payload::<ContainerClose>(&mut payload)?;
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
                            &play_dimension,
                            profile.uuid,
                            &chunk_sender,
                            &mut chunk_state,
                            &mut position,
                            &mut survival,
                            &mut next_teleport_id,
                        )
                        .await?;
                    }
                    continue;
                }

                if packet_id == MovePlayerPos::ID {
                    let movement = crate::connection::decode_payload::<MovePlayerPos>(&mut payload)?;
                    let previous = position;
                    position.x = movement.x;
                    position.y = movement.y;
                    position.z = movement.z;
                    position.on_ground = movement.flags & 0x01 != 0;
                    apply_survival_movement(
                        sink,
                        world,
                        players,
                        entities,
                        &mut survival,
                        world_config.game_mode,
                        &play_dimension,
                        profile.uuid,
                        session.player.entity_id,
                        previous,
                        position,
                        &mut inventory,
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
                            players,
                            entities,
                            &play_dimension,
                            profile.uuid,
                            session.player.entity_id,
                            position,
                            &mut inventory,
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
                    apply_survival_movement(
                        sink,
                        world,
                        players,
                        entities,
                        &mut survival,
                        world_config.game_mode,
                        &play_dimension,
                        profile.uuid,
                        session.player.entity_id,
                        previous,
                        position,
                        &mut inventory,
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
                            players,
                            entities,
                            &play_dimension,
                            profile.uuid,
                            session.player.entity_id,
                            position,
                            &mut inventory,
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
                    apply_survival_movement(
                        sink,
                        world,
                        players,
                        entities,
                        &mut survival,
                        world_config.game_mode,
                        &play_dimension,
                        profile.uuid,
                        session.player.entity_id,
                        previous,
                        position,
                        &mut inventory,
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
                        )
                        .await?
                    {
                        pending_dig = None;
                    }
                    players.update_position(profile.uuid, position);
                    session.player.position = position;
                    if !survival.is_dead() {
                        collect_nearby_drops(
                            sink,
                            players,
                            entities,
                            &play_dimension,
                            profile.uuid,
                            session.player.entity_id,
                            position,
                            &mut inventory,
                        )
                        .await?;
                    }
                    continue;
                }

                if packet_id == MovePlayerStatusOnly::ID {
                    let movement = crate::connection::decode_payload::<MovePlayerStatusOnly>(&mut payload)?;
                    let previous = position;
                    position.on_ground = movement.flags & 0x01 != 0;
                    apply_survival_movement(
                        sink,
                        world,
                        players,
                        entities,
                        &mut survival,
                        world_config.game_mode,
                        &play_dimension,
                        profile.uuid,
                        session.player.entity_id,
                        previous,
                        position,
                        &mut inventory,
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
                        )
                        .await?
                    {
                        pending_dig = None;
                    }
                    players.update_position(profile.uuid, position);
                    session.player.position = position;
                    if !survival.is_dead() {
                        collect_nearby_drops(
                            sink,
                            players,
                            entities,
                            &play_dimension,
                            profile.uuid,
                            session.player.entity_id,
                            position,
                            &mut inventory,
                        )
                        .await?;
                    }
                    continue;
                }

                if packet_id == PlayerInput::ID {
                    let input = crate::connection::decode_payload::<PlayerInput>(&mut payload)?;
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
                    log::debug!("received chat message: {}", chat.message);
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
                        anyhow::bail!("client did not initialize Mojang secure chat session");
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
    saved_player.update_runtime(&play_dimension, position, &inventory, survival.to_stored());
    if let Err(err) = player_data.save(saved_player).await {
        log::warn!(
            "failed to save player data: uuid={}, error={err:#}",
            profile.uuid
        );
    }

    result
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
    lobby: &qexed_config::app::qexed::server::Lobby,
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
    let mut values = command_suggestion_candidates(text, players, lobby)
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
    lobby: &qexed_config::app::qexed::server::Lobby,
) -> Vec<String> {
    let trimmed = text.trim_start_matches('/').trim_start();
    let mut parts = trimmed.split_whitespace();
    match parts
        .next()
        .map(crate::commands::normalize_command_name)
        .as_deref()
    {
        None | Some("") => crate::commands::command_names_for_suggestions()
            .into_iter()
            .map(ToString::to_string)
            .collect(),
        Some("teleport") => {
            let mut values = default_dimension_suggestions();
            values.extend(players.online_names());
            values
        }
        Some("server") => lobby
            .servers
            .iter()
            .filter_map(|server| {
                let id = server.id.trim();
                (!id.is_empty()).then_some(id.to_string())
            })
            .collect(),
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

fn default_dimension_suggestions() -> Vec<String> {
    vec![
        "minecraft:overworld".to_string(),
        "minecraft:the_nether".to_string(),
        "minecraft:the_end".to_string(),
    ]
}

fn login_dimension_names(primary: &str) -> Vec<String> {
    let mut dimensions = vec![
        primary.to_string(),
        "minecraft:overworld".to_string(),
        "minecraft:the_nether".to_string(),
        "minecraft:the_end".to_string(),
    ];
    dimensions.sort();
    dimensions.dedup();
    dimensions
}

async fn apply_survival_movement<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    world: &WorldManager,
    players: &PlayerManager,
    entities: &crate::entities::EntityManager,
    survival: &mut SurvivalState,
    game_mode: GameMode,
    dimension: &str,
    actor: uuid::Uuid,
    collector_entity_id: i32,
    previous: EntityPosition,
    current: EntityPosition,
    inventory: &mut crate::inventory::PlayerInventory,
    rendering: &qexed_config::app::qexed::server::EntityRendering,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let fall_context = fall_context_at(world, dimension, current);
    let outcome = survival.apply_movement(game_mode, previous, current, fall_context);
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
    rendering: &qexed_config::app::qexed::server::EntityRendering,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    broadcast_death_message(sink, players, message, actor, collector_entity_id).await?;

    if game_mode != GameMode::Survival {
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
        let Some(drop) = entities
            .drop_item_with_rendering(players, actor, dimension, position, item, rendering)?
        else {
            continue;
        };
        for packet in drop.spawn_packets(crate::entities::entity_type_id("minecraft:item")?)? {
            sink.send_raw(packet).await?;
        }
    }
    Ok(())
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

async fn respawn_player<W>(
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
    survival: &mut SurvivalState,
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
    let dimension_rule = world_rules.snapshot(dimension);
    sink.send(Respawn {
        dimension_type: VarInt(dimension_type_holder_id(&dimension_rule.dimension_type)),
        dimension_name: dimension.to_string(),
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
    send_respawn_player_state(sink, world_config, world_rules, dimension, *position).await?;
    sink.send(survival.health_packet()).await?;
    chunk_state
        .reset_after_respawn(sink, chunk_sender, world, plugins, position.x, position.z)
        .await?;
    players.update_position(actor, *position);
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

#[derive(Debug, Default)]
struct MenuActionOutcome {
    opened_menu: bool,
    opened_menu_id: Option<String>,
}

async fn run_menu_action<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
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
    action: qexed_config::app::qexed::server::MenuAction,
) -> Result<MenuActionOutcome>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    match action.kind {
        qexed_config::app::qexed::server::MenuActionKind::None => Ok(MenuActionOutcome::default()),
        qexed_config::app::qexed::server::MenuActionKind::OpenMenu => {
            let opened = menus.open_menu(sink, &action.target).await?;
            Ok(MenuActionOutcome {
                opened_menu: opened.is_some(),
                opened_menu_id: opened,
            })
        }
        qexed_config::app::qexed::server::MenuActionKind::Transfer => {
            let target = action.target.trim();
            if target.is_empty() {
                return Ok(MenuActionOutcome::default());
            }
            if lobby.enabled() {
                lobby
                    .transfer_to_server_with_message(sink, target, &action.message, lobby_status)
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
) -> bool {
    let clicked_fixed_hotbar_slot =
        inventory_window_hotbar_slot(click.slot).is_some_and(|slot| menus.fixed_hotbar_slot(slot));
    let changed_fixed_hotbar_slot = click.changed_slots.0.keys().any(|slot| {
        inventory_window_hotbar_slot(*slot).is_some_and(|slot| menus.fixed_hotbar_slot(slot))
    });
    clicked_fixed_hotbar_slot || changed_fixed_hotbar_slot
}

fn inventory_window_hotbar_slot(slot: i16) -> Option<usize> {
    (36..=44)
        .contains(&slot)
        .then(|| usize::try_from(slot - 36).ok())
        .flatten()
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
) -> Result<bool>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let Some(block_position) = stepped_block_position(*position) else {
        *last_stepped_block = None;
        return Ok(false);
    };
    let block_state = world
        .block_state_at(play_dimension, &block_position)
        .unwrap_or_else(crate::inventory::air_block_state);
    if crate::inventory::is_air_block_state(block_state) {
        *last_stepped_block = None;
        return Ok(false);
    }
    if last_stepped_block.as_ref() == Some(&block_position) {
        return Ok(false);
    }
    *last_stepped_block = Some(block_position.clone());

    let block_name = crate::inventory::block_name_for_state(block_state)
        .unwrap_or_else(|| format!("minecraft:unknown_block_state_{block_state}"));
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
    }
    Ok(handled)
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
    chunk_sender: &tokio::sync::mpsc::UnboundedSender<chunks::ChunkLoadResult>,
    chunk_state: &mut ChunkSendState,
    position: &mut EntityPosition,
    next_teleport_id: &mut i32,
    play_dimension: &mut String,
    menus: &menus::MenuRuntime,
    active_config_menu: &mut Option<String>,
    players_hidden: &mut bool,
    visible_player_entities: &mut HashSet<uuid::Uuid>,
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
    if entity.kind != crate::entities::ManagedEntityKind::Npc {
        return Ok(PluginNpcInteractOutcome::default());
    }

    log::debug!(
        "dispatching plugin npc interact: player={}, entity_key={}, entity_id={}, action={}",
        player.profile.username,
        entity.key,
        entity.entity_id,
        action_name
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
        action_name,
    );
    let mut handled = response.handled || !response.actions.is_empty();
    for action in response.actions {
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
    }

    Ok(PluginNpcInteractOutcome { handled })
}

fn is_primary_interact(interact: &Interact) -> bool {
    interact.hand.0 == 0
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
    players: &PlayerManager,
    entities: &crate::entities::EntityManager,
    plugins: &crate::plugins::PluginManager,
    world_config: &qexed_config::app::qexed::server::World,
    dimension: &str,
    actor: uuid::Uuid,
    held_item: &qexed_protocol::types::Slot,
    position: BlockPosition,
    rendering: &qexed_config::app::qexed::server::EntityRendering,
) -> Result<Option<DestroyedBlockChange>>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let current = world
        .block_state_at(dimension, &position)
        .unwrap_or_else(crate::inventory::air_block_state);
    if crate::inventory::is_air_block_state(current) {
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
            )
            .await?;
        }
        drop_broken_block(
            sink,
            players,
            entities,
            plugins,
            world_config.game_mode,
            actor,
            dimension,
            current,
            held_item,
            &position,
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
    players: &PlayerManager,
    entities: &crate::entities::EntityManager,
    plugins: &crate::plugins::PluginManager,
    game_mode: GameMode,
    actor: uuid::Uuid,
    dimension: &str,
    block_state: i32,
    held_item: &qexed_protocol::types::Slot,
    position: &BlockPosition,
    rendering: &qexed_config::app::qexed::server::EntityRendering,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    if game_mode != GameMode::Survival {
        return Ok(());
    }

    let drop_position = mining::drop_position(position);
    for item in mining::default_block_drops(block_state, position, held_item, plugins) {
        let Some(drop) = entities.drop_item_with_rendering(
            players,
            actor,
            dimension,
            drop_position,
            item,
            rendering,
        )?
        else {
            continue;
        };
        for packet in drop.spawn_packets(crate::entities::entity_type_id("minecraft:item")?)? {
            sink.send_raw(packet).await?;
        }
    }
    Ok(())
}

async fn collect_nearby_drops<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    players: &PlayerManager,
    entities: &crate::entities::EntityManager,
    dimension: &str,
    actor: uuid::Uuid,
    collector_entity_id: i32,
    position: EntityPosition,
    inventory: &mut crate::inventory::PlayerInventory,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let Some((items, changes)) =
        drops::collect_dropped_items(entities, dimension, position, inventory)?
    else {
        return Ok(());
    };

    sync_inventory_changes(
        sink,
        players,
        actor,
        collector_entity_id,
        inventory.selected_slot(),
        changes,
    )
    .await?;

    for item in items {
        let packets = item.pickup_packets(collector_entity_id)?;
        for packet in &packets {
            sink.send_raw(packet.clone()).await?;
        }
        players.broadcast_packets_except(actor, packets);
    }
    Ok(())
}

async fn begin_destroy_block<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    world: &WorldManager,
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
        send_block_rollback(sink, world, dimension, position.clone()).await?;
        return Ok(None);
    }

    let block_state = world
        .block_state_at(dimension, position)
        .unwrap_or_else(crate::inventory::air_block_state);
    if crate::inventory::is_air_block_state(block_state) {
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
        send_block_rollback(sink, world, dimension, position.clone()).await?;
        return Ok(false);
    }

    let block_state = world
        .block_state_at(dimension, position)
        .unwrap_or_else(crate::inventory::air_block_state);
    let Some(pending) = pending else {
        send_block_rollback(sink, world, dimension, position.clone()).await?;
        return Ok(false);
    };
    if !pending.matches(position, block_state, held_item) {
        send_block_rollback(sink, world, dimension, position.clone()).await?;
        return Ok(false);
    }

    if !pending.is_complete(Instant::now()) {
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
) -> Result<bool>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    if !can_modify_world_with_dimension_rules(world_config, world_rules, dimension, &position) {
        log::debug!(
            "blocked world edit: read_only={}, spawn_protection_radius={}, position=({}, {}, {})",
            world_config.read_only,
            world_config.spawn_protection_radius,
            position.x,
            position.y,
            position.z
        );
        send_block_rollback(sink, world, dimension, position).await?;
        return Ok(false);
    }

    world.place_block(dimension, position.clone(), block_state);
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

fn can_modify_world_with_dimension_rules(
    world_config: &qexed_config::app::qexed::server::World,
    world_rules: &crate::world::WorldRulesManager,
    dimension: &str,
    position: &BlockPosition,
) -> bool {
    if !can_modify_world(world_config, position) {
        return false;
    }
    let rule = world_rules.snapshot(dimension);
    !rule.read_only && rule.block_updates
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

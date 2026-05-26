mod bootstrap;
mod chat;
mod chunks;
mod drops;
mod events;
mod mining;
mod recipes;
mod scoreboard;
mod session;
mod survival;
mod util;

use anyhow::Result;
use std::time::{Duration, Instant};

use qexed_config::app::qexed::server::GameMode;
use qexed_packet::{
    Packet,
    net_types::{Position as BlockPosition, VarInt},
};
use qexed_protocol::to_client::play::{
    add_entity::EntityPosition,
    keep_alive::KeepAlive as ClientboundKeepAlive,
    login::Login,
    player_chat::{PackedMessageSignature, PlayerChat},
    player_info_update::{PlayerInfoActions, PlayerInfoEntry, PlayerInfoUpdate},
    position::Position,
    respawn::{KEEP_NO_DATA, Respawn},
    set_held_slot::SetHeldSlot,
    system_chat::SystemChat,
};
use qexed_protocol::to_server::play::{
    accept_teleportation::AcceptTeleportation, chat_ack::ChatAck, chat_command::ChatCommand,
    chat_message::ChatMessage, chat_session_update::ChatSessionUpdate,
    chunk_batch_received::ChunkBatchReceived, client_command::ClientCommand,
    keep_alive::KeepAlive as ServerboundKeepAlive, move_player_pos::MovePlayerPos,
    move_player_pos_rot::MovePlayerPosRot, move_player_rot::MovePlayerRot,
    move_player_status_only::MovePlayerStatusOnly, pick_item_from_block::PickItemFromBlock,
    player_action::PlayerAction, set_carried_item::SetCarriedItem,
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
const MINING_EXHAUSTION_PER_BLOCK: f32 = 0.005;
const PLAYER_ACTION_START_DESTROY_BLOCK: i32 = 0;
const PLAYER_ACTION_CANCEL_DESTROY_BLOCK: i32 = 1;
const PLAYER_ACTION_STOP_DESTROY_BLOCK: i32 = 2;
const PLAYER_HEIGHT_BLOCKS: f64 = 1.8;

pub async fn initialize<R, W>(
    packets: &mut qexed_tcp_connect::PacketStream<R>,
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    config: &qexed_config::app::qexed::Qexed,
    authenticator: &crate::auth::Authenticator,
    world: &WorldManager,
    players: &PlayerManager,
    entities: &crate::entities::EntityManager,
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
    let view_distance = world_config.view_distance.max(1);
    let chunk_load_parallelism = chunk_load_parallelism_limit(world_config.chunk_load_parallelism);
    let simulation_distance = world_config.simulation_distance.max(1);
    let inventory = saved_player.inventory();
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
        inventory.visible_equipment(),
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
        saved_player.survival,
        permissions,
    )
    .await?;
    send_existing_players(sink, players, profile.uuid, player_entity_type).await?;
    send_existing_entities(sink, entities, &play_dimension).await?;

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
        players,
        player_data,
        entities,
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
    config: &qexed_config::app::qexed::Qexed,
    authenticator: &crate::auth::Authenticator,
    world: &WorldManager,
    players: &PlayerManager,
    player_data: &PlayerDataManager,
    entities: &crate::entities::EntityManager,
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
    let mut pending_keep_alive = None;
    let mut chat_session: Option<crate::secure_chat::SecureChatSession> = None;
    let mut next_chat_global_index = 0;
    let enforce_secure_chat = config.server.online_mode;
    let world_config = &config.server.world;
    let mut position = session.player.position;
    let mut survival = SurvivalState::from_stored(saved_player.survival, world_config.game_mode);
    let mut pending_dig: Option<mining::PendingDig> = None;
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
                ).await? {
                    pending_dig = None;
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
                    sink.flush().await?;
                    continue;
                }

                if packet_id == UseItemOn::ID {
                    let use_item_on = crate::connection::decode_payload::<UseItemOn>(&mut payload)?;
                    sink.send(
                        crate::inventory::acknowledge_block_change(use_item_on.sequence.clone())
                            .packet(),
                    )
                    .await?;
                    if survival.is_dead() {
                        sink.flush().await?;
                        continue;
                    }
                    pending_dig = None;
                    if use_item_on.hand.0 == 0 {
                        if let Some(block_state) = crate::inventory::placed_block_state_for_item(inventory.held_item()) {
                            if place_held_block(
                                sink,
                                world,
                                players,
                                world_config,
                                &play_dimension,
                                profile.uuid,
                                &position,
                                &use_item_on,
                                block_state,
                            )
                            .await?
                            {
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
                    sink.flush().await?;
                    continue;
                }

                if packet_id == PlayerAction::ID {
                    let action = crate::connection::decode_payload::<PlayerAction>(&mut payload)?;
                    sink.send(crate::inventory::acknowledge_block_change(action.sequence).packet()).await?;
                    if survival.is_dead() {
                        pending_dig = None;
                        sink.flush().await?;
                        continue;
                    }
                    match action.status.0 {
                        PLAYER_ACTION_START_DESTROY_BLOCK => {
                            if should_destroy_block(world_config.game_mode, PLAYER_ACTION_START_DESTROY_BLOCK) {
                                destroy_block(
                                    sink,
                                    world,
                                    players,
                                    entities,
                                    plugins,
                                    world_config,
                                    &play_dimension,
                                    profile.uuid,
                                    inventory.held_item(),
                                    action.location,
                                )
                                .await?;
                                sink.flush().await?;
                                continue;
                            }
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
                            sink.flush().await?;
                        }
                        PLAYER_ACTION_CANCEL_DESTROY_BLOCK => {
                            pending_dig = None;
                            sink.flush().await?;
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
                            if !can_destroy {
                                sink.flush().await?;
                                continue;
                            }
                            destroy_block(
                                sink,
                                world,
                                players,
                                entities,
                                plugins,
                                world_config,
                                &play_dimension,
                                profile.uuid,
                                inventory.held_item(),
                                action.location,
                            )
                            .await?;
                            survival.apply_exhaustion(MINING_EXHAUSTION_PER_BLOCK);
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
                            sink.flush().await?;
                        }
                        _ => {
                            sink.flush().await?;
                        }
                    }
                    continue;
                }

                if packet_id == UseItem::ID {
                    let use_item = crate::connection::decode_payload::<UseItem>(&mut payload)?;
                    sink.send(crate::inventory::acknowledge_block_change(use_item.sequence).packet()).await?;
                    sink.flush().await?;
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
                    )
                    .await?;
                    if survival.is_dead() {
                        pending_dig = None;
                    }
                    chunk_state
                        .update_center(sink, &chunk_sender, world, movement.x, movement.z)
                        .await?;
                    players.update_position(profile.uuid, position);
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
                    )
                    .await?;
                    if survival.is_dead() {
                        pending_dig = None;
                    }
                    chunk_state
                        .update_center(sink, &chunk_sender, world, movement.x, movement.z)
                        .await?;
                    players.update_position(profile.uuid, position);
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
                    )
                    .await?;
                    if survival.is_dead() {
                        pending_dig = None;
                    }
                    players.update_position(profile.uuid, position);
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
                    )
                    .await?;
                    if survival.is_dead() {
                        pending_dig = None;
                    }
                    players.update_position(profile.uuid, position);
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
                    handle_chat_command(sink, config, players, permissions, profile, &command.command).await?;
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
            }
        }
    }
    .await;

    saved_player.update_runtime(&play_dimension, position, &inventory, survival.to_stored());
    if let Err(err) = player_data.save(saved_player).await {
        log::warn!(
            "failed to save player data: uuid={}, error={err:#}",
            profile.uuid
        );
    }

    result
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
        let Some(drop) = entities.drop_item(players, actor, dimension, position, item)? else {
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
    players.broadcast_block_changed(actor, landing_position, dirt, None);
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
    sink.send(Respawn {
        dimension_type: VarInt(dimension_type_holder_id(&world_config.dimension_type)),
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
    send_respawn_player_state(sink, world_config, dimension, *position).await?;
    sink.send(survival.health_packet()).await?;
    chunk_state
        .reset_after_respawn(sink, chunk_sender, world, plugins, position.x, position.z)
        .await?;
    players.update_position(actor, *position);
    sink.flush().await?;
    Ok(())
}

async fn place_held_block<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    world: &WorldManager,
    players: &PlayerManager,
    world_config: &qexed_config::app::qexed::server::World,
    dimension: &str,
    actor: uuid::Uuid,
    player_position: &EntityPosition,
    use_item_on: &UseItemOn,
    block_state: i32,
) -> Result<bool>
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
        return Ok(false);
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
            return Ok(false);
        }

        let placed_lower = apply_block_change(
            sink,
            world,
            players,
            world_config,
            dimension,
            actor,
            target.clone(),
            block_state,
        )
        .await?;
        if !placed_lower {
            return Ok(false);
        }
        apply_block_change(
            sink,
            world,
            players,
            world_config,
            dimension,
            actor,
            upper,
            upper_state,
        )
        .await?;
        return Ok(true);
    }

    apply_block_change(
        sink,
        world,
        players,
        world_config,
        dimension,
        actor,
        target,
        block_state,
    )
    .await
}

async fn destroy_block<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    world: &WorldManager,
    players: &PlayerManager,
    entities: &crate::entities::EntityManager,
    plugins: &crate::plugins::PluginManager,
    world_config: &qexed_config::app::qexed::server::World,
    dimension: &str,
    actor: uuid::Uuid,
    held_item: &qexed_protocol::types::Slot,
    position: BlockPosition,
) -> Result<bool>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let current = world
        .block_state_at(dimension, &position)
        .unwrap_or_else(crate::inventory::air_block_state);
    if crate::inventory::is_air_block_state(current) {
        send_block_rollback(sink, world, dimension, position).await?;
        return Ok(false);
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
        )
        .await?;
    }
    Ok(destroyed)
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
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    if game_mode != GameMode::Survival {
        return Ok(());
    }

    let drop_position = mining::drop_position(position);
    for item in mining::default_block_drops(block_state, position, held_item, plugins) {
        let Some(drop) = entities.drop_item(players, actor, dimension, drop_position, item)? else {
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

async fn apply_block_change<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    world: &WorldManager,
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
    if !can_modify_world(world_config, &position) {
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
    let light_update = if world.dynamic_light_enabled() {
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
    players.broadcast_block_changed(actor, position, block_state, light_update);
    Ok(true)
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

#[cfg(test)]
mod tests;

use anyhow::Result;
use qexed_packet::net_types::VarInt;
use qexed_protocol::to_client::play::{
    change_difficulty::ChangeDifficulty,
    game_state_change::GameStateChange,
    initialize_border::InitializeBorder,
    player_abilities::PlayerAbilities,
    player_info_update::{PlayerInfoActions, PlayerInfoEntry, PlayerInfoUpdate},
    server_data::ServerData,
    set_default_spawn_position::SetDefaultSpawnPosition,
    set_experience::SetExperience,
    set_health::SetHealth,
    set_held_slot::SetHeldSlot,
    set_simulation_distance::SetSimulationDistance,
    set_time::SetTime,
    ticking_state::TickingState,
    update_view_distance::UpdateViewDistance,
    update_view_position::UpdateViewPosition,
};

use crate::players::{OnlinePlayer, PlayerManager};

use super::util::{chunk_coord, favicon_bytes, player_ability_flags, text_component};

pub(super) async fn send_initial_player_state<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    config: &qexed_config::app::qexed::Qexed,
    world_config: &qexed_config::app::qexed::server::World,
    world_rules: &crate::world::WorldRulesManager,
    play_dimension: &str,
    player: &OnlinePlayer,
    inventory: &crate::inventory::PlayerInventory,
    survival: crate::player_data::StoredSurvival,
    permissions: &crate::permissions::PermissionManager,
    plugins: &crate::plugins::PluginManager,
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
    super::recipes::send_initial_recipe_book(sink).await?;

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
    let lobby = super::lobby::LobbyRuntime::new(&config.server.lobby);
    let lobby_server_ids = lobby.server_targets();
    let mut plugin_commands = Vec::new();
    for command in plugins.plugin_commands() {
        if permissions
            .can_run_command(&player.profile, &command.name)
            .await?
        {
            plugin_commands.push(command.name);
        }
    }
    let command_tree = crate::commands::command_tree_for_lobby_with_extra(
        &visible_commands,
        &lobby_server_ids,
        &plugin_commands,
    );
    sink.send(command_tree).await?;

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

    sink.send(GameStateChange {
        reason: 13,
        game_mode: 0.0,
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

pub(super) async fn send_respawn_player_state<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    world_config: &qexed_config::app::qexed::server::World,
    world_rules: &crate::world::WorldRulesManager,
    play_dimension: &str,
    position: qexed_protocol::to_client::play::add_entity::EntityPosition,
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
    sink.send(GameStateChange {
        reason: 13,
        game_mode: 0.0,
    })
    .await?;
    sink.send(TickingState::default()).await?;
    sink.send(SetSimulationDistance {
        simulation_distance: VarInt(simulation_distance),
    })
    .await?;
    sink.send(UpdateViewDistance {
        view_distance: VarInt(view_distance),
    })
    .await?;
    sink.send(UpdateViewPosition {
        chunk_x: VarInt(chunk_coord(position.x)),
        chunk_z: VarInt(chunk_coord(position.z)),
    })
    .await?;

    Ok(())
}

pub(super) async fn send_existing_players<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    players: &PlayerManager,
    profile_id: uuid::Uuid,
    player_entity_type: i32,
    play_dimension: &str,
    viewer_position: qexed_protocol::to_client::play::add_entity::EntityPosition,
    render_distance: f64,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    for player in players.list_except(profile_id) {
        if !within_render_distance(viewer_position, player.position, render_distance) {
            continue;
        }
        let event = crate::players::PlayerEvent::Joined(player);
        for packet in event.packets(player_entity_type, play_dimension)? {
            sink.send_raw(packet).await?;
        }
    }
    Ok(())
}

pub(super) async fn send_existing_entities<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    entities: &crate::entities::EntityManager,
    dimension: &str,
    viewer_position: qexed_protocol::to_client::play::add_entity::EntityPosition,
    rendering: &qexed_config::app::qexed::server::EntityRendering,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    for packet in entities.spawn_packets_for_view(dimension, viewer_position, rendering)? {
        sink.send_raw(packet).await?;
    }
    Ok(())
}

fn within_render_distance(
    left: qexed_protocol::to_client::play::add_entity::EntityPosition,
    right: qexed_protocol::to_client::play::add_entity::EntityPosition,
    distance: f64,
) -> bool {
    if distance <= 0.0 {
        return false;
    }
    let dx = left.x - right.x;
    let dz = left.z - right.z;
    (dx * dx + dz * dz) <= distance * distance
}

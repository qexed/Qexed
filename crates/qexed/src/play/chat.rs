use anyhow::Result;
use qexed_packet::net_types::VarInt;
use qexed_protocol::to_client::play::{
    add_entity::EntityPosition,
    custom_payload::CustomPayload,
    position::Position,
    respawn::{KEEP_NO_DATA, Respawn},
    system_chat::SystemChat,
    transfer::Transfer,
};

use qexed_config::app::qexed::server::{ForwardingMode, Server};

use crate::players::PlayerManager;

use super::util::{text_component, translatable_component};

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(super) struct CommandOutcome {
    pub teleported: bool,
    pub opened_lobby_menu: bool,
    pub opened_config_menu: bool,
    pub opened_config_menu_id: Option<String>,
}

pub(super) async fn handle_chat_command<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    config: &crate::config::RuntimeConfig,
    world: &crate::world::WorldManager,
    world_rules: &crate::world::WorldRulesManager,
    players: &PlayerManager,
    entities: &crate::entities::EntityManager,
    permissions: &crate::permissions::PermissionManager,
    plugins: &crate::plugins::PluginManager,
    profile: &qexed_packet::net_types::GameProfile,
    actor_entity_id: i32,
    command: &str,
    lobby: &super::lobby::LobbyRuntime,
    lobby_status: &mut super::lobby::LobbyStatusSnapshot,
    menus: &super::menus::MenuRuntime,
    active_config_menu: &mut Option<String>,
    players_hidden: &mut bool,
    visible_player_entities: &mut std::collections::HashSet<uuid::Uuid>,
    viewer_position: EntityPosition,
    render_distance: f64,
    chunk_sender: &tokio::sync::mpsc::UnboundedSender<super::chunks::ChunkLoadResult>,
    chunk_state: &mut super::ChunkSendState,
    position: &mut qexed_protocol::to_client::play::add_entity::EntityPosition,
    next_teleport_id: &mut i32,
    play_dimension: &mut String,
) -> Result<CommandOutcome>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let command = command.trim();
    if !permissions.can_run_command(profile, command).await? {
        sink.send(SystemChat {
            content: text_component(permissions.denied_message().to_string()),
            overlay: false,
        })
        .await?;
        return Ok(CommandOutcome::default());
    }

    let mut parts = command.split_whitespace();
    let raw_name = parts.next().unwrap_or_default();
    let name = crate::commands::normalize_command_name(raw_name);
    let argument = parts.collect::<Vec<_>>().join(" ");
    match name.as_str() {
        "help" => {
            let locale = player_locale(players, profile, &config.language);
            send_help_messages(sink, permissions, plugins, profile, &locale).await?;
            Ok(CommandOutcome::default())
        }
        "list" => {
            let mut names = players.online_names();
            names.sort();
            let locale = player_locale(players, profile, &config.language);
            sink.send(SystemChat {
                content: text_component(crate::commands::localized_list(
                    &locale,
                    players.online_count(),
                    config.server.max_player,
                    &names,
                )),
                overlay: false,
            })
            .await?;
            Ok(CommandOutcome::default())
        }
        "lobby" | "server" if !lobby.enabled() => {
            sink.send(SystemChat {
                content: translatable_component("commands.help.failed", Vec::new()),
                overlay: false,
            })
            .await?;
            Ok(CommandOutcome::default())
        }
        "lobby" if argument.trim() == "status" => {
            let labels = lobby.server_labels(lobby_status);
            let servers = if labels.is_empty() {
                "none".to_string()
            } else {
                labels.join(", ")
            };
            sink.send(SystemChat {
                content: translatable_component(
                    "commands.datapack.list.enabled.success",
                    vec![
                        text_component(labels.len().to_string()),
                        text_component(servers),
                    ],
                ),
                overlay: false,
            })
            .await?;
            Ok(CommandOutcome::default())
        }
        "lobby" if argument.trim() == "refresh" => {
            *lobby_status = lobby.refresh_status().await;
            lobby.update_boss_bar_status(sink, lobby_status).await?;
            let labels = lobby.server_labels(lobby_status);
            let servers = if labels.is_empty() {
                "none".to_string()
            } else {
                labels.join(", ")
            };
            sink.send(SystemChat {
                content: translatable_component(
                    "commands.datapack.list.enabled.success",
                    vec![
                        text_component(labels.len().to_string()),
                        text_component(servers),
                    ],
                ),
                overlay: false,
            })
            .await?;
            Ok(CommandOutcome::default())
        }
        "lobby" => {
            lobby.open_menu(sink, lobby_status).await?;
            sink.send(SystemChat {
                content: translatable_component(
                    "commands.trigger.simple.success",
                    vec![text_component("lobby")],
                ),
                overlay: false,
            })
            .await?;
            Ok(CommandOutcome {
                opened_lobby_menu: true,
                ..CommandOutcome::default()
            })
        }
        "server" if argument.trim().is_empty() => {
            let entries = lobby.server_command_entries(lobby_status);
            let servers = if entries.is_empty() {
                "none".to_string()
            } else {
                entries.join(", ")
            };
            sink.send(SystemChat {
                content: translatable_component(
                    "commands.datapack.list.available.success",
                    vec![
                        text_component(entries.len().to_string()),
                        text_component(servers),
                    ],
                ),
                overlay: false,
            })
            .await?;
            Ok(CommandOutcome::default())
        }
        "server" => {
            let Some(server_id) = lobby.resolve_server_id(&argument) else {
                sink.send(SystemChat {
                    content: translatable_component(
                        "commands.datapack.unknown",
                        vec![text_component(argument.clone())],
                    ),
                    overlay: false,
                })
                .await?;
                return Ok(CommandOutcome::default());
            };
            let proxy_context = super::lobby::ProxyConnectContext {
                server_config: &config.server,
                plugins,
                players,
                actor: profile.uuid,
            };
            lobby
                .transfer_to_server(sink, &server_id, lobby_status, Some(&proxy_context))
                .await?;
            Ok(CommandOutcome::default())
        }
        "spawn" => {
            super::teleport_to_spawn(
                sink,
                world,
                world_rules,
                players,
                plugins,
                &config.world,
                play_dimension,
                profile.uuid,
                chunk_sender,
                chunk_state,
                position,
                next_teleport_id,
            )
            .await?;
            sink.send(SystemChat {
                content: translatable_component(
                    "commands.teleport.success.location.single",
                    vec![
                        text_component(profile.username.clone()),
                        text_component(config.world.spawn.x.floor().to_string()),
                        text_component(config.world.spawn.y.floor().to_string()),
                        text_component(config.world.spawn.z.floor().to_string()),
                    ],
                ),
                overlay: false,
            })
            .await?;
            Ok(CommandOutcome {
                teleported: true,
                ..CommandOutcome::default()
            })
        }
        "teleport" => {
            let teleported = handle_teleport_command(
                sink,
                world,
                world_rules,
                &config.world,
                players,
                plugins,
                profile,
                actor_entity_id,
                chunk_sender,
                chunk_state,
                position,
                next_teleport_id,
                play_dimension,
                argument.as_str(),
            )
            .await?;
            Ok(CommandOutcome {
                teleported,
                ..CommandOutcome::default()
            })
        }
        "time" => {
            handle_time_command(sink, world_rules, argument.as_str()).await?;
            Ok(CommandOutcome::default())
        }
        "gamerule" => {
            handle_gamerule_command(sink, world_rules, argument.as_str()).await?;
            Ok(CommandOutcome::default())
        }
        "scoreboard" => {
            handle_scoreboard_command(sink, &config.server.scoreboard, argument.as_str()).await?;
            Ok(CommandOutcome::default())
        }
        "entity" => {
            handle_entity_command(
                sink,
                players,
                entities,
                &config.server.entity_rendering,
                play_dimension,
                *position,
                argument.as_str(),
            )
            .await?;
            Ok(CommandOutcome::default())
        }
        "npc" => {
            handle_npc_command(
                sink,
                players,
                entities,
                &config.server.entity_rendering,
                play_dimension,
                *position,
                argument.as_str(),
            )
            .await?;
            Ok(CommandOutcome::default())
        }
        "structure" => {
            handle_structure_command(
                sink,
                world,
                world_rules,
                players,
                &config.world,
                play_dimension,
                profile.uuid,
                *position,
                argument.as_str(),
            )
            .await?;
            Ok(CommandOutcome::default())
        }
        _ => {
            let plugin_response = plugins.execute_command(
                &crate::players::OnlinePlayer {
                    profile: profile.clone(),
                    entity_id: actor_entity_id,
                    position: *position,
                    dimension: play_dimension.to_string(),
                    equipment: Vec::new(),
                    language: players
                        .player_by_uuid(profile.uuid)
                        .map(|player| player.language)
                        .unwrap_or_else(|| config.language.clone()),
                    displayed_skin_parts: players
                        .player_by_uuid(profile.uuid)
                        .map(|player| player.displayed_skin_parts)
                        .unwrap_or(crate::players::DEFAULT_DISPLAYED_SKIN_PARTS),
                },
                &name,
                argument.as_str(),
            );
            if plugin_response.handled || !plugin_response.actions.is_empty() {
                let mut teleported = false;
                for action in plugin_response.actions {
                    teleported |= apply_plugin_action(
                        sink,
                        Some(&config.server),
                        world,
                        world_rules,
                        &config.world,
                        players,
                        plugins,
                        profile.uuid,
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
                return Ok(CommandOutcome {
                    teleported,
                    opened_config_menu: active_config_menu.is_some(),
                    opened_config_menu_id: active_config_menu.clone(),
                    ..CommandOutcome::default()
                });
            }
            if crate::commands::is_known_vanilla_command(&name) {
                let locale = player_locale(players, profile, &config.language);
                send_unimplemented_command(sink, &locale, &name).await?;
                return Ok(CommandOutcome::default());
            }
            send_unknown_or_incomplete_command(sink, command.to_string()).await?;
            Ok(CommandOutcome::default())
        }
    }
}

async fn handle_teleport_command<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    world: &crate::world::WorldManager,
    world_rules: &crate::world::WorldRulesManager,
    world_config: &qexed_config::app::qexed::server::World,
    players: &PlayerManager,
    plugins: &crate::plugins::PluginManager,
    profile: &qexed_packet::net_types::GameProfile,
    actor_entity_id: i32,
    chunk_sender: &tokio::sync::mpsc::UnboundedSender<super::chunks::ChunkLoadResult>,
    chunk_state: &mut super::ChunkSendState,
    position: &mut EntityPosition,
    next_teleport_id: &mut i32,
    play_dimension: &mut String,
    argument: &str,
) -> Result<bool>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let parts = argument.split_whitespace().collect::<Vec<_>>();
    if parts.is_empty() {
        send_unknown_or_incomplete_command(sink, "tp".to_string()).await?;
        return Ok(false);
    }

    let source = players.player_by_uuid(profile.uuid).unwrap_or_else(|| {
        command_source_player(
            profile,
            actor_entity_id,
            *position,
            play_dimension,
            String::new(),
        )
    });
    let mut executor_teleported = false;

    let parsed = parse_teleport_command(parts.as_slice(), players, &source, *position);
    match parsed {
        TeleportParse::ToPlayer(destination) => {
            teleport_command_target(
                sink,
                world,
                world_rules,
                world_config,
                players,
                plugins,
                source.clone(),
                destination.dimension.clone(),
                destination.position,
                Some(destination.profile.username.clone()),
                profile.uuid,
                chunk_sender,
                chunk_state,
                position,
                next_teleport_id,
                play_dimension,
                &mut executor_teleported,
            )
            .await?;
        }
        TeleportParse::TargetToPlayer {
            target,
            destination,
        } => {
            teleport_command_target(
                sink,
                world,
                world_rules,
                world_config,
                players,
                plugins,
                target,
                destination.dimension.clone(),
                destination.position,
                Some(destination.profile.username.clone()),
                profile.uuid,
                chunk_sender,
                chunk_state,
                position,
                next_teleport_id,
                play_dimension,
                &mut executor_teleported,
            )
            .await?;
        }
        TeleportParse::TargetToLocation {
            target,
            dimension,
            position: destination,
        } => {
            teleport_command_target(
                sink,
                world,
                world_rules,
                world_config,
                players,
                plugins,
                target,
                dimension,
                destination,
                None,
                profile.uuid,
                chunk_sender,
                chunk_state,
                position,
                next_teleport_id,
                play_dimension,
                &mut executor_teleported,
            )
            .await?;
        }
        TeleportParse::UnknownPlayer(name) => {
            send_unknown_player(sink, &name).await?;
            return Ok(false);
        }
        TeleportParse::Invalid => {
            send_unknown_or_incomplete_command(sink, format!("tp {}", argument.trim())).await?;
            return Ok(false);
        }
    }

    Ok(executor_teleported)
}

#[allow(clippy::too_many_arguments)]
async fn teleport_command_target<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    world: &crate::world::WorldManager,
    world_rules: &crate::world::WorldRulesManager,
    world_config: &qexed_config::app::qexed::server::World,
    players: &PlayerManager,
    plugins: &crate::plugins::PluginManager,
    target: crate::players::OnlinePlayer,
    destination_dimension: String,
    destination: EntityPosition,
    destination_name: Option<String>,
    executor: uuid::Uuid,
    chunk_sender: &tokio::sync::mpsc::UnboundedSender<super::chunks::ChunkLoadResult>,
    chunk_state: &mut super::ChunkSendState,
    position: &mut EntityPosition,
    next_teleport_id: &mut i32,
    play_dimension: &mut String,
    executor_teleported: &mut bool,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    world_rules.ensure_loaded(&destination_dimension)?;
    if target.profile.uuid == executor {
        *executor_teleported |= teleport_current_player(
            sink,
            world,
            world_rules,
            world_config,
            players,
            plugins,
            executor,
            chunk_sender,
            chunk_state,
            position,
            next_teleport_id,
            play_dimension,
            &destination_dimension,
            destination,
        )
        .await?;
    } else if !players.teleport_player(
        target.profile.uuid,
        destination_dimension.clone(),
        destination,
    ) {
        send_unknown_player(sink, &target.profile.username).await?;
        return Ok(());
    }

    if let Some(destination_name) = destination_name {
        send_translatable(
            sink,
            "commands.teleport.success.entity.single",
            vec![
                text_component(target.profile.username),
                text_component(destination_name),
            ],
        )
        .await
    } else {
        send_translatable(
            sink,
            "commands.teleport.success.location.single",
            vec![
                text_component(target.profile.username),
                text_component(format_teleport_coord(destination.x)),
                text_component(format_teleport_coord(destination.y)),
                text_component(format_teleport_coord(destination.z)),
            ],
        )
        .await
    }
}

#[allow(clippy::too_many_arguments)]
async fn teleport_current_player<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    world: &crate::world::WorldManager,
    world_rules: &crate::world::WorldRulesManager,
    world_config: &qexed_config::app::qexed::server::World,
    players: &PlayerManager,
    plugins: &crate::plugins::PluginManager,
    actor: uuid::Uuid,
    chunk_sender: &tokio::sync::mpsc::UnboundedSender<super::chunks::ChunkLoadResult>,
    chunk_state: &mut super::ChunkSendState,
    position: &mut EntityPosition,
    next_teleport_id: &mut i32,
    play_dimension: &mut String,
    destination_dimension: &str,
    destination: EntityPosition,
) -> Result<bool>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    world_rules.ensure_loaded(destination_dimension)?;
    let changed_dimension = play_dimension != destination_dimension;
    *position = destination;

    if changed_dimension {
        let dimension_rule = world_rules.snapshot(destination_dimension);
        sink.send(Respawn {
            dimension_type: VarInt(super::util::dimension_type_holder_id(
                &dimension_rule.dimension_type,
            )),
            dimension_name: destination_dimension.to_string(),
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
        *play_dimension = destination_dimension.to_string();
    }

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

    if changed_dimension {
        super::send_respawn_player_state(
            sink,
            world_config,
            world_rules,
            destination_dimension,
            *position,
        )
        .await?;
        chunk_state
            .reset_dimension_after_respawn(
                sink,
                chunk_sender,
                world,
                plugins,
                destination_dimension.to_string(),
                position.x,
                position.z,
            )
            .await?;
        players.update_position_and_dimension(actor, destination_dimension.to_string(), *position);
    } else {
        chunk_state
            .reset_after_respawn(sink, chunk_sender, world, plugins, position.x, position.z)
            .await?;
        players.update_position(actor, *position);
    }

    Ok(true)
}

fn command_source_player(
    profile: &qexed_packet::net_types::GameProfile,
    actor_entity_id: i32,
    position: EntityPosition,
    dimension: &str,
    language: String,
) -> crate::players::OnlinePlayer {
    crate::players::OnlinePlayer {
        profile: profile.clone(),
        entity_id: actor_entity_id,
        position,
        dimension: dimension.to_string(),
        equipment: Vec::new(),
        language,
        displayed_skin_parts: crate::players::DEFAULT_DISPLAYED_SKIN_PARTS,
    }
}

fn resolve_single_player(
    players: &PlayerManager,
    source: &crate::players::OnlinePlayer,
    value: &str,
) -> Option<crate::players::OnlinePlayer> {
    match value {
        "@s" | "@p" => Some(source.clone()),
        name => players.player_by_name(name),
    }
}

enum TeleportParse {
    ToPlayer(crate::players::OnlinePlayer),
    TargetToPlayer {
        target: crate::players::OnlinePlayer,
        destination: crate::players::OnlinePlayer,
    },
    TargetToLocation {
        target: crate::players::OnlinePlayer,
        dimension: String,
        position: EntityPosition,
    },
    UnknownPlayer(String),
    Invalid,
}

fn parse_teleport_command(
    parts: &[&str],
    players: &PlayerManager,
    source: &crate::players::OnlinePlayer,
    current_position: EntityPosition,
) -> TeleportParse {
    if parts.is_empty() {
        return TeleportParse::Invalid;
    }

    if parts.len() == 1 {
        return resolve_single_player(players, source, parts[0])
            .map(TeleportParse::ToPlayer)
            .unwrap_or_else(|| TeleportParse::UnknownPlayer(parts[0].to_string()));
    }

    if is_dimension_id(parts[0]) {
        return match parts.len() {
            4 | 6 => parse_teleport_location(parts, 0, source, source, current_position),
            _ => TeleportParse::Invalid,
        };
    }

    if let Some(target) = resolve_single_player(players, source, parts[0]) {
        if parts.len() == 2 {
            let Some(destination) = resolve_single_player(players, source, parts[1]) else {
                return TeleportParse::UnknownPlayer(parts[1].to_string());
            };
            return TeleportParse::TargetToPlayer {
                target,
                destination,
            };
        }

        return match parts.len() {
            4 | 6 => parse_teleport_location(parts, 1, source, &target, target.position),
            5 | 7 if parts.get(1).is_some_and(|value| is_dimension_id(value)) => {
                parse_teleport_location(parts, 1, source, &target, target.position)
            }
            _ => TeleportParse::Invalid,
        };
    }

    match parts.len() {
        3 | 5 => parse_teleport_location(parts, 0, source, source, current_position),
        _ => TeleportParse::UnknownPlayer(parts[0].to_string()),
    }
}

fn parse_teleport_location(
    parts: &[&str],
    start: usize,
    source: &crate::players::OnlinePlayer,
    target: &crate::players::OnlinePlayer,
    base: EntityPosition,
) -> TeleportParse {
    let (dimension, offset) = if parts.get(start).is_some_and(|value| is_dimension_id(value)) {
        (parts[start].to_string(), start + 1)
    } else {
        (target.dimension.clone(), start)
    };
    let Some(position) = parse_teleport_position(parts, offset, base) else {
        return TeleportParse::Invalid;
    };
    TeleportParse::TargetToLocation {
        target: if start == 0 {
            source.clone()
        } else {
            target.clone()
        },
        dimension,
        position,
    }
}

fn is_dimension_id(value: &str) -> bool {
    matches!(
        value,
        "minecraft:overworld" | "minecraft:the_nether" | "minecraft:the_end"
    )
}

fn parse_teleport_position(
    parts: &[&str],
    offset: usize,
    base: EntityPosition,
) -> Option<EntityPosition> {
    let x = parse_command_coord_f64(parts.get(offset).copied()?, base.x)?;
    let y = parse_command_coord_f64(parts.get(offset + 1).copied()?, base.y)?;
    let z = parse_command_coord_f64(parts.get(offset + 2).copied()?, base.z)?;
    let (yaw, pitch) = if parts.len() == offset + 5 {
        (
            parse_command_angle(parts.get(offset + 3).copied()?, base.yaw)?,
            parse_command_angle(parts.get(offset + 4).copied()?, base.pitch)?,
        )
    } else if parts.len() == offset + 3 {
        (base.yaw, base.pitch)
    } else {
        return None;
    };
    Some(EntityPosition {
        x,
        y,
        z,
        yaw,
        pitch,
        on_ground: false,
    })
}

fn parse_command_coord_f64(value: &str, base: f64) -> Option<f64> {
    if value == "~" {
        return Some(base);
    }
    if let Some(offset) = value.strip_prefix('~') {
        let offset = if offset.is_empty() {
            0.0
        } else {
            offset.parse::<f64>().ok()?
        };
        return Some(base + offset);
    }
    value.parse::<f64>().ok().filter(|value| value.is_finite())
}

fn parse_command_angle(value: &str, base: f32) -> Option<f32> {
    parse_command_coord_f64(value, f64::from(base)).map(|value| value as f32)
}

fn format_teleport_coord(value: f64) -> String {
    if (value.fract()).abs() < f64::EPSILON {
        format!("{value:.0}")
    } else {
        value.to_string()
    }
}

async fn send_unknown_player<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    player: &str,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    send_translatable(
        sink,
        "argument.player.unknown",
        vec![text_component(player.to_string())],
    )
    .await
}

async fn handle_time_command<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    world_rules: &crate::world::WorldRulesManager,
    argument: &str,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let mut parts = argument.split_whitespace();
    let Some(action) = parts.next() else {
        send_unknown_or_incomplete_command(sink, "time".to_string()).await?;
        return Ok(());
    };

    match action {
        "set" => {
            let Some((dimension, value)) = parse_dimension_with_value(parts) else {
                send_unknown_or_incomplete_command(
                    sink,
                    "time set <dimension> <value>".to_string(),
                )
                .await?;
                return Ok(());
            };
            let Some(value) = parse_time_value(value) else {
                send_unknown_or_incomplete_command(sink, format!("time set {dimension} <value>"))
                    .await?;
                return Ok(());
            };
            world_rules.set_time_value(dimension, value)?;
            send_translatable(
                sink,
                "commands.time.set",
                vec![text_component(value.to_string())],
            )
            .await?;
        }
        "add" => {
            let Some((dimension, value)) = parse_dimension_with_value(parts) else {
                send_unknown_or_incomplete_command(
                    sink,
                    "time add <dimension> <value>".to_string(),
                )
                .await?;
                return Ok(());
            };
            let Some(delta) = parse_time_value(value) else {
                send_unknown_or_incomplete_command(sink, format!("time add {dimension} <value>"))
                    .await?;
                return Ok(());
            };
            let snapshot = world_rules.add_time_value(dimension, delta)?;
            send_translatable(
                sink,
                "commands.time.set",
                vec![text_component(snapshot.time_value.to_string())],
            )
            .await?;
        }
        "query" => {
            let Some(dimension) = parts.next() else {
                send_unknown_or_incomplete_command(sink, "time query <dimension>".to_string())
                    .await?;
                return Ok(());
            };
            if parts.next().is_some() {
                send_unknown_or_incomplete_command(sink, "time query <dimension>".to_string())
                    .await?;
                return Ok(());
            }
            let value = world_rules.current_time(dimension);
            send_translatable(
                sink,
                "commands.time.query",
                vec![text_component(value.to_string())],
            )
            .await?;
        }
        _ => {
            send_unknown_or_incomplete_command(sink, format!("time {argument}")).await?;
        }
    }

    Ok(())
}

async fn handle_gamerule_command<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    world_rules: &crate::world::WorldRulesManager,
    argument: &str,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let mut parts = argument.split_whitespace();
    let Some(dimension) = parts.next() else {
        send_unknown_or_incomplete_command(sink, "gamerule <dimension> <rule> [value]".to_string())
            .await?;
        return Ok(());
    };
    let Some(rule_name) = parts.next() else {
        send_unknown_or_incomplete_command(sink, format!("gamerule {dimension} <rule> [value]"))
            .await?;
        return Ok(());
    };
    let value = parts.next();
    if parts.next().is_some() {
        send_unknown_or_incomplete_command(
            sink,
            format!("gamerule {dimension} {rule_name} [value]"),
        )
        .await?;
        return Ok(());
    }

    match rule_name {
        "doDaylightCycle" => {
            if let Some(value) = value {
                let Some(enabled) = parse_bool_arg(value) else {
                    send_unknown_or_incomplete_command(
                        sink,
                        format!("gamerule {dimension} doDaylightCycle <true|false>"),
                    )
                    .await?;
                    return Ok(());
                };
                let snapshot = world_rules.set_daylight_cycle(dimension, enabled)?;
                send_translatable(
                    sink,
                    "commands.gamerule.set",
                    vec![
                        text_component(rule_name.to_string()),
                        text_component(snapshot.daylight_cycle.to_string()),
                    ],
                )
                .await?;
            } else {
                let snapshot = world_rules.snapshot(dimension);
                send_translatable(
                    sink,
                    "commands.gamerule.query",
                    vec![
                        text_component(rule_name.to_string()),
                        text_component(snapshot.daylight_cycle.to_string()),
                    ],
                )
                .await?;
            }
        }
        "doBlockUpdates" => {
            if let Some(value) = value {
                let Some(enabled) = parse_bool_arg(value) else {
                    send_unknown_or_incomplete_command(
                        sink,
                        format!("gamerule {dimension} doBlockUpdates <true|false>"),
                    )
                    .await?;
                    return Ok(());
                };
                let snapshot = world_rules.set_block_updates(dimension, enabled)?;
                send_translatable(
                    sink,
                    "commands.gamerule.set",
                    vec![
                        text_component(rule_name.to_string()),
                        text_component(snapshot.block_updates.to_string()),
                    ],
                )
                .await?;
            } else {
                let snapshot = world_rules.snapshot(dimension);
                send_translatable(
                    sink,
                    "commands.gamerule.query",
                    vec![
                        text_component(rule_name.to_string()),
                        text_component(snapshot.block_updates.to_string()),
                    ],
                )
                .await?;
            }
        }
        "doWorldReadOnly" => {
            if let Some(value) = value {
                let Some(read_only) = parse_bool_arg(value) else {
                    send_unknown_or_incomplete_command(
                        sink,
                        format!("gamerule {dimension} doWorldReadOnly <true|false>"),
                    )
                    .await?;
                    return Ok(());
                };
                let snapshot = world_rules.set_read_only(dimension, read_only)?;
                send_translatable(
                    sink,
                    "commands.gamerule.set",
                    vec![
                        text_component(rule_name.to_string()),
                        text_component(snapshot.read_only.to_string()),
                    ],
                )
                .await?;
            } else {
                let snapshot = world_rules.snapshot(dimension);
                send_translatable(
                    sink,
                    "commands.gamerule.query",
                    vec![
                        text_component(rule_name.to_string()),
                        text_component(snapshot.read_only.to_string()),
                    ],
                )
                .await?;
            }
        }
        "timeTickStep" => {
            if let Some(value) = value {
                let Ok(step) = value.parse::<i64>() else {
                    send_unknown_or_incomplete_command(
                        sink,
                        format!("gamerule {dimension} timeTickStep <int>"),
                    )
                    .await?;
                    return Ok(());
                };
                let snapshot = world_rules.set_tick_step(dimension, step)?;
                send_translatable(
                    sink,
                    "commands.gamerule.set",
                    vec![
                        text_component(rule_name.to_string()),
                        text_component(snapshot.tick_step.to_string()),
                    ],
                )
                .await?;
            } else {
                let snapshot = world_rules.snapshot(dimension);
                send_translatable(
                    sink,
                    "commands.gamerule.query",
                    vec![
                        text_component(rule_name.to_string()),
                        text_component(snapshot.tick_step.to_string()),
                    ],
                )
                .await?;
            }
        }
        "fixedTime" => {
            if let Some(value) = value {
                let fixed = if value.eq_ignore_ascii_case("none") {
                    None
                } else {
                    let Ok(parsed) = value.parse::<i64>() else {
                        send_unknown_or_incomplete_command(
                            sink,
                            format!("gamerule {dimension} fixedTime <int|none>"),
                        )
                        .await?;
                        return Ok(());
                    };
                    Some(parsed)
                };
                let snapshot = world_rules.set_fixed_time(dimension, fixed)?;
                let label = snapshot
                    .fixed_time
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "none".to_string());
                send_translatable(
                    sink,
                    "commands.gamerule.set",
                    vec![text_component(rule_name.to_string()), text_component(label)],
                )
                .await?;
            } else {
                let snapshot = world_rules.snapshot(dimension);
                let label = snapshot
                    .fixed_time
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "none".to_string());
                send_translatable(
                    sink,
                    "commands.gamerule.query",
                    vec![text_component(rule_name.to_string()), text_component(label)],
                )
                .await?;
            }
        }
        "worldLight" => {
            if let Some(value) = value {
                let Some(light) = parse_light_mode(value) else {
                    send_unknown_or_incomplete_command(
                        sink,
                        format!("gamerule {dimension} worldLight <static|dynamic|0..15>"),
                    )
                    .await?;
                    return Ok(());
                };
                let snapshot = world_rules.set_light(dimension, light)?;
                send_translatable(
                    sink,
                    "commands.gamerule.set",
                    vec![
                        text_component(rule_name.to_string()),
                        text_component(light_mode_label(&snapshot.light)),
                    ],
                )
                .await?;
            } else {
                let snapshot = world_rules.snapshot(dimension);
                send_translatable(
                    sink,
                    "commands.gamerule.query",
                    vec![
                        text_component(rule_name.to_string()),
                        text_component(light_mode_label(&snapshot.light)),
                    ],
                )
                .await?;
            }
        }
        _ => {
            send_unknown_or_incomplete_command(
                sink,
                format!(
                    "gamerule {dimension} <doDaylightCycle|doBlockUpdates|doWorldReadOnly|timeTickStep|fixedTime|worldLight>"
                ),
            )
            .await?;
        }
    }

    Ok(())
}

async fn handle_scoreboard_command<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    config: &qexed_config::app::qexed::server::Scoreboard,
    argument: &str,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let parts = argument.split_whitespace().collect::<Vec<_>>();
    match parts.as_slice() {
        ["sidebar", "on"] | ["sidebar", "reload"] => {
            for packet in super::scoreboard::custom_sidebar_packets(
                &config.objective,
                &config.title,
                &config.lines,
            )? {
                sink.send_raw(packet).await?;
            }
            send_translatable(
                sink,
                "commands.scoreboard.objectives.setdisplay.success",
                vec![
                    text_component("sidebar"),
                    text_component(config.objective.clone()),
                ],
            )
            .await?;
        }
        ["sidebar", "off"] => {
            for packet in super::scoreboard::clear_sidebar_packet(&config.objective)? {
                sink.send_raw(packet).await?;
            }
            send_translatable(
                sink,
                "commands.scoreboard.objectives.setdisplay.cleared",
                vec![text_component("sidebar")],
            )
            .await?;
        }
        ["objectives", "list"] => {
            send_translatable(
                sink,
                "commands.scoreboard.objectives.list.success",
                vec![
                    text_component("1"),
                    text_component(config.objective.clone()),
                ],
            )
            .await?;
        }
        ["objectives", "add", objective, "dummy"] => {
            sink.send(
                qexed_protocol::to_client::play::set_objective::SetObjective::create(
                    sanitize_scoreboard_objective(objective),
                    text_component(*objective),
                ),
            )
            .await?;
            send_translatable(
                sink,
                "commands.scoreboard.objectives.add.success",
                vec![text_component((*objective).to_string())],
            )
            .await?;
        }
        ["objectives", "add", objective, "dummy", display @ ..] if !display.is_empty() => {
            let display = display.join(" ");
            sink.send(
                qexed_protocol::to_client::play::set_objective::SetObjective::create(
                    sanitize_scoreboard_objective(objective),
                    text_component(display),
                ),
            )
            .await?;
            send_translatable(
                sink,
                "commands.scoreboard.objectives.add.success",
                vec![text_component((*objective).to_string())],
            )
            .await?;
        }
        ["objectives", "remove", objective] => {
            sink.send(
                qexed_protocol::to_client::play::set_objective::SetObjective::remove(
                    sanitize_scoreboard_objective(objective),
                ),
            )
            .await?;
            send_translatable(
                sink,
                "commands.scoreboard.objectives.remove.success",
                vec![text_component((*objective).to_string())],
            )
            .await?;
        }
        ["objectives", "setdisplay", "sidebar", objective] => {
            sink.send(qexed_protocol::to_client::play::set_display_objective::SetDisplayObjective::sidebar(
                sanitize_scoreboard_objective(objective),
            ))
            .await?;
            send_translatable(
                sink,
                "commands.scoreboard.objectives.setdisplay.success",
                vec![
                    text_component("sidebar"),
                    text_component((*objective).to_string()),
                ],
            )
            .await?;
        }
        ["players", "list"] => {
            send_translatable(sink, "commands.scoreboard.players.list.empty", Vec::new()).await?;
        }
        ["players", "set", target, objective, score]
        | ["players", "add", target, objective, score]
        | ["players", "remove", target, objective, score] => {
            let Ok(score) = score.parse::<i32>() else {
                send_unknown_or_incomplete_command(sink, format!("scoreboard {argument}")).await?;
                return Ok(());
            };
            sink.send(qexed_protocol::to_client::play::set_score::SetScore::new(
                (*target).to_string(),
                sanitize_scoreboard_objective(objective),
                score,
                Some(text_component((*target).to_string())),
            ))
            .await?;
            send_translatable(
                sink,
                "commands.scoreboard.players.set.success.single",
                vec![
                    text_component((*objective).to_string()),
                    text_component((*target).to_string()),
                    text_component(score.to_string()),
                ],
            )
            .await?;
        }
        ["players", "reset", target] | ["players", "reset", target, _] => {
            send_translatable(
                sink,
                "commands.scoreboard.players.reset.success.single",
                vec![text_component((*target).to_string())],
            )
            .await?;
        }
        _ => {
            send_unknown_or_incomplete_command(
                sink,
                if argument.trim().is_empty() {
                    "scoreboard".to_string()
                } else {
                    format!("scoreboard {}", argument.trim())
                },
            )
            .await?;
        }
    }
    Ok(())
}

fn sanitize_scoreboard_objective(value: &str) -> String {
    let mut objective = value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.'))
        .take(16)
        .collect::<String>();
    if objective.is_empty() {
        objective = "qexed".to_string();
    }
    objective
}

fn parse_dimension_with_value<'a>(
    mut parts: impl Iterator<Item = &'a str>,
) -> Option<(&'a str, &'a str)> {
    let dimension = parts.next()?;
    let value = parts.next()?;
    if parts.next().is_some() {
        return None;
    }
    Some((dimension, value))
}

fn parse_time_value(value: &str) -> Option<i64> {
    match value {
        "day" => Some(1000),
        "noon" => Some(6000),
        "night" => Some(13000),
        "midnight" => Some(18000),
        _ => value.parse::<i64>().ok(),
    }
}

fn parse_bool_arg(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

fn parse_light_mode(value: &str) -> Option<qexed_config::app::qexed::server::LightMode> {
    match value.trim().to_ascii_lowercase().as_str() {
        "static" => Some(qexed_config::app::qexed::server::LightMode::Static),
        "dynamic" => Some(qexed_config::app::qexed::server::LightMode::Dynamic),
        other => {
            let level = other.parse::<u8>().ok()?;
            (level <= 15).then_some(qexed_config::app::qexed::server::LightMode::Fixed(level))
        }
    }
}

fn light_mode_label(mode: &qexed_config::app::qexed::server::LightMode) -> String {
    match mode {
        qexed_config::app::qexed::server::LightMode::Static => "static".to_string(),
        qexed_config::app::qexed::server::LightMode::Dynamic => "dynamic".to_string(),
        qexed_config::app::qexed::server::LightMode::Fixed(level) => level.to_string(),
    }
}

async fn handle_entity_command<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    players: &PlayerManager,
    entities: &crate::entities::EntityManager,
    rendering: &qexed_config::app::qexed::server::EntityRendering,
    dimension: &str,
    player_position: EntityPosition,
    argument: &str,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let mut parts = argument.split_whitespace();
    let Some(action) = parts.next() else {
        send_entity_usage(sink, argument).await?;
        return Ok(());
    };

    match action {
        "list" => {
            let mut labels = entities
                .list_for_dimension(dimension)
                .into_iter()
                .map(|entity| {
                    format!(
                        "{}({:?}) @ {:.1} {:.1} {:.1}",
                        entity.key,
                        entity.kind,
                        entity.position.x,
                        entity.position.y,
                        entity.position.z
                    )
                })
                .collect::<Vec<_>>();
            labels.sort();
            let message = if labels.is_empty() {
                send_translatable(sink, "commands.datapack.list.available.none", Vec::new())
                    .await?;
                return Ok(());
            } else {
                labels.join(", ")
            };
            send_translatable(
                sink,
                "commands.datapack.list.available.success",
                vec![
                    text_component(labels.len().to_string()),
                    text_component(message),
                ],
            )
            .await?;
        }
        "spawn" => {
            let Some(kind) = parts.next().and_then(parse_entity_kind) else {
                send_entity_usage(sink, argument).await?;
                return Ok(());
            };
            if kind == crate::entities::ManagedEntityKind::Npc {
                send_entity_usage(sink, argument).await?;
                return Ok(());
            }
            let Some(key) = parts.next() else {
                send_entity_usage(sink, argument).await?;
                return Ok(());
            };

            let (entity_type, mut name) = if kind == crate::entities::ManagedEntityKind::Entity {
                let entity_type = parts.next().unwrap_or("minecraft:armor_stand").to_string();
                (entity_type, parts.collect::<Vec<_>>().join(" "))
            } else {
                (String::new(), parts.collect::<Vec<_>>().join(" "))
            };
            if name.trim().is_empty() && kind != crate::entities::ManagedEntityKind::Entity {
                name = key.to_string();
            }
            let entity = match entities.spawn_local(crate::entities::EntitySpawnRequest {
                key: key.to_string(),
                kind,
                entity_type,
                entity_type_id_override: None,
                dimension: dimension.to_string(),
                position: player_position,
                name: name.clone(),
                display_name: name,
                skin_textures: String::new(),
                skin_signature: String::new(),
                data: 0,
                ai: String::new(),
                ai_params: Default::default(),
                auto_jump: false,
                spawn_rule: String::new(),
                custom_type: String::new(),
                look_at_players: false,
                main_hand_event: "interact".to_string(),
                off_hand_event: "interact_off_hand".to_string(),
                attack_event: "attack".to_string(),
            }) {
                Ok(entity) => entity,
                Err(err) => {
                    send_translatable(
                        sink,
                        "command.exception",
                        vec![text_component(format!("{err:#}"))],
                    )
                    .await?;
                    return Ok(());
                }
            };
            entities.send_spawn_to_rendered_viewers(players, rendering, &entity)?;
            send_translatable(
                sink,
                "commands.summon.success",
                vec![text_component(entity.key)],
            )
            .await?;
        }
        "move" => {
            let Some(key) = parts.next() else {
                send_entity_usage(sink, argument).await?;
                return Ok(());
            };
            let moved = match entities.move_entity_local(key, player_position) {
                Ok(entity) => entity,
                Err(err) => {
                    send_translatable(
                        sink,
                        "command.exception",
                        vec![text_component(format!("{err:#}"))],
                    )
                    .await?;
                    return Ok(());
                }
            };
            entities.send_move_to_rendered_viewers(players, rendering, &moved)?;
            send_translatable(
                sink,
                "commands.teleport.success.location.single",
                vec![
                    text_component(key.to_string()),
                    text_component(player_position.x.floor().to_string()),
                    text_component(player_position.y.floor().to_string()),
                    text_component(player_position.z.floor().to_string()),
                ],
            )
            .await?;
        }
        "remove" => {
            let Some(key) = parts.next() else {
                send_entity_usage(sink, argument).await?;
                return Ok(());
            };
            let removed = match entities.remove_local(key) {
                Ok(entity) => entity,
                Err(err) => {
                    send_translatable(
                        sink,
                        "command.exception",
                        vec![text_component(format!("{err:#}"))],
                    )
                    .await?;
                    return Ok(());
                }
            };
            entities.send_remove_to_rendered_viewers(players, rendering, &removed)?;
            send_translatable(
                sink,
                "commands.bossbar.remove.success",
                vec![text_component(key.to_string())],
            )
            .await?;
        }
        _ => {
            send_entity_usage(sink, argument).await?;
        }
    }
    Ok(())
}

async fn handle_npc_command<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    players: &PlayerManager,
    entities: &crate::entities::EntityManager,
    rendering: &qexed_config::app::qexed::server::EntityRendering,
    dimension: &str,
    player_position: EntityPosition,
    argument: &str,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let mut parts = argument.split_whitespace();
    let Some(action) = parts.next() else {
        send_npc_usage(sink, argument).await?;
        return Ok(());
    };

    match action {
        "list" => {
            let mut labels = entities
                .list_for_dimension(dimension)
                .into_iter()
                .filter(|entity| entity.kind == crate::entities::ManagedEntityKind::Npc)
                .map(|entity| {
                    format!(
                        "{} @ {:.1} {:.1} {:.1}",
                        entity.key, entity.position.x, entity.position.y, entity.position.z
                    )
                })
                .collect::<Vec<_>>();
            labels.sort();
            if labels.is_empty() {
                send_translatable(sink, "commands.datapack.list.available.none", Vec::new())
                    .await?;
            } else {
                send_translatable(
                    sink,
                    "commands.datapack.list.available.success",
                    vec![
                        text_component(labels.len().to_string()),
                        text_component(labels.join(", ")),
                    ],
                )
                .await?;
            }
        }
        "spawn" => {
            let args = parts.collect::<Vec<_>>();
            let Some(args) = parse_npc_spawn_args(&args) else {
                send_npc_usage(sink, argument).await?;
                return Ok(());
            };
            let entity = match entities.spawn_local(crate::entities::EntitySpawnRequest {
                key: args.key,
                kind: crate::entities::ManagedEntityKind::Npc,
                entity_type: args.entity_type,
                entity_type_id_override: None,
                dimension: dimension.to_string(),
                position: player_position,
                name: args.name.clone(),
                display_name: args.name,
                skin_textures: String::new(),
                skin_signature: String::new(),
                data: 0,
                ai: String::new(),
                ai_params: Default::default(),
                auto_jump: false,
                spawn_rule: String::new(),
                custom_type: String::new(),
                look_at_players: false,
                main_hand_event: "interact".to_string(),
                off_hand_event: "interact_off_hand".to_string(),
                attack_event: "attack".to_string(),
            }) {
                Ok(entity) => entity,
                Err(err) => {
                    send_translatable(
                        sink,
                        "command.exception",
                        vec![text_component(format!("{err:#}"))],
                    )
                    .await?;
                    return Ok(());
                }
            };
            entities.send_spawn_to_rendered_viewers(players, rendering, &entity)?;
            send_translatable(
                sink,
                "commands.summon.success",
                vec![text_component(entity.key)],
            )
            .await?;
        }
        "move" => {
            let Some(key) = parts.next() else {
                send_npc_usage(sink, argument).await?;
                return Ok(());
            };
            if entities
                .entity_by_key(key)
                .is_some_and(|entity| entity.kind != crate::entities::ManagedEntityKind::Npc)
            {
                send_npc_usage(sink, argument).await?;
                return Ok(());
            }
            let moved = match entities.move_entity_local(key, player_position) {
                Ok(entity) => entity,
                Err(err) => {
                    send_translatable(
                        sink,
                        "command.exception",
                        vec![text_component(format!("{err:#}"))],
                    )
                    .await?;
                    return Ok(());
                }
            };
            entities.send_move_to_rendered_viewers(players, rendering, &moved)?;
            send_translatable(
                sink,
                "commands.teleport.success.location.single",
                vec![
                    text_component(key.to_string()),
                    text_component(player_position.x.floor().to_string()),
                    text_component(player_position.y.floor().to_string()),
                    text_component(player_position.z.floor().to_string()),
                ],
            )
            .await?;
        }
        "remove" => {
            let Some(key) = parts.next() else {
                send_npc_usage(sink, argument).await?;
                return Ok(());
            };
            if entities
                .entity_by_key(key)
                .is_some_and(|entity| entity.kind != crate::entities::ManagedEntityKind::Npc)
            {
                send_npc_usage(sink, argument).await?;
                return Ok(());
            }
            let removed = match entities.remove_local(key) {
                Ok(entity) => entity,
                Err(err) => {
                    send_translatable(
                        sink,
                        "command.exception",
                        vec![text_component(format!("{err:#}"))],
                    )
                    .await?;
                    return Ok(());
                }
            };
            entities.send_remove_to_rendered_viewers(players, rendering, &removed)?;
            send_translatable(
                sink,
                "commands.bossbar.remove.success",
                vec![text_component(key.to_string())],
            )
            .await?;
        }
        _ => {
            send_npc_usage(sink, argument).await?;
        }
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NpcSpawnArgs {
    key: String,
    entity_type: String,
    name: String,
}

fn parse_npc_spawn_args(parts: &[&str]) -> Option<NpcSpawnArgs> {
    let key = parts.first()?.trim();
    if key.is_empty() {
        return None;
    }

    let (entity_type, name_offset) = if let Some(entity_type) = parts
        .get(1)
        .and_then(|value| normalize_npc_spawn_entity_type(value))
    {
        (entity_type, 2)
    } else {
        ("minecraft:player".to_string(), 1)
    };
    let mut name = parts[name_offset..].join(" ");
    if name.trim().is_empty() {
        name = key.to_string();
    }

    Some(NpcSpawnArgs {
        key: key.to_string(),
        entity_type,
        name,
    })
}

fn normalize_npc_spawn_entity_type(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() || !value.contains(':') {
        return None;
    }
    let entity_type = value.to_string();
    crate::entities::entity_type_id(&entity_type)
        .is_ok()
        .then_some(entity_type)
}

async fn handle_structure_command<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    world: &crate::world::WorldManager,
    world_rules: &crate::world::WorldRulesManager,
    players: &PlayerManager,
    world_config: &qexed_config::app::qexed::server::World,
    dimension: &str,
    actor: uuid::Uuid,
    player_position: EntityPosition,
    argument: &str,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let mut parts = argument.split_whitespace();
    let Some(action) = parts.next() else {
        send_structure_usage(sink, argument).await?;
        return Ok(());
    };

    match action {
        "list" => {
            let templates = crate::structures::list_templates()
                .iter()
                .map(|template| template.id)
                .collect::<Vec<_>>();
            if templates.is_empty() {
                send_translatable(sink, "commands.datapack.list.available.none", Vec::new())
                    .await?;
            } else {
                send_translatable(
                    sink,
                    "commands.datapack.list.available.success",
                    vec![
                        text_component(templates.len().to_string()),
                        text_component(templates.join(", ")),
                    ],
                )
                .await?;
            }
        }
        "place" => {
            let Some(id) = parts.next() else {
                send_structure_usage(sink, argument).await?;
                return Ok(());
            };
            let Some(origin) = parse_structure_position(&mut parts, player_position) else {
                send_structure_usage(sink, argument).await?;
                return Ok(());
            };
            let blocks = match crate::structures::instantiate(id, origin.clone()) {
                Ok(blocks) => blocks,
                Err(err) => {
                    send_translatable(
                        sink,
                        "command.exception",
                        vec![text_component(format!("{err:#}"))],
                    )
                    .await?;
                    return Ok(());
                }
            };
            let dimension_rule = world_rules.snapshot(dimension);
            let blocks = blocks
                .into_iter()
                .filter(|(position, _)| {
                    super::util::can_modify_world(world_config, position)
                        && dimension_rule.block_updates
                        && !dimension_rule.read_only
                })
                .collect::<Vec<_>>();
            if blocks.is_empty() {
                send_translatable(sink, "commands.place.structure.failed", Vec::new()).await?;
                return Ok(());
            }

            let updates = match world.place_blocks(dimension, blocks) {
                Ok(updates) => updates,
                Err(err) => {
                    send_translatable(
                        sink,
                        "command.exception",
                        vec![text_component(format!("{err:#}"))],
                    )
                    .await?;
                    return Ok(());
                }
            };
            let mut light_chunks = std::collections::BTreeSet::new();
            for update in &updates {
                sink.send(update.clone()).await?;
                light_chunks.insert((
                    update.location.x.div_euclid(16),
                    update.location.z.div_euclid(16),
                ));
                players.broadcast_block_changed(
                    actor,
                    dimension,
                    update.location.clone(),
                    update.block_state.0,
                    None,
                );
            }
            if world.dynamic_light_enabled()
                && matches!(
                    dimension_rule.light,
                    qexed_config::app::qexed::server::LightMode::Dynamic
                )
            {
                for (chunk_x, chunk_z) in light_chunks {
                    let update = world.light_update(dimension, chunk_x, chunk_z);
                    sink.send(update.clone()).await?;
                    let packet =
                        qexed_tcp_connect::PacketSink::<tokio::io::Sink>::build_send_packet(
                            update,
                        )?;
                    players.broadcast_packets_except(actor, vec![packet]);
                }
            }
            send_translatable(
                sink,
                "commands.place.structure.success",
                vec![
                    text_component(id.to_string()),
                    text_component(origin.x.to_string()),
                    text_component(origin.y.to_string()),
                    text_component(origin.z.to_string()),
                ],
            )
            .await?;
        }
        "locate" => {
            let Some(id) = parts.next() else {
                send_structure_usage(sink, argument).await?;
                return Ok(());
            };
            let Some(template) = crate::structures::get_template(id) else {
                send_translatable(
                    sink,
                    "commands.locate.structure.invalid",
                    vec![text_component(id.to_string())],
                )
                .await?;
                return Ok(());
            };
            let position = nearest_structure_candidate(template.id, player_position);
            let dx = position.x - player_position.x.floor() as i32;
            let dz = position.z - player_position.z.floor() as i32;
            let distance = ((dx * dx + dz * dz) as f64).sqrt().round() as i32;
            send_translatable(
                sink,
                "commands.locate.structure.success",
                vec![
                    text_component(template.id.to_string()),
                    text_component(format!("{}, {}", position.x, position.z)),
                    text_component(distance.to_string()),
                ],
            )
            .await?;
        }
        _ => {
            send_structure_usage(sink, argument).await?;
        }
    }

    Ok(())
}

fn parse_structure_position<'a>(
    parts: &mut impl Iterator<Item = &'a str>,
    player_position: EntityPosition,
) -> Option<qexed_packet::net_types::Position> {
    let Some(x) = parts.next() else {
        return Some(qexed_packet::net_types::Position {
            x: player_position.x.floor() as i32,
            y: player_position.y.floor() as i32,
            z: player_position.z.floor() as i32,
        });
    };
    let y = parts.next()?;
    let z = parts.next()?;
    if parts.next().is_some() {
        return None;
    }
    Some(qexed_packet::net_types::Position {
        x: parse_command_coord(x, player_position.x)?,
        y: parse_command_coord(y, player_position.y)?,
        z: parse_command_coord(z, player_position.z)?,
    })
}

fn parse_command_coord(value: &str, base: f64) -> Option<i32> {
    if value == "~" {
        return Some(base.floor() as i32);
    }
    if let Some(offset) = value.strip_prefix('~') {
        let offset = if offset.is_empty() {
            0.0
        } else {
            offset.parse::<f64>().ok()?
        };
        return Some((base + offset).floor() as i32);
    }
    Some(value.parse::<f64>().ok()?.floor() as i32)
}

fn nearest_structure_candidate(
    id: &str,
    player_position: EntityPosition,
) -> qexed_packet::net_types::Position {
    let spacing = match id {
        "qexed:desert_well" => 32,
        "qexed:obsidian_pillar" => 48,
        _ => 16,
    };
    let x = nearest_grid_coord(player_position.x.floor() as i32, spacing);
    let z = nearest_grid_coord(player_position.z.floor() as i32, spacing);
    qexed_packet::net_types::Position {
        x,
        y: player_position.y.floor() as i32,
        z,
    }
}

fn nearest_grid_coord(value: i32, spacing: i32) -> i32 {
    let lower = value.div_euclid(spacing) * spacing;
    let upper = lower + spacing;
    if (value - lower).abs() <= (upper - value).abs() {
        lower
    } else {
        upper
    }
}

fn parse_entity_kind(value: &str) -> Option<crate::entities::ManagedEntityKind> {
    match value {
        "entity" => Some(crate::entities::ManagedEntityKind::Entity),
        "hologram" => Some(crate::entities::ManagedEntityKind::Hologram),
        _ => None,
    }
}

async fn send_entity_usage<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    argument: &str,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    send_unknown_or_incomplete_command(
        sink,
        if argument.trim().is_empty() {
            "entity".to_string()
        } else {
            format!("entity {}", argument.trim())
        },
    )
    .await
}

async fn send_npc_usage<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    argument: &str,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    send_unknown_or_incomplete_command(
        sink,
        if argument.trim().is_empty() {
            "npc".to_string()
        } else {
            format!("npc {}", argument.trim())
        },
    )
    .await
}

async fn send_structure_usage<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    argument: &str,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    send_unknown_or_incomplete_command(
        sink,
        if argument.trim().is_empty() {
            "structure".to_string()
        } else {
            format!("structure {}", argument.trim())
        },
    )
    .await
}

async fn send_translatable<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    key: impl Into<String>,
    with: Vec<qexed_protocol::types::TextComponent>,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    sink.send(SystemChat {
        content: translatable_component(key, with),
        overlay: false,
    })
    .await?;
    Ok(())
}

async fn send_unknown_or_incomplete_command<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    command: String,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let mut message = std::collections::HashMap::new();
    message.insert(
        "translate".to_string(),
        qexed_nbt::Tag::String(std::sync::Arc::from("command.unknown.command")),
    );
    message.insert(
        "color".to_string(),
        qexed_nbt::Tag::String(std::sync::Arc::from("red")),
    );

    let mut context_text = std::collections::HashMap::new();
    context_text.insert(
        "text".to_string(),
        qexed_nbt::Tag::String(std::sync::Arc::from(format!("\n/{command}"))),
    );
    context_text.insert(
        "color".to_string(),
        qexed_nbt::Tag::String(std::sync::Arc::from("red")),
    );

    let mut context_here = std::collections::HashMap::new();
    context_here.insert(
        "translate".to_string(),
        qexed_nbt::Tag::String(std::sync::Arc::from("command.context.here")),
    );
    context_here.insert(
        "color".to_string(),
        qexed_nbt::Tag::String(std::sync::Arc::from("red")),
    );
    context_here.insert("italic".to_string(), qexed_nbt::Tag::Byte(1));

    message.insert(
        "extra".to_string(),
        qexed_nbt::Tag::List(
            qexed_nbt::ListHeader {
                tag_id: qexed_nbt::tag_id::COMPOUND,
                length: 2,
            },
            std::sync::Arc::from(
                vec![
                    qexed_nbt::Tag::Compound(std::sync::Arc::new(context_text)),
                    qexed_nbt::Tag::Compound(std::sync::Arc::new(context_here)),
                ]
                .into_boxed_slice(),
            ),
        ),
    );

    sink.send(SystemChat {
        content: qexed_nbt::Tag::Compound(std::sync::Arc::new(message)),
        overlay: false,
    })
    .await?;
    Ok(())
}

async fn send_unimplemented_command<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    locale: &str,
    command: &str,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    sink.send(SystemChat {
        content: text_component(crate::commands::localized_unimplemented(locale, command)),
        overlay: false,
    })
    .await?;
    Ok(())
}

async fn send_help_messages<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    permissions: &crate::permissions::PermissionManager,
    plugins: &crate::plugins::PluginManager,
    profile: &qexed_packet::net_types::GameProfile,
    locale: &str,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let mut entries = Vec::new();
    let visible_commands = crate::commands::visible_commands(permissions, profile).await?;
    entries.extend(crate::commands::localized_builtin_help_entries(
        &visible_commands,
        locale,
    ));

    for plugin_command in plugins.plugin_commands() {
        if !permissions
            .can_run_command(profile, &plugin_command.name)
            .await?
        {
            continue;
        }
        entries.push(crate::commands::localized_plugin_help_entry(
            &plugin_command.name,
            &plugin_command.description_key,
            locale,
        ));
    }

    entries.sort_by(|left, right| left.usage.cmp(&right.usage));
    sink.send(SystemChat {
        content: text_component(crate::commands::localized_server_help(locale, &entries)),
        overlay: false,
    })
    .await?;

    Ok(())
}

fn player_locale(
    players: &PlayerManager,
    profile: &qexed_packet::net_types::GameProfile,
    fallback: &str,
) -> String {
    players
        .player_by_uuid(profile.uuid)
        .map(|player| player.language)
        .unwrap_or_else(|| fallback.to_string())
}

pub(super) async fn apply_plugin_action<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    server_config: Option<&Server>,
    world: &crate::world::WorldManager,
    world_rules: &crate::world::WorldRulesManager,
    world_config: &qexed_config::app::qexed::server::World,
    players: &crate::players::PlayerManager,
    plugins: &crate::plugins::PluginManager,
    actor: uuid::Uuid,
    chunk_sender: &tokio::sync::mpsc::UnboundedSender<super::chunks::ChunkLoadResult>,
    chunk_state: &mut super::ChunkSendState,
    position: &mut EntityPosition,
    next_teleport_id: &mut i32,
    play_dimension: &mut String,
    menus: &super::menus::MenuRuntime,
    active_config_menu: &mut Option<String>,
    players_hidden: &mut bool,
    visible_player_entities: &mut std::collections::HashSet<uuid::Uuid>,
    viewer_position: EntityPosition,
    render_distance: f64,
    action: crate::plugins::PlayerAction,
) -> Result<bool>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    match action {
        crate::plugins::PlayerAction::SystemMessage {
            text,
            translate,
            with,
            overlay,
        } => {
            let content = if !translate.trim().is_empty() {
                translatable_component(
                    translate,
                    with.into_iter().map(text_component).collect::<Vec<_>>(),
                )
            } else {
                text_component(text)
            };
            sink.send(SystemChat { content, overlay }).await?;
            Ok(false)
        }
        crate::plugins::PlayerAction::Teleport {
            dimension,
            x,
            y,
            z,
            yaw,
            pitch,
        } => {
            let mut destination = *position;
            destination.x = x;
            destination.y = y;
            destination.z = z;
            if let Some(yaw) = yaw {
                destination.yaw = yaw;
            }
            if let Some(pitch) = pitch {
                destination.pitch = pitch;
            }
            let dimension = if dimension.trim().is_empty() {
                play_dimension.clone()
            } else {
                dimension.trim().to_string()
            };
            teleport_current_player(
                sink,
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
                &dimension,
                destination,
            )
            .await
        }
        crate::plugins::PlayerAction::Transfer {
            host,
            port,
            message,
        } => {
            if !message.trim().is_empty() {
                sink.send(SystemChat {
                    content: text_component(message),
                    overlay: false,
                })
                .await?;
            }
            if !host.trim().is_empty() && port > 0 {
                sink.send(Transfer::new(host.trim(), port)).await?;
            }
            Ok(false)
        }
        crate::plugins::PlayerAction::ProxyConnect { server, message } => {
            apply_proxy_connect_action(
                sink,
                server_config,
                plugins,
                players,
                actor,
                &server,
                &message,
            )
            .await?;
            Ok(false)
        }
        crate::plugins::PlayerAction::OpenMenu { menu } => {
            let opened = menus.open_menu(sink, &menu).await?;
            *active_config_menu = opened;
            Ok(false)
        }
        crate::plugins::PlayerAction::SetPlayersVisible { visible } => {
            super::set_other_players_visible(
                sink,
                players,
                actor,
                play_dimension,
                viewer_position,
                render_distance,
                visible_player_entities,
                visible,
            )
            .await?;
            *players_hidden = !visible;
            Ok(false)
        }
        crate::plugins::PlayerAction::Velocity { x, y, z, additive } => {
            send_player_velocity(sink, position, next_teleport_id, x, y, z, additive).await?;
            Ok(false)
        }
    }
}

async fn send_player_velocity<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    position: &EntityPosition,
    next_teleport_id: &mut i32,
    x: f64,
    y: f64,
    z: f64,
    additive: bool,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    if !x.is_finite() || !y.is_finite() || !z.is_finite() {
        return Ok(());
    }

    const RELATIVE_POSITION_AND_ROTATION: i32 = 0x01 | 0x02 | 0x04 | 0x08 | 0x10;
    const RELATIVE_DELTA: i32 = 0x20 | 0x40 | 0x80;
    let teleport_id = *next_teleport_id;
    *next_teleport_id = next_teleport_id.saturating_add(1);
    sink.send(Position {
        teleport_id: VarInt(teleport_id),
        x: 0.0,
        y: 0.0,
        z: 0.0,
        dx: x,
        dy: y,
        dz: z,
        yaw: 0.0,
        pitch: 0.0,
        flags: RELATIVE_POSITION_AND_ROTATION | if additive { RELATIVE_DELTA } else { 0 },
    })
    .await?;
    log::debug!(
        "sent player velocity action: x={x}, y={y}, z={z}, additive={additive}, player_x={}, player_y={}, player_z={}",
        position.x,
        position.y,
        position.z
    );
    Ok(())
}

pub(super) async fn apply_proxy_connect_action<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    server_config: Option<&qexed_config::app::qexed::server::Server>,
    plugins: &crate::plugins::PluginManager,
    players: &crate::players::PlayerManager,
    actor: uuid::Uuid,
    server: &str,
    message: &str,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let server = server.trim();
    let Some(server_config) = server_config else {
        send_proxy_connect_feedback(
            sink,
            "Proxy transfer is unavailable: proxy config is missing.",
        )
        .await?;
        emit_proxy_connect_result(
            plugins,
            players,
            actor,
            server,
            "",
            "",
            ProxyConnectStatus::ProxyContextMissing,
            "proxy config was not available while executing proxy connect action",
        );
        return Ok(());
    };
    if server.is_empty() {
        send_proxy_connect_feedback(
            sink,
            "Proxy transfer is unavailable: target server is empty.",
        )
        .await?;
        emit_proxy_connect_result(
            plugins,
            players,
            actor,
            server,
            &server_config.proxy_server_id,
            &server_config.proxy_protocol.to_string(),
            ProxyConnectStatus::TargetEmpty,
            "target proxy server is empty",
        );
        return Ok(());
    }
    if !server_config.proxy {
        send_proxy_connect_feedback(sink, "Proxy transfer is unavailable: proxy is disabled.")
            .await?;
        emit_proxy_connect_result(
            plugins,
            players,
            actor,
            server,
            &server_config.proxy_server_id,
            &server_config.proxy_protocol.to_string(),
            ProxyConnectStatus::ProxyDisabled,
            "proxy forwarding is not enabled",
        );
        return Ok(());
    }
    if !matches!(
        server_config.proxy_protocol,
        ForwardingMode::Velocity | ForwardingMode::Victory | ForwardingMode::BungeeCord
    ) {
        send_proxy_connect_feedback(
            sink,
            "Proxy transfer is unavailable: proxy protocol does not support backend switching.",
        )
        .await?;
        emit_proxy_connect_result(
            plugins,
            players,
            actor,
            server,
            &server_config.proxy_server_id,
            &server_config.proxy_protocol.to_string(),
            ProxyConnectStatus::UnsupportedProxyProtocol,
            "proxy protocol does not support backend connect plugin messages",
        );
        return Ok(());
    }
    let target = match preflight_proxy_connect_target(server_config, server).await {
        ProxyTargetPreflight::Allowed { server_id } => server_id,
        ProxyTargetPreflight::Rejected {
            status,
            user_message,
            result_message,
        } => {
            send_proxy_connect_feedback(sink, &user_message).await?;
            emit_proxy_connect_result(
                plugins,
                players,
                actor,
                server,
                &server_config.proxy_server_id,
                &server_config.proxy_protocol.to_string(),
                status,
                &result_message,
            );
            return Ok(());
        }
    };
    if !message.trim().is_empty() {
        sink.send(SystemChat {
            content: text_component(message),
            overlay: false,
        })
        .await?;
    }
    if let Err(err) = sink.send(proxy_connect_payload(&target)).await {
        emit_proxy_connect_result(
            plugins,
            players,
            actor,
            &target,
            &server_config.proxy_server_id,
            &server_config.proxy_protocol.to_string(),
            ProxyConnectStatus::SendFailed,
            &format!("failed to send proxy connect payload: {err}"),
        );
        return Err(err.into());
    }
    emit_proxy_connect_result(
        plugins,
        players,
        actor,
        &target,
        &server_config.proxy_server_id,
        &server_config.proxy_protocol.to_string(),
        ProxyConnectStatus::SentToProxy,
        "proxy connect request was sent to the proxy",
    );
    Ok(())
}

enum ProxyTargetPreflight {
    Allowed {
        server_id: String,
    },
    Rejected {
        status: ProxyConnectStatus,
        user_message: String,
        result_message: String,
    },
}

async fn preflight_proxy_connect_target(
    server_config: &Server,
    target: &str,
) -> ProxyTargetPreflight {
    let target = target.trim();
    if !server_config.proxy_server_id.trim().is_empty()
        && server_config.proxy_server_id.eq_ignore_ascii_case(target)
    {
        return proxy_target_rejected(
            ProxyConnectStatus::AlreadyConnected,
            format!("You are already connected to {target}."),
            "player is already connected to the target proxy server".to_string(),
        );
    }

    ProxyTargetPreflight::Allowed {
        server_id: target.to_string(),
    }
}

fn proxy_target_rejected(
    status: ProxyConnectStatus,
    user_message: String,
    result_message: String,
) -> ProxyTargetPreflight {
    ProxyTargetPreflight::Rejected {
        status,
        user_message,
        result_message,
    }
}

async fn send_proxy_connect_feedback<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    message: &str,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    sink.send(SystemChat {
        content: text_component(message),
        overlay: false,
    })
    .await?;
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProxyConnectStatus {
    SentToProxy,
    TargetEmpty,
    ProxyDisabled,
    UnsupportedProxyProtocol,
    AlreadyConnected,
    SendFailed,
    ProxyContextMissing,
    TargetUnknown,
    TargetDisabled,
    TargetMaintenance,
    TargetOffline,
}

impl ProxyConnectStatus {
    fn code(self) -> i32 {
        match self {
            Self::SentToProxy => 0,
            Self::TargetEmpty => 100,
            Self::ProxyDisabled => 101,
            Self::UnsupportedProxyProtocol => 102,
            Self::AlreadyConnected => 103,
            Self::SendFailed => 104,
            Self::ProxyContextMissing => 105,
            Self::TargetUnknown => 106,
            Self::TargetDisabled => 107,
            Self::TargetMaintenance => 108,
            Self::TargetOffline => 109,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::SentToProxy => "sent_to_proxy",
            Self::TargetEmpty => "target_empty",
            Self::ProxyDisabled => "proxy_disabled",
            Self::UnsupportedProxyProtocol => "unsupported_proxy_protocol",
            Self::AlreadyConnected => "already_connected",
            Self::SendFailed => "send_failed",
            Self::ProxyContextMissing => "proxy_context_missing",
            Self::TargetUnknown => "target_unknown",
            Self::TargetDisabled => "target_disabled",
            Self::TargetMaintenance => "target_maintenance",
            Self::TargetOffline => "target_offline",
        }
    }

    fn success(self) -> bool {
        matches!(self, Self::SentToProxy)
    }
}

fn emit_proxy_connect_result(
    plugins: &crate::plugins::PluginManager,
    players: &crate::players::PlayerManager,
    actor: uuid::Uuid,
    target_server: &str,
    current_server: &str,
    proxy_protocol: &str,
    status: ProxyConnectStatus,
    message: &str,
) {
    let Some(player) = players.player_by_uuid(actor) else {
        log::warn!(
            "proxy connect result dropped because player is no longer online: player={actor}, target={target_server}, status={}",
            status.as_str()
        );
        return;
    };
    plugins.emit_proxy_connect_result(&crate::plugins::ProxyConnectResultPayload {
        player: crate::plugins::PlayerPayloadOwned {
            uuid: player.profile.uuid.to_string(),
            username: player.profile.username,
            entity_id: player.entity_id,
            language: player.language,
            dimension: player.dimension,
        },
        target_server: target_server.to_string(),
        current_server: current_server.trim().to_string(),
        proxy_protocol: proxy_protocol.to_string(),
        status_code: status.code(),
        status: status.as_str().to_string(),
        success: status.success(),
        message: message.to_string(),
    });
}

fn proxy_connect_payload(server: &str) -> CustomPayload {
    CustomPayload {
        channel: "bungeecord:main".to_string(),
        data: qexed_packet::net_types::RestBuffer(bungee_connect_data(server)),
    }
}

fn bungee_connect_data(server: &str) -> Vec<u8> {
    let mut data = Vec::with_capacity("Connect".len() + server.len() + 4);
    write_modified_utf8(&mut data, "Connect");
    write_modified_utf8(&mut data, server);
    data
}

fn write_modified_utf8(out: &mut Vec<u8>, value: &str) {
    let bytes = value.as_bytes();
    let len = u16::try_from(bytes.len()).unwrap_or(u16::MAX);
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(&bytes[..usize::from(len)]);
}

#[cfg(test)]
mod tests {
    use super::{bungee_connect_data, proxy_connect_payload};

    fn proxy_server() -> qexed_config::app::qexed::server::Server {
        let mut server = qexed_config::app::qexed::server::Server::default();
        server.proxy = true;
        server.proxy_protocol = qexed_config::app::qexed::server::ForwardingMode::Velocity;
        server.proxy_server_id = "lobby-1".to_string();
        server
    }

    #[test]
    fn velocity_connect_payload_uses_bungeecord_connect_channel() {
        let payload = proxy_connect_payload("lobby-1");

        assert_eq!(payload.channel, "bungeecord:main");
        assert_eq!(
            payload.data.0,
            vec![
                0, 7, b'C', b'o', b'n', b'n', b'e', b'c', b't', 0, 7, b'l', b'o', b'b', b'b', b'y',
                b'-', b'1',
            ]
        );
    }

    #[test]
    fn velocity_connect_payload_packet_uses_play_custom_payload_id() {
        let packet = qexed_tcp_connect::PacketSink::<tokio::io::Sink>::build_send_packet(
            proxy_connect_payload("lobby-1"),
        )
        .unwrap();

        assert_eq!(packet[0], 0x18);
        assert!(
            packet
                .windows("bungeecord:main".len())
                .any(|window| window == "bungeecord:main".as_bytes())
        );
    }

    #[test]
    fn proxy_connect_payload_contains_target_backend_name() {
        let packet = qexed_tcp_connect::PacketSink::<tokio::io::Sink>::build_send_packet(
            proxy_connect_payload("lobby-1"),
        )
        .unwrap();

        assert!(
            packet
                .windows("lobby-1".len())
                .any(|window| window == "lobby-1".as_bytes())
        );
        assert!(
            packet
                .windows("Connect".len())
                .any(|window| window == "Connect".as_bytes())
        );
    }

    #[test]
    fn bungee_connect_data_truncates_oversized_server_names() {
        let server = "a".repeat(usize::from(u16::MAX) + 1);
        let data = bungee_connect_data(&server);

        assert_eq!(
            &data[..9],
            &[0, 7, b'C', b'o', b'n', b'n', b'e', b'c', b't']
        );
        assert_eq!(&data[9..11], &u16::MAX.to_be_bytes());
        assert_eq!(data.len(), 9 + 2 + usize::from(u16::MAX));
    }

    #[test]
    fn npc_spawn_args_default_to_player_npc() {
        let args = super::parse_npc_spawn_args(&["guard", "Guard", "One"]).unwrap();

        assert_eq!(args.key, "guard");
        assert_eq!(args.entity_type, "minecraft:player");
        assert_eq!(args.name, "Guard One");
    }

    #[test]
    fn npc_spawn_args_accept_client_entity_shell() {
        let args = super::parse_npc_spawn_args(&["guard", "minecraft:zombie", "Guard"]).unwrap();

        assert_eq!(args.key, "guard");
        assert_eq!(args.entity_type, "minecraft:zombie");
        assert_eq!(args.name, "Guard");
    }

    #[test]
    fn npc_spawn_args_keep_short_entity_like_name_as_display_name() {
        let args = super::parse_npc_spawn_args(&["guard", "zombie"]).unwrap();

        assert_eq!(args.key, "guard");
        assert_eq!(args.entity_type, "minecraft:player");
        assert_eq!(args.name, "zombie");
    }

    #[test]
    fn proxy_connect_status_codes_are_stable() {
        assert_eq!(super::ProxyConnectStatus::SentToProxy.code(), 0);
        assert_eq!(super::ProxyConnectStatus::TargetEmpty.code(), 100);
        assert_eq!(super::ProxyConnectStatus::ProxyDisabled.code(), 101);
        assert_eq!(
            super::ProxyConnectStatus::UnsupportedProxyProtocol.code(),
            102
        );
        assert_eq!(super::ProxyConnectStatus::AlreadyConnected.code(), 103);
        assert_eq!(super::ProxyConnectStatus::SendFailed.code(), 104);
        assert_eq!(super::ProxyConnectStatus::ProxyContextMissing.code(), 105);
        assert_eq!(super::ProxyConnectStatus::TargetUnknown.code(), 106);
        assert_eq!(super::ProxyConnectStatus::TargetDisabled.code(), 107);
        assert_eq!(super::ProxyConnectStatus::TargetMaintenance.code(), 108);
        assert_eq!(super::ProxyConnectStatus::TargetOffline.code(), 109);
    }

    #[test]
    fn proxy_connect_success_only_for_sent_to_proxy() {
        assert!(super::ProxyConnectStatus::SentToProxy.success());
        assert!(!super::ProxyConnectStatus::AlreadyConnected.success());
        assert!(!super::ProxyConnectStatus::ProxyDisabled.success());
        assert!(!super::ProxyConnectStatus::TargetOffline.success());
    }

    #[tokio::test]
    async fn proxy_connect_preflight_allows_proxy_owned_target_names() {
        let config = proxy_server();

        let result = super::preflight_proxy_connect_target(&config, "missing").await;

        match result {
            super::ProxyTargetPreflight::Allowed { server_id } => {
                assert_eq!(server_id, "missing");
            }
            super::ProxyTargetPreflight::Rejected { .. } => {
                panic!("proxy-owned backend names should be forwarded to Velocity/Victory");
            }
        }
    }

    #[tokio::test]
    async fn proxy_connect_preflight_rejects_current_server() {
        let config = proxy_server();

        let result = super::preflight_proxy_connect_target(&config, "lobby-1").await;

        match result {
            super::ProxyTargetPreflight::Rejected { status, .. } => {
                assert_eq!(status, super::ProxyConnectStatus::AlreadyConnected);
            }
            super::ProxyTargetPreflight::Allowed { .. } => {
                panic!("current proxy target should be rejected before Velocity receives it");
            }
        }
    }
}

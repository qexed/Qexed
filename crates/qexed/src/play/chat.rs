use anyhow::Result;
use qexed_packet::net_types::VarInt;
use qexed_protocol::to_client::play::{
    add_entity::EntityPosition, position::Position, system_chat::SystemChat, transfer::Transfer,
};

use crate::players::PlayerManager;

use super::util::{text_component, translatable_component};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(super) struct CommandOutcome {
    pub teleported: bool,
    pub opened_lobby_menu: bool,
}

pub(super) async fn handle_chat_command<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    config: &qexed_config::app::qexed::Qexed,
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
    chunk_sender: &tokio::sync::mpsc::UnboundedSender<super::chunks::ChunkLoadResult>,
    chunk_state: &mut super::ChunkSendState,
    position: &mut qexed_protocol::to_client::play::add_entity::EntityPosition,
    next_teleport_id: &mut i32,
    play_dimension: &str,
) -> Result<CommandOutcome>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let command = command.trim();
    if !permissions.can_run_command(profile, command).await? {
        sink.send(SystemChat {
            content: translatable_component("commands.help.failed", Vec::new()),
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
            let requested_page = parse_help_page(argument.trim());
            send_help_messages(sink, permissions, plugins, profile, requested_page).await?;
            Ok(CommandOutcome::default())
        }
        "list" => {
            let mut names = players.online_names();
            names.sort();
            let joined = names.join(", ");
            let max_players = if config.server.max_player < 0 {
                translatable_component("effect.duration.infinite", Vec::new())
            } else {
                text_component(config.server.max_player.to_string())
            };
            sink.send(SystemChat {
                content: translatable_component(
                    "commands.list.players",
                    vec![
                        text_component(players.online_count().to_string()),
                        max_players,
                        text_component(joined),
                    ],
                ),
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
            lobby
                .transfer_to_server(sink, &server_id, lobby_status)
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
                &config.server.world,
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
                        text_component(config.server.world.spawn.x.floor().to_string()),
                        text_component(config.server.world.spawn.y.floor().to_string()),
                        text_component(config.server.world.spawn.z.floor().to_string()),
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
        "time" => {
            handle_time_command(sink, world_rules, argument.as_str()).await?;
            Ok(CommandOutcome::default())
        }
        "gamerule" => {
            handle_gamerule_command(sink, world_rules, argument.as_str()).await?;
            Ok(CommandOutcome::default())
        }
        "entity" => {
            handle_entity_command(
                sink,
                players,
                entities,
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
                &config.server.world,
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
                    equipment: Vec::new(),
                },
                &name,
                argument.as_str(),
            );
            if plugin_response.handled || !plugin_response.actions.is_empty() {
                let mut teleported = false;
                for action in plugin_response.actions {
                    teleported |= apply_plugin_action(
                        sink,
                        world,
                        players,
                        plugins,
                        profile.uuid,
                        chunk_sender,
                        chunk_state,
                        position,
                        next_teleport_id,
                        action,
                    )
                    .await?;
                }
                return Ok(CommandOutcome {
                    teleported,
                    ..CommandOutcome::default()
                });
            }
            if crate::commands::is_known_vanilla_command(&name) {
                send_unimplemented_command(sink, &name).await?;
                return Ok(CommandOutcome::default());
            }
            send_unknown_or_incomplete_command(sink, command.to_string()).await?;
            Ok(CommandOutcome::default())
        }
    }
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
                send_unknown_or_incomplete_command(
                    sink,
                    format!("time set {dimension} <value>"),
                )
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
                send_unknown_or_incomplete_command(
                    sink,
                    format!("time add {dimension} <value>"),
                )
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
                send_unknown_or_incomplete_command(
                    sink,
                    "time query <dimension>".to_string(),
                )
                .await?;
                return Ok(());
            };
            if parts.next().is_some() {
                send_unknown_or_incomplete_command(
                    sink,
                    "time query <dimension>".to_string(),
                )
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
        send_unknown_or_incomplete_command(
            sink,
            "gamerule <dimension> <rule> [value]".to_string(),
        )
        .await?;
        return Ok(());
    };
    let Some(rule_name) = parts.next() else {
        send_unknown_or_incomplete_command(
            sink,
            format!("gamerule {dimension} <rule> [value]"),
        )
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
            let entity = match entities.spawn(
                players,
                crate::entities::EntitySpawnRequest {
                    key: key.to_string(),
                    kind,
                    entity_type,
                    dimension: dimension.to_string(),
                    position: player_position,
                    name: name.clone(),
                    display_name: name,
                    skin_textures: String::new(),
                    skin_signature: String::new(),
                    data: 0,
                },
            ) {
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
            if let Err(err) = entities.move_entity(players, key, player_position) {
                send_translatable(
                    sink,
                    "command.exception",
                    vec![text_component(format!("{err:#}"))],
                )
                .await?;
                return Ok(());
            }
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
            if let Err(err) = entities.remove(players, key) {
                send_translatable(
                    sink,
                    "command.exception",
                    vec![text_component(format!("{err:#}"))],
                )
                .await?;
                return Ok(());
            }
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
        "npc" => Some(crate::entities::ManagedEntityKind::Npc),
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
    command: &str,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    sink.send(SystemChat {
        content: text_component(format!(
            "Command is reserved but not implemented yet: /{command}"
        )),
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
    requested_page: Option<usize>,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    const HELP_PAGE_SIZE: usize = 8;

    let mut entries = Vec::new();
    let visible_commands = crate::commands::visible_commands(permissions, profile).await?;
    for command in visible_commands {
        let description = match command {
            "help" => Some("commands.help.usage"),
            "list" => Some("commands.list.usage"),
            "spawn" => Some("commands.spawnpoint.usage"),
            "time" => Some("commands.time.set"),
            "gamerule" => Some("commands.gamerule.set"),
            _ => None,
        };
        entries.push(HelpEntry {
            command: command.to_string(),
            description_key: description.map(ToString::to_string),
            color: None,
        });
    }

    for plugin_command in plugins.plugin_commands() {
        if !permissions
            .can_run_command(profile, &plugin_command.name)
            .await?
        {
            continue;
        }
        entries.push(HelpEntry {
            command: plugin_command.name.clone(),
            description_key: (!plugin_command.description_key.trim().is_empty())
                .then_some(plugin_command.description_key.clone()),
            color: Some("gold".to_string()),
        });
    }

    entries.sort_by(|left, right| left.command.cmp(&right.command));
    let total_pages = entries.len().div_ceil(HELP_PAGE_SIZE).max(1);
    let page = requested_page.unwrap_or(1);
    if page == 0 || page > total_pages {
        send_translatable(sink, "commands.help.failed", Vec::new()).await?;
        return Ok(());
    }

    send_translatable(
        sink,
        "commands.help.header",
        vec![text_component(page.to_string()), text_component(total_pages.to_string())],
    )
    .await?;

    let start = (page - 1) * HELP_PAGE_SIZE;
    let end = (start + HELP_PAGE_SIZE).min(entries.len());
    for entry in &entries[start..end] {
        sink.send(SystemChat {
            content: help_line(
                entry.command.as_str(),
                entry.description_key.as_deref(),
                entry.color.as_deref(),
            ),
            overlay: false,
        })
        .await?;
    }

    Ok(())
}

#[derive(Debug, Clone)]
struct HelpEntry {
    command: String,
    description_key: Option<String>,
    color: Option<String>,
}

fn parse_help_page(argument: &str) -> Option<usize> {
    if argument.is_empty() {
        return Some(1);
    }
    argument.parse::<usize>().ok()
}

fn help_line(command: &str, description_key: Option<&str>, color: Option<&str>) -> qexed_protocol::types::TextComponent {
    let mut root = std::collections::HashMap::new();
    root.insert(
        "text".to_string(),
        qexed_nbt::Tag::String(std::sync::Arc::from(format!("/{command}"))),
    );
    if let Some(color) = color {
        root.insert(
            "color".to_string(),
            qexed_nbt::Tag::String(std::sync::Arc::from(color.to_string())),
        );
    }

    let Some(description_key) = description_key else {
        return qexed_nbt::Tag::Compound(std::sync::Arc::new(root));
    };

    let mut divider = std::collections::HashMap::new();
    divider.insert(
        "text".to_string(),
        qexed_nbt::Tag::String(std::sync::Arc::from(" - ".to_string())),
    );
    divider.insert(
        "color".to_string(),
        qexed_nbt::Tag::String(std::sync::Arc::from("gray".to_string())),
    );

    let mut description = std::collections::HashMap::new();
    description.insert(
        "translate".to_string(),
        qexed_nbt::Tag::String(std::sync::Arc::from(description_key.to_string())),
    );
    description.insert(
        "color".to_string(),
        qexed_nbt::Tag::String(std::sync::Arc::from("gray".to_string())),
    );

    root.insert(
        "extra".to_string(),
        qexed_nbt::Tag::List(
            qexed_nbt::ListHeader {
                tag_id: qexed_nbt::tag_id::COMPOUND,
                length: 2,
            },
            std::sync::Arc::from(
                vec![
                    qexed_nbt::Tag::Compound(std::sync::Arc::new(divider)),
                    qexed_nbt::Tag::Compound(std::sync::Arc::new(description)),
                ]
                .into_boxed_slice(),
            ),
        ),
    );
    qexed_nbt::Tag::Compound(std::sync::Arc::new(root))
}

async fn apply_plugin_action<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    world: &crate::world::WorldManager,
    players: &crate::players::PlayerManager,
    plugins: &crate::plugins::PluginManager,
    actor: uuid::Uuid,
    chunk_sender: &tokio::sync::mpsc::UnboundedSender<super::chunks::ChunkLoadResult>,
    chunk_state: &mut super::ChunkSendState,
    position: &mut EntityPosition,
    next_teleport_id: &mut i32,
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
            x,
            y,
            z,
            yaw,
            pitch,
        } => {
            let teleport_id = *next_teleport_id;
            *next_teleport_id = next_teleport_id.saturating_add(1);
            position.x = x;
            position.y = y;
            position.z = z;
            if let Some(yaw) = yaw {
                position.yaw = yaw;
            }
            if let Some(pitch) = pitch {
                position.pitch = pitch;
            }
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
            chunk_state
                .reset_after_respawn(
                    sink,
                    chunk_sender,
                    world,
                    plugins,
                    position.x,
                    position.z,
                )
                .await?;
            players.update_position(actor, *position);
            Ok(true)
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
    }
}

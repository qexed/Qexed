use anyhow::Result;
use qexed_protocol::to_client::play::add_entity::EntityPosition;
use qexed_protocol::to_client::play::system_chat::SystemChat;

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
    players: &PlayerManager,
    entities: &crate::entities::EntityManager,
    permissions: &crate::permissions::PermissionManager,
    plugins: &crate::plugins::PluginManager,
    profile: &qexed_packet::net_types::GameProfile,
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
    let name = parts.next().unwrap_or_default();
    let argument = parts.collect::<Vec<_>>().join(" ");
    match name {
        "help" => {
            sink.send(SystemChat {
                content: translatable_component("commands.help.failed", Vec::new()),
                overlay: false,
            })
            .await?;
            Ok(CommandOutcome::default())
        }
        "list" => {
            let mut names = players.online_names();
            names.sort();
            let joined = names.join(", ");
            sink.send(SystemChat {
                content: translatable_component(
                    "commands.list.players",
                    vec![
                        text_component(players.online_count().to_string()),
                        text_component(if config.server.max_player < 0 {
                            "unlimited".to_string()
                        } else {
                            config.server.max_player.to_string()
                        }),
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
            sink.send(SystemChat {
                content: translatable_component("command.unknown.command", Vec::new()),
                overlay: false,
            })
            .await?;
            Ok(CommandOutcome::default())
        }
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
        send_entity_usage(sink).await?;
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
                send_entity_usage(sink).await?;
                return Ok(());
            };
            let Some(key) = parts.next() else {
                send_entity_usage(sink).await?;
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
                send_entity_usage(sink).await?;
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
                send_entity_usage(sink).await?;
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
            send_entity_usage(sink).await?;
        }
    }
    Ok(())
}

async fn handle_structure_command<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    world: &crate::world::WorldManager,
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
        send_structure_usage(sink).await?;
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
                send_structure_usage(sink).await?;
                return Ok(());
            };
            let Some(origin) = parse_structure_position(&mut parts, player_position) else {
                send_structure_usage(sink).await?;
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
            let blocks = blocks
                .into_iter()
                .filter(|(position, _)| super::util::can_modify_world(world_config, position))
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
            if world.dynamic_light_enabled() {
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
                send_structure_usage(sink).await?;
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
            send_structure_usage(sink).await?;
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

async fn send_entity_usage<W>(sink: &mut qexed_tcp_connect::PacketSink<W>) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    send_translatable(sink, "command.unknown.argument", Vec::new()).await
}

async fn send_structure_usage<W>(sink: &mut qexed_tcp_connect::PacketSink<W>) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    send_translatable(sink, "command.unknown.argument", Vec::new()).await
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

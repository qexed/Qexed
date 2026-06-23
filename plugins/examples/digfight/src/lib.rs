use qexed_plugin_sdk::{
    BlockDropItem, BlockDropQuery, BlockDropResponse, ConfigReloadPayload, NpcInteractPayload,
    NpcMutationOp, NpcMutationResponse, NpcUpsert, PlayerAction, PlayerAttackQuery,
    PlayerAttackResponse, PlayerPayload, PlayerTickPayload, PluginCommandDefinition,
    PluginCommandQuery, PluginCommandResponse, WorldEditRegion, config_load_or_create,
    config_read_to_string, entity_remove, storage_delete, storage_get_typed, storage_set_typed,
    time_millis, world_register_edit_region, world_set_blocks,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

qexed_plugin_sdk::qexed_plugin_memory!();

const CONFIG_PATH: &str = "config.toml";
const COMMAND_NAME: &str = "digfight";
const NPC_KEY: &str = "digfight:guide";
const NPC_EVENT: &str = "digfight";
const SESSION_PREFIX: &str = "digfight/session/";
const WORLD_EDIT_BATCH_LIMIT: usize = 4096;

// ============ Plugin lifecycle ============

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_priority() -> i32 {
    270
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_init() {
    let _ = config_load_or_create(CONFIG_PATH, DEFAULT_CONFIG);
    let config = load_config();
    register_lobby(&config);
    if config.enable {
        build_lobby(&config);
    }
    qexed_plugin_sdk::log("digfight initialized: 掘一死战");
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_config_reload(ptr: i32, len: i32) {
    let Some(payload) =
        (unsafe { qexed_plugin_sdk::decode_payload::<ConfigReloadPayload>(ptr, len) })
    else {
        return;
    };
    let path = payload.path.replace('\\', "/");
    if path.ends_with("digfight/config.toml") || path.ends_with(CONFIG_PATH) {
        let config = load_config();
        register_lobby(&config);
        if config.enable {
            build_lobby(&config);
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_player_join(ptr: i32, len: i32) {
    let Some(payload) = (unsafe { qexed_plugin_sdk::decode_payload::<PlayerPayload>(ptr, len) })
    else {
        return;
    };
    let _ = storage_delete(&session_key(&payload.uuid));
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_player_leave(ptr: i32, len: i32) {
    let Some(payload) = (unsafe { qexed_plugin_sdk::decode_payload::<PlayerPayload>(ptr, len) })
    else {
        return;
    };
    remove_player_from_game(&payload.uuid);
    let _ = storage_delete(&session_key(&payload.uuid));
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_block_drops(ptr: i32, len: i32) -> i64 {
    let Some(payload) = (unsafe { qexed_plugin_sdk::decode_payload::<BlockDropQuery>(ptr, len) })
    else {
        return qexed_plugin_sdk::response_ptr_len(&BlockDropResponse::default());
    };

    let config = load_config();
    if !config.enable {
        return qexed_plugin_sdk::response_ptr_len(&BlockDropResponse::default());
    }

    let Some(player_ref) = &payload.player else {
        return qexed_plugin_sdk::response_ptr_len(&BlockDropResponse::default());
    };

    let Some(session) = active_session(&player_ref.uuid) else {
        return qexed_plugin_sdk::response_ptr_len(&BlockDropResponse::default());
    };

    let Some(game) = current_game(&session.game_id) else {
        return qexed_plugin_sdk::response_ptr_len(&BlockDropResponse::default());
    };

    if game.phase.is_finished() {
        return qexed_plugin_sdk::response_ptr_len(&BlockDropResponse::default());
    }

    // Check if the broken block is within the arena
    let block_name = payload.block_name.as_str();
    let loot = match pick_loot(&config, block_name) {
        Some(loot) => loot,
        None => return qexed_plugin_sdk::response_ptr_len(&BlockDropResponse::default()),
    };

    qexed_plugin_sdk::response_ptr_len(&BlockDropResponse {
        replace: true,
        items: vec![BlockDropItem {
            item_id: 0,
            item_name: loot,
            count: 1,
        }],
        break_positions: Vec::new(),
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_player_tick(ptr: i32, len: i32) -> i64 {
    let Some(payload) =
        (unsafe { qexed_plugin_sdk::decode_payload::<PlayerTickPayload>(ptr, len) })
    else {
        return qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse::default());
    };

    let config = load_config();
    if !config.enable {
        return qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse::default());
    }

    let Some(session) = active_session(&payload.player.uuid) else {
        return qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse::default());
    };

    let Some(game) = current_game(&session.game_id) else {
        return qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse::default());
    };

    if !session.alive || game.phase.is_finished() {
        return qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse::default());
    }

    let now = time_millis();
    let mut game = game;

    // Phase transitions
    if game.phase == GamePhase::DigPhase && now >= game.phase_end_ms {
        game.phase = GamePhase::FightPhase;
        game.phase_end_ms = now.saturating_add(config.arena.shrink_start_delay_ms);
        game.next_shrink_ms = game.phase_end_ms;
        save_game(&game);

        let _alive_players = alive_player_list(&game);
        return qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse {
            handled: true,
            actions: broadcast_bossbar(&_alive_players, &game, &config),
        });
    }

    if game.phase == GamePhase::FightPhase {
        if now < game.next_shrink_ms || game.current_radius <= config.arena.min_radius.max(2) {
            return qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse::default());
        }

        // Shrink the border
        game.next_shrink_ms = now.saturating_add(config.arena.shrink_interval_ms);
        shrink_border(&config, &mut game);

        // Check if any player fell into void after shrink
        let mut killed_players = Vec::new();
        let min_y = game.origin_y as f64 - 10.0;
        for player_id in &game.player_ids {
            if game.alive_map.get(player_id) != Some(&true) {
                continue;
            }
            if payload.player.uuid == *player_id && payload.position.y < min_y {
                killed_players.push(player_id.clone());
            }
        }

        for player_id in &killed_players {
            game.alive_map.insert(player_id.clone(), false);
            game.death_messages
                .push(format!("{} 掉出了竞技场", player_name(&game, player_id)));
        }

        save_game(&game);
        let alive = alive_player_list(&game);
        let response = check_game_end(&config, &mut game, &alive);
        if response.handled {
            return qexed_plugin_sdk::response_ptr_len(&response);
        }

        let radius = game.current_radius;
        let mut actions = broadcast_bossbar(&alive, &game, &config);
        actions.push(message(format!(
            "边界塌陷！剩余半径 {} 格，存活 {} 人",
            radius,
            alive.len()
        )));
        return qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse {
            handled: true,
            actions,
        });
    }

    qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse::default())
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_player_attack(ptr: i32, len: i32) -> i64 {
    let Some(payload) =
        (unsafe { qexed_plugin_sdk::decode_payload::<PlayerAttackQuery>(ptr, len) })
    else {
        return qexed_plugin_sdk::response_ptr_len(&PlayerAttackResponse::default());
    };

    let config = load_config();
    if !config.enable {
        return qexed_plugin_sdk::response_ptr_len(&PlayerAttackResponse::default());
    }

    let Some(attacker_session) = active_session(&payload.player.uuid) else {
        return qexed_plugin_sdk::response_ptr_len(&PlayerAttackResponse::default());
    };

    let Some(game) = current_game(&attacker_session.game_id) else {
        return qexed_plugin_sdk::response_ptr_len(&PlayerAttackResponse::default());
    };

    if game.phase != GamePhase::FightPhase {
        return qexed_plugin_sdk::response_ptr_len(&PlayerAttackResponse::default());
    }

    // The target_uuid is [u8; 16], need to convert to uuid string
    // For now, we handle kills in player_tick by checking death via fall check
    // The attack event just lets us know damage is happening; actual kill tracking
    // is via player_leave/vanish from the arena
    qexed_plugin_sdk::response_ptr_len(&PlayerAttackResponse::default())
}

// ============ Commands ============

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_commands(_ptr: i32, _len: i32) -> i64 {
    qexed_plugin_sdk::response_ptr_len(&PluginCommandDefinition {
        name: COMMAND_NAME.to_string(),
        description_key: "掘一死战".to_string(),
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_command_execute(ptr: i32, len: i32) -> i64 {
    let Some(payload) =
        (unsafe { qexed_plugin_sdk::decode_payload::<PluginCommandQuery>(ptr, len) })
    else {
        return qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse::default());
    };
    if payload.command != COMMAND_NAME {
        return qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse::default());
    }

    let config = load_config();
    if !config.enable {
        return qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse {
            handled: true,
            actions: vec![message("掘一死战已关闭")],
        });
    }

    let response = match payload
        .argument
        .split_whitespace()
        .next()
        .unwrap_or_default()
    {
        "" | "menu" => PluginCommandResponse {
            handled: true,
            actions: vec![PlayerAction::OpenMenu {
                menu: config.menu_id.clone(),
            }],
        },
        "join" | "queue" => join_queue(&config, &payload.player.uuid, &payload.player.username),
        "leave" | "quit" => leave_queue(&config, &payload.player.uuid),
        "stats" | "record" => show_stats(&payload.player.uuid),
        _ => PluginCommandResponse {
            handled: true,
            actions: vec![message(
                "用法: /digfight join | /digfight leave | /digfight stats",
            )],
        },
    };
    qexed_plugin_sdk::response_ptr_len(&response)
}

// ============ NPC ============

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_npc_mutations(_ptr: i32, _len: i32) -> i64 {
    let config = load_config();
    if !config.enable {
        return qexed_plugin_sdk::response_ptr_len(&NpcMutationResponse::default());
    }
    qexed_plugin_sdk::response_ptr_len(&NpcMutationResponse {
        operations: vec![NpcMutationOp::Upsert {
            npc: NpcUpsert {
                key: NPC_KEY.to_string(),
                dimension: config.lobby.dimension.clone(),
                x: config.lobby.npc_x,
                y: config.lobby.npc_y,
                z: config.lobby.npc_z,
                yaw: config.lobby.npc_yaw,
                pitch: config.lobby.npc_pitch,
                name: "掘一死战".to_string(),
                display_name: config.lobby.npc_display_name.clone(),
                entity_type: config.lobby.npc_entity_type.clone(),
                skin_textures: String::new(),
                skin_signature: String::new(),
                look_at_players: true,
                main_hand_event: NPC_EVENT.to_string(),
                off_hand_event: NPC_EVENT.to_string(),
                attack_event: NPC_EVENT.to_string(),
            },
        }],
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_npc_interact(ptr: i32, len: i32) -> i64 {
    let Some(payload) =
        (unsafe { qexed_plugin_sdk::decode_payload::<NpcInteractPayload>(ptr, len) })
    else {
        return qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse::default());
    };
    if payload.entity.key != NPC_KEY || payload.configured_event != NPC_EVENT {
        return qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse::default());
    }
    qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse {
        handled: true,
        actions: vec![PlayerAction::OpenMenu {
            menu: load_config().menu_id.clone(),
        }],
    })
}

// ============ Queue system ============

fn join_queue(config: &Config, player_uuid: &str, username: &str) -> PluginCommandResponse {
    let existing = active_session(player_uuid);
    if let Some(session) = existing {
        if session.alive {
            if let Some(game) = current_game(&session.game_id) {
                if !game.phase.is_finished() {
                    return PluginCommandResponse {
                        handled: true,
                        actions: vec![message("你已经在游戏中！")],
                    };
                }
            }
        }
    }

    let game_id = {
        let mut runtime = runtime_state().lock().expect("digfight runtime poisoned");
        // Find or create queue game
        let queue_game = runtime
            .games
            .values_mut()
            .find(|game| game.phase == GamePhase::Queuing);

        if let Some(game) = queue_game {
            if game.player_ids.contains(&player_uuid.to_string()) {
                return PluginCommandResponse {
                    handled: true,
                    actions: vec![message("你已经在排队中！")],
                };
            }
            game.player_ids.push(player_uuid.to_string());
            game.player_names
                .insert(player_uuid.to_string(), username.to_string());
            game.alive_map.insert(player_uuid.to_string(), true);
            game.participants.insert(player_uuid.to_string(), true);
            save_game(game);
            game.id.clone()
        } else {
            let id = format!("game_{}", time_millis());
            let mut player_ids = Vec::new();
            player_ids.push(player_uuid.to_string());
            let mut player_names = HashMap::new();
            player_names.insert(player_uuid.to_string(), username.to_string());
            let mut alive_map = HashMap::new();
            alive_map.insert(player_uuid.to_string(), true);
            let mut participants = HashMap::new();
            participants.insert(player_uuid.to_string(), true);
            let game = GameState {
                id: id.clone(),
                phase: GamePhase::Queuing,
                phase_end_ms: 0,
                next_shrink_ms: 0,
                current_radius: 0,
                player_ids,
                player_names,
                alive_map,
                participants,
                origin_x: 0,
                origin_y: 0,
                origin_z: 0,
                death_messages: Vec::new(),
            };
            save_game(&game);
            runtime.games.insert(id.clone(), game);
            id
        }
    };

    let game = current_game(&game_id).unwrap();
    let queued = game.player_ids.len();
    let needed = config.arena.min_players.max(2);

    let _ = storage_set_typed(
        &session_key(player_uuid),
        &PlayerSession {
            game_id: game_id.clone(),
            alive: true,
            kills: 0,
        },
    );

    let mut actions = vec![message(format!(
        "你已加入掘一死战队列 ({}/{})，等待更多玩家...",
        queued, needed
    ))];

    if queued >= needed as usize {
        // Start the game
        drop(game);
        let start_result = start_game(config, &game_id);
        actions.extend(start_result.actions);
    }

    PluginCommandResponse {
        handled: true,
        actions,
    }
}

fn leave_queue(config: &Config, player_uuid: &str) -> PluginCommandResponse {
    let Some(session) = active_session(player_uuid) else {
        return PluginCommandResponse {
            handled: true,
            actions: vec![message("你不在队列或游戏中")],
        };
    };

    let game_id = session.game_id.clone();
    let was_alive = session.alive;
    let _ = storage_delete(&session_key(player_uuid));

    let mut runtime = runtime_state().lock().expect("digfight runtime poisoned");
    if let Some(game) = runtime.games.get_mut(&game_id) {
        if game.phase == GamePhase::Queuing {
            game.player_ids.retain(|id| id != player_uuid);
            game.player_names.remove(player_uuid);
            game.alive_map.remove(player_uuid);
            save_game(game);
            return PluginCommandResponse {
                handled: true,
                actions: vec![message("你已离开队列")],
            };
        }
        if was_alive && !game.phase.is_finished() {
            game.alive_map.insert(player_uuid.to_string(), false);
            game.death_messages.push(format!(
                "{} 退出了游戏",
                game.player_names
                    .get(player_uuid)
                    .cloned()
                    .unwrap_or_else(|| player_uuid.to_string())
            ));
            save_game(game);
            let alive = alive_player_list(game);
            check_game_end(config, game, &alive);
        }
    }

    PluginCommandResponse {
        handled: true,
        actions: vec![
            message("已退出游戏，正在返回大厅"),
            PlayerAction::SetPlayersVisible { visible: true },
            PlayerAction::Teleport {
                dimension: config.lobby.dimension.clone(),
                x: config.lobby.spawn_x,
                y: config.lobby.spawn_y,
                z: config.lobby.spawn_z,
                yaw: Some(config.lobby.spawn_yaw),
                pitch: Some(config.lobby.spawn_pitch),
            },
        ],
    }
}

// ============ Game lifecycle ============

fn start_game(config: &Config, game_id: &str) -> PluginCommandResponse {
    let mut runtime = runtime_state().lock().expect("digfight runtime poisoned");
    let Some(game) = runtime.games.get_mut(game_id) else {
        return PluginCommandResponse::default();
    };

    let origin = (
        config.arena.origin_x,
        config.arena.origin_y,
        config.arena.origin_z,
    );
    game.origin_x = origin.0;
    game.origin_y = origin.1;
    game.origin_z = origin.2;
    game.current_radius = config.arena.initial_radius.max(config.arena.min_radius + 2);

    // Register edit region
    register_arena_region(config, game);

    // Build arena layers
    if !build_arena(config, game) {
        return PluginCommandResponse {
            handled: true,
            actions: vec![message("竞技场建造失败，请稍后重试")],
        };
    }

    let now = time_millis();
    game.phase = GamePhase::DigPhase;
    game.phase_end_ms = now.saturating_add(config.arena.dig_phase_ms);
    save_game(game);

    let player_ids = game.player_ids.clone();
    let _player_names = game.player_names.clone();
    drop(runtime);

    let mut actions = Vec::new();
    let count = player_ids.len();

    // Broadcast to all players
    for (i, player_id) in player_ids.iter().enumerate() {
        // Spread players around the arena center
        let angle = (i as f64) * (std::f64::consts::TAU / count as f64);
        let spawn_r = 3.0;
        let spawn_x = origin.0 as f64 + 0.5 + angle.cos() * spawn_r;
        let spawn_z = origin.2 as f64 + 0.5 + angle.sin() * spawn_r;

        let _ = storage_set_typed(
            &session_key(player_id),
            &PlayerSession {
                game_id: game_id.to_string(),
                alive: true,
                kills: 0,
            },
        );

        actions.push(PlayerAction::Teleport {
            dimension: config.arena.dimension.clone(),
            x: spawn_x,
            y: (origin.1 + config.arena.layer_count.max(0) + 1) as f64,
            z: spawn_z,
            yaw: Some(0.0),
            pitch: Some(0.0),
        });
    }

    let mut broadcast_actions = vec![
        message(format!(
            "掘一死战开始！{} 名玩家进入竞技场，挖掘阶段 {} 秒！",
            count,
            config.arena.dig_phase_ms / 1000
        )),
        PlayerAction::SetPlayersVisible { visible: true },
    ];

    // Add teleport actions for all players
    broadcast_actions.extend(actions);

    PluginCommandResponse {
        handled: true,
        actions: broadcast_actions,
    }
}

fn check_game_end(
    config: &Config,
    game: &mut GameState,
    alive: &[String],
) -> PluginCommandResponse {
    if alive.len() > 1 {
        return PluginCommandResponse::default();
    }

    let winner = alive.first().cloned();
    game.phase = GamePhase::Finished;
    save_game(game);

    let winner_name = winner
        .as_ref()
        .and_then(|id| game.player_names.get(id).cloned())
        .unwrap_or_else(|| "无".to_string());

    let death_log = if game.death_messages.is_empty() {
        String::new()
    } else {
        format!("\n淘汰记录:\n{}", game.death_messages.join("\n"))
    };

    let mut actions = vec![
        PlayerAction::SetPlayersVisible { visible: true },
        message(format!(
            "掘一死战结束！获胜者: {}{}",
            winner_name, death_log
        )),
    ];

    // Teleport all participants back to lobby
    for player_id in &game.participants.keys().cloned().collect::<Vec<_>>() {
        actions.push(PlayerAction::Teleport {
            dimension: config.lobby.dimension.clone(),
            x: config.lobby.spawn_x,
            y: config.lobby.spawn_y,
            z: config.lobby.spawn_z,
            yaw: Some(config.lobby.spawn_yaw),
            pitch: Some(config.lobby.spawn_pitch),
        });
        let _ = storage_delete(&session_key(player_id));
    }

    // Remove runner entity references
    for player_id in game.participants.keys() {
        let _ = entity_remove(&runner_key(player_id));
    }

    PluginCommandResponse {
        handled: true,
        actions,
    }
}

fn remove_player_from_game(player_uuid: &str) {
    let Some(session) = active_session(player_uuid) else {
        return;
    };
    let _ = entity_remove(&runner_key(player_uuid));

    let mut runtime = runtime_state().lock().expect("digfight runtime poisoned");
    if let Some(game) = runtime.games.get_mut(&session.game_id) {
        if session.alive && !game.phase.is_finished() {
            game.alive_map.insert(player_uuid.to_string(), false);
            game.death_messages.push(format!(
                "{} 离开了游戏",
                game.player_names
                    .get(player_uuid)
                    .cloned()
                    .unwrap_or_else(|| player_uuid.to_string())
            ));
            save_game(game);
        }
    }
}

// ============ Arena building ============

fn register_arena_region(config: &Config, game: &GameState) {
    let radius = game.current_radius + 4;
    let height = config.arena.layer_count.max(1) + 4;
    let _ = world_register_edit_region(&WorldEditRegion {
        id: &format!("digfight_{}", game.id),
        dimension: &config.arena.dimension,
        min: (
            game.origin_x - radius,
            game.origin_y - 2,
            game.origin_z - radius,
        ),
        max: (
            game.origin_x + radius,
            game.origin_y + height,
            game.origin_z + radius,
        ),
        allow_player_break: true,
        allow_player_place: false,
        allow_plugin_write: true,
        runtime_only: true,
    });
}

fn build_arena(config: &Config, game: &GameState) -> bool {
    let radius = game.current_radius;
    let layers = config.arena.layer_count.max(1);
    let dim = config.arena.dimension.as_str();
    let mut blocks = Vec::new();
    let origin = (game.origin_x, game.origin_y, game.origin_z);

    // Build layered arena floor
    for dy in 0..=layers {
        let y = origin.1 + dy;
        for dx in -radius..=radius {
            for dz in -radius..=radius {
                // Border wall: only at edge
                if dx.abs() == radius || dz.abs() == radius {
                    // Unbreakable wall (barrier blocks) for border
                    blocks.push((dim, (origin.0 + dx, y, origin.2 + dz), "minecraft:barrier"));
                } else if dy < layers {
                    // Diggable layers
                    let block = match dy {
                        0 => config.arena.deep_layer_block.as_str(),
                        1 => config.arena.mid_layer_block.as_str(),
                        _ => config.arena.top_layer_block.as_str(),
                    };
                    blocks.push((dim, (origin.0 + dx, y, origin.2 + dz), block));
                }
            }
        }
    }

    // Clear air above arena
    for dy in (layers + 1)..=(layers + 6) {
        let y = origin.1 + dy;
        for dx in -radius..=radius {
            for dz in -radius..=radius {
                blocks.push((dim, (origin.0 + dx, y, origin.2 + dz), "minecraft:air"));
            }
        }
    }

    // Scatter special blocks for legendary loot
    let special_count = (radius * radius / 6).max(5) as usize;
    let mut hash = stable_seed(&game.id, 0);
    for _ in 0..special_count {
        hash = hash
            .wrapping_mul(0x100000001b3)
            .wrapping_add(0xcbf29ce484222325);
        let x = ((hash % (radius as u64 * 2)) as i32) - radius;
        hash = hash
            .wrapping_mul(0x100000001b3)
            .wrapping_add(0xcbf29ce484222325);
        let z = ((hash % (radius as u64 * 2)) as i32) - radius;
        let layer = (hash % layers as u64) as i32;
        let y = origin.1 + layer;
        blocks.push((
            dim,
            (origin.0 + x, y, origin.2 + z),
            config.arena.special_block.as_str(),
        ));
    }

    // Send blocks in batches
    for chunk in blocks.chunks(WORLD_EDIT_BATCH_LIMIT) {
        if !world_set_blocks(chunk.iter().map(|(d, p, b)| (*d, *p, *b))) {
            return false;
        }
    }
    true
}

fn shrink_border(config: &Config, game: &mut GameState) {
    let new_radius =
        (game.current_radius - config.arena.shrink_step).max(config.arena.min_radius.max(2));
    if new_radius >= game.current_radius {
        return;
    }
    game.current_radius = new_radius;

    let dim = config.arena.dimension.as_str();
    let origin = (game.origin_x, game.origin_y, game.origin_z);
    let layers = config.arena.layer_count.max(1);

    let mut blocks = Vec::new();
    let old_radius = new_radius + config.arena.shrink_step;
    for dy in 0..=layers + 2 {
        let y = origin.1 + dy;
        for r in new_radius..old_radius {
            for sign in [-1, 1] {
                // Top and bottom edges
                for dx in -r..=r {
                    blocks.push((
                        dim,
                        (origin.0 + dx, y, origin.2 + sign * r),
                        "minecraft:air",
                    ));
                }
                // Left and right edges (excluding corners already handled)
                for dz in -(r - 1)..=(r - 1) {
                    blocks.push((
                        dim,
                        (origin.0 + sign * r, y, origin.2 + dz),
                        "minecraft:air",
                    ));
                }
            }
        }
        // Place barrier wall at new border
        if dy <= layers {
            for dx in -new_radius..=new_radius {
                blocks.push((
                    dim,
                    (origin.0 + dx, y, origin.2 + new_radius),
                    "minecraft:barrier",
                ));
                blocks.push((
                    dim,
                    (origin.0 + dx, y, origin.2 - new_radius),
                    "minecraft:barrier",
                ));
            }
            for dz in -(new_radius - 1)..=(new_radius - 1) {
                blocks.push((
                    dim,
                    (origin.0 + new_radius, y, origin.2 + dz),
                    "minecraft:barrier",
                ));
                blocks.push((
                    dim,
                    (origin.0 - new_radius, y, origin.2 + dz),
                    "minecraft:barrier",
                ));
            }
        }
    }

    for chunk in blocks.chunks(WORLD_EDIT_BATCH_LIMIT) {
        let _ = world_set_blocks(chunk.iter().map(|(d, p, b)| (*d, *p, *b)));
    }
}

// ============ Loot system ============

fn pick_loot(config: &Config, block_name: &str) -> Option<String> {
    if block_name == config.arena.special_block {
        Some(lottery_roll_single(&config.loot.special_items)?)
    } else if block_name == config.arena.top_layer_block {
        Some(lottery_roll_single(&config.loot.common_items)?)
    } else if block_name == config.arena.mid_layer_block {
        Some(lottery_roll_single(&config.loot.uncommon_items)?)
    } else if block_name == config.arena.deep_layer_block {
        Some(lottery_roll_single(&config.loot.rare_items)?)
    } else {
        None
    }
}

fn lottery_roll_single(items: &[String]) -> Option<String> {
    if items.is_empty() {
        return None;
    }
    let mut entries: Vec<(String, u64)> = Vec::new();
    for item in items {
        let mut parts = item.split(':');
        let Some(item_name) = parts.next() else {
            continue;
        };
        let Some(weight) = parts.next().and_then(|w| w.parse::<u64>().ok()) else {
            continue;
        };
        entries.push((item_name.to_string(), weight));
    }
    if entries.is_empty() {
        return None;
    }

    let total: u64 = entries.iter().map(|(_, w)| w).sum();
    if total == 0 {
        return entries.first().map(|(name, _)| name.clone());
    }
    let seed = time_millis() as u64;
    let mut roll = seed.wrapping_mul(0x9e3779b97f4a7c15) % total;
    for (name, weight) in &entries {
        if roll < *weight {
            return Some(name.clone());
        }
        roll -= *weight;
    }
    entries.last().map(|(name, _)| name.clone())
}

// ============ Lobby building ============

fn register_lobby(config: &Config) {
    let y = config.lobby.floor_y;
    let _ = world_register_edit_region(&WorldEditRegion {
        id: "digfight_lobby",
        dimension: &config.lobby.dimension,
        min: (-12, y - 1, -12),
        max: (12, y + 6, 12),
        allow_player_break: false,
        allow_player_place: false,
        allow_plugin_write: true,
        runtime_only: true,
    });
}

fn build_lobby(config: &Config) {
    let y = config.lobby.floor_y;
    let mut blocks = Vec::new();
    for x in -5..=5 {
        for z in -5..=5 {
            blocks.push((
                config.lobby.dimension.as_str(),
                (x, y - 1, z),
                "minecraft:polished_andesite",
            ));
            for air_y in y..=y + 4 {
                blocks.push((
                    config.lobby.dimension.as_str(),
                    (x, air_y, z),
                    "minecraft:air",
                ));
            }
        }
    }
    blocks.push((
        config.lobby.dimension.as_str(),
        (0, y, 0),
        "minecraft:sea_lantern",
    ));
    let _ = world_set_blocks(blocks);
}

// ============ Helpers ============

fn current_game(game_id: &str) -> Option<GameState> {
    let runtime = runtime_state().lock().expect("digfight runtime poisoned");
    runtime.games.get(game_id).cloned()
}

fn save_game(game: &GameState) {
    let mut runtime = runtime_state().lock().expect("digfight runtime poisoned");
    runtime.games.insert(game.id.clone(), game.clone());
}

fn active_session(player_uuid: &str) -> Option<PlayerSession> {
    storage_get_typed::<PlayerSession>(&session_key(player_uuid))
}

fn alive_player_list(game: &GameState) -> Vec<String> {
    game.player_ids
        .iter()
        .filter(|id| game.alive_map.get(*id) == Some(&true))
        .cloned()
        .collect()
}

fn broadcast_bossbar(alive: &[String], game: &GameState, _config: &Config) -> Vec<PlayerAction> {
    let _phase_text = match game.phase {
        GamePhase::DigPhase => "挖掘阶段",
        GamePhase::FightPhase => "战斗阶段",
        _ => "",
    };
    let _title = format!(
        "掘一死战 | {} | {} | 存活 {} 人 | 半径 {}",
        _phase_text,
        format_countdown(game.phase_end_ms),
        alive.len(),
        game.current_radius
    );
    Vec::new() // BossBar actions would go here, but they're per-player
}

fn format_countdown(end_ms: i64) -> String {
    let remaining = (end_ms - time_millis()).max(0) / 1000;
    format!("{}:{:02}", remaining / 60, remaining % 60)
}

fn player_name(game: &GameState, player_id: &str) -> String {
    game.player_names
        .get(player_id)
        .cloned()
        .unwrap_or_else(|| player_id.to_string())
}

fn show_stats(player_uuid: &str) -> PluginCommandResponse {
    let session = active_session(player_uuid);
    match session {
        Some(s) => PluginCommandResponse {
            handled: true,
            actions: vec![message(format!(
                "你在游戏 [{}] 中，状态: {}，击杀: {}",
                s.game_id,
                if s.alive { "存活" } else { "已淘汰" },
                s.kills
            ))],
        },
        None => PluginCommandResponse {
            handled: true,
            actions: vec![message("你当前不在游戏中")],
        },
    }
}

// ============ Key helpers ============

fn session_key(player_uuid: &str) -> String {
    format!("{SESSION_PREFIX}{}", player_uuid.replace('-', ""))
}

fn runner_key(player_uuid: &str) -> String {
    format!("digfight/runner/{}", player_uuid.replace('-', ""))
}

fn stable_seed(value: &str, salt: u64) -> u64 {
    let mut hash = 0xcbf29ce484222325u64 ^ salt;
    for byte in value.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn message(text: impl Into<String>) -> PlayerAction {
    PlayerAction::SystemMessage {
        text: text.into(),
        translate: String::new(),
        with: Vec::new(),
        overlay: false,
    }
}

fn load_config() -> Config {
    config_read_to_string(CONFIG_PATH)
        .and_then(|content| toml::from_str::<Config>(&content).ok())
        .unwrap_or_default()
}

// ============ Runtime state ============

#[derive(Default)]
struct RuntimeState {
    games: HashMap<String, GameState>,
}

fn runtime_state() -> &'static Mutex<RuntimeState> {
    static STATE: OnceLock<Mutex<RuntimeState>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(RuntimeState::default()))
}

// ============ Data types ============

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PlayerSession {
    game_id: String,
    alive: bool,
    kills: i32,
}

#[derive(Debug, Clone)]
struct GameState {
    id: String,
    phase: GamePhase,
    phase_end_ms: i64,
    next_shrink_ms: i64,
    current_radius: i32,
    player_ids: Vec<String>,
    player_names: HashMap<String, String>,
    alive_map: HashMap<String, bool>,
    participants: HashMap<String, bool>,
    origin_x: i32,
    origin_y: i32,
    origin_z: i32,
    death_messages: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GamePhase {
    Queuing,
    DigPhase,
    FightPhase,
    Finished,
}

impl GamePhase {
    fn is_finished(self) -> bool {
        matches!(self, GamePhase::Finished)
    }
}

// ============ Config ============

#[derive(Debug, Clone, Deserialize)]
struct Config {
    #[serde(default = "default_true")]
    enable: bool,
    #[serde(default = "default_menu_id")]
    menu_id: String,
    #[serde(default)]
    lobby: LobbyConfig,
    #[serde(default)]
    arena: ArenaConfig,
    #[serde(default)]
    loot: LootConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enable: true,
            menu_id: default_menu_id(),
            lobby: LobbyConfig::default(),
            arena: ArenaConfig::default(),
            loot: LootConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct LobbyConfig {
    #[serde(default = "default_lobby_dimension")]
    dimension: String,
    #[serde(default)]
    spawn_x: f64,
    #[serde(default = "default_lobby_y")]
    spawn_y: f64,
    #[serde(default)]
    spawn_z: f64,
    #[serde(default)]
    spawn_yaw: f32,
    #[serde(default)]
    spawn_pitch: f32,
    #[serde(default = "default_lobby_floor_y")]
    floor_y: i32,
    #[serde(default)]
    npc_x: f64,
    #[serde(default = "default_lobby_y")]
    npc_y: f64,
    #[serde(default = "default_npc_z")]
    npc_z: f64,
    #[serde(default = "default_npc_yaw")]
    npc_yaw: f32,
    #[serde(default)]
    npc_pitch: f32,
    #[serde(default = "default_npc_entity_type")]
    npc_entity_type: String,
    #[serde(default = "default_npc_display_name")]
    npc_display_name: String,
}

impl Default for LobbyConfig {
    fn default() -> Self {
        Self {
            dimension: default_lobby_dimension(),
            spawn_x: 0.5,
            spawn_y: default_lobby_y(),
            spawn_z: 0.5,
            spawn_yaw: 0.0,
            spawn_pitch: 0.0,
            floor_y: default_lobby_floor_y(),
            npc_x: 0.0,
            npc_y: default_lobby_y(),
            npc_z: default_npc_z(),
            npc_yaw: default_npc_yaw(),
            npc_pitch: 0.0,
            npc_entity_type: default_npc_entity_type(),
            npc_display_name: default_npc_display_name(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct ArenaConfig {
    #[serde(default = "default_arena_dimension")]
    dimension: String,
    #[serde(default = "default_arena_origin_x")]
    origin_x: i32,
    #[serde(default = "default_arena_origin_y")]
    origin_y: i32,
    #[serde(default = "default_arena_origin_z")]
    origin_z: i32,
    #[serde(default = "default_min_players")]
    min_players: i32,
    #[serde(default = "default_initial_radius")]
    initial_radius: i32,
    #[serde(default = "default_min_radius")]
    min_radius: i32,
    #[serde(default = "default_shrink_step")]
    shrink_step: i32,
    #[serde(default = "default_shrink_interval_ms")]
    shrink_interval_ms: i64,
    #[serde(default = "default_shrink_start_delay_ms")]
    shrink_start_delay_ms: i64,
    #[serde(default = "default_dig_phase_ms")]
    dig_phase_ms: i64,
    #[serde(default = "default_layer_count")]
    layer_count: i32,
    #[serde(default = "default_top_layer_block")]
    top_layer_block: String,
    #[serde(default = "default_mid_layer_block")]
    mid_layer_block: String,
    #[serde(default = "default_deep_layer_block")]
    deep_layer_block: String,
    #[serde(default = "default_special_block")]
    special_block: String,
}

impl Default for ArenaConfig {
    fn default() -> Self {
        Self {
            dimension: default_arena_dimension(),
            origin_x: default_arena_origin_x(),
            origin_y: default_arena_origin_y(),
            origin_z: default_arena_origin_z(),
            min_players: default_min_players(),
            initial_radius: default_initial_radius(),
            min_radius: default_min_radius(),
            shrink_step: default_shrink_step(),
            shrink_interval_ms: default_shrink_interval_ms(),
            shrink_start_delay_ms: default_shrink_start_delay_ms(),
            dig_phase_ms: default_dig_phase_ms(),
            layer_count: default_layer_count(),
            top_layer_block: default_top_layer_block(),
            mid_layer_block: default_mid_layer_block(),
            deep_layer_block: default_deep_layer_block(),
            special_block: default_special_block(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct LootConfig {
    #[serde(default = "default_common_items")]
    common_items: Vec<String>,
    #[serde(default = "default_uncommon_items")]
    uncommon_items: Vec<String>,
    #[serde(default = "default_rare_items")]
    rare_items: Vec<String>,
    #[serde(default = "default_special_items")]
    special_items: Vec<String>,
}

impl Default for LootConfig {
    fn default() -> Self {
        Self {
            common_items: default_common_items(),
            uncommon_items: default_uncommon_items(),
            rare_items: default_rare_items(),
            special_items: default_special_items(),
        }
    }
}

// ============ Default value functions ============

fn default_true() -> bool {
    true
}
fn default_menu_id() -> String {
    "digfight".to_string()
}

fn default_lobby_dimension() -> String {
    "qexed:digfight_lobby".to_string()
}
fn default_lobby_y() -> f64 {
    -60.0
}
fn default_lobby_floor_y() -> i32 {
    -60
}
fn default_npc_z() -> f64 {
    3.5
}
fn default_npc_yaw() -> f32 {
    180.0
}
fn default_npc_entity_type() -> String {
    "minecraft:villager".to_string()
}
fn default_npc_display_name() -> String {
    "{\"text\":\"掘一死战\",\"color\":\"gold\"}".to_string()
}

fn default_arena_dimension() -> String {
    "qexed:digfight".to_string()
}
fn default_arena_origin_x() -> i32 {
    2048
}
fn default_arena_origin_y() -> i32 {
    -60
}
fn default_arena_origin_z() -> i32 {
    2048
}
fn default_min_players() -> i32 {
    4
}
fn default_initial_radius() -> i32 {
    12
}
fn default_min_radius() -> i32 {
    3
}
fn default_shrink_step() -> i32 {
    1
}
fn default_shrink_interval_ms() -> i64 {
    15000
}
fn default_shrink_start_delay_ms() -> i64 {
    5000
}
fn default_dig_phase_ms() -> i64 {
    60000
}
fn default_layer_count() -> i32 {
    3
}
fn default_top_layer_block() -> String {
    "minecraft:sand".to_string()
}
fn default_mid_layer_block() -> String {
    "minecraft:dirt".to_string()
}
fn default_deep_layer_block() -> String {
    "minecraft:stone".to_string()
}
fn default_special_block() -> String {
    "minecraft:diamond_block".to_string()
}

fn default_common_items() -> Vec<String> {
    vec![
        "minecraft:leather_helmet:20".to_string(),
        "minecraft:leather_chestplate:20".to_string(),
        "minecraft:leather_leggings:20".to_string(),
        "minecraft:leather_boots:20".to_string(),
        "minecraft:wooden_sword:30".to_string(),
        "minecraft:stone_sword:15".to_string(),
        "minecraft:stone_pickaxe:10".to_string(),
        "minecraft:bow:5".to_string(),
        "minecraft:arrow:20".to_string(),
        "minecraft:bread:15".to_string(),
        "minecraft:cooked_porkchop:10".to_string(),
    ]
}

fn default_uncommon_items() -> Vec<String> {
    vec![
        "minecraft:iron_helmet:20".to_string(),
        "minecraft:iron_chestplate:20".to_string(),
        "minecraft:iron_leggings:20".to_string(),
        "minecraft:iron_boots:20".to_string(),
        "minecraft:iron_sword:30".to_string(),
        "minecraft:iron_pickaxe:15".to_string(),
        "minecraft:shield:10".to_string(),
        "minecraft:bow:15".to_string(),
        "minecraft:arrow:25".to_string(),
        "minecraft:cooked_beef:15".to_string(),
        "minecraft:golden_apple:3".to_string(),
    ]
}

fn default_rare_items() -> Vec<String> {
    vec![
        "minecraft:diamond_helmet:20".to_string(),
        "minecraft:diamond_chestplate:20".to_string(),
        "minecraft:diamond_leggings:20".to_string(),
        "minecraft:diamond_boots:20".to_string(),
        "minecraft:diamond_sword:25".to_string(),
        "minecraft:diamond_pickaxe:15".to_string(),
        "minecraft:diamond_axe:10".to_string(),
        "minecraft:shield:10".to_string(),
        "minecraft:bow:15".to_string(),
        "minecraft:arrow:25".to_string(),
        "minecraft:golden_apple:10".to_string(),
        "minecraft:ender_pearl:5".to_string(),
    ]
}

fn default_special_items() -> Vec<String> {
    vec![
        "minecraft:diamond_sword:15".to_string(),
        "minecraft:diamond_chestplate:15".to_string(),
        "minecraft:netherite_sword:5".to_string(),
        "minecraft:enchanted_golden_apple:15".to_string(),
        "minecraft:golden_apple:20".to_string(),
        "minecraft:ender_pearl:15".to_string(),
        "minecraft:shield:10".to_string(),
        "minecraft:bow:10".to_string(),
        "minecraft:trident:5".to_string(),
        "minecraft:totem_of_undying:2".to_string(),
    ]
}

const DEFAULT_CONFIG: &str = r#"enable = true
menu_id = "digfight"

[lobby]
dimension = "qexed:digfight_lobby"
spawn_x = 0.5
spawn_y = -60.0
spawn_z = 0.5
spawn_yaw = 0.0
spawn_pitch = 0.0
floor_y = -60
npc_x = 0.0
npc_y = -60.0
npc_z = 3.5
npc_yaw = 180.0
npc_pitch = 0.0
npc_entity_type = "minecraft:villager"
npc_display_name = "{\"text\":\"掘一死战\",\"color\":\"gold\"}"

[arena]
dimension = "qexed:digfight"
origin_x = 2048
origin_y = -60
origin_z = 2048
min_players = 4
initial_radius = 12
min_radius = 3
shrink_step = 1
shrink_interval_ms = 15000
shrink_start_delay_ms = 5000
dig_phase_ms = 60000
layer_count = 3
top_layer_block = "minecraft:sand"
mid_layer_block = "minecraft:dirt"
deep_layer_block = "minecraft:stone"
special_block = "minecraft:diamond_block"

[loot]
common_items = [
    "minecraft:leather_helmet:20",
    "minecraft:leather_chestplate:20",
    "minecraft:leather_leggings:20",
    "minecraft:leather_boots:20",
    "minecraft:wooden_sword:30",
    "minecraft:stone_sword:15",
    "minecraft:stone_pickaxe:10",
    "minecraft:bow:5",
    "minecraft:arrow:20",
    "minecraft:bread:15",
    "minecraft:cooked_porkchop:10",
]
uncommon_items = [
    "minecraft:iron_helmet:20",
    "minecraft:iron_chestplate:20",
    "minecraft:iron_leggings:20",
    "minecraft:iron_boots:20",
    "minecraft:iron_sword:30",
    "minecraft:iron_pickaxe:15",
    "minecraft:shield:10",
    "minecraft:bow:15",
    "minecraft:arrow:25",
    "minecraft:cooked_beef:15",
    "minecraft:golden_apple:3",
]
rare_items = [
    "minecraft:diamond_helmet:20",
    "minecraft:diamond_chestplate:20",
    "minecraft:diamond_leggings:20",
    "minecraft:diamond_boots:20",
    "minecraft:diamond_sword:25",
    "minecraft:diamond_pickaxe:15",
    "minecraft:diamond_axe:10",
    "minecraft:shield:10",
    "minecraft:bow:15",
    "minecraft:arrow:25",
    "minecraft:golden_apple:10",
    "minecraft:ender_pearl:5",
]
special_items = [
    "minecraft:diamond_sword:15",
    "minecraft:diamond_chestplate:15",
    "minecraft:netherite_sword:5",
    "minecraft:enchanted_golden_apple:15",
    "minecraft:golden_apple:20",
    "minecraft:ender_pearl:15",
    "minecraft:shield:10",
    "minecraft:bow:10",
    "minecraft:trident:5",
    "minecraft:totem_of_undying:2",
]
"#;

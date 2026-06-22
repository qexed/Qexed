use qexed_plugin_sdk::{
    ConfigReloadPayload, ItemStackPayload, NpcInteractPayload, NpcMutationOp, NpcMutationResponse,
    NpcUpsert, PlayerAction, PlayerAttackQuery, PlayerAttackResponse, PlayerPayload,
    PlayerTickPayload, PlayerUseItemPayload, PluginCommandDefinition, PluginCommandQuery,
    PluginCommandResponse, ProjectileHitPlayerPayload, WorldEditRegion, config_load_or_create,
    config_read_to_string, storage_delete, storage_get_typed, storage_set_typed, time_millis,
    world_register_edit_region, world_set_blocks,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

qexed_plugin_sdk::qexed_plugin_memory!();

const CONFIG_PATH: &str = "config.toml";
const COMMAND_NAME: &str = "murder";
const NPC_KEY: &str = "murder:guide";
const NPC_EVENT: &str = "murder";
const DETECTIVE_ARROW_EVENT: &str = "murder:detective_arrow";
const SESSION_PREFIX: &str = "murder/session/";
const BOSS_BAR_ID: &str = "murder:state";

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_priority() -> i32 {
    320
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_init() {
    let _ = config_load_or_create(CONFIG_PATH, DEFAULT_CONFIG);
    let config = load_config();
    register_regions(&config);
    if config.build_lobby || config.build_arena {
        build_map(&config);
    }
    qexed_plugin_sdk::log("murder_mystery initialized");
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_config_reload(ptr: i32, len: i32) {
    let Some(payload) =
        (unsafe { qexed_plugin_sdk::decode_payload::<ConfigReloadPayload>(ptr, len) })
    else {
        return;
    };
    let path = payload.path.replace('\\', "/");
    if path.ends_with("murder_mystery/config.toml") || path.ends_with(CONFIG_PATH) {
        let config = load_config();
        register_regions(&config);
        if config.build_lobby || config.build_arena {
            build_map(&config);
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
    remove_player(&payload.uuid);
    let _ = storage_delete(&session_key(&payload.uuid));
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_commands(_ptr: i32, _len: i32) -> i64 {
    qexed_plugin_sdk::response_ptr_len(&PluginCommandDefinition {
        name: COMMAND_NAME.to_string(),
        description_key: "谁是凶手".to_string(),
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
    let response = handle_command(&config, &payload);
    qexed_plugin_sdk::response_ptr_len(&response)
}

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
                name: "谁是凶手".to_string(),
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
    let config = load_config();
    qexed_plugin_sdk::response_ptr_len(&open_menu(&config))
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_player_tick(ptr: i32, len: i32) -> i64 {
    let Some(payload) =
        (unsafe { qexed_plugin_sdk::decode_payload::<PlayerTickPayload>(ptr, len) })
    else {
        return qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse::default());
    };
    let config = load_config();
    let response = handle_player_tick(&config, &payload);
    qexed_plugin_sdk::response_ptr_len(&response)
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_player_attack(ptr: i32, len: i32) -> i64 {
    let Some(payload) =
        (unsafe { qexed_plugin_sdk::decode_payload::<PlayerAttackQuery>(ptr, len) })
    else {
        return qexed_plugin_sdk::response_ptr_len(&PlayerAttackResponse::default());
    };
    let config = load_config();
    let response = handle_player_attack(&config, &payload);
    qexed_plugin_sdk::response_ptr_len(&response)
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_player_use_item(ptr: i32, len: i32) -> i64 {
    let Some(payload) =
        (unsafe { qexed_plugin_sdk::decode_payload::<PlayerUseItemPayload>(ptr, len) })
    else {
        return qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse::default());
    };
    let config = load_config();
    let response = handle_player_use_item(&config, &payload);
    qexed_plugin_sdk::response_ptr_len(&response)
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_projectile_hit_player(ptr: i32, len: i32) -> i64 {
    let Some(payload) =
        (unsafe { qexed_plugin_sdk::decode_payload::<ProjectileHitPlayerPayload>(ptr, len) })
    else {
        return qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse::default());
    };
    let config = load_config();
    let response = handle_projectile_hit_player(&config, &payload);
    qexed_plugin_sdk::response_ptr_len(&response)
}

fn handle_command(config: &Config, payload: &PluginCommandQuery) -> PluginCommandResponse {
    if !config.enable {
        return handled(vec![message("谁是凶手已关闭")]);
    }
    let mut parts = payload.argument.split_whitespace();
    match parts.next().unwrap_or_default() {
        "" | "menu" | "select" => open_menu(config),
        "match" | "queue" | "join" | "play" => {
            join_queue(config, &payload.player.uuid, &payload.player.username)
        }
        "leave" | "quit" | "lobby" => leave_game(config, &payload.player.uuid),
        "start" => force_start(config),
        "status" => status(config, &payload.player.uuid),
        _ => handled(vec![message(
            "用法: /murder match | /murder leave | /murder start | /murder status",
        )]),
    }
}

fn open_menu(config: &Config) -> PluginCommandResponse {
    handled(vec![PlayerAction::OpenMenu {
        menu: config.menu_id.clone(),
    }])
}

fn join_queue(config: &Config, player_uuid: &str, username: &str) -> PluginCommandResponse {
    let mut state = runtime_state().lock().expect("murder runtime poisoned");
    if let Some(game) = state.active_game_mut() {
        if game.players.iter().any(|player| player.uuid == player_uuid) {
            return handled(vec![message("你已经在谁是凶手房间中")]);
        }
        if !matches!(game.phase, GamePhase::Waiting) {
            return handled(vec![message("当前对局已开始，请稍后加入下一局")]);
        }
        game.players.push(PlayerState::new(player_uuid, username));
    } else {
        let mut game = GameState::new(format!("murder_{}", time_millis()));
        game.players.push(PlayerState::new(player_uuid, username));
        state.games.insert(game.id.clone(), game);
    }
    let game = state.active_game().expect("active murder game exists");
    let count = game.players.len();
    let start_now = count >= config.game.min_players.max(2) as usize;
    let game_id = game.id.clone();
    drop(state);

    let _ = storage_set_typed(
        &session_key(player_uuid),
        &PlayerSession {
            game_id: game_id.clone(),
            role: Role::Innocent,
            alive: true,
            entered_game: false,
            kit_given: false,
            finished: false,
            last_shot_ms: 0,
            last_x: config.lobby.spawn_x,
            last_y: config.lobby.spawn_y,
            last_z: config.lobby.spawn_z,
            last_yaw: 0.0,
            last_pitch: 0.0,
        },
    );

    let mut actions = vec![
        message(format!(
            "已加入谁是凶手 ({}/{})",
            count,
            config.game.min_players.max(2)
        )),
        PlayerAction::Teleport {
            dimension: config.lobby.dimension.clone(),
            x: config.lobby.spawn_x,
            y: config.lobby.spawn_y,
            z: config.lobby.spawn_z,
            yaw: Some(config.lobby.spawn_yaw),
            pitch: Some(config.lobby.spawn_pitch),
        },
    ];
    if start_now {
        actions.extend(start_game(config, &game_id).actions);
    }
    handled(actions)
}

fn force_start(config: &Config) -> PluginCommandResponse {
    let state = runtime_state().lock().expect("murder runtime poisoned");
    let Some(game) = state.active_game() else {
        return handled(vec![message("当前没有等待中的房间")]);
    };
    if game.players.len() < 2 {
        return handled(vec![message("至少需要 2 名玩家")]);
    }
    let game_id = game.id.clone();
    drop(state);
    start_game(config, &game_id)
}

fn start_game(config: &Config, game_id: &str) -> PluginCommandResponse {
    let mut state = runtime_state().lock().expect("murder runtime poisoned");
    let Some(game) = state.games.get_mut(game_id) else {
        return PluginCommandResponse::default();
    };
    if !matches!(game.phase, GamePhase::Waiting) {
        return PluginCommandResponse::default();
    }
    let now = time_millis();
    let murderer_index = (fnv1a64(game_id.as_bytes()) as usize) % game.players.len();
    let detective_index = if game.players.len() > 2 {
        (murderer_index + 1 + (now.unsigned_abs() as usize % (game.players.len() - 1)))
            % game.players.len()
    } else {
        1 - murderer_index
    };
    for (index, player) in game.players.iter_mut().enumerate() {
        player.role = if index == murderer_index {
            Role::Murderer
        } else if index == detective_index {
            Role::Detective
        } else {
            Role::Innocent
        };
        player.alive = true;
        player.entered_game = false;
        player.kit_given = false;
    }
    game.phase = GamePhase::Running;
    game.started_at_ms = now;
    game.ends_at_ms = now.saturating_add(config.game.duration_ms.max(30_000));
    let players = game.players.clone();
    drop(state);

    let actions = vec![message("谁是凶手开始！身份已发放。")];
    for player in players {
        let _ = storage_set_typed(
            &session_key(&player.uuid),
            &PlayerSession {
                game_id: game_id.to_string(),
                role: player.role,
                alive: true,
                entered_game: false,
                kit_given: false,
                finished: false,
                last_shot_ms: 0,
                last_x: config.game.center_x,
                last_y: config.game.spawn_y,
                last_z: config.game.center_z,
                last_yaw: 0.0,
                last_pitch: 0.0,
            },
        );
    }
    handled(actions)
}

fn leave_game(config: &Config, player_uuid: &str) -> PluginCommandResponse {
    remove_player(player_uuid);
    let _ = storage_delete(&session_key(player_uuid));
    handled(vec![
        PlayerAction::SetPlayersVisible { visible: true },
        PlayerAction::Teleport {
            dimension: config.lobby.dimension.clone(),
            x: config.lobby.spawn_x,
            y: config.lobby.spawn_y,
            z: config.lobby.spawn_z,
            yaw: Some(config.lobby.spawn_yaw),
            pitch: Some(config.lobby.spawn_pitch),
        },
        reset_inventory_action(),
        PlayerAction::RemoveBossBar {
            id: BOSS_BAR_ID.to_string(),
        },
        message("已离开谁是凶手"),
    ])
}

fn status(config: &Config, player_uuid: &str) -> PluginCommandResponse {
    let state = runtime_state().lock().expect("murder runtime poisoned");
    let Some(game) = state.game_for_player(player_uuid) else {
        return handled(vec![message("你当前不在谁是凶手房间")]);
    };
    let alive = game.alive_count();
    let text = match game.phase {
        GamePhase::Waiting => format!(
            "等待中: {}/{}",
            game.players.len(),
            config.game.min_players.max(2)
        ),
        GamePhase::Running => format!("进行中: 存活 {alive} 人"),
        GamePhase::Finished => "已结束".to_string(),
    };
    handled(vec![message(text)])
}

fn handle_player_tick(config: &Config, payload: &PlayerTickPayload) -> PluginCommandResponse {
    let Some(mut session) = storage_get_typed::<PlayerSession>(&session_key(&payload.player.uuid))
    else {
        return PluginCommandResponse::default();
    };
    session.last_x = payload.position.x;
    session.last_y = payload.position.y;
    session.last_z = payload.position.z;
    session.last_yaw = payload.position.yaw;
    session.last_pitch = payload.position.pitch;

    let mut state = runtime_state().lock().expect("murder runtime poisoned");
    let Some(game) = state.games.get_mut(&session.game_id) else {
        let _ = storage_delete(&session_key(&payload.player.uuid));
        return PluginCommandResponse::default();
    };
    if let Some(player) = game.player_mut(&payload.player.uuid) {
        player.entity_id = payload.player.entity_id;
        player.dimension = payload.dimension.clone();
        player.x = payload.position.x;
        player.y = payload.position.y;
        player.z = payload.position.z;
        player.yaw = payload.position.yaw;
        player.pitch = payload.position.pitch;
    }
    if matches!(game.phase, GamePhase::Finished) {
        let reason = game
            .finish_reason
            .clone()
            .unwrap_or_else(|| "对局已结束".to_string());
        drop(state);
        return finish_player(config, &payload.player.uuid, &mut session, &reason);
    }
    if !matches!(game.phase, GamePhase::Running) {
        let _ = storage_set_typed(&session_key(&payload.player.uuid), &session);
        return PluginCommandResponse::default();
    }

    let now = time_millis();
    if now >= game.ends_at_ms {
        game.phase = GamePhase::Finished;
        game.finish_reason = Some("时间到，平民获胜".to_string());
        let reason = game.finish_reason.clone().expect("finish reason set");
        drop(state);
        return finish_player(config, &payload.player.uuid, &mut session, &reason);
    }
    if session.alive && !game.is_alive(&payload.player.uuid) {
        session.alive = false;
        let _ = storage_set_typed(&session_key(&payload.player.uuid), &session);
        drop(state);
        return handled(eliminate_actions(config, "你被淘汰了，已返回大厅"));
    }
    if !session.alive {
        let _ = storage_set_typed(&session_key(&payload.player.uuid), &session);
        return PluginCommandResponse::default();
    }

    let mut actions = Vec::new();
    if !session.entered_game {
        session.entered_game = true;
        let spawn = spawn_position_for_player(config, game, &payload.player.uuid);
        actions.extend(enter_game_actions(config, spawn));
        if let Some(player) = game.player_mut(&payload.player.uuid) {
            player.entered_game = true;
        }
    }
    if session.alive && session.entered_game && !session.kit_given {
        session.kit_given = true;
        if let Some(player) = game.player_mut(&payload.player.uuid) {
            player.kit_given = true;
        }
        actions.extend(role_intro_actions(config, session.role));
    }
    let _ = storage_set_typed(&session_key(&payload.player.uuid), &session);
    actions.push(progress_bar(config, game, session.role));
    PluginCommandResponse {
        handled: !actions.is_empty(),
        actions,
    }
}

fn handle_player_attack(config: &Config, payload: &PlayerAttackQuery) -> PlayerAttackResponse {
    let Some(session) = storage_get_typed::<PlayerSession>(&session_key(&payload.player.uuid))
    else {
        return PlayerAttackResponse::default();
    };
    if session.role != Role::Murderer || !session.alive {
        return PlayerAttackResponse::default();
    }
    if !is_murder_weapon(&payload.weapon, &config.items.murder_weapon) {
        return PlayerAttackResponse::default();
    }

    let mut state = runtime_state().lock().expect("murder runtime poisoned");
    let Some(game) = state.games.get_mut(&session.game_id) else {
        return PlayerAttackResponse::default();
    };
    if !matches!(game.phase, GamePhase::Running) {
        return PlayerAttackResponse::default();
    }
    let Some(target_uuid) = game.uuid_for_entity(payload.target_entity_id) else {
        return PlayerAttackResponse::default();
    };
    if target_uuid == payload.player.uuid || !game.is_alive(&target_uuid) {
        return PlayerAttackResponse::default();
    }
    game.kill(&target_uuid);
    let winner = winner_message(game);
    if let Some(winner) = winner.clone() {
        game.phase = GamePhase::Finished;
        game.finish_reason = Some(winner);
    }
    drop(state);

    let mut actions = vec![message("命中目标")];
    if let Some(winner) = winner {
        actions.extend(finish_actions(config, &winner));
    }
    PlayerAttackResponse {
        cancel: true,
        actions,
        ..PlayerAttackResponse::default()
    }
}

fn handle_player_use_item(
    config: &Config,
    payload: &PlayerUseItemPayload,
) -> PluginCommandResponse {
    if payload.action != "release_use_item" {
        return PluginCommandResponse::default();
    }
    let Some(mut session) = storage_get_typed::<PlayerSession>(&session_key(&payload.player.uuid))
    else {
        return PluginCommandResponse::default();
    };
    if session.role != Role::Detective || !session.alive {
        return PluginCommandResponse::default();
    }
    if !is_detective_bow(&payload.item, &config.items) {
        return PluginCommandResponse::default();
    }
    let now = time_millis();
    if now.saturating_sub(session.last_shot_ms) < config.combat.bow_cooldown_ms {
        return handled(vec![message("侦探弓冷却中")]);
    }
    session.last_shot_ms = now;
    let _ = storage_set_typed(&session_key(&payload.player.uuid), &session);

    let state = runtime_state().lock().expect("murder runtime poisoned");
    let Some(game) = state.games.get(&session.game_id) else {
        return PluginCommandResponse::default();
    };
    if !matches!(game.phase, GamePhase::Running) {
        return PluginCommandResponse::default();
    }
    drop(state);

    handled(vec![detective_arrow_projectile(config, payload)])
}

fn handle_projectile_hit_player(
    config: &Config,
    payload: &ProjectileHitPlayerPayload,
) -> PluginCommandResponse {
    if payload.configured_event != DETECTIVE_ARROW_EVENT {
        return PluginCommandResponse::default();
    }
    if payload.projectile_kind != "arrow" {
        return PluginCommandResponse::default();
    }
    if !payload
        .tag
        .trim()
        .ends_with(&config.items.detective_bow_name)
    {
        return PluginCommandResponse::default();
    }
    let Some(mut session) = storage_get_typed::<PlayerSession>(&session_key(&payload.shooter.uuid))
    else {
        return PluginCommandResponse::default();
    };
    if session.role != Role::Detective || !session.alive {
        return PluginCommandResponse::default();
    }

    let mut state = runtime_state().lock().expect("murder runtime poisoned");
    let Some(game) = state.games.get_mut(&session.game_id) else {
        return PluginCommandResponse::default();
    };
    if !matches!(game.phase, GamePhase::Running)
        || !game.is_alive(&payload.shooter.uuid)
        || !game.is_alive(&payload.target.uuid)
    {
        return PluginCommandResponse::default();
    }
    if payload.shooter.uuid == payload.target.uuid {
        return PluginCommandResponse::default();
    }
    let target_uuid = payload.target.uuid.clone();
    let target_role = game
        .player(&target_uuid)
        .map(|player| player.role)
        .unwrap_or(Role::Innocent);
    let victim = if target_role == Role::Murderer {
        target_uuid.clone()
    } else {
        payload.shooter.uuid.clone()
    };
    game.kill(&victim);
    let winner = winner_message(game);
    let players = game.players.clone();
    if let Some(winner) = winner.clone() {
        game.phase = GamePhase::Finished;
        game.finish_reason = Some(winner);
    }
    drop(state);

    let mut actions = Vec::new();
    if victim == payload.shooter.uuid {
        session.alive = false;
        let _ = storage_set_typed(&session_key(&payload.shooter.uuid), &session);
        actions.push(message("你射错了目标，侦探被淘汰"));
        if winner.is_none() {
            actions.extend(eliminate_actions(config, "你已返回大厅"));
        }
    } else {
        actions.push(message(format!(
            "侦探命中凶手 {}",
            game_player_name(&players, &victim)
        )));
    }
    if let Some(winner) = winner {
        actions.extend(finish_actions(config, &winner));
    }
    handled(actions)
}

fn enter_game_actions(config: &Config, spawn: SpawnPosition) -> Vec<PlayerAction> {
    vec![
        reset_inventory_without_menu_action(),
        PlayerAction::SetPlayersVisible { visible: true },
        PlayerAction::Teleport {
            dimension: config.game.dimension.clone(),
            x: spawn.x,
            y: spawn.y,
            z: spawn.z,
            yaw: Some(spawn.yaw),
            pitch: Some(0.0),
        },
    ]
}

#[derive(Debug, Clone, Copy)]
struct SpawnPosition {
    x: f64,
    y: f64,
    z: f64,
    yaw: f32,
}

fn spawn_position_for_player(
    config: &Config,
    game: &GameState,
    player_uuid: &str,
) -> SpawnPosition {
    let count = game.players.len().max(1);
    let index = game
        .players
        .iter()
        .position(|player| player.uuid == player_uuid)
        .unwrap_or(0);
    let radius = (f64::from(config.game.radius) * 0.45).clamp(3.0, 10.0);
    let angle = std::f64::consts::TAU * index as f64 / count as f64;
    let x = config.game.center_x + angle.cos() * radius;
    let z = config.game.center_z + angle.sin() * radius;
    let dx = config.game.center_x - x;
    let dz = config.game.center_z - z;
    let yaw = (dz.atan2(dx).to_degrees() - 90.0) as f32;
    SpawnPosition {
        x,
        y: config.game.spawn_y,
        z,
        yaw,
    }
}

fn detective_arrow_projectile(config: &Config, payload: &PlayerUseItemPayload) -> PlayerAction {
    let (dir_x, dir_y, dir_z) = look_direction(payload.yaw, payload.pitch);
    let speed = 1.8;
    PlayerAction::SpawnProjectile {
        kind: "arrow".to_string(),
        configured_event: DETECTIVE_ARROW_EVENT.to_string(),
        tag: payload.item.display_name.clone(),
        dimension: payload.dimension.clone(),
        x: payload.position.x + dir_x * 0.6,
        y: payload.position.y + 1.55 + dir_y * 0.6,
        z: payload.position.z + dir_z * 0.6,
        velocity_x: dir_x * speed,
        velocity_y: dir_y * speed,
        velocity_z: dir_z * speed,
        source_entity_id: payload.player.entity_id,
        damage: 0.0,
        knockback: 0.0,
        gravity_per_tick: 0.03,
        hit_radius: config.combat.bow_hit_radius.min(0.35),
        lifetime_ticks: ((config.combat.bow_range / speed).ceil() as i32 + 6).clamp(10, 80),
    }
}

fn role_intro_actions(config: &Config, role: Role) -> Vec<PlayerAction> {
    let mut actions = vec![message(role_message(role))];
    match role {
        Role::Murderer => actions.push(PlayerAction::GiveItem {
            item: config.items.murder_weapon.clone(),
            count: 1,
            name: "凶手的刀".to_string(),
            lore: vec!["不要被发现".to_string()],
            enchantments: Vec::new(),
            plugin_enchantments: Vec::new(),
        }),
        Role::Detective => {
            actions.push(PlayerAction::GiveItem {
                item: config.items.detective_bow.clone(),
                count: 1,
                name: "侦探弓".to_string(),
                lore: vec!["射错人会淘汰自己".to_string()],
                enchantments: Vec::new(),
                plugin_enchantments: Vec::new(),
            });
            actions.push(PlayerAction::GiveItem {
                item: "minecraft:arrow".to_string(),
                count: 1,
                name: String::new(),
                lore: Vec::new(),
                enchantments: Vec::new(),
                plugin_enchantments: Vec::new(),
            });
        }
        Role::Innocent => {}
    }
    actions
}

fn finish_player(
    config: &Config,
    player_uuid: &str,
    session: &mut PlayerSession,
    reason: &str,
) -> PluginCommandResponse {
    if session.finished {
        let _ = storage_delete(&session_key(player_uuid));
        return PluginCommandResponse::default();
    }
    session.finished = true;
    let _ = storage_set_typed(&session_key(player_uuid), session);
    let _ = storage_delete(&session_key(player_uuid));
    handled(finish_actions(config, reason))
}

fn eliminate_actions(config: &Config, text: &str) -> Vec<PlayerAction> {
    vec![
        message(text.to_string()),
        PlayerAction::SetPlayersVisible { visible: true },
        PlayerAction::Teleport {
            dimension: config.lobby.dimension.clone(),
            x: config.lobby.spawn_x,
            y: config.lobby.spawn_y,
            z: config.lobby.spawn_z,
            yaw: Some(config.lobby.spawn_yaw),
            pitch: Some(config.lobby.spawn_pitch),
        },
        reset_inventory_action(),
        PlayerAction::RemoveBossBar {
            id: BOSS_BAR_ID.to_string(),
        },
    ]
}

fn finish_actions(config: &Config, text: &str) -> Vec<PlayerAction> {
    vec![
        message(text.to_string()),
        PlayerAction::SetPlayersVisible { visible: true },
        PlayerAction::Teleport {
            dimension: config.lobby.dimension.clone(),
            x: config.lobby.spawn_x,
            y: config.lobby.spawn_y,
            z: config.lobby.spawn_z,
            yaw: Some(config.lobby.spawn_yaw),
            pitch: Some(config.lobby.spawn_pitch),
        },
        reset_inventory_action(),
        PlayerAction::RemoveBossBar {
            id: BOSS_BAR_ID.to_string(),
        },
    ]
}

fn reset_inventory_action() -> PlayerAction {
    PlayerAction::ResetInventory {
        restore_menu_items: true,
    }
}

fn reset_inventory_without_menu_action() -> PlayerAction {
    PlayerAction::ResetInventory {
        restore_menu_items: false,
    }
}

fn progress_bar(config: &Config, game: &GameState, role: Role) -> PlayerAction {
    let now = time_millis();
    let remaining = game.ends_at_ms.saturating_sub(now).max(0);
    let total = game.ends_at_ms.saturating_sub(game.started_at_ms).max(1) as f32;
    PlayerAction::BossBar {
        id: BOSS_BAR_ID.to_string(),
        title: format!(
            "{} | {} | 存活 {}",
            config.title,
            role_label(role),
            game.alive_count()
        ),
        progress: (remaining as f32 / total).clamp(0.0, 1.0),
        color: if role == Role::Murderer {
            "red".to_string()
        } else {
            "green".to_string()
        },
        overlay: "progress".to_string(),
    }
}

fn look_direction(yaw: f32, pitch: f32) -> (f64, f64, f64) {
    let yaw = f64::from(yaw).to_radians();
    let pitch = f64::from(pitch).to_radians();
    (
        -yaw.sin() * pitch.cos(),
        -pitch.sin(),
        yaw.cos() * pitch.cos(),
    )
}

fn register_regions(config: &Config) {
    let _ = world_register_edit_region(&WorldEditRegion {
        id: "murder_lobby",
        dimension: &config.lobby.dimension,
        min: (-16, config.lobby.floor_y - 2, -16),
        max: (16, config.lobby.floor_y + 8, 16),
        allow_player_break: false,
        allow_player_place: false,
        allow_plugin_write: true,
        runtime_only: true,
    });
    let radius = config.game.radius.max(8);
    let _ = world_register_edit_region(&WorldEditRegion {
        id: "murder_arena",
        dimension: &config.game.dimension,
        min: (
            config.game.center_x as i32 - radius - 4,
            config.game.floor_y - 2,
            config.game.center_z as i32 - radius - 4,
        ),
        max: (
            config.game.center_x as i32 + radius + 4,
            config.game.floor_y + 10,
            config.game.center_z as i32 + radius + 4,
        ),
        allow_player_break: false,
        allow_player_place: false,
        allow_plugin_write: true,
        runtime_only: true,
    });
}

fn build_map(config: &Config) {
    let mut blocks = Vec::new();
    if config.build_lobby {
        let y = config.lobby.floor_y;
        for x in -8..=8 {
            for z in -8..=8 {
                blocks.push((
                    config.lobby.dimension.as_str(),
                    (x, y - 1, z),
                    "minecraft:quartz_block",
                ));
                for air_y in y..=y + 5 {
                    blocks.push((
                        config.lobby.dimension.as_str(),
                        (x, air_y, z),
                        "minecraft:air",
                    ));
                }
            }
        }
    }
    if config.build_arena {
        let radius = config.game.radius.max(8);
        let cx = config.game.center_x as i32;
        let cz = config.game.center_z as i32;
        let y = config.game.floor_y;
        for x in -radius..=radius {
            for z in -radius..=radius {
                let edge = x.abs() == radius || z.abs() == radius;
                blocks.push((
                    config.game.dimension.as_str(),
                    (cx + x, y - 1, cz + z),
                    if edge {
                        "minecraft:stone_bricks"
                    } else {
                        "minecraft:smooth_stone"
                    },
                ));
                for air_y in y..=y + 6 {
                    blocks.push((
                        config.game.dimension.as_str(),
                        (cx + x, air_y, cz + z),
                        "minecraft:air",
                    ));
                }
            }
        }
        for x in (-radius + 3..=radius - 3).step_by(6) {
            blocks.push((
                config.game.dimension.as_str(),
                (cx + x, y, cz + 3),
                "minecraft:oak_planks",
            ));
            blocks.push((
                config.game.dimension.as_str(),
                (cx + x, y + 1, cz + 3),
                "minecraft:oak_planks",
            ));
        }
    }
    let _ = world_set_blocks(blocks);
}

fn remove_player(player_uuid: &str) {
    let mut state = runtime_state().lock().expect("murder runtime poisoned");
    let Some(game) = state.game_for_player_mut(player_uuid) else {
        return;
    };
    if matches!(game.phase, GamePhase::Waiting) {
        game.players.retain(|player| player.uuid != player_uuid);
    } else if let Some(player) = game.player_mut(player_uuid) {
        player.alive = false;
    }
}

fn winner_message(game: &GameState) -> Option<String> {
    let murderer_alive = game
        .players
        .iter()
        .any(|player| player.alive && player.role == Role::Murderer);
    if !murderer_alive {
        return Some("凶手被找出，平民获胜".to_string());
    }
    let alive_non_murderers = game
        .players
        .iter()
        .filter(|player| player.alive && player.role != Role::Murderer)
        .count();
    if alive_non_murderers <= 1 {
        return Some("凶手获胜".to_string());
    }
    None
}

fn game_player_name(players: &[PlayerState], player_uuid: &str) -> String {
    players
        .iter()
        .find(|player| player.uuid == player_uuid)
        .map(|player| player.username.clone())
        .unwrap_or_else(|| player_uuid.to_string())
}

fn is_murder_weapon(item: &ItemStackPayload, expected: &str) -> bool {
    same_item(item, expected)
}

fn is_detective_bow(item: &ItemStackPayload, config: &ItemConfig) -> bool {
    same_item(item, &config.detective_bow) && named_item_ends_with(item, &config.detective_bow_name)
}

fn same_item(item: &ItemStackPayload, expected: &str) -> bool {
    item.item_name == expected
        || (!expected.trim().is_empty() && item.item_name.ends_with(expected))
}

fn named_item_ends_with(item: &ItemStackPayload, expected: &str) -> bool {
    let expected = expected.trim();
    !expected.is_empty() && item.display_name.trim().ends_with(expected)
}

fn role_message(role: Role) -> &'static str {
    match role {
        Role::Murderer => "你的身份是凶手：用刀淘汰所有人。",
        Role::Detective => "你的身份是侦探：用弓找出凶手，射错会淘汰自己。",
        Role::Innocent => "你的身份是平民：躲避凶手，等待侦探找出真相。",
    }
}

fn role_label(role: Role) -> &'static str {
    match role {
        Role::Murderer => "凶手",
        Role::Detective => "侦探",
        Role::Innocent => "平民",
    }
}

fn handled(actions: Vec<PlayerAction>) -> PluginCommandResponse {
    PluginCommandResponse {
        handled: true,
        actions,
    }
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
        .and_then(|contents| toml::from_str(&contents).ok())
        .unwrap_or_else(default_config)
}

fn default_config() -> Config {
    toml::from_str(DEFAULT_CONFIG).expect("default murder_mystery config is valid")
}

fn session_key(player_uuid: &str) -> String {
    format!("{SESSION_PREFIX}{player_uuid}")
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

fn runtime_state() -> &'static Mutex<RuntimeState> {
    static RUNTIME: OnceLock<Mutex<RuntimeState>> = OnceLock::new();
    RUNTIME.get_or_init(|| Mutex::new(RuntimeState::default()))
}

#[derive(Debug, Default)]
struct RuntimeState {
    games: HashMap<String, GameState>,
}

impl RuntimeState {
    fn active_game(&self) -> Option<&GameState> {
        self.games
            .values()
            .find(|game| !matches!(game.phase, GamePhase::Finished))
    }

    fn active_game_mut(&mut self) -> Option<&mut GameState> {
        self.games
            .values_mut()
            .find(|game| !matches!(game.phase, GamePhase::Finished))
    }

    fn game_for_player(&self, player_uuid: &str) -> Option<&GameState> {
        self.games
            .values()
            .find(|game| game.players.iter().any(|player| player.uuid == player_uuid))
    }

    fn game_for_player_mut(&mut self, player_uuid: &str) -> Option<&mut GameState> {
        self.games
            .values_mut()
            .find(|game| game.players.iter().any(|player| player.uuid == player_uuid))
    }
}

#[derive(Debug, Clone)]
struct GameState {
    id: String,
    phase: GamePhase,
    players: Vec<PlayerState>,
    started_at_ms: i64,
    ends_at_ms: i64,
    finish_reason: Option<String>,
}

impl GameState {
    fn new(id: String) -> Self {
        Self {
            id,
            phase: GamePhase::Waiting,
            players: Vec::new(),
            started_at_ms: 0,
            ends_at_ms: 0,
            finish_reason: None,
        }
    }

    fn player(&self, uuid: &str) -> Option<&PlayerState> {
        self.players.iter().find(|player| player.uuid == uuid)
    }

    fn player_mut(&mut self, uuid: &str) -> Option<&mut PlayerState> {
        self.players.iter_mut().find(|player| player.uuid == uuid)
    }

    fn uuid_for_entity(&self, entity_id: i32) -> Option<String> {
        self.players
            .iter()
            .find(|player| player.entity_id == entity_id)
            .map(|player| player.uuid.clone())
    }

    fn is_alive(&self, uuid: &str) -> bool {
        self.player(uuid).is_some_and(|player| player.alive)
    }

    fn kill(&mut self, uuid: &str) {
        if let Some(player) = self.player_mut(uuid) {
            player.alive = false;
        }
    }

    fn alive_count(&self) -> usize {
        self.players.iter().filter(|player| player.alive).count()
    }
}

#[derive(Debug, Clone)]
struct PlayerState {
    uuid: String,
    username: String,
    entity_id: i32,
    role: Role,
    alive: bool,
    entered_game: bool,
    kit_given: bool,
    dimension: String,
    x: f64,
    y: f64,
    z: f64,
    yaw: f32,
    pitch: f32,
}

impl PlayerState {
    fn new(uuid: &str, username: &str) -> Self {
        Self {
            uuid: uuid.to_string(),
            username: username.to_string(),
            entity_id: 0,
            role: Role::Innocent,
            alive: true,
            entered_game: false,
            kit_given: false,
            dimension: String::new(),
            x: 0.0,
            y: 0.0,
            z: 0.0,
            yaw: 0.0,
            pitch: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
enum Role {
    Innocent,
    Detective,
    Murderer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GamePhase {
    Waiting,
    Running,
    Finished,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PlayerSession {
    game_id: String,
    role: Role,
    alive: bool,
    #[serde(default)]
    entered_game: bool,
    kit_given: bool,
    #[serde(default)]
    finished: bool,
    last_shot_ms: i64,
    last_x: f64,
    last_y: f64,
    last_z: f64,
    last_yaw: f32,
    last_pitch: f32,
}

#[derive(Debug, Clone, Deserialize)]
struct Config {
    #[serde(default = "default_true")]
    enable: bool,
    #[serde(default = "default_title")]
    title: String,
    #[serde(default = "default_menu_id")]
    menu_id: String,
    #[serde(default = "default_true")]
    build_lobby: bool,
    #[serde(default = "default_true")]
    build_arena: bool,
    #[serde(default)]
    lobby: LobbyConfig,
    #[serde(default)]
    game: GameConfig,
    #[serde(default)]
    combat: CombatConfig,
    #[serde(default)]
    items: ItemConfig,
}

#[derive(Debug, Clone, Deserialize)]
struct LobbyConfig {
    #[serde(default = "default_lobby_dimension")]
    dimension: String,
    #[serde(default)]
    spawn_x: f64,
    #[serde(default = "default_spawn_y")]
    spawn_y: f64,
    #[serde(default)]
    spawn_z: f64,
    #[serde(default)]
    spawn_yaw: f32,
    #[serde(default)]
    spawn_pitch: f32,
    #[serde(default = "default_floor_y")]
    floor_y: i32,
    #[serde(default)]
    npc_x: f64,
    #[serde(default = "default_spawn_y")]
    npc_y: f64,
    #[serde(default = "default_npc_z")]
    npc_z: f64,
    #[serde(default = "default_npc_yaw")]
    npc_yaw: f32,
    #[serde(default)]
    npc_pitch: f32,
    #[serde(default = "default_npc_name")]
    npc_display_name: String,
    #[serde(default = "default_npc_type")]
    npc_entity_type: String,
}

impl Default for LobbyConfig {
    fn default() -> Self {
        Self {
            dimension: default_lobby_dimension(),
            spawn_x: 0.5,
            spawn_y: default_spawn_y(),
            spawn_z: 0.5,
            spawn_yaw: 0.0,
            spawn_pitch: 0.0,
            floor_y: default_floor_y(),
            npc_x: 2.5,
            npc_y: default_spawn_y(),
            npc_z: default_npc_z(),
            npc_yaw: default_npc_yaw(),
            npc_pitch: 0.0,
            npc_display_name: default_npc_name(),
            npc_entity_type: default_npc_type(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct GameConfig {
    #[serde(default = "default_game_dimension")]
    dimension: String,
    #[serde(default = "default_min_players")]
    min_players: i32,
    #[serde(default = "default_duration_ms")]
    duration_ms: i64,
    #[serde(default)]
    center_x: f64,
    #[serde(default = "default_spawn_y")]
    spawn_y: f64,
    #[serde(default)]
    center_z: f64,
    #[serde(default = "default_floor_y")]
    floor_y: i32,
    #[serde(default = "default_radius")]
    radius: i32,
}

impl Default for GameConfig {
    fn default() -> Self {
        Self {
            dimension: default_game_dimension(),
            min_players: default_min_players(),
            duration_ms: default_duration_ms(),
            center_x: 0.5,
            spawn_y: default_spawn_y(),
            center_z: 0.5,
            floor_y: default_floor_y(),
            radius: default_radius(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct CombatConfig {
    #[serde(default = "default_bow_range")]
    bow_range: f64,
    #[serde(default = "default_bow_hit_radius")]
    bow_hit_radius: f64,
    #[serde(default = "default_bow_cooldown_ms")]
    bow_cooldown_ms: i64,
}

impl Default for CombatConfig {
    fn default() -> Self {
        Self {
            bow_range: default_bow_range(),
            bow_hit_radius: default_bow_hit_radius(),
            bow_cooldown_ms: default_bow_cooldown_ms(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct ItemConfig {
    #[serde(default = "default_murder_weapon")]
    murder_weapon: String,
    #[serde(default = "default_detective_bow")]
    detective_bow: String,
    #[serde(default = "default_detective_bow_name")]
    detective_bow_name: String,
}

impl Default for ItemConfig {
    fn default() -> Self {
        Self {
            murder_weapon: default_murder_weapon(),
            detective_bow: default_detective_bow(),
            detective_bow_name: default_detective_bow_name(),
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_title() -> String {
    "谁是凶手".to_string()
}

fn default_menu_id() -> String {
    "murder_select".to_string()
}

fn default_lobby_dimension() -> String {
    "qexed:murder_lobby".to_string()
}

fn default_game_dimension() -> String {
    "qexed:murder_game".to_string()
}

fn default_spawn_y() -> f64 {
    80.0
}

fn default_floor_y() -> i32 {
    80
}

fn default_npc_z() -> f64 {
    2.5
}

fn default_npc_yaw() -> f32 {
    180.0
}

fn default_npc_name() -> String {
    "谁是凶手".to_string()
}

fn default_npc_type() -> String {
    "minecraft:villager".to_string()
}

fn default_min_players() -> i32 {
    2
}

fn default_duration_ms() -> i64 {
    300_000
}

fn default_radius() -> i32 {
    18
}

fn default_bow_range() -> f64 {
    32.0
}

fn default_bow_hit_radius() -> f64 {
    1.25
}

fn default_bow_cooldown_ms() -> i64 {
    1500
}

fn default_murder_weapon() -> String {
    "minecraft:iron_sword".to_string()
}

fn default_detective_bow() -> String {
    "minecraft:bow".to_string()
}

fn default_detective_bow_name() -> String {
    "侦探弓".to_string()
}

const DEFAULT_CONFIG: &str = r#"enable = true
title = "谁是凶手"
menu_id = "murder_select"
build_lobby = true
build_arena = true

[lobby]
dimension = "qexed:murder_lobby"
spawn_x = 0.5
spawn_y = 80.0
spawn_z = 0.5
floor_y = 80
npc_x = 2.5
npc_y = 80.0
npc_z = 2.5
npc_yaw = 180.0
npc_display_name = "§c谁是凶手"
npc_entity_type = "minecraft:villager"

[game]
dimension = "qexed:murder_game"
min_players = 2
duration_ms = 300000
center_x = 0.5
spawn_y = 80.0
center_z = 0.5
floor_y = 80
radius = 18

[combat]
bow_range = 32.0
bow_hit_radius = 1.25
bow_cooldown_ms = 1500

[items]
murder_weapon = "minecraft:iron_sword"
detective_bow = "minecraft:bow"
detective_bow_name = "侦探弓"
"#;

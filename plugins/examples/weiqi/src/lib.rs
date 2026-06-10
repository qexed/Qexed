use qexed_plugin_sdk::{
    ConfigReloadPayload, NpcInteractPayload, NpcMutationOp, NpcMutationResponse, NpcUpsert,
    PlaceholderQuery, PlaceholderReplacement, PlaceholderResponse, PlayerAction,
    PlayerBlockInteractPayload, PlayerPayload, PlayerTickPayload, PluginCommandDefinition,
    PluginCommandQuery, PluginCommandResponse, PluginManifest, WorldEditRegion,
    config_load_or_create, config_read_to_string, storage_delete, storage_get_typed,
    storage_set_typed, time_millis, world_register_edit_region, world_set_blocks,
};
use serde::{Deserialize, Serialize};

qexed_plugin_sdk::qexed_plugin_memory!();
qexed_plugin_sdk::qexed_plugin_manifest!(PluginManifest {
    id: "qexed.weiqi".to_string(),
    version: "0.1.0".to_string(),
    depends: Vec::new(),
    optional_depends: Vec::new(),
    services: Vec::new(),
});

const CONFIG_PATH: &str = "config.toml";
const COMMAND_NAME: &str = "weiqi";
const NPC_KEY: &str = "weiqi:guide";
const NPC_EVENT: &str = "weiqi";
const SESSION_PREFIX: &str = "weiqi/session/";
const GAME_PREFIX: &str = "weiqi/game/";
const ROOM_PREFIX: &str = "weiqi/room/";
const PENDING_PREFIX: &str = "weiqi/pending/";
const QUEUE_KEY: &str = "weiqi/queue";
const BOARD_SIZE: usize = 19;
const INSTANCE_GRID_WIDTH: u64 = 2048;

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_priority() -> i32 {
    277
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_init() {
    let _ = config_load_or_create(CONFIG_PATH, DEFAULT_CONFIG);
    let config = load_config();
    register_lobby(&config);
    build_lobby(&config);
    qexed_plugin_sdk::log("weiqi initialized");
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_config_reload(ptr: i32, len: i32) {
    let Some(payload) =
        (unsafe { qexed_plugin_sdk::decode_payload::<ConfigReloadPayload>(ptr, len) })
    else {
        return;
    };
    let path = payload.path.replace('\\', "/");
    if path.ends_with("weiqi/config.toml") || path.ends_with(CONFIG_PATH) {
        let config = load_config();
        register_lobby(&config);
        build_lobby(&config);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_player_join(ptr: i32, len: i32) {
    let Some(payload) = (unsafe { qexed_plugin_sdk::decode_payload::<PlayerPayload>(ptr, len) })
    else {
        return;
    };
    let _ = storage_delete(&session_key(&payload.uuid));
    let _ = storage_delete(&pending_key(&payload.uuid));
    let config = load_config();
    push_pending(&payload.uuid, lobby_spawn_actions(&config));
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_player_leave(ptr: i32, len: i32) {
    let Some(payload) = (unsafe { qexed_plugin_sdk::decode_payload::<PlayerPayload>(ptr, len) })
    else {
        return;
    };
    let config = load_config();
    exit_current_game(&config, &payload.uuid);
    let _ = storage_delete(&pending_key(&payload.uuid));
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_commands(_ptr: i32, _len: i32) -> i64 {
    qexed_plugin_sdk::response_ptr_len(&PluginCommandDefinition {
        name: COMMAND_NAME.to_string(),
        description_key: "qexed.plugin.weiqi.command.description".to_string(),
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
    qexed_plugin_sdk::response_ptr_len(&handle_command(&config, &payload))
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
                name: "Weiqi".to_string(),
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
pub extern "C" fn qexed_plugin_player_block_interact(ptr: i32, len: i32) -> i64 {
    let Some(payload) =
        (unsafe { qexed_plugin_sdk::decode_payload::<PlayerBlockInteractPayload>(ptr, len) })
    else {
        return qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse::default());
    };
    let config = load_config();
    let response = handle_board_position(
        &config,
        &payload.player.uuid,
        &payload.player.username,
        &payload.dimension,
        payload.position.x,
        payload.position.y,
        payload.position.z,
    );
    qexed_plugin_sdk::response_ptr_len(&response)
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_player_tick(ptr: i32, len: i32) -> i64 {
    let Some(payload) =
        (unsafe { qexed_plugin_sdk::decode_payload::<PlayerTickPayload>(ptr, len) })
    else {
        return qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse::default());
    };
    let Some(pending) = storage_get_typed::<PendingActions>(&pending_key(&payload.player.uuid))
    else {
        return qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse::default());
    };
    let _ = storage_delete(&pending_key(&payload.player.uuid));
    qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse {
        handled: true,
        actions: pending.actions,
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_placeholders(ptr: i32, len: i32) -> i64 {
    let Some(payload) = (unsafe { qexed_plugin_sdk::decode_payload::<PlaceholderQuery>(ptr, len) })
    else {
        return qexed_plugin_sdk::response_ptr_len(&PlaceholderResponse::default());
    };
    let Some(player) = payload.player else {
        return qexed_plugin_sdk::response_ptr_len(&PlaceholderResponse::default());
    };
    let game = stored_session(&player.uuid).and_then(|session| stored_game(&session.game_id));
    qexed_plugin_sdk::response_ptr_len(&PlaceholderResponse {
        replacements: replacements(game.as_ref(), &player.uuid),
    })
}

fn handle_command(config: &Config, payload: &PluginCommandQuery) -> PluginCommandResponse {
    if !config.enable {
        return handled(vec![message("围棋插件已禁用")]);
    }
    let mut parts = payload.argument.split_whitespace();
    match parts.next().unwrap_or_default() {
        "" | "menu" | "select" => open_menu(config),
        "help" => help_response(),
        "match" | "queue" => {
            join_match_queue(config, &payload.player.uuid, &payload.player.username)
        }
        "bot" | "ai" => start_bot_game(config, &payload.player.uuid, &payload.player.username),
        "create" | "room" => create_room(config, &payload.player.uuid, &payload.player.username),
        "join" => join_room(
            config,
            &payload.player.uuid,
            &payload.player.username,
            parts.next().unwrap_or_default(),
        ),
        "move" | "put" | "place" => move_command(config, &payload.player.uuid, parts.collect()),
        "pass" => pass_turn(config, &payload.player.uuid),
        "resign" | "surrender" => resign_game(config, &payload.player.uuid),
        "status" | "info" => status_command(config, &payload.player.uuid),
        "leave" | "lobby" | "spawn" => leave_to_lobby(config, &payload.player.uuid),
        _ => handled(vec![message(
            "用法: /weiqi match | bot | create | join <房间码> | move <x> <z> | pass | resign | leave",
        )]),
    }
}

fn open_menu(config: &Config) -> PluginCommandResponse {
    handled(vec![PlayerAction::OpenMenu {
        menu: config.menu_id.clone(),
    }])
}

fn help_response() -> PluginCommandResponse {
    handled(vec![message(
        "围棋: 点击棋盘空位或 /weiqi move <x> <z> 落子；/weiqi pass 停一手；双方连续停手后自动数子。",
    )])
}

fn join_match_queue(
    config: &Config,
    player_uuid: &str,
    player_name: &str,
) -> PluginCommandResponse {
    if active_session(player_uuid).is_some() {
        return handled(vec![message("你已经在一局围棋中，先 /weiqi leave 再匹配")]);
    }
    let now = time_millis();
    if let Some(queue) = storage_get_typed::<MatchQueue>(QUEUE_KEY) {
        if queue.player_uuid == player_uuid {
            return handled(vec![message("你已经在围棋匹配队列中")]);
        }
        if now - queue.queued_at_ms <= config.matchmaking.queue_ttl_ms {
            let _ = storage_delete(QUEUE_KEY);
            let game = new_game(
                config,
                "match",
                &queue.player_uuid,
                &queue.player_name,
                player_uuid,
                player_name,
                "",
            );
            save_started_game(config, &game);
            push_pending(
                &queue.player_uuid,
                start_actions(config, &game, &queue.player_uuid, "匹配成功，你执黑先手"),
            );
            return handled(start_actions(
                config,
                &game,
                player_uuid,
                "匹配成功，你执白后手",
            ));
        }
    }
    let _ = storage_set_typed(
        QUEUE_KEY,
        &MatchQueue {
            player_uuid: player_uuid.to_string(),
            player_name: player_name.to_string(),
            queued_at_ms: now,
        },
    );
    handled(vec![message("已进入围棋匹配队列，等待下一名玩家加入")])
}

fn start_bot_game(config: &Config, player_uuid: &str, player_name: &str) -> PluginCommandResponse {
    if active_session(player_uuid).is_some() {
        return handled(vec![message(
            "你已经在一局围棋中，先 /weiqi leave 再开始人机",
        )]);
    }
    let mut game = new_game(
        config,
        "bot",
        player_uuid,
        player_name,
        "bot",
        &config.bot.name,
        "",
    );
    game.bot = true;
    save_started_game(config, &game);
    handled(start_actions(
        config,
        &game,
        player_uuid,
        "围棋人机开始，你执黑先手",
    ))
}

fn create_room(config: &Config, player_uuid: &str, player_name: &str) -> PluginCommandResponse {
    if active_session(player_uuid).is_some() {
        return handled(vec![message(
            "你已经在一局围棋中，先 /weiqi leave 再创建房间",
        )]);
    }
    let room_code = room_code_for(player_uuid);
    let game_id = game_id_for(player_uuid, &room_code, time_millis());
    let game = GameState::waiting(
        &game_id,
        &room_code,
        player_uuid,
        player_name,
        &config.board.dimension,
        instance_origin(config, &game_id),
    );
    let _ = storage_set_typed(&game_key(&game_id), &game);
    let _ = storage_set_typed(&room_key(&room_code), &RoomState { game_id });
    let _ = storage_set_typed(
        &session_key(player_uuid),
        &PlayerSession {
            game_id: game.id.clone(),
        },
    );
    handled(vec![message(format!(
        "房间已创建，房间码 {room_code}。另一名玩家输入 /weiqi join {room_code} 加入"
    ))])
}

fn join_room(
    config: &Config,
    player_uuid: &str,
    player_name: &str,
    code: &str,
) -> PluginCommandResponse {
    let code = normalize_room_code(code);
    if code.is_empty() {
        return handled(vec![message("用法: /weiqi join <房间码>")]);
    }
    if active_session(player_uuid).is_some() {
        return handled(vec![message(
            "你已经在一局围棋中，先 /weiqi leave 再加入房间",
        )]);
    }
    let Some(room) = storage_get_typed::<RoomState>(&room_key(&code)) else {
        return handled(vec![message("房间不存在或已过期")]);
    };
    let Some(mut game) = stored_game(&room.game_id) else {
        let _ = storage_delete(&room_key(&code));
        return handled(vec![message("房间数据不存在，请重新创建")]);
    };
    if game.status != "waiting" {
        return handled(vec![message("房间已经开局")]);
    }
    if game.black_player == player_uuid {
        return handled(vec![message("不能加入自己的房间")]);
    }
    game.white_player = player_uuid.to_string();
    game.white_name = player_name.to_string();
    game.status = "playing".to_string();
    game.started_at_ms = time_millis();
    let _ = storage_delete(&room_key(&code));
    save_started_game(config, &game);
    push_pending(
        &game.black_player,
        start_actions(config, &game, &game.black_player, "对手已加入，你执黑先手"),
    );
    handled(start_actions(
        config,
        &game,
        player_uuid,
        "加入成功，你执白后手",
    ))
}

fn move_command(config: &Config, player_uuid: &str, args: Vec<&str>) -> PluginCommandResponse {
    if args.len() != 2 {
        return handled(vec![message("用法: /weiqi move <x> <z>，坐标范围 1..19")]);
    }
    let Some(x) = parse_coord(args[0]) else {
        return handled(vec![message("x 坐标必须是 1..19")]);
    };
    let Some(z) = parse_coord(args[1]) else {
        return handled(vec![message("z 坐标必须是 1..19")]);
    };
    make_move(config, player_uuid, x, z)
}

fn status_command(_config: &Config, player_uuid: &str) -> PluginCommandResponse {
    let Some(session) = stored_session(player_uuid) else {
        return handled(vec![message("当前没有围棋对局")]);
    };
    let Some(game) = stored_game(&session.game_id) else {
        return handled(vec![message("当前对局数据不存在")]);
    };
    handled(vec![
        message(format!(
            "围棋状态: {}, 模式: {}, 回合: {}, 对手: {}, 提子: 黑 {} / 白 {}",
            status_label(&game),
            mode_label(&game),
            turn_label(&game),
            opponent_label(&game, player_uuid),
            game.black_captures,
            game.white_captures
        )),
        progress_bar(player_uuid, &game),
        teleport_to_game(&game),
    ])
}

fn leave_to_lobby(config: &Config, player_uuid: &str) -> PluginCommandResponse {
    exit_current_game(config, player_uuid);
    let mut actions = vec![message("正在返回大厅")];
    actions.extend(lobby_spawn_actions(config));
    actions.push(PlayerAction::RemoveBossBar {
        id: boss_bar_id(player_uuid),
    });
    if !config.lobby_server.trim().is_empty() {
        actions.push(PlayerAction::ProxyConnect {
            server: config.lobby_server.clone(),
            message: String::new(),
        });
    }
    handled(actions)
}

fn lobby_spawn_actions(config: &Config) -> Vec<PlayerAction> {
    vec![
        PlayerAction::SetPlayersVisible { visible: true },
        PlayerAction::Teleport {
            dimension: config.lobby.dimension.clone(),
            x: config.lobby.spawn_x,
            y: config.lobby.spawn_y,
            z: config.lobby.spawn_z,
            yaw: Some(config.lobby.spawn_yaw),
            pitch: Some(config.lobby.spawn_pitch),
        },
    ]
}

fn exit_current_game(config: &Config, player_uuid: &str) {
    remove_from_queue(player_uuid);
    if let Some(session) = stored_session(player_uuid) {
        if let Some(mut game) = stored_game(&session.game_id) {
            if game.status == "waiting" {
                if !game.room_code.is_empty() {
                    let _ = storage_delete(&room_key(&game.room_code));
                }
                let _ = storage_delete(&game_key(&game.id));
            } else if game.status == "playing" && game.bot {
                let _ = storage_delete(&game_key(&game.id));
            } else if game.status == "playing" && !game.bot {
                game.status = "forfeit".to_string();
                game.winner = opponent_mark(mark_for_player(&game, player_uuid).unwrap_or(2));
                finish_game(&mut game);
                save_game(&game);
                if let Some(opponent) = opponent_uuid(&game, player_uuid) {
                    push_pending(
                        opponent,
                        vec![
                            message("对手已离开，你获胜"),
                            progress_bar(opponent, &game),
                            PlayerAction::OpenMenu {
                                menu: config.menu_id.clone(),
                            },
                        ],
                    );
                }
            }
        }
    }
    let _ = storage_delete(&session_key(player_uuid));
}

fn handle_board_position(
    config: &Config,
    player_uuid: &str,
    _player_name: &str,
    dimension: &str,
    x: i32,
    y: i32,
    z: i32,
) -> PluginCommandResponse {
    let Some(session) = active_session(player_uuid) else {
        return PluginCommandResponse::default();
    };
    let Some(game) = stored_game(&session.game_id) else {
        return PluginCommandResponse::default();
    };
    if game.dimension != dimension {
        return PluginCommandResponse::default();
    }
    let Some((cell_x, cell_z)) = cell_from_position(&game, x, y, z) else {
        return PluginCommandResponse::default();
    };
    make_move(config, player_uuid, cell_x, cell_z)
}

fn make_move(config: &Config, player_uuid: &str, x: usize, z: usize) -> PluginCommandResponse {
    let Some(session) = active_session(player_uuid) else {
        return handled(vec![message("当前没有进行中的围棋")]);
    };
    let Some(mut game) = stored_game(&session.game_id) else {
        return handled(vec![message("当前对局数据不存在")]);
    };
    if game.status != "playing" {
        return handled(vec![message(format!("当前对局{}", status_label(&game)))]);
    }
    let Some(mark) = mark_for_player(&game, player_uuid) else {
        return handled(vec![message("你不是这局围棋的玩家")]);
    };
    if game.turn != mark {
        return handled(vec![message(format!(
            "还没轮到你，当前回合: {}",
            turn_label(&game)
        ))]);
    }

    let mut actions = match apply_player_move(&mut game, x, z, mark) {
        Ok(captured) => {
            let mut actions = vec![message(format!(
                "你落在 ({}, {}){}",
                x + 1,
                z + 1,
                capture_suffix(captured)
            ))];
            actions.extend(run_bot_if_needed(&mut game));
            actions
        }
        Err(err) => return handled(vec![message(err.label())]),
    };

    save_game(&game);
    render_board(config, &game);
    actions.extend(post_turn_actions(config, &game, player_uuid));
    handled(actions)
}

fn pass_turn(config: &Config, player_uuid: &str) -> PluginCommandResponse {
    let Some(session) = active_session(player_uuid) else {
        return handled(vec![message("当前没有进行中的围棋")]);
    };
    let Some(mut game) = stored_game(&session.game_id) else {
        return handled(vec![message("当前对局数据不存在")]);
    };
    if game.status != "playing" {
        return handled(vec![message(format!("当前对局{}", status_label(&game)))]);
    }
    let Some(mark) = mark_for_player(&game, player_uuid) else {
        return handled(vec![message("你不是这局围棋的玩家")]);
    };
    if game.turn != mark {
        return handled(vec![message(format!(
            "还没轮到你，当前回合: {}",
            turn_label(&game)
        ))]);
    }

    apply_pass(&mut game, mark);
    let mut actions = vec![message("你选择停一手")];
    actions.extend(run_bot_if_needed(&mut game));
    save_game(&game);
    render_board(config, &game);
    actions.extend(post_turn_actions(config, &game, player_uuid));
    handled(actions)
}

fn resign_game(config: &Config, player_uuid: &str) -> PluginCommandResponse {
    let Some(session) = active_session(player_uuid) else {
        return handled(vec![message("当前没有进行中的围棋")]);
    };
    let Some(mut game) = stored_game(&session.game_id) else {
        return handled(vec![message("当前对局数据不存在")]);
    };
    let Some(mark) = mark_for_player(&game, player_uuid) else {
        return handled(vec![message("你不是这局围棋的玩家")]);
    };
    game.status = "forfeit".to_string();
    game.winner = opponent_mark(mark);
    finish_game(&mut game);
    save_game(&game);
    render_board(config, &game);
    handled(finish_actions(config, &game, player_uuid))
}

fn apply_player_move(
    game: &mut GameState,
    x: usize,
    z: usize,
    mark: u8,
) -> Result<usize, MoveError> {
    let outcome = play_stone(&game.board, &game.previous_board, BOARD_SIZE, x, z, mark)?;
    game.previous_board = game.board.clone();
    game.board = outcome.board;
    if mark == 1 {
        game.black_captures = game.black_captures.saturating_add(outcome.captured as u16);
    } else {
        game.white_captures = game.white_captures.saturating_add(outcome.captured as u16);
    }
    game.consecutive_passes = 0;
    game.turn = opponent_mark(mark);
    Ok(outcome.captured)
}

fn apply_pass(game: &mut GameState, mark: u8) {
    game.previous_board = game.board.clone();
    game.consecutive_passes = game.consecutive_passes.saturating_add(1);
    if game.consecutive_passes >= 2 {
        finish_by_score(game);
    } else {
        game.turn = opponent_mark(mark);
    }
}

fn run_bot_if_needed(game: &mut GameState) -> Vec<PlayerAction> {
    if game.status != "playing" || !game.bot || game.turn != 2 {
        return Vec::new();
    }
    if let Some((x, z)) = bot_move(game) {
        match apply_player_move(game, x, z, 2) {
            Ok(captured) => vec![message(format!(
                "{} 落在 ({}, {}){}",
                game.white_name,
                x + 1,
                z + 1,
                capture_suffix(captured)
            ))],
            Err(_) => {
                apply_pass(game, 2);
                vec![message(format!("{} 停一手", game.white_name))]
            }
        }
    } else {
        apply_pass(game, 2);
        vec![message(format!("{} 停一手", game.white_name))]
    }
}

fn post_turn_actions(config: &Config, game: &GameState, player_uuid: &str) -> Vec<PlayerAction> {
    let mut actions = Vec::new();
    if game.status == "playing" {
        actions.push(progress_bar(player_uuid, game));
        if let Some(opponent) = opponent_uuid(game, player_uuid) {
            push_pending(
                opponent,
                vec![
                    message("对方已行动，轮到你"),
                    progress_bar(opponent, game),
                    teleport_to_game(game),
                ],
            );
        }
    } else {
        actions.extend(finish_actions(config, game, player_uuid));
        if let Some(opponent) = opponent_uuid(game, player_uuid) {
            push_pending(opponent, finish_actions(config, game, opponent));
        }
    }
    actions
}

fn capture_suffix(captured: usize) -> String {
    if captured == 0 {
        String::new()
    } else {
        format!("，提子 {captured}")
    }
}

fn finish_by_score(game: &mut GameState) {
    let (black, white) = score_area(&game.board, BOARD_SIZE);
    game.black_score = black + i32::from(game.black_captures);
    game.white_score = white + i32::from(game.white_captures);
    game.winner = if game.black_score > game.white_score {
        1
    } else if game.white_score > game.black_score {
        2
    } else {
        0
    };
    game.status = "finished".to_string();
    finish_game(game);
}

fn finish_game(game: &mut GameState) {
    game.ended_elapsed_ms = elapsed_millis(game.started_at_ms);
}

fn finish_actions(config: &Config, game: &GameState, player_uuid: &str) -> Vec<PlayerAction> {
    vec![
        message(result_message(game, player_uuid)),
        progress_bar(player_uuid, game),
        PlayerAction::OpenMenu {
            menu: config.menu_id.clone(),
        },
    ]
}

fn result_message(game: &GameState, player_uuid: &str) -> String {
    if game.winner == 0 && game.status == "finished" {
        return format!(
            "平局，黑 {} 目 / 白 {} 目，用时 {}",
            game.black_score,
            game.white_score,
            format_duration_ms(game.ended_elapsed_ms)
        );
    }
    let Some(mark) = mark_for_player(game, player_uuid) else {
        return "对局结束".to_string();
    };
    let score = if game.status == "finished" {
        format!("，黑 {} 目 / 白 {} 目", game.black_score, game.white_score)
    } else {
        String::new()
    };
    if game.winner == mark {
        format!(
            "你赢了{score}，用时 {}",
            format_duration_ms(game.ended_elapsed_ms)
        )
    } else {
        format!(
            "你输了{score}，用时 {}",
            format_duration_ms(game.ended_elapsed_ms)
        )
    }
}

fn start_actions(
    config: &Config,
    game: &GameState,
    player_uuid: &str,
    text: &str,
) -> Vec<PlayerAction> {
    vec![
        message(format!(
            "{}。服务器: {}，QQ群: {}",
            text, config.server_name, config.qq_group
        )),
        message("点击棋盘空位或使用 /weiqi move <x> <z> 落子；/weiqi pass 停一手"),
        PlayerAction::SetPlayersVisible { visible: true },
        progress_bar(player_uuid, game),
        teleport_to_game(game),
    ]
}

fn teleport_to_game(game: &GameState) -> PlayerAction {
    PlayerAction::Teleport {
        dimension: game.dimension.clone(),
        x: game.origin_x as f64 + BOARD_SIZE as f64 / 2.0,
        y: game.origin_y as f64 + 2.0,
        z: game.origin_z as f64 - 1.5,
        yaw: Some(0.0),
        pitch: Some(35.0),
    }
}

fn progress_bar(player_uuid: &str, game: &GameState) -> PlayerAction {
    let filled = game.board.iter().filter(|cell| **cell != 0).count() as f32;
    PlayerAction::BossBar {
        id: boss_bar_id(player_uuid),
        title: format!(
            "围棋 {} | 回合 {} | 提子 黑{} 白{}",
            status_label(game),
            turn_label(game),
            game.black_captures,
            game.white_captures
        ),
        progress: (filled / (BOARD_SIZE * BOARD_SIZE) as f32).clamp(0.0, 1.0),
        color: if game.turn == 1 { "purple" } else { "white" }.to_string(),
        overlay: "progress".to_string(),
    }
}

fn register_lobby(config: &Config) {
    let center_x = config.lobby.spawn_x.floor() as i32;
    let center_z = config.lobby.spawn_z.floor() as i32;
    let y = config.lobby.floor_y;
    let _ = world_register_edit_region(&WorldEditRegion {
        id: "weiqi_lobby",
        dimension: &config.lobby.dimension,
        min: (center_x - 8, y - 2, center_z - 8),
        max: (center_x + 8, y + 6, center_z + 8),
        allow_player_break: false,
        allow_player_place: false,
        allow_plugin_write: true,
        runtime_only: true,
    });
}

fn build_lobby(config: &Config) -> bool {
    let center_x = config.lobby.spawn_x.floor() as i32;
    let center_z = config.lobby.spawn_z.floor() as i32;
    let y = config.lobby.floor_y;
    let dimension = config.lobby.dimension.as_str();
    let mut blocks = Vec::new();
    for dx in -6i32..=6 {
        for dz in -6i32..=6 {
            let x = center_x + dx;
            let z = center_z + dz;
            let floor = if dx.abs() == 6 || dz.abs() == 6 {
                config.board.border_block.as_str()
            } else {
                config.board.floor_block.as_str()
            };
            blocks.push((dimension, (x, y - 1, z), floor));
            for air_y in y..=y + 4 {
                blocks.push((dimension, (x, air_y, z), "minecraft:air"));
            }
        }
    }
    world_set_blocks(blocks)
}

fn register_arena(game: &GameState) {
    let id = format!("weiqi_{}", game.id);
    let _ = world_register_edit_region(&WorldEditRegion {
        id: &id,
        dimension: &game.dimension,
        min: (game.origin_x - 2, game.origin_y - 2, game.origin_z - 3),
        max: (
            game.origin_x + BOARD_SIZE as i32 + 1,
            game.origin_y + 6,
            game.origin_z + BOARD_SIZE as i32 + 1,
        ),
        allow_player_break: false,
        allow_player_place: false,
        allow_plugin_write: true,
        runtime_only: true,
    });
}

fn render_board(config: &Config, game: &GameState) -> bool {
    let mut blocks = Vec::new();
    let dimension = game.dimension.as_str();
    for dx in -1..=BOARD_SIZE as i32 {
        for dz in -2..=BOARD_SIZE as i32 {
            blocks.push((
                dimension,
                (game.origin_x + dx, game.origin_y - 1, game.origin_z + dz),
                config.board.floor_block.as_str(),
            ));
            for air_y in game.origin_y..=game.origin_y + 4 {
                blocks.push((
                    dimension,
                    (game.origin_x + dx, air_y, game.origin_z + dz),
                    "minecraft:air",
                ));
            }
        }
    }
    for dx in -1..=BOARD_SIZE as i32 {
        for dz in -1..=BOARD_SIZE as i32 {
            if dx == -1 || dz == -1 || dx == BOARD_SIZE as i32 || dz == BOARD_SIZE as i32 {
                blocks.push((
                    dimension,
                    (game.origin_x + dx, game.origin_y, game.origin_z + dz),
                    config.board.border_block.as_str(),
                ));
            }
        }
    }
    for z in 0..BOARD_SIZE {
        for x in 0..BOARD_SIZE {
            let mark = game.board[index_of(x, z, BOARD_SIZE)];
            blocks.push((
                dimension,
                (
                    game.origin_x + x as i32,
                    game.origin_y,
                    game.origin_z + z as i32,
                ),
                block_for_point(config, mark, x, z),
            ));
        }
    }
    world_set_blocks(blocks)
}

fn block_for_point(config: &Config, mark: u8, x: usize, z: usize) -> &str {
    match mark {
        1 => config.board.black_block.as_str(),
        2 => config.board.white_block.as_str(),
        _ if is_star_point(x, z) => config.board.star_block.as_str(),
        _ => config.board.board_block.as_str(),
    }
}

fn is_star_point(x: usize, z: usize) -> bool {
    matches!(x, 3 | 9 | 15) && matches!(z, 3 | 9 | 15)
}

fn cell_from_position(game: &GameState, x: i32, y: i32, z: i32) -> Option<(usize, usize)> {
    if y != game.origin_y {
        return None;
    }
    let local_x = x.checked_sub(game.origin_x)?;
    let local_z = z.checked_sub(game.origin_z)?;
    if local_x < 0 || local_z < 0 || local_x >= BOARD_SIZE as i32 || local_z >= BOARD_SIZE as i32 {
        return None;
    }
    Some((local_x as usize, local_z as usize))
}

fn save_started_game(config: &Config, game: &GameState) {
    register_arena(game);
    render_board(config, game);
    save_game(game);
    let _ = storage_set_typed(
        &session_key(&game.black_player),
        &PlayerSession {
            game_id: game.id.clone(),
        },
    );
    if !game.white_player.is_empty() && game.white_player != "bot" {
        let _ = storage_set_typed(
            &session_key(&game.white_player),
            &PlayerSession {
                game_id: game.id.clone(),
            },
        );
    }
}

fn save_game(game: &GameState) {
    let _ = storage_set_typed(&game_key(&game.id), game);
}

fn stored_session(player_uuid: &str) -> Option<PlayerSession> {
    storage_get_typed(&session_key(player_uuid))
}

fn active_session(player_uuid: &str) -> Option<PlayerSession> {
    let session = stored_session(player_uuid)?;
    stored_game(&session.game_id)
        .filter(|game| matches!(game.status.as_str(), "waiting" | "playing"))
        .map(|_| session)
}

fn stored_game(game_id: &str) -> Option<GameState> {
    storage_get_typed(&game_key(game_id))
}

fn remove_from_queue(player_uuid: &str) {
    if storage_get_typed::<MatchQueue>(QUEUE_KEY)
        .is_some_and(|queue| queue.player_uuid == player_uuid)
    {
        let _ = storage_delete(QUEUE_KEY);
    }
}

fn push_pending(player_uuid: &str, mut actions: Vec<PlayerAction>) {
    if player_uuid == "bot" {
        return;
    }
    let key = pending_key(player_uuid);
    let mut pending = storage_get_typed::<PendingActions>(&key).unwrap_or_default();
    pending.actions.append(&mut actions);
    let _ = storage_set_typed(&key, &pending);
}

fn mark_for_player(game: &GameState, player_uuid: &str) -> Option<u8> {
    if game.black_player == player_uuid {
        Some(1)
    } else if game.white_player == player_uuid {
        Some(2)
    } else {
        None
    }
}

fn opponent_uuid<'a>(game: &'a GameState, player_uuid: &str) -> Option<&'a str> {
    if game.black_player == player_uuid
        && !game.white_player.is_empty()
        && game.white_player != "bot"
    {
        Some(game.white_player.as_str())
    } else if game.white_player == player_uuid {
        Some(game.black_player.as_str())
    } else {
        None
    }
}

fn opponent_label(game: &GameState, player_uuid: &str) -> String {
    if game.black_player == player_uuid {
        game.white_name.clone()
    } else if game.white_player == player_uuid {
        game.black_name.clone()
    } else {
        "-".to_string()
    }
}

fn turn_label(game: &GameState) -> String {
    match game.turn {
        1 => format!("黑({})", game.black_name),
        2 => format!("白({})", game.white_name),
        _ => "-".to_string(),
    }
}

fn status_label(game: &GameState) -> String {
    match game.status.as_str() {
        "waiting" => "等待玩家".to_string(),
        "playing" => "进行中".to_string(),
        "finished" => match game.winner {
            1 => "黑胜".to_string(),
            2 => "白胜".to_string(),
            _ => "平局".to_string(),
        },
        "forfeit" => "有人离开".to_string(),
        _ => "已结束".to_string(),
    }
}

fn mode_label(game: &GameState) -> String {
    match game.mode.as_str() {
        "match" => "匹配".to_string(),
        "room" => "房间".to_string(),
        "bot" => "人机".to_string(),
        _ => game.mode.clone(),
    }
}

fn replacements(game: Option<&GameState>, player_uuid: &str) -> Vec<PlaceholderReplacement> {
    vec![
        placeholder(
            "weiqi_state",
            game.map(status_label).unwrap_or_else(|| "大厅".to_string()),
        ),
        placeholder(
            "weiqi_mode",
            game.map(mode_label).unwrap_or_else(|| "-".to_string()),
        ),
        placeholder(
            "weiqi_turn",
            game.map(turn_label).unwrap_or_else(|| "-".to_string()),
        ),
        placeholder(
            "weiqi_opponent",
            game.map(|game| opponent_label(game, player_uuid))
                .unwrap_or_else(|| "-".to_string()),
        ),
        placeholder(
            "weiqi_room",
            game.and_then(|game| (!game.room_code.is_empty()).then(|| game.room_code.clone()))
                .unwrap_or_else(|| "-".to_string()),
        ),
        placeholder(
            "weiqi_captures",
            game.map(|game| format!("黑{} 白{}", game.black_captures, game.white_captures))
                .unwrap_or_else(|| "-".to_string()),
        ),
        placeholder(
            "weiqi_score",
            game.map(|game| format!("黑{} 白{}", game.black_score, game.white_score))
                .unwrap_or_else(|| "-".to_string()),
        ),
        placeholder(
            "weiqi_time",
            game.map(game_elapsed_label)
                .unwrap_or_else(|| "--:--".to_string()),
        ),
    ]
}

fn placeholder(key: &str, value: impl Into<String>) -> PlaceholderReplacement {
    PlaceholderReplacement {
        key: key.to_string(),
        value: value.into(),
    }
}

fn game_elapsed_label(game: &GameState) -> String {
    if game.status == "playing" {
        format_duration_ms(elapsed_millis(game.started_at_ms))
    } else {
        format_duration_ms(game.ended_elapsed_ms)
    }
}

fn parse_coord(value: &str) -> Option<usize> {
    let parsed = value.parse::<usize>().ok()?;
    (1..=BOARD_SIZE).contains(&parsed).then_some(parsed - 1)
}

fn elapsed_millis(started_at_ms: i64) -> i64 {
    (time_millis() - started_at_ms).max(0)
}

fn format_duration_ms(ms: i64) -> String {
    let total_seconds = (ms.max(0) / 1000) as u64;
    let minutes = total_seconds / 60;
    let seconds = total_seconds % 60;
    format!("{minutes:02}:{seconds:02}")
}

fn room_code_for(player_uuid: &str) -> String {
    let value = fnv1a64(format!("{}:{}", player_uuid, time_millis()).as_bytes()) % 10000;
    format!("{value:04}")
}

fn normalize_room_code(code: &str) -> String {
    code.chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .take(8)
        .collect::<String>()
        .to_ascii_uppercase()
}

fn game_id_for(left: &str, right: &str, seed: i64) -> String {
    format!(
        "{:016x}",
        fnv1a64(format!("{left}:{right}:{seed}").as_bytes())
    )
}

fn instance_origin(config: &Config, game_id: &str) -> (i32, i32, i32) {
    let hash = fnv1a64(game_id.as_bytes());
    let grid_x = (hash % INSTANCE_GRID_WIDTH) as i32;
    let grid_z = ((hash / INSTANCE_GRID_WIDTH) % INSTANCE_GRID_WIDTH) as i32;
    (
        config.board.origin_x + grid_x * config.board.instance_spacing,
        config.board.origin_y,
        config.board.origin_z + grid_z * config.board.instance_spacing,
    )
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
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

fn boss_bar_id(player_uuid: &str) -> String {
    format!("weiqi:{player_uuid}")
}

fn session_key(player_uuid: &str) -> String {
    format!("{SESSION_PREFIX}{player_uuid}")
}

fn game_key(game_id: &str) -> String {
    format!("{GAME_PREFIX}{game_id}")
}

fn room_key(code: &str) -> String {
    format!("{ROOM_PREFIX}{code}")
}

fn pending_key(player_uuid: &str) -> String {
    format!("{PENDING_PREFIX}{player_uuid}")
}

fn load_config() -> Config {
    config_read_to_string(CONFIG_PATH)
        .and_then(|contents| toml::from_str(&contents).ok())
        .unwrap_or_default()
}

fn opponent_mark(mark: u8) -> u8 {
    if mark == 1 { 2 } else { 1 }
}

fn bot_move(game: &GameState) -> Option<(usize, usize)> {
    let candidates = candidate_moves(game);
    candidates
        .into_iter()
        .filter_map(|(x, z)| {
            let outcome =
                play_stone(&game.board, &game.previous_board, BOARD_SIZE, x, z, 2).ok()?;
            Some((
                (x, z),
                bot_score(&game.board, &outcome.board, x, z, outcome.captured),
            ))
        })
        .max_by_key(|(_, score)| *score)
        .map(|(position, _)| position)
}

fn candidate_moves(game: &GameState) -> Vec<(usize, usize)> {
    if game.board.iter().all(|cell| *cell == 0) {
        return vec![(BOARD_SIZE / 2, BOARD_SIZE / 2)];
    }
    let mut result = Vec::new();
    for z in 0..BOARD_SIZE {
        for x in 0..BOARD_SIZE {
            if game.board[index_of(x, z, BOARD_SIZE)] != 0 {
                continue;
            }
            if has_neighbor(&game.board, BOARD_SIZE, x, z, 2) {
                result.push((x, z));
            }
        }
    }
    if result.is_empty() {
        result.push((BOARD_SIZE / 2, BOARD_SIZE / 2));
    }
    result
}

fn has_neighbor(board: &[u8], size: usize, x: usize, z: usize, radius: i32) -> bool {
    for dz in -radius..=radius {
        for dx in -radius..=radius {
            if dx == 0 && dz == 0 {
                continue;
            }
            let nx = x as i32 + dx;
            let nz = z as i32 + dz;
            if nx >= 0 && nz >= 0 && nx < size as i32 && nz < size as i32 {
                let index = index_of(nx as usize, nz as usize, size);
                if board[index] != 0 {
                    return true;
                }
            }
        }
    }
    false
}

fn bot_score(old_board: &[u8], new_board: &[u8], x: usize, z: usize, captured: usize) -> i32 {
    let center = BOARD_SIZE as i32 / 2;
    let distance = (x as i32 - center).abs() + (z as i32 - center).abs();
    let (_, liberties) = group_and_liberties(new_board, BOARD_SIZE, index_of(x, z, BOARD_SIZE));
    let adjacent_enemy = neighbors(x, z, BOARD_SIZE)
        .into_iter()
        .filter(|index| old_board[*index] == 1)
        .count() as i32;
    (captured as i32 * 100) + (liberties as i32 * 10) + adjacent_enemy * 4 - distance
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MoveError {
    Occupied,
    Suicide,
    Ko,
    OutOfBounds,
}

impl MoveError {
    fn label(self) -> &'static str {
        match self {
            Self::Occupied => "这个位置已经有棋子了",
            Self::Suicide => "这里没有气，不能自杀落子",
            Self::Ko => "违反劫规则，不能立刻提回",
            Self::OutOfBounds => "坐标超出棋盘范围",
        }
    }
}

#[derive(Debug, Clone)]
struct MoveOutcome {
    board: Vec<u8>,
    captured: usize,
}

fn play_stone(
    board: &[u8],
    previous_board: &[u8],
    size: usize,
    x: usize,
    z: usize,
    mark: u8,
) -> Result<MoveOutcome, MoveError> {
    if x >= size || z >= size {
        return Err(MoveError::OutOfBounds);
    }
    let index = index_of(x, z, size);
    if board.get(index).copied().unwrap_or_default() != 0 {
        return Err(MoveError::Occupied);
    }

    let mut next = board.to_vec();
    next[index] = mark;
    let opponent = opponent_mark(mark);
    let mut captured = Vec::new();
    for neighbor in neighbors(x, z, size) {
        if next[neighbor] != opponent || captured.contains(&neighbor) {
            continue;
        }
        let (group, liberties) = group_and_liberties(&next, size, neighbor);
        if liberties == 0 {
            captured.extend(group);
        }
    }
    captured.sort_unstable();
    captured.dedup();
    for captured_index in &captured {
        next[*captured_index] = 0;
    }

    let (_, own_liberties) = group_and_liberties(&next, size, index);
    if own_liberties == 0 {
        return Err(MoveError::Suicide);
    }
    if previous_board.len() == next.len() && previous_board == next {
        return Err(MoveError::Ko);
    }
    Ok(MoveOutcome {
        board: next,
        captured: captured.len(),
    })
}

fn group_and_liberties(board: &[u8], size: usize, start: usize) -> (Vec<usize>, usize) {
    let mark = board[start];
    if mark == 0 {
        return (Vec::new(), 0);
    }
    let mut visited = vec![false; board.len()];
    let mut liberty_seen = vec![false; board.len()];
    let mut group = Vec::new();
    let mut stack = vec![start];
    let mut liberties = 0;
    while let Some(index) = stack.pop() {
        if visited[index] {
            continue;
        }
        visited[index] = true;
        group.push(index);
        let (x, z) = coord_of(index, size);
        for neighbor in neighbors(x, z, size) {
            if board[neighbor] == 0 {
                if !liberty_seen[neighbor] {
                    liberty_seen[neighbor] = true;
                    liberties += 1;
                }
            } else if board[neighbor] == mark && !visited[neighbor] {
                stack.push(neighbor);
            }
        }
    }
    (group, liberties)
}

fn score_area(board: &[u8], size: usize) -> (i32, i32) {
    let mut visited = vec![false; board.len()];
    let mut black = 0;
    let mut white = 0;
    for index in 0..board.len() {
        match board[index] {
            1 => {
                black += 1;
                visited[index] = true;
            }
            2 => {
                white += 1;
                visited[index] = true;
            }
            _ if visited[index] => {}
            _ => {
                let (region, touches_black, touches_white) = empty_region(board, size, index);
                for cell in &region {
                    visited[*cell] = true;
                }
                if touches_black && !touches_white {
                    black += region.len() as i32;
                } else if touches_white && !touches_black {
                    white += region.len() as i32;
                }
            }
        }
    }
    (black, white)
}

fn empty_region(board: &[u8], size: usize, start: usize) -> (Vec<usize>, bool, bool) {
    let mut visited = vec![false; board.len()];
    let mut region = Vec::new();
    let mut stack = vec![start];
    let mut touches_black = false;
    let mut touches_white = false;
    while let Some(index) = stack.pop() {
        if visited[index] || board[index] != 0 {
            continue;
        }
        visited[index] = true;
        region.push(index);
        let (x, z) = coord_of(index, size);
        for neighbor in neighbors(x, z, size) {
            match board[neighbor] {
                0 if !visited[neighbor] => stack.push(neighbor),
                1 => touches_black = true,
                2 => touches_white = true,
                _ => {}
            }
        }
    }
    (region, touches_black, touches_white)
}

fn neighbors(x: usize, z: usize, size: usize) -> Vec<usize> {
    let mut result = Vec::with_capacity(4);
    if x > 0 {
        result.push(index_of(x - 1, z, size));
    }
    if z > 0 {
        result.push(index_of(x, z - 1, size));
    }
    if x + 1 < size {
        result.push(index_of(x + 1, z, size));
    }
    if z + 1 < size {
        result.push(index_of(x, z + 1, size));
    }
    result
}

fn index_of(x: usize, z: usize, size: usize) -> usize {
    z * size + x
}

fn coord_of(index: usize, size: usize) -> (usize, usize) {
    (index % size, index / size)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PlayerSession {
    game_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GameState {
    id: String,
    status: String,
    mode: String,
    #[serde(default)]
    room_code: String,
    black_player: String,
    black_name: String,
    #[serde(default)]
    white_player: String,
    #[serde(default)]
    white_name: String,
    #[serde(default)]
    bot: bool,
    board: Vec<u8>,
    #[serde(default)]
    previous_board: Vec<u8>,
    turn: u8,
    #[serde(default)]
    consecutive_passes: u8,
    #[serde(default)]
    black_captures: u16,
    #[serde(default)]
    white_captures: u16,
    #[serde(default)]
    black_score: i32,
    #[serde(default)]
    white_score: i32,
    #[serde(default)]
    winner: u8,
    dimension: String,
    origin_x: i32,
    origin_y: i32,
    origin_z: i32,
    started_at_ms: i64,
    #[serde(default)]
    ended_elapsed_ms: i64,
}

impl GameState {
    fn waiting(
        id: &str,
        room_code: &str,
        black_player: &str,
        black_name: &str,
        dimension: &str,
        origin: (i32, i32, i32),
    ) -> Self {
        Self {
            id: id.to_string(),
            status: "waiting".to_string(),
            mode: "room".to_string(),
            room_code: room_code.to_string(),
            black_player: black_player.to_string(),
            black_name: black_name.to_string(),
            white_player: String::new(),
            white_name: String::new(),
            bot: false,
            board: vec![0; BOARD_SIZE * BOARD_SIZE],
            previous_board: vec![0; BOARD_SIZE * BOARD_SIZE],
            turn: 1,
            consecutive_passes: 0,
            black_captures: 0,
            white_captures: 0,
            black_score: 0,
            white_score: 0,
            winner: 0,
            dimension: dimension.to_string(),
            origin_x: origin.0,
            origin_y: origin.1,
            origin_z: origin.2,
            started_at_ms: time_millis(),
            ended_elapsed_ms: 0,
        }
    }
}

fn new_game(
    config: &Config,
    mode: &str,
    black_player: &str,
    black_name: &str,
    white_player: &str,
    white_name: &str,
    room_code: &str,
) -> GameState {
    let id = game_id_for(black_player, white_player, time_millis());
    let origin = instance_origin(config, &id);
    let mut game = GameState::waiting(
        &id,
        room_code,
        black_player,
        black_name,
        &config.board.dimension,
        origin,
    );
    game.mode = mode.to_string();
    game.white_player = white_player.to_string();
    game.white_name = white_name.to_string();
    game.status = "playing".to_string();
    game.started_at_ms = time_millis();
    game.bot = white_player == "bot";
    game
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MatchQueue {
    player_uuid: String,
    player_name: String,
    queued_at_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RoomState {
    game_id: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct PendingActions {
    actions: Vec<PlayerAction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Config {
    #[serde(default = "default_enable")]
    enable: bool,
    #[serde(default = "default_server_name")]
    server_name: String,
    #[serde(default = "default_qq_group")]
    qq_group: String,
    #[serde(default = "default_lobby_server")]
    lobby_server: String,
    #[serde(default = "default_menu_id")]
    menu_id: String,
    #[serde(default)]
    matchmaking: MatchmakingConfig,
    #[serde(default)]
    bot: BotConfig,
    #[serde(default)]
    lobby: LobbyConfig,
    #[serde(default)]
    board: BoardConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enable: default_enable(),
            server_name: default_server_name(),
            qq_group: default_qq_group(),
            lobby_server: default_lobby_server(),
            menu_id: default_menu_id(),
            matchmaking: MatchmakingConfig::default(),
            bot: BotConfig::default(),
            lobby: LobbyConfig::default(),
            board: BoardConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MatchmakingConfig {
    #[serde(default = "default_queue_ttl_ms")]
    queue_ttl_ms: i64,
}

impl Default for MatchmakingConfig {
    fn default() -> Self {
        Self {
            queue_ttl_ms: default_queue_ttl_ms(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BotConfig {
    #[serde(default = "default_bot_name")]
    name: String,
}

impl Default for BotConfig {
    fn default() -> Self {
        Self {
            name: default_bot_name(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LobbyConfig {
    #[serde(default = "default_lobby_dimension")]
    dimension: String,
    #[serde(default = "default_floor_y")]
    floor_y: i32,
    #[serde(default = "default_spawn_x")]
    spawn_x: f64,
    #[serde(default = "default_spawn_y")]
    spawn_y: f64,
    #[serde(default = "default_spawn_z")]
    spawn_z: f64,
    #[serde(default)]
    spawn_yaw: f32,
    #[serde(default)]
    spawn_pitch: f32,
    #[serde(default = "default_npc_x")]
    npc_x: f64,
    #[serde(default = "default_npc_y")]
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
            floor_y: default_floor_y(),
            spawn_x: default_spawn_x(),
            spawn_y: default_spawn_y(),
            spawn_z: default_spawn_z(),
            spawn_yaw: 0.0,
            spawn_pitch: 0.0,
            npc_x: default_npc_x(),
            npc_y: default_npc_y(),
            npc_z: default_npc_z(),
            npc_yaw: default_npc_yaw(),
            npc_pitch: 0.0,
            npc_entity_type: default_npc_entity_type(),
            npc_display_name: default_npc_display_name(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BoardConfig {
    #[serde(default = "default_board_dimension")]
    dimension: String,
    #[serde(default = "default_board_origin_x")]
    origin_x: i32,
    #[serde(default = "default_floor_y")]
    origin_y: i32,
    #[serde(default = "default_board_origin_z")]
    origin_z: i32,
    #[serde(default = "default_instance_spacing")]
    instance_spacing: i32,
    #[serde(default = "default_floor_block")]
    floor_block: String,
    #[serde(default = "default_border_block")]
    border_block: String,
    #[serde(default = "default_board_block")]
    board_block: String,
    #[serde(default = "default_star_block")]
    star_block: String,
    #[serde(default = "default_black_block")]
    black_block: String,
    #[serde(default = "default_white_block")]
    white_block: String,
}

impl Default for BoardConfig {
    fn default() -> Self {
        Self {
            dimension: default_board_dimension(),
            origin_x: default_board_origin_x(),
            origin_y: default_floor_y(),
            origin_z: default_board_origin_z(),
            instance_spacing: default_instance_spacing(),
            floor_block: default_floor_block(),
            border_block: default_border_block(),
            board_block: default_board_block(),
            star_block: default_star_block(),
            black_block: default_black_block(),
            white_block: default_white_block(),
        }
    }
}

fn default_enable() -> bool {
    true
}

fn default_server_name() -> String {
    "浅屿闲游".to_string()
}

fn default_qq_group() -> String {
    "722632621".to_string()
}

fn default_lobby_server() -> String {
    "lobby_1".to_string()
}

fn default_menu_id() -> String {
    "weiqi_select".to_string()
}

fn default_queue_ttl_ms() -> i64 {
    300_000
}

fn default_bot_name() -> String {
    "浅屿棋手".to_string()
}

fn default_lobby_dimension() -> String {
    "qexed:weiqi_lobby".to_string()
}

fn default_board_dimension() -> String {
    "qexed:weiqi_game".to_string()
}

fn default_floor_y() -> i32 {
    80
}

fn default_spawn_x() -> f64 {
    0.5
}

fn default_spawn_y() -> f64 {
    80.0
}

fn default_spawn_z() -> f64 {
    0.5
}

fn default_npc_x() -> f64 {
    0.5
}

fn default_npc_y() -> f64 {
    80.0
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
    r#"{"text":"围棋入口","color":"gold"}"#.to_string()
}

fn default_board_origin_x() -> i32 {
    4096
}

fn default_board_origin_z() -> i32 {
    4096
}

fn default_instance_spacing() -> i32 {
    64
}

fn default_floor_block() -> String {
    "minecraft:smooth_stone".to_string()
}

fn default_border_block() -> String {
    "minecraft:dark_oak_planks".to_string()
}

fn default_board_block() -> String {
    "minecraft:birch_planks".to_string()
}

fn default_star_block() -> String {
    "minecraft:oak_planks".to_string()
}

fn default_black_block() -> String {
    "minecraft:black_concrete".to_string()
}

fn default_white_block() -> String {
    "minecraft:white_concrete".to_string()
}

const DEFAULT_CONFIG: &str = r#"enable = true
server_name = "浅屿闲游"
qq_group = "722632621"
lobby_server = "lobby_1"
menu_id = "weiqi_select"

[matchmaking]
queue_ttl_ms = 300000

[bot]
name = "浅屿棋手"

[lobby]
dimension = "qexed:weiqi_lobby"
floor_y = 80
spawn_x = 0.5
spawn_y = 80.0
spawn_z = 0.5
spawn_yaw = 0.0
spawn_pitch = 0.0
npc_x = 0.5
npc_y = 80.0
npc_z = 3.5
npc_yaw = 180.0
npc_pitch = 0.0
npc_entity_type = "minecraft:villager"
npc_display_name = "{\"text\":\"围棋入口\",\"color\":\"gold\"}"

[board]
dimension = "qexed:weiqi_game"
origin_x = 4096
origin_y = 80
origin_z = 4096
instance_spacing = 64
floor_block = "minecraft:smooth_stone"
border_block = "minecraft:dark_oak_planks"
board_block = "minecraft:birch_planks"
star_block = "minecraft:oak_planks"
black_block = "minecraft:black_concrete"
white_block = "minecraft:white_concrete"
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captures_group_without_liberties() {
        let size = 5;
        let mut board = vec![0; size * size];
        board[index_of(2, 2, size)] = 2;
        board[index_of(1, 2, size)] = 1;
        board[index_of(2, 1, size)] = 1;
        board[index_of(3, 2, size)] = 1;
        let outcome = play_stone(&board, &[], size, 2, 3, 1).unwrap();
        assert_eq!(outcome.captured, 1);
        assert_eq!(outcome.board[index_of(2, 2, size)], 0);
    }

    #[test]
    fn rejects_suicide_without_capture() {
        let size = 5;
        let mut board = vec![0; size * size];
        board[index_of(1, 2, size)] = 2;
        board[index_of(2, 1, size)] = 2;
        board[index_of(3, 2, size)] = 2;
        board[index_of(2, 3, size)] = 2;
        assert_eq!(
            play_stone(&board, &[], size, 2, 2, 1).unwrap_err(),
            MoveError::Suicide
        );
    }

    #[test]
    fn scores_enclosed_area_for_single_color() {
        let size = 5;
        let mut board = vec![0; size * size];
        for x in 1..=3 {
            board[index_of(x, 1, size)] = 1;
            board[index_of(x, 3, size)] = 1;
        }
        board[index_of(1, 2, size)] = 1;
        board[index_of(3, 2, size)] = 1;
        let (black, white) = score_area(&board, size);
        assert_eq!(black, 9);
        assert_eq!(white, 0);
    }
}

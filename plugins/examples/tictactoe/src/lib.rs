use qexed_plugin_sdk::{
    BlockStepPayload, ConfigReloadPayload, HttpHeader, HttpRequest, NpcInteractPayload,
    NpcMutationOp, NpcMutationResponse, NpcUpsert, PlaceholderQuery, PlaceholderReplacement,
    PlaceholderResponse, PlayerAction, PlayerBlockInteractPayload, PlayerPayload,
    PlayerTickPayload, PluginCommandDefinition, PluginCommandQuery, PluginCommandResponse,
    PluginManifest, WorldEditRegion, config_load_or_create, config_read_to_string, http_request,
    storage_delete, storage_get_typed, storage_set_typed, time_millis, world_register_edit_region,
    world_set_blocks,
};
use serde::{Deserialize, Serialize};

qexed_plugin_sdk::qexed_plugin_memory!();
qexed_plugin_sdk::qexed_plugin_manifest!(PluginManifest {
    id: "qexed.tictactoe".to_string(),
    version: "0.1.0".to_string(),
    depends: Vec::new(),
    optional_depends: Vec::new(),
    services: Vec::new(),
});

const CONFIG_PATH: &str = "config.toml";
const COMMAND_NAME: &str = "tictactoe";
const NPC_KEY: &str = "tictactoe:guide";
const NPC_EVENT: &str = "tictactoe";
const SESSION_PREFIX: &str = "tictactoe/session/";
const GAME_PREFIX: &str = "tictactoe/game/";
const ROOM_PREFIX: &str = "tictactoe/room/";
const PENDING_PREFIX: &str = "tictactoe/pending/";
const STATS_PREFIX: &str = "tictactoe/stats/";
const QUEUE_KEY: &str = "tictactoe/queue";
const INSTANCE_GRID_WIDTH: u64 = 4096;
const BOARD_CELLS: usize = 9;
const WIN_LINES: [[usize; 3]; 8] = [
    [0, 1, 2],
    [3, 4, 5],
    [6, 7, 8],
    [0, 3, 6],
    [1, 4, 7],
    [2, 5, 8],
    [0, 4, 8],
    [2, 4, 6],
];

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_priority() -> i32 {
    275
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_init() {
    let _ = config_load_or_create(CONFIG_PATH, DEFAULT_CONFIG);
    let config = load_config();
    register_lobby(&config);
    build_lobby(&config);
    qexed_plugin_sdk::log("tictactoe initialized");
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_config_reload(ptr: i32, len: i32) {
    let Some(payload) =
        (unsafe { qexed_plugin_sdk::decode_payload::<ConfigReloadPayload>(ptr, len) })
    else {
        return;
    };
    let path = payload.path.replace('\\', "/");
    if path.ends_with("tictactoe/config.toml") || path.ends_with(CONFIG_PATH) {
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
    if load_config().enable {
        let _ = storage_delete(&session_key(&payload.uuid));
        let _ = storage_delete(&pending_key(&payload.uuid));
    }
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
        description_key: "qexed.plugin.tictactoe.command.description".to_string(),
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
                name: "TicTacToe".to_string(),
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
pub extern "C" fn qexed_plugin_player_block_step(ptr: i32, len: i32) -> i64 {
    let Some(payload) = (unsafe { qexed_plugin_sdk::decode_payload::<BlockStepPayload>(ptr, len) })
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
    let session = stored_session(&player.uuid);
    let game = session
        .as_ref()
        .and_then(|session| stored_game(&session.game_id));
    let stats = stored_stats(&player.uuid);
    qexed_plugin_sdk::response_ptr_len(&PlaceholderResponse {
        replacements: replacements(game.as_ref(), &player.uuid, stats.as_ref()),
    })
}

fn handle_command(config: &Config, payload: &PluginCommandQuery) -> PluginCommandResponse {
    if !config.enable {
        return handled(vec![message("井字棋插件已禁用")]);
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
        "move" | "put" | "place" => move_command(
            config,
            &payload.player.uuid,
            &payload.player.username,
            parts.collect(),
        ),
        "status" | "info" => status_command(&payload.player.uuid),
        "leave" | "lobby" | "spawn" => leave_to_lobby(config, &payload.player.uuid),
        _ => handled(vec![message(
            "用法: /tictactoe match | bot | create | join <房间码> | move <1-9> | leave",
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
        "井字棋: /tictactoe match 匹配, /tictactoe create 创建房间, /tictactoe join <房间码> 加入, /tictactoe bot 人机, 点击棋格或 /tictactoe move <1-9> 落子",
    )])
}

fn join_match_queue(
    config: &Config,
    player_uuid: &str,
    player_name: &str,
) -> PluginCommandResponse {
    if let Some(session) = active_session(player_uuid) {
        if let Some(game) = stored_game(&session.game_id) {
            return handled(vec![
                message(format!("你已经在一局井字棋中: {}", status_label(&game))),
                teleport_to_game(&game),
            ]);
        }
    }

    notify_matchmaking_api(config, "match", player_uuid, player_name, "");

    let now = time_millis();
    let queue = storage_get_typed::<MatchQueue>(QUEUE_KEY);
    if let Some(queue) = queue {
        if queue.player_uuid == player_uuid {
            return handled(vec![message("你已经在匹配队列中，请等待另一名玩家加入")]);
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
            save_started_game(&game);
            push_pending(
                &queue.player_uuid,
                start_actions(config, &game, &queue.player_uuid, "匹配成功，你执 X 先手"),
            );
            return handled(start_actions(
                config,
                &game,
                player_uuid,
                "匹配成功，你执 O 后手",
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
    handled(vec![message("已进入井字棋匹配队列，等待下一名玩家加入")])
}

fn start_bot_game(config: &Config, player_uuid: &str, player_name: &str) -> PluginCommandResponse {
    if active_session(player_uuid).is_some() {
        return handled(vec![message(
            "你已经在一局井字棋中，先 /tictactoe leave 再开始人机",
        )]);
    }
    notify_matchmaking_api(config, "bot", player_uuid, player_name, "");
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
    save_started_game(&game);
    handled(start_actions(
        config,
        &game,
        player_uuid,
        "人机井字棋开始，你执 X 先手",
    ))
}

fn create_room(config: &Config, player_uuid: &str, player_name: &str) -> PluginCommandResponse {
    if active_session(player_uuid).is_some() {
        return handled(vec![message(
            "你已经在一局井字棋中，先 /tictactoe leave 再创建房间",
        )]);
    }
    notify_matchmaking_api(config, "create_room", player_uuid, player_name, "");
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
        &PlayerSession { game_id: game.id },
    );
    handled(vec![
        message(format!(
            "房间已创建，房间码 {}。另一名玩家输入 /tictactoe join {} 加入",
            room_code, room_code
        )),
        PlayerAction::OpenMenu {
            menu: config.menu_id.clone(),
        },
    ])
}

fn join_room(
    config: &Config,
    player_uuid: &str,
    player_name: &str,
    code: &str,
) -> PluginCommandResponse {
    let code = normalize_room_code(code);
    if code.is_empty() {
        return handled(vec![message("用法: /tictactoe join <房间码>")]);
    }
    if active_session(player_uuid).is_some() {
        return handled(vec![message(
            "你已经在一局井字棋中，先 /tictactoe leave 再加入房间",
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
    if game.x_player == player_uuid {
        return handled(vec![message("不能加入自己的房间，请等待另一名玩家")]);
    }
    notify_matchmaking_api(config, "join_room", player_uuid, player_name, &code);

    game.o_player = player_uuid.to_string();
    game.o_name = player_name.to_string();
    game.status = "playing".to_string();
    game.started_at_ms = time_millis();
    let _ = storage_delete(&room_key(&code));
    save_started_game(&game);
    push_pending(
        &game.x_player,
        start_actions(config, &game, &game.x_player, "对手已加入，你执 X 先手"),
    );
    handled(start_actions(
        config,
        &game,
        player_uuid,
        "加入成功，你执 O 后手",
    ))
}

fn move_command(
    config: &Config,
    player_uuid: &str,
    player_name: &str,
    args: Vec<&str>,
) -> PluginCommandResponse {
    if args.len() != 1 {
        return handled(vec![message("用法: /tictactoe move <1-9>")]);
    }
    let Some(index) = args[0]
        .parse::<usize>()
        .ok()
        .and_then(|value| value.checked_sub(1))
        .filter(|value| *value < BOARD_CELLS)
    else {
        return handled(vec![message("格子编号必须是 1 到 9")]);
    };
    make_move(config, player_uuid, player_name, index)
}

fn status_command(player_uuid: &str) -> PluginCommandResponse {
    let Some(session) = stored_session(player_uuid) else {
        return handled(vec![message("当前没有井字棋对局")]);
    };
    let Some(game) = stored_game(&session.game_id) else {
        return handled(vec![message("当前对局数据不存在")]);
    };
    handled(vec![
        message(format!(
            "井字棋状态: {}, 模式: {}, 回合: {}, 对手: {}",
            status_label(&game),
            mode_label(&game),
            turn_label(&game),
            opponent_label(&game, player_uuid)
        )),
        progress_bar(player_uuid, &game),
        teleport_to_game(&game),
    ])
}

fn leave_to_lobby(config: &Config, player_uuid: &str) -> PluginCommandResponse {
    exit_current_game(config, player_uuid);
    let mut actions = vec![
        message("正在返回大厅"),
        PlayerAction::SetPlayersVisible { visible: true },
        PlayerAction::RemoveBossBar {
            id: boss_bar_id(player_uuid),
        },
        PlayerAction::Teleport {
            dimension: config.lobby.dimension.clone(),
            x: config.lobby.spawn_x,
            y: config.lobby.spawn_y,
            z: config.lobby.spawn_z,
            yaw: Some(config.lobby.spawn_yaw),
            pitch: Some(config.lobby.spawn_pitch),
        },
    ];
    if !config.lobby_server.trim().is_empty() {
        actions.push(PlayerAction::ProxyConnect {
            server: config.lobby_server.clone(),
            message: String::new(),
        });
    }
    handled(actions)
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
                game.winner = if game.x_player == player_uuid { 2 } else { 1 };
                finish_game(&mut game);
                save_game(&game);
                update_stats_for_finished_game(&game);
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
    player_name: &str,
    dimension: &str,
    x: i32,
    y: i32,
    z: i32,
) -> PluginCommandResponse {
    if !config.enable {
        return PluginCommandResponse::default();
    }
    let Some(session) = active_session(player_uuid) else {
        return PluginCommandResponse::default();
    };
    let Some(game) = stored_game(&session.game_id) else {
        return PluginCommandResponse::default();
    };
    if game.dimension != dimension {
        return PluginCommandResponse::default();
    }
    let Some(index) = cell_from_position(&game, x, y, z) else {
        return PluginCommandResponse::default();
    };
    make_move(config, player_uuid, player_name, index)
}

fn make_move(
    config: &Config,
    player_uuid: &str,
    _player_name: &str,
    index: usize,
) -> PluginCommandResponse {
    let Some(session) = active_session(player_uuid) else {
        return handled(vec![message("当前没有进行中的井字棋")]);
    };
    let Some(mut game) = stored_game(&session.game_id) else {
        return handled(vec![message("当前对局数据不存在")]);
    };
    if game.status != "playing" {
        return handled(vec![message(format!("当前对局{}", status_label(&game)))]);
    }
    let Some(mark) = mark_for_player(&game, player_uuid) else {
        return handled(vec![message("你不是这局井字棋的玩家")]);
    };
    if game.turn != mark {
        return handled(vec![message(format!(
            "还没轮到你，当前回合: {}",
            turn_label(&game)
        ))]);
    }
    if game.board.get(index).copied().unwrap_or(1) != 0 {
        return handled(vec![message("这个格子已经有棋子了")]);
    }

    apply_mark(&mut game, index, mark);
    let mut actions = vec![message(format!("你落在 {} 号格", index + 1))];
    if game.status == "playing" && game.bot && game.turn == 2 {
        if let Some(bot_index) = bot_move_index(&game) {
            apply_mark(&mut game, bot_index, 2);
            actions.push(message(format!(
                "{} 落在 {} 号格",
                game.o_name,
                bot_index + 1
            )));
        }
    }
    save_game(&game);
    render_board(config, &game);

    if game.status == "playing" {
        actions.push(progress_bar(player_uuid, &game));
        if let Some(opponent) = opponent_uuid(&game, player_uuid) {
            push_pending(
                opponent,
                vec![
                    message("对方已落子，轮到你"),
                    progress_bar(opponent, &game),
                    teleport_to_game(&game),
                ],
            );
        }
    } else {
        update_stats_for_finished_game(&game);
        actions.extend(finish_actions(config, &game, player_uuid));
        if let Some(opponent) = opponent_uuid(&game, player_uuid) {
            push_pending(opponent, finish_actions(config, &game, opponent));
        }
    }

    handled(actions)
}

fn apply_mark(game: &mut GameState, index: usize, mark: u8) {
    if index >= BOARD_CELLS || game.board[index] != 0 {
        return;
    }
    game.board[index] = mark;
    if winner(&game.board).is_some() {
        game.status = "won".to_string();
        game.winner = mark;
        finish_game(game);
        return;
    }
    if game.board.iter().all(|cell| *cell != 0) {
        game.status = "draw".to_string();
        game.winner = 0;
        finish_game(game);
        return;
    }
    game.turn = if mark == 1 { 2 } else { 1 };
}

fn save_started_game(game: &GameState) {
    register_arena(game);
    render_board(&load_config(), game);
    save_game(game);
    let _ = storage_set_typed(
        &session_key(&game.x_player),
        &PlayerSession {
            game_id: game.id.clone(),
        },
    );
    if !game.o_player.is_empty() && game.o_player != "bot" {
        let _ = storage_set_typed(
            &session_key(&game.o_player),
            &PlayerSession {
                game_id: game.id.clone(),
            },
        );
    }
}

fn save_game(game: &GameState) {
    let _ = storage_set_typed(&game_key(&game.id), game);
}

fn finish_game(game: &mut GameState) {
    game.ended_elapsed_ms = elapsed_millis(game.started_at_ms);
}

fn update_stats_for_finished_game(game: &GameState) {
    if game.ended_elapsed_ms <= 0 {
        return;
    }
    update_player_stats(
        &game.x_player,
        result_for_mark(game, 1),
        game.ended_elapsed_ms,
    );
    if !game.bot && !game.o_player.is_empty() {
        update_player_stats(
            &game.o_player,
            result_for_mark(game, 2),
            game.ended_elapsed_ms,
        );
    }
}

fn update_player_stats(player_uuid: &str, result: PlayerResult, elapsed_ms: i64) {
    let mut stats = stored_stats(player_uuid).unwrap_or_default();
    match result {
        PlayerResult::Win => {
            stats.wins = stats.wins.saturating_add(1);
            stats.best_win_ms = match stats.best_win_ms {
                0 => elapsed_ms,
                current => current.min(elapsed_ms),
            };
        }
        PlayerResult::Loss => stats.losses = stats.losses.saturating_add(1),
        PlayerResult::Draw => stats.draws = stats.draws.saturating_add(1),
    }
    let _ = storage_set_typed(&stats_key(player_uuid), &stats);
}

fn result_for_mark(game: &GameState, mark: u8) -> PlayerResult {
    if game.status == "draw" {
        return PlayerResult::Draw;
    }
    if game.winner == mark {
        PlayerResult::Win
    } else {
        PlayerResult::Loss
    }
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
    if game.status == "draw" {
        return format!("平局，用时 {}", format_duration_ms(game.ended_elapsed_ms));
    }
    let Some(mark) = mark_for_player(game, player_uuid) else {
        return "对局结束".to_string();
    };
    if game.winner == mark {
        format!("你赢了，用时 {}", format_duration_ms(game.ended_elapsed_ms))
    } else {
        format!("你输了，用时 {}", format_duration_ms(game.ended_elapsed_ms))
    }
}

fn bot_move_index(game: &GameState) -> Option<usize> {
    winning_move(&game.board, 2)
        .or_else(|| winning_move(&game.board, 1))
        .or_else(|| empty_if(game, 4))
        .or_else(|| {
            [0, 2, 6, 8]
                .into_iter()
                .find(|index| game.board[*index] == 0)
        })
        .or_else(|| game.board.iter().position(|cell| *cell == 0))
}

fn winning_move(board: &[u8], mark: u8) -> Option<usize> {
    for line in WIN_LINES {
        let values = [board[line[0]], board[line[1]], board[line[2]]];
        let marks = values.iter().filter(|value| **value == mark).count();
        let empty = values.iter().filter(|value| **value == 0).count();
        if marks == 2 && empty == 1 {
            return line.into_iter().find(|index| board[*index] == 0);
        }
    }
    None
}

fn empty_if(game: &GameState, index: usize) -> Option<usize> {
    (game.board.get(index).copied() == Some(0)).then_some(index)
}

fn winner(board: &[u8]) -> Option<u8> {
    WIN_LINES.into_iter().find_map(|line| {
        let mark = board[line[0]];
        (mark != 0 && board[line[1]] == mark && board[line[2]] == mark).then_some(mark)
    })
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
        message("点击棋盘格或使用 /tictactoe move <1-9> 落子，编号从左到右、从上到下"),
        PlayerAction::SetPlayersVisible { visible: true },
        progress_bar(player_uuid, game),
        teleport_to_game(game),
    ]
}

fn teleport_to_game(game: &GameState) -> PlayerAction {
    PlayerAction::Teleport {
        dimension: game.dimension.clone(),
        x: game.origin_x as f64 + 1.5,
        y: game.origin_y as f64 + 1.0,
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
            "井字棋 {} | 回合 {} | {}",
            status_label(game),
            turn_label(game),
            board_line(game)
        ),
        progress: (filled / BOARD_CELLS as f32).clamp(0.0, 1.0),
        color: if game.turn == 1 { "red" } else { "blue" }.to_string(),
        overlay: "progress".to_string(),
    }
}

fn board_line(game: &GameState) -> String {
    game.board
        .iter()
        .enumerate()
        .map(|(index, cell)| match cell {
            1 => "X".to_string(),
            2 => "O".to_string(),
            _ => (index + 1).to_string(),
        })
        .collect::<Vec<_>>()
        .join("")
}

fn register_lobby(config: &Config) {
    let center_x = config.lobby.spawn_x.floor() as i32;
    let center_z = config.lobby.spawn_z.floor() as i32;
    let y = config.lobby.floor_y;
    let _ = world_register_edit_region(&WorldEditRegion {
        id: "tictactoe_lobby",
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
    let id = format!("tictactoe_{}", game.id);
    let _ = world_register_edit_region(&WorldEditRegion {
        id: &id,
        dimension: &game.dimension,
        min: (game.origin_x - 2, game.origin_y - 2, game.origin_z - 3),
        max: (game.origin_x + 4, game.origin_y + 6, game.origin_z + 4),
        allow_player_break: false,
        allow_player_place: false,
        allow_plugin_write: true,
        runtime_only: true,
    });
}

fn render_board(config: &Config, game: &GameState) -> bool {
    let mut blocks = Vec::new();
    let dimension = game.dimension.as_str();
    for dx in -1..=3 {
        for dz in -2..=3 {
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
    for dx in -1..=3 {
        for dz in -1..=3 {
            if dx == -1 || dx == 3 || dz == -1 || dz == 3 {
                blocks.push((
                    dimension,
                    (game.origin_x + dx, game.origin_y, game.origin_z + dz),
                    config.board.border_block.as_str(),
                ));
            }
        }
    }
    for dx in 0..=2 {
        blocks.push((
            dimension,
            (game.origin_x + dx, game.origin_y, game.origin_z - 2),
            config.board.floor_block.as_str(),
        ));
    }
    for index in 0..BOARD_CELLS {
        let (x, z) = index_to_cell(index);
        blocks.push((
            dimension,
            (
                game.origin_x + x as i32,
                game.origin_y,
                game.origin_z + z as i32,
            ),
            block_for_mark(config, game.board[index]),
        ));
    }
    world_set_blocks(blocks)
}

fn block_for_mark(config: &Config, mark: u8) -> &str {
    match mark {
        1 => config.board.x_block.as_str(),
        2 => config.board.o_block.as_str(),
        _ => config.board.empty_block.as_str(),
    }
}

fn cell_from_position(game: &GameState, x: i32, y: i32, z: i32) -> Option<usize> {
    if y != game.origin_y {
        return None;
    }
    let local_x = x.checked_sub(game.origin_x)?;
    let local_z = z.checked_sub(game.origin_z)?;
    if !(0..=2).contains(&local_x) || !(0..=2).contains(&local_z) {
        return None;
    }
    Some(local_z as usize * 3 + local_x as usize)
}

fn index_to_cell(index: usize) -> (usize, usize) {
    (index % 3, index / 3)
}

fn new_game(
    config: &Config,
    mode: &str,
    x_player: &str,
    x_name: &str,
    o_player: &str,
    o_name: &str,
    room_code: &str,
) -> GameState {
    let id = game_id_for(x_player, o_player, time_millis());
    GameState {
        id: id.clone(),
        status: "playing".to_string(),
        mode: mode.to_string(),
        room_code: room_code.to_string(),
        x_player: x_player.to_string(),
        x_name: x_name.to_string(),
        o_player: o_player.to_string(),
        o_name: o_name.to_string(),
        bot: o_player == "bot",
        board: vec![0; BOARD_CELLS],
        turn: 1,
        winner: 0,
        dimension: config.board.dimension.clone(),
        origin_x: instance_origin(config, &id).0,
        origin_y: instance_origin(config, &id).1,
        origin_z: instance_origin(config, &id).2,
        started_at_ms: time_millis(),
        ended_elapsed_ms: 0,
    }
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

fn notify_matchmaking_api(
    config: &Config,
    event: &str,
    player_uuid: &str,
    player_name: &str,
    room: &str,
) {
    let url = config.matchmaking.api_url.trim();
    if url.is_empty() {
        return;
    }
    let body = format!(
        "{{\"server\":\"{}\",\"event\":\"{}\",\"player\":\"{}\",\"name\":\"{}\",\"room\":\"{}\"}}",
        escape_json(&config.server_name),
        escape_json(event),
        escape_json(player_uuid),
        escape_json(player_name),
        escape_json(room)
    );
    let response = http_request(&HttpRequest {
        method: "POST".to_string(),
        url: url.to_string(),
        headers: vec![HttpHeader {
            name: "content-type".to_string(),
            value: "application/json".to_string(),
        }],
        body: body.into_bytes(),
        timeout_ms: config.matchmaking.timeout_ms,
    });
    if let Some(response) = response {
        if response.status == 0 {
            qexed_plugin_sdk::log(&format!(
                "tictactoe matchmaking api failed: {}",
                response.error
            ));
        }
    }
}

fn escape_json(value: &str) -> String {
    let mut out = String::new();
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(ch),
        }
    }
    out
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

fn active_session(player_uuid: &str) -> Option<PlayerSession> {
    stored_session(player_uuid).and_then(|session| {
        let game = stored_game(&session.game_id)?;
        matches!(game.status.as_str(), "waiting" | "playing").then_some(session)
    })
}

fn stored_session(player_uuid: &str) -> Option<PlayerSession> {
    storage_get_typed(&session_key(player_uuid))
}

fn stored_game(game_id: &str) -> Option<GameState> {
    storage_get_typed(&game_key(game_id))
}

fn stored_stats(player_uuid: &str) -> Option<PlayerStats> {
    storage_get_typed(&stats_key(player_uuid))
}

fn remove_from_queue(player_uuid: &str) {
    if storage_get_typed::<MatchQueue>(QUEUE_KEY)
        .is_some_and(|queue| queue.player_uuid == player_uuid)
    {
        let _ = storage_delete(QUEUE_KEY);
    }
}

fn mark_for_player(game: &GameState, player_uuid: &str) -> Option<u8> {
    if game.x_player == player_uuid {
        Some(1)
    } else if game.o_player == player_uuid {
        Some(2)
    } else {
        None
    }
}

fn opponent_uuid<'a>(game: &'a GameState, player_uuid: &str) -> Option<&'a str> {
    if game.x_player == player_uuid && !game.o_player.is_empty() && game.o_player != "bot" {
        Some(game.o_player.as_str())
    } else if game.o_player == player_uuid {
        Some(game.x_player.as_str())
    } else {
        None
    }
}

fn opponent_label(game: &GameState, player_uuid: &str) -> String {
    if game.x_player == player_uuid {
        game.o_name.clone()
    } else if game.o_player == player_uuid {
        game.x_name.clone()
    } else {
        "-".to_string()
    }
}

fn turn_label(game: &GameState) -> String {
    match game.turn {
        1 => format!("X({})", game.x_name),
        2 => format!("O({})", game.o_name),
        _ => "-".to_string(),
    }
}

fn status_label(game: &GameState) -> String {
    match game.status.as_str() {
        "waiting" => "等待玩家".to_string(),
        "playing" => "进行中".to_string(),
        "won" => format!("{} 获胜", if game.winner == 1 { "X" } else { "O" }),
        "draw" => "平局".to_string(),
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

fn replacements(
    game: Option<&GameState>,
    player_uuid: &str,
    stats: Option<&PlayerStats>,
) -> Vec<PlaceholderReplacement> {
    vec![
        placeholder(
            "tictactoe_state",
            game.map(status_label).unwrap_or_else(|| "大厅".to_string()),
        ),
        placeholder(
            "tictactoe_mode",
            game.map(mode_label).unwrap_or_else(|| "-".to_string()),
        ),
        placeholder(
            "tictactoe_turn",
            game.map(turn_label).unwrap_or_else(|| "-".to_string()),
        ),
        placeholder(
            "tictactoe_opponent",
            game.map(|game| opponent_label(game, player_uuid))
                .unwrap_or_else(|| "-".to_string()),
        ),
        placeholder(
            "tictactoe_room",
            game.and_then(|game| (!game.room_code.is_empty()).then(|| game.room_code.clone()))
                .unwrap_or_else(|| "-".to_string()),
        ),
        placeholder(
            "tictactoe_time",
            game.map(game_elapsed_label)
                .unwrap_or_else(|| "--:--".to_string()),
        ),
        placeholder(
            "tictactoe_record",
            stats
                .map(|stats| format!("{}/{}/{}", stats.wins, stats.losses, stats.draws))
                .unwrap_or_else(|| "0/0/0".to_string()),
        ),
        placeholder(
            "tictactoe_best",
            stats
                .and_then(|stats| (stats.best_win_ms > 0).then_some(stats.best_win_ms))
                .map(format_duration_ms)
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
    format!("tictactoe:{player_uuid}")
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

fn stats_key(player_uuid: &str) -> String {
    format!("{STATS_PREFIX}{player_uuid}")
}

fn load_config() -> Config {
    config_read_to_string(CONFIG_PATH)
        .and_then(|contents| toml::from_str(&contents).ok())
        .unwrap_or_default()
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
    x_player: String,
    x_name: String,
    #[serde(default)]
    o_player: String,
    #[serde(default)]
    o_name: String,
    #[serde(default)]
    bot: bool,
    board: Vec<u8>,
    turn: u8,
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
        player_uuid: &str,
        player_name: &str,
        dimension: &str,
        origin: (i32, i32, i32),
    ) -> Self {
        Self {
            id: id.to_string(),
            status: "waiting".to_string(),
            mode: "room".to_string(),
            room_code: room_code.to_string(),
            x_player: player_uuid.to_string(),
            x_name: player_name.to_string(),
            o_player: String::new(),
            o_name: String::new(),
            bot: false,
            board: vec![0; BOARD_CELLS],
            turn: 1,
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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct PlayerStats {
    wins: u32,
    losses: u32,
    draws: u32,
    best_win_ms: i64,
}

#[derive(Debug, Clone, Copy)]
enum PlayerResult {
    Win,
    Loss,
    Draw,
}

#[derive(Debug, Clone, Deserialize)]
struct Config {
    #[serde(default = "default_true")]
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
            enable: true,
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

#[derive(Debug, Clone, Deserialize)]
struct MatchmakingConfig {
    #[serde(default)]
    api_url: String,
    #[serde(default = "default_http_timeout_ms")]
    timeout_ms: u64,
    #[serde(default = "default_queue_ttl_ms")]
    queue_ttl_ms: i64,
}

impl Default for MatchmakingConfig {
    fn default() -> Self {
        Self {
            api_url: String::new(),
            timeout_ms: default_http_timeout_ms(),
            queue_ttl_ms: default_queue_ttl_ms(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
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

#[derive(Debug, Clone, Deserialize)]
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

#[derive(Debug, Clone, Deserialize)]
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
    #[serde(default = "default_empty_block")]
    empty_block: String,
    #[serde(default = "default_x_block")]
    x_block: String,
    #[serde(default = "default_o_block")]
    o_block: String,
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
            empty_block: default_empty_block(),
            x_block: default_x_block(),
            o_block: default_o_block(),
        }
    }
}

fn default_true() -> bool {
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
    "tictactoe_select".to_string()
}

fn default_http_timeout_ms() -> u64 {
    2000
}

fn default_queue_ttl_ms() -> i64 {
    300_000
}

fn default_bot_name() -> String {
    "浅屿机器人".to_string()
}

fn default_lobby_dimension() -> String {
    "qexed:tictactoe_lobby".to_string()
}

fn default_board_dimension() -> String {
    "qexed:tictactoe_game".to_string()
}

fn default_floor_y() -> i32 {
    -52
}

fn default_spawn_x() -> f64 {
    0.5
}

fn default_spawn_y() -> f64 {
    -52.0
}

fn default_spawn_z() -> f64 {
    0.5
}

fn default_npc_x() -> f64 {
    0.5
}

fn default_npc_y() -> f64 {
    -52.0
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
    "{\"text\":\"井字棋入口\",\"color\":\"gold\"}".to_string()
}

fn default_board_origin_x() -> i32 {
    2048
}

fn default_board_origin_z() -> i32 {
    2048
}

fn default_instance_spacing() -> i32 {
    32
}

fn default_floor_block() -> String {
    "minecraft:smooth_stone".to_string()
}

fn default_border_block() -> String {
    "minecraft:deepslate_tiles".to_string()
}

fn default_empty_block() -> String {
    "minecraft:white_concrete".to_string()
}

fn default_x_block() -> String {
    "minecraft:red_concrete".to_string()
}

fn default_o_block() -> String {
    "minecraft:blue_concrete".to_string()
}

const DEFAULT_CONFIG: &str = r#"enable = true
server_name = "浅屿闲游"
qq_group = "722632621"
lobby_server = "lobby_1"
menu_id = "tictactoe_select"

[matchmaking]
api_url = ""
timeout_ms = 2000
queue_ttl_ms = 300000

[bot]
name = "浅屿机器人"

[lobby]
dimension = "qexed:tictactoe_lobby"
floor_y = -52
spawn_x = 0.5
spawn_y = -52.0
spawn_z = 0.5
spawn_yaw = 0.0
spawn_pitch = 0.0
npc_x = 0.5
npc_y = -52.0
npc_z = 3.5
npc_yaw = 180.0
npc_pitch = 0.0
npc_entity_type = "minecraft:villager"
npc_display_name = "{\"text\":\"井字棋入口\",\"color\":\"gold\"}"

[board]
dimension = "qexed:tictactoe_game"
origin_x = 2048
origin_y = -52
origin_z = 2048
instance_spacing = 32
floor_block = "minecraft:smooth_stone"
border_block = "minecraft:deepslate_tiles"
empty_block = "minecraft:white_concrete"
x_block = "minecraft:red_concrete"
o_block = "minecraft:blue_concrete"
"#;

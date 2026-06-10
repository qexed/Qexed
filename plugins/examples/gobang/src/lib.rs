use qexed_plugin_sdk::{
    ConfigReloadPayload, HttpHeader, HttpRequest, NpcInteractPayload, NpcMutationOp,
    NpcMutationResponse, NpcUpsert, PlaceholderQuery, PlaceholderReplacement, PlaceholderResponse,
    PlayerAction, PlayerBlockInteractPayload, PlayerPayload, PlayerTickPayload,
    PluginCommandDefinition, PluginCommandQuery, PluginCommandResponse, PluginManifest,
    WorldEditRegion, config_load_or_create, config_read_to_string, http_request, storage_delete,
    storage_get_typed, storage_set_typed, time_millis, world_register_edit_region,
    world_set_blocks,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

qexed_plugin_sdk::qexed_plugin_memory!();
qexed_plugin_sdk::qexed_plugin_manifest!(PluginManifest {
    id: "qexed.gobang".to_string(),
    version: "0.1.0".to_string(),
    depends: Vec::new(),
    optional_depends: Vec::new(),
    services: Vec::new(),
});

const CONFIG_PATH: &str = "config.toml";
const COMMAND_NAME: &str = "gobang";
const NPC_KEY: &str = "gobang:guide";
const NPC_EVENT: &str = "gobang";
const SESSION_PREFIX: &str = "gobang/session/";
const GAME_PREFIX: &str = "gobang/game/";
const ROOM_PREFIX: &str = "gobang/room/";
const PENDING_PREFIX: &str = "gobang/pending/";
const QUEUE_KEY: &str = "gobang/queue";
const BOARD_SIZE: usize = 15;
const WIN_LENGTH: usize = 5;
const INSTANCE_GRID_WIDTH: u64 = 2048;

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_priority() -> i32 {
    276
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_init() {
    let _ = config_load_or_create(CONFIG_PATH, DEFAULT_CONFIG);
    let config = load_config();
    register_lobby(&config);
    build_lobby(&config);
    qexed_plugin_sdk::log("gobang initialized");
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_config_reload(ptr: i32, len: i32) {
    let Some(payload) =
        (unsafe { qexed_plugin_sdk::decode_payload::<ConfigReloadPayload>(ptr, len) })
    else {
        return;
    };
    let path = payload.path.replace('\\', "/");
    if path.ends_with("gobang/config.toml") || path.ends_with(CONFIG_PATH) {
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
        description_key: "qexed.plugin.gobang.command.description".to_string(),
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
                name: "Gobang".to_string(),
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
        replacements: replacements(game.as_ref(), &player.uuid, &load_config()),
    })
}

fn handle_command(config: &Config, payload: &PluginCommandQuery) -> PluginCommandResponse {
    if !config.enable {
        return handled(vec![message("五子棋插件已禁用")]);
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
        "status" | "info" => status_command(config, &payload.player.uuid),
        "leave" | "lobby" | "spawn" => leave_to_lobby(config, &payload.player.uuid),
        _ => handled(vec![message(
            "用法: /gobang match | bot | create | join <房间码> | move <x> <z> | leave",
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
        "五子棋: /gobang bot 人机, /gobang match 匹配, /gobang create 创建房间, /gobang join <房间码> 加入, 点击棋盘或 /gobang move <x> <z> 落子",
    )])
}

fn join_match_queue(
    config: &Config,
    player_uuid: &str,
    player_name: &str,
) -> PluginCommandResponse {
    if active_session(player_uuid).is_some() {
        return handled(vec![message(
            "你已经在一局五子棋中，先 /gobang leave 再匹配",
        )]);
    }
    let now = time_millis();
    if let Some(queue) = storage_get_typed::<MatchQueue>(QUEUE_KEY) {
        if queue.player_uuid == player_uuid {
            return handled(vec![message("你已经在五子棋匹配队列中")]);
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
    handled(vec![message("已进入五子棋匹配队列，等待下一名玩家加入")])
}

fn start_bot_game(config: &Config, player_uuid: &str, player_name: &str) -> PluginCommandResponse {
    if active_session(player_uuid).is_some() {
        return handled(vec![message(
            "你已经在一局五子棋中，先 /gobang leave 再开始人机",
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
    let ai_label = if config.ai.enable_openai {
        format!("AI 已配置为 {}", config.ai.model)
    } else {
        "当前使用本地启发式人机，外部 AI 默认未启用".to_string()
    };
    let mut actions = start_actions(config, &game, player_uuid, "人机五子棋开始，你执黑先手");
    actions.push(message(ai_label));
    handled(actions)
}

fn create_room(config: &Config, player_uuid: &str, player_name: &str) -> PluginCommandResponse {
    if active_session(player_uuid).is_some() {
        return handled(vec![message(
            "你已经在一局五子棋中，先 /gobang leave 再创建房间",
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
        &PlayerSession { game_id: game.id },
    );
    handled(vec![message(format!(
        "房间已创建，房间码 {}。另一名玩家输入 /gobang join {} 加入",
        room_code, room_code
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
        return handled(vec![message("用法: /gobang join <房间码>")]);
    }
    if active_session(player_uuid).is_some() {
        return handled(vec![message(
            "你已经在一局五子棋中，先 /gobang leave 再加入房间",
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

fn move_command(
    config: &Config,
    player_uuid: &str,
    player_name: &str,
    args: Vec<&str>,
) -> PluginCommandResponse {
    if args.len() != 2 {
        return handled(vec![message("用法: /gobang move <x> <z>，坐标范围 1..15")]);
    }
    let Some(x) = parse_coord(args[0]) else {
        return handled(vec![message("x 坐标必须是 1..15")]);
    };
    let Some(z) = parse_coord(args[1]) else {
        return handled(vec![message("z 坐标必须是 1..15")]);
    };
    make_move(config, player_uuid, player_name, x, z)
}

fn status_command(config: &Config, player_uuid: &str) -> PluginCommandResponse {
    let Some(session) = stored_session(player_uuid) else {
        return handled(vec![message("当前没有五子棋对局")]);
    };
    let Some(game) = stored_game(&session.game_id) else {
        return handled(vec![message("当前对局数据不存在")]);
    };
    handled(vec![
        message(format!(
            "五子棋状态: {}, 模式: {}, 回合: {}, 对手: {}",
            status_label(&game),
            mode_label(&game),
            turn_label(&game),
            opponent_label(&game, player_uuid)
        )),
        progress_bar(player_uuid, &game),
        teleport_to_game(&game),
        PlayerAction::OpenMenu {
            menu: config.menu_id.clone(),
        },
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
                game.winner = if game.black_player == player_uuid {
                    2
                } else {
                    1
                };
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
    player_name: &str,
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
    make_move(config, player_uuid, player_name, cell_x, cell_z)
}

fn make_move(
    config: &Config,
    player_uuid: &str,
    _player_name: &str,
    x: usize,
    z: usize,
) -> PluginCommandResponse {
    let Some(session) = active_session(player_uuid) else {
        return handled(vec![message("当前没有进行中的五子棋")]);
    };
    let Some(mut game) = stored_game(&session.game_id) else {
        return handled(vec![message("当前对局数据不存在")]);
    };
    if game.status != "playing" {
        return handled(vec![message(format!("当前对局{}", status_label(&game)))]);
    }
    let Some(mark) = mark_for_player(&game, player_uuid) else {
        return handled(vec![message("你不是这局五子棋的玩家")]);
    };
    if game.turn != mark {
        return handled(vec![message(format!(
            "还没轮到你，当前回合: {}",
            turn_label(&game)
        ))]);
    }
    if !is_empty(&game, x, z) {
        return handled(vec![message("这个位置已经有棋子了")]);
    }

    apply_mark(&mut game, x, z, mark);
    let mut actions = vec![message(format!("你落在 ({}, {})", x + 1, z + 1))];
    if game.status == "playing" && game.bot && game.turn == 2 {
        let ai_move = tactical_move(&game, 2)
            .or_else(|| {
                openai_move(config, &game).filter(|(bot_x, bot_z)| {
                    move_preserves_tactical_safety(&game, 2, *bot_x, *bot_z)
                })
            })
            .or_else(|| heuristic_move(&game, 2));
        if let Some((bot_x, bot_z)) = ai_move {
            apply_mark(&mut game, bot_x, bot_z, 2);
            actions.push(message(format!(
                "{} 落在 ({}, {})",
                game.white_name,
                bot_x + 1,
                bot_z + 1
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
        actions.extend(finish_actions(config, &game, player_uuid));
        if let Some(opponent) = opponent_uuid(&game, player_uuid) {
            push_pending(opponent, finish_actions(config, &game, opponent));
        }
    }
    handled(actions)
}

fn apply_mark(game: &mut GameState, x: usize, z: usize, mark: u8) {
    let index = game.index(x, z);
    if game.board[index] != 0 {
        return;
    }
    game.board[index] = mark;
    if has_five(game, x, z, mark) {
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

fn has_five(game: &GameState, x: usize, z: usize, mark: u8) -> bool {
    [(1, 0), (0, 1), (1, 1), (1, -1)]
        .into_iter()
        .any(|(dx, dz)| {
            1 + count_dir(game, x, z, dx, dz, mark) + count_dir(game, x, z, -dx, -dz, mark)
                >= WIN_LENGTH
        })
}

fn count_dir(game: &GameState, x: usize, z: usize, dx: i32, dz: i32, mark: u8) -> usize {
    let mut count = 0;
    let mut nx = x as i32 + dx;
    let mut nz = z as i32 + dz;
    while nx >= 0 && nz >= 0 && nx < BOARD_SIZE as i32 && nz < BOARD_SIZE as i32 {
        if game.board[game.index(nx as usize, nz as usize)] != mark {
            break;
        }
        count += 1;
        nx += dx;
        nz += dz;
    }
    count
}

fn openai_move(config: &Config, game: &GameState) -> Option<(usize, usize)> {
    if !config.ai.enable_openai {
        return None;
    }
    let url = config.ai.api_url.trim();
    if url.is_empty() {
        return None;
    }
    let board = board_prompt(game);
    let body = json!({
        "model": config.ai.model,
        "temperature": config.ai.temperature,
        "messages": [
            {
                "role": "system",
                "content": "你是五子棋 AI。只返回 JSON，例如 {\"x\":8,\"z\":8}。x 和 z 都是 1 到 15 的整数，必须选择空位。"
            },
            {
                "role": "user",
                "content": format!("你执白子 O。棋盘如下，. 为空，X 是黑子，O 是白子。请给出下一步。\n{board}")
            }
        ]
    });
    let mut headers = vec![HttpHeader {
        name: "content-type".to_string(),
        value: "application/json".to_string(),
    }];
    if !config.ai.api_key.trim().is_empty() {
        headers.push(HttpHeader {
            name: "authorization".to_string(),
            value: format!("Bearer {}", config.ai.api_key.trim()),
        });
    }
    let response = http_request(&HttpRequest {
        method: "POST".to_string(),
        url: url.to_string(),
        headers,
        body: serde_json::to_vec(&body).ok()?,
        timeout_ms: config.ai.timeout_ms,
    })?;
    if !(200..300).contains(&response.status) {
        qexed_plugin_sdk::log(&format!(
            "gobang ai request failed: status={}, error={}",
            response.status, response.error
        ));
        return None;
    }
    let value = serde_json::from_slice::<serde_json::Value>(&response.body).ok()?;
    let content = value
        .get("choices")?
        .get(0)?
        .get("message")?
        .get("content")?
        .as_str()?;
    parse_ai_move(content).filter(|(x, z)| is_empty(game, *x, *z))
}

fn parse_ai_move(content: &str) -> Option<(usize, usize)> {
    if let (Some(start), Some(end)) = (content.find('{'), content.rfind('}')) {
        if start <= end {
            let value = serde_json::from_str::<serde_json::Value>(&content[start..=end]).ok()?;
            let x = value.get("x")?.as_u64()? as usize;
            let z = value.get("z")?.as_u64()? as usize;
            return coord_pair(x, z);
        }
    }
    let numbers = content
        .split(|ch: char| !ch.is_ascii_digit())
        .filter(|part| !part.is_empty())
        .filter_map(|part| part.parse::<usize>().ok())
        .collect::<Vec<_>>();
    coord_pair(*numbers.first()?, *numbers.get(1)?)
}

fn coord_pair(x: usize, z: usize) -> Option<(usize, usize)> {
    if (1..=BOARD_SIZE).contains(&x) && (1..=BOARD_SIZE).contains(&z) {
        Some((x - 1, z - 1))
    } else {
        None
    }
}

fn board_prompt(game: &GameState) -> String {
    let mut out = String::new();
    for z in 0..BOARD_SIZE {
        out.push_str(&format!("{:02} ", z + 1));
        for x in 0..BOARD_SIZE {
            out.push(match game.board[game.index(x, z)] {
                1 => 'X',
                2 => 'O',
                _ => '.',
            });
        }
        out.push('\n');
    }
    out
}

fn heuristic_move(game: &GameState, mark: u8) -> Option<(usize, usize)> {
    tactical_move(game, mark).or_else(|| best_scored_move(game, mark))
}

fn tactical_move(game: &GameState, mark: u8) -> Option<(usize, usize)> {
    winning_move(game, mark).or_else(|| winning_move(game, opponent_mark(mark)))
}

fn opponent_mark(mark: u8) -> u8 {
    if mark == 1 { 2 } else { 1 }
}

fn move_preserves_tactical_safety(game: &GameState, mark: u8, x: usize, z: usize) -> bool {
    if !is_empty(game, x, z) {
        return false;
    }
    let mut trial = game.clone();
    apply_mark(&mut trial, x, z, mark);
    if trial.status != "playing" {
        return true;
    }
    winning_move(&trial, opponent_mark(mark)).is_none()
}

fn winning_move(game: &GameState, mark: u8) -> Option<(usize, usize)> {
    for (x, z) in candidate_moves(game) {
        let mut trial = game.clone();
        apply_mark(&mut trial, x, z, mark);
        if trial.status == "won" && trial.winner == mark {
            return Some((x, z));
        }
    }
    None
}

fn best_scored_move(game: &GameState, mark: u8) -> Option<(usize, usize)> {
    candidate_moves(game)
        .into_iter()
        .max_by_key(|(x, z)| move_score(game, *x, *z, mark))
}

fn candidate_moves(game: &GameState) -> Vec<(usize, usize)> {
    if game.board.iter().all(|cell| *cell == 0) {
        return vec![(BOARD_SIZE / 2, BOARD_SIZE / 2)];
    }
    let mut result = Vec::new();
    for z in 0..BOARD_SIZE {
        for x in 0..BOARD_SIZE {
            if !is_empty(game, x, z) {
                continue;
            }
            if has_neighbor(game, x, z, 2) {
                result.push((x, z));
            }
        }
    }
    if result.is_empty() {
        result.push((BOARD_SIZE / 2, BOARD_SIZE / 2));
    }
    result
}

fn has_neighbor(game: &GameState, x: usize, z: usize, radius: i32) -> bool {
    for dz in -radius..=radius {
        for dx in -radius..=radius {
            if dx == 0 && dz == 0 {
                continue;
            }
            let nx = x as i32 + dx;
            let nz = z as i32 + dz;
            if nx >= 0
                && nz >= 0
                && nx < BOARD_SIZE as i32
                && nz < BOARD_SIZE as i32
                && game.board[game.index(nx as usize, nz as usize)] != 0
            {
                return true;
            }
        }
    }
    false
}

fn move_score(game: &GameState, x: usize, z: usize, mark: u8) -> i32 {
    let opponent = if mark == 1 { 2 } else { 1 };
    let center = BOARD_SIZE as i32 / 2;
    let distance = (x as i32 - center).abs() + (z as i32 - center).abs();
    let mut score = 100 - distance;
    for (dx, dz) in [(1, 0), (0, 1), (1, 1), (1, -1)] {
        let own = count_dir(game, x, z, dx, dz, mark) + count_dir(game, x, z, -dx, -dz, mark);
        let block =
            count_dir(game, x, z, dx, dz, opponent) + count_dir(game, x, z, -dx, -dz, opponent);
        score += (own * own * 12 + block * block * 10) as i32;
    }
    score
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
        message("点击棋盘空位或使用 /gobang move <x> <z> 落子，坐标从 1 到 15"),
        PlayerAction::SetPlayersVisible { visible: true },
        progress_bar(player_uuid, game),
        teleport_to_game(game),
    ]
}

fn teleport_to_game(game: &GameState) -> PlayerAction {
    PlayerAction::Teleport {
        dimension: game.dimension.clone(),
        x: game.origin_x as f64 + BOARD_SIZE as f64 / 2.0,
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
            "五子棋 {} | 回合 {} | {}/{}",
            status_label(game),
            turn_label(game),
            filled as usize,
            BOARD_SIZE * BOARD_SIZE
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
        id: "gobang_lobby",
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
    let id = format!("gobang_{}", game.id);
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
    for dx in 0..BOARD_SIZE as i32 {
        blocks.push((
            dimension,
            (game.origin_x + dx, game.origin_y, game.origin_z - 2),
            config.board.floor_block.as_str(),
        ));
    }
    for z in 0..BOARD_SIZE {
        for x in 0..BOARD_SIZE {
            let mark = game.board[game.index(x, z)];
            blocks.push((
                dimension,
                (
                    game.origin_x + x as i32,
                    game.origin_y,
                    game.origin_z + z as i32,
                ),
                block_for_mark(config, mark),
            ));
        }
    }
    world_set_blocks(blocks)
}

fn block_for_mark(config: &Config, mark: u8) -> &str {
    match mark {
        1 => config.board.black_block.as_str(),
        2 => config.board.white_block.as_str(),
        _ => config.board.board_block.as_str(),
    }
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
    GameState {
        id,
        status: "playing".to_string(),
        mode: mode.to_string(),
        room_code: room_code.to_string(),
        black_player: black_player.to_string(),
        black_name: black_name.to_string(),
        white_player: white_player.to_string(),
        white_name: white_name.to_string(),
        bot: white_player == "bot",
        board: vec![0; BOARD_SIZE * BOARD_SIZE],
        turn: 1,
        winner: 0,
        dimension: config.board.dimension.clone(),
        origin_x: origin.0,
        origin_y: origin.1,
        origin_z: origin.2,
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
        "won" => format!("{} 获胜", if game.winner == 1 { "黑" } else { "白" }),
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
    config: &Config,
) -> Vec<PlaceholderReplacement> {
    vec![
        placeholder(
            "gobang_state",
            game.map(status_label).unwrap_or_else(|| "大厅".to_string()),
        ),
        placeholder(
            "gobang_mode",
            game.map(mode_label).unwrap_or_else(|| "-".to_string()),
        ),
        placeholder(
            "gobang_turn",
            game.map(turn_label).unwrap_or_else(|| "-".to_string()),
        ),
        placeholder(
            "gobang_opponent",
            game.map(|game| opponent_label(game, player_uuid))
                .unwrap_or_else(|| "-".to_string()),
        ),
        placeholder(
            "gobang_room",
            game.and_then(|game| (!game.room_code.is_empty()).then(|| game.room_code.clone()))
                .unwrap_or_else(|| "-".to_string()),
        ),
        placeholder(
            "gobang_time",
            game.map(game_elapsed_label)
                .unwrap_or_else(|| "--:--".to_string()),
        ),
        placeholder(
            "gobang_ai",
            if config.ai.enable_openai {
                config.ai.model.clone()
            } else {
                "本地启发式".to_string()
            },
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

fn is_empty(game: &GameState, x: usize, z: usize) -> bool {
    x < BOARD_SIZE && z < BOARD_SIZE && game.board[game.index(x, z)] == 0
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
    format!("gobang:{player_uuid}")
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
            black_player: player_uuid.to_string(),
            black_name: player_name.to_string(),
            white_player: String::new(),
            white_name: String::new(),
            bot: false,
            board: vec![0; BOARD_SIZE * BOARD_SIZE],
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

    fn index(&self, x: usize, z: usize) -> usize {
        z * BOARD_SIZE + x
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
    ai: AiConfig,
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
            ai: AiConfig::default(),
            bot: BotConfig::default(),
            lobby: LobbyConfig::default(),
            board: BoardConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
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

#[derive(Debug, Clone, Deserialize)]
struct AiConfig {
    #[serde(default)]
    enable_openai: bool,
    #[serde(default = "default_ai_api_url")]
    api_url: String,
    #[serde(default = "default_ai_model")]
    model: String,
    #[serde(default)]
    api_key: String,
    #[serde(default = "default_ai_timeout_ms")]
    timeout_ms: u64,
    #[serde(default = "default_ai_temperature")]
    temperature: f32,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            enable_openai: false,
            api_url: default_ai_api_url(),
            model: default_ai_model(),
            api_key: String::new(),
            timeout_ms: default_ai_timeout_ms(),
            temperature: default_ai_temperature(),
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
    #[serde(default = "default_board_block")]
    board_block: String,
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
            black_block: default_black_block(),
            white_block: default_white_block(),
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
    "gobang_select".to_string()
}

fn default_queue_ttl_ms() -> i64 {
    300_000
}

fn default_ai_api_url() -> String {
    "http://127.0.0.1:11434/v1/chat/completions".to_string()
}

fn default_ai_model() -> String {
    "qwen2.5:3b-instruct-q4_K_M".to_string()
}

fn default_ai_timeout_ms() -> u64 {
    10_000
}

fn default_ai_temperature() -> f32 {
    0.1
}

fn default_bot_name() -> String {
    "浅屿棋手".to_string()
}

fn default_lobby_dimension() -> String {
    "qexed:gobang_lobby".to_string()
}

fn default_board_dimension() -> String {
    "qexed:gobang_game".to_string()
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
    "{\"text\":\"五子棋入口\",\"color\":\"gold\"}".to_string()
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
menu_id = "gobang_select"

[matchmaking]
queue_ttl_ms = 300000

[ai]
enable_openai = false
api_url = "http://127.0.0.1:11434/v1/chat/completions"
model = "qwen2.5:3b-instruct-q4_K_M"
api_key = ""
timeout_ms = 10000
temperature = 0.1

[bot]
name = "浅屿棋手"

[lobby]
dimension = "qexed:gobang_lobby"
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
npc_display_name = "{\"text\":\"五子棋入口\",\"color\":\"gold\"}"

[board]
dimension = "qexed:gobang_game"
origin_x = 4096
origin_y = -52
origin_z = 4096
instance_spacing = 64
floor_block = "minecraft:smooth_stone"
border_block = "minecraft:dark_oak_planks"
board_block = "minecraft:birch_planks"
black_block = "minecraft:black_concrete"
white_block = "minecraft:white_concrete"
"#;

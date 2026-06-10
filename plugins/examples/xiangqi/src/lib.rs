use qexed_plugin_sdk::{
    ConfigReloadPayload, NpcInteractPayload, NpcMutationOp, NpcMutationResponse, NpcUpsert,
    PlaceholderQuery, PlaceholderReplacement, PlaceholderResponse, PlayerAction,
    PlayerBlockInteractPayload, PlayerPayload, PlayerTickPayload, PluginCommandDefinition,
    PluginCommandQuery, PluginCommandResponse, PluginManifest, RuntimeEntity, RuntimeEntityEvents,
    WorldEditRegion, config_load_or_create, config_read_to_string, entity_move, entity_remove,
    entity_upsert_with_events, storage_delete, storage_get_typed, storage_set_typed, time_millis,
    world_register_edit_region, world_set_blocks,
};
use serde::{Deserialize, Serialize};

qexed_plugin_sdk::qexed_plugin_memory!();
qexed_plugin_sdk::qexed_plugin_manifest!(PluginManifest {
    id: "qexed.xiangqi".to_string(),
    version: "0.1.0".to_string(),
    depends: Vec::new(),
    optional_depends: Vec::new(),
    services: Vec::new(),
});

const CONFIG_PATH: &str = "config.toml";
const COMMAND_NAME: &str = "xiangqi";
const NPC_KEY: &str = "xiangqi:guide";
const NPC_EVENT: &str = "xiangqi";
const PIECE_EVENT: &str = "xiangqi_piece";
const SESSION_PREFIX: &str = "xiangqi/session/";
const GAME_PREFIX: &str = "xiangqi/game/";
const ROOM_PREFIX: &str = "xiangqi/room/";
const PENDING_PREFIX: &str = "xiangqi/pending/";
const ENTITY_SYNC_PREFIX: &str = "xiangqi/entity_sync/";
const INTERACT_COOLDOWN_PREFIX: &str = "xiangqi/interact_cooldown/";
const QUEUE_KEY: &str = "xiangqi/queue";
const INTERACT_COOLDOWN_MS: i64 = 120;
const BOARD_WIDTH: usize = 9;
const BOARD_HEIGHT: usize = 10;
const INSTANCE_GRID_WIDTH: u64 = 2048;
const RED: u8 = 1;
const BLACK: u8 = 2;

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_priority() -> i32 {
    278
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_init() {
    let _ = config_load_or_create(CONFIG_PATH, DEFAULT_CONFIG);
    let config = load_config();
    register_lobby(&config);
    build_lobby(&config);
    qexed_plugin_sdk::log("xiangqi initialized");
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_config_reload(ptr: i32, len: i32) {
    let Some(payload) =
        (unsafe { qexed_plugin_sdk::decode_payload::<ConfigReloadPayload>(ptr, len) })
    else {
        return;
    };
    let path = payload.path.replace('\\', "/");
    if path.ends_with("xiangqi/config.toml") || path.ends_with(CONFIG_PATH) {
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
    exit_current_game(&config, &payload.uuid, true);
    let _ = storage_delete(&pending_key(&payload.uuid));
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_commands(_ptr: i32, _len: i32) -> i64 {
    qexed_plugin_sdk::response_ptr_len(&PluginCommandDefinition {
        name: COMMAND_NAME.to_string(),
        description_key: "qexed.plugin.xiangqi.command.description".to_string(),
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
                name: "Xiangqi".to_string(),
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
    let config = load_config();
    let response = if payload.entity.key == NPC_KEY && payload.configured_event == NPC_EVENT {
        open_menu(&config)
    } else if payload.configured_event == PIECE_EVENT {
        handle_piece_entity_interact(&config, &payload)
    } else {
        PluginCommandResponse::default()
    };
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
    let config = load_config();
    sync_entities_for_player_if_needed(&config, &payload);
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
    qexed_plugin_sdk::response_ptr_len(&PlaceholderResponse {
        replacements: replacements(game.as_ref(), session.as_ref(), &player.uuid),
    })
}

fn handle_command(config: &Config, payload: &PluginCommandQuery) -> PluginCommandResponse {
    if !config.enable {
        return handled(vec![message("中国象棋插件已禁用")]);
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
        "move" | "go" => command_move(config, &payload.player.uuid, parts.collect()),
        "pick" | "choose" => command_select(config, &payload.player.uuid, parts.collect()),
        "status" | "info" => status_command(config, &payload.player.uuid),
        "resign" | "surrender" => resign_game(config, &payload.player.uuid),
        "leave" | "lobby" | "spawn" => leave_to_lobby(config, &payload.player.uuid),
        _ => handled(vec![message(
            "用法: /xiangqi match | bot | create | join <房间码> | pick <x> <z> | move <x1> <z1> <x2> <z2> | resign | leave",
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
        "中国象棋: 点击己方生物棋子选择，再点击棋盘落点。也可用 /xiangqi move <x1> <z1> <x2> <z2>。",
    )])
}

fn join_match_queue(
    config: &Config,
    player_uuid: &str,
    player_name: &str,
) -> PluginCommandResponse {
    clear_inactive_session(config, player_uuid);
    if active_session(player_uuid).is_some() {
        return handled(vec![message(
            "你已经在一局中国象棋中，先 /xiangqi leave 再匹配",
        )]);
    }
    let now = time_millis();
    if let Some(queue) = storage_get_typed::<MatchQueue>(QUEUE_KEY) {
        if queue.player_uuid == player_uuid {
            return handled(vec![message("你已经在中国象棋匹配队列中")]);
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
                start_actions(config, &game, &queue.player_uuid, "匹配成功，你执红先行"),
            );
            return handled(start_actions(
                config,
                &game,
                player_uuid,
                "匹配成功，你执黑后行",
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
    handled(vec![message("已进入中国象棋匹配队列，等待下一名玩家加入")])
}

fn start_bot_game(config: &Config, player_uuid: &str, player_name: &str) -> PluginCommandResponse {
    clear_inactive_session(config, player_uuid);
    if active_session(player_uuid).is_some() {
        return handled(vec![message(
            "你已经在一局中国象棋中，先 /xiangqi leave 再开始人机",
        )]);
    }
    let game = new_game(
        config,
        "bot",
        player_uuid,
        player_name,
        "bot",
        &config.bot.name,
        "",
    );
    save_started_game(config, &game);
    handled(start_actions(
        config,
        &game,
        player_uuid,
        "人机中国象棋开始，你执红先行",
    ))
}

fn create_room(config: &Config, player_uuid: &str, player_name: &str) -> PluginCommandResponse {
    clear_inactive_session(config, player_uuid);
    if active_session(player_uuid).is_some() {
        return handled(vec![message(
            "你已经在一局中国象棋中，先 /xiangqi leave 再创建房间",
        )]);
    }
    let room_code = room_code_for(player_uuid);
    let mut game = new_game(config, "room", player_uuid, player_name, "", "", &room_code);
    game.status = "waiting".to_string();
    let _ = storage_set_typed(&game_key(&game.id), &game);
    let _ = storage_set_typed(
        &room_key(&room_code),
        &RoomState {
            game_id: game.id.clone(),
        },
    );
    let _ = storage_set_typed(
        &session_key(player_uuid),
        &PlayerSession {
            game_id: game.id,
            selected_piece_id: String::new(),
        },
    );
    handled(vec![message(format!(
        "房间已创建，房间码 {}。另一名玩家输入 /xiangqi join {} 加入",
        room_code, room_code
    ))])
}

fn join_room(
    config: &Config,
    player_uuid: &str,
    player_name: &str,
    code: &str,
) -> PluginCommandResponse {
    clear_inactive_session(config, player_uuid);
    let code = normalize_room_code(code);
    if code.is_empty() {
        return handled(vec![message("用法: /xiangqi join <房间码>")]);
    }
    if active_session(player_uuid).is_some() {
        return handled(vec![message(
            "你已经在一局中国象棋中，先 /xiangqi leave 再加入房间",
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
    if game.red_player == player_uuid {
        return handled(vec![message("不能加入自己的房间")]);
    }
    game.black_player = player_uuid.to_string();
    game.black_name = player_name.to_string();
    game.status = "playing".to_string();
    game.started_at_ms = time_millis();
    let _ = storage_delete(&room_key(&code));
    save_started_game(config, &game);
    push_pending(
        &game.red_player,
        start_actions(config, &game, &game.red_player, "对手已加入，你执红先行"),
    );
    handled(start_actions(
        config,
        &game,
        player_uuid,
        "加入成功，你执黑后行",
    ))
}

fn command_select(config: &Config, player_uuid: &str, args: Vec<&str>) -> PluginCommandResponse {
    if args.len() != 2 {
        return handled(vec![message(
            "用法: /xiangqi pick <x> <z>，坐标 x=1..9 z=1..10",
        )]);
    }
    let Some(x) = parse_x(args[0]) else {
        return handled(vec![message("x 坐标必须是 1..9")]);
    };
    let Some(z) = parse_z(args[1]) else {
        return handled(vec![message("z 坐标必须是 1..10")]);
    };
    select_piece_at(config, player_uuid, x, z)
}

fn command_move(config: &Config, player_uuid: &str, args: Vec<&str>) -> PluginCommandResponse {
    match args.as_slice() {
        [to_x, to_z] => {
            let Some(to_x) = parse_x(to_x) else {
                return handled(vec![message("目标 x 坐标必须是 1..9")]);
            };
            let Some(to_z) = parse_z(to_z) else {
                return handled(vec![message("目标 z 坐标必须是 1..10")]);
            };
            make_selected_move(config, player_uuid, to_x, to_z)
        }
        [from_x, from_z, to_x, to_z] => {
            let Some(from_x) = parse_x(from_x) else {
                return handled(vec![message("起点 x 坐标必须是 1..9")]);
            };
            let Some(from_z) = parse_z(from_z) else {
                return handled(vec![message("起点 z 坐标必须是 1..10")]);
            };
            let Some(to_x) = parse_x(to_x) else {
                return handled(vec![message("目标 x 坐标必须是 1..9")]);
            };
            let Some(to_z) = parse_z(to_z) else {
                return handled(vec![message("目标 z 坐标必须是 1..10")]);
            };
            let selected = match selected_piece_id_at(player_uuid, from_x, from_z) {
                Ok(piece_id) => piece_id,
                Err(response) => return response,
            };
            save_selected_piece(player_uuid, &selected);
            make_selected_move(config, player_uuid, to_x, to_z)
        }
        _ => handled(vec![message(
            "用法: /xiangqi move <目标x> <目标z> 或 /xiangqi move <起点x> <起点z> <目标x> <目标z>",
        )]),
    }
}

fn status_command(config: &Config, player_uuid: &str) -> PluginCommandResponse {
    let Some(session) = stored_session(player_uuid) else {
        return handled(vec![message("当前没有中国象棋对局")]);
    };
    let Some(game) = stored_game(&session.game_id) else {
        return handled(vec![message("当前对局数据不存在")]);
    };
    let selected = selected_label(&game, &session);
    handled(vec![
        message(format!(
            "中国象棋状态: {}，模式: {}，回合: {}，对手: {}，已选: {}",
            status_label(&game),
            mode_label(&game),
            turn_label(&game),
            opponent_label(&game, player_uuid),
            selected
        )),
        progress_bar(player_uuid, &game),
        teleport_to_game(&game, player_uuid),
        PlayerAction::OpenMenu {
            menu: config.menu_id.clone(),
        },
    ])
}

fn resign_game(config: &Config, player_uuid: &str) -> PluginCommandResponse {
    let Some(session) = active_session(player_uuid) else {
        return handled(vec![message("当前没有进行中的中国象棋对局")]);
    };
    let Some(mut game) = stored_game(&session.game_id) else {
        return handled(vec![message("当前对局数据不存在")]);
    };
    let Some(color) = color_for_player(&game, player_uuid) else {
        return handled(vec![message("你不是这局中国象棋的玩家")]);
    };
    game.status = "forfeit".to_string();
    game.winner = opponent_color(color);
    finish_game(&mut game);
    save_game(&game);
    cleanup_game_entities(&game);
    if let Some(opponent) = opponent_uuid(&game, player_uuid) {
        push_pending(opponent, finish_actions(config, &game, opponent));
    }
    handled(finish_actions(config, &game, player_uuid))
}

fn leave_to_lobby(config: &Config, player_uuid: &str) -> PluginCommandResponse {
    exit_current_game(config, player_uuid, true);
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

fn handle_piece_entity_interact(
    config: &Config,
    payload: &NpcInteractPayload,
) -> PluginCommandResponse {
    let Some(piece_id) = piece_id_from_entity_key(&payload.entity.key) else {
        return PluginCommandResponse::default();
    };
    if !consume_interaction(&payload.player.uuid) {
        return PluginCommandResponse::default();
    }
    let Some(session) = active_session(&payload.player.uuid) else {
        return handled(vec![message("先通过菜单开始一局中国象棋")]);
    };
    let Some(game) = stored_game(&session.game_id) else {
        return handled(vec![message("当前对局数据不存在")]);
    };
    let Some(piece) = game
        .pieces
        .iter()
        .find(|piece| piece.id == piece_id && piece.alive)
    else {
        return handled(vec![message("这个棋子已经不在棋盘上")]);
    };
    if piece.color == color_for_player(&game, &payload.player.uuid).unwrap_or(0) {
        select_piece_by_id(config, &payload.player.uuid, &game, &piece.id)
    } else if !session.selected_piece_id.is_empty() {
        make_selected_move(config, &payload.player.uuid, piece.x, piece.z)
    } else {
        handled(vec![message(
            "这是对方棋子。先点击自己的生物棋子，再点击目标格或目标棋子",
        )])
    }
}

fn handle_board_position(
    config: &Config,
    player_uuid: &str,
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
    if !consume_interaction(player_uuid) {
        return PluginCommandResponse::default();
    }
    if let Some(piece) = piece_at(&game, cell_x, cell_z)
        && piece.color == color_for_player(&game, player_uuid).unwrap_or(0)
        && (session.selected_piece_id.is_empty() || session.selected_piece_id != piece.id)
    {
        return select_piece_by_id(config, player_uuid, &game, &piece.id);
    }
    make_selected_move(config, player_uuid, cell_x, cell_z)
}

fn select_piece_at(
    config: &Config,
    player_uuid: &str,
    x: usize,
    z: usize,
) -> PluginCommandResponse {
    let Some(session) = active_session(player_uuid) else {
        return handled(vec![message("当前没有进行中的中国象棋对局")]);
    };
    let Some(game) = stored_game(&session.game_id) else {
        return handled(vec![message("当前对局数据不存在")]);
    };
    let Some(piece) = piece_at(&game, x, z) else {
        return handled(vec![message("这个位置没有棋子")]);
    };
    select_piece_by_id(config, player_uuid, &game, &piece.id)
}

fn select_piece_by_id(
    _config: &Config,
    player_uuid: &str,
    game: &GameState,
    piece_id: &str,
) -> PluginCommandResponse {
    let Some(color) = color_for_player(game, player_uuid) else {
        return handled(vec![message("你不是这局中国象棋的玩家")]);
    };
    if game.status != "playing" {
        return handled(vec![message(format!("当前对局{}", status_label(game)))]);
    }
    if game.turn != color {
        return handled(vec![message(format!(
            "还没轮到你，当前回合: {}",
            turn_label(game)
        ))]);
    }
    let Some(piece) = game
        .pieces
        .iter()
        .find(|piece| piece.id == piece_id && piece.alive)
    else {
        return handled(vec![message("这个棋子已经不在棋盘上")]);
    };
    if piece.color != color {
        return handled(vec![message("只能选择自己的棋子")]);
    }
    save_selected_piece(player_uuid, piece_id);
    handled(vec![message(format!(
        "已选择 {} ({}, {})，现在点击目标格",
        piece_label(piece),
        piece.x + 1,
        piece.z + 1
    ))])
}

fn make_selected_move(
    config: &Config,
    player_uuid: &str,
    to_x: usize,
    to_z: usize,
) -> PluginCommandResponse {
    let Some(mut session) = active_session(player_uuid) else {
        return handled(vec![message("当前没有进行中的中国象棋对局")]);
    };
    let Some(mut game) = stored_game(&session.game_id) else {
        return handled(vec![message("当前对局数据不存在")]);
    };
    if game.status != "playing" {
        return handled(vec![message(format!("当前对局{}", status_label(&game)))]);
    }
    let Some(color) = color_for_player(&game, player_uuid) else {
        return handled(vec![message("你不是这局中国象棋的玩家")]);
    };
    if game.turn != color {
        return handled(vec![message(format!(
            "还没轮到你，当前回合: {}",
            turn_label(&game)
        ))]);
    }
    let selected_piece_id = session.selected_piece_id.clone();
    if selected_piece_id.is_empty() {
        return handled(vec![message("先点击自己的生物棋子，再点击落点")]);
    }
    if let Some(target) = piece_at(&game, to_x, to_z)
        && target.color == color
        && target.id != selected_piece_id
    {
        return select_piece_by_id(config, player_uuid, &game, &target.id);
    }
    if let Err(reason) = validate_move(&game, &selected_piece_id, to_x, to_z) {
        return handled(vec![message(reason)]);
    }

    let outcome = apply_move_unchecked(&mut game, &selected_piece_id, to_x, to_z);
    session.selected_piece_id.clear();
    let _ = storage_set_typed(&session_key(player_uuid), &session);
    save_game(&game);
    render_board(config, &game);
    sync_move_entities(config, &game, &outcome);

    let mut actions = vec![move_message(&outcome, "你")];
    if game.status == "playing" && game.bot && game.turn == BLACK {
        if let Some(bot_move) = bot_choose_move(&game, BLACK, &config.bot) {
            let bot_outcome =
                apply_move_unchecked(&mut game, &bot_move.piece_id, bot_move.x, bot_move.z);
            actions.push(move_message(&bot_outcome, &game.black_name));
            save_game(&game);
            render_board(config, &game);
            sync_move_entities(config, &game, &bot_outcome);
        }
    }

    if game.status == "playing" {
        save_game(&game);
        actions.push(progress_bar(player_uuid, &game));
        if let Some(opponent) = opponent_uuid(&game, player_uuid) {
            push_pending(
                opponent,
                vec![
                    message("对方已走棋，轮到你"),
                    progress_bar(opponent, &game),
                    teleport_to_game(&game, opponent),
                ],
            );
        }
    } else {
        finish_game(&mut game);
        save_game(&game);
        actions.extend(finish_actions(config, &game, player_uuid));
        if let Some(opponent) = opponent_uuid(&game, player_uuid) {
            push_pending(opponent, finish_actions(config, &game, opponent));
        }
    }
    handled(actions)
}

fn consume_interaction(player_uuid: &str) -> bool {
    let now = time_millis();
    let key = format!("{INTERACT_COOLDOWN_PREFIX}{player_uuid}");
    if storage_get_typed::<i64>(&key).is_some_and(|last| now - last < INTERACT_COOLDOWN_MS) {
        return false;
    }
    let _ = storage_set_typed(&key, &now);
    true
}

fn selected_piece_id_at(
    player_uuid: &str,
    x: usize,
    z: usize,
) -> Result<String, PluginCommandResponse> {
    let Some(session) = active_session(player_uuid) else {
        return Err(handled(vec![message("当前没有进行中的中国象棋对局")]));
    };
    let Some(game) = stored_game(&session.game_id) else {
        return Err(handled(vec![message("当前对局数据不存在")]));
    };
    let Some(color) = color_for_player(&game, player_uuid) else {
        return Err(handled(vec![message("你不是这局中国象棋的玩家")]));
    };
    let Some(piece) = piece_at(&game, x, z) else {
        return Err(handled(vec![message("起点没有棋子")]));
    };
    if piece.color != color {
        return Err(handled(vec![message("起点必须是自己的棋子")]));
    }
    Ok(piece.id.clone())
}

fn save_started_game(config: &Config, game: &GameState) {
    register_arena(game);
    render_board(config, game);
    save_game(game);
    let _ = storage_set_typed(
        &session_key(&game.red_player),
        &PlayerSession {
            game_id: game.id.clone(),
            selected_piece_id: String::new(),
        },
    );
    if !game.black_player.is_empty() && game.black_player != "bot" {
        let _ = storage_set_typed(
            &session_key(&game.black_player),
            &PlayerSession {
                game_id: game.id.clone(),
                selected_piece_id: String::new(),
            },
        );
    }
}

fn sync_entities_for_player_if_needed(config: &Config, payload: &PlayerTickPayload) {
    let Some(session) = active_session(&payload.player.uuid) else {
        return;
    };
    let Some(game) = stored_game(&session.game_id) else {
        return;
    };
    if game.status != "playing" || payload.dimension != game.dimension {
        return;
    }
    let key = entity_sync_key(&payload.player.uuid, &game.id);
    if storage_get_typed::<bool>(&key).unwrap_or(false) {
        return;
    }
    spawn_game_entities(config, &game);
    let _ = storage_set_typed(&key, &true);
    qexed_plugin_sdk::log(&format!(
        "xiangqi synced piece entities for player={}, game={}",
        payload.player.username, game.id
    ));
}

fn save_game(game: &GameState) {
    let _ = storage_set_typed(&game_key(&game.id), game);
}

fn finish_game(game: &mut GameState) {
    if game.ended_elapsed_ms == 0 {
        game.ended_elapsed_ms = elapsed_millis(game.started_at_ms);
    }
}

fn exit_current_game(config: &Config, player_uuid: &str, notify_opponent: bool) {
    remove_from_queue(player_uuid);
    if let Some(session) = stored_session(player_uuid)
        && let Some(mut game) = stored_game(&session.game_id)
    {
        cleanup_game_entities(&game);
        if game.status == "waiting" {
            if !game.room_code.is_empty() {
                let _ = storage_delete(&room_key(&game.room_code));
            }
            let _ = storage_delete(&game_key(&game.id));
        } else if game.status == "playing" {
            if let Some(color) = color_for_player(&game, player_uuid) {
                game.status = "forfeit".to_string();
                game.winner = opponent_color(color);
                finish_game(&mut game);
                save_game(&game);
                if notify_opponent && let Some(opponent) = opponent_uuid(&game, player_uuid) {
                    push_pending(opponent, finish_actions(config, &game, opponent));
                }
            }
        }
    }
    let _ = storage_delete(&session_key(player_uuid));
}

fn clear_inactive_session(config: &Config, player_uuid: &str) {
    if stored_session(player_uuid).is_some() && active_session(player_uuid).is_none() {
        exit_current_game(config, player_uuid, false);
    }
}

fn register_lobby(config: &Config) {
    let center_x = config.lobby.spawn_x.floor() as i32;
    let center_z = config.lobby.spawn_z.floor() as i32;
    let y = config.lobby.floor_y;
    let _ = world_register_edit_region(&WorldEditRegion {
        id: "xiangqi_lobby",
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
    let id = format!("xiangqi_{}", game.id);
    let _ = world_register_edit_region(&WorldEditRegion {
        id: &id,
        dimension: &game.dimension,
        min: (game.origin_x - 3, game.origin_y - 2, game.origin_z - 3),
        max: (
            game.origin_x + BOARD_WIDTH as i32 + 2,
            game.origin_y + 7,
            game.origin_z + BOARD_HEIGHT as i32 + 2,
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
    for dx in -2..=BOARD_WIDTH as i32 + 1 {
        for dz in -2..=BOARD_HEIGHT as i32 + 1 {
            let x = game.origin_x + dx;
            let z = game.origin_z + dz;
            blocks.push((
                dimension,
                (x, game.origin_y - 1, z),
                config.board.floor_block.as_str(),
            ));
            for air_y in game.origin_y..=game.origin_y + 5 {
                blocks.push((dimension, (x, air_y, z), "minecraft:air"));
            }
        }
    }
    for dx in -1..=BOARD_WIDTH as i32 {
        for dz in -1..=BOARD_HEIGHT as i32 {
            if dx == -1 || dz == -1 || dx == BOARD_WIDTH as i32 || dz == BOARD_HEIGHT as i32 {
                blocks.push((
                    dimension,
                    (game.origin_x + dx, game.origin_y, game.origin_z + dz),
                    config.board.border_block.as_str(),
                ));
            }
        }
    }
    for z in 0..BOARD_HEIGHT {
        for x in 0..BOARD_WIDTH {
            blocks.push((
                dimension,
                (
                    game.origin_x + x as i32,
                    game.origin_y,
                    game.origin_z + z as i32,
                ),
                block_for_cell(config, game, x, z),
            ));
        }
    }
    world_set_blocks(blocks)
}

fn block_for_cell<'a>(config: &'a Config, game: &GameState, x: usize, z: usize) -> &'a str {
    if let Some(piece) = piece_at(game, x, z) {
        if piece.color == RED {
            return config.board.red_block.as_str();
        }
        return config.board.black_block.as_str();
    }
    if z == 4 || z == 5 {
        config.board.river_block.as_str()
    } else if in_palace(RED, x, z) || in_palace(BLACK, x, z) {
        config.board.palace_block.as_str()
    } else {
        config.board.board_block.as_str()
    }
}

fn cell_from_position(game: &GameState, x: i32, y: i32, z: i32) -> Option<(usize, usize)> {
    if y != game.origin_y {
        return None;
    }
    let local_x = x.checked_sub(game.origin_x)?;
    let local_z = z.checked_sub(game.origin_z)?;
    if local_x < 0 || local_z < 0 || local_x >= BOARD_WIDTH as i32 || local_z >= BOARD_HEIGHT as i32
    {
        return None;
    }
    Some((local_x as usize, local_z as usize))
}

fn spawn_game_entities(config: &Config, game: &GameState) {
    for piece in game.pieces.iter().filter(|piece| piece.alive) {
        spawn_piece_entity(config, game, piece);
    }
}

fn spawn_piece_entity(config: &Config, game: &GameState, piece: &Piece) {
    let key = piece_entity_key(game, piece);
    let display_name = piece_display_name(piece);
    let _ = entity_upsert_with_events(
        &RuntimeEntity {
            key: &key,
            dimension: &game.dimension,
            entity_type: entity_type_for_piece(config, piece),
            x: piece_entity_x(game, piece.x),
            y: game.origin_y as f64 + config.pieces.y_offset,
            z: piece_entity_z(game, piece.z),
            yaw: if piece.color == RED { 180.0 } else { 0.0 },
            pitch: 0.0,
            display_name: &display_name,
            ai: "none",
            ai_params_json: "{}",
            auto_jump: false,
        },
        &RuntimeEntityEvents {
            main_hand_event: PIECE_EVENT,
            off_hand_event: PIECE_EVENT,
            attack_event: PIECE_EVENT,
        },
    );
}

fn sync_move_entities(config: &Config, game: &GameState, outcome: &MoveOutcome) {
    if let Some(captured) = &outcome.captured {
        let _ = entity_remove(&piece_entity_key(game, captured));
    }
    if let Some(piece) = game
        .pieces
        .iter()
        .find(|piece| piece.id == outcome.piece_id && piece.alive)
    {
        let key = piece_entity_key(game, piece);
        if !entity_move(
            &key,
            &game.dimension,
            piece_entity_x(game, piece.x),
            game.origin_y as f64 + config.pieces.y_offset,
            piece_entity_z(game, piece.z),
            if piece.color == RED { 180.0 } else { 0.0 },
            0.0,
        ) {
            spawn_piece_entity(config, game, piece);
        }
    }
}

fn cleanup_game_entities(game: &GameState) {
    for piece in &game.pieces {
        let _ = entity_remove(&piece_entity_key(game, piece));
    }
}

fn piece_entity_x(game: &GameState, x: usize) -> f64 {
    game.origin_x as f64 + x as f64 + 0.5
}

fn piece_entity_z(game: &GameState, z: usize) -> f64 {
    game.origin_z as f64 + z as f64 + 0.5
}

fn entity_type_for_piece<'a>(config: &'a Config, piece: &Piece) -> &'a str {
    match piece.kind {
        PieceKind::General => config.pieces.general_entity_type.as_str(),
        PieceKind::Advisor => config.pieces.advisor_entity_type.as_str(),
        PieceKind::Elephant => config.pieces.elephant_entity_type.as_str(),
        PieceKind::Horse => config.pieces.horse_entity_type.as_str(),
        PieceKind::Rook => config.pieces.rook_entity_type.as_str(),
        PieceKind::Cannon => config.pieces.cannon_entity_type.as_str(),
        PieceKind::Pawn => config.pieces.pawn_entity_type.as_str(),
    }
}

fn validate_move(game: &GameState, piece_id: &str, to_x: usize, to_z: usize) -> Result<(), String> {
    if to_x >= BOARD_WIDTH || to_z >= BOARD_HEIGHT {
        return Err("目标坐标超出棋盘".to_string());
    }
    let Some(piece) = game
        .pieces
        .iter()
        .find(|piece| piece.id == piece_id && piece.alive)
    else {
        return Err("所选棋子已经不在棋盘上".to_string());
    };
    if piece.x == to_x && piece.z == to_z {
        return Err("不能原地走棋".to_string());
    }
    if let Some(target) = piece_at(game, to_x, to_z)
        && target.color == piece.color
    {
        return Err("不能吃自己的棋子".to_string());
    }
    if !legal_piece_shape(game, piece, to_x, to_z) {
        return Err(format!("{} 不能这样走", piece_label(piece)));
    }
    let mut trial = game.clone();
    apply_move_unchecked(&mut trial, piece_id, to_x, to_z);
    if kings_face(&trial) {
        return Err("将帅不能直接照面".to_string());
    }
    if is_king_in_check(&trial, piece.color) {
        return Err("不能走完后让自己的将帅被将军".to_string());
    }
    Ok(())
}

fn legal_piece_shape(game: &GameState, piece: &Piece, to_x: usize, to_z: usize) -> bool {
    let dx = to_x as i32 - piece.x as i32;
    let dz = to_z as i32 - piece.z as i32;
    let adx = dx.abs();
    let adz = dz.abs();
    match piece.kind {
        PieceKind::General => {
            if let Some(target) = piece_at(game, to_x, to_z)
                && target.kind == PieceKind::General
                && target.color != piece.color
                && piece.x == to_x
                && pieces_between(game, piece.x, piece.z, to_x, to_z) == 0
            {
                return true;
            }
            adx + adz == 1 && in_palace(piece.color, to_x, to_z)
        }
        PieceKind::Advisor => adx == 1 && adz == 1 && in_palace(piece.color, to_x, to_z),
        PieceKind::Elephant => {
            adx == 2
                && adz == 2
                && elephant_stays_home(piece.color, to_z)
                && piece_at(
                    game,
                    ((piece.x as i32 + to_x as i32) / 2) as usize,
                    ((piece.z as i32 + to_z as i32) / 2) as usize,
                )
                .is_none()
        }
        PieceKind::Horse => {
            if !((adx == 1 && adz == 2) || (adx == 2 && adz == 1)) {
                return false;
            }
            let leg = if adx == 2 {
                ((piece.x as i32 + dx / 2) as usize, piece.z)
            } else {
                (piece.x, (piece.z as i32 + dz / 2) as usize)
            };
            piece_at(game, leg.0, leg.1).is_none()
        }
        PieceKind::Rook => {
            (dx == 0 || dz == 0) && pieces_between(game, piece.x, piece.z, to_x, to_z) == 0
        }
        PieceKind::Cannon => {
            if dx != 0 && dz != 0 {
                return false;
            }
            let screens = pieces_between(game, piece.x, piece.z, to_x, to_z);
            if piece_at(game, to_x, to_z).is_some() {
                screens == 1
            } else {
                screens == 0
            }
        }
        PieceKind::Pawn => {
            let forward = if piece.color == RED { -1 } else { 1 };
            if dx == 0 && dz == forward {
                return true;
            }
            pawn_crossed_river(piece.color, piece.z) && adx == 1 && dz == 0
        }
    }
}

fn apply_move_unchecked(
    game: &mut GameState,
    piece_id: &str,
    to_x: usize,
    to_z: usize,
) -> MoveOutcome {
    let piece_index = game
        .pieces
        .iter()
        .position(|piece| piece.id == piece_id && piece.alive)
        .expect("validated piece must exist");
    let from_x = game.pieces[piece_index].x;
    let from_z = game.pieces[piece_index].z;
    let target_index = piece_index_at(game, to_x, to_z);
    let captured = target_index.map(|index| game.pieces[index].clone());
    if let Some(index) = target_index {
        game.pieces[index].alive = false;
    }
    game.pieces[piece_index].x = to_x;
    game.pieces[piece_index].z = to_z;
    if captured
        .as_ref()
        .is_some_and(|piece| piece.kind == PieceKind::General)
    {
        game.status = "won".to_string();
        game.winner = game.pieces[piece_index].color;
    } else if game.status == "playing" {
        game.turn = opponent_color(game.turn);
    }
    MoveOutcome {
        piece_id: piece_id.to_string(),
        piece_label: piece_label(&game.pieces[piece_index]),
        from_x,
        from_z,
        to_x,
        to_z,
        captured,
    }
}

const BOT_WIN_SCORE: i32 = 1_000_000;
const BOT_SEARCH_INF: i32 = 2_000_000;
const BOT_MIN_DEPTH: usize = 1;
const BOT_MAX_DEPTH: usize = 3;

fn bot_choose_move(game: &GameState, color: u8, config: &BotConfig) -> Option<BotMove> {
    let depth = config.search_depth.clamp(BOT_MIN_DEPTH, BOT_MAX_DEPTH);
    let max_candidates = config.max_candidates.clamp(4, 32);
    let mut node_budget = config.node_budget.clamp(100, 5_000);
    let moves = ordered_legal_moves(game, color, max_candidates);
    let mut best: Option<(BotMove, i32)> = None;
    let mut alpha = -BOT_SEARCH_INF;
    for candidate in moves {
        if node_budget == 0 {
            break;
        }
        let mut trial = game.clone();
        apply_move_unchecked(&mut trial, &candidate.piece_id, candidate.x, candidate.z);
        let score = -bot_negamax(
            &trial,
            opponent_color(color),
            depth.saturating_sub(1),
            -BOT_SEARCH_INF,
            -alpha,
            max_candidates,
            &mut node_budget,
        );
        if best.as_ref().is_none_or(|(_, current)| score > *current) {
            best = Some((candidate, score));
        }
        alpha = alpha.max(score);
    }
    best.map(|(value, _)| value)
}

fn bot_negamax(
    game: &GameState,
    color: u8,
    depth: usize,
    mut alpha: i32,
    beta: i32,
    max_candidates: usize,
    node_budget: &mut usize,
) -> i32 {
    if *node_budget == 0 {
        return evaluate_position(game, color);
    }
    *node_budget = (*node_budget).saturating_sub(1);

    if game.status == "won" {
        return if game.winner == color {
            BOT_WIN_SCORE + depth as i32
        } else {
            -BOT_WIN_SCORE - depth as i32
        };
    }
    if depth == 0 {
        return evaluate_position(game, color);
    }

    let moves = ordered_legal_moves(game, color, max_candidates);
    if moves.is_empty() {
        return if is_king_in_check(game, color) {
            -BOT_WIN_SCORE / 2 - depth as i32
        } else {
            -200
        };
    }

    let mut best = -BOT_SEARCH_INF;
    for candidate in moves {
        let mut trial = game.clone();
        apply_move_unchecked(&mut trial, &candidate.piece_id, candidate.x, candidate.z);
        let score = -bot_negamax(
            &trial,
            opponent_color(color),
            depth.saturating_sub(1),
            -beta,
            -alpha,
            max_candidates,
            node_budget,
        );
        best = best.max(score);
        alpha = alpha.max(score);
        if alpha >= beta {
            break;
        }
    }
    best
}

fn ordered_legal_moves(game: &GameState, color: u8, max_candidates: usize) -> Vec<BotMove> {
    let mut moves = legal_moves(game, color);
    moves.sort_by_cached_key(|candidate| {
        std::cmp::Reverse(move_order_score(game, candidate, color))
    });
    moves.truncate(max_candidates);
    moves
}

fn legal_moves(game: &GameState, color: u8) -> Vec<BotMove> {
    let mut moves = Vec::new();
    for piece in game
        .pieces
        .iter()
        .filter(|piece| piece.alive && piece.color == color)
    {
        for z in 0..BOARD_HEIGHT {
            for x in 0..BOARD_WIDTH {
                if validate_move(game, &piece.id, x, z).is_ok() {
                    moves.push(BotMove {
                        piece_id: piece.id.clone(),
                        x,
                        z,
                    });
                }
            }
        }
    }
    moves
}

fn move_order_score(game: &GameState, candidate: &BotMove, color: u8) -> i32 {
    let Some(piece) = game
        .pieces
        .iter()
        .find(|piece| piece.id == candidate.piece_id && piece.alive)
    else {
        return 0;
    };
    let mut score = piece_positional_score(piece, candidate.x, candidate.z);
    if let Some(target) = piece_at(game, candidate.x, candidate.z) {
        score += piece_value(target.kind) * 100 - piece_value(piece.kind);
    }

    let mut trial = game.clone();
    apply_move_unchecked(&mut trial, &candidate.piece_id, candidate.x, candidate.z);
    if trial.status == "won" && trial.winner == color {
        score += BOT_WIN_SCORE;
    } else if is_king_in_check(&trial, opponent_color(color)) {
        score += 2_000;
    }
    score
}

fn evaluate_position(game: &GameState, color: u8) -> i32 {
    if game.status == "won" {
        return if game.winner == color {
            BOT_WIN_SCORE
        } else {
            -BOT_WIN_SCORE
        };
    }

    let opponent = opponent_color(color);
    let mut score = 0;
    for piece in game.pieces.iter().filter(|piece| piece.alive) {
        let sign = if piece.color == color { 1 } else { -1 };
        let value = piece_value(piece.kind) * 10 + piece_positional_score(piece, piece.x, piece.z);
        score += sign * value;

        let attack_penalty = (piece_value(piece.kind) * 3).max(20);
        if is_piece_attacked_by(game, piece, opponent_color(piece.color)) {
            score -= sign * attack_penalty;
        }
        if is_piece_protected(game, piece) {
            score += sign * (piece_value(piece.kind) / 2).max(8);
        }
    }

    if is_king_in_check(game, color) {
        score -= 1_500;
    }
    if is_king_in_check(game, opponent) {
        score += 1_200;
    }

    score
}

fn piece_positional_score(piece: &Piece, x: usize, z: usize) -> i32 {
    let center = 4 - (x as i32 - 4).abs();
    match piece.kind {
        PieceKind::General => 40 - ((x as i32 - 4).abs() + palace_home_z_distance(piece.color, z)) * 12,
        PieceKind::Advisor | PieceKind::Elephant => 20 + center * 4,
        PieceKind::Horse => 35 + center * 8 + advancement_score(piece.color, z) * 2,
        PieceKind::Rook => 40 + center * 6 + advancement_score(piece.color, z),
        PieceKind::Cannon => 30 + center * 7 + advancement_score(piece.color, z),
        PieceKind::Pawn => {
            let crossed = if pawn_crossed_river(piece.color, z) { 45 } else { 0 };
            20 + crossed + center * 6 + advancement_score(piece.color, z) * 12
        }
    }
}

fn advancement_score(color: u8, z: usize) -> i32 {
    match color {
        RED => (6_i32 - z as i32).max(0),
        BLACK => (z as i32 - 3).max(0),
        _ => 0,
    }
}

fn palace_home_z_distance(color: u8, z: usize) -> i32 {
    match color {
        RED => (z as i32 - 9).abs(),
        BLACK => z as i32,
        _ => 0,
    }
}

fn is_piece_attacked_by(game: &GameState, target: &Piece, attacker_color: u8) -> bool {
    game.pieces
        .iter()
        .filter(|piece| piece.alive && piece.color == attacker_color)
        .any(|piece| legal_piece_shape(game, piece, target.x, target.z))
}

fn is_piece_protected(game: &GameState, target: &Piece) -> bool {
    game.pieces
        .iter()
        .filter(|piece| piece.alive && piece.color == target.color && piece.id != target.id)
        .any(|piece| legal_piece_shape(game, piece, target.x, target.z))
}

fn piece_value(kind: PieceKind) -> i32 {
    match kind {
        PieceKind::General => 10_000,
        PieceKind::Rook => 500,
        PieceKind::Cannon => 350,
        PieceKind::Horse => 300,
        PieceKind::Elephant | PieceKind::Advisor => 160,
        PieceKind::Pawn => 80,
    }
}

fn is_king_in_check(game: &GameState, color: u8) -> bool {
    let Some(king) = game
        .pieces
        .iter()
        .find(|piece| piece.alive && piece.color == color && piece.kind == PieceKind::General)
    else {
        return true;
    };
    game.pieces
        .iter()
        .filter(|piece| piece.alive && piece.color != color)
        .any(|piece| legal_piece_shape(game, piece, king.x, king.z))
}

fn kings_face(game: &GameState) -> bool {
    let red_king = game
        .pieces
        .iter()
        .find(|piece| piece.alive && piece.color == RED && piece.kind == PieceKind::General);
    let black_king = game
        .pieces
        .iter()
        .find(|piece| piece.alive && piece.color == BLACK && piece.kind == PieceKind::General);
    let (Some(red_king), Some(black_king)) = (red_king, black_king) else {
        return false;
    };
    red_king.x == black_king.x
        && pieces_between(game, red_king.x, red_king.z, black_king.x, black_king.z) == 0
}

fn pieces_between(
    game: &GameState,
    from_x: usize,
    from_z: usize,
    to_x: usize,
    to_z: usize,
) -> usize {
    if from_x != to_x && from_z != to_z {
        return usize::MAX;
    }
    let step_x = (to_x as i32 - from_x as i32).signum();
    let step_z = (to_z as i32 - from_z as i32).signum();
    let mut x = from_x as i32 + step_x;
    let mut z = from_z as i32 + step_z;
    let end_x = to_x as i32;
    let end_z = to_z as i32;
    let mut count = 0;
    while x != end_x || z != end_z {
        if piece_at(game, x as usize, z as usize).is_some() {
            count += 1;
        }
        x += step_x;
        z += step_z;
    }
    count
}

fn in_palace(color: u8, x: usize, z: usize) -> bool {
    match color {
        RED => (3..=5).contains(&x) && (7..=9).contains(&z),
        BLACK => (3..=5).contains(&x) && (0..=2).contains(&z),
        _ => false,
    }
}

fn elephant_stays_home(color: u8, z: usize) -> bool {
    match color {
        RED => z >= 5,
        BLACK => z <= 4,
        _ => false,
    }
}

fn pawn_crossed_river(color: u8, z: usize) -> bool {
    match color {
        RED => z <= 4,
        BLACK => z >= 5,
        _ => false,
    }
}

fn new_game(
    config: &Config,
    mode: &str,
    red_player: &str,
    red_name: &str,
    black_player: &str,
    black_name: &str,
    room_code: &str,
) -> GameState {
    let id = game_id_for(red_player, black_player, time_millis());
    let origin = instance_origin(config, &id);
    GameState {
        id,
        status: "playing".to_string(),
        mode: mode.to_string(),
        room_code: room_code.to_string(),
        red_player: red_player.to_string(),
        red_name: red_name.to_string(),
        black_player: black_player.to_string(),
        black_name: black_name.to_string(),
        bot: black_player == "bot",
        pieces: initial_pieces(),
        turn: RED,
        winner: 0,
        dimension: config.board.dimension.clone(),
        origin_x: origin.0,
        origin_y: origin.1,
        origin_z: origin.2,
        started_at_ms: time_millis(),
        ended_elapsed_ms: 0,
    }
}

fn initial_pieces() -> Vec<Piece> {
    let mut pieces = Vec::new();
    add_back_rank(&mut pieces, BLACK, 0);
    add_piece(&mut pieces, BLACK, PieceKind::Cannon, "b_cannon_left", 1, 2);
    add_piece(
        &mut pieces,
        BLACK,
        PieceKind::Cannon,
        "b_cannon_right",
        7,
        2,
    );
    for (index, x) in [0, 2, 4, 6, 8].into_iter().enumerate() {
        add_piece(
            &mut pieces,
            BLACK,
            PieceKind::Pawn,
            &format!("b_pawn_{index}"),
            x,
            3,
        );
    }
    add_back_rank(&mut pieces, RED, 9);
    add_piece(&mut pieces, RED, PieceKind::Cannon, "r_cannon_left", 1, 7);
    add_piece(&mut pieces, RED, PieceKind::Cannon, "r_cannon_right", 7, 7);
    for (index, x) in [0, 2, 4, 6, 8].into_iter().enumerate() {
        add_piece(
            &mut pieces,
            RED,
            PieceKind::Pawn,
            &format!("r_pawn_{index}"),
            x,
            6,
        );
    }
    pieces
}

fn add_back_rank(pieces: &mut Vec<Piece>, color: u8, z: usize) {
    let prefix = if color == RED { "r" } else { "b" };
    let kinds = [
        PieceKind::Rook,
        PieceKind::Horse,
        PieceKind::Elephant,
        PieceKind::Advisor,
        PieceKind::General,
        PieceKind::Advisor,
        PieceKind::Elephant,
        PieceKind::Horse,
        PieceKind::Rook,
    ];
    for (x, kind) in kinds.into_iter().enumerate() {
        add_piece(
            pieces,
            color,
            kind,
            &format!("{prefix}_{kind:?}_{x}").to_ascii_lowercase(),
            x,
            z,
        );
    }
}

fn add_piece(pieces: &mut Vec<Piece>, color: u8, kind: PieceKind, id: &str, x: usize, z: usize) {
    pieces.push(Piece {
        id: id.to_string(),
        kind,
        color,
        x,
        z,
        alive: true,
    });
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

fn piece_at(game: &GameState, x: usize, z: usize) -> Option<&Piece> {
    game.pieces
        .iter()
        .find(|piece| piece.alive && piece.x == x && piece.z == z)
}

fn piece_index_at(game: &GameState, x: usize, z: usize) -> Option<usize> {
    game.pieces
        .iter()
        .position(|piece| piece.alive && piece.x == x && piece.z == z)
}

fn color_for_player(game: &GameState, player_uuid: &str) -> Option<u8> {
    if game.red_player == player_uuid {
        Some(RED)
    } else if game.black_player == player_uuid {
        Some(BLACK)
    } else {
        None
    }
}

fn opponent_uuid<'a>(game: &'a GameState, player_uuid: &str) -> Option<&'a str> {
    if game.red_player == player_uuid && game.black_player != "bot" && !game.black_player.is_empty()
    {
        Some(&game.black_player)
    } else if game.black_player == player_uuid {
        Some(&game.red_player)
    } else {
        None
    }
}

fn opponent_color(color: u8) -> u8 {
    if color == RED { BLACK } else { RED }
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
    let mut pending =
        storage_get_typed::<PendingActions>(&pending_key(player_uuid)).unwrap_or_default();
    pending.actions.append(&mut actions);
    let _ = storage_set_typed(&pending_key(player_uuid), &pending);
}

fn save_selected_piece(player_uuid: &str, piece_id: &str) {
    if let Some(mut session) = active_session(player_uuid) {
        session.selected_piece_id = piece_id.to_string();
        let _ = storage_set_typed(&session_key(player_uuid), &session);
    }
}

fn move_message(outcome: &MoveOutcome, actor: &str) -> PlayerAction {
    let capture = outcome
        .captured
        .as_ref()
        .map(|piece| format!("，吃掉 {}", piece_label(piece)))
        .unwrap_or_default();
    message(format!(
        "{}走 {}: ({}, {}) -> ({}, {}){}",
        actor,
        outcome.piece_label,
        outcome.from_x + 1,
        outcome.from_z + 1,
        outcome.to_x + 1,
        outcome.to_z + 1,
        capture
    ))
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
        message("点击自己的生物棋子选择，再点击棋盘落点；吃子时可直接点目标棋子"),
        PlayerAction::SetPlayersVisible { visible: true },
        progress_bar(player_uuid, game),
        teleport_to_game(game, player_uuid),
    ]
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
    if game.status == "forfeit" {
        if color_for_player(game, player_uuid) == Some(game.winner) {
            return format!("对手离开，你获胜，用时 {}", game_elapsed_label(game));
        }
        return format!("你已认输，用时 {}", game_elapsed_label(game));
    }
    if color_for_player(game, player_uuid) == Some(game.winner) {
        format!("将死对方，你获胜，用时 {}", game_elapsed_label(game))
    } else {
        format!("你的将帅被吃，失败，用时 {}", game_elapsed_label(game))
    }
}

fn teleport_to_game(game: &GameState, player_uuid: &str) -> PlayerAction {
    let is_black = game.black_player == player_uuid;
    PlayerAction::Teleport {
        dimension: game.dimension.clone(),
        x: game.origin_x as f64 + BOARD_WIDTH as f64 / 2.0,
        y: game.origin_y as f64 + 1.5,
        z: if is_black {
            game.origin_z as f64 - 1.5
        } else {
            game.origin_z as f64 + BOARD_HEIGHT as f64 + 1.5
        },
        yaw: Some(if is_black { 0.0 } else { 180.0 }),
        pitch: Some(35.0),
    }
}

fn lobby_spawn_actions(config: &Config) -> Vec<PlayerAction> {
    vec![
        PlayerAction::Teleport {
            dimension: config.lobby.dimension.clone(),
            x: config.lobby.spawn_x,
            y: config.lobby.spawn_y,
            z: config.lobby.spawn_z,
            yaw: Some(config.lobby.spawn_yaw),
            pitch: Some(config.lobby.spawn_pitch),
        },
        PlayerAction::OpenMenu {
            menu: config.menu_id.clone(),
        },
    ]
}

fn progress_bar(player_uuid: &str, game: &GameState) -> PlayerAction {
    let alive = game.pieces.iter().filter(|piece| piece.alive).count();
    PlayerAction::BossBar {
        id: boss_bar_id(player_uuid),
        title: format!(
            "中国象棋 {} | 回合 {} | 棋子 {}/32",
            status_label(game),
            turn_label(game),
            alive
        ),
        progress: (alive as f32 / 32.0).clamp(0.0, 1.0),
        color: if game.turn == RED { "red" } else { "blue" }.to_string(),
        overlay: "progress".to_string(),
    }
}

fn status_label(game: &GameState) -> String {
    match game.status.as_str() {
        "waiting" => "等待玩家".to_string(),
        "playing" => "进行中".to_string(),
        "won" => match game.winner {
            RED => "红方胜".to_string(),
            BLACK => "黑方胜".to_string(),
            _ => "已结束".to_string(),
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

fn turn_label(game: &GameState) -> String {
    match game.turn {
        RED => format!("红({})", game.red_name),
        BLACK => format!("黑({})", game.black_name),
        _ => "-".to_string(),
    }
}

fn opponent_label(game: &GameState, player_uuid: &str) -> String {
    if game.red_player == player_uuid {
        game.black_name.clone()
    } else if game.black_player == player_uuid {
        game.red_name.clone()
    } else {
        "-".to_string()
    }
}

fn selected_label(game: &GameState, session: &PlayerSession) -> String {
    if session.selected_piece_id.is_empty() {
        return "-".to_string();
    }
    game.pieces
        .iter()
        .find(|piece| piece.id == session.selected_piece_id && piece.alive)
        .map(piece_label)
        .unwrap_or_else(|| "-".to_string())
}

fn replacements(
    game: Option<&GameState>,
    session: Option<&PlayerSession>,
    player_uuid: &str,
) -> Vec<PlaceholderReplacement> {
    vec![
        placeholder(
            "xiangqi_state",
            game.map(status_label).unwrap_or_else(|| "大厅".to_string()),
        ),
        placeholder(
            "xiangqi_mode",
            game.map(mode_label).unwrap_or_else(|| "-".to_string()),
        ),
        placeholder(
            "xiangqi_turn",
            game.map(turn_label).unwrap_or_else(|| "-".to_string()),
        ),
        placeholder(
            "xiangqi_opponent",
            game.map(|game| opponent_label(game, player_uuid))
                .unwrap_or_else(|| "-".to_string()),
        ),
        placeholder(
            "xiangqi_selected",
            game.zip(session)
                .map(|(game, session)| selected_label(game, session))
                .unwrap_or_else(|| "-".to_string()),
        ),
        placeholder(
            "xiangqi_room",
            game.and_then(|game| (!game.room_code.is_empty()).then(|| game.room_code.clone()))
                .unwrap_or_else(|| "-".to_string()),
        ),
        placeholder(
            "xiangqi_time",
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

fn piece_label(piece: &Piece) -> String {
    let side = if piece.color == RED { "红" } else { "黑" };
    format!("{}{}", side, kind_label(piece.kind, piece.color))
}

fn kind_label(kind: PieceKind, color: u8) -> &'static str {
    match (kind, color) {
        (PieceKind::General, RED) => "帅",
        (PieceKind::General, BLACK) => "将",
        (PieceKind::Advisor, RED) => "仕",
        (PieceKind::Advisor, BLACK) => "士",
        (PieceKind::Elephant, RED) => "相",
        (PieceKind::Elephant, BLACK) => "象",
        (PieceKind::Horse, _) => "马",
        (PieceKind::Rook, _) => "车",
        (PieceKind::Cannon, _) => "炮",
        (PieceKind::Pawn, RED) => "兵",
        (PieceKind::Pawn, BLACK) => "卒",
        _ => "?",
    }
}

fn piece_display_name(piece: &Piece) -> String {
    let color = if piece.color == RED {
        "red"
    } else {
        "dark_blue"
    };
    format!(
        r#"{{"text":"{}","color":"{}","bold":true}}"#,
        piece_label(piece),
        color
    )
}

fn piece_entity_key(game: &GameState, piece: &Piece) -> String {
    format!("game/{}/piece/{}", game.id, piece.id)
}

fn piece_id_from_entity_key(key: &str) -> Option<String> {
    key.split("/piece/").nth(1).map(str::to_string)
}

fn parse_x(value: &str) -> Option<usize> {
    let parsed = value.parse::<usize>().ok()?;
    (1..=BOARD_WIDTH).contains(&parsed).then_some(parsed - 1)
}

fn parse_z(value: &str) -> Option<usize> {
    let parsed = value.parse::<usize>().ok()?;
    (1..=BOARD_HEIGHT).contains(&parsed).then_some(parsed - 1)
}

fn elapsed_millis(started_at_ms: i64) -> i64 {
    (time_millis() - started_at_ms).max(0)
}

fn game_elapsed_label(game: &GameState) -> String {
    if game.status == "playing" || game.status == "waiting" {
        format_duration_ms(elapsed_millis(game.started_at_ms))
    } else {
        format_duration_ms(game.ended_elapsed_ms)
    }
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
    format!("xiangqi:{player_uuid}")
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

fn entity_sync_key(player_uuid: &str, game_id: &str) -> String {
    format!("{ENTITY_SYNC_PREFIX}{player_uuid}/{game_id}")
}

fn load_config() -> Config {
    config_read_to_string(CONFIG_PATH)
        .and_then(|contents| toml::from_str(&contents).ok())
        .unwrap_or_default()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
enum PieceKind {
    General,
    Advisor,
    Elephant,
    Horse,
    Rook,
    Cannon,
    Pawn,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Piece {
    id: String,
    kind: PieceKind,
    color: u8,
    x: usize,
    z: usize,
    alive: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GameState {
    id: String,
    status: String,
    mode: String,
    room_code: String,
    red_player: String,
    red_name: String,
    black_player: String,
    black_name: String,
    bot: bool,
    pieces: Vec<Piece>,
    turn: u8,
    winner: u8,
    dimension: String,
    origin_x: i32,
    origin_y: i32,
    origin_z: i32,
    started_at_ms: i64,
    ended_elapsed_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PlayerSession {
    game_id: String,
    #[serde(default)]
    selected_piece_id: String,
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

#[derive(Debug, Clone)]
struct MoveOutcome {
    piece_id: String,
    piece_label: String,
    from_x: usize,
    from_z: usize,
    to_x: usize,
    to_z: usize,
    captured: Option<Piece>,
}

#[derive(Debug, Clone)]
struct BotMove {
    piece_id: String,
    x: usize,
    z: usize,
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
    #[serde(default)]
    pieces: PieceEntityConfig,
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
            pieces: PieceEntityConfig::default(),
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
struct BotConfig {
    #[serde(default = "default_bot_name")]
    name: String,
    #[serde(default = "default_bot_search_depth")]
    search_depth: usize,
    #[serde(default = "default_bot_max_candidates")]
    max_candidates: usize,
    #[serde(default = "default_bot_node_budget")]
    node_budget: usize,
}

impl Default for BotConfig {
    fn default() -> Self {
        Self {
            name: default_bot_name(),
            search_depth: default_bot_search_depth(),
            max_candidates: default_bot_max_candidates(),
            node_budget: default_bot_node_budget(),
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
    #[serde(default = "default_river_block")]
    river_block: String,
    #[serde(default = "default_palace_block")]
    palace_block: String,
    #[serde(default = "default_red_block")]
    red_block: String,
    #[serde(default = "default_black_block")]
    black_block: String,
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
            river_block: default_river_block(),
            palace_block: default_palace_block(),
            red_block: default_red_block(),
            black_block: default_black_block(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct PieceEntityConfig {
    #[serde(default = "default_piece_y_offset")]
    y_offset: f64,
    #[serde(default = "default_general_entity_type")]
    general_entity_type: String,
    #[serde(default = "default_advisor_entity_type")]
    advisor_entity_type: String,
    #[serde(default = "default_elephant_entity_type")]
    elephant_entity_type: String,
    #[serde(default = "default_horse_entity_type")]
    horse_entity_type: String,
    #[serde(default = "default_rook_entity_type")]
    rook_entity_type: String,
    #[serde(default = "default_cannon_entity_type")]
    cannon_entity_type: String,
    #[serde(default = "default_pawn_entity_type")]
    pawn_entity_type: String,
}

impl Default for PieceEntityConfig {
    fn default() -> Self {
        Self {
            y_offset: default_piece_y_offset(),
            general_entity_type: default_general_entity_type(),
            advisor_entity_type: default_advisor_entity_type(),
            elephant_entity_type: default_elephant_entity_type(),
            horse_entity_type: default_horse_entity_type(),
            rook_entity_type: default_rook_entity_type(),
            cannon_entity_type: default_cannon_entity_type(),
            pawn_entity_type: default_pawn_entity_type(),
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
    "xiangqi_select".to_string()
}

fn default_queue_ttl_ms() -> i64 {
    300_000
}

fn default_bot_name() -> String {
    "浅屿棋手".to_string()
}

fn default_bot_search_depth() -> usize {
    2
}

fn default_bot_max_candidates() -> usize {
    16
}

fn default_bot_node_budget() -> usize {
    900
}

fn default_lobby_dimension() -> String {
    "qexed:xiangqi_lobby".to_string()
}

fn default_board_dimension() -> String {
    "qexed:xiangqi_game".to_string()
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
    r#"{"text":"中国象棋入口","color":"gold","bold":true}"#.to_string()
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

fn default_river_block() -> String {
    "minecraft:warped_planks".to_string()
}

fn default_palace_block() -> String {
    "minecraft:oak_planks".to_string()
}

fn default_red_block() -> String {
    "minecraft:red_concrete".to_string()
}

fn default_black_block() -> String {
    "minecraft:blue_concrete".to_string()
}

fn default_piece_y_offset() -> f64 {
    1.0
}

fn default_general_entity_type() -> String {
    "minecraft:villager".to_string()
}

fn default_advisor_entity_type() -> String {
    "minecraft:wandering_trader".to_string()
}

fn default_elephant_entity_type() -> String {
    "minecraft:llama".to_string()
}

fn default_horse_entity_type() -> String {
    "minecraft:horse".to_string()
}

fn default_rook_entity_type() -> String {
    "minecraft:cow".to_string()
}

fn default_cannon_entity_type() -> String {
    "minecraft:goat".to_string()
}

fn default_pawn_entity_type() -> String {
    "minecraft:pig".to_string()
}

const DEFAULT_CONFIG: &str = r#"enable = true
server_name = "浅屿闲游"
qq_group = "722632621"
lobby_server = "lobby_1"
menu_id = "xiangqi_select"

[matchmaking]
queue_ttl_ms = 300000

[bot]
name = "浅屿棋手"
search_depth = 2
max_candidates = 16
node_budget = 900

[lobby]
dimension = "qexed:xiangqi_lobby"
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
npc_display_name = "{\"text\":\"中国象棋入口\",\"color\":\"gold\",\"bold\":true}"

[board]
dimension = "qexed:xiangqi_game"
origin_x = 4096
origin_y = 80
origin_z = 4096
instance_spacing = 64
floor_block = "minecraft:smooth_stone"
border_block = "minecraft:dark_oak_planks"
board_block = "minecraft:birch_planks"
river_block = "minecraft:warped_planks"
palace_block = "minecraft:oak_planks"
red_block = "minecraft:red_concrete"
black_block = "minecraft:blue_concrete"

[pieces]
y_offset = 1.0
general_entity_type = "minecraft:villager"
advisor_entity_type = "minecraft:wandering_trader"
elephant_entity_type = "minecraft:llama"
horse_entity_type = "minecraft:horse"
rook_entity_type = "minecraft:cow"
cannon_entity_type = "minecraft:goat"
pawn_entity_type = "minecraft:pig"
"#;

#[cfg(test)]
mod tests {
    use super::*;

    fn game() -> GameState {
        GameState {
            id: "test".to_string(),
            status: "playing".to_string(),
            mode: "test".to_string(),
            room_code: String::new(),
            red_player: "red".to_string(),
            red_name: "red".to_string(),
            black_player: "black".to_string(),
            black_name: "black".to_string(),
            bot: false,
            pieces: initial_pieces(),
            turn: RED,
            winner: 0,
            dimension: "qexed:test".to_string(),
            origin_x: 0,
            origin_y: 80,
            origin_z: 0,
            started_at_ms: 0,
            ended_elapsed_ms: 0,
        }
    }

    #[test]
    fn horse_leg_blocks_move() {
        let game = game();
        assert!(validate_move(&game, "r_horse_1", 2, 7).is_err());
    }

    #[test]
    fn cannon_requires_single_screen_to_capture() {
        let mut game = game();
        game.pieces.retain(|piece| {
            matches!(
                piece.id.as_str(),
                "r_cannon_left" | "b_cannon_left" | "b_pawn_0" | "r_general_4" | "b_general_4"
            )
        });
        let cannon = game
            .pieces
            .iter_mut()
            .find(|piece| piece.id == "r_cannon_left")
            .unwrap();
        cannon.x = 1;
        cannon.z = 7;
        let target = game
            .pieces
            .iter_mut()
            .find(|piece| piece.id == "b_cannon_left")
            .unwrap();
        target.x = 1;
        target.z = 2;
        assert!(validate_move(&game, "r_cannon_left", 1, 2).is_err());
        game.pieces
            .iter_mut()
            .find(|piece| piece.id == "b_pawn_0")
            .unwrap()
            .x = 1;
        assert!(validate_move(&game, "r_cannon_left", 1, 2).is_ok());
    }

    #[test]
    fn kings_may_not_face_after_move() {
        let mut game = game();
        game.pieces.retain(|piece| {
            matches!(
                piece.id.as_str(),
                "r_general_4" | "b_general_4" | "r_rook_0"
            )
        });
        let rook = game
            .pieces
            .iter_mut()
            .find(|piece| piece.id == "r_rook_0")
            .unwrap();
        rook.x = 4;
        rook.z = 5;
        assert!(validate_move(&game, "r_rook_0", 3, 5).is_err());
    }

    #[test]
    fn pawn_moves_sideways_only_after_river() {
        let mut game = game();
        {
            let pawn = game
                .pieces
                .iter_mut()
                .find(|piece| piece.id == "r_pawn_0")
                .unwrap();
            pawn.x = 0;
            pawn.z = 6;
        }
        assert!(validate_move(&game, "r_pawn_0", 1, 6).is_err());
        game.pieces
            .iter_mut()
            .find(|piece| piece.id == "r_pawn_0")
            .unwrap()
            .z = 4;
        assert!(validate_move(&game, "r_pawn_0", 1, 4).is_ok());
    }

}

use chess::{Board, BoardStatus, ChessMove, Color, File, MoveGen, Piece, Rank, Square};
use qexed_plugin_sdk::{
    ConfigReloadPayload, NpcInteractPayload, NpcMutationOp, NpcMutationResponse, NpcUpsert,
    PlaceholderQuery, PlaceholderReplacement, PlaceholderResponse, PlayerAction,
    PlayerBlockInteractPayload, PlayerPayload, PlayerTickPayload, PluginCommandDefinition,
    PluginCommandQuery, PluginCommandResponse, PluginManifest, RuntimeEntity, RuntimeEntityEvents,
    WorldEditRegion, config_load_or_create, config_read_to_string, entity_remove,
    entity_upsert_with_events, storage_delete, storage_get_typed, storage_set_typed, time_millis,
    world_register_edit_region, world_set_blocks,
};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

qexed_plugin_sdk::qexed_plugin_memory!();
qexed_plugin_sdk::qexed_plugin_manifest!(PluginManifest {
    id: "qexed.chess".to_string(),
    version: "0.1.0".to_string(),
    depends: Vec::new(),
    optional_depends: Vec::new(),
    services: Vec::new(),
});

const CONFIG_PATH: &str = "config.toml";
const COMMAND_NAME: &str = "chess";
const NPC_KEY: &str = "chess:guide";
const NPC_EVENT: &str = "chess";
const PIECE_EVENT: &str = "chess_piece";
const SESSION_PREFIX: &str = "chess/session/";
const GAME_PREFIX: &str = "chess/game/";
const ROOM_PREFIX: &str = "chess/room/";
const PENDING_PREFIX: &str = "chess/pending/";
const ENTITY_SYNC_PREFIX: &str = "chess/entity_sync/";
const INTERACT_COOLDOWN_PREFIX: &str = "chess/interact_cooldown/";
const QUEUE_KEY: &str = "chess/queue";
const INTERACT_COOLDOWN_MS: i64 = 120;
const BOARD_SIZE: usize = 8;
const INSTANCE_GRID_WIDTH: u64 = 2048;
const WHITE: u8 = 1;
const BLACK: u8 = 2;

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_priority() -> i32 {
    279
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_init() {
    let _ = config_load_or_create(CONFIG_PATH, DEFAULT_CONFIG);
    let config = load_config();
    register_lobby(&config);
    build_lobby(&config);
    qexed_plugin_sdk::log("chess initialized");
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_config_reload(ptr: i32, len: i32) {
    let Some(payload) =
        (unsafe { qexed_plugin_sdk::decode_payload::<ConfigReloadPayload>(ptr, len) })
    else {
        return;
    };
    let path = payload.path.replace('\\', "/");
    if path.ends_with("chess/config.toml") || path.ends_with(CONFIG_PATH) {
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
    push_pending(&payload.uuid, lobby_spawn_actions(&load_config()));
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_player_leave(ptr: i32, len: i32) {
    let Some(payload) = (unsafe { qexed_plugin_sdk::decode_payload::<PlayerPayload>(ptr, len) })
    else {
        return;
    };
    exit_current_game(&load_config(), &payload.uuid);
    let _ = storage_delete(&pending_key(&payload.uuid));
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_commands(_ptr: i32, _len: i32) -> i64 {
    qexed_plugin_sdk::response_ptr_len(&PluginCommandDefinition {
        name: COMMAND_NAME.to_string(),
        description_key: "qexed.plugin.chess.command.description".to_string(),
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
                name: "Chess".to_string(),
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
        handle_piece_interact(&config, &payload)
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
    let mut actions = Vec::new();
    if let Some(session) = active_session(&payload.player.uuid)
        && let Some(game) = stored_game(&session.game_id)
    {
        sync_entities_for_player_if_needed(&config, &payload.player.uuid, &game);
    }
    if let Some(pending) = storage_get_typed::<PendingActions>(&pending_key(&payload.player.uuid)) {
        let _ = storage_delete(&pending_key(&payload.player.uuid));
        actions.extend(pending.actions);
    }
    if actions.is_empty() {
        return qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse::default());
    }
    qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse {
        handled: true,
        actions,
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
        return handled(vec![message("国际象棋插件已禁用")]);
    }
    let mut parts = payload.argument.split_whitespace();
    match parts.next().unwrap_or_default() {
        "" | "menu" | "select" => open_menu(config),
        "help" => help_response(config),
        "bot" | "ai" => start_bot_game(config, &payload.player.uuid, &payload.player.username),
        "match" | "queue" => {
            join_match_queue(config, &payload.player.uuid, &payload.player.username)
        }
        "create" | "room" => create_room(config, &payload.player.uuid, &payload.player.username),
        "join" => join_room(
            config,
            &payload.player.uuid,
            &payload.player.username,
            parts.next().unwrap_or_default(),
        ),
        "move" => {
            let from = parts.next().unwrap_or_default();
            let to = parts.next().unwrap_or_default();
            command_move(config, &payload.player.uuid, from, to)
        }
        "status" | "info" => status_command(config, &payload.player.uuid),
        "resign" | "surrender" => resign_game(config, &payload.player.uuid),
        "leave" | "lobby" | "spawn" => leave_to_lobby(config, &payload.player.uuid),
        _ => handled(vec![message(
            "用法: /chess bot | match | create | join <房间码> | move <e2> <e4> | status | resign | leave",
        )]),
    }
}

fn open_menu(config: &Config) -> PluginCommandResponse {
    handled(vec![PlayerAction::OpenMenu {
        menu: config.menu_id.clone(),
    }])
}

fn help_response(config: &Config) -> PluginCommandResponse {
    handled(vec![message(format!(
        "国际象棋 - {} QQ群 {}: /chess bot 人机, /chess match 匹配, /chess create 创建房间, /chess join <房间码> 加入",
        config.server_name, config.qq_group
    ))])
}

fn start_bot_game(config: &Config, player_uuid: &str, player_name: &str) -> PluginCommandResponse {
    if active_session(player_uuid).is_some() {
        return handled(vec![message("你已经在一局国际象棋中，先 /chess leave")]);
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
    handled(vec![
        message("已开始国际象棋人机局。你执白棋，点击白方棋子后点击目标格。"),
        teleport_to_game(&game, player_uuid),
        progress_bar(player_uuid, &game),
    ])
}

fn join_match_queue(
    config: &Config,
    player_uuid: &str,
    player_name: &str,
) -> PluginCommandResponse {
    if active_session(player_uuid).is_some() {
        return handled(vec![message("你已经在对局中，先 /chess leave")]);
    }
    let now = time_millis();
    if let Some(queue) = storage_get_typed::<MatchQueue>(QUEUE_KEY) {
        if queue.player_uuid == player_uuid {
            return handled(vec![message("你已经在国际象棋匹配队列中")]);
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
                vec![
                    message(format!("已匹配到 {player_name}，你执白棋")),
                    teleport_to_game(&game, &queue.player_uuid),
                    progress_bar(&queue.player_uuid, &game),
                ],
            );
            return handled(vec![
                message(format!("已匹配到 {}，你执黑棋", queue.player_name)),
                teleport_to_game(&game, player_uuid),
                progress_bar(player_uuid, &game),
            ]);
        }
        let _ = storage_delete(QUEUE_KEY);
    }
    let _ = storage_set_typed(
        QUEUE_KEY,
        &MatchQueue {
            player_uuid: player_uuid.to_string(),
            player_name: player_name.to_string(),
            queued_at_ms: now,
        },
    );
    handled(vec![message("已进入国际象棋匹配队列")])
}

fn create_room(config: &Config, player_uuid: &str, player_name: &str) -> PluginCommandResponse {
    if active_session(player_uuid).is_some() {
        return handled(vec![message("你已经在对局中，先 /chess leave")]);
    }
    let mut game = new_game(config, "room", player_uuid, player_name, "", "", "");
    game.status = "waiting".to_string();
    game.room_code = room_code(player_uuid);
    save_game(&game);
    let _ = storage_set_typed(
        &room_key(&game.room_code),
        &RoomState {
            game_id: game.id.clone(),
        },
    );
    let _ = storage_set_typed(
        &session_key(player_uuid),
        &PlayerSession {
            game_id: game.id.clone(),
            selected: String::new(),
        },
    );
    handled(vec![message(format!(
        "国际象棋房间已创建，房间码 {}。对方使用 /chess join {}",
        game.room_code, game.room_code
    ))])
}

fn join_room(
    config: &Config,
    player_uuid: &str,
    player_name: &str,
    code: &str,
) -> PluginCommandResponse {
    if active_session(player_uuid).is_some() {
        return handled(vec![message("你已经在对局中，先 /chess leave")]);
    }
    let Some(room) = storage_get_typed::<RoomState>(&room_key(code)) else {
        return handled(vec![message("房间不存在")]);
    };
    let Some(mut game) = stored_game(&room.game_id) else {
        return handled(vec![message("房间数据不存在")]);
    };
    if game.status != "waiting" || !game.black_player.is_empty() {
        return handled(vec![message("房间已经开始")]);
    }
    game.status = "playing".to_string();
    game.black_player = player_uuid.to_string();
    game.black_name = player_name.to_string();
    game.started_at_ms = time_millis();
    save_started_game(config, &game);
    let _ = storage_delete(&room_key(code));
    push_pending(
        &game.white_player,
        vec![
            message(format!("{player_name} 已加入房间，你执白棋先行")),
            teleport_to_game(&game, &game.white_player),
            progress_bar(&game.white_player, &game),
        ],
    );
    handled(vec![
        message(format!("已加入 {} 的房间，你执黑棋", game.white_name)),
        teleport_to_game(&game, player_uuid),
        progress_bar(player_uuid, &game),
    ])
}

fn command_move(config: &Config, player_uuid: &str, from: &str, to: &str) -> PluginCommandResponse {
    let Some(from_square) = parse_square_name(from) else {
        return handled(vec![message("起点格式应为 e2")]);
    };
    let Some(to_square) = parse_square_name(to) else {
        return handled(vec![message("终点格式应为 e4")]);
    };
    let Some(session) = active_session(player_uuid) else {
        return handled(vec![message("当前没有进行中的国际象棋对局")]);
    };
    save_selected(player_uuid, &session.game_id, from_square);
    make_selected_move(config, player_uuid, to_square)
}

fn status_command(config: &Config, player_uuid: &str) -> PluginCommandResponse {
    let Some(session) = stored_session(player_uuid) else {
        return handled(vec![message("当前没有国际象棋对局")]);
    };
    let Some(game) = stored_game(&session.game_id) else {
        return handled(vec![message("当前对局数据不存在")]);
    };
    handled(vec![
        message(format!(
            "国际象棋状态: {}，模式: {}，回合: {}，对手: {}，已选: {}",
            status_label(&game),
            mode_label(&game),
            turn_label(&game),
            opponent_label(&game, player_uuid),
            selected_label(&game, &session)
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
        return handled(vec![message("当前没有进行中的国际象棋对局")]);
    };
    let Some(mut game) = stored_game(&session.game_id) else {
        return handled(vec![message("当前对局数据不存在")]);
    };
    let Some(color) = chess_color_for_player(&game, player_uuid).map(color_to_side) else {
        return handled(vec![message("你不是这局国际象棋的玩家")]);
    };
    game.status = "won".to_string();
    game.winner = opponent_side(color);
    finish_game(&mut game);
    save_game(&game);
    cleanup_game_entities(&game);
    if let Some(opponent) = opponent_uuid(&game, player_uuid) {
        push_pending(opponent, finish_actions(config, &game, opponent));
    }
    handled(finish_actions(config, &game, player_uuid))
}

fn leave_to_lobby(config: &Config, player_uuid: &str) -> PluginCommandResponse {
    exit_current_game(config, player_uuid);
    let mut actions = vec![
        message("已返回国际象棋大厅"),
        PlayerAction::RemoveBossBar {
            id: boss_bar_id(player_uuid),
        },
    ];
    actions.extend(lobby_spawn_actions(config));
    handled(actions)
}

fn handle_piece_interact(config: &Config, payload: &NpcInteractPayload) -> PluginCommandResponse {
    let Some((game_id, square)) = parse_piece_key(&payload.entity.key) else {
        return PluginCommandResponse::default();
    };
    let Some(session) = active_session(&payload.player.uuid) else {
        return handled(vec![message("先通过菜单开始一局国际象棋")]);
    };
    if session.game_id != game_id {
        return PluginCommandResponse::default();
    }
    if !consume_interaction(&payload.player.uuid) {
        return PluginCommandResponse::default();
    }
    let Some(game) = stored_game(&game_id) else {
        return handled(vec![message("当前对局数据不存在")]);
    };
    let Some(board) = board(&game) else {
        return handled(vec![message("棋盘数据损坏")]);
    };
    let Some(piece_color) = board.color_on(square) else {
        return handled(vec![message("这个棋子已经不在棋盘上")]);
    };
    if piece_color == chess_color_for_player(&game, &payload.player.uuid).unwrap_or(Color::White) {
        select_square(&payload.player.uuid, &game, square)
    } else if !session.selected.is_empty() {
        make_selected_move(config, &payload.player.uuid, square)
    } else {
        handled(vec![message("先点击自己的棋子，再点击目标格或目标棋子")])
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
    let Some(square) = square_from_position(&game, x, y, z) else {
        return PluginCommandResponse::default();
    };
    if !consume_interaction(player_uuid) {
        return PluginCommandResponse::default();
    }
    let Some(board) = board(&game) else {
        return handled(vec![message("棋盘数据损坏")]);
    };
    if let Some(piece_color) = board.color_on(square)
        && Some(piece_color) == chess_color_for_player(&game, player_uuid)
    {
        return select_square(player_uuid, &game, square);
    }
    make_selected_move(config, player_uuid, square)
}

fn select_square(player_uuid: &str, game: &GameState, square: Square) -> PluginCommandResponse {
    if game.status != "playing" {
        return handled(vec![message(format!("当前对局{}", status_label(game)))]);
    }
    let Some(color) = chess_color_for_player(game, player_uuid) else {
        return handled(vec![message("你不是这局国际象棋的玩家")]);
    };
    let Some(board) = board(game) else {
        return handled(vec![message("棋盘数据损坏")]);
    };
    if board.side_to_move() != color {
        return handled(vec![message(format!(
            "还没轮到你，当前回合: {}",
            turn_label(game)
        ))]);
    }
    if board.color_on(square) != Some(color) {
        return handled(vec![message("只能选择自己的棋子")]);
    }
    save_selected(player_uuid, &game.id, square);
    handled(vec![message(format!(
        "已选择 {}，点击目标格",
        square_name(square)
    ))])
}

fn make_selected_move(
    config: &Config,
    player_uuid: &str,
    to_square: Square,
) -> PluginCommandResponse {
    let Some(mut session) = active_session(player_uuid) else {
        return handled(vec![message("当前没有进行中的国际象棋对局")]);
    };
    let Some(mut game) = stored_game(&session.game_id) else {
        return handled(vec![message("当前对局数据不存在")]);
    };
    if game.status != "playing" {
        return handled(vec![message(format!("当前对局{}", status_label(&game)))]);
    }
    let Some(color) = chess_color_for_player(&game, player_uuid) else {
        return handled(vec![message("你不是这局国际象棋的玩家")]);
    };
    let Some(mut board) = board(&game) else {
        return handled(vec![message("棋盘数据损坏")]);
    };
    if board.side_to_move() != color {
        return handled(vec![message(format!(
            "还没轮到你，当前回合: {}",
            turn_label(&game)
        ))]);
    }
    let Some(from_square) = parse_square_name(&session.selected) else {
        return handled(vec![message("先点击自己的棋子，再点击落点")]);
    };
    let Some(chess_move) = legal_move_between(&board, from_square, to_square) else {
        return handled(vec![message("这个走法不合法")]);
    };

    board = board.make_move_new(chess_move);
    game.board_fen = board.to_string();
    update_game_status(&mut game, &board, color_to_side(color));
    session.selected.clear();
    let _ = storage_set_typed(&session_key(player_uuid), &session);
    save_game(&game);
    render_board(config, &game);
    render_piece_entities(config, &game);

    let mut actions = vec![message(format!(
        "你走了 {} -> {}",
        square_name(from_square),
        square_name(to_square)
    ))];
    if game.status == "playing" && game.bot && board.side_to_move() == Color::Black {
        if let Some(bot_move) = bot_choose_move(&board, &config.bot) {
            let bot_from = bot_move.get_source();
            let bot_to = bot_move.get_dest();
            board = board.make_move_new(bot_move);
            game.board_fen = board.to_string();
            update_game_status(&mut game, &board, BLACK);
            save_game(&game);
            render_board(config, &game);
            render_piece_entities(config, &game);
            actions.push(message(format!(
                "{} 走了 {} -> {}",
                game.black_name,
                square_name(bot_from),
                square_name(bot_to)
            )));
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
        cleanup_game_entities(&game);
        actions.extend(finish_actions(config, &game, player_uuid));
        if let Some(opponent) = opponent_uuid(&game, player_uuid) {
            push_pending(opponent, finish_actions(config, &game, opponent));
        }
    }
    handled(actions)
}

fn legal_move_between(board: &Board, from: Square, to: Square) -> Option<ChessMove> {
    MoveGen::new_legal(board)
        .filter(|candidate| candidate.get_source() == from && candidate.get_dest() == to)
        .max_by_key(|candidate| promotion_score(candidate.get_promotion()))
}

fn promotion_score(piece: Option<Piece>) -> i32 {
    match piece {
        Some(Piece::Queen) => 5,
        Some(Piece::Rook) => 4,
        Some(Piece::Bishop) => 3,
        Some(Piece::Knight) => 2,
        Some(Piece::Pawn) | Some(Piece::King) | None => 1,
    }
}

fn bot_choose_move(board: &Board, config: &BotConfig) -> Option<ChessMove> {
    let depth = config.search_depth.clamp(1, 3);
    let max_candidates = config.max_candidates.clamp(4, 32);
    let mut budget = config.node_budget.clamp(100, 5_000);
    let mut moves = ordered_moves(board, max_candidates);
    let mut best = None;
    let mut alpha = -2_000_000;
    for candidate in moves.drain(..) {
        if budget == 0 {
            break;
        }
        let trial = board.make_move_new(candidate);
        let score = -negamax(
            &trial,
            depth.saturating_sub(1),
            -2_000_000,
            -alpha,
            max_candidates,
            &mut budget,
        );
        if best.as_ref().is_none_or(|(_, current)| score > *current) {
            best = Some((candidate, score));
        }
        alpha = alpha.max(score);
    }
    best.map(|(chess_move, _)| chess_move)
}

fn negamax(
    board: &Board,
    depth: usize,
    mut alpha: i32,
    beta: i32,
    max_candidates: usize,
    budget: &mut usize,
) -> i32 {
    if *budget == 0 {
        return evaluate_board(board);
    }
    *budget = (*budget).saturating_sub(1);
    match board.status() {
        BoardStatus::Checkmate => return -1_000_000 - depth as i32,
        BoardStatus::Stalemate => return 0,
        BoardStatus::Ongoing => {}
    }
    if depth == 0 {
        return evaluate_board(board);
    }
    let moves = ordered_moves(board, max_candidates);
    if moves.is_empty() {
        return evaluate_board(board);
    }
    let mut best = -2_000_000;
    for candidate in moves {
        let trial = board.make_move_new(candidate);
        let score = -negamax(
            &trial,
            depth.saturating_sub(1),
            -beta,
            -alpha,
            max_candidates,
            budget,
        );
        best = best.max(score);
        alpha = alpha.max(score);
        if alpha >= beta {
            break;
        }
    }
    best
}

fn ordered_moves(board: &Board, max_candidates: usize) -> Vec<ChessMove> {
    let mut moves = MoveGen::new_legal(board).collect::<Vec<_>>();
    moves.sort_by_cached_key(|candidate| std::cmp::Reverse(move_score(board, *candidate)));
    moves.truncate(max_candidates);
    moves
}

fn move_score(board: &Board, chess_move: ChessMove) -> i32 {
    let mut score = 0;
    if let Some(piece) = board.piece_on(chess_move.get_dest()) {
        score += piece_value(piece) * 20;
    }
    score += promotion_score(chess_move.get_promotion()) * 100;
    let trial = board.make_move_new(chess_move);
    if trial.checkers().popcnt() > 0 {
        score += 80;
    }
    score
}

fn evaluate_board(board: &Board) -> i32 {
    let side = board.side_to_move();
    let mut score = 0;
    for rank in 0..BOARD_SIZE {
        for file in 0..BOARD_SIZE {
            let square = square(file, rank);
            let Some(piece) = board.piece_on(square) else {
                continue;
            };
            let sign = if board.color_on(square) == Some(side) {
                1
            } else {
                -1
            };
            score += sign * (piece_value(piece) * 10 + positional_score(piece, square));
        }
    }
    score
}

fn piece_value(piece: Piece) -> i32 {
    match piece {
        Piece::Pawn => 100,
        Piece::Knight => 320,
        Piece::Bishop => 330,
        Piece::Rook => 500,
        Piece::Queen => 900,
        Piece::King => 20_000,
    }
}

fn positional_score(piece: Piece, square: Square) -> i32 {
    let file = square.get_file().to_index() as i32;
    let rank = square.get_rank().to_index() as i32;
    let center = 6 - ((file - 3).abs() + (rank - 3).abs());
    match piece {
        Piece::Pawn => rank * 8 + center * 2,
        Piece::Knight | Piece::Bishop => center * 8,
        Piece::Rook => center * 3,
        Piece::Queen => center * 5,
        Piece::King => -center * 2,
    }
}

fn update_game_status(game: &mut GameState, board: &Board, moved_side: u8) {
    match board.status() {
        BoardStatus::Ongoing => {
            game.status = "playing".to_string();
            game.winner = 0;
        }
        BoardStatus::Stalemate => {
            game.status = "draw".to_string();
            game.winner = 0;
        }
        BoardStatus::Checkmate => {
            game.status = "won".to_string();
            game.winner = moved_side;
        }
    }
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

fn register_lobby(config: &Config) {
    let center_x = config.lobby.spawn_x.floor() as i32;
    let center_z = config.lobby.spawn_z.floor() as i32;
    let y = config.lobby.floor_y;
    let _ = world_register_edit_region(&WorldEditRegion {
        id: "chess_lobby",
        dimension: &config.lobby.dimension,
        min: (center_x - 8, y - 2, center_z - 8),
        max: (center_x + 8, y + 6, center_z + 8),
        allow_player_break: false,
        allow_player_place: false,
        allow_plugin_write: true,
        runtime_only: true,
    });
}

fn build_lobby(config: &Config) {
    let dimension = config.lobby.dimension.as_str();
    let y = config.lobby.floor_y;
    let center_x = config.lobby.spawn_x.floor() as i32;
    let center_z = config.lobby.spawn_z.floor() as i32;
    let mut blocks = Vec::new();
    for dx in -5..=5 {
        for dz in -5..=5 {
            let x = center_x + dx;
            let z = center_z + dz;
            let floor = if dx.abs() == 5 || dz.abs() == 5 {
                config.board.border_block.as_str()
            } else {
                config.board.light_square_block.as_str()
            };
            blocks.push((dimension, (x, y - 1, z), floor));
            for air_y in y..=y + 4 {
                blocks.push((dimension, (x, air_y, z), "minecraft:air"));
            }
        }
    }
    let _ = world_set_blocks(blocks);
}

fn render_board(config: &Config, game: &GameState) {
    let dimension = game.dimension.as_str();
    let y = game.origin_y;
    let mut blocks = Vec::new();
    for dx in -1..=BOARD_SIZE as i32 {
        for dz in -1..=BOARD_SIZE as i32 {
            let x = game.origin_x + dx;
            let z = game.origin_z + dz;
            if dx < 0 || dz < 0 || dx >= BOARD_SIZE as i32 || dz >= BOARD_SIZE as i32 {
                blocks.push((dimension, (x, y - 1, z), config.board.border_block.as_str()));
            } else {
                let block = if (dx + dz) % 2 == 0 {
                    config.board.light_square_block.as_str()
                } else {
                    config.board.dark_square_block.as_str()
                };
                blocks.push((dimension, (x, y - 1, z), block));
            }
            for air_y in y..=y + 4 {
                blocks.push((dimension, (x, air_y, z), "minecraft:air"));
            }
        }
    }
    let _ = world_set_blocks(blocks);
}

fn render_piece_entities(config: &Config, game: &GameState) {
    cleanup_game_entities(game);
    let Some(board) = board(game) else {
        return;
    };
    for rank in 0..BOARD_SIZE {
        for file in 0..BOARD_SIZE {
            let square = square(file, rank);
            let Some(piece) = board.piece_on(square) else {
                continue;
            };
            let color = board.color_on(square).unwrap_or(Color::White);
            spawn_piece_entity(config, game, square, piece, color);
        }
    }
}

fn spawn_piece_entity(
    config: &Config,
    game: &GameState,
    square: Square,
    piece: Piece,
    color: Color,
) {
    let key = piece_entity_key(game, square);
    let display_name = piece_display_name(piece, color);
    let _ = entity_upsert_with_events(
        &RuntimeEntity {
            key: &key,
            dimension: &game.dimension,
            entity_type: entity_type_for_piece(config, piece),
            x: game.origin_x as f64 + file_index(square) as f64 + 0.5,
            y: game.origin_y as f64 + config.pieces.y_offset,
            z: game.origin_z as f64 + rank_from_square(square) as f64 + 0.5,
            yaw: if color == Color::White { 180.0 } else { 0.0 },
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

fn cleanup_game_entities(game: &GameState) {
    for rank in 0..BOARD_SIZE {
        for file in 0..BOARD_SIZE {
            let _ = entity_remove(&piece_entity_key(game, square(file, rank)));
        }
    }
}

fn sync_entities_for_player_if_needed(config: &Config, player_uuid: &str, game: &GameState) {
    let key = entity_sync_key(player_uuid, &game.id);
    if storage_get_typed::<bool>(&key).unwrap_or(false) {
        return;
    }
    render_piece_entities(config, game);
    let _ = storage_set_typed(&key, &true);
}

fn save_started_game(config: &Config, game: &GameState) {
    register_arena(game);
    render_board(config, game);
    save_game(game);
    let _ = storage_set_typed(
        &session_key(&game.white_player),
        &PlayerSession {
            game_id: game.id.clone(),
            selected: String::new(),
        },
    );
    if !game.black_player.is_empty() && game.black_player != "bot" {
        let _ = storage_set_typed(
            &session_key(&game.black_player),
            &PlayerSession {
                game_id: game.id.clone(),
                selected: String::new(),
            },
        );
    }
}

fn register_arena(game: &GameState) {
    let id = format!("chess_{}", game.id);
    let _ = world_register_edit_region(&WorldEditRegion {
        id: &id,
        dimension: &game.dimension,
        min: (game.origin_x - 2, game.origin_y - 2, game.origin_z - 2),
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

fn exit_current_game(config: &Config, player_uuid: &str) {
    if let Some(session) = stored_session(player_uuid)
        && let Some(mut game) = stored_game(&session.game_id)
    {
        if game.status == "playing" || game.status == "waiting" {
            game.status = "won".to_string();
            game.winner = if game.white_player == player_uuid {
                BLACK
            } else {
                WHITE
            };
            finish_game(&mut game);
            save_game(&game);
            cleanup_game_entities(&game);
            if let Some(opponent) = opponent_uuid(&game, player_uuid) {
                push_pending(opponent, finish_actions(config, &game, opponent));
            }
        }
    }
    let _ = storage_delete(&session_key(player_uuid));
}

fn new_game(
    config: &Config,
    mode: &str,
    white_player: &str,
    white_name: &str,
    black_player: &str,
    black_name: &str,
    room_code: &str,
) -> GameState {
    let id = game_id_for(white_player, black_player, time_millis());
    let origin = instance_origin(config, &id);
    GameState {
        id,
        status: "playing".to_string(),
        mode: mode.to_string(),
        room_code: room_code.to_string(),
        white_player: white_player.to_string(),
        white_name: white_name.to_string(),
        black_player: black_player.to_string(),
        black_name: black_name.to_string(),
        bot: black_player == "bot",
        board_fen: Board::default().to_string(),
        winner: 0,
        dimension: config.board.dimension.clone(),
        origin_x: origin.0,
        origin_y: origin.1,
        origin_z: origin.2,
        started_at_ms: time_millis(),
        ended_elapsed_ms: 0,
    }
}

fn board(game: &GameState) -> Option<Board> {
    Board::from_str(&game.board_fen).ok()
}

fn square(file: usize, rank_from_top: usize) -> Square {
    Square::make_square(
        Rank::from_index(BOARD_SIZE - 1 - rank_from_top),
        File::from_index(file),
    )
}

fn file_index(square: Square) -> usize {
    square.get_file().to_index()
}

fn rank_from_square(square: Square) -> usize {
    BOARD_SIZE - 1 - square.get_rank().to_index()
}

fn parse_square_name(value: &str) -> Option<Square> {
    let value = value.trim().to_ascii_lowercase();
    let bytes = value.as_bytes();
    if bytes.len() != 2 {
        return None;
    }
    let file = bytes[0];
    let rank = bytes[1];
    if !(b'a'..=b'h').contains(&file) || !(b'1'..=b'8').contains(&rank) {
        return None;
    }
    let file = (file - b'a') as usize;
    let rank_from_top = BOARD_SIZE - (rank - b'0') as usize;
    Some(square(file, rank_from_top))
}

fn square_name(square: Square) -> String {
    let file = (b'a' + file_index(square) as u8) as char;
    let rank = (BOARD_SIZE - rank_from_square(square)).to_string();
    format!("{file}{rank}")
}

fn square_from_position(game: &GameState, x: i32, y: i32, z: i32) -> Option<Square> {
    if y < game.origin_y - 1 || y > game.origin_y + 3 {
        return None;
    }
    let local_x = x.checked_sub(game.origin_x)?;
    let local_z = z.checked_sub(game.origin_z)?;
    if local_x < 0 || local_z < 0 || local_x >= BOARD_SIZE as i32 || local_z >= BOARD_SIZE as i32 {
        return None;
    }
    Some(square(local_x as usize, local_z as usize))
}

fn color_to_side(color: Color) -> u8 {
    if color == Color::White { WHITE } else { BLACK }
}

fn chess_color_for_player(game: &GameState, player_uuid: &str) -> Option<Color> {
    if game.white_player == player_uuid {
        Some(Color::White)
    } else if game.black_player == player_uuid {
        Some(Color::Black)
    } else {
        None
    }
}

fn opponent_uuid<'a>(game: &'a GameState, player_uuid: &str) -> Option<&'a str> {
    if game.white_player == player_uuid
        && game.black_player != "bot"
        && !game.black_player.is_empty()
    {
        Some(&game.black_player)
    } else if game.black_player == player_uuid {
        Some(&game.white_player)
    } else {
        None
    }
}

fn turn_label(game: &GameState) -> String {
    board(game)
        .map(|board| {
            if board.side_to_move() == Color::White {
                "白棋".to_string()
            } else {
                "黑棋".to_string()
            }
        })
        .unwrap_or_else(|| "未知".to_string())
}

fn status_label(game: &GameState) -> String {
    match game.status.as_str() {
        "waiting" => "等待玩家".to_string(),
        "playing" => format!("进行中，{}回合", turn_label(game)),
        "won" => format!("结束，{}获胜", side_label(game.winner)),
        "draw" => "结束，和棋".to_string(),
        _ => game.status.clone(),
    }
}

fn side_label(side: u8) -> &'static str {
    match side {
        WHITE => "白棋",
        BLACK => "黑棋",
        _ => "无人",
    }
}

fn opponent_side(side: u8) -> u8 {
    if side == WHITE { BLACK } else { WHITE }
}

fn mode_label(game: &GameState) -> String {
    match game.mode.as_str() {
        "match" => "匹配".to_string(),
        "room" => "房间".to_string(),
        "bot" => "人机".to_string(),
        _ => game.mode.clone(),
    }
}

fn opponent_label(game: &GameState, player_uuid: &str) -> String {
    if game.white_player == player_uuid {
        game.black_name.clone()
    } else if game.black_player == player_uuid {
        game.white_name.clone()
    } else {
        "-".to_string()
    }
}

fn selected_label(game: &GameState, session: &PlayerSession) -> String {
    let Some(square) = parse_square_name(&session.selected) else {
        return "-".to_string();
    };
    let Some(board) = board(game) else {
        return "-".to_string();
    };
    let Some(piece) = board.piece_on(square) else {
        return "-".to_string();
    };
    let color = board.color_on(square).unwrap_or(Color::White);
    format!(
        "{}{} {}",
        if color == Color::White { "白" } else { "黑" },
        piece_label(piece),
        square_name(square)
    )
}

fn replacements(
    game: Option<&GameState>,
    session: Option<&PlayerSession>,
    player_uuid: &str,
) -> Vec<PlaceholderReplacement> {
    vec![
        placeholder(
            "chess_state",
            game.map(status_label).unwrap_or_else(|| "大厅".to_string()),
        ),
        placeholder(
            "chess_mode",
            game.map(mode_label).unwrap_or_else(|| "-".to_string()),
        ),
        placeholder(
            "chess_turn",
            game.map(turn_label).unwrap_or_else(|| "-".to_string()),
        ),
        placeholder(
            "chess_opponent",
            game.map(|game| opponent_label(game, player_uuid))
                .unwrap_or_else(|| "-".to_string()),
        ),
        placeholder(
            "chess_selected",
            game.zip(session)
                .map(|(game, session)| selected_label(game, session))
                .unwrap_or_else(|| "-".to_string()),
        ),
        placeholder(
            "chess_room",
            game.and_then(|game| (!game.room_code.is_empty()).then(|| game.room_code.clone()))
                .unwrap_or_else(|| "-".to_string()),
        ),
        placeholder(
            "chess_time",
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

fn save_selected(player_uuid: &str, game_id: &str, square: Square) {
    let _ = storage_set_typed(
        &session_key(player_uuid),
        &PlayerSession {
            game_id: game_id.to_string(),
            selected: square_name(square),
        },
    );
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
        return format!("和棋，用时 {}", format_duration_ms(game.ended_elapsed_ms));
    }
    let Some(color) = chess_color_for_player(game, player_uuid).map(color_to_side) else {
        return "对局结束".to_string();
    };
    if game.winner == color {
        format!("你赢了，用时 {}", format_duration_ms(game.ended_elapsed_ms))
    } else {
        format!("你输了，用时 {}", format_duration_ms(game.ended_elapsed_ms))
    }
}

fn teleport_to_game(game: &GameState, _player_uuid: &str) -> PlayerAction {
    PlayerAction::Teleport {
        dimension: game.dimension.clone(),
        x: game.origin_x as f64 + 3.5,
        y: game.origin_y as f64 + 2.0,
        z: game.origin_z as f64 + 9.5,
        yaw: Some(180.0),
        pitch: Some(45.0),
    }
}

fn lobby_spawn_actions(config: &Config) -> Vec<PlayerAction> {
    let mut actions = vec![
        PlayerAction::Teleport {
            dimension: config.lobby.dimension.clone(),
            x: config.lobby.spawn_x,
            y: config.lobby.spawn_y,
            z: config.lobby.spawn_z,
            yaw: Some(config.lobby.spawn_yaw),
            pitch: Some(config.lobby.spawn_pitch),
        },
        PlayerAction::SetPlayersVisible { visible: true },
    ];
    if !config.lobby_server.trim().is_empty() {
        actions.push(PlayerAction::ProxyConnect {
            server: config.lobby_server.clone(),
            message: String::new(),
        });
    }
    actions
}

fn progress_bar(player_uuid: &str, game: &GameState) -> PlayerAction {
    PlayerAction::BossBar {
        id: boss_bar_id(player_uuid),
        title: format!(
            "国际象棋 | {} | {}",
            status_label(game),
            player_name(game, player_uuid)
        ),
        progress: 1.0,
        color: "white".to_string(),
        overlay: "progress".to_string(),
    }
}

fn player_name(game: &GameState, player_uuid: &str) -> String {
    if game.white_player == player_uuid {
        game.white_name.clone()
    } else if game.black_player == player_uuid {
        game.black_name.clone()
    } else {
        "观战".to_string()
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
    format!("chess:{player_uuid}")
}

fn handled(actions: Vec<PlayerAction>) -> PluginCommandResponse {
    PluginCommandResponse {
        handled: true,
        actions,
    }
}

fn load_config() -> Config {
    config_read_to_string(CONFIG_PATH)
        .and_then(|contents| toml::from_str(&contents).ok())
        .unwrap_or_default()
}

fn active_session(player_uuid: &str) -> Option<PlayerSession> {
    storage_get_typed(&session_key(player_uuid)).and_then(|session: PlayerSession| {
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

fn push_pending(player_uuid: &str, actions: Vec<PlayerAction>) {
    if player_uuid == "bot" {
        return;
    }
    let mut pending =
        storage_get_typed::<PendingActions>(&pending_key(player_uuid)).unwrap_or_default();
    pending.actions.extend(actions);
    let _ = storage_set_typed(&pending_key(player_uuid), &pending);
}

fn game_id_for(white_player: &str, black_player: &str, now: i64) -> String {
    format!("{white_player}-{black_player}-{now}")
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect()
}

fn room_code(player_uuid: &str) -> String {
    let value = fnv1a64(format!("{}:{}", player_uuid, time_millis()).as_bytes()) % 10000;
    format!("{value:04}")
}

fn instance_origin(config: &Config, game_id: &str) -> (i32, i32, i32) {
    let hash = fnv1a64(game_id.as_bytes());
    let lane = (hash % INSTANCE_GRID_WIDTH) as i32;
    let row = ((hash / INSTANCE_GRID_WIDTH) % INSTANCE_GRID_WIDTH) as i32;
    (
        config.board.origin_x + lane * config.board.instance_spacing,
        config.board.floor_y,
        config.board.origin_z + row * config.board.instance_spacing,
    )
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn elapsed_millis(started_at_ms: i64) -> i64 {
    (time_millis() - started_at_ms).max(0)
}

fn format_duration_ms(ms: i64) -> String {
    let seconds = (ms / 1000).max(0);
    format!("{:02}:{:02}", seconds / 60, seconds % 60)
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

fn piece_entity_key(game: &GameState, square: Square) -> String {
    format!("game/{}/piece/{}", game.id, square_name(square))
}

fn parse_piece_key(key: &str) -> Option<(String, Square)> {
    let tail = key.strip_prefix("game/")?;
    let (game_id, square) = tail.split_once("/piece/")?;
    Some((game_id.to_string(), parse_square_name(square)?))
}

fn piece_display_name(piece: Piece, color: Color) -> String {
    let color_name = if color == Color::White {
        "white"
    } else {
        "dark_gray"
    };
    format!(
        r#"{{"text":"{}{}","color":"{}","bold":true}}"#,
        if color == Color::White { "白" } else { "黑" },
        piece_label(piece),
        color_name
    )
}

fn piece_label(piece: Piece) -> &'static str {
    match piece {
        Piece::King => "王",
        Piece::Queen => "后",
        Piece::Rook => "车",
        Piece::Bishop => "象",
        Piece::Knight => "马",
        Piece::Pawn => "兵",
    }
}

fn entity_type_for_piece(config: &Config, piece: Piece) -> &str {
    match piece {
        Piece::King => config.pieces.king_entity_type.as_str(),
        Piece::Queen => config.pieces.queen_entity_type.as_str(),
        Piece::Rook => config.pieces.rook_entity_type.as_str(),
        Piece::Bishop => config.pieces.bishop_entity_type.as_str(),
        Piece::Knight => config.pieces.knight_entity_type.as_str(),
        Piece::Pawn => config.pieces.pawn_entity_type.as_str(),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GameState {
    id: String,
    status: String,
    mode: String,
    room_code: String,
    white_player: String,
    white_name: String,
    black_player: String,
    black_name: String,
    bot: bool,
    board_fen: String,
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
    selected: String,
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
    #[serde(default = "default_floor_y")]
    floor_y: i32,
    #[serde(default = "default_board_origin_x")]
    origin_x: i32,
    #[serde(default = "default_board_origin_z")]
    origin_z: i32,
    #[serde(default = "default_instance_spacing")]
    instance_spacing: i32,
    #[serde(default = "default_light_square_block")]
    light_square_block: String,
    #[serde(default = "default_dark_square_block")]
    dark_square_block: String,
    #[serde(default = "default_border_block")]
    border_block: String,
}

impl Default for BoardConfig {
    fn default() -> Self {
        Self {
            dimension: default_board_dimension(),
            floor_y: default_floor_y(),
            origin_x: default_board_origin_x(),
            origin_z: default_board_origin_z(),
            instance_spacing: default_instance_spacing(),
            light_square_block: default_light_square_block(),
            dark_square_block: default_dark_square_block(),
            border_block: default_border_block(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct PieceEntityConfig {
    #[serde(default = "default_piece_y_offset")]
    y_offset: f64,
    #[serde(default = "default_king_entity_type")]
    king_entity_type: String,
    #[serde(default = "default_queen_entity_type")]
    queen_entity_type: String,
    #[serde(default = "default_rook_entity_type")]
    rook_entity_type: String,
    #[serde(default = "default_bishop_entity_type")]
    bishop_entity_type: String,
    #[serde(default = "default_knight_entity_type")]
    knight_entity_type: String,
    #[serde(default = "default_pawn_entity_type")]
    pawn_entity_type: String,
}

impl Default for PieceEntityConfig {
    fn default() -> Self {
        Self {
            y_offset: default_piece_y_offset(),
            king_entity_type: default_king_entity_type(),
            queen_entity_type: default_queen_entity_type(),
            rook_entity_type: default_rook_entity_type(),
            bishop_entity_type: default_bishop_entity_type(),
            knight_entity_type: default_knight_entity_type(),
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
    "chess_select".to_string()
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
    18
}

fn default_bot_node_budget() -> usize {
    1000
}

fn default_lobby_dimension() -> String {
    "qexed:chess_lobby".to_string()
}

fn default_board_dimension() -> String {
    "qexed:chess_game".to_string()
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
    r#"{"text":"国际象棋入口","color":"gold","bold":true}"#.to_string()
}

fn default_board_origin_x() -> i32 {
    8192
}

fn default_board_origin_z() -> i32 {
    8192
}

fn default_instance_spacing() -> i32 {
    48
}

fn default_light_square_block() -> String {
    "minecraft:birch_planks".to_string()
}

fn default_dark_square_block() -> String {
    "minecraft:spruce_planks".to_string()
}

fn default_border_block() -> String {
    "minecraft:dark_oak_planks".to_string()
}

fn default_piece_y_offset() -> f64 {
    1.0
}

fn default_king_entity_type() -> String {
    "minecraft:villager".to_string()
}

fn default_queen_entity_type() -> String {
    "minecraft:witch".to_string()
}

fn default_rook_entity_type() -> String {
    "minecraft:iron_golem".to_string()
}

fn default_bishop_entity_type() -> String {
    "minecraft:wandering_trader".to_string()
}

fn default_knight_entity_type() -> String {
    "minecraft:horse".to_string()
}

fn default_pawn_entity_type() -> String {
    "minecraft:pig".to_string()
}

const DEFAULT_CONFIG: &str = r#"enable = true
server_name = "浅屿闲游"
qq_group = "722632621"
lobby_server = "lobby_1"
menu_id = "chess_select"

[matchmaking]
queue_ttl_ms = 300000

[bot]
name = "浅屿棋手"
search_depth = 2
max_candidates = 18
node_budget = 1000

[lobby]
dimension = "qexed:chess_lobby"
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
npc_display_name = "{\"text\":\"国际象棋入口\",\"color\":\"gold\",\"bold\":true}"

[board]
dimension = "qexed:chess_game"
floor_y = 80
origin_x = 8192
origin_z = 8192
instance_spacing = 48
light_square_block = "minecraft:birch_planks"
dark_square_block = "minecraft:spruce_planks"
border_block = "minecraft:dark_oak_planks"

[pieces]
y_offset = 1.0
king_entity_type = "minecraft:villager"
queen_entity_type = "minecraft:witch"
rook_entity_type = "minecraft:iron_golem"
bishop_entity_type = "minecraft:wandering_trader"
knight_entity_type = "minecraft:horse"
pawn_entity_type = "minecraft:pig"
"#;

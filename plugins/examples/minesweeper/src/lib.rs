use qexed_plugin_sdk::{
    BlockDropQuery, BlockDropResponse, BlockStepPayload, ConfigReloadPayload, NpcInteractPayload,
    NpcMutationOp, NpcMutationResponse, NpcUpsert, PlaceholderQuery, PlaceholderReplacement,
    PlaceholderResponse, PlayerAction, PlayerBlockInteractPayload, PlayerPayload,
    PlayerTickPayload, PluginCommandDefinition, PluginCommandQuery, PluginCommandResponse,
    WorldEditRegion, config_load_or_create, config_read_to_string, storage_delete,
    storage_get_typed, storage_set_typed, time_millis, world_register_edit_region, world_set_block,
    world_set_blocks,
};
use serde::{Deserialize, Serialize};

qexed_plugin_sdk::qexed_plugin_memory!();

const CONFIG_PATH: &str = "config.toml";
const COMMAND_NAME: &str = "minesweeper";
const NPC_KEY: &str = "minesweeper:guide";
const NPC_EVENT: &str = "minesweeper";
const SESSION_PREFIX: &str = "minesweeper/session/";
const PENDING_MENU_PREFIX: &str = "minesweeper/pending_menu/";
const BOSS_BAR_PREFIX: &str = "minesweeper:";
const BEST_PREFIX: &str = "minesweeper/best/";
const MAX_BOARD_SIDE: usize = 32;
const INSTANCE_GRID_WIDTH: u64 = 4096;
const DEFAULT_NUMBER_BLOCKS: [&str; 8] = [
    "minecraft:blue_concrete",
    "minecraft:green_concrete",
    "minecraft:red_concrete",
    "minecraft:purple_concrete",
    "minecraft:orange_concrete",
    "minecraft:cyan_concrete",
    "minecraft:brown_concrete",
    "minecraft:black_concrete",
];

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_priority() -> i32 {
    270
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_init() {
    let _ = config_load_or_create(CONFIG_PATH, DEFAULT_CONFIG);
    let config = load_config();
    register_lobby(&config);
    build_lobby(&config);
    qexed_plugin_sdk::log("minesweeper initialized");
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_config_reload(ptr: i32, len: i32) {
    let Some(payload) =
        (unsafe { qexed_plugin_sdk::decode_payload::<ConfigReloadPayload>(ptr, len) })
    else {
        return;
    };
    let path = payload.path.replace('\\', "/");
    if path.ends_with("minesweeper/config.toml") || path.ends_with(CONFIG_PATH) {
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
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_player_leave(ptr: i32, len: i32) {
    let Some(payload) = (unsafe { qexed_plugin_sdk::decode_payload::<PlayerPayload>(ptr, len) })
    else {
        return;
    };
    let _ = storage_delete(&session_key(&payload.uuid));
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_commands(_ptr: i32, _len: i32) -> i64 {
    qexed_plugin_sdk::response_ptr_len(&PluginCommandDefinition {
        name: COMMAND_NAME.to_string(),
        description_key: "qexed.plugin.minesweeper.command.description".to_string(),
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
                name: "Minesweeper".to_string(),
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
    qexed_plugin_sdk::response_ptr_len(&open_select_menu(&config))
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_player_block_step(ptr: i32, len: i32) -> i64 {
    let Some(payload) = (unsafe { qexed_plugin_sdk::decode_payload::<BlockStepPayload>(ptr, len) })
    else {
        return qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse::default());
    };

    let config = load_config();
    if !config.enable {
        return qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse::default());
    }
    let Some(mut session) = active_session(&payload.player.uuid) else {
        return qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse::default());
    };
    if payload.dimension != session.dimension {
        return qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse::default());
    }
    let Some((x, z)) = cell_from_position(
        &session,
        payload.position.x,
        payload.position.y,
        payload.position.z,
    ) else {
        return qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse::default());
    };

    let outcome = reveal_cell(&config, &payload.player.uuid, &mut session, x, z);
    if matches!(
        outcome,
        RevealOutcome::AlreadyRevealed | RevealOutcome::Flagged
    ) {
        return qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse::default());
    }
    let _ = storage_set_typed(&session_key(&payload.player.uuid), &session);
    qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse {
        handled: true,
        actions: reveal_actions(&config, &payload.player.uuid, &session, outcome, false),
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_player_block_interact(ptr: i32, len: i32) -> i64 {
    let Some(payload) =
        (unsafe { qexed_plugin_sdk::decode_payload::<PlayerBlockInteractPayload>(ptr, len) })
    else {
        return qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse::default());
    };

    let config = load_config();
    if !config.enable || !config.controls.hand_can_mark(&payload.hand) {
        return qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse::default());
    }
    let Some(mut session) = active_session(&payload.player.uuid) else {
        return qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse::default());
    };
    if payload.dimension != session.dimension {
        return qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse::default());
    }
    let Some((x, z)) = cell_from_position(
        &session,
        payload.position.x,
        payload.position.y,
        payload.position.z,
    ) else {
        return qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse::default());
    };

    let response = toggle_flag_cell(&config, &payload.player.uuid, &mut session, x, z, false);
    qexed_plugin_sdk::response_ptr_len(&response)
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_block_drops(ptr: i32, len: i32) -> i64 {
    let Some(payload) = (unsafe { qexed_plugin_sdk::decode_payload::<BlockDropQuery>(ptr, len) })
    else {
        return qexed_plugin_sdk::response_ptr_len(&BlockDropResponse::default());
    };
    let Some(player) = payload.player else {
        return qexed_plugin_sdk::response_ptr_len(&BlockDropResponse::default());
    };

    let config = load_config();
    if !config.enable {
        return qexed_plugin_sdk::response_ptr_len(&BlockDropResponse::default());
    }
    let Some(mut session) = active_session(&player.uuid) else {
        return qexed_plugin_sdk::response_ptr_len(&BlockDropResponse::default());
    };
    if player.dimension != session.dimension {
        return qexed_plugin_sdk::response_ptr_len(&BlockDropResponse::default());
    }
    let Some((x, z)) = cell_from_position(
        &session,
        payload.position.x,
        payload.position.y,
        payload.position.z,
    ) else {
        return qexed_plugin_sdk::response_ptr_len(&BlockDropResponse::default());
    };

    let outcome = reveal_cell(&config, &player.uuid, &mut session, x, z);
    if matches!(outcome, RevealOutcome::Won | RevealOutcome::Lost) {
        let _ = storage_set_typed(&pending_menu_key(&player.uuid), &true);
    }
    if matches!(
        outcome,
        RevealOutcome::AlreadyRevealed | RevealOutcome::Flagged
    ) {
        render_cell(&config, &session, x, z);
    }
    let _ = storage_set_typed(&session_key(&player.uuid), &session);
    qexed_plugin_sdk::response_ptr_len(&BlockDropResponse {
        replace: true,
        items: Vec::new(),
        break_positions: Vec::new(),
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
    let best = session
        .as_ref()
        .and_then(|session| best_time_for_session(&player.uuid, session))
        .or_else(|| best_configured_time_ms(&load_config(), &player.uuid));

    qexed_plugin_sdk::response_ptr_len(&PlaceholderResponse {
        replacements: minesweeper_replacements(session.as_ref(), best),
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_player_tick(ptr: i32, len: i32) -> i64 {
    let Some(payload) =
        (unsafe { qexed_plugin_sdk::decode_payload::<PlayerTickPayload>(ptr, len) })
    else {
        return qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse::default());
    };
    if !storage_get_typed::<bool>(&pending_menu_key(&payload.player.uuid)).unwrap_or(false) {
        return qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse::default());
    }

    let _ = storage_delete(&pending_menu_key(&payload.player.uuid));
    let config = load_config();
    qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse {
        handled: true,
        actions: vec![PlayerAction::OpenMenu {
            menu: config.menu_id,
        }],
    })
}

fn handle_command(config: &Config, payload: &PluginCommandQuery) -> PluginCommandResponse {
    if !config.enable {
        return PluginCommandResponse {
            handled: true,
            actions: vec![message("扫雷插件已禁用")],
        };
    }

    let mut parts = payload.argument.split_whitespace();
    match parts.next().unwrap_or_default() {
        "" | "menu" | "select" => open_select_menu(config),
        "help" => help_response(),
        "new" | "reset" => start_game(config, &payload.player.uuid),
        "start" => {
            let difficulty_id = parts.next().unwrap_or(config.default_difficulty.as_str());
            start_difficulty_game(config, &payload.player.uuid, difficulty_id)
        }
        "custom" => custom_game_command(config, &payload.player.uuid, parts.collect()),
        "reveal" | "open" => reveal_command(config, &payload.player.uuid, parts.collect()),
        "flag" | "mark" => flag_command(config, &payload.player.uuid, parts.collect()),
        "status" | "info" => status_command(&payload.player.uuid),
        "leave" | "lobby" | "spawn" => leave_to_lobby(config, &payload.player.uuid),
        _ => PluginCommandResponse {
            handled: true,
            actions: vec![message(
                "用法: /minesweeper new | reveal <x> <z> | flag <x> <z> | status | leave",
            )],
        },
    }
}

fn open_select_menu(config: &Config) -> PluginCommandResponse {
    PluginCommandResponse {
        handled: true,
        actions: vec![PlayerAction::OpenMenu {
            menu: config.menu_id.clone(),
        }],
    }
}

fn help_response() -> PluginCommandResponse {
    PluginCommandResponse {
        handled: true,
        actions: vec![message(
            "扫雷: /minesweeper new 开始，踩格或 /minesweeper reveal <x> <z> 翻开，/minesweeper flag <x> <z> 标记，坐标从 1 开始",
        )],
    }
}

fn start_difficulty_game(
    config: &Config,
    player_uuid: &str,
    difficulty_id: &str,
) -> PluginCommandResponse {
    let Some(difficulty) = config.difficulty(difficulty_id) else {
        return PluginCommandResponse {
            handled: true,
            actions: vec![
                message("未知扫雷难度，请从菜单重新选择"),
                PlayerAction::OpenMenu {
                    menu: config.menu_id.clone(),
                },
            ],
        };
    };
    start_game_with_settings(
        config,
        player_uuid,
        BoardSettings::from_values(difficulty.width, difficulty.height, difficulty.mines),
        &difficulty.id,
        &difficulty.label,
    )
}

fn custom_game_command(
    config: &Config,
    player_uuid: &str,
    args: Vec<&str>,
) -> PluginCommandResponse {
    if args.len() != 3 {
        return PluginCommandResponse {
            handled: true,
            actions: vec![
                message("自定义用法: /minesweeper custom <宽> <高> <雷数>，范围 4..32，雷数至少 1"),
                PlayerAction::OpenMenu {
                    menu: config.menu_id.clone(),
                },
            ],
        };
    }
    let Some(width) = args[0].parse::<usize>().ok() else {
        return invalid_custom_response(config);
    };
    let Some(height) = args[1].parse::<usize>().ok() else {
        return invalid_custom_response(config);
    };
    let Some(mines) = args[2].parse::<usize>().ok() else {
        return invalid_custom_response(config);
    };
    let settings = BoardSettings::from_values(width, height, mines);
    let label = format!(
        "自定义 {}x{} / {} 雷",
        settings.width, settings.height, settings.mines
    );
    let difficulty_id = custom_difficulty_id(settings);
    start_game_with_settings(config, player_uuid, settings, &difficulty_id, &label)
}

fn invalid_custom_response(config: &Config) -> PluginCommandResponse {
    PluginCommandResponse {
        handled: true,
        actions: vec![
            message("自定义参数必须是正整数，例如 /minesweeper custom 16 16 40"),
            PlayerAction::OpenMenu {
                menu: config.menu_id.clone(),
            },
        ],
    }
}

fn start_game_with_settings(
    config: &Config,
    player_uuid: &str,
    settings: BoardSettings,
    difficulty_id: &str,
    difficulty_label: &str,
) -> PluginCommandResponse {
    if !config.enable {
        return PluginCommandResponse::default();
    }

    let origin = instance_origin(config, player_uuid);
    register_arena(config, player_uuid, origin, settings);
    let session = GameSession::new(
        &config.board.dimension,
        origin,
        settings,
        difficulty_id,
        difficulty_label,
    );
    if !build_arena(config, &session) {
        return PluginCommandResponse {
            handled: true,
            actions: vec![message("扫雷棋盘生成失败，请稍后重试")],
        };
    }
    let _ = storage_set_typed(&session_key(player_uuid), &session);

    PluginCommandResponse {
        handled: true,
        actions: vec![
            message(format!(
                "{} 扫雷开始，{}，QQ群 {}，棋盘 {}x{}，雷数 {}",
                config.server_name,
                difficulty_label,
                config.qq_group,
                session.width,
                session.height,
                session.mines
            )),
            message(
                "踩上格子可翻开；首点必定无雷；需要精确操作时使用 /minesweeper reveal <x> <z> 或 /minesweeper flag <x> <z>",
            ),
            PlayerAction::SetPlayersVisible { visible: false },
            progress_bar(player_uuid, &session),
            PlayerAction::Teleport {
                dimension: session.dimension.clone(),
                x: session.origin_x as f64 + f64::from(session.width) / 2.0,
                y: session.origin_y as f64 + 1.0,
                z: session.origin_z as f64 - 1.5,
                yaw: Some(0.0),
                pitch: Some(35.0),
            },
        ],
    }
}

fn start_game(config: &Config, player_uuid: &str) -> PluginCommandResponse {
    if !config.enable {
        return PluginCommandResponse::default();
    }

    let settings = BoardSettings::from_config(&config.board);
    let origin = instance_origin(config, player_uuid);
    register_arena(config, player_uuid, origin, settings);
    let session = GameSession::new(
        &config.board.dimension,
        origin,
        settings,
        &config.default_difficulty,
        "默认",
    );
    if !build_arena(config, &session) {
        return PluginCommandResponse {
            handled: true,
            actions: vec![message("扫雷棋盘生成失败，请稍后重试")],
        };
    }
    let _ = storage_set_typed(&session_key(player_uuid), &session);

    PluginCommandResponse {
        handled: true,
        actions: vec![
            message(format!(
                "{} 扫雷开始，QQ群 {}，棋盘 {}x{}，雷数 {}",
                config.server_name, config.qq_group, session.width, session.height, session.mines
            )),
            message(
                "踩上格子可翻开；需要精确操作时使用 /minesweeper reveal <x> <z> 或 /minesweeper flag <x> <z>",
            ),
            PlayerAction::SetPlayersVisible { visible: false },
            progress_bar(player_uuid, &session),
            PlayerAction::Teleport {
                dimension: session.dimension.clone(),
                x: session.origin_x as f64 + f64::from(session.width) / 2.0,
                y: session.origin_y as f64 + 1.0,
                z: session.origin_z as f64 - 1.5,
                yaw: Some(0.0),
                pitch: Some(35.0),
            },
        ],
    }
}

fn reveal_command(config: &Config, player_uuid: &str, args: Vec<&str>) -> PluginCommandResponse {
    let Some(mut session) = active_session(player_uuid) else {
        return PluginCommandResponse {
            handled: true,
            actions: vec![message("还没有进行中的扫雷局，使用 /minesweeper new 开始")],
        };
    };
    let Some((x, z)) = parse_cell_args(&session, &args) else {
        return PluginCommandResponse {
            handled: true,
            actions: vec![message(format!(
                "坐标范围: x=1..{} z=1..{}",
                session.width, session.height
            ))],
        };
    };

    let outcome = reveal_cell(config, player_uuid, &mut session, x, z);
    let _ = storage_set_typed(&session_key(player_uuid), &session);
    PluginCommandResponse {
        handled: true,
        actions: reveal_actions(config, player_uuid, &session, outcome, true),
    }
}

fn flag_command(config: &Config, player_uuid: &str, args: Vec<&str>) -> PluginCommandResponse {
    let Some(mut session) = active_session(player_uuid) else {
        return PluginCommandResponse {
            handled: true,
            actions: vec![message("还没有进行中的扫雷局，使用 /minesweeper new 开始")],
        };
    };
    let Some((x, z)) = parse_cell_args(&session, &args) else {
        return PluginCommandResponse {
            handled: true,
            actions: vec![message(format!(
                "坐标范围: x=1..{} z=1..{}",
                session.width, session.height
            ))],
        };
    };
    toggle_flag_cell(config, player_uuid, &mut session, x, z, true)
}

fn toggle_flag_cell(
    config: &Config,
    player_uuid: &str,
    session: &mut GameSession,
    x: usize,
    z: usize,
    verbose: bool,
) -> PluginCommandResponse {
    let index = session.index(x, z);
    if session.cells[index].revealed {
        return PluginCommandResponse {
            handled: true,
            actions: vec![message("这个格子已经翻开，不能标记")],
        };
    }

    session.cells[index].flagged = !session.cells[index].flagged;
    render_cell(config, session, x, z);
    let flagged = session.flagged_count();
    let _ = storage_set_typed(&session_key(player_uuid), session);
    let mut actions = Vec::new();
    if verbose {
        actions.push(message(format!(
            "已{} ({}, {})，当前标记 {}/{}",
            if session.cells[index].flagged {
                "标记"
            } else {
                "取消标记"
            },
            x + 1,
            z + 1,
            flagged,
            session.mines
        )));
    }
    actions.push(progress_bar(player_uuid, session));
    PluginCommandResponse {
        handled: true,
        actions,
    }
}

fn status_command(player_uuid: &str) -> PluginCommandResponse {
    let Some(session) = active_session(player_uuid) else {
        return PluginCommandResponse {
            handled: true,
            actions: vec![message("还没有进行中的扫雷局，使用 /minesweeper new 开始")],
        };
    };
    PluginCommandResponse {
        handled: true,
        actions: vec![
            message(format!(
                "扫雷进度: 已翻开 {}/{}，标记 {}/{}，用时 {} 秒",
                session.revealed_safe_count(),
                session.safe_total(),
                session.flagged_count(),
                session.mines,
                elapsed_seconds(session.started_at_ms)
            )),
            progress_bar(player_uuid, &session),
        ],
    }
}

fn leave_to_lobby(config: &Config, player_uuid: &str) -> PluginCommandResponse {
    let _ = storage_delete(&session_key(player_uuid));
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
    PluginCommandResponse {
        handled: true,
        actions,
    }
}

fn reveal_cell(
    config: &Config,
    player_uuid: &str,
    session: &mut GameSession,
    x: usize,
    z: usize,
) -> RevealOutcome {
    if !session.active {
        return RevealOutcome::AlreadyEnded;
    }
    let index = session.index(x, z);
    if session.cells[index].revealed {
        return RevealOutcome::AlreadyRevealed;
    }
    if session.cells[index].flagged {
        return RevealOutcome::Flagged;
    }
    if !session.mines_placed {
        place_mines_with_seed(session, seed_for(player_uuid, x, z), x, z);
    }
    if session.cells[index].mine {
        session.cells[index].revealed = true;
        finish_session(session, "lost");
        reveal_all_mines(session);
        render_board(config, session);
        return RevealOutcome::Lost;
    }

    let revealed = flood_reveal(session, x, z);
    render_board(config, session);
    if session.has_won() {
        let elapsed_ms = finish_session(session, "won");
        record_best_time(player_uuid, session, elapsed_ms);
        flag_all_mines(session);
        render_board(config, session);
        return RevealOutcome::Won;
    }
    RevealOutcome::Revealed(revealed)
}

fn reveal_actions(
    config: &Config,
    player_uuid: &str,
    session: &GameSession,
    outcome: RevealOutcome,
    verbose: bool,
) -> Vec<PlayerAction> {
    match outcome {
        RevealOutcome::Revealed(count) => {
            let mut actions = Vec::new();
            if verbose || count > 1 {
                actions.push(message(format!(
                    "翻开 {} 个格子，剩余安全格 {}",
                    count,
                    session
                        .safe_total()
                        .saturating_sub(session.revealed_safe_count())
                )));
            }
            actions.push(progress_bar(player_uuid, session));
            actions
        }
        RevealOutcome::Won => vec![
            message(format!(
                "扫雷成功，用时 {} 秒。使用 /minesweeper new 再开一局，或 /minesweeper leave 返回大厅",
                elapsed_seconds(session.started_at_ms)
            )),
            PlayerAction::RemoveBossBar {
                id: boss_bar_id(player_uuid),
            },
            PlayerAction::OpenMenu {
                menu: config.menu_id.clone(),
            },
        ],
        RevealOutcome::Lost => vec![
            message(format!(
                "踩雷了，本局结束。使用 /minesweeper new 重开，或 /minesweeper leave 返回大厅"
            )),
            PlayerAction::RemoveBossBar {
                id: boss_bar_id(player_uuid),
            },
            PlayerAction::OpenMenu {
                menu: config.menu_id.clone(),
            },
        ],
        RevealOutcome::Flagged => vec![message("这个格子已标记，先取消标记再翻开")],
        RevealOutcome::AlreadyRevealed => vec![message("这个格子已经翻开")],
        RevealOutcome::AlreadyEnded => {
            vec![message("本局已经结束，使用 /minesweeper new 开始新局")]
        }
    }
}

fn place_mines_with_seed(session: &mut GameSession, seed: u64, safe_x: usize, safe_z: usize) {
    let mine_goal = usize::from(session.mines).min(session.cell_count().saturating_sub(1));
    let mut candidates = mine_candidates(session, safe_x, safe_z, true);
    if candidates.len() < mine_goal {
        candidates = mine_candidates(session, safe_x, safe_z, false);
    }

    let mut rng = XorShift64::new(seed);
    let mut placed = 0usize;
    while placed < mine_goal && !candidates.is_empty() {
        let index = rng.next_index(candidates.len());
        let cell_index = candidates.swap_remove(index);
        if !session.cells[cell_index].mine {
            session.cells[cell_index].mine = true;
            placed += 1;
        }
    }
    session.mines = placed as u16;
    update_adjacent_counts(session);
    session.mines_placed = true;
}

fn mine_candidates(
    session: &GameSession,
    safe_x: usize,
    safe_z: usize,
    protect_neighbors: bool,
) -> Vec<usize> {
    let mut candidates = Vec::new();
    for z in 0..usize::from(session.height) {
        for x in 0..usize::from(session.width) {
            let protected = if protect_neighbors {
                x.abs_diff(safe_x) <= 1 && z.abs_diff(safe_z) <= 1
            } else {
                x == safe_x && z == safe_z
            };
            if !protected {
                candidates.push(session.index(x, z));
            }
        }
    }
    candidates
}

fn update_adjacent_counts(session: &mut GameSession) {
    for index in 0..session.cells.len() {
        session.cells[index].adjacent = 0;
    }
    for z in 0..usize::from(session.height) {
        for x in 0..usize::from(session.width) {
            let index = session.index(x, z);
            if !session.cells[index].mine {
                continue;
            }
            for neighbor in neighbor_indices(session, x, z) {
                session.cells[neighbor].adjacent =
                    session.cells[neighbor].adjacent.saturating_add(1);
            }
        }
    }
}

fn flood_reveal(session: &mut GameSession, x: usize, z: usize) -> usize {
    let mut stack = vec![(x, z)];
    let mut revealed = 0usize;
    while let Some((cx, cz)) = stack.pop() {
        let index = session.index(cx, cz);
        let cell = &mut session.cells[index];
        if cell.revealed || cell.flagged || cell.mine {
            continue;
        }
        cell.revealed = true;
        revealed += 1;
        if cell.adjacent != 0 {
            continue;
        }
        for neighbor in neighbor_coordinates(session, cx, cz) {
            stack.push(neighbor);
        }
    }
    revealed
}

fn neighbor_coordinates(session: &GameSession, x: usize, z: usize) -> Vec<(usize, usize)> {
    let width = usize::from(session.width);
    let height = usize::from(session.height);
    let mut neighbors = Vec::with_capacity(8);
    for dz in -1isize..=1 {
        for dx in -1isize..=1 {
            if dx == 0 && dz == 0 {
                continue;
            }
            let nx = x as isize + dx;
            let nz = z as isize + dz;
            if nx >= 0 && nz >= 0 && (nx as usize) < width && (nz as usize) < height {
                neighbors.push((nx as usize, nz as usize));
            }
        }
    }
    neighbors
}

fn neighbor_indices(session: &GameSession, x: usize, z: usize) -> Vec<usize> {
    neighbor_coordinates(session, x, z)
        .into_iter()
        .map(|(x, z)| session.index(x, z))
        .collect()
}

fn reveal_all_mines(session: &mut GameSession) {
    for cell in &mut session.cells {
        if cell.mine {
            cell.revealed = true;
        }
    }
}

fn flag_all_mines(session: &mut GameSession) {
    for cell in &mut session.cells {
        if cell.mine {
            cell.flagged = true;
        }
    }
}

fn build_arena(config: &Config, session: &GameSession) -> bool {
    let width = i32::from(session.width);
    let height = i32::from(session.height);
    let min_x = session.origin_x - 1;
    let max_x = session.origin_x + width;
    let min_z = session.origin_z - 2;
    let max_z = session.origin_z + height;
    let y = session.origin_y;
    let dimension = session.dimension.as_str();
    let mut blocks = Vec::new();

    for x in min_x..=max_x {
        for z in min_z..=max_z {
            blocks.push((dimension, (x, y - 1, z), config.board.base_block.as_str()));
            for air_y in y + 1..=y + 5 {
                blocks.push((dimension, (x, air_y, z), "minecraft:air"));
            }
            let block = if x >= session.origin_x
                && x < session.origin_x + width
                && z >= session.origin_z
                && z < session.origin_z + height
            {
                config.board.hidden_block.as_str()
            } else if z == session.origin_z - 2 {
                config.board.floor_block.as_str()
            } else {
                config.board.border_block.as_str()
            };
            blocks.push((dimension, (x, y, z), block));
        }
    }

    world_set_blocks(blocks)
}

fn render_board(config: &Config, session: &GameSession) -> bool {
    let mut blocks = Vec::with_capacity(session.cells.len());
    for z in 0..usize::from(session.height) {
        for x in 0..usize::from(session.width) {
            let index = session.index(x, z);
            blocks.push((
                session.dimension.as_str(),
                cell_position(session, x, z),
                block_for_cell(config, &session.cells[index]),
            ));
        }
    }
    world_set_blocks(blocks)
}

fn render_cell(config: &Config, session: &GameSession, x: usize, z: usize) -> bool {
    let index = session.index(x, z);
    world_set_block(
        &session.dimension,
        cell_position(session, x, z),
        block_for_cell(config, &session.cells[index]),
    )
}

fn block_for_cell<'a>(config: &'a Config, cell: &CellState) -> &'a str {
    if !cell.revealed {
        return if cell.flagged {
            config.board.flag_block.as_str()
        } else {
            config.board.hidden_block.as_str()
        };
    }
    if cell.mine {
        return config.board.mine_block.as_str();
    }
    if cell.adjacent == 0 {
        return config.board.empty_block.as_str();
    }
    config
        .board
        .number_blocks
        .get(usize::from(cell.adjacent - 1))
        .map(String::as_str)
        .unwrap_or(DEFAULT_NUMBER_BLOCKS[usize::from(cell.adjacent - 1)])
}

fn register_lobby(config: &Config) {
    let center_x = config.lobby.spawn_x.floor() as i32;
    let center_z = config.lobby.spawn_z.floor() as i32;
    let y = config.lobby.floor_y;
    let _ = world_register_edit_region(&WorldEditRegion {
        id: "minesweeper_lobby",
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
    for dx in -6..=6 {
        for dz in -6..=6 {
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

fn register_arena(
    config: &Config,
    player_uuid: &str,
    origin: (i32, i32, i32),
    settings: BoardSettings,
) {
    let id = format!("minesweeper_{}", player_uuid.replace('-', ""));
    let _ = world_register_edit_region(&WorldEditRegion {
        id: &id,
        dimension: &config.board.dimension,
        min: (origin.0 - 2, origin.1 - 2, origin.2 - 3),
        max: (
            origin.0 + settings.width as i32 + 1,
            origin.1 + 6,
            origin.2 + settings.height as i32 + 1,
        ),
        allow_player_break: true,
        allow_player_place: false,
        allow_plugin_write: true,
        runtime_only: true,
    });
}

fn instance_origin(config: &Config, player_uuid: &str) -> (i32, i32, i32) {
    let hash = fnv1a64(player_uuid.as_bytes());
    let grid_x = (hash % INSTANCE_GRID_WIDTH) as i32;
    let grid_z = ((hash / INSTANCE_GRID_WIDTH) % INSTANCE_GRID_WIDTH) as i32;
    (
        config.board.origin_x + grid_x * config.board.instance_spacing,
        config.board.origin_y,
        config.board.origin_z + grid_z * config.board.instance_spacing,
    )
}

fn active_session(player_uuid: &str) -> Option<GameSession> {
    storage_get_typed::<GameSession>(&session_key(player_uuid)).filter(|session| session.active)
}

fn stored_session(player_uuid: &str) -> Option<GameSession> {
    storage_get_typed::<GameSession>(&session_key(player_uuid))
}

fn session_key(player_uuid: &str) -> String {
    format!("{SESSION_PREFIX}{player_uuid}")
}

fn best_key(player_uuid: &str, difficulty: &str) -> String {
    format!("{BEST_PREFIX}{player_uuid}/{}", difficulty.trim())
}

fn pending_menu_key(player_uuid: &str) -> String {
    format!("{PENDING_MENU_PREFIX}{player_uuid}")
}

fn boss_bar_id(player_uuid: &str) -> String {
    format!("{BOSS_BAR_PREFIX}{player_uuid}")
}

fn custom_difficulty_id(settings: BoardSettings) -> String {
    format!(
        "custom_{}x{}_{}",
        settings.width, settings.height, settings.mines
    )
}

fn session_difficulty_id(session: &GameSession) -> String {
    let difficulty = session.difficulty.trim();
    if !difficulty.is_empty() {
        return difficulty.to_string();
    }
    format!(
        "custom_{}x{}_{}",
        session.width, session.height, session.mines
    )
}

fn finish_session(session: &mut GameSession, result: &str) -> i64 {
    let elapsed = elapsed_millis(session.started_at_ms);
    session.active = false;
    session.result = result.to_string();
    session.ended_elapsed_ms = elapsed;
    elapsed
}

fn record_best_time(player_uuid: &str, session: &GameSession, elapsed_ms: i64) {
    let difficulty = session_difficulty_id(session);
    let key = best_key(player_uuid, &difficulty);
    let should_update = storage_get_typed::<i64>(&key)
        .map(|best| elapsed_ms < best)
        .unwrap_or(true);
    if should_update {
        let _ = storage_set_typed(&key, &elapsed_ms);
    }
}

fn best_time_for_session(player_uuid: &str, session: &GameSession) -> Option<i64> {
    best_time_ms(player_uuid, &session_difficulty_id(session))
}

fn best_time_ms(player_uuid: &str, difficulty: &str) -> Option<i64> {
    let difficulty = difficulty.trim();
    if difficulty.is_empty() {
        return None;
    }
    storage_get_typed::<i64>(&best_key(player_uuid, difficulty))
}

fn best_configured_time_ms(config: &Config, player_uuid: &str) -> Option<i64> {
    config
        .difficulties
        .iter()
        .filter_map(|difficulty| best_time_ms(player_uuid, &difficulty.id))
        .min()
}

fn minesweeper_replacements(
    session: Option<&GameSession>,
    best_ms: Option<i64>,
) -> Vec<PlaceholderReplacement> {
    vec![
        placeholder("minesweeper_state", session_state_label(session)),
        placeholder("minesweeper_difficulty", session_difficulty_label(session)),
        placeholder("minesweeper_size", session_size_label(session)),
        placeholder("minesweeper_mines", session_mines_label(session)),
        placeholder("minesweeper_revealed", session_revealed_label(session)),
        placeholder("minesweeper_flags", session_flags_label(session)),
        placeholder("minesweeper_time", session_time_label(session)),
        placeholder(
            "minesweeper_best_time",
            best_ms
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

fn session_state_label(session: Option<&GameSession>) -> String {
    let Some(session) = session else {
        return "大厅".to_string();
    };
    if session.active {
        return "进行中".to_string();
    }
    match session.result.as_str() {
        "won" => "胜利".to_string(),
        "lost" => "失败".to_string(),
        _ => "已结束".to_string(),
    }
}

fn session_difficulty_label(session: Option<&GameSession>) -> String {
    session
        .and_then(|session| {
            let label = session.difficulty_label.trim();
            (!label.is_empty()).then(|| label.to_string())
        })
        .unwrap_or_else(|| "大厅".to_string())
}

fn session_size_label(session: Option<&GameSession>) -> String {
    session
        .map(|session| format!("{}x{}", session.width, session.height))
        .unwrap_or_else(|| "-".to_string())
}

fn session_mines_label(session: Option<&GameSession>) -> String {
    session
        .map(|session| session.mines.to_string())
        .unwrap_or_else(|| "-".to_string())
}

fn session_revealed_label(session: Option<&GameSession>) -> String {
    session
        .map(|session| format!("{}/{}", session.revealed_safe_count(), session.safe_total()))
        .unwrap_or_else(|| "-".to_string())
}

fn session_flags_label(session: Option<&GameSession>) -> String {
    session
        .map(|session| format!("{}/{}", session.flagged_count(), session.mines))
        .unwrap_or_else(|| "-".to_string())
}

fn session_time_label(session: Option<&GameSession>) -> String {
    session
        .map(|session| format_duration_ms(session_elapsed_ms(session)))
        .unwrap_or_else(|| "--:--".to_string())
}

fn session_elapsed_ms(session: &GameSession) -> i64 {
    if session.active {
        elapsed_millis(session.started_at_ms)
    } else {
        session.ended_elapsed_ms.max(0)
    }
}

fn parse_cell_args(session: &GameSession, args: &[&str]) -> Option<(usize, usize)> {
    if args.len() != 2 {
        return None;
    }
    let x = args[0].parse::<usize>().ok()?;
    let z = args[1].parse::<usize>().ok()?;
    if x == 0 || z == 0 || x > usize::from(session.width) || z > usize::from(session.height) {
        return None;
    }
    Some((x - 1, z - 1))
}

fn cell_from_position(session: &GameSession, x: i32, y: i32, z: i32) -> Option<(usize, usize)> {
    if y != session.origin_y {
        return None;
    }
    let local_x = x.checked_sub(session.origin_x)?;
    let local_z = z.checked_sub(session.origin_z)?;
    if local_x < 0
        || local_z < 0
        || local_x >= i32::from(session.width)
        || local_z >= i32::from(session.height)
    {
        return None;
    }
    Some((local_x as usize, local_z as usize))
}

fn cell_position(session: &GameSession, x: usize, z: usize) -> (i32, i32, i32) {
    (
        session.origin_x + x as i32,
        session.origin_y,
        session.origin_z + z as i32,
    )
}

fn progress_bar(player_uuid: &str, session: &GameSession) -> PlayerAction {
    let safe_total = session.safe_total().max(1);
    let progress = session.revealed_safe_count() as f32 / safe_total as f32;
    PlayerAction::BossBar {
        id: boss_bar_id(player_uuid),
        title: format!(
            "扫雷 {}/{} | 标记 {}/{}",
            session.revealed_safe_count(),
            session.safe_total(),
            session.flagged_count(),
            session.mines
        ),
        progress,
        color: "green".to_string(),
        overlay: "progress".to_string(),
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

fn elapsed_seconds(started_at_ms: i64) -> i64 {
    elapsed_millis(started_at_ms) / 1000
}

fn elapsed_millis(started_at_ms: i64) -> i64 {
    (time_millis() - started_at_ms).max(0)
}

fn format_duration_ms(ms: i64) -> String {
    let total_seconds = (ms.max(0) / 1000) as u64;
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes:02}:{seconds:02}")
    }
}

fn seed_for(player_uuid: &str, safe_x: usize, safe_z: usize) -> u64 {
    let base = fnv1a64(player_uuid.as_bytes());
    base ^ (time_millis() as u64).rotate_left(17) ^ ((safe_x as u64) << 32) ^ safe_z as u64
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

#[derive(Debug, Clone, Copy)]
struct XorShift64 {
    state: u64,
}

impl XorShift64 {
    fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 { 0x9e3779b97f4a7c15 } else { seed },
        }
    }

    fn next(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }

    fn next_index(&mut self, len: usize) -> usize {
        (self.next() as usize) % len
    }
}

#[derive(Debug, Clone, Copy)]
enum RevealOutcome {
    Revealed(usize),
    Won,
    Lost,
    Flagged,
    AlreadyRevealed,
    AlreadyEnded,
}

#[derive(Debug, Clone, Copy)]
struct BoardSettings {
    width: usize,
    height: usize,
    mines: usize,
}

impl BoardSettings {
    fn from_config(config: &BoardConfig) -> Self {
        Self::from_values(config.width, config.height, config.mines)
    }

    fn from_values(width: usize, height: usize, mines: usize) -> Self {
        let width = width.clamp(4, MAX_BOARD_SIDE);
        let height = height.clamp(4, MAX_BOARD_SIDE);
        let max_mines = width.saturating_mul(height).saturating_sub(1).max(1);
        Self {
            width,
            height,
            mines: mines.clamp(1, max_mines),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GameSession {
    active: bool,
    #[serde(default)]
    difficulty: String,
    #[serde(default)]
    difficulty_label: String,
    #[serde(default)]
    result: String,
    dimension: String,
    origin_x: i32,
    origin_y: i32,
    origin_z: i32,
    width: u8,
    height: u8,
    mines: u16,
    mines_placed: bool,
    started_at_ms: i64,
    #[serde(default)]
    ended_elapsed_ms: i64,
    cells: Vec<CellState>,
}

impl GameSession {
    fn new(
        dimension: &str,
        origin: (i32, i32, i32),
        settings: BoardSettings,
        difficulty: &str,
        difficulty_label: &str,
    ) -> Self {
        let cell_count = settings.width.saturating_mul(settings.height);
        Self {
            active: true,
            difficulty: difficulty.to_string(),
            difficulty_label: difficulty_label.to_string(),
            result: "playing".to_string(),
            dimension: dimension.to_string(),
            origin_x: origin.0,
            origin_y: origin.1,
            origin_z: origin.2,
            width: settings.width as u8,
            height: settings.height as u8,
            mines: settings.mines as u16,
            mines_placed: false,
            started_at_ms: time_millis(),
            ended_elapsed_ms: 0,
            cells: vec![CellState::default(); cell_count],
        }
    }

    fn index(&self, x: usize, z: usize) -> usize {
        z * usize::from(self.width) + x
    }

    fn cell_count(&self) -> usize {
        usize::from(self.width) * usize::from(self.height)
    }

    fn safe_total(&self) -> usize {
        self.cell_count().saturating_sub(usize::from(self.mines))
    }

    fn revealed_safe_count(&self) -> usize {
        self.cells
            .iter()
            .filter(|cell| cell.revealed && !cell.mine)
            .count()
    }

    fn flagged_count(&self) -> usize {
        self.cells.iter().filter(|cell| cell.flagged).count()
    }

    fn has_won(&self) -> bool {
        self.cells
            .iter()
            .filter(|cell| !cell.mine)
            .all(|cell| cell.revealed)
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
struct CellState {
    mine: bool,
    revealed: bool,
    flagged: bool,
    adjacent: u8,
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
    #[serde(default = "default_default_difficulty")]
    default_difficulty: String,
    #[serde(default = "default_difficulties")]
    difficulties: Vec<DifficultyConfig>,
    #[serde(default)]
    controls: ControlsConfig,
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
            default_difficulty: default_default_difficulty(),
            difficulties: default_difficulties(),
            controls: ControlsConfig::default(),
            lobby: LobbyConfig::default(),
            board: BoardConfig::default(),
        }
    }
}

impl Config {
    fn difficulty(&self, id: &str) -> Option<DifficultyConfig> {
        self.difficulties
            .iter()
            .find(|difficulty| difficulty.id == id)
            .cloned()
    }
}

#[derive(Debug, Clone, Deserialize)]
struct ControlsConfig {
    #[serde(default = "default_true")]
    mark_with_main_hand: bool,
    #[serde(default = "default_true")]
    mark_with_off_hand: bool,
}

impl Default for ControlsConfig {
    fn default() -> Self {
        Self {
            mark_with_main_hand: false,
            mark_with_off_hand: false,
        }
    }
}

impl ControlsConfig {
    fn hand_can_mark(&self, hand: &str) -> bool {
        match hand {
            "main_hand" => self.mark_with_main_hand,
            "off_hand" => self.mark_with_off_hand,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct DifficultyConfig {
    #[serde(default)]
    id: String,
    #[serde(default)]
    label: String,
    #[serde(default = "default_board_width")]
    width: usize,
    #[serde(default = "default_board_height")]
    height: usize,
    #[serde(default = "default_mines")]
    mines: usize,
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
    #[serde(default = "default_board_width")]
    width: usize,
    #[serde(default = "default_board_height")]
    height: usize,
    #[serde(default = "default_mines")]
    mines: usize,
    #[serde(default = "default_floor_block")]
    floor_block: String,
    #[serde(default = "default_base_block")]
    base_block: String,
    #[serde(default = "default_border_block")]
    border_block: String,
    #[serde(default = "default_hidden_block")]
    hidden_block: String,
    #[serde(default = "default_flag_block")]
    flag_block: String,
    #[serde(default = "default_mine_block")]
    mine_block: String,
    #[serde(default = "default_empty_block")]
    empty_block: String,
    #[serde(default = "default_number_blocks")]
    number_blocks: Vec<String>,
}

impl Default for BoardConfig {
    fn default() -> Self {
        Self {
            dimension: default_board_dimension(),
            origin_x: default_board_origin_x(),
            origin_y: default_floor_y(),
            origin_z: default_board_origin_z(),
            instance_spacing: default_instance_spacing(),
            width: default_board_width(),
            height: default_board_height(),
            mines: default_mines(),
            floor_block: default_floor_block(),
            base_block: default_base_block(),
            border_block: default_border_block(),
            hidden_block: default_hidden_block(),
            flag_block: default_flag_block(),
            mine_block: default_mine_block(),
            empty_block: default_empty_block(),
            number_blocks: default_number_blocks(),
        }
    }
}

fn load_config() -> Config {
    config_read_to_string(CONFIG_PATH)
        .and_then(|contents| toml::from_str::<Config>(&contents).ok())
        .unwrap_or_default()
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
    "minesweeper_select".to_string()
}

fn default_default_difficulty() -> String {
    "beginner".to_string()
}

fn default_difficulties() -> Vec<DifficultyConfig> {
    vec![
        DifficultyConfig {
            id: "beginner".to_string(),
            label: "初级 9x9 / 10 雷".to_string(),
            width: 9,
            height: 9,
            mines: 10,
        },
        DifficultyConfig {
            id: "intermediate".to_string(),
            label: "中级 16x16 / 40 雷".to_string(),
            width: 16,
            height: 16,
            mines: 40,
        },
        DifficultyConfig {
            id: "expert".to_string(),
            label: "高级 30x16 / 99 雷".to_string(),
            width: 30,
            height: 16,
            mines: 99,
        },
    ]
}

fn default_lobby_dimension() -> String {
    "qexed:minesweeper_lobby".to_string()
}

fn default_board_dimension() -> String {
    "qexed:minesweeper_game".to_string()
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
    "{\"text\":\"扫雷入口\",\"color\":\"gold\"}".to_string()
}

fn default_board_origin_x() -> i32 {
    2048
}

fn default_board_origin_z() -> i32 {
    2048
}

fn default_instance_spacing() -> i32 {
    96
}

fn default_board_width() -> usize {
    9
}

fn default_board_height() -> usize {
    9
}

fn default_mines() -> usize {
    10
}

fn default_floor_block() -> String {
    "minecraft:smooth_stone".to_string()
}

fn default_base_block() -> String {
    "minecraft:polished_andesite".to_string()
}

fn default_border_block() -> String {
    "minecraft:deepslate_tiles".to_string()
}

fn default_hidden_block() -> String {
    "minecraft:light_gray_concrete".to_string()
}

fn default_flag_block() -> String {
    "minecraft:red_concrete".to_string()
}

fn default_mine_block() -> String {
    "minecraft:tnt".to_string()
}

fn default_empty_block() -> String {
    "minecraft:white_concrete".to_string()
}

fn default_number_blocks() -> Vec<String> {
    DEFAULT_NUMBER_BLOCKS
        .iter()
        .map(|block| (*block).to_string())
        .collect()
}

const DEFAULT_CONFIG: &str = r#"enable = true
server_name = "浅屿闲游"
qq_group = "722632621"
lobby_server = "lobby_1"
menu_id = "minesweeper_select"
default_difficulty = "beginner"

[controls]
mark_with_main_hand = false
mark_with_off_hand = false

[[difficulties]]
id = "beginner"
label = "初级 9x9 / 10 雷"
width = 9
height = 9
mines = 10

[[difficulties]]
id = "intermediate"
label = "中级 16x16 / 40 雷"
width = 16
height = 16
mines = 40

[[difficulties]]
id = "expert"
label = "高级 30x16 / 99 雷"
width = 30
height = 16
mines = 99

[lobby]
dimension = "qexed:minesweeper_lobby"
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
npc_display_name = "{\"text\":\"扫雷入口\",\"color\":\"gold\"}"

[board]
dimension = "qexed:minesweeper_game"
origin_x = 2048
origin_y = -52
origin_z = 2048
instance_spacing = 96
width = 9
height = 9
mines = 10
floor_block = "minecraft:smooth_stone"
base_block = "minecraft:polished_andesite"
border_block = "minecraft:deepslate_tiles"
hidden_block = "minecraft:light_gray_concrete"
flag_block = "minecraft:red_concrete"
mine_block = "minecraft:tnt"
empty_block = "minecraft:white_concrete"
number_blocks = [
    "minecraft:blue_concrete",
    "minecraft:green_concrete",
    "minecraft:red_concrete",
    "minecraft:purple_concrete",
    "minecraft:orange_concrete",
    "minecraft:cyan_concrete",
    "minecraft:brown_concrete",
    "minecraft:black_concrete",
]
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_reveal_places_no_mine_near_start_when_possible() {
        let settings = BoardSettings {
            width: 9,
            height: 9,
            mines: 10,
        };
        let mut session = GameSession {
            active: true,
            difficulty: "beginner".to_string(),
            difficulty_label: "初级".to_string(),
            result: "playing".to_string(),
            dimension: "qexed:test".to_string(),
            origin_x: 0,
            origin_y: 64,
            origin_z: 0,
            width: settings.width as u8,
            height: settings.height as u8,
            mines: settings.mines as u16,
            mines_placed: false,
            started_at_ms: 0,
            ended_elapsed_ms: 0,
            cells: vec![CellState::default(); settings.width * settings.height],
        };

        place_mines_with_seed(&mut session, 42, 4, 4);

        for z in 3..=5 {
            for x in 3..=5 {
                assert!(!session.cells[session.index(x, z)].mine);
            }
        }
        assert_eq!(session.cells.iter().filter(|cell| cell.mine).count(), 10);
    }

    #[test]
    fn flood_reveal_opens_empty_area() {
        let mut session = GameSession {
            active: true,
            difficulty: "beginner".to_string(),
            difficulty_label: "初级".to_string(),
            result: "playing".to_string(),
            dimension: "qexed:test".to_string(),
            origin_x: 0,
            origin_y: 64,
            origin_z: 0,
            width: 4,
            height: 4,
            mines: 1,
            mines_placed: true,
            started_at_ms: 0,
            ended_elapsed_ms: 0,
            cells: vec![CellState::default(); 16],
        };
        let mine = session.index(3, 3);
        session.cells[mine].mine = true;
        update_adjacent_counts(&mut session);

        let revealed = flood_reveal(&mut session, 0, 0);

        assert!(revealed > 1);
        assert!(!session.cells[mine].revealed);
    }
}

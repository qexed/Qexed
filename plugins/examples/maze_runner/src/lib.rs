use qexed_plugin_sdk::{
    ConfigReloadPayload, NpcInteractPayload, NpcMutationOp, NpcMutationResponse, NpcUpsert,
    PlaceholderQuery, PlaceholderReplacement, PlaceholderResponse, PlayerAction, PlayerMovePayload,
    PlayerPayload, PlayerTickPayload, PluginCommandDefinition, PluginCommandQuery,
    PluginCommandResponse, WorldEditRegion, config_load_or_create, config_read_to_string,
    storage_delete, storage_get, storage_get_typed, storage_set, storage_set_typed, time_millis,
    world_register_edit_region, world_set_blocks,
};
use serde::{Deserialize, Serialize};

qexed_plugin_sdk::qexed_plugin_memory!();

const CONFIG_PATH: &str = "config.toml";
const COMMAND_NAME: &str = "maze";
const NPC_KEY: &str = "maze_runner:selector";
const NPC_EVENT: &str = "maze_select";
const STARTED_PREFIX: &str = "maze_started/";
const BEST_PREFIX: &str = "maze_best/";
const INSTANCE_PREFIX: &str = "maze_instance/";
const BUILD_PREFIX: &str = "maze_build/";
const CELL_SPACING: i32 = 8;
const CELL_MARGIN: i32 = 4;
const CORRIDOR_RADIUS: i32 = 1;
const PLAYER_JUMP_AIR_HEIGHT: i32 = 3;
const WORLD_EDIT_BATCH_LIMIT: usize = 4096;
const MAZE_LOADING_BAR_ID: &str = "maze_runner:loading";
const BUILD_BLOCKS_PER_TICK: usize = 1024;
const BUILD_BLOCKS_PER_STORAGE_CHUNK: usize = 4096;
const MAX_STALE_BUILD_STORAGE_CHUNKS: usize = 256;
const MAZE_GENERATION_ATTEMPTS: usize = 8;

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_priority() -> i32 {
    280
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_init() {
    let _ = config_load_or_create(CONFIG_PATH, DEFAULT_CONFIG);
    let config = load_config();
    register_lobby_region(&config);
    qexed_plugin_sdk::log("maze_runner initialized");
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_config_reload(ptr: i32, len: i32) {
    let Some(payload) =
        (unsafe { qexed_plugin_sdk::decode_payload::<ConfigReloadPayload>(ptr, len) })
    else {
        return;
    };
    let path = payload.path.replace('\\', "/");
    if path.ends_with("maze_runner/config.toml") || path.ends_with(CONFIG_PATH) {
        let config = load_config();
        register_lobby_region(&config);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_player_join(ptr: i32, len: i32) {
    let Some(payload) = (unsafe { qexed_plugin_sdk::decode_payload::<PlayerPayload>(ptr, len) })
    else {
        return;
    };
    let config = load_config();
    if config.enable && payload.dimension == config.lobby.dimension {
        clear_build_storage(&payload.uuid);
        let _ = storage_set_typed(
            &instance_key(&payload.uuid),
            &MazeSession::lobby(&config.lobby.dimension),
        );
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_commands(_ptr: i32, _len: i32) -> i64 {
    qexed_plugin_sdk::response_ptr_len(&PluginCommandDefinition {
        name: COMMAND_NAME.to_string(),
        description_key: "qexed.plugin.maze.command.description".to_string(),
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
    let argument = payload.argument.trim();
    let response = match argument {
        "" | "menu" => PluginCommandResponse {
            handled: true,
            actions: vec![PlayerAction::OpenMenu {
                menu: config.menu_id.clone(),
            }],
        },
        "leave" | "lobby" | "spawn" => leave_to_lobby(&config, &payload.player.uuid),
        _ => {
            let mut parts = argument.split_whitespace();
            match parts.next() {
                Some("start") => {
                    let difficulty_id = parts.next().unwrap_or_default();
                    start_maze_loading(&config, &payload.player.uuid, difficulty_id)
                }
                _ => PluginCommandResponse {
                    handled: true,
                    actions: vec![message(
                        "用法: /maze menu | /maze start <easy|normal|hard> | /maze leave",
                    )],
                },
            }
        }
    };
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
                name: "Maze".to_string(),
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
    qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse {
        handled: true,
        actions: vec![PlayerAction::OpenMenu {
            menu: config.menu_id.clone(),
        }],
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_player_move(ptr: i32, len: i32) -> i64 {
    let Some(payload) =
        (unsafe { qexed_plugin_sdk::decode_payload::<PlayerMovePayload>(ptr, len) })
    else {
        return qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse::default());
    };
    let config = load_config();
    let Some(session) = storage_get_typed::<MazeSession>(&instance_key(&payload.player.uuid))
    else {
        return qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse::default());
    };
    if !session.active || payload.dimension != session.dimension {
        return qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse::default());
    }

    let mut actions = Vec::new();
    if payload.position.y < session.min_y as f64 - 8.0 {
        actions.extend(respawn_actions(&session));
    } else if reached_finish(
        &session,
        payload.position.x,
        payload.position.y,
        payload.position.z,
    ) {
        let elapsed = elapsed_ms(session.started_at_ms);
        let best_key = best_key(&payload.player.uuid, &session.difficulty);
        let best = storage_get_typed::<i64>(&best_key);
        if best.is_none_or(|value| elapsed < value) {
            let _ = storage_set_typed(&best_key, &elapsed);
        }
        let _ = storage_set_typed(
            &instance_key(&payload.player.uuid),
            &MazeSession::lobby(&config.lobby.dimension),
        );
        actions.push(message(format!(
            "通关成功，难度 {}，用时 {}",
            session.difficulty_label,
            format_duration(elapsed)
        )));
        actions.push(PlayerAction::SetPlayersVisible { visible: true });
        actions.push(PlayerAction::Teleport {
            dimension: config.lobby.dimension,
            x: config.lobby.spawn_x,
            y: config.lobby.spawn_y,
            z: config.lobby.spawn_z,
            yaw: Some(config.lobby.spawn_yaw),
            pitch: Some(config.lobby.spawn_pitch),
        });
    }

    qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse {
        handled: !actions.is_empty(),
        actions,
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_player_tick(ptr: i32, len: i32) -> i64 {
    let Some(payload) =
        (unsafe { qexed_plugin_sdk::decode_payload::<PlayerTickPayload>(ptr, len) })
    else {
        return qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse::default());
    };
    let response = continue_maze_build(&payload.player.uuid);
    qexed_plugin_sdk::response_ptr_len(&response)
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
    let session = storage_get_typed::<MazeSession>(&instance_key(&player.uuid))
        .unwrap_or_else(|| MazeSession::lobby("minecraft:overworld"));
    let current = if session.active {
        format_duration(elapsed_ms(session.started_at_ms))
    } else {
        "--:--".to_string()
    };
    let best = if session.active {
        storage_get_typed::<i64>(&best_key(&player.uuid, &session.difficulty))
    } else {
        ["easy", "normal", "hard"]
            .into_iter()
            .filter_map(|difficulty| storage_get_typed::<i64>(&best_key(&player.uuid, difficulty)))
            .min()
    }
    .map(format_duration)
    .unwrap_or_else(|| "--:--".to_string());

    qexed_plugin_sdk::response_ptr_len(&PlaceholderResponse {
        replacements: vec![
            PlaceholderReplacement {
                key: "maze_difficulty".to_string(),
                value: if session.active {
                    session.difficulty_label
                } else {
                    "大厅".to_string()
                },
            },
            PlaceholderReplacement {
                key: "maze_current_time".to_string(),
                value: current,
            },
            PlaceholderReplacement {
                key: "maze_best_time".to_string(),
                value: best,
            },
        ],
    })
}

fn start_maze_loading(
    config: &Config,
    player_uuid: &str,
    difficulty_id: &str,
) -> PluginCommandResponse {
    if !config.enable {
        return PluginCommandResponse::default();
    }
    if storage_get(&build_key(player_uuid)).is_some() {
        return PluginCommandResponse {
            handled: true,
            actions: vec![
                loading_bar("迷宫正在生成", 0.05, "purple"),
                message("迷宫正在生成，请稍候..."),
            ],
        };
    }
    let Some(difficulty) = config.difficulty(difficulty_id).cloned() else {
        return PluginCommandResponse {
            handled: true,
            actions: vec![message("未知难度，可选 easy, normal, hard")],
        };
    };
    let instance = instance_origin(config, player_uuid, &difficulty);
    let maze = generate_maze(&difficulty, stable_seed(player_uuid, &difficulty.id));
    let session = MazeSession {
        active: true,
        difficulty: difficulty.id.clone(),
        difficulty_label: difficulty.label.clone(),
        dimension: difficulty.dimension.clone(),
        start_x: instance.0 + cell_center_offset(0),
        start_y: instance.1 + 1,
        start_z: instance.2 + cell_center_offset(0),
        finish_x: instance.0 + cell_center_offset(maze.finish_x),
        finish_y: instance.1 + maze.finish_level * difficulty.level_height + 1,
        finish_z: instance.2 + cell_center_offset(maze.finish_z),
        min_y: instance.1,
        started_at_ms: time_millis(),
    };
    register_instance_region(&difficulty, instance);
    let blocks = maze_blocks(&difficulty, instance, &maze);
    let build = MazeBuildJob {
        session,
        total: blocks.len(),
        written: 0,
        chunk_count: build_chunk_count(blocks.len()),
    };
    if !store_build_job(player_uuid, &build, &blocks) {
        clear_build_storage(player_uuid);
        return PluginCommandResponse {
            handled: true,
            actions: vec![
                PlayerAction::RemoveBossBar {
                    id: MAZE_LOADING_BAR_ID.to_string(),
                },
                message("Maze generation failed, retry."),
            ],
        };
    }

    PluginCommandResponse {
        handled: true,
        actions: vec![
            loading_bar(
                &format!("正在生成 {} 迷宫", difficulty.label),
                0.01,
                "purple",
            ),
            message(format!("正在生成 {} 迷宫，请稍候...", difficulty.label)),
        ],
    }
}

#[allow(dead_code)]
fn start_maze(config: &Config, player_uuid: &str, difficulty_id: &str) -> PluginCommandResponse {
    if !config.enable {
        return PluginCommandResponse::default();
    }
    let Some(difficulty) = config.difficulty(difficulty_id).cloned() else {
        return PluginCommandResponse {
            handled: true,
            actions: vec![message("未知难度，可选: easy, normal, hard")],
        };
    };
    let instance = instance_origin(config, player_uuid, &difficulty);
    let maze = generate_maze(&difficulty, stable_seed(player_uuid, &difficulty.id));
    let session = MazeSession {
        active: true,
        difficulty: difficulty.id.clone(),
        difficulty_label: difficulty.label.clone(),
        dimension: difficulty.dimension.clone(),
        start_x: instance.0 + cell_center_offset(0),
        start_y: instance.1 + 1,
        start_z: instance.2 + cell_center_offset(0),
        finish_x: instance.0 + cell_center_offset(maze.finish_x),
        finish_y: instance.1 + maze.finish_level * difficulty.level_height + 1,
        finish_z: instance.2 + cell_center_offset(maze.finish_z),
        min_y: instance.1,
        started_at_ms: time_millis(),
    };
    register_instance_region(&difficulty, instance);
    build_maze(&difficulty, instance, &maze);
    let _ = storage_set_typed(&instance_key(player_uuid), &session);
    let _ = storage_set_typed(
        &format!("{STARTED_PREFIX}{player_uuid}"),
        &session.started_at_ms,
    );

    PluginCommandResponse {
        handled: true,
        actions: vec![
            message(format!("进入 {} 迷宫", difficulty.label)),
            PlayerAction::SetPlayersVisible { visible: false },
            PlayerAction::Teleport {
                dimension: difficulty.dimension,
                x: session.start_x as f64 + 0.5,
                y: session.start_y as f64,
                z: session.start_z as f64 + 0.5,
                yaw: Some(0.0),
                pitch: Some(0.0),
            },
        ],
    }
}

fn leave_to_lobby(config: &Config, player_uuid: &str) -> PluginCommandResponse {
    clear_build_storage(player_uuid);
    let _ = storage_set_typed(
        &instance_key(player_uuid),
        &MazeSession::lobby(&config.lobby.dimension),
    );
    PluginCommandResponse {
        handled: true,
        actions: vec![
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

fn continue_maze_build(player_uuid: &str) -> PluginCommandResponse {
    let Some(bytes) = storage_get(&build_key(player_uuid)) else {
        return PluginCommandResponse::default();
    };
    let Some(mut build) = decode_build_job(&String::from_utf8_lossy(&bytes)) else {
        clear_build_storage(player_uuid);
        return PluginCommandResponse {
            handled: true,
            actions: vec![PlayerAction::RemoveBossBar {
                id: MAZE_LOADING_BAR_ID.to_string(),
            }],
        };
    };

    let start = build.written.min(build.total);
    let end = (start + BUILD_BLOCKS_PER_TICK).min(build.total);
    let Some(blocks) = load_build_blocks(player_uuid, &build, start, end) else {
        clear_build_storage(player_uuid);
        return PluginCommandResponse {
            handled: true,
            actions: vec![
                PlayerAction::RemoveBossBar {
                    id: MAZE_LOADING_BAR_ID.to_string(),
                },
                message("Maze generation data is invalid, retry."),
            ],
        };
    };
    if !world_set_blocks(blocks.iter().map(|block| {
        (
            block.dimension.as_str(),
            block.position,
            block.block.as_str(),
        )
    })) {
        return PluginCommandResponse {
            handled: true,
            actions: vec![loading_bar(
                &format!(
                    "{} maze generation retrying",
                    build.session.difficulty_label
                ),
                build_progress(&build),
                "red",
            )],
        };
    }
    build.written = end;

    if build.written < build.total {
        let progress = build_progress(&build);
        let _ = storage_set(&build_key(player_uuid), encode_build_job(&build).as_bytes());
        return PluginCommandResponse {
            handled: true,
            actions: vec![loading_bar(
                &format!(
                    "正在生成 {} 迷宫 {}%",
                    build.session.difficulty_label,
                    (progress * 100.0).round() as i32
                ),
                progress,
                "purple",
            )],
        };
    }

    let mut session = build.session;
    session.started_at_ms = time_millis();
    let _ = storage_set_typed(&instance_key(player_uuid), &session);
    let _ = storage_set_typed(
        &format!("{STARTED_PREFIX}{player_uuid}"),
        &session.started_at_ms,
    );
    clear_build_storage(player_uuid);

    PluginCommandResponse {
        handled: true,
        actions: vec![
            loading_bar(
                &format!("{} 迷宫生成完成", session.difficulty_label),
                1.0,
                "green",
            ),
            message(format!("进入 {} 迷宫", session.difficulty_label)),
            PlayerAction::RemoveBossBar {
                id: MAZE_LOADING_BAR_ID.to_string(),
            },
            PlayerAction::SetPlayersVisible { visible: false },
            PlayerAction::Teleport {
                dimension: session.dimension,
                x: session.start_x as f64 + 0.5,
                y: session.start_y as f64,
                z: session.start_z as f64 + 0.5,
                yaw: Some(0.0),
                pitch: Some(0.0),
            },
        ],
    }
}

fn build_progress(build: &MazeBuildJob) -> f32 {
    if build.total == 0 {
        1.0
    } else {
        (build.written as f32 / build.total as f32).clamp(0.01, 1.0)
    }
}

fn respawn_actions(session: &MazeSession) -> Vec<PlayerAction> {
    vec![
        message("已回到迷宫起点"),
        PlayerAction::Teleport {
            dimension: session.dimension.clone(),
            x: session.start_x as f64 + 0.5,
            y: session.start_y as f64,
            z: session.start_z as f64 + 0.5,
            yaw: Some(0.0),
            pitch: Some(0.0),
        },
    ]
}

fn register_lobby_region(config: &Config) {
    let _ = world_register_edit_region(&WorldEditRegion {
        id: "maze_lobby",
        dimension: &config.lobby.dimension,
        min: (-32, config.lobby.floor_y - 1, -32),
        max: (32, config.lobby.floor_y + 8, 32),
        allow_player_break: false,
        allow_player_place: false,
        allow_plugin_write: true,
        runtime_only: true,
    });
    build_lobby(config);
}

fn build_lobby(config: &Config) {
    let mut sink = BatchedBlockSink::new();
    let y = config.lobby.floor_y;
    for x in -12_i32..=12 {
        for z in -12_i32..=12 {
            let border = x.abs() == 12 || z.abs() == 12;
            let block = if border {
                "minecraft:polished_deepslate"
            } else {
                "minecraft:smooth_stone"
            };
            sink.set_block(&config.lobby.dimension, (x, y - 1, z), block);
            if border {
                for h in 0..=3 {
                    sink.set_block(&config.lobby.dimension, (x, y + h, z), "minecraft:glass");
                }
            }
        }
    }
    for x in -12_i32..=12 {
        for z in -12_i32..=12 {
            for h in 0..=3 {
                if x.abs() != 12 && z.abs() != 12 {
                    sink.set_block(&config.lobby.dimension, (x, y + h, z), "minecraft:air");
                }
            }
        }
    }
    sink.flush();
}

fn register_instance_region(difficulty: &DifficultyConfig, origin: (i32, i32, i32)) {
    let max_x = origin.0 + maze_horizontal_extent(difficulty.width) + 2;
    let max_y = origin.1 + difficulty.levels * difficulty.level_height + 8;
    let max_z = origin.2 + maze_horizontal_extent(difficulty.depth) + 2;
    let id = format!("maze_{}_{}_{}", difficulty.id, origin.0, origin.2);
    let _ = world_register_edit_region(&WorldEditRegion {
        id: &id,
        dimension: &difficulty.dimension,
        min: (origin.0 - 2, origin.1 - 2, origin.2 - 2),
        max: (max_x, max_y, max_z),
        allow_player_break: false,
        allow_player_place: false,
        allow_plugin_write: true,
        runtime_only: true,
    });
}

#[allow(dead_code)]
fn build_maze(difficulty: &DifficultyConfig, origin: (i32, i32, i32), maze: &MazeData) {
    let mut sink = BatchedBlockSink::new();
    build_maze_into(difficulty, origin, maze, &mut sink);
    sink.flush();
}

fn maze_blocks(
    difficulty: &DifficultyConfig,
    origin: (i32, i32, i32),
    maze: &MazeData,
) -> Vec<MazeBlock> {
    let mut sink = CollectingBlockSink::default();
    build_maze_into(difficulty, origin, maze, &mut sink);
    sink.blocks
}

fn build_maze_into(
    difficulty: &DifficultyConfig,
    origin: (i32, i32, i32),
    maze: &MazeData,
    sink: &mut impl BlockSink,
) {
    let width_blocks = maze_horizontal_extent(difficulty.width);
    let depth_blocks = maze_horizontal_extent(difficulty.depth);
    for level in 0..difficulty.levels {
        let base_y = origin.1 + level * difficulty.level_height;
        for x in 0..=width_blocks {
            for z in 0..=depth_blocks {
                set_block(
                    sink,
                    &difficulty.dimension,
                    (origin.0 + x, base_y, origin.2 + z),
                    "minecraft:deepslate_tiles",
                );
                fill_wall_column(sink, difficulty, origin.0 + x, base_y, origin.2 + z);
            }
        }
        for cell_x in 0..difficulty.width {
            for cell_z in 0..difficulty.depth {
                let center_x = origin.0 + cell_center_offset(cell_x);
                let center_z = origin.2 + cell_center_offset(cell_z);
                carve_room(sink, difficulty, center_x, base_y, center_z);
                let cell = maze.cell(level, cell_x, cell_z);
                if cell.east {
                    carve_walkway_line(
                        sink,
                        difficulty,
                        (center_x, center_z),
                        (center_x + CELL_SPACING, center_z),
                        base_y,
                    );
                }
                if cell.south {
                    carve_walkway_line(
                        sink,
                        difficulty,
                        (center_x, center_z),
                        (center_x, center_z + CELL_SPACING),
                        base_y,
                    );
                }
            }
        }
        if difficulty.ceiling {
            for x in 0..=width_blocks {
                for z in 0..=depth_blocks {
                    set_block(
                        sink,
                        &difficulty.dimension,
                        (
                            origin.0 + x,
                            base_y + difficulty.wall_height + 1,
                            origin.2 + z,
                        ),
                        "minecraft:dark_oak_planks",
                    );
                }
            }
        }
    }

    for level in 0..(difficulty.levels - 1) {
        let base_y = origin.1 + level * difficulty.level_height;
        for cell_x in 0..difficulty.width {
            for cell_z in 0..difficulty.depth {
                if maze.cell(level, cell_x, cell_z).up {
                    build_vertical_connector(
                        sink,
                        difficulty,
                        origin.0 + cell_center_offset(cell_x),
                        base_y,
                        origin.2 + cell_center_offset(cell_z),
                    );
                }
            }
        }
    }

    let finish_floor_y = origin.1 + maze.finish_level * difficulty.level_height;
    set_block(
        sink,
        &difficulty.dimension,
        (
            origin.0 + cell_center_offset(maze.finish_x),
            finish_floor_y,
            origin.2 + cell_center_offset(maze.finish_z),
        ),
        "minecraft:emerald_block",
    );
}

fn fill_wall_column(
    sink: &mut impl BlockSink,
    difficulty: &DifficultyConfig,
    x: i32,
    base_y: i32,
    z: i32,
) {
    for y in 1..=difficulty.wall_height {
        set_block(
            sink,
            &difficulty.dimension,
            (x, base_y + y, z),
            "minecraft:stone_bricks",
        );
    }
}

fn carve_column(
    sink: &mut impl BlockSink,
    difficulty: &DifficultyConfig,
    x: i32,
    base_y: i32,
    z: i32,
) {
    for y in 1..=difficulty.wall_height {
        set_block(
            sink,
            &difficulty.dimension,
            (x, base_y + y, z),
            "minecraft:air",
        );
    }
}

fn carve_room(
    sink: &mut impl BlockSink,
    difficulty: &DifficultyConfig,
    x: i32,
    base_y: i32,
    z: i32,
) {
    for dx in -CORRIDOR_RADIUS..=CORRIDOR_RADIUS {
        for dz in -CORRIDOR_RADIUS..=CORRIDOR_RADIUS {
            carve_column(sink, difficulty, x + dx, base_y, z + dz);
        }
    }
}

fn carve_walkway_line(
    sink: &mut impl BlockSink,
    difficulty: &DifficultyConfig,
    from: (i32, i32),
    to: (i32, i32),
    base_y: i32,
) {
    let dx = (to.0 - from.0).signum();
    let dz = (to.1 - from.1).signum();
    let steps = (to.0 - from.0).abs().max((to.1 - from.1).abs());
    for step in 0..=steps {
        let x = from.0 + dx * step;
        let z = from.1 + dz * step;
        if dx != 0 {
            for offset in -CORRIDOR_RADIUS..=CORRIDOR_RADIUS {
                carve_column(sink, difficulty, x, base_y, z + offset);
            }
        } else {
            for offset in -CORRIDOR_RADIUS..=CORRIDOR_RADIUS {
                carve_column(sink, difficulty, x + offset, base_y, z);
            }
        }
    }
}

fn build_vertical_connector(
    sink: &mut impl BlockSink,
    difficulty: &DifficultyConfig,
    x: i32,
    base_y: i32,
    z: i32,
) {
    let path = stair_offsets(difficulty.level_height);
    for (rise, (dx, dz)) in path.iter().copied().enumerate() {
        let floor_y = base_y + rise as i32;
        for air in 1..=PLAYER_JUMP_AIR_HEIGHT {
            set_block(
                sink,
                &difficulty.dimension,
                (x + dx, floor_y + air, z + dz),
                "minecraft:air",
            );
        }
        if rise > 0 && rise < difficulty.level_height as usize {
            set_block(
                sink,
                &difficulty.dimension,
                (x + dx, floor_y, z + dz),
                "minecraft:deepslate_tiles",
            );
        }
    }
}

fn stair_offsets(level_height: i32) -> Vec<(i32, i32)> {
    let needed = level_height.max(1) as usize + 1;
    let mut offsets = Vec::with_capacity(needed);
    offsets.push((0, 0));
    let directions = [(1, 0), (0, 1), (-1, 0), (0, -1)];
    let mut current = (0, 0);
    let mut direction = 0usize;
    let mut run = 1;
    while offsets.len() < needed {
        for _ in 0..2 {
            let (dx, dz) = directions[direction % directions.len()];
            for _ in 0..run {
                if offsets.len() >= needed {
                    break;
                }
                current.0 += dx;
                current.1 += dz;
                offsets.push(current);
            }
            direction += 1;
        }
        run += 1;
    }
    offsets
}

fn cell_center_offset(cell: i32) -> i32 {
    CELL_MARGIN + cell.max(0) * CELL_SPACING
}

fn maze_horizontal_extent(cells: i32) -> i32 {
    CELL_MARGIN * 2 + (cells.max(1) - 1) * CELL_SPACING
}

trait BlockSink {
    fn set_block(&mut self, dimension: &str, position: (i32, i32, i32), block: &str);
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MazeBlock {
    dimension: String,
    position: (i32, i32, i32),
    block: String,
}

#[derive(Default)]
struct CollectingBlockSink {
    blocks: Vec<MazeBlock>,
}

impl BlockSink for CollectingBlockSink {
    fn set_block(&mut self, dimension: &str, position: (i32, i32, i32), block: &str) {
        self.blocks.push(MazeBlock {
            dimension: dimension.to_string(),
            position,
            block: block.to_string(),
        });
    }
}

struct BatchedBlockSink {
    blocks: Vec<MazeBlock>,
}

impl BatchedBlockSink {
    fn new() -> Self {
        Self {
            blocks: Vec::with_capacity(WORLD_EDIT_BATCH_LIMIT),
        }
    }

    fn flush(&mut self) {
        if self.blocks.is_empty() {
            return;
        }
        let _ = world_set_blocks(self.blocks.iter().map(|block| {
            (
                block.dimension.as_str(),
                block.position,
                block.block.as_str(),
            )
        }));
        self.blocks.clear();
    }
}

impl BlockSink for BatchedBlockSink {
    fn set_block(&mut self, dimension: &str, position: (i32, i32, i32), block: &str) {
        self.blocks.push(MazeBlock {
            dimension: dimension.to_string(),
            position,
            block: block.to_string(),
        });
        if self.blocks.len() >= WORLD_EDIT_BATCH_LIMIT {
            self.flush();
        }
    }
}

fn set_block(sink: &mut impl BlockSink, dimension: &str, position: (i32, i32, i32), block: &str) {
    sink.set_block(dimension, position, block);
}

fn generate_maze(difficulty: &DifficultyConfig, seed: u64) -> MazeData {
    let mut best = build_maze_candidate(difficulty, seed);
    for attempt in 1..MAZE_GENERATION_ATTEMPTS {
        let candidate = build_maze_candidate(
            difficulty,
            seed.wrapping_add((attempt as u64).wrapping_mul(0x9e3779b97f4a7c15)),
        );
        if candidate.finish_distance > best.finish_distance {
            best = candidate;
        }
    }
    best
}

fn build_maze_candidate(difficulty: &DifficultyConfig, seed: u64) -> MazeData {
    let mut rng = Rng::new(seed);
    let total = (difficulty.levels * difficulty.width * difficulty.depth).max(1) as usize;
    let mut cells = vec![MazeCell::default(); total];
    let mut visited = vec![false; total];
    let mut stack = vec![(0, 0, 0)];
    visited[cell_index(difficulty, 0, 0, 0)] = true;
    while let Some(&(level, x, z)) = stack.last() {
        let mut neighbors = Vec::new();
        for (nl, nx, nz, dir) in [
            (level, x + 1, z, 0),
            (level, x - 1, z, 1),
            (level, x, z + 1, 2),
            (level, x, z - 1, 3),
            (level + 1, x, z, 4),
            (level - 1, x, z, 5),
        ] {
            if nl >= 0
                && nl < difficulty.levels
                && nx >= 0
                && nx < difficulty.width
                && nz >= 0
                && nz < difficulty.depth
                && !visited[cell_index(difficulty, nl, nx, nz)]
                && (difficulty.levels > 1 || dir < 4)
            {
                neighbors.push((nl, nx, nz, dir));
            }
        }
        if neighbors.is_empty() {
            stack.pop();
            continue;
        }
        let picked = rng.range(neighbors.len() as u32) as usize;
        let (nl, nx, nz, dir) = neighbors[picked];
        let current = cell_index(difficulty, level, x, z);
        let next = cell_index(difficulty, nl, nx, nz);
        match dir {
            0 => cells[current].east = true,
            1 => cells[next].east = true,
            2 => cells[current].south = true,
            3 => cells[next].south = true,
            4 => cells[current].up = true,
            5 => cells[next].up = true,
            _ => {}
        }
        visited[next] = true;
        stack.push((nl, nx, nz));
    }
    let (finish_level, finish_x, finish_z, finish_distance) = maze_finish(difficulty, &cells);
    MazeData {
        width: difficulty.width,
        depth: difficulty.depth,
        finish_level,
        finish_x,
        finish_z,
        finish_distance,
        cells,
    }
}

fn maze_finish(difficulty: &DifficultyConfig, cells: &[MazeCell]) -> (i32, i32, i32, i32) {
    let total = (difficulty.levels * difficulty.width * difficulty.depth).max(1) as usize;
    let mut distance = vec![-1_i32; total];
    let mut queue = std::collections::VecDeque::new();
    distance[cell_index(difficulty, 0, 0, 0)] = 0;
    queue.push_back((0, 0, 0));

    while let Some((level, x, z)) = queue.pop_front() {
        let index = cell_index(difficulty, level, x, z);
        let next_distance = distance[index] + 1;
        for (next_level, next_x, next_z) in maze_neighbors(difficulty, cells, level, x, z) {
            let next_index = cell_index(difficulty, next_level, next_x, next_z);
            if distance[next_index] >= 0 {
                continue;
            }
            distance[next_index] = next_distance;
            queue.push_back((next_level, next_x, next_z));
        }
    }

    let target_level = difficulty.levels.saturating_sub(1).max(0);
    let mut best = farthest_cell_on_level(difficulty, &distance, target_level);
    if best.3 >= 0 {
        return best;
    }

    for level in 0..difficulty.levels {
        let candidate = farthest_cell_on_level(difficulty, &distance, level);
        if candidate.3 > best.3 {
            best = candidate;
        }
    }
    best
}

fn farthest_cell_on_level(
    difficulty: &DifficultyConfig,
    distance: &[i32],
    level: i32,
) -> (i32, i32, i32, i32) {
    let mut best = (level, 0, 0, -1_i32);
    for z in 0..difficulty.depth {
        for x in 0..difficulty.width {
            let value = distance[cell_index(difficulty, level, x, z)];
            if value > best.3 {
                best = (level, x, z, value);
            }
        }
    }
    best
}

fn maze_neighbors(
    difficulty: &DifficultyConfig,
    cells: &[MazeCell],
    level: i32,
    x: i32,
    z: i32,
) -> Vec<(i32, i32, i32)> {
    let mut neighbors = Vec::with_capacity(6);
    let cell = cells
        .get(cell_index(difficulty, level, x, z))
        .copied()
        .unwrap_or_default();
    if cell.east && x + 1 < difficulty.width {
        neighbors.push((level, x + 1, z));
    }
    if x > 0
        && cells
            .get(cell_index(difficulty, level, x - 1, z))
            .is_some_and(|west| west.east)
    {
        neighbors.push((level, x - 1, z));
    }
    if cell.south && z + 1 < difficulty.depth {
        neighbors.push((level, x, z + 1));
    }
    if z > 0
        && cells
            .get(cell_index(difficulty, level, x, z - 1))
            .is_some_and(|north| north.south)
    {
        neighbors.push((level, x, z - 1));
    }
    if cell.up && level + 1 < difficulty.levels {
        neighbors.push((level + 1, x, z));
    }
    if level > 0
        && cells
            .get(cell_index(difficulty, level - 1, x, z))
            .is_some_and(|below| below.up)
    {
        neighbors.push((level - 1, x, z));
    }
    neighbors
}

fn cell_index(difficulty: &DifficultyConfig, level: i32, x: i32, z: i32) -> usize {
    ((level * difficulty.width * difficulty.depth) + (z * difficulty.width) + x) as usize
}

fn instance_origin(
    config: &Config,
    player_uuid: &str,
    difficulty: &DifficultyConfig,
) -> (i32, i32, i32) {
    let hash = stable_seed(player_uuid, &difficulty.id);
    let slot_x = (hash & 0xff) as i32;
    let slot_z = ((hash >> 8) & 0xff) as i32;
    (
        difficulty.origin_x + slot_x * config.instance_spacing,
        difficulty.origin_y,
        difficulty.origin_z + slot_z * config.instance_spacing,
    )
}

fn reached_finish(session: &MazeSession, x: f64, y: f64, z: f64) -> bool {
    (x - (session.finish_x as f64 + 0.5)).abs() <= 1.25
        && (y - session.finish_y as f64).abs() <= 2.5
        && (z - (session.finish_z as f64 + 0.5)).abs() <= 1.25
}

fn elapsed_ms(started_at_ms: i64) -> i64 {
    time_millis().saturating_sub(started_at_ms).max(0)
}

fn format_duration(ms: i64) -> String {
    let seconds = ms.max(0) / 1000;
    let minutes = seconds / 60;
    format!("{minutes:02}:{:02}", seconds % 60)
}

fn message(text: impl Into<String>) -> PlayerAction {
    PlayerAction::SystemMessage {
        text: text.into(),
        translate: String::new(),
        with: Vec::new(),
        overlay: false,
    }
}

fn loading_bar(title: &str, progress: f32, color: &str) -> PlayerAction {
    PlayerAction::BossBar {
        id: MAZE_LOADING_BAR_ID.to_string(),
        title: title.to_string(),
        progress: progress.clamp(0.0, 1.0),
        color: color.to_string(),
        overlay: "progress".to_string(),
    }
}

fn stable_seed(player_uuid: &str, difficulty: &str) -> u64 {
    let mut value = 0xcbf29ce484222325_u64;
    for byte in player_uuid.bytes().chain(difficulty.bytes()) {
        value ^= byte as u64;
        value = value.wrapping_mul(0x100000001b3);
    }
    value
}

fn instance_key(uuid: &str) -> String {
    format!("{INSTANCE_PREFIX}{uuid}")
}

fn build_key(uuid: &str) -> String {
    format!("{BUILD_PREFIX}{uuid}")
}

fn build_chunk_key(uuid: &str, chunk: usize) -> String {
    format!("{BUILD_PREFIX}{uuid}/chunk/{chunk}")
}

fn clear_build_storage(uuid: &str) {
    let chunk_count = storage_get(&build_key(uuid))
        .and_then(|bytes| decode_build_job(&String::from_utf8_lossy(&bytes)))
        .map(|build| build.chunk_count)
        .unwrap_or(MAX_STALE_BUILD_STORAGE_CHUNKS);
    for chunk in 0..chunk_count.min(MAX_STALE_BUILD_STORAGE_CHUNKS) {
        let _ = storage_delete(&build_chunk_key(uuid, chunk));
    }
    let _ = storage_delete(&build_key(uuid));
}

fn best_key(uuid: &str, difficulty: &str) -> String {
    format!("{BEST_PREFIX}{difficulty}/{uuid}")
}

fn load_config() -> Config {
    config_read_to_string(CONFIG_PATH)
        .and_then(|contents| toml::from_str::<Config>(&contents).ok())
        .unwrap_or_default()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MazeSession {
    active: bool,
    difficulty: String,
    difficulty_label: String,
    dimension: String,
    start_x: i32,
    start_y: i32,
    start_z: i32,
    finish_x: i32,
    finish_y: i32,
    finish_z: i32,
    min_y: i32,
    started_at_ms: i64,
}

impl MazeSession {
    fn lobby(dimension: &str) -> Self {
        Self {
            active: false,
            difficulty: String::new(),
            difficulty_label: "大厅".to_string(),
            dimension: dimension.to_string(),
            start_x: 0,
            start_y: 0,
            start_z: 0,
            finish_x: 0,
            finish_y: 0,
            finish_z: 0,
            min_y: -64,
            started_at_ms: 0,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct Config {
    #[serde(default = "default_true")]
    enable: bool,
    #[serde(default = "default_menu_id")]
    menu_id: String,
    #[serde(default = "default_instance_spacing")]
    instance_spacing: i32,
    #[serde(default)]
    lobby: LobbyConfig,
    #[serde(default = "default_difficulties")]
    difficulties: Vec<DifficultyConfig>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enable: true,
            menu_id: default_menu_id(),
            instance_spacing: default_instance_spacing(),
            lobby: LobbyConfig::default(),
            difficulties: default_difficulties(),
        }
    }
}

impl Config {
    fn difficulty(&self, id: &str) -> Option<&DifficultyConfig> {
        self.difficulties
            .iter()
            .find(|difficulty| difficulty.id == id)
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
    #[serde(default = "default_npc_x")]
    npc_x: f64,
    #[serde(default = "default_lobby_y")]
    npc_y: f64,
    #[serde(default = "default_npc_z")]
    npc_z: f64,
    #[serde(default)]
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
            spawn_x: 0.0,
            spawn_y: default_lobby_y(),
            spawn_z: 0.0,
            spawn_yaw: 0.0,
            spawn_pitch: 0.0,
            floor_y: default_lobby_floor_y(),
            npc_x: default_npc_x(),
            npc_y: default_lobby_y(),
            npc_z: default_npc_z(),
            npc_yaw: 0.0,
            npc_pitch: 0.0,
            npc_entity_type: default_npc_entity_type(),
            npc_display_name: default_npc_display_name(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct DifficultyConfig {
    id: String,
    label: String,
    dimension: String,
    width: i32,
    depth: i32,
    levels: i32,
    level_height: i32,
    wall_height: i32,
    ceiling: bool,
    origin_x: i32,
    origin_y: i32,
    origin_z: i32,
}

#[derive(Debug, Clone)]
struct MazeData {
    width: i32,
    depth: i32,
    finish_level: i32,
    finish_x: i32,
    finish_z: i32,
    finish_distance: i32,
    cells: Vec<MazeCell>,
}

impl MazeData {
    fn cell(&self, level: i32, x: i32, z: i32) -> MazeCell {
        let index = ((level * self.width * self.depth) + (z * self.width) + x) as usize;
        self.cells.get(index).copied().unwrap_or_default()
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct MazeCell {
    east: bool,
    south: bool,
    up: bool,
}

struct MazeBuildJob {
    session: MazeSession,
    total: usize,
    written: usize,
    chunk_count: usize,
}

fn encode_build_job(job: &MazeBuildJob) -> String {
    let mut encoded = String::new();
    encode_session(&mut encoded, &job.session);
    encoded.push('\n');
    encoded.push_str(&job.total.to_string());
    encoded.push('\n');
    encoded.push_str(&job.written.to_string());
    encoded.push('\n');
    encoded.push_str(&job.chunk_count.to_string());
    encoded
}

fn decode_build_job(encoded: &str) -> Option<MazeBuildJob> {
    let mut lines = encoded.lines();
    let session = decode_session(lines.next()?)?;
    let total = lines.next()?.parse().ok()?;
    let written = lines.next()?.parse().ok()?;
    let chunk_count = lines.next()?.parse().ok()?;
    Some(MazeBuildJob {
        session,
        total,
        written,
        chunk_count,
    })
}

fn store_build_job(uuid: &str, job: &MazeBuildJob, blocks: &[MazeBlock]) -> bool {
    if !storage_set(&build_key(uuid), encode_build_job(job).as_bytes()) {
        return false;
    }
    for (chunk_index, chunk) in blocks.chunks(BUILD_BLOCKS_PER_STORAGE_CHUNK).enumerate() {
        if !storage_set(
            &build_chunk_key(uuid, chunk_index),
            encode_build_blocks(chunk).as_bytes(),
        ) {
            return false;
        }
    }
    true
}

fn load_build_blocks(
    uuid: &str,
    job: &MazeBuildJob,
    start: usize,
    end: usize,
) -> Option<Vec<MazeBlock>> {
    if start >= end {
        return Some(Vec::new());
    }
    let first_chunk = start / BUILD_BLOCKS_PER_STORAGE_CHUNK;
    let last_chunk = (end - 1) / BUILD_BLOCKS_PER_STORAGE_CHUNK;
    if last_chunk >= job.chunk_count {
        return None;
    }
    let mut result = Vec::with_capacity(end - start);
    for chunk_index in first_chunk..=last_chunk {
        let bytes = storage_get(&build_chunk_key(uuid, chunk_index))?;
        let chunk = decode_build_blocks(&String::from_utf8_lossy(&bytes))?;
        let chunk_start = chunk_index * BUILD_BLOCKS_PER_STORAGE_CHUNK;
        let from = start.saturating_sub(chunk_start);
        let to = (end - chunk_start).min(chunk.len());
        if from > to {
            return None;
        }
        result.extend_from_slice(&chunk[from..to]);
    }
    Some(result)
}

fn build_chunk_count(total: usize) -> usize {
    total.div_ceil(BUILD_BLOCKS_PER_STORAGE_CHUNK)
}

fn encode_build_blocks(blocks: &[MazeBlock]) -> String {
    let mut encoded = String::new();
    for block in blocks {
        encoded.push_str(&block.dimension);
        encoded.push('\t');
        encoded.push_str(&block.position.0.to_string());
        encoded.push('\t');
        encoded.push_str(&block.position.1.to_string());
        encoded.push('\t');
        encoded.push_str(&block.position.2.to_string());
        encoded.push('\t');
        encoded.push_str(&block.block);
        encoded.push('\n');
    }
    encoded
}

fn decode_build_blocks(encoded: &str) -> Option<Vec<MazeBlock>> {
    let mut blocks = Vec::new();
    for line in encoded.lines() {
        let mut parts = line.split('\t');
        blocks.push(MazeBlock {
            dimension: parts.next()?.to_string(),
            position: (
                parts.next()?.parse().ok()?,
                parts.next()?.parse().ok()?,
                parts.next()?.parse().ok()?,
            ),
            block: parts.next()?.to_string(),
        });
    }
    Some(blocks)
}

fn encode_session(encoded: &mut String, session: &MazeSession) {
    encoded.push_str(if session.active { "1" } else { "0" });
    encoded.push('\t');
    encoded.push_str(&session.difficulty);
    encoded.push('\t');
    encoded.push_str(&session.difficulty_label);
    encoded.push('\t');
    encoded.push_str(&session.dimension);
    encoded.push('\t');
    encoded.push_str(&session.start_x.to_string());
    encoded.push('\t');
    encoded.push_str(&session.start_y.to_string());
    encoded.push('\t');
    encoded.push_str(&session.start_z.to_string());
    encoded.push('\t');
    encoded.push_str(&session.finish_x.to_string());
    encoded.push('\t');
    encoded.push_str(&session.finish_y.to_string());
    encoded.push('\t');
    encoded.push_str(&session.finish_z.to_string());
    encoded.push('\t');
    encoded.push_str(&session.min_y.to_string());
    encoded.push('\t');
    encoded.push_str(&session.started_at_ms.to_string());
}

fn decode_session(encoded: &str) -> Option<MazeSession> {
    let mut parts = encoded.split('\t');
    Some(MazeSession {
        active: parts.next()? == "1",
        difficulty: parts.next()?.to_string(),
        difficulty_label: parts.next()?.to_string(),
        dimension: parts.next()?.to_string(),
        start_x: parts.next()?.parse().ok()?,
        start_y: parts.next()?.parse().ok()?,
        start_z: parts.next()?.parse().ok()?,
        finish_x: parts.next()?.parse().ok()?,
        finish_y: parts.next()?.parse().ok()?,
        finish_z: parts.next()?.parse().ok()?,
        min_y: parts.next()?.parse().ok()?,
        started_at_ms: parts.next()?.parse().ok()?,
    })
}

struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed.max(1))
    }

    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 32) as u32
    }

    fn range(&mut self, max: u32) -> u32 {
        if max == 0 { 0 } else { self.next() % max }
    }
}

fn default_true() -> bool {
    true
}

fn default_menu_id() -> String {
    "maze".to_string()
}

fn default_instance_spacing() -> i32 {
    256
}

fn default_lobby_dimension() -> String {
    "qexed:maze_lobby".to_string()
}

fn default_lobby_y() -> f64 {
    -52.0
}

fn default_lobby_floor_y() -> i32 {
    -52
}

fn default_npc_x() -> f64 {
    0.5
}

fn default_npc_z() -> f64 {
    4.5
}

fn default_npc_entity_type() -> String {
    "minecraft:villager".to_string()
}

fn default_npc_display_name() -> String {
    "{\"text\":\"迷宫入口\",\"color\":\"gold\"}".to_string()
}

fn default_difficulties() -> Vec<DifficultyConfig> {
    vec![
        DifficultyConfig {
            id: "easy".to_string(),
            label: "简单".to_string(),
            dimension: "qexed:maze_easy".to_string(),
            width: 12,
            depth: 12,
            levels: 1,
            level_height: 5,
            wall_height: 2,
            ceiling: false,
            origin_x: 1024,
            origin_y: -52,
            origin_z: 1024,
        },
        DifficultyConfig {
            id: "normal".to_string(),
            label: "普通".to_string(),
            dimension: "qexed:maze_normal".to_string(),
            width: 20,
            depth: 20,
            levels: 1,
            level_height: 5,
            wall_height: 3,
            ceiling: true,
            origin_x: 1024,
            origin_y: -52,
            origin_z: 1024,
        },
        DifficultyConfig {
            id: "hard".to_string(),
            label: "困难".to_string(),
            dimension: "qexed:maze_hard".to_string(),
            width: 14,
            depth: 14,
            levels: 3,
            level_height: 5,
            wall_height: 3,
            ceiling: true,
            origin_x: 1024,
            origin_y: -52,
            origin_z: 1024,
        },
    ]
}

const DEFAULT_CONFIG: &str = r#"enable = true
menu_id = "maze"
instance_spacing = 256

[lobby]
dimension = "qexed:maze_lobby"
spawn_x = 0.5
spawn_y = -52.0
spawn_z = 0.5
spawn_yaw = 0.0
spawn_pitch = 0.0
floor_y = -52
npc_x = 0.5
npc_y = -52.0
npc_z = 4.5
npc_yaw = 180.0
npc_pitch = 0.0
npc_entity_type = "minecraft:villager"
npc_display_name = "{\"text\":\"迷宫入口\",\"color\":\"gold\"}"

[[difficulties]]
id = "easy"
label = "简单"
dimension = "qexed:maze_easy"
width = 12
depth = 12
levels = 1
level_height = 5
wall_height = 2
ceiling = false
origin_x = 1024
origin_y = -52
origin_z = 1024

[[difficulties]]
id = "normal"
label = "普通"
dimension = "qexed:maze_normal"
width = 20
depth = 20
levels = 1
level_height = 5
wall_height = 3
ceiling = true
origin_x = 1024
origin_y = -52
origin_z = 1024

[[difficulties]]
id = "hard"
label = "困难"
dimension = "qexed:maze_hard"
width = 14
depth = 14
levels = 3
level_height = 5
wall_height = 3
ceiling = true
origin_x = 1024
origin_y = -52
origin_z = 1024
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashMap, HashSet, VecDeque};

    const PLAYER_STAND_AIR_HEIGHT: i32 = 2;

    #[unsafe(no_mangle)]
    extern "C" fn log(_ptr: i32, _len: i32) {}

    #[unsafe(no_mangle)]
    extern "C" fn config_exists(_ptr: i32, _len: i32) -> i32 {
        0
    }

    #[unsafe(no_mangle)]
    extern "C" fn config_read(_path_ptr: i32, _path_len: i32, _out_ptr: i32, _out_len: i32) -> i64 {
        -1
    }

    #[unsafe(no_mangle)]
    extern "C" fn config_write(
        _path_ptr: i32,
        _path_len: i32,
        _data_ptr: i32,
        _data_len: i32,
    ) -> i32 {
        0
    }

    #[unsafe(no_mangle)]
    extern "C" fn storage_exists(_ptr: i32, _len: i32) -> i32 {
        0
    }

    #[unsafe(no_mangle)]
    extern "C" fn storage_get(_key_ptr: i32, _key_len: i32, _out_ptr: i32, _out_len: i32) -> i64 {
        -1
    }

    #[unsafe(no_mangle)]
    extern "C" fn storage_set(_key_ptr: i32, _key_len: i32, _data_ptr: i32, _data_len: i32) -> i32 {
        0
    }

    #[unsafe(no_mangle)]
    extern "C" fn storage_delete(_ptr: i32, _len: i32) -> i32 {
        0
    }

    #[unsafe(no_mangle)]
    extern "C" fn economy_register_currency(
        _id_ptr: i32,
        _id_len: i32,
        _name_ptr: i32,
        _name_len: i32,
        _symbol_ptr: i32,
        _symbol_len: i32,
        _fractional_digits: i32,
    ) -> i32 {
        0
    }

    #[unsafe(no_mangle)]
    extern "C" fn economy_currency_info(
        _currency_ptr: i32,
        _currency_len: i32,
        _out_ptr: i32,
        _out_len: i32,
    ) -> i64 {
        -1
    }

    #[unsafe(no_mangle)]
    extern "C" fn economy_balance(
        _player_ptr: i32,
        _player_len: i32,
        _currency_ptr: i32,
        _currency_len: i32,
    ) -> i64 {
        i64::MIN + 1
    }

    #[unsafe(no_mangle)]
    extern "C" fn economy_set_balance(
        _player_ptr: i32,
        _player_len: i32,
        _currency_ptr: i32,
        _currency_len: i32,
        _amount: i64,
    ) -> i64 {
        i64::MIN + 1
    }

    #[unsafe(no_mangle)]
    extern "C" fn economy_deposit(
        _player_ptr: i32,
        _player_len: i32,
        _currency_ptr: i32,
        _currency_len: i32,
        _amount: i64,
    ) -> i64 {
        i64::MIN + 1
    }

    #[unsafe(no_mangle)]
    extern "C" fn economy_withdraw(
        _player_ptr: i32,
        _player_len: i32,
        _currency_ptr: i32,
        _currency_len: i32,
        _amount: i64,
    ) -> i64 {
        i64::MIN + 1
    }

    #[unsafe(no_mangle)]
    extern "C" fn lottery_roll(
        _entries_ptr: i32,
        _entries_len: i32,
        _out_ptr: i32,
        _out_len: i32,
    ) -> i64 {
        -1
    }

    #[unsafe(no_mangle)]
    extern "C" fn pathfinding_find(
        _query_ptr: i32,
        _query_len: i32,
        _out_ptr: i32,
        _out_len: i32,
    ) -> i64 {
        -1
    }

    #[unsafe(no_mangle)]
    extern "C" fn world_set_block(_query_ptr: i32, _query_len: i32) -> i32 {
        0
    }

    #[unsafe(no_mangle)]
    extern "C" fn world_set_blocks(_query_ptr: i32, _query_len: i32) -> i32 {
        0
    }

    #[unsafe(no_mangle)]
    extern "C" fn world_break_block(_query_ptr: i32, _query_len: i32) -> i32 {
        0
    }

    #[unsafe(no_mangle)]
    extern "C" fn world_register_edit_region(_query_ptr: i32, _query_len: i32) -> i32 {
        0
    }

    #[unsafe(no_mangle)]
    extern "C" fn random_pool_roll(
        _request_ptr: i32,
        _request_len: i32,
        _out_ptr: i32,
        _out_len: i32,
    ) -> i64 {
        -1
    }

    #[unsafe(no_mangle)]
    extern "C" fn time_millis() -> i64 {
        0
    }

    #[test]
    fn all_default_difficulties_build_physically_walkable_mazes() {
        for difficulty in default_difficulties() {
            for seed in 1..=16 {
                let maze = generate_maze(&difficulty, seed);
                assert!(
                    maze_is_walkable(&difficulty, (0, 0, 0), &maze),
                    "maze is not physically walkable: difficulty={}, seed={seed}",
                    difficulty.id
                );
            }
        }
    }

    #[test]
    fn hard_maze_vertical_connectors_reach_last_level_finish() {
        let difficulty = default_difficulties()
            .into_iter()
            .find(|difficulty| difficulty.id == "hard")
            .expect("hard difficulty exists");
        let maze = generate_maze(&difficulty, 0x5eed);

        assert_eq!(maze.finish_level, difficulty.levels - 1);
        assert!(maze.finish_distance >= hard_min_finish_distance(&difficulty));
        assert!(maze_is_walkable(&difficulty, (0, 0, 0), &maze));
    }

    #[test]
    fn hard_maze_keeps_long_route_for_common_seeds() {
        let difficulty = default_difficulties()
            .into_iter()
            .find(|difficulty| difficulty.id == "hard")
            .expect("hard difficulty exists");
        let min_distance = hard_min_finish_distance(&difficulty);
        for seed in 1..=32 {
            let maze = generate_maze(&difficulty, seed);
            assert_eq!(maze.finish_level, difficulty.levels - 1);
            assert!(
                maze.finish_distance >= min_distance,
                "hard maze route too short: seed={seed}, distance={}, min={min_distance}",
                maze.finish_distance
            );
        }
    }

    fn hard_min_finish_distance(difficulty: &DifficultyConfig) -> i32 {
        (difficulty.width * difficulty.depth * difficulty.levels / 3).max(1)
    }

    fn maze_is_walkable(
        difficulty: &DifficultyConfig,
        origin: (i32, i32, i32),
        maze: &MazeData,
    ) -> bool {
        let mut sink = MemoryBlockSink::default();
        build_maze_into(difficulty, origin, maze, &mut sink);

        let start = (
            origin.0 + cell_center_offset(0),
            origin.1 + 1,
            origin.2 + cell_center_offset(0),
        );
        let finish = (
            origin.0 + cell_center_offset(maze.finish_x),
            origin.1 + maze.finish_level * difficulty.level_height + 1,
            origin.2 + cell_center_offset(maze.finish_z),
        );
        sink.can_walk_between(start, finish) && sink.can_walk_between(finish, start)
    }

    #[derive(Default)]
    struct MemoryBlockSink {
        blocks: HashMap<(i32, i32, i32), String>,
    }

    impl BlockSink for MemoryBlockSink {
        fn set_block(&mut self, _dimension: &str, position: (i32, i32, i32), block: &str) {
            self.blocks.insert(position, block.to_string());
        }
    }

    impl MemoryBlockSink {
        fn can_walk_between(&self, start: (i32, i32, i32), finish: (i32, i32, i32)) -> bool {
            if !self.can_stand(start) || !self.can_stand(finish) {
                return false;
            }

            let mut visited = HashSet::new();
            let mut queue = VecDeque::from([start]);
            visited.insert(start);

            while let Some(position) = queue.pop_front() {
                if position == finish {
                    return true;
                }
                for next in self.walk_neighbors(position) {
                    if visited.insert(next) {
                        queue.push_back(next);
                    }
                }
            }

            false
        }

        fn walk_neighbors(&self, position: (i32, i32, i32)) -> Vec<(i32, i32, i32)> {
            let mut neighbors = Vec::with_capacity(12);
            for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let x = position.0 + dx;
                let z = position.2 + dz;
                let same_level = (x, position.1, z);
                if self.can_stand(same_level) {
                    neighbors.push(same_level);
                }

                let step_up = (x, position.1 + 1, z);
                if self.can_step_up(position, step_up) {
                    neighbors.push(step_up);
                }

                let step_down = (x, position.1 - 1, z);
                if self.can_stand(step_down) {
                    neighbors.push(step_down);
                }
            }
            neighbors
        }

        fn can_step_up(&self, from: (i32, i32, i32), to: (i32, i32, i32)) -> bool {
            self.can_stand(to)
                && self.has_air_column(from, PLAYER_JUMP_AIR_HEIGHT)
                && self.has_air_column(to, PLAYER_JUMP_AIR_HEIGHT)
        }

        fn can_stand(&self, position: (i32, i32, i32)) -> bool {
            self.solid((position.0, position.1 - 1, position.2))
                && self.has_air_column(position, PLAYER_STAND_AIR_HEIGHT)
        }

        fn has_air_column(&self, position: (i32, i32, i32), height: i32) -> bool {
            (0..height).all(|dy| !self.solid((position.0, position.1 + dy, position.2)))
        }

        fn solid(&self, position: (i32, i32, i32)) -> bool {
            self.blocks
                .get(&position)
                .is_none_or(|block| block != "minecraft:air")
        }
    }
}

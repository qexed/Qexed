//! maze_runner：v4 → v6 原生插件移植 —— 多层迷宫竞速小游戏。
//!
//! v4 导出 → v6 事件映射：
//! - commands / command_execute   → ctx.register_command("maze") + ChatCommandEvent（cancel 接管）
//! - player_join / player_leave   → PlayerJoinEvent / PlayerLeaveEvent
//! - player_move                  → PlayerMoveEvent（坠落重生 + 终点判定）
//! - player_tick                  → PlayerTickEvent（休眠：宿主接线后自动生效；分段建造/罗盘/剧情/守卫）
//! - npc_mutations / npc_interact → NpcMutationsEvent（Mutex 回写）/ NpcInteractEvent
//! - placeholders                 → PlaceholdersEvent（Mutex 回写）
//! - config_reload                → ConfigReloadEvent
//! 语义差异：v6 entity_move 用 NpcPosition；存储键一律不带冒号。

use qexed_plugin_api::payload::{
    NpcMutationOp, NpcPosition, NpcUpsert, PlayerAction as Pa, PlayerPositionPayload,
};
use qexed_plugin_api::services::{RuntimeEntitySpec, WorldEditRegionSpec};
use qexed_plugin_api::{
    ChatCommandEvent, ConfigReloadEvent, EventContext, LogLevel, NpcInteractEvent,
    NpcMutationsEvent, PlaceholdersEvent, PlayerJoinEvent, PlayerLeaveEvent, PlayerMoveEvent,
    PlayerTickEvent,
};
use qexed_plugin_sdk::{InitFuture, PluginContext};
use serde::{Deserialize, Serialize};

const COMMAND_NAME: &str = "maze";
const NPC_KEY: &str = "maze_runner_selector"; // v4 "maze_runner:selector"：v6 存储键不允许冒号
const NPC_EVENT: &str = "maze_select";
const STARTED_PREFIX: &str = "maze_started";
const BEST_PREFIX: &str = "maze_best";
const INSTANCE_PREFIX: &str = "maze_instance";
const BUILD_PREFIX: &str = "maze_build";
const CELL_SPACING: i32 = 8;
const CELL_MARGIN: i32 = 4;
const CORRIDOR_RADIUS: i32 = 1;
const PLAYER_JUMP_AIR_HEIGHT: i32 = 3;
const WORLD_EDIT_BATCH_LIMIT: usize = 4096;
const MAZE_LOADING_BAR_ID: &str = "maze_runner_loading";
const BUILD_BLOCKS_PER_TICK: usize = 1024;
const BUILD_BLOCKS_PER_STORAGE_CHUNK: usize = 4096;
const MAX_STALE_BUILD_STORAGE_CHUNKS: usize = 256;
const MAZE_GENERATION_ATTEMPTS: usize = 8;
const MAZE_COMPASS_BAR_ID: &str = "maze_runner_compass";
const MAZE_STORY_PREFIX: &str = "maze_story";
const MAZE_GUARD_PREFIX: &str = "maze_guard";
const GUARD_REACH_DISTANCE: f64 = 0.35;
const GUARD_DETECT_DISTANCE: f64 = 7.5;
const GUARD_DETECT_DOT: f64 = 0.35;

qexed_plugin_sdk::declare_simple_plugin!("maze_runner", "Maze Runner", "0.1.0", |ctx: PluginContext| -> InitFuture {
    Box::pin(async move {
        let plugin_id: &'static str = ctx.id();
        let sdk = ctx.sdk();
        let _ = sdk.config_load_or_create(DEFAULT_CONFIG);
        let config = load_config(&sdk);
        register_lobby_region(&sdk, &config);
        ctx.register_command(COMMAND_NAME);
        ctx.log(LogLevel::Info, "maze_runner initialized");

        ctx.register::<ChatCommandEvent>(move |api, ev, ectx: &EventContext| {
            let plugin_id = plugin_id;
            Box::pin(async move {
                if ev.command != COMMAND_NAME {
                    return;
                }
                ectx.cancel();
                let sdk = qexed_plugin_sdk::Sdk::new(api, plugin_id);
                let config = load_config(&sdk);
                let player = &ev.player;
                let argument = ev.argument.trim();
                let actions = match argument {
                    "" | "menu" => vec![Pa::OpenMenu { menu: config.menu_id.clone() }],
                    "leave" | "lobby" | "spawn" => leave_to_lobby(&sdk, &config, &player.uuid),
                    _ => {
                        let mut parts = argument.split_whitespace();
                        match parts.next() {
                            Some("start") => start_maze_loading(
                                &sdk,
                                &config,
                                &player.uuid,
                                parts.next().unwrap_or_default(),
                            ),
                            _ => vec![message(
                                "用法: /maze menu | /maze start <easy|normal|hard> | /maze leave",
                            )],
                        }
                    }
                };
                sdk.perform_all(&actions.iter().map(|a| (player, a.clone())).collect::<Vec<_>>());
            })
        });

        ctx.register::<PlayerJoinEvent>(move |api, ev, _| {
            let plugin_id = plugin_id;
            Box::pin(async move {
                let sdk = qexed_plugin_sdk::Sdk::new(api, plugin_id);
                let config = load_config(&sdk);
                if config.enable && ev.player.dimension == config.lobby.dimension {
                    clear_build_storage(&sdk, &ev.player.uuid);
                    let _ = sdk.storage_set_json(
                        &instance_key(&ev.player.uuid),
                        &MazeSession::lobby(&config.lobby.dimension),
                    );
                }
            })
        });

        ctx.register::<PlayerLeaveEvent>(move |api, ev, _| {
            let plugin_id = plugin_id;
            Box::pin(async move {
                let sdk = qexed_plugin_sdk::Sdk::new(api, plugin_id);
                clear_guard_runtime(&sdk, &ev.player.uuid);
            })
        });

        ctx.register::<PlayerMoveEvent>(move |api, ev, _| {
            let plugin_id = plugin_id;
            Box::pin(async move {
                let sdk = qexed_plugin_sdk::Sdk::new(api, plugin_id);
                let config = load_config(&sdk);
                let Some(session) = sdk.storage_get_json::<MazeSession>(&instance_key(&ev.player.uuid)) else {
                    return;
                };
                if !session.active || ev.dimension != session.dimension {
                    return;
                }
                let pos = &ev.position;
                let mut actions = Vec::new();
                if pos.y < f64::from(session.min_y) - 8.0 {
                    actions.extend(respawn_actions(&session));
                } else if reached_finish(&session, pos.x, pos.y, pos.z) {
                    let elapsed = elapsed_ms(session.started_at_ms);
                    let best_key = best_key(&ev.player.uuid, &session.difficulty);
                    let best = sdk.storage_get_json::<i64>(&best_key);
                    if best.is_none_or(|value| elapsed < value) {
                        let _ = sdk.storage_set_json(&best_key, &elapsed);
                    }
                    clear_guard_runtime(&sdk, &ev.player.uuid);
                    let _ = sdk.storage_set_json(
                        &instance_key(&ev.player.uuid),
                        &MazeSession::lobby(&config.lobby.dimension),
                    );
                    actions.push(message(format!(
                        "通关成功，难度 {}，用时 {}",
                        session.difficulty_label,
                        format_duration(elapsed)
                    )));
                    actions.push(Pa::SetPlayersVisible { visible: true });
                    actions.push(Pa::RemoveBossBar { id: MAZE_COMPASS_BAR_ID.to_string() });
                    actions.push(Pa::Teleport {
                        dimension: config.lobby.dimension.clone(),
                        x: config.lobby.spawn_x,
                        y: config.lobby.spawn_y,
                        z: config.lobby.spawn_z,
                        yaw: Some(config.lobby.spawn_yaw),
                        pitch: Some(config.lobby.spawn_pitch),
                    });
                }
                if !actions.is_empty() {
                    sdk.perform_all(&actions.iter().map(|a| (&ev.player, a.clone())).collect::<Vec<_>>());
                }
            })
        });

        // 宿主接线后自动生效：分段建造、罗盘、剧情房、守卫巡逻都由 tick 驱动
        ctx.register::<PlayerTickEvent>(move |api, ev, _| {
            let plugin_id = plugin_id;
            Box::pin(async move {
                let sdk = qexed_plugin_sdk::Sdk::new(api, plugin_id);
                let mut actions = continue_maze_build(&sdk, &ev.player.uuid);
                if let Some(mut more) = tick_active_maze(&sdk, &ev) {
                    actions.append(&mut more);
                }
                if !actions.is_empty() {
                    sdk.perform_all(&actions.iter().map(|a| (&ev.player, a.clone())).collect::<Vec<_>>());
                }
            })
        });

        ctx.register::<NpcMutationsEvent>(move |api, ev, _| {
            let plugin_id = plugin_id;
            Box::pin(async move {
                let sdk = qexed_plugin_sdk::Sdk::new(api, plugin_id);
                let config = load_config(&sdk);
                if !config.enable {
                    return;
                }
                ev.response.lock().unwrap().operations = vec![NpcMutationOp::Upsert {
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
                }];
            })
        });

        ctx.register::<NpcInteractEvent>(move |api, ev, _| {
            let plugin_id = plugin_id;
            Box::pin(async move {
                let p = &ev.payload;
                if p.entity.key != NPC_KEY || p.configured_event != NPC_EVENT {
                    return;
                }
                let sdk = qexed_plugin_sdk::Sdk::new(api, plugin_id);
                let config = load_config(&sdk);
                let player = qexed_plugin_api::PlayerInfo {
                    uuid: p.player.uuid.clone(),
                    username: p.player.username.clone(),
                    entity_id: p.player.entity_id,
                    dimension: p.player.dimension.clone(),
                };
                sdk.perform(&player, Pa::OpenMenu { menu: config.menu_id.clone() });
            })
        });

        ctx.register::<PlaceholdersEvent>(move |api, ev, _| {
            let plugin_id = plugin_id;
            Box::pin(async move {
                let sdk = qexed_plugin_sdk::Sdk::new(api, plugin_id);
                let Some(player) = ev.query.player.clone() else {
                    return;
                };
                let session = sdk
                    .storage_get_json::<MazeSession>(&instance_key(&player.uuid))
                    .unwrap_or_else(|| MazeSession::lobby("minecraft:overworld"));
                let current = if session.active {
                    format_duration(elapsed_ms(session.started_at_ms))
                } else {
                    "--:--".to_string()
                };
                let best = if session.active {
                    sdk.storage_get_json::<i64>(&best_key(&player.uuid, &session.difficulty))
                } else {
                    ["easy", "normal", "hard"]
                        .into_iter()
                        .filter_map(|d| sdk.storage_get_json::<i64>(&best_key(&player.uuid, d)))
                        .min()
                }
                .map(format_duration)
                .unwrap_or_else(|| "--:--".to_string());
                let mut response = ev.response.lock().unwrap();
                response.replacements = vec![
                    qexed_plugin_api::PlaceholderReplacement {
                        key: "maze_difficulty".to_string(),
                        value: if session.active { session.difficulty_label.clone() } else { "大厅".to_string() },
                    },
                    qexed_plugin_api::PlaceholderReplacement {
                        key: "maze_current_time".to_string(),
                        value: current,
                    },
                    qexed_plugin_api::PlaceholderReplacement {
                        key: "maze_best_time".to_string(),
                        value: best,
                    },
                ];
            })
        });

        ctx.register::<ConfigReloadEvent>(move |api, ev, _| {
            let plugin_id = plugin_id;
            Box::pin(async move {
                let path = ev.payload.path.replace('\\', "/");
                if !(path.ends_with("maze_runner/config.toml")
                    || path.ends_with("config.toml") && path.contains("maze"))
                {
                    return;
                }
                let sdk = qexed_plugin_sdk::Sdk::new(api, plugin_id);
                let config = load_config(&sdk);
                register_lobby_region(&sdk, &config);
                api.log(LogLevel::Info, "maze_runner config reloaded");
            })
        });
    })
});

// ─────────────────────────── 游戏流程 ───────────────────────────

fn start_maze_loading(
    sdk: &qexed_plugin_sdk::Sdk,
    config: &Config,
    player_uuid: &str,
    difficulty_id: &str,
) -> Vec<Pa> {
    if !config.enable {
        return Vec::new();
    }
    if sdk.storage_get(&build_key(player_uuid)).is_some() {
        return vec![
            loading_bar("迷宫正在生成", 0.05, "purple"),
            message("迷宫正在生成，请稍候..."),
        ];
    }
    let Some(difficulty) = config.difficulty(difficulty_id).cloned() else {
        return vec![message("未知难度，可选 easy, normal, hard")];
    };
    clear_guard_runtime(sdk, player_uuid);
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
        started_at_ms: 0,
        origin_x: instance.0,
        origin_y: instance.1,
        origin_z: instance.2,
    };
    register_instance_region(sdk, &difficulty, instance);
    let blocks = maze_blocks(&difficulty, instance, &maze);
    let build = MazeBuildJob {
        session,
        total: blocks.len(),
        written: 0,
        chunk_count: build_chunk_count(blocks.len()),
    };
    if !store_build_job(sdk, player_uuid, &build, &blocks) {
        clear_build_storage(sdk, player_uuid);
        return vec![
            Pa::RemoveBossBar { id: MAZE_LOADING_BAR_ID.to_string() },
            message("Maze generation failed, retry."),
        ];
    }
    vec![
        loading_bar(&format!("正在生成 {} 迷宫", difficulty.label), 0.01, "purple"),
        message(format!("正在生成 {} 迷宫，请稍候...", difficulty.label)),
    ]
}

fn leave_to_lobby(sdk: &qexed_plugin_sdk::Sdk, config: &Config, player_uuid: &str) -> Vec<Pa> {
    clear_build_storage(sdk, player_uuid);
    clear_guard_runtime(sdk, player_uuid);
    let _ = sdk.storage_set_json(
        &instance_key(player_uuid),
        &MazeSession::lobby(&config.lobby.dimension),
    );
    vec![
        Pa::SetPlayersVisible { visible: true },
        Pa::RemoveBossBar { id: MAZE_COMPASS_BAR_ID.to_string() },
        Pa::Teleport {
            dimension: config.lobby.dimension.clone(),
            x: config.lobby.spawn_x,
            y: config.lobby.spawn_y,
            z: config.lobby.spawn_z,
            yaw: Some(config.lobby.spawn_yaw),
            pitch: Some(config.lobby.spawn_pitch),
        },
    ]
}

fn continue_maze_build(sdk: &qexed_plugin_sdk::Sdk, player_uuid: &str) -> Vec<Pa> {
    let Some(bytes) = sdk.storage_get(&build_key(player_uuid)) else {
        return Vec::new();
    };
    let Some(mut build) = decode_build_job(&String::from_utf8_lossy(&bytes)) else {
        clear_build_storage(sdk, player_uuid);
        return vec![Pa::RemoveBossBar { id: MAZE_LOADING_BAR_ID.to_string() }];
    };
    let start = build.written.min(build.total);
    let end = (start + BUILD_BLOCKS_PER_TICK).min(build.total);
    let Some(blocks) = load_build_blocks(sdk, player_uuid, &build, start, end) else {
        clear_build_storage(sdk, player_uuid);
        return vec![
            Pa::RemoveBossBar { id: MAZE_LOADING_BAR_ID.to_string() },
            message("Maze generation data is invalid, retry."),
        ];
    };
    let block_refs: Vec<(&str, (i32, i32, i32), &str)> = blocks
        .iter()
        .map(|b| (b.dimension.as_str(), b.position, b.block.as_str()))
        .collect();
    if sdk.world_set_blocks(&block_refs) == 0 {
        return vec![loading_bar(
            &format!("{} 迷宫生成重试中", build.session.difficulty_label),
            build_progress(&build),
            "red",
        )];
    }
    build.written = end;

    if build.written < build.total {
        let progress = build_progress(&build);
        let _ = sdk.storage_set(&build_key(player_uuid), encode_build_job(&build).as_bytes());
        return vec![loading_bar(
            &format!(
                "正在生成 {} 迷宫 {}%",
                build.session.difficulty_label,
                (progress * 100.0).round() as i32
            ),
            progress,
            "purple",
        )];
    }

    let mut session = build.session;
    session.started_at_ms = now_millis();
    let _ = sdk.storage_set_json(&instance_key(player_uuid), &session);
    let _ = sdk.storage_set_json(&format!("{STARTED_PREFIX}/{player_uuid}"), &session.started_at_ms);
    clear_build_storage(sdk, player_uuid);
    if let Some(difficulty) = difficulty_by_id(sdk, &session.difficulty) {
        let origin = session.origin();
        let maze = generate_maze(&difficulty, stable_seed(player_uuid, &difficulty.id));
        initialize_runtime_features(sdk, &difficulty, origin, &maze, player_uuid);
    }
    vec![
        loading_bar(&format!("{} 迷宫生成完成", session.difficulty_label), 1.0, "green"),
        message(format!("进入 {} 迷宫", session.difficulty_label)),
        Pa::RemoveBossBar { id: MAZE_LOADING_BAR_ID.to_string() },
        Pa::SetPlayersVisible { visible: false },
        compass_item(),
        Pa::Teleport {
            dimension: session.dimension.clone(),
            x: f64::from(session.start_x) + 0.5,
            y: f64::from(session.start_y),
            z: f64::from(session.start_z) + 0.5,
            yaw: Some(0.0),
            pitch: Some(0.0),
        },
    ]
}

fn build_progress(build: &MazeBuildJob) -> f32 {
    if build.total == 0 {
        1.0
    } else {
        (build.written as f32 / build.total as f32).clamp(0.01, 1.0)
    }
}

fn respawn_actions(session: &MazeSession) -> Vec<Pa> {
    vec![
        message("已回到迷宫起点"),
        Pa::Teleport {
            dimension: session.dimension.clone(),
            x: f64::from(session.start_x) + 0.5,
            y: f64::from(session.start_y),
            z: f64::from(session.start_z) + 0.5,
            yaw: Some(0.0),
            pitch: Some(0.0),
        },
    ]
}

// ─────────────────────────── 世界构建 ───────────────────────────

fn register_lobby_region(sdk: &qexed_plugin_sdk::Sdk, config: &Config) {
    let _ = sdk.world_register_edit_region(&WorldEditRegionSpec {
        id: "maze_lobby".to_string(),
        dimension: config.lobby.dimension.clone(),
        min: (-32, config.lobby.floor_y - 1, -32),
        max: (32, config.lobby.floor_y + 8, 32),
        allow_player_break: false,
        allow_player_place: false,
        allow_plugin_write: true,
        runtime_only: true,
    });
    build_lobby(sdk, config);
}

fn build_lobby(sdk: &qexed_plugin_sdk::Sdk, config: &Config) {
    let mut sink = BatchedBlockSink::new(sdk);
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

fn register_instance_region(
    sdk: &qexed_plugin_sdk::Sdk,
    difficulty: &DifficultyConfig,
    origin: (i32, i32, i32),
) {
    let max_x = origin.0 + maze_horizontal_extent(difficulty.width) + 2;
    let max_y = origin.1 + difficulty.levels * difficulty.level_height + 8;
    let max_z = origin.2 + maze_horizontal_extent(difficulty.depth) + 2;
    let id = format!("maze_{}_{}_{}", difficulty.id, origin.0, origin.2);
    let _ = sdk.world_register_edit_region(&WorldEditRegionSpec {
        id,
        dimension: difficulty.dimension.clone(),
        min: (origin.0 - 2, origin.1 - 2, origin.2 - 2),
        max: (max_x, max_y, max_z),
        allow_player_break: false,
        allow_player_place: false,
        allow_plugin_write: true,
        runtime_only: true,
    });
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
    let _ = (width_blocks, depth_blocks);
    for level in 0..difficulty.levels {
        let base_y = origin.1 + level * difficulty.level_height;
        for x in 0..=maze_horizontal_extent(difficulty.width) {
            for z in 0..=maze_horizontal_extent(difficulty.depth) {
                sink.set_block(&difficulty.dimension, (origin.0 + x, base_y, origin.2 + z), "minecraft:deepslate_tiles");
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
                    carve_walkway_line(sink, difficulty, (center_x, center_z), (center_x + CELL_SPACING, center_z), base_y);
                }
                if cell.south {
                    carve_walkway_line(sink, difficulty, (center_x, center_z), (center_x, center_z + CELL_SPACING), base_y);
                }
            }
        }
        if difficulty.ceiling {
            for x in 0..=maze_horizontal_extent(difficulty.width) {
                for z in 0..=maze_horizontal_extent(difficulty.depth) {
                    sink.set_block(
                        &difficulty.dimension,
                        (origin.0 + x, base_y + difficulty.wall_height + 1, origin.2 + z),
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
    sink.set_block(
        &difficulty.dimension,
        (
            origin.0 + cell_center_offset(maze.finish_x),
            finish_floor_y,
            origin.2 + cell_center_offset(maze.finish_z),
        ),
        "minecraft:emerald_block",
    );
    decorate_story_rooms(sink, difficulty, origin, maze);
}

fn decorate_story_rooms(
    sink: &mut impl BlockSink,
    difficulty: &DifficultyConfig,
    origin: (i32, i32, i32),
    maze: &MazeData,
) {
    if difficulty.levels > 1 {
        return;
    }
    let route = maze_route(difficulty, maze);
    for (index, point) in story_points(&route).into_iter().enumerate() {
        let base_y = origin.1 + point.level * difficulty.level_height;
        let center_x = origin.0 + cell_center_offset(point.x);
        let center_z = origin.2 + cell_center_offset(point.z);
        let marker = match index {
            0 => "minecraft:chiseled_stone_bricks",
            1 => "minecraft:cracked_stone_bricks",
            _ => "minecraft:gold_block",
        };
        for dx in -1..=1 {
            for dz in -1..=1 {
                sink.set_block(&difficulty.dimension, (center_x + dx, base_y, center_z + dz), marker);
            }
        }
    }
}

fn fill_wall_column(sink: &mut impl BlockSink, difficulty: &DifficultyConfig, x: i32, base_y: i32, z: i32) {
    for y in 1..=difficulty.wall_height {
        sink.set_block(&difficulty.dimension, (x, base_y + y, z), "minecraft:stone_bricks");
    }
}

fn carve_column(sink: &mut impl BlockSink, difficulty: &DifficultyConfig, x: i32, base_y: i32, z: i32) {
    for y in 1..=difficulty.wall_height {
        sink.set_block(&difficulty.dimension, (x, base_y + y, z), "minecraft:air");
    }
}

fn carve_room(sink: &mut impl BlockSink, difficulty: &DifficultyConfig, x: i32, base_y: i32, z: i32) {
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
            sink.set_block(&difficulty.dimension, (x + dx, floor_y + air, z + dz), "minecraft:air");
        }
        if rise > 0 && rise < difficulty.level_height as usize {
            sink.set_block(&difficulty.dimension, (x + dx, floor_y, z + dz), "minecraft:deepslate_tiles");
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

/// 直接写世界的批量 sink（大厅等一次性构建用）。
struct BatchedBlockSink<'a> {
    sdk: &'a qexed_plugin_sdk::Sdk<'a>,
    blocks: Vec<MazeBlock>,
}

impl<'a> BatchedBlockSink<'a> {
    fn new(sdk: &'a qexed_plugin_sdk::Sdk<'a>) -> Self {
        Self { sdk, blocks: Vec::with_capacity(WORLD_EDIT_BATCH_LIMIT) }
    }

    fn flush(&mut self) {
        if self.blocks.is_empty() {
            return;
        }
        let refs: Vec<(&str, (i32, i32, i32), &str)> = self
            .blocks
            .iter()
            .map(|b| (b.dimension.as_str(), b.position, b.block.as_str()))
            .collect();
        let _ = self.sdk.world_set_blocks(&refs);
        self.blocks.clear();
    }
}

impl BlockSink for BatchedBlockSink<'_> {
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

// ─────────────────────────── 迷宫生成 ───────────────────────────

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
    (x - (f64::from(session.finish_x) + 0.5)).abs() <= 1.25
        && (y - f64::from(session.finish_y)).abs() <= 2.5
        && (z - (f64::from(session.finish_z) + 0.5)).abs() <= 1.25
}

// ─────────────────────────── 罗盘 / 剧情 / 守卫 ───────────────────────────

fn tick_active_maze(sdk: &qexed_plugin_sdk::Sdk, ev: &PlayerTickEvent) -> Option<Vec<Pa>> {
    let session = sdk.storage_get_json::<MazeSession>(&instance_key(&ev.player.uuid))?;
    if !session.active || ev.dimension != session.dimension {
        return None;
    }
    let config = load_config(sdk);
    let difficulty = config.difficulty(&session.difficulty)?.clone();
    let origin = if session.origin_x == 0 && session.origin_z == 0 {
        instance_origin(&config, &ev.player.uuid, &difficulty)
    } else {
        session.origin()
    };
    let maze = generate_maze(&difficulty, stable_seed(&ev.player.uuid, &difficulty.id));
    let route = maze_route(&difficulty, &maze);
    let mut actions = vec![compass_bar(&session, &ev.position)];
    actions.extend(trigger_story_rooms(
        sdk,
        &ev.player.uuid,
        &difficulty,
        origin,
        &route,
        ev.position.x,
        ev.position.y,
        ev.position.z,
    ));
    update_maze_guards(
        sdk,
        &ev.player.uuid,
        &difficulty,
        &route,
        &ev.position,
        ev.tick_millis,
    );
    Some(actions)
}

fn initialize_runtime_features(
    sdk: &qexed_plugin_sdk::Sdk,
    difficulty: &DifficultyConfig,
    origin: (i32, i32, i32),
    maze: &MazeData,
    player_uuid: &str,
) {
    let route = maze_route(difficulty, maze);
    let _ = sdk.storage_delete(&story_key(player_uuid));
    initialize_maze_guards(sdk, player_uuid, difficulty, origin, &route);
}

fn compass_item() -> Pa {
    Pa::GiveItem {
        item: "minecraft:compass".to_string(),
        count: 1,
        name: "出口罗盘".to_string(),
        lore: vec!["罗盘条会显示终点方向".to_string()],
        enchantments: Vec::new(),
        plugin_enchantments: Vec::new(),
    }
}

fn compass_bar(session: &MazeSession, position: &PlayerPositionPayload) -> Pa {
    let dx = f64::from(session.finish_x) + 0.5 - position.x;
    let dz = f64::from(session.finish_z) + 0.5 - position.z;
    let distance = (dx * dx + dz * dz).sqrt();
    let direction = compass_direction(dx, dz, position.yaw);
    Pa::BossBar {
        id: MAZE_COMPASS_BAR_ID.to_string(),
        title: format!("出口方向: {direction}  距离: {}m", distance.round() as i32),
        progress: (1.0 - (distance / 180.0) as f32).clamp(0.05, 1.0),
        color: "yellow".to_string(),
        overlay: "progress".to_string(),
    }
}

fn compass_direction(dx: f64, dz: f64, yaw: f32) -> &'static str {
    let target = dx.atan2(dz).to_degrees();
    let mut relative = target - f64::from(yaw);
    while relative <= -180.0 {
        relative += 360.0;
    }
    while relative > 180.0 {
        relative -= 360.0;
    }
    match relative {
        value if value.abs() <= 22.5 => "正前方",
        value if value > 22.5 && value <= 67.5 => "右前方",
        value if value > 67.5 && value <= 112.5 => "右侧",
        value if value > 112.5 && value <= 157.5 => "右后方",
        value if value < -22.5 && value >= -67.5 => "左前方",
        value if value < -67.5 && value >= -112.5 => "左侧",
        value if value < -112.5 && value >= -157.5 => "左后方",
        _ => "后方",
    }
}

fn trigger_story_rooms(
    sdk: &qexed_plugin_sdk::Sdk,
    player_uuid: &str,
    difficulty: &DifficultyConfig,
    origin: (i32, i32, i32),
    route: &[MazePoint],
    player_x: f64,
    player_y: f64,
    player_z: f64,
) -> Vec<Pa> {
    if route.len() < 5 {
        return Vec::new();
    }
    let mut seen = sdk
        .storage_get_json::<StoryState>(&story_key(player_uuid))
        .unwrap_or_default();
    let stories = story_points(route);
    let mut actions = Vec::new();
    for (index, point) in stories.into_iter().enumerate() {
        if seen.triggered.contains(&(index as u8)) {
            continue;
        }
        let (x, y, z) = world_cell_center(difficulty, origin, point);
        if distance_sq((player_x, player_y, player_z), (x, y, z)) <= 5.0 * 5.0 {
            seen.triggered.push(index as u8);
            actions.push(message(story_text(index)));
        }
    }
    if !actions.is_empty() {
        let _ = sdk.storage_set_json(&story_key(player_uuid), &seen);
    }
    actions
}

fn story_points(route: &[MazePoint]) -> Vec<MazePoint> {
    [route.len() / 4, route.len() / 2, route.len() * 3 / 4]
        .into_iter()
        .filter_map(|index| route.get(index).copied())
        .collect()
}

fn story_text(index: usize) -> &'static str {
    match index {
        0 => "墙上刻着旧字：别相信第一条直路。",
        1 => "你听见远处有铁链拖过石面的声音。",
        _ => "空气变冷了，出口应该已经不远。",
    }
}

fn initialize_maze_guards(
    sdk: &qexed_plugin_sdk::Sdk,
    player_uuid: &str,
    difficulty: &DifficultyConfig,
    origin: (i32, i32, i32),
    route: &[MazePoint],
) {
    clear_guard_runtime(sdk, player_uuid);
    let count = guard_count(difficulty);
    if count == 0 || route.len() < 6 {
        return;
    }
    for index in 0..count {
        let start_index = ((index + 1) * route.len() / (count + 2)).min(route.len() - 2);
        let end_index = (start_index + route.len().max(8) / 8).min(route.len() - 1);
        let route_points = route[start_index..=end_index]
            .iter()
            .copied()
            .map(|point| world_cell_center(difficulty, origin, point))
            .collect::<Vec<_>>();
        if route_points.len() < 2 {
            continue;
        }
        let state = GuardState {
            index: index as u8,
            route: route_points,
            cursor: 0,
            forward: true,
            x: 0.0,
            y: 0.0,
            z: 0.0,
            yaw: 0.0,
            alert: false,
        }
        .spawned_at_start();
        let key = guard_key(player_uuid, index);
        let _ = sdk.entity_upsert(&guard_entity(&key, difficulty, &state, false));
        let _ = sdk.storage_set_json(&guard_state_key(player_uuid, index), &state);
    }
}

fn update_maze_guards(
    sdk: &qexed_plugin_sdk::Sdk,
    player_uuid: &str,
    difficulty: &DifficultyConfig,
    _route: &[MazePoint],
    player: &PlayerPositionPayload,
    tick_millis: u64,
) {
    for index in 0..guard_count(difficulty) {
        let key = guard_key(player_uuid, index);
        let Some(mut state) = sdk.storage_get_json::<GuardState>(&guard_state_key(player_uuid, index)) else {
            continue;
        };
        let detected = guard_sees_player(&state, player);
        if detected != state.alert {
            state.alert = detected;
            let _ = sdk.entity_upsert(&guard_entity(&key, difficulty, &state, detected));
        }
        if !detected {
            state.patrol_step(guard_speed(difficulty) * tick_millis as f64 / 50.0);
            let _ = sdk.entity_move(
                &key,
                &difficulty.dimension,
                NpcPosition { x: state.x, y: state.y, z: state.z, yaw: state.yaw, pitch: 0.0 },
            );
        }
        let _ = sdk.storage_set_json(&guard_state_key(player_uuid, index), &state);
    }
}

fn guard_entity(
    key: &str,
    difficulty: &DifficultyConfig,
    state: &GuardState,
    alert: bool,
) -> RuntimeEntitySpec {
    RuntimeEntitySpec {
        key: key.to_string(),
        dimension: difficulty.dimension.clone(),
        entity_type: "minecraft:zombie".to_string(),
        x: state.x,
        y: state.y,
        z: state.z,
        yaw: state.yaw,
        pitch: 0.0,
        display_name: "迷宫巡逻者".to_string(),
        ai: if alert { "hostile_melee".to_string() } else { "plugin:maze_patrol".to_string() },
        ai_params_json: if alert {
            r#"{"attack_damage":3.0,"follow_range":10.0,"attack_range":1.8}"#.to_string()
        } else {
            "{}".to_string()
        },
        auto_jump: true,
    }
}

fn guard_sees_player(state: &GuardState, player: &PlayerPositionPayload) -> bool {
    let dx = player.x - state.x;
    let dz = player.z - state.z;
    let distance = (dx * dx + dz * dz).sqrt();
    if distance > GUARD_DETECT_DISTANCE || (player.y - state.y).abs() > 2.5 || distance <= 0.1 {
        return false;
    }
    let facing = yaw_vector(state.yaw);
    let dot = (facing.0 * dx + facing.1 * dz) / distance;
    dot >= GUARD_DETECT_DOT
}

fn yaw_vector(yaw: f32) -> (f64, f64) {
    let radians = f64::from(yaw).to_radians();
    (-radians.sin(), radians.cos())
}

fn guard_count(difficulty: &DifficultyConfig) -> usize {
    match difficulty.id.as_str() {
        "hard" => 3,
        "normal" => 1,
        _ => 0,
    }
}

fn guard_speed(difficulty: &DifficultyConfig) -> f64 {
    if difficulty.id == "hard" { 0.18 } else { 0.14 }
}

fn clear_guard_runtime(sdk: &qexed_plugin_sdk::Sdk, player_uuid: &str) {
    for index in 0..4 {
        let _ = sdk.entity_remove(&guard_key(player_uuid, index));
        let _ = sdk.storage_delete(&guard_state_key(player_uuid, index));
    }
    let _ = sdk.storage_delete(&story_key(player_uuid));
}

fn guard_key(player_uuid: &str, index: usize) -> String {
    format!("{MAZE_GUARD_PREFIX}/{player_uuid}/{index}")
}

fn guard_state_key(player_uuid: &str, index: usize) -> String {
    format!("{MAZE_GUARD_PREFIX}/state/{player_uuid}/{index}")
}

fn story_key(player_uuid: &str) -> String {
    format!("{MAZE_STORY_PREFIX}/{player_uuid}")
}

fn difficulty_by_id(sdk: &qexed_plugin_sdk::Sdk, id: &str) -> Option<DifficultyConfig> {
    load_config(sdk).difficulties.into_iter().find(|d| d.id == id)
}

fn distance_sq(a: (f64, f64, f64), b: (f64, f64, f64)) -> f64 {
    let dx = a.0 - b.0;
    let dy = a.1 - b.1;
    let dz = a.2 - b.2;
    dx * dx + dy * dy + dz * dz
}

fn world_cell_center(
    difficulty: &DifficultyConfig,
    origin: (i32, i32, i32),
    point: MazePoint,
) -> (f64, f64, f64) {
    (
        f64::from(origin.0 + cell_center_offset(point.x)) + 0.5,
        f64::from(origin.1 + point.level * difficulty.level_height) + 1.0,
        f64::from(origin.2 + cell_center_offset(point.z)) + 0.5,
    )
}

// ─────────────────────────── 路由 / 工具 / 状态 ───────────────────────────

fn maze_route(difficulty: &DifficultyConfig, maze: &MazeData) -> Vec<MazePoint> {
    let total = (difficulty.levels * difficulty.width * difficulty.depth).max(1) as usize;
    let start = cell_index(difficulty, 0, 0, 0);
    let finish = cell_index(difficulty, maze.finish_level, maze.finish_x, maze.finish_z);
    let mut parent = vec![usize::MAX; total];
    parent[start] = start;
    let mut queue = std::collections::VecDeque::new();
    queue.push_back((0, 0, 0));
    while let Some((level, x, z)) = queue.pop_front() {
        let current = cell_index(difficulty, level, x, z);
        if current == finish {
            break;
        }
        for (next_level, next_x, next_z) in maze_neighbors(difficulty, &maze.cells, level, x, z) {
            let next = cell_index(difficulty, next_level, next_x, next_z);
            if parent[next] == usize::MAX {
                parent[next] = current;
                queue.push_back((next_level, next_x, next_z));
            }
        }
    }
    if parent[finish] == usize::MAX {
        return vec![MazePoint { level: 0, x: 0, z: 0 }];
    }
    let mut route = Vec::new();
    let mut current = finish;
    loop {
        route.push(point_from_index(difficulty, current));
        if current == start {
            break;
        }
        current = parent[current];
    }
    route.reverse();
    route
}

fn point_from_index(difficulty: &DifficultyConfig, index: usize) -> MazePoint {
    let index = index as i32;
    let layer = difficulty.width * difficulty.depth;
    let level = index / layer;
    let local = index % layer;
    MazePoint {
        level,
        x: local % difficulty.width,
        z: local / difficulty.width,
    }
}

#[derive(Debug, Clone, Copy)]
struct MazePoint {
    level: i32,
    x: i32,
    z: i32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct StoryState {
    triggered: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GuardState {
    index: u8,
    route: Vec<(f64, f64, f64)>,
    cursor: usize,
    forward: bool,
    x: f64,
    y: f64,
    z: f64,
    yaw: f32,
    alert: bool,
}

impl GuardState {
    fn spawned_at_start(mut self) -> Self {
        let first = self.route[0];
        let second = self.route.get(1).copied().unwrap_or(first);
        self.x = first.0;
        self.y = first.1;
        self.z = first.2;
        self.yaw = yaw_between(first, second);
        self
    }

    fn patrol_step(&mut self, distance: f64) {
        if self.route.len() < 2 {
            return;
        }
        let target_index = self.next_cursor();
        let target = self.route[target_index];
        let dx = target.0 - self.x;
        let dy = target.1 - self.y;
        let dz = target.2 - self.z;
        let remaining = (dx * dx + dy * dy + dz * dz).sqrt();
        self.yaw = yaw_between((self.x, self.y, self.z), target);
        if remaining <= GUARD_REACH_DISTANCE || remaining <= distance {
            self.x = target.0;
            self.y = target.1;
            self.z = target.2;
            self.cursor = target_index;
            if self.cursor == 0 || self.cursor + 1 == self.route.len() {
                self.forward = !self.forward;
            }
            return;
        }
        let scale = distance / remaining;
        self.x += dx * scale;
        self.y += dy * scale;
        self.z += dz * scale;
    }

    fn next_cursor(&self) -> usize {
        if self.forward {
            (self.cursor + 1).min(self.route.len() - 1)
        } else {
            self.cursor.saturating_sub(1)
        }
    }
}

fn yaw_between(from: (f64, f64, f64), to: (f64, f64, f64)) -> f32 {
    let dx = to.0 - from.0;
    let dz = to.2 - from.2;
    (-dx.atan2(dz).to_degrees()) as f32
}

fn elapsed_ms(started_at_ms: i64) -> i64 {
    now_millis().saturating_sub(started_at_ms).max(0)
}

fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn format_duration(ms: i64) -> String {
    let seconds = ms.max(0) / 1000;
    let minutes = seconds / 60;
    format!("{minutes:02}:{:02}", seconds % 60)
}

fn message(text: impl Into<String>) -> Pa {
    Pa::SystemMessage {
        text: text.into(),
        translate: String::new(),
        with: Vec::new(),
        overlay: false,
    }
}

fn loading_bar(title: &str, progress: f32, color: &str) -> Pa {
    Pa::BossBar {
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
        value ^= u64::from(byte);
        value = value.wrapping_mul(0x100000001b3);
    }
    value
}

fn instance_key(uuid: &str) -> String {
    format!("{INSTANCE_PREFIX}/{uuid}")
}

fn build_key(uuid: &str) -> String {
    format!("{BUILD_PREFIX}/{uuid}")
}

fn build_chunk_key(uuid: &str, chunk: usize) -> String {
    format!("{BUILD_PREFIX}/{uuid}/chunk/{chunk}")
}

fn clear_build_storage(sdk: &qexed_plugin_sdk::Sdk, uuid: &str) {
    let chunk_count = sdk
        .storage_get(&build_key(uuid))
        .and_then(|bytes| decode_build_job(&String::from_utf8_lossy(&bytes)))
        .map(|build| build.chunk_count)
        .unwrap_or(MAX_STALE_BUILD_STORAGE_CHUNKS);
    for chunk in 0..chunk_count.min(MAX_STALE_BUILD_STORAGE_CHUNKS) {
        let _ = sdk.storage_delete(&build_chunk_key(uuid, chunk));
    }
    let _ = sdk.storage_delete(&build_key(uuid));
}

fn best_key(uuid: &str, difficulty: &str) -> String {
    format!("{BEST_PREFIX}/{difficulty}/{uuid}")
}

fn load_config(sdk: &qexed_plugin_sdk::Sdk) -> Config {
    sdk.read_config()
        .ok()
        .flatten()
        .and_then(|contents| {
            contents
                .parse::<toml_edit::DocumentMut>()
                .ok()
                .and_then(|doc| qexed_toml::from_document::<Config>(doc).ok())
        })
        .unwrap_or_default()
}

// ─────────────────────────── 数据结构 ───────────────────────────

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
    #[serde(default)]
    origin_x: i32,
    #[serde(default)]
    origin_y: i32,
    #[serde(default)]
    origin_z: i32,
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
            origin_x: 0,
            origin_y: 0,
            origin_z: 0,
        }
    }

    fn origin(&self) -> (i32, i32, i32) {
        (self.origin_x, self.origin_y, self.origin_z)
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
        self.difficulties.iter().find(|difficulty| difficulty.id == id)
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
    Some(MazeBuildJob { session, total, written, chunk_count })
}

fn store_build_job(
    sdk: &qexed_plugin_sdk::Sdk,
    uuid: &str,
    job: &MazeBuildJob,
    blocks: &[MazeBlock],
) -> bool {
    if !sdk.storage_set(&build_key(uuid), encode_build_job(job).as_bytes()) {
        return false;
    }
    for (chunk_index, chunk) in blocks.chunks(BUILD_BLOCKS_PER_STORAGE_CHUNK).enumerate() {
        if !sdk.storage_set(
            &build_chunk_key(uuid, chunk_index),
            encode_build_blocks(chunk).as_bytes(),
        ) {
            return false;
        }
    }
    true
}

fn load_build_blocks(
    sdk: &qexed_plugin_sdk::Sdk,
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
        let bytes = sdk.storage_get(&build_chunk_key(uuid, chunk_index))?;
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
    for value in [
        session.difficulty.as_str(),
        session.difficulty_label.as_str(),
        session.dimension.as_str(),
        &session.start_x.to_string(),
        &session.start_y.to_string(),
        &session.start_z.to_string(),
        &session.finish_x.to_string(),
        &session.finish_y.to_string(),
        &session.finish_z.to_string(),
        &session.min_y.to_string(),
        &session.started_at_ms.to_string(),
        &session.origin_x.to_string(),
        &session.origin_y.to_string(),
        &session.origin_z.to_string(),
    ] {
        encoded.push('\t');
        encoded.push_str(value);
    }
}

fn decode_session(encoded: &str) -> Option<MazeSession> {
    let mut parts = encoded.split('\t');
    let active = parts.next()? == "1";
    let difficulty = parts.next()?.to_string();
    let difficulty_label = parts.next()?.to_string();
    let dimension = parts.next()?.to_string();
    let start_x = parts.next()?.parse().ok()?;
    let start_y = parts.next()?.parse().ok()?;
    let start_z = parts.next()?.parse().ok()?;
    let finish_x = parts.next()?.parse().ok()?;
    let finish_y = parts.next()?.parse().ok()?;
    let finish_z = parts.next()?.parse().ok()?;
    let min_y = parts.next()?.parse().ok()?;
    let started_at_ms = parts.next()?.parse().ok()?;
    Some(MazeSession {
        active,
        difficulty,
        difficulty_label,
        dimension,
        start_x,
        start_y,
        start_z,
        finish_x,
        finish_y,
        finish_z,
        min_y,
        started_at_ms,
        origin_x: parts.next().and_then(|part| part.parse().ok()).unwrap_or(0),
        origin_y: parts.next().and_then(|part| part.parse().ok()).unwrap_or(0),
        origin_z: parts.next().and_then(|part| part.parse().ok()).unwrap_or(0),
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

    #[test]
    fn default_difficulties_generate_long_hard_routes() {
        let difficulty = default_difficulties()
            .into_iter()
            .find(|d| d.id == "hard")
            .expect("hard difficulty exists");
        let min_distance = (difficulty.width * difficulty.depth * difficulty.levels / 3).max(1);
        for seed in 1..=32u64 {
            let maze = generate_maze(&difficulty, seed);
            assert_eq!(maze.finish_level, difficulty.levels - 1);
            assert!(
                maze.finish_distance >= min_distance,
                "hard maze route too short: seed={seed}, distance={}, min={min_distance}",
                maze.finish_distance
            );
        }
    }

    #[test]
    fn easy_single_level_finish_reachable() {
        let difficulty = default_difficulties()
            .into_iter()
            .find(|d| d.id == "easy")
            .expect("easy difficulty exists");
        for seed in 1..=16u64 {
            let maze = generate_maze(&difficulty, seed);
            let route = maze_route(&difficulty, &maze);
            let last = route.last().expect("route nonempty");
            assert_eq!(last.level, maze.finish_level);
            assert_eq!(last.x, maze.finish_x);
            assert_eq!(last.z, maze.finish_z);
        }
    }

    #[test]
    fn build_job_roundtrip() {
        let job = MazeBuildJob {
            session: MazeSession::lobby("qexed:maze_lobby"),
            total: 8192,
            written: 1024,
            chunk_count: 2,
        };
        let decoded = decode_build_job(&encode_build_job(&job)).expect("decode ok");
        assert_eq!(decoded.total, 8192);
        assert_eq!(decoded.written, 1024);
        assert_eq!(decoded.chunk_count, 2);
    }

    #[test]
    fn build_blocks_roundtrip() {
        let blocks = vec![
            MazeBlock {
                dimension: "qexed:maze_easy".to_string(),
                position: (1, -52, 2),
                block: "minecraft:air".to_string(),
            },
            MazeBlock {
                dimension: "qexed:maze_easy".to_string(),
                position: (3, -51, 4),
                block: "minecraft:stone_bricks".to_string(),
            },
        ];
        let decoded = decode_build_blocks(&encode_build_blocks(&blocks)).expect("decode ok");
        assert_eq!(decoded.len(), 2);
        assert_eq!(decoded[1].position, (3, -51, 4));
        assert_eq!(decoded[1].block, "minecraft:stone_bricks");
    }

    #[test]
    fn guard_patrol_reverses_at_route_ends() {
        let state = GuardState {
            index: 0,
            route: vec![(0.0, 0.0, 0.0), (5.0, 0.0, 0.0)],
            cursor: 0,
            forward: true,
            x: 0.0,
            y: 0.0,
            z: 0.0,
            yaw: 0.0,
            alert: false,
        }
        .spawned_at_start();
        let mut state = state;
        for _ in 0..10 {
            state.patrol_step(0.5);
        }
        assert!((state.x - 5.0).abs() < 0.6, "far end x={}", state.x);
        assert!(!state.forward, "should reverse at far end");
        for _ in 0..10 {
            state.patrol_step(0.5);
        }
        assert!(state.x.abs() < 0.6, "back at start x={}", state.x);
        assert!(state.forward, "should reverse again at start");
    }
}

use qexed_plugin_sdk::entity_upsert;
use qexed_plugin_sdk::{
    ConfigReloadPayload, NpcInteractPayload, NpcMutationOp, NpcMutationResponse, NpcUpsert,
    PlaceholderQuery, PlaceholderReplacement, PlaceholderResponse, PlayerAction, PlayerMovePayload,
    PlayerPayload, PlayerPayloadOwned, PlayerTickPayload, PluginCommandDefinition,
    PluginCommandQuery, PluginCommandResponse, PluginManifest, RuntimeEntity, WorldEditRegion,
    config_load_or_create, config_read_to_string, storage_delete, storage_get_typed,
    storage_set_typed, time_millis, world_register_edit_region, world_set_blocks,
};
use serde::{Deserialize, Deserializer, Serialize};

qexed_plugin_sdk::qexed_plugin_memory!();
qexed_plugin_sdk::qexed_plugin_manifest!(PluginManifest {
    id: "qexed.parkour".to_string(),
    version: "0.1.0".to_string(),
    depends: Vec::new(),
    optional_depends: Vec::new(),
    load_after: Vec::new(),
    services: Vec::new(),
});

const CONFIG_PATH: &str = "config.toml";
const COMMAND_NAME: &str = "parkour";
const MENU_ID: &str = "parkour";
const NPC_KEY: &str = "parkour.entry";
const NPC_EVENT: &str = "parkour.menu";
const BUILD_BLOCKS_PER_TICK: usize = 1800;
const BUILD_BLOCKS_PER_PAGE: usize = BUILD_BLOCKS_PER_TICK;
const COURSE_CLEAR_RADIUS: i32 = 8;
const MIXED_GAP_MAX: usize = 8;
const LEADERBOARD_ROWS: usize = 10;
const DAY_MS: i64 = 24 * 60 * 60 * 1000;
const LOCAL_TIME_OFFSET_MS: i64 = 8 * 60 * 60 * 1000;

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_priority() -> i32 {
    250
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_init() {
    let _ = config_load_or_create(CONFIG_PATH, DEFAULT_CONFIG);
    let config = load_config();
    register_lobby_region(&config);
    refresh_leaderboard_display(&config, true);
    qexed_plugin_sdk::log("parkour initialized");
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_config_reload(ptr: i32, len: i32) {
    let Some(payload) =
        (unsafe { qexed_plugin_sdk::decode_payload::<ConfigReloadPayload>(ptr, len) })
    else {
        return;
    };
    let path = payload.path.replace('\\', "/");
    if path.ends_with("parkour/config.toml") || path.ends_with(CONFIG_PATH) {
        let config = load_config();
        register_lobby_region(&config);
        refresh_leaderboard_display(&config, true);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_player_join(ptr: i32, len: i32) {
    let Some(player) = (unsafe { qexed_plugin_sdk::decode_payload::<PlayerPayload>(ptr, len) })
    else {
        return;
    };
    let config = load_config();
    remember_player_name(&player.uuid, &player.username);
    if !storage_get_typed::<PlayerProgress>(&progress_key(&player.uuid)).is_some() {
        let _ = storage_set_typed(&progress_key(&player.uuid), &PlayerProgress::default());
    }
    let _ = storage_set_typed(&session_key(&player.uuid), &ParkourSession::lobby(&config));
    maybe_refresh_daily_leaderboard(&config);
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_player_leave(ptr: i32, len: i32) {
    let Some(player) = (unsafe { qexed_plugin_sdk::decode_payload::<PlayerPayload>(ptr, len) })
    else {
        return;
    };
    clear_runtime(&player.uuid);
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_commands(_ptr: i32, _len: i32) -> i64 {
    qexed_plugin_sdk::response_ptr_len(&PluginCommandDefinition {
        name: COMMAND_NAME.to_string(),
        description_key: "qexed.plugin.parkour.command.description".to_string(),
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
    let mut parts = payload.argument.split_whitespace();
    let response = match parts.next().unwrap_or_default() {
        "" | "menu" => PluginCommandResponse {
            handled: true,
            actions: vec![PlayerAction::OpenMenu {
                menu: config.menu_id.clone(),
            }],
        },
        "race" | "match" => start_run(
            &config,
            &payload.player.uuid,
            CourseMode::Race,
            SeedKind::Weekly,
            parts.next(),
        ),
        "casual" | "free" => start_run(
            &config,
            &payload.player.uuid,
            CourseMode::Casual,
            SeedKind::Player,
            parts.next(),
        ),
        "weekly" => start_run(
            &config,
            &payload.player.uuid,
            CourseMode::Weekly,
            SeedKind::Weekly,
            parts.next(),
        ),
        "exam" => start_exam(&config, &payload.player.uuid, parts.next()),
        "practice" => start_practice(&config, &payload.player.uuid, parts.next()),
        "custom" => {
            let seed = parts.next().unwrap_or_default();
            start_custom(&config, &payload.player.uuid, seed)
        }
        "leave" | "lobby" | "spawn" => leave_to_lobby(&config, &payload.player.uuid),
        "leaderboard" | "rankings" | "top" => {
            refresh_leaderboard_display(&config, true);
            PluginCommandResponse {
                handled: true,
                actions: vec![message("跑酷排行榜已刷新。")],
            }
        }
        "status" | "progress" => status_response(&config, &payload.player),
        "stats" => stats_response(&config, &payload.player),
        _ => PluginCommandResponse {
            handled: true,
            actions: vec![message(
                "用法: /parkour race|casual|weekly|exam|practice [类型]|custom <种子>|leaderboard|status|stats|leave",
            )],
        },
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
                name: "Parkour".to_string(),
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
pub extern "C" fn qexed_plugin_player_tick(ptr: i32, len: i32) -> i64 {
    let Some(payload) =
        (unsafe { qexed_plugin_sdk::decode_payload::<PlayerTickPayload>(ptr, len) })
    else {
        return qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse::default());
    };
    let config = load_config();
    remember_player_name(&payload.player.uuid, &payload.player.username);
    maybe_refresh_daily_leaderboard(&config);
    let mut actions = continue_build(&config, &payload.player.uuid).actions;
    if let Some(mut session_actions) = tick_session(&config, &payload) {
        actions.append(&mut session_actions);
    }
    qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse {
        handled: !actions.is_empty(),
        actions,
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
    let actions = update_session_position(
        &config,
        &payload.player.uuid,
        &payload.dimension,
        payload.position.x,
        payload.position.y,
        payload.position.z,
    );
    qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse {
        handled: !actions.is_empty(),
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
    let config = load_config();
    let session = storage_get_typed::<ParkourSession>(&session_key(&player.uuid))
        .unwrap_or_else(|| ParkourSession::lobby(&config));
    let progress = progress(&player.uuid);
    let title = display_title(&player.uuid, &progress);
    let elapsed = if session.active {
        format_duration(time_millis().saturating_sub(session.started_at_ms))
    } else {
        "--:--".to_string()
    };
    let best = best_for_mode(&player.uuid, &session.mode).unwrap_or(0);
    qexed_plugin_sdk::response_ptr_len(&PlaceholderResponse {
        replacements: vec![
            replacement("parkour_title", title),
            replacement("parkour_xp", progress.xp.to_string()),
            replacement("parkour_level", progress.level.to_string()),
            replacement("parkour_mode", session.mode.label().to_string()),
            replacement("parkour_course", session.course_label.clone()),
            replacement("parkour_time", elapsed),
            replacement("parkour_best", format_best(best)),
            replacement("parkour_current_jump", current_jump_label(&session)),
            replacement("parkour_jump_index", jump_index_text(&session)),
            replacement("parkour_jump_total", jump_total_text(&session)),
            replacement("parkour_jump_sequence", jump_sequence_text(&session, 12)),
            replacement(
                "parkour_rank",
                rank_for(&player.uuid, &session.mode)
                    .map(|rank| rank.to_string())
                    .unwrap_or_else(|| "-".to_string()),
            ),
        ],
    })
}

fn start_exam(config: &Config, player_uuid: &str, target: Option<&str>) -> PluginCommandResponse {
    let target = target.unwrap_or("novice");
    let Some(exam) = config
        .exams
        .iter()
        .find(|exam| exam.id.eq_ignore_ascii_case(target))
        .or_else(|| config.exams.first())
    else {
        return PluginCommandResponse {
            handled: true,
            actions: vec![message("未配置考核。")],
        };
    };
    start_run_with_profile(
        config,
        player_uuid,
        CourseMode::Exam,
        SeedKind::Exam(exam.id.clone()),
        Some(exam.course_id.as_str()),
        Some(exam),
        None,
    )
}

fn start_practice(
    config: &Config,
    player_uuid: &str,
    target: Option<&str>,
) -> PluginCommandResponse {
    let practice_id = target.unwrap_or("three_block");
    let Some(practice) = config
        .practice
        .iter()
        .find(|practice| practice.id.eq_ignore_ascii_case(practice_id))
        .or_else(|| config.practice.first())
    else {
        return PluginCommandResponse {
            handled: true,
            actions: vec![message("未配置练习项目。")],
        };
    };
    start_run_with_profile(
        config,
        player_uuid,
        CourseMode::Practice,
        SeedKind::Practice(practice.id.clone()),
        Some(practice.course_id.as_str()),
        None,
        Some(practice.id.as_str()),
    )
}

fn start_custom(config: &Config, player_uuid: &str, seed: &str) -> PluginCommandResponse {
    if seed.trim().is_empty() {
        return PluginCommandResponse {
            handled: true,
            actions: vec![message("用法: /parkour custom <种子>")],
        };
    }
    start_run(
        config,
        player_uuid,
        CourseMode::Custom,
        SeedKind::Custom(seed.trim().to_string()),
        None,
    )
}

fn start_run(
    config: &Config,
    player_uuid: &str,
    mode: CourseMode,
    seed: SeedKind,
    course_id: Option<&str>,
) -> PluginCommandResponse {
    start_run_with_profile(config, player_uuid, mode, seed, course_id, None, None)
}

fn start_run_with_profile(
    config: &Config,
    player_uuid: &str,
    mode: CourseMode,
    seed_kind: SeedKind,
    course_id: Option<&str>,
    exam: Option<&ExamConfig>,
    practice_id: Option<&str>,
) -> PluginCommandResponse {
    if !config.enable {
        return PluginCommandResponse {
            handled: true,
            actions: vec![message("跑酷玩法暂未启用。")],
        };
    }
    clear_runtime(player_uuid);
    let Some(course) = select_course(config, course_id, mode, player_uuid) else {
        return PluginCommandResponse {
            handled: true,
            actions: vec![message("未配置赛道。")],
        };
    };
    if mode == CourseMode::Casual && progress(player_uuid).level < course.min_level {
        return PluginCommandResponse {
            handled: true,
            actions: vec![message(format!(
                "等级不足，需要 {} 级才能进入 {}。",
                course.min_level, course.label
            ))],
        };
    }

    let seed = seed_kind.value(player_uuid);
    let origin = instance_origin(config, player_uuid, &mode);
    register_course_region(config, origin, &course);
    let generated = generate_course(config, &course, origin, seed, mode, practice_id);
    let mut session = ParkourSession::new(&config, &course, origin, seed, mode, exam);
    session.finish_z = generated.finish_z;
    session.checkpoints = generated.checkpoints;
    session.jump_sequence = generated.jump_sequence;
    let build = BuildJob {
        session,
        total_blocks: generated.blocks.len(),
        page_count: build_page_count(generated.blocks.len()),
        written: 0,
    };
    if !store_build_pages(player_uuid, generated.blocks)
        || !storage_set_typed(&build_key(player_uuid), &build)
    {
        clear_runtime(player_uuid);
        return PluginCommandResponse {
            handled: true,
            actions: vec![message("赛道生成失败。")],
        };
    }

    PluginCommandResponse {
        handled: true,
        actions: vec![
            PlayerAction::SetPlayersVisible {
                visible: mode.players_visible(),
            },
            loading_bar(
                &format!("正在生成 {} {}", mode.label(), course.label),
                0.01,
                "yellow",
            ),
            message(format!(
                "正在生成 {} {}，种子 {}。",
                mode.label(),
                course.label,
                seed
            )),
        ],
    }
}

fn continue_build(config: &Config, player_uuid: &str) -> PluginCommandResponse {
    let Some(mut build) = storage_get_typed::<BuildJob>(&build_key(player_uuid)) else {
        return PluginCommandResponse::default();
    };
    let start = build.written.min(build.total_blocks);
    let end = (start + BUILD_BLOCKS_PER_TICK).min(build.total_blocks);
    let Some(blocks) = build_blocks_for_range(player_uuid, start, end) else {
        clear_runtime(player_uuid);
        let _ = storage_set_typed(&session_key(player_uuid), &ParkourSession::lobby(config));
        return PluginCommandResponse {
            handled: true,
            actions: vec![
                PlayerAction::RemoveBossBar { id: build_bar_id() },
                message("赛道生成失败。"),
            ],
        };
    };
    let ok = world_set_blocks(blocks.iter().map(|block| {
        (
            block.dimension.as_str(),
            block.position,
            block.block.as_str(),
        )
    }));
    if !ok {
        clear_runtime(player_uuid);
        let _ = storage_set_typed(&session_key(player_uuid), &ParkourSession::lobby(config));
        return PluginCommandResponse {
            handled: true,
            actions: vec![
                PlayerAction::RemoveBossBar { id: build_bar_id() },
                message("赛道生成失败。"),
            ],
        };
    }
    build.written = end;
    if build.written < build.total_blocks {
        let progress = build_progress(&build);
        let _ = storage_set_typed(&build_key(player_uuid), &build);
        return PluginCommandResponse {
            handled: true,
            actions: vec![loading_bar(
                &format!("正在生成 {}", build.session.course_label),
                progress,
                "yellow",
            )],
        };
    }

    let mut session = build.session;
    session.started_at_ms = time_millis();
    session.active = true;
    let _ = storage_set_typed(&session_key(player_uuid), &session);
    clear_runtime(player_uuid);
    PluginCommandResponse {
        handled: true,
        actions: vec![
            PlayerAction::RemoveBossBar { id: build_bar_id() },
            run_bar(&session),
            PlayerAction::Teleport {
                dimension: session.dimension.clone(),
                x: session.start_x,
                y: session.start_y,
                z: session.start_z,
                yaw: Some(0.0),
                pitch: Some(0.0),
            },
            message("开始跑酷。掉落会回到最近检查点。"),
        ],
    }
}

fn tick_session(config: &Config, payload: &PlayerTickPayload) -> Option<Vec<PlayerAction>> {
    let mut session = storage_get_typed::<ParkourSession>(&session_key(&payload.player.uuid))?;
    if !session.active || payload.dimension != session.dimension {
        return None;
    }
    let elapsed = time_millis().saturating_sub(session.started_at_ms);
    if session.mode == CourseMode::Exam && elapsed > session.exam_limit_ms {
        session.active = false;
        let _ = storage_set_typed(&session_key(&payload.player.uuid), &session);
        return Some(vec![
            PlayerAction::RemoveBossBar { id: run_bar_id() },
            message("考核超时，未通过。"),
            PlayerAction::Teleport {
                dimension: config.lobby.dimension.clone(),
                x: config.lobby.spawn_x,
                y: config.lobby.spawn_y,
                z: config.lobby.spawn_z,
                yaw: Some(config.lobby.spawn_yaw),
                pitch: Some(config.lobby.spawn_pitch),
            },
        ]);
    }
    Some(vec![run_bar(&session)])
}

fn update_session_position(
    config: &Config,
    player_uuid: &str,
    dimension: &str,
    x: f64,
    y: f64,
    z: f64,
) -> Vec<PlayerAction> {
    let Some(mut session) = storage_get_typed::<ParkourSession>(&session_key(player_uuid)) else {
        return Vec::new();
    };
    if !session.active || dimension != session.dimension {
        return Vec::new();
    }
    if y < session.origin_y as f64 - config.fall_reset_distance {
        return respawn_actions(&session);
    }
    let jump_changed = update_current_jump(&mut session, z);
    if jump_changed {
        let _ = storage_set_typed(&session_key(player_uuid), &session);
    }
    while session.next_checkpoint < session.checkpoints.len()
        && z >= session.checkpoints[session.next_checkpoint].z
    {
        let checkpoint = session.checkpoints[session.next_checkpoint].clone();
        session.checkpoint_x = checkpoint.x;
        session.checkpoint_y = checkpoint.y;
        session.checkpoint_z = checkpoint.z;
        session.next_checkpoint += 1;
        let _ = storage_set_typed(&session_key(player_uuid), &session);
        return vec![message(format!("检查点 {}", session.next_checkpoint))];
    }
    if z >= session.finish_z as f64
        && (x - session.start_x).abs() <= session.course_half_width + 4.0
    {
        return finish_run(config, player_uuid, session);
    }
    Vec::new()
}

fn finish_run(
    config: &Config,
    player_uuid: &str,
    mut session: ParkourSession,
) -> Vec<PlayerAction> {
    session.active = false;
    let elapsed = time_millis().saturating_sub(session.started_at_ms);
    let mut progress = progress(player_uuid);
    let xp = xp_reward(&session, elapsed);
    progress.xp = progress.xp.saturating_add(xp);
    progress.level = level_for_xp(progress.xp);
    if session.mode == CourseMode::Exam && elapsed <= session.exam_limit_ms {
        progress.exam_rank = progress.exam_rank.max(session.exam_rank_reward);
    }
    let _ = storage_set_typed(&progress_key(player_uuid), &progress);
    update_best(player_uuid, &session.mode, elapsed);
    update_leaderboard(player_uuid, &session.mode, elapsed);
    let _ = storage_set_typed(&session_key(player_uuid), &ParkourSession::lobby(config));
    vec![
        PlayerAction::RemoveBossBar { id: run_bar_id() },
        PlayerAction::SetPlayersVisible { visible: true },
        message(format!(
            "完成 {}，用时 {}，获得 {} XP。当前称号: {}",
            session.course_label,
            format_duration(elapsed),
            xp,
            display_title(player_uuid, &progress)
        )),
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

fn leave_to_lobby(config: &Config, player_uuid: &str) -> PluginCommandResponse {
    clear_runtime(player_uuid);
    let _ = storage_set_typed(&session_key(player_uuid), &ParkourSession::lobby(config));
    PluginCommandResponse {
        handled: true,
        actions: vec![
            PlayerAction::RemoveBossBar { id: run_bar_id() },
            PlayerAction::RemoveBossBar { id: build_bar_id() },
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

fn stats_response(_config: &Config, player: &PlayerPayloadOwned) -> PluginCommandResponse {
    let progress = progress(&player.uuid);
    PluginCommandResponse {
        handled: true,
        actions: vec![message(format!(
            "{}: {}  等级 {}  XP {}  竞速最佳 {}",
            player.username,
            display_title(&player.uuid, &progress),
            progress.level,
            progress.xp,
            format_best(best_for_mode(&player.uuid, &CourseMode::Race).unwrap_or(0))
        ))],
    }
}

fn status_response(config: &Config, player: &PlayerPayloadOwned) -> PluginCommandResponse {
    let session = storage_get_typed::<ParkourSession>(&session_key(&player.uuid))
        .unwrap_or_else(|| ParkourSession::lobby(config));
    PluginCommandResponse {
        handled: true,
        actions: vec![message(format!(
            "当前: {}  进度: {}/{}  顺序: {}",
            current_jump_label(&session),
            jump_index_text(&session),
            jump_total_text(&session),
            jump_sequence_text(&session, 16)
        ))],
    }
}

fn generate_course(
    _config: &Config,
    course: &CourseConfig,
    origin: (i32, i32, i32),
    seed: u64,
    mode: CourseMode,
    practice_id: Option<&str>,
) -> GeneratedCourse {
    let mut rng = Rng::new(seed);
    let mut blocks = Vec::new();
    let mut jump_sequence = Vec::new();
    let dimension = course.dimension.clone();
    let (ox, oy, oz) = origin;
    let half_width = course.path_half_width();
    let clear_bottom = oy - 12;
    let clear_top = oy + 9;
    for z in -8..=course.length + 12 {
        for x in -COURSE_CLEAR_RADIUS..=COURSE_CLEAR_RADIUS {
            for y in clear_bottom..=clear_top {
                blocks.push(CourseBlock {
                    dimension: dimension.clone(),
                    position: (ox + x, y, oz + z),
                    block: "minecraft:air".to_string(),
                });
            }
        }
    }

    let mut z = 0;
    let mut checkpoint_index = 0;
    let mut checkpoints = Vec::new();
    build_start_platform(&mut blocks, course, origin);
    let course_plan = generate_course_plan(&mut rng, course, mode, practice_id);
    for segment in course_plan {
        let start_z = z;
        build_segment(&mut blocks, course, origin, z, segment);
        append_jump_steps(&mut jump_sequence, segment, start_z);
        if z > 0 && z / course.checkpoint_interval > checkpoint_index {
            checkpoint_index = z / course.checkpoint_interval;
            build_checkpoint(&mut blocks, course, origin, z);
            checkpoints.push(checkpoint_at(origin, z));
        }
        z += segment.length();
    }
    for dz in 0..=4 {
        for x in -half_width..=half_width {
            push_block(
                &mut blocks,
                &dimension,
                (ox + x, oy - 1, oz + course.length + dz),
                "minecraft:emerald_block",
            );
        }
    }
    GeneratedCourse {
        blocks,
        finish_z: oz + course.length,
        checkpoints,
        jump_sequence,
    }
}

fn generate_course_plan(
    rng: &mut Rng,
    course: &CourseConfig,
    mode: CourseMode,
    practice_id: Option<&str>,
) -> Vec<Segment> {
    let mut plan = Vec::new();
    let mut z = 0;
    while z <= course.length {
        let segment = choose_segment(rng, course, mode, practice_id, z);
        z += segment.length();
        plan.push(segment);
    }
    plan
}

fn append_jump_steps(steps: &mut Vec<JumpStep>, segment: Segment, start_z: i32) {
    match segment {
        Segment::PrecisionGap { gap, repeats } | Segment::DenseGap { gap, repeats } => {
            append_repeated_gap_steps(steps, segment.id(), segment.label(), start_z, gap, repeats);
        }
        Segment::MixedGaps { gaps, repeats } => {
            let mut local_z = start_z;
            for gap in gaps.iter().take(mixed_gap_count(repeats)) {
                let next_z = local_z + *gap + 1;
                push_jump_step(steps, gap_id(*gap), gap_label(*gap), local_z, next_z - 1);
                local_z = next_z;
            }
        }
        Segment::Gap { before, gap, after } => {
            let jump_start = start_z + before.saturating_sub(1);
            let jump_end = start_z + before + gap + after.saturating_sub(1);
            push_jump_step(steps, gap_id(gap), gap_label(gap), jump_start, jump_end);
        }
        _ => {
            push_jump_step(
                steps,
                segment.id(),
                segment.label(),
                start_z,
                start_z + segment.length().saturating_sub(1),
            );
        }
    }
}

fn append_repeated_gap_steps(
    steps: &mut Vec<JumpStep>,
    id: &str,
    label: &str,
    start_z: i32,
    gap: i32,
    repeats: i32,
) {
    for index in 0..repeats.max(1) {
        let step_start = start_z + index * (gap + 1);
        push_jump_step(steps, id, label, step_start, step_start + gap);
    }
}

fn push_jump_step(steps: &mut Vec<JumpStep>, id: &str, label: &str, start_z: i32, end_z: i32) {
    steps.push(JumpStep {
        id: id.to_string(),
        label: label.to_string(),
        start_z,
        end_z,
    });
}

fn build_segment(
    blocks: &mut Vec<CourseBlock>,
    course: &CourseConfig,
    origin: (i32, i32, i32),
    start_z: i32,
    segment: Segment,
) {
    let (ox, oy, oz) = origin;
    let half = course.path_half_width();
    match segment {
        Segment::Straight { len } => {
            for dz in 0..len {
                for x in -half..=half {
                    push_block(
                        blocks,
                        &course.dimension,
                        (ox + x, oy - 1, oz + start_z + dz),
                        course.floor_block.as_str(),
                    );
                }
            }
        }
        Segment::Gap { before, gap, after } => {
            build_segment(
                blocks,
                course,
                origin,
                start_z,
                Segment::Straight { len: before },
            );
            build_segment(
                blocks,
                course,
                origin,
                start_z + before + gap,
                Segment::Straight { len: after },
            );
        }
        Segment::PrecisionGap { gap, repeats } => {
            build_precision_gap(blocks, course, origin, start_z, gap, repeats, false);
        }
        Segment::DenseGap { gap, repeats } => {
            build_precision_gap(blocks, course, origin, start_z, gap, repeats, true);
        }
        Segment::MixedGaps { gaps, repeats } => {
            let mut local_z = start_z;
            for gap in gaps.iter().take(mixed_gap_count(repeats)) {
                push_landing_pad(
                    blocks,
                    &course.dimension,
                    (ox, oy - 1, oz + local_z),
                    1,
                    match *gap {
                        1 => "minecraft:lime_concrete",
                        2 => "minecraft:yellow_concrete",
                        _ => "minecraft:orange_concrete",
                    },
                );
                local_z += *gap + 1;
            }
            push_landing_pad(
                blocks,
                &course.dimension,
                (ox, oy - 1, oz + local_z),
                1,
                "minecraft:lime_concrete",
            );
        }
        Segment::Narrow { len } => {
            for dz in 0..len {
                push_block(
                    blocks,
                    &course.dimension,
                    (ox, oy - 1, oz + start_z + dz),
                    "minecraft:stone_bricks",
                );
            }
        }
        Segment::Ice { len } => {
            for dz in 0..len {
                for x in -half..=half {
                    push_block(
                        blocks,
                        &course.dimension,
                        (ox + x, oy - 1, oz + start_z + dz),
                        "minecraft:ice",
                    );
                }
            }
        }
        Segment::Slime { len } => {
            for index in 0..=len / 3 {
                let z = start_z + index * 3;
                push_landing_pad(
                    blocks,
                    &course.dimension,
                    (ox, oy - 1, oz + z),
                    1,
                    if index % 2 == 0 {
                        "minecraft:slime_block"
                    } else {
                        "minecraft:lime_concrete"
                    },
                );
            }
        }
        Segment::HeadHitter { len } => {
            build_head_hitter_jump(blocks, course, origin, start_z, len);
        }
        Segment::Ladder { len } => {
            build_ladder_jump(blocks, course, origin, start_z, len);
        }
        Segment::Dropper { len } => {
            for dz in 0..len {
                for x in -2_i32..=2 {
                    let block = if dz % 4 == 0 && x.abs() == 2 {
                        "minecraft:red_concrete"
                    } else {
                        "minecraft:air"
                    };
                    push_block(
                        blocks,
                        &course.dimension,
                        (ox + x, oy - 1, oz + start_z + dz),
                        block,
                    );
                }
            }
            push_block(
                blocks,
                &course.dimension,
                (ox, oy - 8, oz + start_z + len - 1),
                "minecraft:water",
            );
            push_landing_pad(
                blocks,
                &course.dimension,
                (ox, oy - 9, oz + start_z + len - 1),
                1,
                "minecraft:blue_concrete",
            );
        }
        Segment::Neo { len } => {
            build_segment(blocks, course, origin, start_z, Segment::Narrow { len });
            let mid = start_z + len / 2;
            for dy in 0..=2 {
                push_block(
                    blocks,
                    &course.dimension,
                    (ox, oy + dy, oz + mid),
                    "minecraft:stone_bricks",
                );
            }
        }
        Segment::Strafe { len } => {
            for dz in 0..len {
                let x = match dz % 4 {
                    0 => -2,
                    1 => 0,
                    2 => 2,
                    _ => 0,
                };
                push_pad(
                    blocks,
                    &course.dimension,
                    (ox + x, oy - 1, oz + start_z + dz),
                    0,
                    "minecraft:purpur_block",
                );
            }
        }
        Segment::Fence { len } => {
            build_center_path(blocks, course, origin, start_z, len, "minecraft:oak_fence");
        }
        Segment::Trapdoor { len } => {
            build_center_path(
                blocks,
                course,
                origin,
                start_z,
                len,
                "minecraft:oak_trapdoor",
            );
        }
        Segment::Pane { len } => {
            build_center_path(blocks, course, origin, start_z, len, "minecraft:glass_pane");
        }
        Segment::IronBar { len } => {
            build_center_path(blocks, course, origin, start_z, len, "minecraft:iron_bars");
        }
        Segment::Chain { len } => {
            build_center_path(blocks, course, origin, start_z, len, "minecraft:chain");
        }
        Segment::EndRod { len } => {
            build_center_path(blocks, course, origin, start_z, len, "minecraft:end_rod");
        }
        Segment::Honey { len } => {
            build_center_path(
                blocks,
                course,
                origin,
                start_z,
                len,
                "minecraft:honey_block",
            );
        }
        Segment::SoulSand { len } => {
            build_center_path(blocks, course, origin, start_z, len, "minecraft:soul_sand");
        }
        Segment::LadderNeo { len } => {
            build_segment(blocks, course, origin, start_z, Segment::Narrow { len });
            let mid = start_z + len / 2;
            for dy in 0..=2 {
                push_block(
                    blocks,
                    &course.dimension,
                    (ox, oy + dy, oz + mid),
                    "minecraft:stone_bricks",
                );
            }
            for dz in mid..=(mid + 2).min(start_z + len - 1) {
                push_block(
                    blocks,
                    &course.dimension,
                    (ox + 1, oy, oz + dz),
                    "minecraft:ladder[facing=east]",
                );
            }
        }
    }
}

fn build_ladder_jump(
    blocks: &mut Vec<CourseBlock>,
    course: &CourseConfig,
    origin: (i32, i32, i32),
    start_z: i32,
    len: i32,
) {
    let (ox, oy, oz) = origin;
    let first_z = start_z;
    let ladder_z = start_z + 3;
    let exit_z = start_z + len - 1;
    push_landing_pad(
        blocks,
        &course.dimension,
        (ox, oy - 1, oz + first_z),
        1,
        course.floor_block.as_str(),
    );
    for dy in 0..=3 {
        push_block(
            blocks,
            &course.dimension,
            (ox, oy + dy, oz + ladder_z + 1),
            "minecraft:oak_planks",
        );
        push_block(
            blocks,
            &course.dimension,
            (ox, oy + dy, oz + ladder_z),
            "minecraft:ladder[facing=north]",
        );
    }
    push_landing_pad(
        blocks,
        &course.dimension,
        (ox, oy + 3, oz + exit_z),
        1,
        "minecraft:oak_planks",
    );
}

fn build_head_hitter_jump(
    blocks: &mut Vec<CourseBlock>,
    course: &CourseConfig,
    origin: (i32, i32, i32),
    start_z: i32,
    len: i32,
) {
    let (ox, oy, oz) = origin;
    let mut dz = 0;
    while dz < len {
        push_landing_pad(
            blocks,
            &course.dimension,
            (ox, oy - 1, oz + start_z + dz),
            0,
            "minecraft:stone_bricks",
        );
        push_block(
            blocks,
            &course.dimension,
            (ox, oy + 2, oz + start_z + dz),
            "minecraft:smooth_stone",
        );
        dz += 2;
    }
}

fn build_start_platform(
    blocks: &mut Vec<CourseBlock>,
    course: &CourseConfig,
    origin: (i32, i32, i32),
) {
    let (ox, oy, oz) = origin;
    let half = course.path_half_width().max(1);
    for x in -half..=half {
        for z in -2..=0 {
            push_block(
                blocks,
                &course.dimension,
                (ox + x, oy - 1, oz + z),
                course.floor_block.as_str(),
            );
        }
    }
}

fn build_center_path(
    blocks: &mut Vec<CourseBlock>,
    course: &CourseConfig,
    origin: (i32, i32, i32),
    start_z: i32,
    len: i32,
    block: &str,
) {
    let (ox, oy, oz) = origin;
    for dz in 0..len {
        push_block(
            blocks,
            &course.dimension,
            (ox, oy - 1, oz + start_z + dz),
            block,
        );
    }
}

fn build_precision_gap(
    blocks: &mut Vec<CourseBlock>,
    course: &CourseConfig,
    origin: (i32, i32, i32),
    start_z: i32,
    gap: i32,
    repeats: i32,
    dense: bool,
) {
    let (ox, oy, oz) = origin;
    let pad_half_width = if dense { 0 } else { 1 };
    for index in 0..=repeats {
        let z = start_z + index * (gap + 1);
        push_landing_pad(
            blocks,
            &course.dimension,
            (ox, oy - 1, oz + z),
            pad_half_width,
            course.floor_block.as_str(),
        );
    }
}

fn push_landing_pad(
    blocks: &mut Vec<CourseBlock>,
    dimension: &str,
    center: (i32, i32, i32),
    half_width: i32,
    block: &str,
) {
    for x in -half_width..=half_width {
        push_block(blocks, dimension, (center.0 + x, center.1, center.2), block);
    }
}

fn push_pad(
    blocks: &mut Vec<CourseBlock>,
    dimension: &str,
    center: (i32, i32, i32),
    radius: i32,
    block: &str,
) {
    for x in -radius..=radius {
        for z in -radius..=radius {
            push_block(
                blocks,
                dimension,
                (center.0 + x, center.1, center.2 + z),
                block,
            );
        }
    }
}

fn mixed_gaps_segment(rng: &mut Rng, repeats: i32) -> Segment {
    let mut gaps = [1; MIXED_GAP_MAX];
    let count = mixed_gap_count(repeats);
    for gap in gaps.iter_mut().take(count) {
        *gap = 1 + rng.range(3);
    }
    Segment::MixedGaps {
        gaps,
        repeats: count as i32,
    }
}

fn mixed_gap_count(repeats: i32) -> usize {
    repeats.clamp(1, MIXED_GAP_MAX as i32) as usize
}

fn gap_id(gap: i32) -> &'static str {
    match gap {
        1 => "single",
        2 => "double",
        3 => "three_block",
        4 => "quad",
        _ => "gap",
    }
}

fn gap_label(gap: i32) -> &'static str {
    match gap {
        1 => "单格跳",
        2 => "双格跳",
        3 => "三格跳",
        4 => "四格跳",
        _ => "跳跃",
    }
}

fn build_checkpoint(
    blocks: &mut Vec<CourseBlock>,
    course: &CourseConfig,
    origin: (i32, i32, i32),
    z: i32,
) {
    let (ox, oy, oz) = origin;
    let half = course.path_half_width();
    for x in -half..=half {
        push_block(
            blocks,
            &course.dimension,
            (ox + x, oy - 1, oz + z),
            "minecraft:gold_block",
        );
    }
}

fn checkpoint_at(origin: (i32, i32, i32), z: i32) -> Checkpoint {
    let (ox, oy, oz) = origin;
    Checkpoint {
        x: ox as f64 + 0.5,
        y: oy as f64 + 1.0,
        z: (oz + z) as f64 + 0.5,
    }
}

fn choose_segment(
    rng: &mut Rng,
    course: &CourseConfig,
    mode: CourseMode,
    practice_id: Option<&str>,
    z: i32,
) -> Segment {
    if mode == CourseMode::Practice {
        return match practice_id.unwrap_or_default() {
            "single" => Segment::PrecisionGap { gap: 1, repeats: 4 },
            "single_dense" => Segment::DenseGap { gap: 1, repeats: 6 },
            "double" => Segment::PrecisionGap { gap: 2, repeats: 3 },
            "double_dense" => Segment::DenseGap { gap: 2, repeats: 5 },
            "three_block" | "triple" => Segment::PrecisionGap { gap: 3, repeats: 3 },
            "triple_dense" => Segment::DenseGap { gap: 3, repeats: 4 },
            "quad" => Segment::PrecisionGap { gap: 4, repeats: 2 },
            "quad_dense" => Segment::DenseGap { gap: 4, repeats: 3 },
            "mixed_gaps" => mixed_gaps_segment(rng, 5),
            "neo" => Segment::Neo { len: 7 },
            "head_hitter" => Segment::HeadHitter { len: 10 },
            "ladder" => Segment::Ladder { len: 6 },
            "ice" => Segment::Ice { len: 6 },
            "slime" => Segment::Slime { len: 6 },
            "strafe" => Segment::Strafe { len: 7 },
            "fence" => Segment::Fence { len: 6 },
            "trapdoor" => Segment::Trapdoor { len: 6 },
            "pane" | "glass_pane" => Segment::Pane { len: 7 },
            "iron_bar" | "iron_bars" => Segment::IronBar { len: 7 },
            "chain" => Segment::Chain { len: 7 },
            "end_rod" => Segment::EndRod { len: 7 },
            "honey" => Segment::Honey { len: 7 },
            "soul_sand" => Segment::SoulSand { len: 7 },
            "ladder_neo" => Segment::LadderNeo { len: 7 },
            "dropper" => Segment::Dropper { len: 7 },
            _ => mixed_gaps_segment(rng, 5),
        };
    }
    if z < 8 {
        return Segment::PrecisionGap { gap: 1, repeats: 3 };
    }
    let weighted_total = course.gap_weight
        + course.narrow_weight
        + course.ice_weight
        + course.slime_weight
        + course.head_hitter_weight
        + course.neo_weight
        + course.ladder_weight
        + course.dropper_weight;
    let straight_weight = (100 - weighted_total).clamp(0, 6);
    let roll = rng.range((weighted_total + straight_weight).max(1));
    if roll < course.gap_weight {
        match rng.range(6) {
            0 => Segment::PrecisionGap { gap: 1, repeats: 4 },
            1 => Segment::PrecisionGap { gap: 2, repeats: 3 },
            2 => Segment::PrecisionGap { gap: 3, repeats: 3 },
            3 => Segment::DenseGap { gap: 2, repeats: 4 },
            4 => mixed_gaps_segment(rng, 5),
            _ => Segment::Gap {
                before: 2,
                gap: 2 + rng.range(2),
                after: 2,
            },
        }
    } else if roll < course.gap_weight + course.narrow_weight {
        Segment::Narrow {
            len: 5 + rng.range(4),
        }
    } else if roll < course.gap_weight + course.narrow_weight + course.ice_weight {
        Segment::Ice {
            len: 5 + rng.range(4),
        }
    } else if roll
        < course.gap_weight + course.narrow_weight + course.ice_weight + course.slime_weight
    {
        Segment::Slime {
            len: 5 + rng.range(3),
        }
    } else if roll
        < course.gap_weight
            + course.narrow_weight
            + course.ice_weight
            + course.slime_weight
            + course.head_hitter_weight
    {
        Segment::HeadHitter {
            len: 5 + rng.range(3),
        }
    } else if roll
        < course.gap_weight
            + course.narrow_weight
            + course.ice_weight
            + course.slime_weight
            + course.head_hitter_weight
            + course.neo_weight
    {
        Segment::Neo { len: 7 }
    } else if roll
        < course.gap_weight
            + course.narrow_weight
            + course.ice_weight
            + course.slime_weight
            + course.head_hitter_weight
            + course.neo_weight
            + course.ladder_weight
    {
        Segment::Ladder { len: 6 }
    } else if roll
        < course.gap_weight
            + course.narrow_weight
            + course.ice_weight
            + course.slime_weight
            + course.head_hitter_weight
            + course.neo_weight
            + course.ladder_weight
            + course.dropper_weight
    {
        match rng.range(4) {
            0 => Segment::Dropper { len: 7 },
            1 => Segment::Strafe { len: 7 },
            2 => Segment::Fence { len: 6 },
            _ => Segment::Trapdoor { len: 6 },
        }
    } else {
        let min = course.segment_min_length.max(3);
        let max = course.segment_max_length.max(min);
        Segment::Straight {
            len: min + rng.range(max - min + 1),
        }
    }
}

fn select_course(
    config: &Config,
    course_id: Option<&str>,
    mode: CourseMode,
    player_uuid: &str,
) -> Option<CourseConfig> {
    if mode == CourseMode::Casual && course_id.is_none() {
        let level = progress(player_uuid).level;
        return config
            .courses
            .iter()
            .filter(|course| course.min_level <= level)
            .max_by_key(|course| course.min_level)
            .or_else(|| config.courses.first())
            .cloned();
    }
    let id = course_id.unwrap_or(config.default_course.as_str());
    config
        .courses
        .iter()
        .find(|course| course.id.eq_ignore_ascii_case(id))
        .or_else(|| config.courses.first())
        .cloned()
}

fn register_lobby_region(config: &Config) {
    let y = config.lobby.floor_y;
    let _ = world_register_edit_region(&WorldEditRegion {
        id: "parkour_lobby",
        dimension: &config.lobby.dimension,
        min: (-24, y - 1, -24),
        max: (24, y + 8, 24),
        allow_player_break: false,
        allow_player_place: false,
        allow_plugin_write: true,
        runtime_only: true,
    });
    let mut blocks = Vec::new();
    for x in -16_i32..=16 {
        for z in -16_i32..=16 {
            let border = x.abs() == 16 || z.abs() == 16;
            blocks.push((
                config.lobby.dimension.as_str(),
                (x, y - 1, z),
                if border {
                    "minecraft:polished_andesite"
                } else {
                    "minecraft:smooth_stone"
                },
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
    let _ = world_set_blocks(blocks);
}

fn register_course_region(_config: &Config, origin: (i32, i32, i32), course: &CourseConfig) {
    let _ = world_register_edit_region(&WorldEditRegion {
        id: &format!("parkour_{}_{}_{}", course.id, origin.0, origin.2),
        dimension: &course.dimension,
        min: (origin.0 - 16, origin.1 - 16, origin.2 - 16),
        max: (origin.0 + 16, origin.1 + 16, origin.2 + course.length + 32),
        allow_player_break: false,
        allow_player_place: false,
        allow_plugin_write: true,
        runtime_only: true,
    });
}

fn respawn_actions(session: &ParkourSession) -> Vec<PlayerAction> {
    vec![
        message("已返回最近检查点。"),
        PlayerAction::Teleport {
            dimension: session.dimension.clone(),
            x: session.checkpoint_x,
            y: session.checkpoint_y,
            z: session.checkpoint_z,
            yaw: Some(0.0),
            pitch: Some(0.0),
        },
    ]
}

fn push_block(
    blocks: &mut Vec<CourseBlock>,
    dimension: &str,
    position: (i32, i32, i32),
    block: &str,
) {
    blocks.push(CourseBlock {
        dimension: dimension.to_string(),
        position,
        block: block.to_string(),
    });
}

fn clear_runtime(player_uuid: &str) {
    if let Some(build) = storage_get_typed::<BuildJob>(&build_key(player_uuid)) {
        for page in 0..build.page_count {
            let _ = storage_delete(&build_page_key(player_uuid, page));
        }
    }
    let _ = storage_delete(&build_key(player_uuid));
}

fn progress(player_uuid: &str) -> PlayerProgress {
    storage_get_typed::<PlayerProgress>(&progress_key(player_uuid)).unwrap_or_default()
}

fn update_best(player_uuid: &str, mode: &CourseMode, elapsed: i64) {
    let key = best_key(player_uuid, mode);
    let old = storage_get_typed::<i64>(&key).unwrap_or(0);
    if old <= 0 || elapsed < old {
        let _ = storage_set_typed(&key, &elapsed);
    }
}

fn best_for_mode(player_uuid: &str, mode: &CourseMode) -> Option<i64> {
    storage_get_typed::<i64>(&best_key(player_uuid, mode))
}

fn update_leaderboard(player_uuid: &str, mode: &CourseMode, elapsed: i64) {
    if !mode.ranked() {
        return;
    }
    let key = leaderboard_key(mode);
    let mut board = storage_get_typed::<Leaderboard>(&key).unwrap_or_default();
    let best_elapsed = best_for_mode(player_uuid, mode)
        .filter(|best| *best > 0)
        .unwrap_or(elapsed)
        .min(elapsed);
    if board
        .entries
        .iter()
        .any(|entry| entry.uuid == player_uuid && entry.elapsed_ms <= best_elapsed)
    {
        return;
    }
    board.entries.retain(|entry| entry.uuid != player_uuid);
    board.entries.push(LeaderboardEntry {
        uuid: player_uuid.to_string(),
        elapsed_ms: best_elapsed,
    });
    board.entries.sort_by_key(|entry| entry.elapsed_ms);
    board.entries.truncate(50);
    let _ = storage_set_typed(&key, &board);
}

fn remember_player_name(player_uuid: &str, username: &str) {
    let username = clean_display_name(username);
    if !username.is_empty() {
        let _ = storage_set_typed(&player_name_key(player_uuid), &username);
    }
}

fn maybe_refresh_daily_leaderboard(config: &Config) {
    if !config.leaderboard_display.enable {
        return;
    }
    let today = local_day_index(time_millis());
    let last = storage_get_typed::<i64>(&leaderboard_display_day_key()).unwrap_or(-1);
    if last < today {
        refresh_leaderboard_display(config, true);
    }
}

fn refresh_leaderboard_display(config: &Config, force: bool) {
    if !config.leaderboard_display.enable {
        return;
    }
    let today = local_day_index(time_millis());
    if !force && storage_get_typed::<i64>(&leaderboard_display_day_key()).unwrap_or(-1) >= today {
        return;
    }

    let board =
        storage_get_typed::<Leaderboard>(&leaderboard_key(&config.leaderboard_display.mode))
            .unwrap_or_default();
    let title = format!("§e{} §7每日榜", config.leaderboard_display.mode.label());
    upsert_leaderboard_line(config, 0, &title);
    for index in 0..LEADERBOARD_ROWS {
        let text = board
            .entries
            .get(index)
            .map(|entry| leaderboard_entry_line(index + 1, entry))
            .unwrap_or_else(|| format!("§7{}. 暂无记录", index + 1));
        upsert_leaderboard_line(config, index + 1, &text);
    }
    let _ = storage_set_typed(&leaderboard_display_day_key(), &today);
}

fn upsert_leaderboard_line(config: &Config, row: usize, text: &str) {
    let display = &config.leaderboard_display;
    let key = leaderboard_entity_key(row);
    let _ = entity_upsert(&RuntimeEntity {
        key: &key,
        dimension: &config.lobby.dimension,
        entity_type: display.entity_type.as_str(),
        x: display.x,
        y: display.y - display.line_spacing * row as f64,
        z: display.z,
        yaw: display.yaw,
        pitch: 0.0,
        display_name: text,
        ai: "",
        ai_params_json: "",
        auto_jump: false,
    });
}

fn leaderboard_entry_line(rank: usize, entry: &LeaderboardEntry) -> String {
    let name = storage_get_typed::<String>(&player_name_key(&entry.uuid))
        .map(|name| clean_display_name(&name))
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| short_uuid(&entry.uuid));
    format!(
        "§f{}. §b{} §7{}",
        rank,
        name,
        format_duration(entry.elapsed_ms)
    )
}

fn local_day_index(now_ms: i64) -> i64 {
    (now_ms + LOCAL_TIME_OFFSET_MS).div_euclid(DAY_MS)
}

fn clean_display_name(name: &str) -> String {
    name.chars()
        .filter(|ch| !ch.is_control())
        .take(16)
        .collect::<String>()
}

fn short_uuid(uuid: &str) -> String {
    let id = uuid
        .chars()
        .filter(|ch| *ch != '-')
        .take(8)
        .collect::<String>();
    if id.is_empty() {
        "未知玩家".to_string()
    } else {
        format!("玩家{id}")
    }
}

fn rank_for(player_uuid: &str, mode: &CourseMode) -> Option<usize> {
    let board = storage_get_typed::<Leaderboard>(&leaderboard_key(mode))?;
    board
        .entries
        .iter()
        .position(|entry| entry.uuid == player_uuid)
        .map(|index| index + 1)
}

fn display_title(player_uuid: &str, progress: &PlayerProgress) -> String {
    if rank_for(player_uuid, &CourseMode::Race) == Some(1) {
        return "冠军".to_string();
    }
    if rank_for(player_uuid, &CourseMode::Race).is_some_and(|rank| rank <= 10) {
        return "候选者".to_string();
    }
    match progress.exam_rank {
        rank if rank >= 4 => "大师",
        3 => "专家",
        2 => "高手",
        _ => "新手",
    }
    .to_string()
}

fn xp_reward(session: &ParkourSession, elapsed_ms: i64) -> u64 {
    let base = match session.mode {
        CourseMode::Race | CourseMode::Weekly => 80,
        CourseMode::Casual => 50,
        CourseMode::Exam => 120,
        CourseMode::Practice => 20,
        CourseMode::Custom => 25,
        CourseMode::Lobby => 0,
    };
    let bonus = if elapsed_ms <= session.par_time_ms {
        40
    } else {
        0
    };
    base + bonus
}

fn level_for_xp(xp: u64) -> u32 {
    ((xp / 250) + 1).min(u32::MAX as u64) as u32
}

fn instance_origin(config: &Config, player_uuid: &str, mode: &CourseMode) -> (i32, i32, i32) {
    let hash = stable_hash(&format!("{player_uuid}:{}", mode.storage_id()));
    let lane = (hash % 64) as i32;
    (
        config.instance_origin_x + lane * config.instance_spacing,
        config.instance_origin_y,
        config.instance_origin_z,
    )
}

fn weekly_seed() -> u64 {
    let week = time_millis().max(0) / (7 * 24 * 60 * 60 * 1000);
    stable_hash(&format!("parkour-week-{week}"))
}

fn stable_hash(input: &str) -> u64 {
    let mut value = 0xcbf29ce484222325_u64;
    for byte in input.as_bytes() {
        value ^= u64::from(*byte);
        value = value.wrapping_mul(0x100000001b3);
    }
    value
}

fn build_progress(build: &BuildJob) -> f32 {
    if build.total_blocks == 0 {
        1.0
    } else {
        (build.written as f32 / build.total_blocks as f32).clamp(0.01, 1.0)
    }
}

fn update_current_jump(session: &mut ParkourSession, player_z: f64) -> bool {
    let relative_z = (player_z.floor() as i32).saturating_sub(session.origin_z);
    if let Some(index) = session
        .jump_sequence
        .iter()
        .position(|step| relative_z >= step.start_z && relative_z <= step.end_z)
    {
        if session.current_jump != index {
            session.current_jump = index;
            return true;
        }
    }
    false
}

fn current_jump_label(session: &ParkourSession) -> String {
    session
        .jump_sequence
        .get(session.current_jump)
        .map(|step| step.label.clone())
        .unwrap_or_else(|| "未开始".to_string())
}

fn jump_index_text(session: &ParkourSession) -> String {
    if session.jump_sequence.is_empty() {
        "-".to_string()
    } else {
        (session.current_jump + 1).to_string()
    }
}

fn jump_total_text(session: &ParkourSession) -> String {
    if session.jump_sequence.is_empty() {
        "-".to_string()
    } else {
        session.jump_sequence.len().to_string()
    }
}

fn jump_sequence_text(session: &ParkourSession, limit: usize) -> String {
    if session.jump_sequence.is_empty() {
        return "未开始".to_string();
    }
    let mut labels = session
        .jump_sequence
        .iter()
        .take(limit)
        .map(|step| step.label.as_str())
        .collect::<Vec<_>>()
        .join(" > ");
    if session.jump_sequence.len() > limit {
        labels.push_str(" > ...");
    }
    labels
}

fn format_duration(ms: i64) -> String {
    let ms = ms.max(0);
    let seconds = ms / 1000;
    let minutes = seconds / 60;
    let seconds = seconds % 60;
    let tenths = (ms % 1000) / 100;
    format!("{minutes}:{seconds:02}.{tenths}")
}

fn format_best(ms: i64) -> String {
    if ms <= 0 {
        "--:--".to_string()
    } else {
        format_duration(ms)
    }
}

fn replacement(key: &str, value: impl Into<String>) -> PlaceholderReplacement {
    PlaceholderReplacement {
        key: key.to_string(),
        value: value.into(),
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

fn loading_bar(title: &str, progress: f32, color: &str) -> PlayerAction {
    PlayerAction::BossBar {
        id: build_bar_id(),
        title: title.to_string(),
        progress,
        color: color.to_string(),
        overlay: "progress".to_string(),
    }
}

fn run_bar(session: &ParkourSession) -> PlayerAction {
    let elapsed = time_millis().saturating_sub(session.started_at_ms);
    let current_jump = current_jump_label(session);
    let title = if session.mode == CourseMode::Exam {
        let remain = session.exam_limit_ms.saturating_sub(elapsed);
        format!(
            "{}  {}  剩余 {}",
            session.course_label,
            current_jump,
            format_duration(remain)
        )
    } else {
        format!(
            "{}  {}  {}",
            session.course_label,
            current_jump,
            format_duration(elapsed)
        )
    };
    PlayerAction::BossBar {
        id: run_bar_id(),
        title,
        progress: 1.0,
        color: if session.mode == CourseMode::Exam {
            "red".to_string()
        } else {
            "green".to_string()
        },
        overlay: "progress".to_string(),
    }
}

fn build_bar_id() -> String {
    "parkour:build".to_string()
}

fn run_bar_id() -> String {
    "parkour:run".to_string()
}

fn session_key(uuid: &str) -> String {
    format!("parkour/session/{uuid}")
}

fn build_key(uuid: &str) -> String {
    format!("parkour/build/{uuid}")
}

fn build_page_key(uuid: &str, page: usize) -> String {
    format!("parkour/build/{uuid}/page/{page}")
}

fn build_page_count(block_count: usize) -> usize {
    block_count.div_ceil(BUILD_BLOCKS_PER_PAGE)
}

fn store_build_pages(player_uuid: &str, blocks: Vec<CourseBlock>) -> bool {
    for (page, chunk) in blocks.chunks(BUILD_BLOCKS_PER_PAGE).enumerate() {
        let page_blocks = CourseBlockPage {
            blocks: chunk.to_vec(),
        };
        if !storage_set_typed(&build_page_key(player_uuid, page), &page_blocks) {
            for stale_page in 0..=page {
                let _ = storage_delete(&build_page_key(player_uuid, stale_page));
            }
            return false;
        }
    }
    true
}

fn build_blocks_for_range(player_uuid: &str, start: usize, end: usize) -> Option<Vec<CourseBlock>> {
    if start >= end {
        return Some(Vec::new());
    }
    let first_page = start / BUILD_BLOCKS_PER_PAGE;
    let last_page = (end - 1) / BUILD_BLOCKS_PER_PAGE;
    let mut blocks = Vec::with_capacity(end - start);
    for page_index in first_page..=last_page {
        let page = storage_get_typed::<CourseBlockPage>(&build_page_key(player_uuid, page_index))?;
        let page_start = page_index * BUILD_BLOCKS_PER_PAGE;
        let slice_start = start.saturating_sub(page_start);
        let slice_end = (end - page_start).min(page.blocks.len());
        if slice_start > slice_end || slice_end > page.blocks.len() {
            return None;
        }
        blocks.extend_from_slice(&page.blocks[slice_start..slice_end]);
    }
    Some(blocks)
}

fn progress_key(uuid: &str) -> String {
    format!("parkour/progress/{uuid}")
}

fn best_key(uuid: &str, mode: &CourseMode) -> String {
    format!("parkour/best/{}/{uuid}", mode.storage_id())
}

fn leaderboard_key(mode: &CourseMode) -> String {
    format!("parkour/leaderboard/{}", mode.storage_id())
}

fn player_name_key(uuid: &str) -> String {
    format!("parkour/player_name/{uuid}")
}

fn leaderboard_display_day_key() -> String {
    "parkour/leaderboard_display/day".to_string()
}

fn leaderboard_entity_key(row: usize) -> String {
    format!("leaderboard/{row}")
}

fn load_config() -> Config {
    config_read_to_string(CONFIG_PATH)
        .and_then(|content| toml::from_str::<Config>(&content).ok())
        .unwrap_or_else(default_config)
}

fn default_config() -> Config {
    toml::from_str(DEFAULT_CONFIG).expect("default parkour config is valid")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
enum CourseMode {
    Lobby,
    Race,
    Casual,
    Weekly,
    Exam,
    Practice,
    Custom,
}

impl CourseMode {
    fn label(self) -> &'static str {
        match self {
            Self::Lobby => "大厅",
            Self::Race => "竞速",
            Self::Casual => "悠闲",
            Self::Weekly => "每周",
            Self::Exam => "考核",
            Self::Practice => "练习",
            Self::Custom => "自定义",
        }
    }

    fn storage_id(self) -> &'static str {
        match self {
            Self::Lobby => "lobby",
            Self::Race => "race",
            Self::Casual => "casual",
            Self::Weekly => "weekly",
            Self::Exam => "exam",
            Self::Practice => "practice",
            Self::Custom => "custom",
        }
    }

    fn ranked(self) -> bool {
        matches!(self, Self::Race | Self::Weekly | Self::Exam)
    }

    fn players_visible(self) -> bool {
        matches!(self, Self::Race | Self::Weekly)
    }
}

enum SeedKind {
    Weekly,
    Player,
    Custom(String),
    Exam(String),
    Practice(String),
}

impl SeedKind {
    fn value(&self, player_uuid: &str) -> u64 {
        match self {
            Self::Weekly => weekly_seed(),
            Self::Player => stable_hash(&format!("{player_uuid}:{}", time_millis() / 60_000)),
            Self::Custom(seed) | Self::Exam(seed) | Self::Practice(seed) => stable_hash(seed),
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum Segment {
    Straight {
        len: i32,
    },
    Gap {
        before: i32,
        gap: i32,
        after: i32,
    },
    PrecisionGap {
        gap: i32,
        repeats: i32,
    },
    DenseGap {
        gap: i32,
        repeats: i32,
    },
    MixedGaps {
        gaps: [i32; MIXED_GAP_MAX],
        repeats: i32,
    },
    Narrow {
        len: i32,
    },
    Ice {
        len: i32,
    },
    Slime {
        len: i32,
    },
    HeadHitter {
        len: i32,
    },
    Ladder {
        len: i32,
    },
    Dropper {
        len: i32,
    },
    Neo {
        len: i32,
    },
    Strafe {
        len: i32,
    },
    Fence {
        len: i32,
    },
    Trapdoor {
        len: i32,
    },
    Pane {
        len: i32,
    },
    IronBar {
        len: i32,
    },
    Chain {
        len: i32,
    },
    EndRod {
        len: i32,
    },
    Honey {
        len: i32,
    },
    SoulSand {
        len: i32,
    },
    LadderNeo {
        len: i32,
    },
}

impl Segment {
    fn length(self) -> i32 {
        match self {
            Self::Straight { len }
            | Self::Narrow { len }
            | Self::Ice { len }
            | Self::Slime { len }
            | Self::HeadHitter { len }
            | Self::Ladder { len }
            | Self::Dropper { len }
            | Self::Neo { len }
            | Self::Strafe { len }
            | Self::Fence { len }
            | Self::Trapdoor { len }
            | Self::Pane { len }
            | Self::IronBar { len }
            | Self::Chain { len }
            | Self::EndRod { len }
            | Self::Honey { len }
            | Self::SoulSand { len }
            | Self::LadderNeo { len } => len,
            Self::Gap { before, gap, after } => before + gap + after,
            Self::PrecisionGap { gap, repeats } => repeats * (gap + 1),
            Self::DenseGap { gap, repeats } => repeats * (gap + 1),
            Self::MixedGaps { gaps, repeats } => gaps
                .iter()
                .take(mixed_gap_count(repeats))
                .map(|gap| gap + 1)
                .sum(),
        }
    }

    fn id(self) -> &'static str {
        match self {
            Self::Straight { .. } => "straight",
            Self::Gap { .. } => "gap",
            Self::PrecisionGap { gap: 1, .. } => "single",
            Self::PrecisionGap { gap: 2, .. } => "double",
            Self::PrecisionGap { gap: 3, .. } => "triple",
            Self::PrecisionGap { gap: 4, .. } => "quad",
            Self::PrecisionGap { .. } => "precision",
            Self::DenseGap { gap: 1, .. } => "single_dense",
            Self::DenseGap { gap: 2, .. } => "double_dense",
            Self::DenseGap { gap: 3, .. } => "triple_dense",
            Self::DenseGap { gap: 4, .. } => "quad_dense",
            Self::DenseGap { .. } => "dense",
            Self::MixedGaps { .. } => "mixed_gaps",
            Self::Narrow { .. } => "narrow",
            Self::Ice { .. } => "ice",
            Self::Slime { .. } => "slime",
            Self::HeadHitter { .. } => "head_hitter",
            Self::Ladder { .. } => "ladder",
            Self::Dropper { .. } => "dropper",
            Self::Neo { .. } => "neo",
            Self::Strafe { .. } => "strafe",
            Self::Fence { .. } => "fence",
            Self::Trapdoor { .. } => "trapdoor",
            Self::Pane { .. } => "pane",
            Self::IronBar { .. } => "iron_bar",
            Self::Chain { .. } => "chain",
            Self::EndRod { .. } => "end_rod",
            Self::Honey { .. } => "honey",
            Self::SoulSand { .. } => "soul_sand",
            Self::LadderNeo { .. } => "ladder_neo",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Straight { .. } => "短平台",
            Self::Gap { .. } => "跳跃",
            Self::PrecisionGap { gap: 1, .. } => "单格跳",
            Self::PrecisionGap { gap: 2, .. } => "双格跳",
            Self::PrecisionGap { gap: 3, .. } => "三格跳",
            Self::PrecisionGap { gap: 4, .. } => "四格跳",
            Self::PrecisionGap { .. } => "精确跳",
            Self::DenseGap { gap: 1, .. } => "密集单格跳",
            Self::DenseGap { gap: 2, .. } => "密集双格跳",
            Self::DenseGap { gap: 3, .. } => "密集三格跳",
            Self::DenseGap { gap: 4, .. } => "密集四格跳",
            Self::DenseGap { .. } => "密集跳",
            Self::MixedGaps { .. } => "单双三混合随机跳",
            Self::Narrow { .. } => "窄路节奏",
            Self::Ice { .. } => "冰面跳",
            Self::Slime { .. } => "史莱姆跳",
            Self::HeadHitter { .. } => "顶头跳",
            Self::Ladder { .. } => "梯子跳",
            Self::Dropper { .. } => "落水跳",
            Self::Neo { .. } => "Neo 跳",
            Self::Strafe { .. } => "侧向跳",
            Self::Fence { .. } => "栅栏跳",
            Self::Trapdoor { .. } => "活板门跳",
            Self::Pane { .. } => "玻璃板跳",
            Self::IronBar { .. } => "铁栏杆跳",
            Self::Chain { .. } => "锁链跳",
            Self::EndRod { .. } => "末地烛跳",
            Self::Honey { .. } => "蜂蜜块跳",
            Self::SoulSand { .. } => "灵魂沙跳",
            Self::LadderNeo { .. } => "梯子 Neo 跳",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ParkourSession {
    active: bool,
    mode: CourseMode,
    dimension: String,
    course_id: String,
    course_label: String,
    origin_x: i32,
    origin_y: i32,
    origin_z: i32,
    start_x: f64,
    start_y: f64,
    start_z: f64,
    finish_z: i32,
    checkpoint_x: f64,
    checkpoint_y: f64,
    checkpoint_z: f64,
    next_checkpoint: usize,
    checkpoints: Vec<Checkpoint>,
    started_at_ms: i64,
    seed: u64,
    course_half_width: f64,
    par_time_ms: i64,
    exam_limit_ms: i64,
    exam_rank_reward: u8,
    jump_sequence: Vec<JumpStep>,
    current_jump: usize,
}

impl ParkourSession {
    fn lobby(config: &Config) -> Self {
        Self {
            active: false,
            mode: CourseMode::Lobby,
            dimension: config.lobby.dimension.clone(),
            course_id: "lobby".to_string(),
            course_label: "大厅".to_string(),
            origin_x: 0,
            origin_y: config.lobby.floor_y,
            origin_z: 0,
            start_x: config.lobby.spawn_x,
            start_y: config.lobby.spawn_y,
            start_z: config.lobby.spawn_z,
            finish_z: 0,
            checkpoint_x: config.lobby.spawn_x,
            checkpoint_y: config.lobby.spawn_y,
            checkpoint_z: config.lobby.spawn_z,
            next_checkpoint: 0,
            checkpoints: Vec::new(),
            started_at_ms: 0,
            seed: 0,
            course_half_width: 4.0,
            par_time_ms: 0,
            exam_limit_ms: 0,
            exam_rank_reward: 0,
            jump_sequence: Vec::new(),
            current_jump: 0,
        }
    }

    fn new(
        _config: &Config,
        course: &CourseConfig,
        origin: (i32, i32, i32),
        seed: u64,
        mode: CourseMode,
        exam: Option<&ExamConfig>,
    ) -> Self {
        let (ox, oy, oz) = origin;
        Self {
            active: false,
            mode,
            dimension: course.dimension.clone(),
            course_id: course.id.clone(),
            course_label: course.label.clone(),
            origin_x: ox,
            origin_y: oy,
            origin_z: oz,
            start_x: ox as f64 + 0.5,
            start_y: oy as f64 + 1.0,
            start_z: oz as f64 + 0.5,
            finish_z: oz + course.length,
            checkpoint_x: ox as f64 + 0.5,
            checkpoint_y: oy as f64 + 1.0,
            checkpoint_z: oz as f64 + 0.5,
            next_checkpoint: 0,
            checkpoints: Vec::new(),
            started_at_ms: 0,
            seed,
            course_half_width: f64::from(course.path_half_width()),
            par_time_ms: course.par_time_ms,
            exam_limit_ms: exam.map(|exam| exam.time_limit_ms).unwrap_or(0),
            exam_rank_reward: exam.map(|exam| exam.rank_reward).unwrap_or(0),
            jump_sequence: Vec::new(),
            current_jump: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Checkpoint {
    x: f64,
    y: f64,
    z: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct JumpStep {
    id: String,
    label: String,
    start_z: i32,
    end_z: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BuildJob {
    session: ParkourSession,
    total_blocks: usize,
    page_count: usize,
    written: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CourseBlock {
    dimension: String,
    position: (i32, i32, i32),
    block: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CourseBlockPage {
    blocks: Vec<CourseBlock>,
}

struct GeneratedCourse {
    blocks: Vec<CourseBlock>,
    finish_z: i32,
    checkpoints: Vec<Checkpoint>,
    jump_sequence: Vec<JumpStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PlayerProgress {
    xp: u64,
    level: u32,
    exam_rank: u8,
}

impl Default for PlayerProgress {
    fn default() -> Self {
        Self {
            xp: 0,
            level: 1,
            exam_rank: 0,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Leaderboard {
    entries: Vec<LeaderboardEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LeaderboardEntry {
    uuid: String,
    elapsed_ms: i64,
}

#[derive(Debug, Clone)]
struct Rng {
    state: u64,
}

impl Rng {
    fn new(seed: u64) -> Self {
        Self { state: seed | 1 }
    }

    fn next(&mut self) -> u64 {
        self.state ^= self.state << 7;
        self.state ^= self.state >> 9;
        self.state = self.state.wrapping_mul(0x9e3779b97f4a7c15);
        self.state
    }

    fn range(&mut self, max: i32) -> i32 {
        if max <= 0 {
            0
        } else {
            (self.next() % max as u64) as i32
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Config {
    #[serde(default = "default_true")]
    enable: bool,
    #[serde(default = "default_menu_id")]
    menu_id: String,
    #[serde(default = "default_course")]
    default_course: String,
    #[serde(default = "default_instance_spacing")]
    instance_spacing: i32,
    #[serde(default = "default_instance_origin_x")]
    instance_origin_x: i32,
    #[serde(default = "default_instance_origin_y")]
    instance_origin_y: i32,
    #[serde(default = "default_instance_origin_z")]
    instance_origin_z: i32,
    #[serde(default = "default_fall_reset_distance")]
    fall_reset_distance: f64,
    #[serde(default)]
    lobby: LobbyConfig,
    #[serde(default)]
    leaderboard_display: LeaderboardDisplayConfig,
    #[serde(default = "default_courses")]
    courses: Vec<CourseConfig>,
    #[serde(default = "default_exams")]
    exams: Vec<ExamConfig>,
    #[serde(default = "default_practice")]
    practice: Vec<PracticeConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LobbyConfig {
    #[serde(default = "default_lobby_dimension")]
    dimension: String,
    #[serde(default = "default_lobby_spawn_x")]
    spawn_x: f64,
    #[serde(default = "default_lobby_spawn_y")]
    spawn_y: f64,
    #[serde(default = "default_lobby_spawn_z")]
    spawn_z: f64,
    #[serde(default)]
    spawn_yaw: f32,
    #[serde(default)]
    spawn_pitch: f32,
    #[serde(default = "default_lobby_floor_y")]
    floor_y: i32,
    #[serde(default = "default_npc_x")]
    npc_x: f64,
    #[serde(default = "default_lobby_spawn_y")]
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
            spawn_x: default_lobby_spawn_x(),
            spawn_y: default_lobby_spawn_y(),
            spawn_z: default_lobby_spawn_z(),
            spawn_yaw: 0.0,
            spawn_pitch: 0.0,
            floor_y: default_lobby_floor_y(),
            npc_x: default_npc_x(),
            npc_y: default_lobby_spawn_y(),
            npc_z: default_npc_z(),
            npc_yaw: default_npc_yaw(),
            npc_pitch: 0.0,
            npc_entity_type: default_npc_entity_type(),
            npc_display_name: default_npc_display_name(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LeaderboardDisplayConfig {
    #[serde(default = "default_true")]
    enable: bool,
    #[serde(
        default = "default_leaderboard_mode",
        deserialize_with = "deserialize_leaderboard_mode"
    )]
    mode: CourseMode,
    #[serde(default = "default_leaderboard_x")]
    x: f64,
    #[serde(default = "default_leaderboard_y")]
    y: f64,
    #[serde(default = "default_leaderboard_z")]
    z: f64,
    #[serde(default = "default_leaderboard_yaw")]
    yaw: f32,
    #[serde(default = "default_leaderboard_line_spacing")]
    line_spacing: f64,
    #[serde(default = "default_leaderboard_entity_type")]
    entity_type: String,
}

impl Default for LeaderboardDisplayConfig {
    fn default() -> Self {
        Self {
            enable: true,
            mode: default_leaderboard_mode(),
            x: default_leaderboard_x(),
            y: default_leaderboard_y(),
            z: default_leaderboard_z(),
            yaw: default_leaderboard_yaw(),
            line_spacing: default_leaderboard_line_spacing(),
            entity_type: default_leaderboard_entity_type(),
        }
    }
}

fn deserialize_leaderboard_mode<'de, D>(deserializer: D) -> Result<CourseMode, D::Error>
where
    D: Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    match value.trim().to_ascii_lowercase().as_str() {
        "race" | "match" | "竞速" => Ok(CourseMode::Race),
        "weekly" | "week" | "每周" => Ok(CourseMode::Weekly),
        "exam" | "考核" => Ok(CourseMode::Exam),
        other => Err(serde::de::Error::custom(format!(
            "unsupported leaderboard mode: {other}"
        ))),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CourseConfig {
    id: String,
    label: String,
    #[serde(default = "default_course_dimension")]
    dimension: String,
    #[serde(default = "default_length")]
    length: i32,
    #[serde(default = "default_path_width")]
    path_width: i32,
    #[serde(default = "default_checkpoint_interval")]
    checkpoint_interval: i32,
    #[serde(default = "default_segment_min_length")]
    segment_min_length: i32,
    #[serde(default = "default_segment_max_length")]
    segment_max_length: i32,
    #[serde(default = "default_floor_block")]
    floor_block: String,
    #[serde(default)]
    min_level: u32,
    #[serde(default = "default_par_time_ms")]
    par_time_ms: i64,
    #[serde(default = "default_gap_weight")]
    gap_weight: i32,
    #[serde(default = "default_narrow_weight")]
    narrow_weight: i32,
    #[serde(default = "default_ice_weight")]
    ice_weight: i32,
    #[serde(default = "default_slime_weight")]
    slime_weight: i32,
    #[serde(default = "default_head_hitter_weight")]
    head_hitter_weight: i32,
    #[serde(default = "default_neo_weight")]
    neo_weight: i32,
    #[serde(default = "default_ladder_weight")]
    ladder_weight: i32,
    #[serde(default = "default_dropper_weight")]
    dropper_weight: i32,
}

impl CourseConfig {
    fn path_half_width(&self) -> i32 {
        (self.path_width.max(1) / 2).max(0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ExamConfig {
    id: String,
    label: String,
    course_id: String,
    time_limit_ms: i64,
    rank_reward: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PracticeConfig {
    id: String,
    label: String,
    course_id: String,
}

fn default_true() -> bool {
    true
}

fn default_menu_id() -> String {
    MENU_ID.to_string()
}

fn default_course() -> String {
    "standard".to_string()
}

fn default_instance_spacing() -> i32 {
    256
}

fn default_instance_origin_x() -> i32 {
    1024
}

fn default_instance_origin_y() -> i32 {
    -52
}

fn default_instance_origin_z() -> i32 {
    1024
}

fn default_fall_reset_distance() -> f64 {
    14.0
}

fn default_lobby_dimension() -> String {
    "qexed:parkour_lobby".to_string()
}

fn default_course_dimension() -> String {
    "qexed:parkour_course".to_string()
}

fn default_lobby_spawn_x() -> f64 {
    0.5
}

fn default_lobby_spawn_y() -> f64 {
    -52.0
}

fn default_lobby_spawn_z() -> f64 {
    0.5
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

fn default_npc_yaw() -> f32 {
    180.0
}

fn default_npc_entity_type() -> String {
    "minecraft:villager".to_string()
}

fn default_npc_display_name() -> String {
    "{\"text\":\"跑酷入口\",\"color\":\"aqua\"}".to_string()
}

fn default_leaderboard_mode() -> CourseMode {
    CourseMode::Race
}

fn default_leaderboard_x() -> f64 {
    4.5
}

fn default_leaderboard_y() -> f64 {
    -50.0
}

fn default_leaderboard_z() -> f64 {
    4.5
}

fn default_leaderboard_yaw() -> f32 {
    180.0
}

fn default_leaderboard_line_spacing() -> f64 {
    0.32
}

fn default_leaderboard_entity_type() -> String {
    "minecraft:text_display".to_string()
}

fn default_length() -> i32 {
    220
}

fn default_path_width() -> i32 {
    5
}

fn default_checkpoint_interval() -> i32 {
    48
}

fn default_segment_min_length() -> i32 {
    8
}

fn default_segment_max_length() -> i32 {
    16
}

fn default_floor_block() -> String {
    "minecraft:quartz_block".to_string()
}

fn default_par_time_ms() -> i64 {
    90_000
}

fn default_gap_weight() -> i32 {
    18
}

fn default_narrow_weight() -> i32 {
    12
}

fn default_ice_weight() -> i32 {
    10
}

fn default_slime_weight() -> i32 {
    10
}

fn default_head_hitter_weight() -> i32 {
    8
}

fn default_neo_weight() -> i32 {
    6
}

fn default_ladder_weight() -> i32 {
    6
}

fn default_dropper_weight() -> i32 {
    3
}

fn default_courses() -> Vec<CourseConfig> {
    vec![
        CourseConfig {
            id: "standard".to_string(),
            label: "标准跑酷".to_string(),
            dimension: default_course_dimension(),
            length: 240,
            path_width: 3,
            checkpoint_interval: 32,
            segment_min_length: 3,
            segment_max_length: 6,
            floor_block: "minecraft:quartz_block".to_string(),
            min_level: 1,
            par_time_ms: 90_000,
            gap_weight: 34,
            narrow_weight: 18,
            ice_weight: 10,
            slime_weight: 12,
            head_hitter_weight: 10,
            neo_weight: 8,
            ladder_weight: 5,
            dropper_weight: 3,
        },
        CourseConfig {
            id: "hardcore".to_string(),
            label: "硬核跑酷".to_string(),
            dimension: default_course_dimension(),
            length: 280,
            path_width: 3,
            checkpoint_interval: 36,
            segment_min_length: 3,
            segment_max_length: 5,
            floor_block: "minecraft:deepslate_tiles".to_string(),
            min_level: 4,
            par_time_ms: 100_000,
            gap_weight: 36,
            narrow_weight: 18,
            ice_weight: 10,
            slime_weight: 10,
            head_hitter_weight: 12,
            neo_weight: 12,
            ladder_weight: 8,
            dropper_weight: 4,
        },
    ]
}

fn default_exams() -> Vec<ExamConfig> {
    vec![
        ExamConfig {
            id: "novice".to_string(),
            label: "新手考核".to_string(),
            course_id: "standard".to_string(),
            time_limit_ms: 120_000,
            rank_reward: 1,
        },
        ExamConfig {
            id: "expert".to_string(),
            label: "高手考核".to_string(),
            course_id: "hardcore".to_string(),
            time_limit_ms: 110_000,
            rank_reward: 2,
        },
        ExamConfig {
            id: "master".to_string(),
            label: "大师考核".to_string(),
            course_id: "hardcore".to_string(),
            time_limit_ms: 90_000,
            rank_reward: 4,
        },
    ]
}

fn default_practice() -> Vec<PracticeConfig> {
    vec![
        PracticeConfig {
            id: "single".to_string(),
            label: "单格跳".to_string(),
            course_id: "standard".to_string(),
        },
        PracticeConfig {
            id: "single_dense".to_string(),
            label: "密集单格跳".to_string(),
            course_id: "standard".to_string(),
        },
        PracticeConfig {
            id: "double".to_string(),
            label: "双格跳".to_string(),
            course_id: "standard".to_string(),
        },
        PracticeConfig {
            id: "double_dense".to_string(),
            label: "密集双格跳".to_string(),
            course_id: "standard".to_string(),
        },
        PracticeConfig {
            id: "three_block".to_string(),
            label: "三格跳".to_string(),
            course_id: "standard".to_string(),
        },
        PracticeConfig {
            id: "triple_dense".to_string(),
            label: "密集三格跳".to_string(),
            course_id: "standard".to_string(),
        },
        PracticeConfig {
            id: "quad".to_string(),
            label: "四格跳".to_string(),
            course_id: "hardcore".to_string(),
        },
        PracticeConfig {
            id: "quad_dense".to_string(),
            label: "密集四格跳".to_string(),
            course_id: "hardcore".to_string(),
        },
        PracticeConfig {
            id: "mixed_gaps".to_string(),
            label: "单双三混合随机跳".to_string(),
            course_id: "standard".to_string(),
        },
        PracticeConfig {
            id: "neo".to_string(),
            label: "Neo 跳".to_string(),
            course_id: "hardcore".to_string(),
        },
        PracticeConfig {
            id: "head_hitter".to_string(),
            label: "顶头跳".to_string(),
            course_id: "standard".to_string(),
        },
        PracticeConfig {
            id: "ladder".to_string(),
            label: "梯子跳".to_string(),
            course_id: "standard".to_string(),
        },
        PracticeConfig {
            id: "ice".to_string(),
            label: "冰面跳".to_string(),
            course_id: "standard".to_string(),
        },
        PracticeConfig {
            id: "slime".to_string(),
            label: "史莱姆跳".to_string(),
            course_id: "standard".to_string(),
        },
        PracticeConfig {
            id: "strafe".to_string(),
            label: "侧向跳".to_string(),
            course_id: "standard".to_string(),
        },
        PracticeConfig {
            id: "fence".to_string(),
            label: "栅栏跳".to_string(),
            course_id: "standard".to_string(),
        },
        PracticeConfig {
            id: "trapdoor".to_string(),
            label: "活板门跳".to_string(),
            course_id: "standard".to_string(),
        },
        PracticeConfig {
            id: "dropper".to_string(),
            label: "落水跳".to_string(),
            course_id: "standard".to_string(),
        },
        PracticeConfig {
            id: "pane".to_string(),
            label: "玻璃板跳".to_string(),
            course_id: "standard".to_string(),
        },
        PracticeConfig {
            id: "iron_bar".to_string(),
            label: "铁栏杆跳".to_string(),
            course_id: "standard".to_string(),
        },
        PracticeConfig {
            id: "chain".to_string(),
            label: "锁链跳".to_string(),
            course_id: "standard".to_string(),
        },
        PracticeConfig {
            id: "end_rod".to_string(),
            label: "末地烛跳".to_string(),
            course_id: "standard".to_string(),
        },
        PracticeConfig {
            id: "honey".to_string(),
            label: "蜂蜜块跳".to_string(),
            course_id: "standard".to_string(),
        },
        PracticeConfig {
            id: "soul_sand".to_string(),
            label: "灵魂沙跳".to_string(),
            course_id: "standard".to_string(),
        },
        PracticeConfig {
            id: "ladder_neo".to_string(),
            label: "梯子 Neo 跳".to_string(),
            course_id: "hardcore".to_string(),
        },
    ]
}

const DEFAULT_CONFIG: &str = r#"# 跑酷子服插件配置
enable = true
menu_id = "parkour"
default_course = "standard"
instance_spacing = 256
instance_origin_x = 1024
instance_origin_y = -52
instance_origin_z = 1024
fall_reset_distance = 14.0

[lobby]
dimension = "qexed:parkour_lobby"
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
npc_display_name = "{\"text\":\"跑酷入口\",\"color\":\"aqua\"}"

[leaderboard_display]
enable = true
mode = "race"
x = 4.5
y = -50.0
z = 4.5
yaw = 180.0
line_spacing = 0.32
entity_type = "minecraft:text_display"

[[courses]]
id = "standard"
label = "标准跑酷"
dimension = "qexed:parkour_course"
length = 240
path_width = 3
checkpoint_interval = 32
segment_min_length = 3
segment_max_length = 6
floor_block = "minecraft:quartz_block"
min_level = 1
par_time_ms = 90000
gap_weight = 34
narrow_weight = 18
ice_weight = 10
slime_weight = 12
head_hitter_weight = 10
neo_weight = 8
ladder_weight = 5
dropper_weight = 3

[[courses]]
id = "hardcore"
label = "硬核跑酷"
dimension = "qexed:parkour_course"
length = 280
path_width = 3
checkpoint_interval = 36
segment_min_length = 3
segment_max_length = 5
floor_block = "minecraft:deepslate_tiles"
min_level = 4
par_time_ms = 100000
gap_weight = 36
narrow_weight = 18
ice_weight = 10
slime_weight = 10
head_hitter_weight = 12
neo_weight = 12
ladder_weight = 8
dropper_weight = 4

[[exams]]
id = "novice"
label = "新手考核"
course_id = "standard"
time_limit_ms = 120000
rank_reward = 1

[[exams]]
id = "expert"
label = "高手考核"
course_id = "hardcore"
time_limit_ms = 110000
rank_reward = 2

[[exams]]
id = "master"
label = "大师考核"
course_id = "hardcore"
time_limit_ms = 90000
rank_reward = 4

[[practice]]
id = "single"
label = "单格跳"
course_id = "standard"

[[practice]]
id = "single_dense"
label = "密集单格跳"
course_id = "standard"

[[practice]]
id = "double"
label = "双格跳"
course_id = "standard"

[[practice]]
id = "double_dense"
label = "密集双格跳"
course_id = "standard"

[[practice]]
id = "three_block"
label = "三格跳"
course_id = "standard"

[[practice]]
id = "triple_dense"
label = "密集三格跳"
course_id = "standard"

[[practice]]
id = "quad"
label = "四格跳"
course_id = "hardcore"

[[practice]]
id = "quad_dense"
label = "密集四格跳"
course_id = "hardcore"

[[practice]]
id = "mixed_gaps"
label = "单双三混合随机跳"
course_id = "standard"

[[practice]]
id = "neo"
label = "Neo 跳"
course_id = "hardcore"

[[practice]]
id = "head_hitter"
label = "顶头跳"
course_id = "standard"

[[practice]]
id = "ladder"
label = "梯子跳"
course_id = "standard"

[[practice]]
id = "ice"
label = "冰面跳"
course_id = "standard"

[[practice]]
id = "slime"
label = "史莱姆跳"
course_id = "standard"

[[practice]]
id = "strafe"
label = "侧向跳"
course_id = "standard"

[[practice]]
id = "fence"
label = "栅栏跳"
course_id = "standard"

[[practice]]
id = "trapdoor"
label = "活板门跳"
course_id = "standard"

[[practice]]
id = "dropper"
label = "落水跳"
course_id = "standard"

[[practice]]
id = "pane"
label = "玻璃板跳"
course_id = "standard"

[[practice]]
id = "iron_bar"
label = "铁栏杆跳"
course_id = "standard"

[[practice]]
id = "chain"
label = "锁链跳"
course_id = "standard"

[[practice]]
id = "end_rod"
label = "末地烛跳"
course_id = "standard"

[[practice]]
id = "honey"
label = "蜂蜜块跳"
course_id = "standard"

[[practice]]
id = "soul_sand"
label = "灵魂沙跳"
course_id = "standard"

[[practice]]
id = "ladder_neo"
label = "梯子 Neo 跳"
course_id = "hardcore"
"#;

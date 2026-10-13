//! temple_run：v4 → v6 原生插件移植 —— 神庙逃亡（自动奔跑 + 换道/跳/滑 + 追击者）。
//!
//! v4 导出 → v6 事件映射：
//! - command_execute  → ChatCommandEvent（cancel 接管 /temple）
//! - player_input     → PlayerInputEvent（左右换道 / 跳 / 滑）
//! - player_move      → PlayerMoveEvent（drive=false：只记录轨迹/判定失败）
//! - player_tick      → PlayerTickEvent（drive=true：驱动速度/建造/追击者/BossBar）
//! - 其余 join/leave/npc/placeholders/config_reload 一一对应。
//! 语义差异：v6 entity_move 用 NpcPosition；world_set_blocks 返回写入数（0 = 失败）。

use qexed_plugin_api::payload::{NpcMutationOp, NpcPosition, NpcUpsert, PlayerAction as Pa};
use qexed_plugin_api::services::{RuntimeEntitySpec, WorldEditRegionSpec};
use qexed_plugin_api::{
    ChatCommandEvent, ConfigReloadEvent, EventContext, LogLevel, NpcInteractEvent,
    NpcMutationsEvent, PlaceholdersEvent, PlayerInfo, PlayerInputEvent, PlayerJoinEvent,
    PlayerLeaveEvent, PlayerMoveEvent, PlayerTickEvent,
};
use qexed_plugin_sdk::{InitFuture, PluginContext};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::{Mutex, OnceLock};

const COMMAND_NAME: &str = "temple";
const NPC_KEY: &str = "temple_run_starter"; // v4 "temple_run:starter"：v6 存储键不允许冒号
const NPC_EVENT: &str = "temple_start";
const MENU_ID: &str = "temple_run";
const BUILD_BAR_ID: &str = "temple_run_loading";
const RUN_BAR_ID: &str = "temple_run_run";
const SESSION_PREFIX: &str = "temple_session";
const BEST_DISTANCE_PREFIX: &str = "temple_best_distance";
const BUILD_BLOCKS_PER_TICK: usize = 1800;
const WORLD_EDIT_BATCH_LIMIT: usize = 4096;
const TRACE_LIMIT: usize = 192;
const TRACE_SAMPLE_MIN_DISTANCE: f64 = 0.45;
const MONSTER_MAX_DISTANCE: f64 = 10.0;
const MONSTER_TARGET_DISTANCE: f64 = 8.5;
const MONSTER_CATCH_DISTANCE: f64 = 1.2;
const LANE_SPACING: f64 = 3.0;
const MIN_LANE: i32 = -1;
const MAX_LANE: i32 = 1;
const ACTION_NEVER_DISTANCE: f64 = -1_000_000_000.0;

qexed_plugin_sdk::declare_simple_plugin!("temple_run", "Temple Run", "0.1.0", |ctx: PluginContext| -> InitFuture {
    Box::pin(async move {
        let plugin_id: &'static str = ctx.id();
        let sdk = ctx.sdk();
        let _ = sdk.config_load_or_create(DEFAULT_CONFIG);
        let config = load_config(&sdk);
        register_lobby_region(&sdk, &config);
        ctx.register_command(COMMAND_NAME);
        ctx.log(LogLevel::Info, "temple_run initialized");

        ctx.register::<ChatCommandEvent>(move |api, ev, ectx: &EventContext| {
            let plugin_id = plugin_id;
            Box::pin(async move {
                if ev.command != COMMAND_NAME {
                    return;
                }
                ectx.cancel();
                let sdk = qexed_plugin_sdk::Sdk::new(api, plugin_id);
                let config = load_config(&sdk);
                let mut parts = ev.argument.split_whitespace();
                let actions = match parts.next().unwrap_or_default() {
                    "" | "menu" => vec![Pa::OpenMenu { menu: config.menu_id.clone() }],
                    "start" => start_loading(&sdk, &config, &ev.player.uuid, parts.next().unwrap_or("normal")),
                    "leave" | "lobby" | "spawn" => leave_to_lobby(&sdk, &config, &ev.player.uuid),
                    _ => vec![message(
                        "用法: /temple menu | /temple start <easy|normal|hard> | /temple leave",
                    )],
                };
                if !actions.is_empty() {
                    sdk.perform_all(&actions.iter().map(|a| (&ev.player, a.clone())).collect::<Vec<_>>());
                }
            })
        });

        ctx.register::<PlayerJoinEvent>(move |api, ev, _| {
            let plugin_id = plugin_id;
            Box::pin(async move {
                let sdk = qexed_plugin_sdk::Sdk::new(api, plugin_id);
                let config = load_config(&sdk);
                if ev.player.dimension == config.lobby.dimension {
                    clear_player_runtime(&sdk, &ev.player.uuid);
                    let _ = sdk.storage_set_json(
                        &session_key(&ev.player.uuid),
                        &TempleSession::lobby(&config.lobby.dimension),
                    );
                }
            })
        });

        ctx.register::<PlayerLeaveEvent>(move |api, ev, _| {
            let plugin_id = plugin_id;
            Box::pin(async move {
                let sdk = qexed_plugin_sdk::Sdk::new(api, plugin_id);
                clear_player_runtime(&sdk, &ev.player.uuid);
            })
        });

        ctx.register::<PlayerInputEvent>(move |api, ev, _| {
            let plugin_id = plugin_id;
            Box::pin(async move {
                let sdk = qexed_plugin_sdk::Sdk::new(api, plugin_id);
                let config = load_config(&sdk);
                let actions = handle_player_input(&sdk, &config, &ev);
                if !actions.is_empty() {
                    sdk.perform_all(&actions.iter().map(|a| (&ev.player, a.clone())).collect::<Vec<_>>());
                }
            })
        });

        ctx.register::<PlayerMoveEvent>(move |api, ev, _| {
            let plugin_id = plugin_id;
            Box::pin(async move {
                let sdk = qexed_plugin_sdk::Sdk::new(api, plugin_id);
                let config = load_config(&sdk);
                let actions = update_running_player(
                    &sdk,
                    &config,
                    &ev.player.uuid,
                    &ev.dimension,
                    ev.position.x,
                    ev.position.y,
                    ev.position.z,
                    50,
                    false,
                );
                if !actions.is_empty() {
                    sdk.perform_all(&actions.iter().map(|a| (&ev.player, a.clone())).collect::<Vec<_>>());
                }
            })
        });

        ctx.register::<PlayerTickEvent>(move |api, ev, _| {
            let plugin_id = plugin_id;
            Box::pin(async move {
                let sdk = qexed_plugin_sdk::Sdk::new(api, plugin_id);
                let config = load_config(&sdk);
                let mut actions = continue_build(&sdk, &config, &ev.player.uuid);
                actions.extend(update_running_player(
                    &sdk,
                    &config,
                    &ev.player.uuid,
                    &ev.dimension,
                    ev.position.x,
                    ev.position.y,
                    ev.position.z,
                    ev.tick_millis,
                    true,
                ));
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
                        name: "Temple Run".to_string(),
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
                let player = PlayerInfo {
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
                    .storage_get_json::<TempleSession>(&session_key(&player.uuid))
                    .unwrap_or_else(|| TempleSession::lobby("qexed:temple_lobby"));
                let running = session.active;
                let elapsed = if running {
                    format_duration(elapsed_ms(session.started_at_ms))
                } else {
                    "--:--".to_string()
                };
                let best = if session.difficulty.is_empty() {
                    ["easy", "normal", "hard"]
                        .into_iter()
                        .filter_map(|difficulty| sdk.storage_get_json::<i64>(&best_key(&player.uuid, difficulty)))
                        .max()
                } else {
                    sdk.storage_get_json::<i64>(&best_key(&player.uuid, &session.difficulty))
                }
                .map(|value| format!("{value}m"))
                .unwrap_or_else(|| "0m".to_string());
                let difficulty_label = if session.difficulty_label.is_empty() {
                    "大厅".to_string()
                } else {
                    session.difficulty_label.clone()
                };
                let mut response = ev.response.lock().unwrap();
                response.replacements = vec![
                    qexed_plugin_api::PlaceholderReplacement { key: "temple_difficulty".into(), value: difficulty_label },
                    qexed_plugin_api::PlaceholderReplacement { key: "temple_current_time".into(), value: elapsed },
                    qexed_plugin_api::PlaceholderReplacement { key: "temple_best_time".into(), value: best.clone() },
                    qexed_plugin_api::PlaceholderReplacement { key: "temple_best_distance".into(), value: best },
                    qexed_plugin_api::PlaceholderReplacement {
                        key: "temple_distance".into(),
                        value: if running { format!("{:.0}m", session.distance.max(0.0)) } else { "0m".to_string() },
                    },
                    qexed_plugin_api::PlaceholderReplacement { key: "temple_coins".into(), value: session.coins.to_string() },
                    qexed_plugin_api::PlaceholderReplacement { key: "temple_score".into(), value: session.score().to_string() },
                    qexed_plugin_api::PlaceholderReplacement {
                        key: "temple_speed".into(),
                        value: if running { format!("{:.1}m/s", session.speed_blocks_per_second()) } else { "0.0m/s".to_string() },
                    },
                ];
            })
        });

        ctx.register::<ConfigReloadEvent>(move |api, ev, _| {
            let plugin_id = plugin_id;
            Box::pin(async move {
                let path = ev.payload.path.replace('\\', "/");
                if !(path.ends_with("temple_run/config.toml")
                    || path.ends_with("config.toml") && path.contains("temple"))
                {
                    return;
                }
                let sdk = qexed_plugin_sdk::Sdk::new(api, plugin_id);
                let config = load_config(&sdk);
                register_lobby_region(&sdk, &config);
                api.log(LogLevel::Info, "temple_run config reloaded");
            })
        });
    })
});

// ─────────────────────────── 运行时状态 ───────────────────────────

#[derive(Default)]
struct RuntimeState {
    sessions: HashMap<String, RuntimeSession>,
    builds: HashMap<String, BuildJob>,
}

fn runtime_state() -> &'static Mutex<RuntimeState> {
    static STATE: OnceLock<Mutex<RuntimeState>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(RuntimeState::default()))
}

#[derive(Debug, Clone)]
struct RuntimeSession {
    session: TempleSession,
    trace: VecDeque<TracePoint>,
    monster: TracePoint,
    last_trace: TracePoint,
    planned_x: f64,
    last_bossbar_bucket: i64,
    spawned: bool,
    pending_build: Option<PendingBuild>,
}

#[derive(Debug, Clone)]
struct BuildJob {
    session: TempleSession,
    blocks: Vec<TempleBlock>,
    written: usize,
}

#[derive(Debug, Clone)]
struct PendingBuild {
    target_z: i32,
    blocks: Vec<TempleBlock>,
    written: usize,
}

#[derive(Debug, Clone)]
struct TempleBlock {
    dimension: String,
    position: (i32, i32, i32),
    block: String,
}

#[derive(Debug, Clone, Copy)]
struct TracePoint {
    x: f64,
    y: f64,
    z: f64,
}

fn trace_distance(a: TracePoint, b: TracePoint) -> f64 {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    let dz = a.z - b.z;
    (dx * dx + dy * dy + dz * dz).sqrt()
}

fn step_towards(from: TracePoint, to: TracePoint, max_step: f64) -> TracePoint {
    let d = trace_distance(from, to);
    if d <= f64::EPSILON || d <= max_step {
        return to;
    }
    let t = max_step / d;
    TracePoint {
        x: from.x + (to.x - from.x) * t,
        y: from.y + (to.y - from.y) * t,
        z: from.z + (to.z - from.z) * t,
    }
}

fn point_towards(origin: TracePoint, point: TracePoint, radius: f64) -> TracePoint {
    let d = trace_distance(origin, point);
    if d <= f64::EPSILON {
        return TracePoint { x: origin.x, y: origin.y, z: origin.z - radius };
    }
    let t = radius / d;
    TracePoint {
        x: origin.x + (point.x - origin.x) * t,
        y: origin.y + (point.y - origin.y) * t,
        z: origin.z + (point.z - origin.z) * t,
    }
}

fn yaw_to(from: TracePoint, to: TracePoint) -> f32 {
    let dx = to.x - from.x;
    let dz = to.z - from.z;
    (-dx.atan2(dz).to_degrees()) as f32
}

// ─────────────────────────── 游戏流程 ───────────────────────────

fn start_loading(
    sdk: &qexed_plugin_sdk::Sdk,
    config: &Config,
    player_uuid: &str,
    difficulty_id: &str,
) -> Vec<Pa> {
    if !config.enable {
        return Vec::new();
    }
    let Some(difficulty) = config.difficulty(difficulty_id).cloned() else {
        return vec![message("未知难度，可选: easy, normal, hard")];
    };
    clear_player_runtime(sdk, player_uuid);
    let seed = stable_seed(player_uuid, &difficulty.id, now_millis());
    let origin = instance_origin(config, player_uuid, &difficulty);
    register_instance_region(sdk, &difficulty, origin);
    let course = generate_course(&difficulty, seed, difficulty.prebuild_length());
    let blocks = course_blocks_range(&difficulty, origin, &course, -16, course.length + 32);
    let session = TempleSession::new(&difficulty, origin, seed, course);
    {
        let mut runtime = runtime_state().lock().expect("temple runtime poisoned");
        runtime.builds.insert(
            player_uuid.to_string(),
            BuildJob { session, blocks, written: 0 },
        );
    }
    vec![
        Pa::SetPlayersVisible { visible: false },
        loading_bar(
            &format!("正在生成 {} 神庙逃亡赛道", difficulty.label),
            0.01,
            "yellow",
        ),
        message(format!(
            "正在生成 {} 神庙逃亡赛道，请稍等。进入后会自动向前奔跑。",
            difficulty.label
        )),
    ]
}

fn continue_build(sdk: &qexed_plugin_sdk::Sdk, config: &Config, player_uuid: &str) -> Vec<Pa> {
    let mut finished = None;
    let mut progress = None;
    {
        let mut runtime = runtime_state().lock().expect("temple runtime poisoned");
        let Some(build) = runtime.builds.get_mut(player_uuid) else {
            return Vec::new();
        };
        let start = build.written;
        let end = (start + BUILD_BLOCKS_PER_TICK).min(build.blocks.len());
        let refs: Vec<(&str, (i32, i32, i32), &str)> = build.blocks[start..end]
            .iter()
            .map(|block| (block.dimension.as_str(), block.position, block.block.as_str()))
            .collect();
        if sdk.world_set_blocks(&refs) == 0 {
            return vec![loading_bar("神庙赛道生成重试中", build_progress(build), "red")];
        }
        build.written = end;
        if build.written >= build.blocks.len() {
            finished = Some(build.session.clone());
        } else {
            progress = Some((build.session.difficulty_label.clone(), build_progress(build)));
        }
    }

    if let Some((label, value)) = progress {
        return vec![loading_bar(
            &format!("正在生成 {label} 神庙逃亡 {}%", (value * 100.0) as i32),
            value,
            "yellow",
        )];
    }

    let Some(mut session) = finished else {
        return Vec::new();
    };
    {
        let mut runtime = runtime_state().lock().expect("temple runtime poisoned");
        runtime.builds.remove(player_uuid);
        session.started_at_ms = now_millis();
        session.last_tick_at_ms = session.started_at_ms;
        let mut trace = VecDeque::new();
        let start = TracePoint {
            x: session.start_x,
            y: session.start_y,
            z: session.start_z,
        };
        trace.push_back(start);
        let monster = monster_start(&session);
        runtime.sessions.insert(
            player_uuid.to_string(),
            RuntimeSession {
                session: session.clone(),
                trace,
                monster,
                last_trace: start,
                planned_x: start.x,
                last_bossbar_bucket: -1,
                spawned: false,
                pending_build: None,
            },
        );
    }
    let _ = sdk.storage_set_json(&session_key(player_uuid), &session);
    spawn_runner_entity(sdk, player_uuid, &config.monster, &session);

    vec![
        Pa::RemoveBossBar { id: BUILD_BAR_ID.to_string() },
        Pa::BossBar {
            id: RUN_BAR_ID.to_string(),
            title: "神庙逃亡开始".to_string(),
            progress: 1.0,
            color: "green".to_string(),
            overlay: "progress".to_string(),
        },
        message(format!(
            "进入 {} 神庙逃亡：自动前进，左右换道，跳跃越过断路，潜行滑过低障碍。",
            session.difficulty_label
        )),
        Pa::Teleport {
            dimension: session.dimension.clone(),
            x: session.start_x,
            y: session.start_y,
            z: session.start_z,
            yaw: Some(0.0),
            pitch: Some(0.0),
        },
    ]
}

fn handle_player_input(sdk: &qexed_plugin_sdk::Sdk, config: &Config, ev: &PlayerInputEvent) -> Vec<Pa> {
    let now = now_millis();
    let mut handled = false;
    let mut persist: Option<TempleSession> = None;
    {
        let mut runtime = runtime_state().lock().expect("temple runtime poisoned");
        let Some(state) = runtime.sessions.get_mut(&ev.player.uuid) else {
            return Vec::new();
        };
        if !state.session.active || state.session.dimension != ev.dimension {
            return Vec::new();
        }
        let input_distance = (ev.position.z - state.session.start_z).max(state.session.distance);

        if ev.input.left && !ev.previous_input.left {
            state.session.target_lane = shift_lane_on_track(state.session.target_lane, 1);
            state.planned_x = lane_x(&state.session, state.session.target_lane);
            handled = true;
        }
        if ev.input.right && !ev.previous_input.right {
            state.session.target_lane = shift_lane_on_track(state.session.target_lane, -1);
            state.planned_x = lane_x(&state.session, state.session.target_lane);
            handled = true;
        }
        if ev.input.jump && !ev.previous_input.jump {
            state.session.last_jump_distance = input_distance;
            state.session.jump_until_ms = now + config.controls.jump_ms;
            handled = true;
        }
        if ev.input.shift {
            state.session.slide_held = true;
            state.session.last_slide_distance = input_distance;
            state.session.slide_until_ms = now + config.controls.slide_ms;
            handled = true;
        } else if ev.previous_input.shift {
            state.session.slide_held = false;
            handled = true;
        }
        if handled {
            persist = Some(state.session.clone());
        }
    }
    if let Some(session) = persist {
        let _ = sdk.storage_set_json(&session_key(&ev.player.uuid), &session);
    }
    if handled {
        vec![Pa::BossBar {
            id: RUN_BAR_ID.to_string(),
            title: "神庙逃亡".to_string(),
            progress: 1.0,
            color: "green".to_string(),
            overlay: "progress".to_string(),
        }]
    } else {
        Vec::new()
    }
}


#[allow(clippy::too_many_arguments)]
fn update_running_player(
    sdk: &qexed_plugin_sdk::Sdk,
    config: &Config,
    player_uuid: &str,
    dimension: &str,
    x: f64,
    y: f64,
    z: f64,
    tick_millis: u64,
    drive_player: bool,
) -> Vec<Pa> {
    let now = now_millis();
    let mut failure = None;
    let mut actions = Vec::new();
    {
        let mut runtime = runtime_state().lock().expect("temple runtime poisoned");
        let Some(state) = runtime.sessions.get_mut(player_uuid) else {
            return Vec::new();
        };
        if !state.session.active || state.session.dimension != dimension {
            return Vec::new();
        }

        if let Some(difficulty) = config.difficulty(&state.session.difficulty) {
            flush_pending_build(sdk, state);
            schedule_course_extension(config, difficulty, state);
        }

        state.session.last_tick_at_ms = now;
        state.session.distance = (z - state.session.start_z).max(state.session.distance);
        let point = TracePoint { x, y, z };
        if trace_distance(point, state.last_trace) >= TRACE_SAMPLE_MIN_DISTANCE {
            state.trace.push_back(point);
            state.last_trace = point;
            while state.trace.len() > TRACE_LIMIT {
                state.trace.pop_front();
            }
        }

        let target_x = lane_x(&state.session, state.session.target_lane);
        if !state.planned_x.is_finite() || (state.planned_x - x).abs() > LANE_SPACING * 4.0 {
            state.planned_x = target_x;
        }
        if drive_player {
            let _ = smooth_lane_delta(&mut state.planned_x, target_x, &config.controls);
        }
        let rule_x = state.planned_x;

        collect_coins(&mut state.session, rule_x, z);
        if state.session.slide_held {
            state.session.last_slide_distance = state.session.distance;
            state.session.slide_until_ms = now + config.controls.slide_ms;
        }
        if y < state.session.min_y {
            failure = Some("跌落神庙");
        } else if let Some(reason) = obstacle_failure(
            &state.session,
            rule_x,
            y,
            z,
            now,
            config.controls.action_buffer_blocks,
            config.controls.action_late_blocks,
        ) {
            failure = Some(reason);
        }

        move_monster(sdk, config, player_uuid, state, point);
        if trace_distance(state.monster, point) <= MONSTER_CATCH_DISTANCE {
            failure = Some("被追击者抓住");
        }

        if failure.is_none() && drive_player {
            let mut speed = state.session.speed_per_tick(tick_millis, config.controls.max_tick_scale);
            speed *= action_speed_multiplier(&state.session, now, &config.controls);
            let dx = smooth_actual_delta(x, state.planned_x, &config.controls);
            let dy = jump_delta(&state.session, y, now, config.controls.jump_lift_per_tick);
            actions.push(Pa::Velocity { x: dx, y: dy, z: speed, additive: true });
            let bossbar_bucket = (state.session.distance / 25.0).floor() as i64;
            if bossbar_bucket != state.last_bossbar_bucket {
                state.last_bossbar_bucket = bossbar_bucket;
                actions.push(Pa::BossBar {
                    id: RUN_BAR_ID.to_string(),
                    title: format!(
                        "{} | {:.0}m | 金币 {}",
                        state.session.difficulty_label, state.session.distance, state.session.coins
                    ),
                    progress: run_bar_progress(&state.session),
                    color: "green".to_string(),
                    overlay: "progress".to_string(),
                });
            }
        }
        let snapshot = state.session.clone();
        let _ = sdk.storage_set_json(&session_key(player_uuid), &snapshot);
    }

    if let Some(reason) = failure {
        finish_run(sdk, config, player_uuid, reason)
    } else {
        actions
    }
}

fn finish_run(sdk: &qexed_plugin_sdk::Sdk, config: &Config, player_uuid: &str, reason: &str) -> Vec<Pa> {
    let session = {
        let mut runtime = runtime_state().lock().expect("temple runtime poisoned");
        runtime.sessions.remove(player_uuid).map(|state| state.session)
    };
    let Some(session) = session else {
        return Vec::new();
    };
    let distance = session.distance.floor().max(0.0) as i64;
    let best_key = best_key(player_uuid, &session.difficulty);
    let old_best = sdk.storage_get_json::<i64>(&best_key).unwrap_or(0);
    if distance > old_best {
        let _ = sdk.storage_set_json(&best_key, &distance);
    }
    let _ = sdk.storage_set_json(
        &session_key(player_uuid),
        &TempleSession::lobby(&config.lobby.dimension),
    );
    remove_runner_entity(sdk, player_uuid);

    vec![
        Pa::RemoveBossBar { id: RUN_BAR_ID.to_string() },
        message(format!(
            "{reason}，本次距离 {distance}m，金币 {}，分数 {}。",
            session.coins,
            session.score()
        )),
        Pa::SetPlayersVisible { visible: true },
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

fn leave_to_lobby(sdk: &qexed_plugin_sdk::Sdk, config: &Config, player_uuid: &str) -> Vec<Pa> {
    clear_player_runtime(sdk, player_uuid);
    let _ = sdk.storage_set_json(
        &session_key(player_uuid),
        &TempleSession::lobby(&config.lobby.dimension),
    );
    vec![
        Pa::RemoveBossBar { id: BUILD_BAR_ID.to_string() },
        Pa::RemoveBossBar { id: RUN_BAR_ID.to_string() },
        Pa::SetPlayersVisible { visible: true },
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

fn clear_player_runtime(sdk: &qexed_plugin_sdk::Sdk, player_uuid: &str) {
    remove_runner_entity(sdk, player_uuid);
    let mut runtime = runtime_state().lock().expect("temple runtime poisoned");
    runtime.sessions.remove(player_uuid);
    runtime.builds.remove(player_uuid);
}

// ─────────────────────────── 世界构建 ───────────────────────────

fn register_lobby_region(sdk: &qexed_plugin_sdk::Sdk, config: &Config) {
    let _ = sdk.world_register_edit_region(&WorldEditRegionSpec {
        id: "temple_lobby".to_string(),
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
    let mut sink = BlockSink::new(sdk);
    let y = config.lobby.floor_y;
    for x in -14_i32..=14 {
        for z in -14_i32..=14 {
            let border = x.abs() == 14 || z.abs() == 14;
            sink.push(
                &config.lobby.dimension,
                (x, y - 1, z),
                if border {
                    "minecraft:chiseled_sandstone"
                } else {
                    "minecraft:smooth_sandstone"
                },
            );
            for air_y in y..=y + 6 {
                sink.push(&config.lobby.dimension, (x, air_y, z), "minecraft:air");
            }
        }
    }
    for z in [-14_i32, 14] {
        for x in -14..=14 {
            for dy in 0..=3 {
                sink.push(&config.lobby.dimension, (x, y + dy, z), "minecraft:sandstone");
            }
        }
    }
    for x in [-14_i32, 14] {
        for z in -14..=14 {
            for dy in 0..=3 {
                sink.push(&config.lobby.dimension, (x, y + dy, z), "minecraft:sandstone");
            }
        }
    }
    for (x, z) in [(-8, -8), (8, -8), (-8, 8), (8, 8)] {
        for dy in 0..=5 {
            sink.push(&config.lobby.dimension, (x, y + dy, z), "minecraft:cut_sandstone");
        }
        sink.push(&config.lobby.dimension, (x, y + 6, z), "minecraft:glowstone");
    }
    sink.flush();
}

fn register_instance_region(sdk: &qexed_plugin_sdk::Sdk, difficulty: &DifficultyConfig, origin: (i32, i32, i32)) {
    let half = difficulty.path_half_width();
    let max_z = origin.2
        + difficulty
            .region_length
            .max(difficulty.prebuild_length() + 1024);
    let _ = sdk.world_register_edit_region(&WorldEditRegionSpec {
        id: format!("temple_{}_{}", difficulty.id, origin.0),
        dimension: difficulty.dimension.clone(),
        min: (origin.0 - half - 8, origin.1 - 10, origin.2 - 24),
        max: (origin.0 + half + 8, origin.1 + 16, max_z),
        allow_player_break: false,
        allow_player_place: false,
        allow_plugin_write: true,
        runtime_only: true,
    });
}

fn schedule_course_extension(
    config: &Config,
    difficulty: &DifficultyConfig,
    state: &mut RuntimeSession,
) {
    if state.pending_build.is_some() {
        return;
    }
    let ahead = state.session.built_until - state.session.distance as i32;
    if ahead > config.controls.generate_ahead_blocks {
        return;
    }
    let start_z = state.session.built_until + 1;
    let target_z = start_z + difficulty.extend_length.max(64);
    let new_segments = generate_segments(difficulty, state.session.seed, state.session.generated_until, target_z);
    state.session.generated_until = new_segments
        .last()
        .map(|segment| segment.end_z())
        .unwrap_or(state.session.generated_until);
    state.session.segments.extend(new_segments);
    let course = Course {
        length: target_z,
        segments: state.session.segments.clone(),
    };
    let blocks = course_blocks_range(
        difficulty,
        (state.session.origin_x, state.session.origin_y, state.session.origin_z),
        &course,
        start_z,
        target_z + 32,
    );
    state.pending_build = Some(PendingBuild {
        target_z,
        blocks,
        written: 0,
    });
}

fn flush_pending_build(sdk: &qexed_plugin_sdk::Sdk, state: &mut RuntimeSession) {
    let Some(pending) = state.pending_build.as_mut() else {
        return;
    };
    let start = pending.written;
    let end = (start + BUILD_BLOCKS_PER_TICK).min(pending.blocks.len());
    let refs: Vec<(&str, (i32, i32, i32), &str)> = pending.blocks[start..end]
        .iter()
        .map(|block| (block.dimension.as_str(), block.position, block.block.as_str()))
        .collect();
    if sdk.world_set_blocks(&refs) == 0 {
        return;
    }
    pending.written = end;
    if pending.written >= pending.blocks.len() {
        state.session.built_until = pending.target_z;
        state.pending_build = None;
    }
}

fn generate_course(difficulty: &DifficultyConfig, seed: u64, target_z: i32) -> Course {
    let segments = generate_segments(difficulty, seed, 0, target_z);
    let length = segments
        .last()
        .map(|segment| segment.end_z())
        .unwrap_or(target_z);
    Course { length, segments }
}

fn generate_segments(difficulty: &DifficultyConfig, seed: u64, start_z: i32, target_z: i32) -> Vec<Segment> {
    let mut segments = Vec::new();
    let mut z = start_z;
    while z < target_z {
        let segment = generate_segment(difficulty, seed, z);
        z = segment.end_z();
        segments.push(segment);
    }
    segments
}

fn generate_segment(difficulty: &DifficultyConfig, seed: u64, start_z: i32) -> Segment {
    let mut rng = Rng::new(seed ^ (start_z as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15));
    let length = difficulty.segment_min_length
        + rng.range(
            (difficulty.segment_max_length - difficulty.segment_min_length + 1).max(1) as u32,
        ) as i32;
    if start_z < difficulty.obstacle_start_distance {
        return Segment {
            start_z,
            length,
            kind: if rng.range(3) == 0 {
                SegmentKind::CoinLine
            } else {
                SegmentKind::Straight
            },
            safe_lane: random_lane(&mut rng),
            blocked_lane: 0,
        };
    }

    let roll = rng.range(100);
    let kind = if roll < difficulty.gap_weight {
        SegmentKind::Gap
    } else if roll < difficulty.gap_weight + difficulty.low_barrier_weight {
        SegmentKind::LowBarrier
    } else if roll < difficulty.gap_weight + difficulty.low_barrier_weight + difficulty.turn_weight {
        SegmentKind::CoinLine
    } else if roll
        < difficulty.gap_weight
            + difficulty.low_barrier_weight
            + difficulty.turn_weight
            + difficulty.double_barrier_weight
    {
        SegmentKind::DoubleBarrier
    } else if roll
        < difficulty.gap_weight
            + difficulty.low_barrier_weight
            + difficulty.turn_weight
            + difficulty.double_barrier_weight
            + difficulty.coin_weight
    {
        SegmentKind::CoinLine
    } else {
        SegmentKind::LaneBarrier
    };
    let safe_lane = random_lane(&mut rng);
    let mut blocked_lane = random_lane(&mut rng);
    if kind == SegmentKind::DoubleBarrier {
        blocked_lane = safe_lane;
    }
    Segment { start_z, length, kind, safe_lane, blocked_lane }
}

fn random_lane(rng: &mut Rng) -> i32 {
    match rng.range(3) {
        0 => -1,
        1 => 0,
        _ => 1,
    }
}

fn course_blocks_range(
    difficulty: &DifficultyConfig,
    origin: (i32, i32, i32),
    course: &Course,
    from_z: i32,
    to_z: i32,
) -> Vec<TempleBlock> {
    let mut blocks = Vec::new();
    let half = difficulty.path_half_width();
    let base_y = origin.1;
    let dimension = difficulty.dimension.clone();
    for local_z in from_z..=to_z {
        let world_z = origin.2 + local_z;
        for x in -half - 3..=half + 3 {
            for y in base_y..=base_y + 6 {
                blocks.push(TempleBlock {
                    dimension: dimension.clone(),
                    position: (origin.0 + x, y, world_z),
                    block: "minecraft:air".to_string(),
                });
            }
            blocks.push(TempleBlock {
                dimension: dimension.clone(),
                position: (origin.0 + x, base_y - 1, world_z),
                block: if local_z < 0 {
                    "minecraft:cut_sandstone".to_string()
                } else if x.abs() <= half {
                    "minecraft:smooth_sandstone".to_string()
                } else {
                    "minecraft:chiseled_sandstone".to_string()
                },
            });
        }
        for side_x in [-half - 3, half + 3] {
            for dy in 0..=2 {
                blocks.push(TempleBlock {
                    dimension: dimension.clone(),
                    position: (origin.0 + side_x, base_y + dy, world_z),
                    block: "minecraft:sandstone_wall".to_string(),
                });
            }
        }
    }

    for segment in course
        .segments
        .iter()
        .filter(|segment| segment.end_z() >= from_z && segment.start_z <= to_z)
    {
        apply_segment_blocks(difficulty, origin, segment, &mut blocks);
    }
    blocks
}

fn apply_segment_blocks(
    difficulty: &DifficultyConfig,
    origin: (i32, i32, i32),
    segment: &Segment,
    blocks: &mut Vec<TempleBlock>,
) {
    let half = difficulty.path_half_width();
    let base_y = origin.1;
    let dimension = difficulty.dimension.clone();
    let mid_z = origin.2 + segment.start_z + segment.length / 2;
    match segment.kind {
        SegmentKind::Straight => {}
        SegmentKind::CoinLine => {
            for local_z in (segment.start_z + 2..segment.end_z() - 1).step_by(2) {
                let x = lane_block_center(segment.safe_lane);
                blocks.push(TempleBlock {
                    dimension: dimension.clone(),
                    position: (origin.0 + x, base_y, origin.2 + local_z),
                    block: "minecraft:light_weighted_pressure_plate".to_string(),
                });
            }
        }
        SegmentKind::Gap => {
            for dz in 0..=2 {
                for x in -half..=half {
                    blocks.push(TempleBlock {
                        dimension: dimension.clone(),
                        position: (origin.0 + x, base_y - 1, mid_z + dz),
                        block: "minecraft:air".to_string(),
                    });
                }
            }
            mark_warning_line(&dimension, origin, base_y, mid_z - 1, blocks);
        }
        SegmentKind::LowBarrier => {
            for dz in 0..=1 {
                for x in -half..=half {
                    blocks.push(TempleBlock {
                        dimension: dimension.clone(),
                        position: (origin.0 + x, base_y + 2, mid_z + dz),
                        block: "minecraft:cut_sandstone".to_string(),
                    });
                }
            }
            mark_warning_line(&dimension, origin, base_y, mid_z - 1, blocks);
        }
        SegmentKind::LaneBarrier => {
            place_lane_barrier(&dimension, origin, base_y, mid_z, segment.blocked_lane, blocks);
        }
        SegmentKind::DoubleBarrier => {
            for lane in MIN_LANE..=MAX_LANE {
                if lane != segment.safe_lane {
                    place_lane_barrier(&dimension, origin, base_y, mid_z, lane, blocks);
                }
            }
        }
        SegmentKind::TurnLeft | SegmentKind::TurnRight => {
            let block = if segment.kind == SegmentKind::TurnLeft {
                "minecraft:blue_carpet"
            } else {
                "minecraft:red_carpet"
            };
            for dz in 0..=2 {
                for x in -half..=half {
                    blocks.push(TempleBlock {
                        dimension: dimension.clone(),
                        position: (origin.0 + x, base_y, mid_z + dz),
                        block: block.to_string(),
                    });
                }
            }
        }
    }
}

fn mark_warning_line(
    dimension: &str,
    origin: (i32, i32, i32),
    base_y: i32,
    world_z: i32,
    blocks: &mut Vec<TempleBlock>,
) {
    for x in -4..=4 {
        blocks.push(TempleBlock {
            dimension: dimension.to_string(),
            position: (origin.0 + x, base_y, world_z),
            block: "minecraft:orange_carpet".to_string(),
        });
    }
}

fn place_lane_barrier(
    dimension: &str,
    origin: (i32, i32, i32),
    base_y: i32,
    world_z: i32,
    lane: i32,
    blocks: &mut Vec<TempleBlock>,
) {
    let center = lane_block_center(lane);
    for dz in 0..=1 {
        for dx in center - 1..=center + 1 {
            for dy in 0..=1 {
                blocks.push(TempleBlock {
                    dimension: dimension.to_string(),
                    position: (origin.0 + dx, base_y + dy, world_z + dz),
                    block: "minecraft:cut_sandstone".to_string(),
                });
            }
        }
    }
}

// ─────────────────────────── 障碍 / 金币 / 追击者 ───────────────────────────

fn obstacle_failure(
    session: &TempleSession,
    x: f64,
    y: f64,
    z: f64,
    _now: i64,
    action_buffer_blocks: f64,
    action_late_blocks: f64,
) -> Option<&'static str> {
    let local_z = (z - session.origin_z as f64).floor() as i32;
    let segment = session
        .segments
        .iter()
        .find(|segment| local_z >= segment.start_z && local_z <= segment.end_z())?;
    let mid_start = segment.start_z + segment.length / 2;
    let in_short_window = local_z >= mid_start && local_z <= mid_start + 2;
    let obstacle_distance = mid_start as f64;
    match segment.kind {
        SegmentKind::Gap if in_short_window => {
            if !jump_ready(session, obstacle_distance, action_buffer_blocks, action_late_blocks)
                && y <= session.start_y + 0.75
            {
                Some("没有跳过断路")
            } else {
                None
            }
        }
        SegmentKind::LowBarrier if in_short_window => {
            if !slide_ready(session, obstacle_distance, action_buffer_blocks, action_late_blocks) {
                Some("没有滑过低障碍")
            } else {
                None
            }
        }
        SegmentKind::LaneBarrier
            if in_short_window && lane_barrier_hit(session, x, segment.blocked_lane) =>
        {
            Some("撞上石墙")
        }
        SegmentKind::DoubleBarrier
            if in_short_window && double_barrier_hit(session, x, segment.safe_lane) =>
        {
            Some("没有换到安全车道")
        }
        SegmentKind::TurnLeft | SegmentKind::TurnRight => None,
        _ => None,
    }
}

fn jump_ready(session: &TempleSession, obstacle_distance: f64, action_buffer_blocks: f64, action_late_blocks: f64) -> bool {
    action_distance_matches(session.last_jump_distance, obstacle_distance, action_buffer_blocks, action_late_blocks)
}

fn slide_ready(session: &TempleSession, obstacle_distance: f64, action_buffer_blocks: f64, action_late_blocks: f64) -> bool {
    session.slide_held
        || action_distance_matches(session.last_slide_distance, obstacle_distance, action_buffer_blocks, action_late_blocks)
}

fn action_distance_matches(action_distance: f64, obstacle_distance: f64, action_buffer_blocks: f64, action_late_blocks: f64) -> bool {
    action_distance >= obstacle_distance - action_buffer_blocks
        && action_distance <= obstacle_distance + action_late_blocks
}

fn lane_barrier_hit(session: &TempleSession, x: f64, blocked_lane: i32) -> bool {
    (x - lane_x(session, blocked_lane)).abs() <= 1.05
}

fn double_barrier_hit(session: &TempleSession, x: f64, safe_lane: i32) -> bool {
    (MIN_LANE..=MAX_LANE)
        .filter(|lane| *lane != safe_lane)
        .any(|lane| lane_barrier_hit(session, x, lane))
}

fn collect_coins(session: &mut TempleSession, x: f64, z: f64) {
    let local_z = (z - session.origin_z as f64).floor() as i32;
    let lane = nearest_lane(session, x);
    let Some(segment) = session
        .segments
        .iter()
        .find(|segment| local_z >= segment.start_z && local_z <= segment.end_z())
    else {
        return;
    };
    if segment.kind == SegmentKind::CoinLine
        && segment.safe_lane == lane
        && segment.start_z > session.last_coin_segment_z
    {
        session.coins += 5;
        session.last_coin_segment_z = segment.start_z;
    }
}

fn move_monster(
    sdk: &qexed_plugin_sdk::Sdk,
    config: &Config,
    player_uuid: &str,
    state: &mut RuntimeSession,
    player: TracePoint,
) {
    if !state.spawned {
        spawn_runner_entity(sdk, player_uuid, &config.monster, &state.session);
        state.spawned = true;
    }
    let target = trace_target(&state.trace, player).unwrap_or(player);
    let current_distance = trace_distance(state.monster, player);
    if current_distance > MONSTER_MAX_DISTANCE {
        state.monster = point_towards(player, state.monster, MONSTER_TARGET_DISTANCE);
    } else {
        let speed = if current_distance > MONSTER_TARGET_DISTANCE {
            config.monster.catchup_speed
        } else {
            config.monster.speed
        };
        state.monster = step_towards(state.monster, target, speed);
        let after = trace_distance(state.monster, player);
        if after > MONSTER_MAX_DISTANCE {
            state.monster = point_towards(player, state.monster, MONSTER_TARGET_DISTANCE);
        }
    }
    let yaw = yaw_to(state.monster, player);
    let _ = sdk.entity_move(
        &runner_entity_key(player_uuid),
        &state.session.dimension,
        NpcPosition {
            x: state.monster.x,
            y: state.monster.y,
            z: state.monster.z,
            yaw,
            pitch: 0.0,
        },
    );
}

fn trace_target(trace: &VecDeque<TracePoint>, player: TracePoint) -> Option<TracePoint> {
    trace
        .iter()
        .rev()
        .copied()
        .find(|point| trace_distance(*point, player) >= MONSTER_TARGET_DISTANCE)
        .or_else(|| trace.front().copied())
}

fn spawn_runner_entity(
    sdk: &qexed_plugin_sdk::Sdk,
    player_uuid: &str,
    monster: &MonsterConfig,
    session: &TempleSession,
) {
    let start = monster_start(session);
    let _ = sdk.entity_upsert(&RuntimeEntitySpec {
        key: runner_entity_key(player_uuid),
        dimension: session.dimension.clone(),
        entity_type: monster.entity_type.clone(),
        x: start.x,
        y: start.y,
        z: start.z,
        yaw: 0.0,
        pitch: 0.0,
        display_name: monster.display_name.clone(),
        ai: "none".to_string(),
        ai_params_json: "{}".to_string(),
        auto_jump: false,
    });
}

fn remove_runner_entity(sdk: &qexed_plugin_sdk::Sdk, player_uuid: &str) {
    let _ = sdk.entity_remove(&runner_entity_key(player_uuid));
}

fn runner_entity_key(player_uuid: &str) -> String {
    format!("runner/{}", player_uuid.replace('-', ""))
}

fn monster_start(session: &TempleSession) -> TracePoint {
    TracePoint {
        x: session.start_x,
        y: session.start_y,
        z: session.start_z - MONSTER_TARGET_DISTANCE,
    }
}

fn instance_origin(config: &Config, player_uuid: &str, difficulty: &DifficultyConfig) -> (i32, i32, i32) {
    let hash = stable_seed(player_uuid, &difficulty.id, 0);
    let lane = (hash % 4096) as i32;
    (
        difficulty.origin_x + lane * config.instance_spacing,
        difficulty.origin_y,
        difficulty.origin_z,
    )
}

fn build_progress(build: &BuildJob) -> f32 {
    if build.blocks.is_empty() {
        1.0
    } else {
        (build.written as f32 / build.blocks.len() as f32).clamp(0.01, 1.0)
    }
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

// ─────────────────────────── 工具函数 ───────────────────────────

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
        id: BUILD_BAR_ID.to_string(),
        title: title.to_string(),
        progress: progress.clamp(0.0, 1.0),
        color: color.to_string(),
        overlay: "progress".to_string(),
    }
}

fn session_key(uuid: &str) -> String {
    format!("{SESSION_PREFIX}/{uuid}")
}

fn best_key(uuid: &str, difficulty: &str) -> String {
    format!("{BEST_DISTANCE_PREFIX}/{difficulty}/{uuid}")
}

fn elapsed_ms(started_at_ms: i64) -> i64 {
    (now_millis() - started_at_ms).max(0)
}

fn format_duration(ms: i64) -> String {
    let seconds = (ms / 1000).max(0);
    let minutes = seconds / 60;
    format!("{minutes:02}:{:02}", seconds % 60)
}

fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn stable_seed(player_uuid: &str, difficulty: &str, salt: i64) -> u64 {
    let mut value = 0xcbf29ce484222325_u64 ^ (salt as u64);
    for byte in player_uuid.bytes().chain(difficulty.bytes()) {
        value ^= u64::from(byte);
        value = value.wrapping_mul(0x100000001b3);
    }
    value
}

fn lane_x(session: &TempleSession, lane: i32) -> f64 {
    session.origin_x as f64 + lane as f64 * LANE_SPACING + 0.5
}

fn lane_block_center(lane: i32) -> i32 {
    lane * LANE_SPACING as i32
}

fn nearest_lane(session: &TempleSession, x: f64) -> i32 {
    ((x - session.origin_x as f64 - 0.5) / LANE_SPACING).round() as i32
}

fn shift_lane_on_track(current_lane: i32, lane_delta: i32) -> i32 {
    (current_lane + lane_delta).clamp(MIN_LANE, MAX_LANE)
}

fn smooth_lane_delta(planned_x: &mut f64, target_x: f64, controls: &ControlConfig) -> f64 {
    let remaining = target_x - *planned_x;
    if remaining.abs() <= 0.03 {
        *planned_x = target_x;
        return 0.0;
    }
    let delta = (remaining * controls.lane_correction).clamp(-controls.max_lane_delta, controls.max_lane_delta);
    *planned_x += delta;
    delta
}

fn smooth_actual_delta(actual_x: f64, planned_x: f64, controls: &ControlConfig) -> f64 {
    let remaining = planned_x - actual_x;
    if remaining.abs() <= 0.03 {
        return 0.0;
    }
    remaining.clamp(-controls.max_lane_delta, controls.max_lane_delta)
}

fn action_speed_multiplier(session: &TempleSession, now: i64, controls: &ControlConfig) -> f64 {
    if now <= session.jump_until_ms {
        controls.jump_speed_multiplier
    } else if now <= session.slide_until_ms {
        controls.slide_speed_multiplier
    } else {
        1.0
    }
}

fn jump_delta(session: &TempleSession, y: f64, now: i64, lift_per_tick: f64) -> f64 {
    if now > session.jump_until_ms {
        return 0.0;
    }
    if y < session.start_y + 1.2 {
        lift_per_tick
    } else {
        0.0
    }
}

fn run_bar_progress(session: &TempleSession) -> f32 {
    let speed_range = (session.max_speed - session.initial_speed).max(0.001);
    (((session.current_speed - session.initial_speed) / speed_range).clamp(0.0, 1.0)) as f32
}

struct BlockSink<'a> {
    sdk: &'a qexed_plugin_sdk::Sdk<'a>,
    blocks: Vec<TempleBlock>,
}

impl<'a> BlockSink<'a> {
    fn new(sdk: &'a qexed_plugin_sdk::Sdk<'a>) -> Self {
        Self { sdk, blocks: Vec::with_capacity(WORLD_EDIT_BATCH_LIMIT) }
    }

    fn push(&mut self, dimension: &str, position: (i32, i32, i32), block: &str) {
        self.blocks.push(TempleBlock {
            dimension: dimension.to_string(),
            position,
            block: block.to_string(),
        });
        if self.blocks.len() >= WORLD_EDIT_BATCH_LIMIT {
            self.flush();
        }
    }

    fn flush(&mut self) {
        if self.blocks.is_empty() {
            return;
        }
        let refs: Vec<(&str, (i32, i32, i32), &str)> = self
            .blocks
            .iter()
            .map(|block| (block.dimension.as_str(), block.position, block.block.as_str()))
            .collect();
        let _ = self.sdk.world_set_blocks(&refs);
        self.blocks.clear();
    }
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

// ─────────────────────────── 数据结构 ───────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TempleSession {
    active: bool,
    difficulty: String,
    difficulty_label: String,
    dimension: String,
    origin_x: i32,
    origin_y: i32,
    origin_z: i32,
    start_x: f64,
    start_y: f64,
    start_z: f64,
    min_y: f64,
    distance: f64,
    coins: i64,
    target_lane: i32,
    seed: u64,
    built_until: i32,
    generated_until: i32,
    started_at_ms: i64,
    last_tick_at_ms: i64,
    jump_until_ms: i64,
    slide_until_ms: i64,
    last_jump_distance: f64,
    last_slide_distance: f64,
    slide_held: bool,
    last_coin_segment_z: i32,
    initial_speed: f64,
    max_speed: f64,
    speed_gain_per_meter: f64,
    current_speed: f64,
    segments: Vec<Segment>,
}

impl TempleSession {
    fn lobby(dimension: &str) -> Self {
        Self {
            active: false,
            difficulty: String::new(),
            difficulty_label: "大厅".to_string(),
            dimension: dimension.to_string(),
            origin_x: 0,
            origin_y: 0,
            origin_z: 0,
            start_x: 0.0,
            start_y: 0.0,
            start_z: 0.0,
            min_y: -128.0,
            distance: 0.0,
            coins: 0,
            target_lane: 0,
            seed: 0,
            built_until: 0,
            generated_until: 0,
            started_at_ms: 0,
            last_tick_at_ms: 0,
            jump_until_ms: 0,
            slide_until_ms: 0,
            last_jump_distance: ACTION_NEVER_DISTANCE,
            last_slide_distance: ACTION_NEVER_DISTANCE,
            slide_held: false,
            last_coin_segment_z: -1,
            initial_speed: 0.0,
            max_speed: 0.0,
            speed_gain_per_meter: 0.0,
            current_speed: 0.0,
            segments: Vec::new(),
        }
    }

    fn new(difficulty: &DifficultyConfig, origin: (i32, i32, i32), seed: u64, course: Course) -> Self {
        Self {
            active: true,
            difficulty: difficulty.id.clone(),
            difficulty_label: difficulty.label.clone(),
            dimension: difficulty.dimension.clone(),
            origin_x: origin.0,
            origin_y: origin.1,
            origin_z: origin.2,
            start_x: origin.0 as f64 + 0.5,
            start_y: origin.1 as f64,
            start_z: origin.2 as f64 + 0.5,
            min_y: origin.1 as f64 - 8.0,
            distance: 0.0,
            coins: 0,
            target_lane: 0,
            seed,
            built_until: course.length,
            generated_until: course.length,
            started_at_ms: now_millis(),
            last_tick_at_ms: now_millis(),
            jump_until_ms: 0,
            slide_until_ms: 0,
            last_jump_distance: ACTION_NEVER_DISTANCE,
            last_slide_distance: ACTION_NEVER_DISTANCE,
            slide_held: false,
            last_coin_segment_z: -1,
            initial_speed: difficulty.initial_speed,
            max_speed: difficulty.max_speed,
            speed_gain_per_meter: difficulty.speed_gain_per_meter,
            current_speed: difficulty.initial_speed,
            segments: course.segments,
        }
    }

    fn score(&self) -> i64 {
        self.distance.floor() as i64 + self.coins * 10
    }

    fn speed_blocks_per_second(&self) -> f64 {
        self.current_speed * 20.0
    }

    fn speed_per_tick(&mut self, tick_millis: u64, max_tick_scale: f64) -> f64 {
        self.current_speed =
            (self.initial_speed + self.distance * self.speed_gain_per_meter).min(self.max_speed);
        let tick_scale = (tick_millis as f64 / 50.0).clamp(0.5, max_tick_scale.max(0.5));
        self.current_speed * tick_scale
    }
}

#[derive(Debug, Clone)]
struct Course {
    length: i32,
    segments: Vec<Segment>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
struct Segment {
    start_z: i32,
    length: i32,
    kind: SegmentKind,
    safe_lane: i32,
    blocked_lane: i32,
}

impl Segment {
    fn end_z(self) -> i32 {
        self.start_z + self.length
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
enum SegmentKind {
    Straight,
    CoinLine,
    Gap,
    LowBarrier,
    LaneBarrier,
    DoubleBarrier,
    TurnLeft,
    TurnRight,
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
    controls: ControlConfig,
    #[serde(default)]
    lobby: LobbyConfig,
    #[serde(default)]
    monster: MonsterConfig,
    #[serde(default = "default_difficulties")]
    difficulties: Vec<DifficultyConfig>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enable: true,
            menu_id: default_menu_id(),
            instance_spacing: default_instance_spacing(),
            controls: ControlConfig::default(),
            lobby: LobbyConfig::default(),
            monster: MonsterConfig::default(),
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
struct ControlConfig {
    #[serde(default = "default_jump_ms")]
    jump_ms: i64,
    #[serde(default = "default_slide_ms")]
    slide_ms: i64,
    #[serde(default = "default_lane_correction")]
    lane_correction: f64,
    #[serde(default = "default_max_lane_delta")]
    max_lane_delta: f64,
    #[serde(default = "default_jump_lift_per_tick")]
    jump_lift_per_tick: f64,
    #[serde(default = "default_jump_speed_multiplier")]
    jump_speed_multiplier: f64,
    #[serde(default = "default_slide_speed_multiplier")]
    slide_speed_multiplier: f64,
    #[serde(default = "default_max_tick_scale")]
    max_tick_scale: f64,
    #[serde(default = "default_action_buffer_blocks")]
    action_buffer_blocks: f64,
    #[serde(default = "default_action_late_blocks")]
    action_late_blocks: f64,
    #[serde(default = "default_generate_ahead_blocks")]
    generate_ahead_blocks: i32,
}

impl Default for ControlConfig {
    fn default() -> Self {
        Self {
            jump_ms: default_jump_ms(),
            slide_ms: default_slide_ms(),
            lane_correction: default_lane_correction(),
            max_lane_delta: default_max_lane_delta(),
            jump_lift_per_tick: default_jump_lift_per_tick(),
            jump_speed_multiplier: default_jump_speed_multiplier(),
            slide_speed_multiplier: default_slide_speed_multiplier(),
            max_tick_scale: default_max_tick_scale(),
            action_buffer_blocks: default_action_buffer_blocks(),
            action_late_blocks: default_action_late_blocks(),
            generate_ahead_blocks: default_generate_ahead_blocks(),
        }
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
            spawn_x: 0.5,
            spawn_y: default_lobby_y(),
            spawn_z: 0.5,
            spawn_yaw: 0.0,
            spawn_pitch: 0.0,
            floor_y: default_lobby_floor_y(),
            npc_x: default_npc_x(),
            npc_y: default_lobby_y(),
            npc_z: default_npc_z(),
            npc_yaw: default_npc_yaw(),
            npc_pitch: 0.0,
            npc_entity_type: default_npc_entity_type(),
            npc_display_name: default_npc_display_name(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct MonsterConfig {
    #[serde(default = "default_monster_entity_type")]
    entity_type: String,
    #[serde(default = "default_monster_display_name")]
    display_name: String,
    #[serde(default = "default_monster_speed")]
    speed: f64,
    #[serde(default = "default_monster_catchup_speed")]
    catchup_speed: f64,
}

impl Default for MonsterConfig {
    fn default() -> Self {
        Self {
            entity_type: default_monster_entity_type(),
            display_name: default_monster_display_name(),
            speed: default_monster_speed(),
            catchup_speed: default_monster_catchup_speed(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct DifficultyConfig {
    id: String,
    label: String,
    dimension: String,
    #[serde(default = "default_prebuild_length")]
    length: i32,
    #[serde(default = "default_extend_length")]
    extend_length: i32,
    #[serde(default = "default_region_length")]
    region_length: i32,
    #[serde(default = "default_path_width")]
    path_width: i32,
    origin_x: i32,
    origin_y: i32,
    origin_z: i32,
    #[serde(default = "default_initial_speed")]
    initial_speed: f64,
    #[serde(default = "default_max_speed")]
    max_speed: f64,
    #[serde(default = "default_speed_gain_per_meter")]
    speed_gain_per_meter: f64,
    #[serde(default = "default_obstacle_start_distance")]
    obstacle_start_distance: i32,
    #[serde(default = "default_segment_min_length")]
    segment_min_length: i32,
    #[serde(default = "default_segment_max_length")]
    segment_max_length: i32,
    #[serde(default = "default_gap_weight")]
    gap_weight: u32,
    #[serde(default = "default_low_barrier_weight")]
    low_barrier_weight: u32,
    #[serde(default = "default_turn_weight")]
    turn_weight: u32,
    #[serde(default = "default_double_barrier_weight")]
    double_barrier_weight: u32,
    #[serde(default = "default_coin_weight")]
    coin_weight: u32,
}

impl DifficultyConfig {
    fn prebuild_length(&self) -> i32 {
        self.length.max(96)
    }

    fn path_half_width(&self) -> i32 {
        (self.path_width.max(7) / 2).max(3)
    }
}

// ─────────────────────────── 配置默认值 ───────────────────────────

fn default_true() -> bool {
    true
}

fn default_menu_id() -> String {
    MENU_ID.to_string()
}

fn default_instance_spacing() -> i32 {
    512
}

fn default_jump_ms() -> i64 {
    900
}

fn default_slide_ms() -> i64 {
    900
}

fn default_lane_correction() -> f64 {
    0.38
}

fn default_max_lane_delta() -> f64 {
    0.36
}

fn default_jump_lift_per_tick() -> f64 {
    0.18
}

fn default_jump_speed_multiplier() -> f64 {
    0.65
}

fn default_slide_speed_multiplier() -> f64 {
    0.65
}

fn default_max_tick_scale() -> f64 {
    1.5
}

fn default_action_buffer_blocks() -> f64 {
    14.0
}

fn default_action_late_blocks() -> f64 {
    3.0
}

fn default_generate_ahead_blocks() -> i32 {
    160
}

fn default_lobby_dimension() -> String {
    "qexed:temple_lobby".to_string()
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

fn default_npc_yaw() -> f32 {
    180.0
}

fn default_npc_entity_type() -> String {
    "minecraft:villager".to_string()
}

fn default_npc_display_name() -> String {
    "{\"text\":\"神庙入口\",\"color\":\"gold\"}".to_string()
}

fn default_monster_entity_type() -> String {
    "minecraft:zombie".to_string()
}

fn default_monster_display_name() -> String {
    "{\"text\":\"神庙追击者\",\"color\":\"dark_red\"}".to_string()
}

fn default_monster_speed() -> f64 {
    0.24
}

fn default_monster_catchup_speed() -> f64 {
    0.9
}

fn default_prebuild_length() -> i32 {
    360
}

fn default_extend_length() -> i32 {
    160
}

fn default_region_length() -> i32 {
    20000
}

fn default_path_width() -> i32 {
    9
}

fn default_initial_speed() -> f64 {
    0.22
}

fn default_max_speed() -> f64 {
    0.44
}

fn default_speed_gain_per_meter() -> f64 {
    0.00022
}

fn default_obstacle_start_distance() -> i32 {
    42
}

fn default_segment_min_length() -> i32 {
    10
}

fn default_segment_max_length() -> i32 {
    18
}

fn default_gap_weight() -> u32 {
    12
}

fn default_low_barrier_weight() -> u32 {
    12
}

fn default_turn_weight() -> u32 {
    10
}

fn default_double_barrier_weight() -> u32 {
    12
}

fn default_coin_weight() -> u32 {
    20
}

fn difficulty(
    id: &str,
    label: &str,
    dimension: &str,
    length: i32,
    origin_x: i32,
    initial_speed: f64,
    max_speed: f64,
    speed_gain_per_meter: f64,
    obstacle_start_distance: i32,
    segment_min_length: i32,
    segment_max_length: i32,
    gap_weight: u32,
    low_barrier_weight: u32,
    turn_weight: u32,
    double_barrier_weight: u32,
    coin_weight: u32,
) -> DifficultyConfig {
    DifficultyConfig {
        id: id.to_string(),
        label: label.to_string(),
        dimension: dimension.to_string(),
        length,
        extend_length: 180,
        region_length: default_region_length(),
        path_width: 9,
        origin_x,
        origin_y: -52,
        origin_z: 2048,
        initial_speed,
        max_speed,
        speed_gain_per_meter,
        obstacle_start_distance,
        segment_min_length,
        segment_max_length,
        gap_weight,
        low_barrier_weight,
        turn_weight,
        double_barrier_weight,
        coin_weight,
    }
}

fn default_difficulties() -> Vec<DifficultyConfig> {
    vec![
        difficulty("easy", "简单", "qexed:temple_easy", 360, 2048, 0.20, 0.38, 0.00012, 58, 12, 20, 8, 10, 8, 8, 26),
        difficulty("normal", "普通", "qexed:temple_normal", 420, 2048, 0.23, 0.44, 0.00018, 44, 10, 18, 12, 13, 12, 14, 18),
        difficulty("hard", "困难", "qexed:temple_hard", 480, 2048, 0.26, 0.50, 0.00024, 36, 9, 16, 16, 15, 14, 18, 14),
    ]
}

const DEFAULT_CONFIG: &str = r#"enable = true
menu_id = "temple_run"
instance_spacing = 512

[controls]
jump_ms = 900
slide_ms = 900
lane_correction = 0.38
max_lane_delta = 0.36
jump_lift_per_tick = 0.18
jump_speed_multiplier = 0.65
slide_speed_multiplier = 0.65
max_tick_scale = 1.5
action_buffer_blocks = 14.0
action_late_blocks = 3.0
generate_ahead_blocks = 160

[lobby]
dimension = "qexed:temple_lobby"
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
npc_display_name = "{\"text\":\"神庙入口\",\"color\":\"gold\"}"

[monster]
entity_type = "minecraft:zombie"
display_name = "{\"text\":\"神庙追击者\",\"color\":\"dark_red\"}"
speed = 0.24
catchup_speed = 0.9

[[difficulties]]
id = "easy"
label = "简单"
dimension = "qexed:temple_easy"
length = 360
extend_length = 180
region_length = 20000
path_width = 9
origin_x = 2048
origin_y = -52
origin_z = 2048
initial_speed = 0.20
max_speed = 0.38
speed_gain_per_meter = 0.00012
obstacle_start_distance = 58
segment_min_length = 12
segment_max_length = 20
gap_weight = 8
low_barrier_weight = 10
turn_weight = 8
double_barrier_weight = 8
coin_weight = 26

[[difficulties]]
id = "normal"
label = "普通"
dimension = "qexed:temple_normal"
length = 420
extend_length = 180
region_length = 20000
path_width = 9
origin_x = 2048
origin_y = -52
origin_z = 2048
initial_speed = 0.23
max_speed = 0.44
speed_gain_per_meter = 0.00018
obstacle_start_distance = 44
segment_min_length = 10
segment_max_length = 18
gap_weight = 12
low_barrier_weight = 13
turn_weight = 12
double_barrier_weight = 14
coin_weight = 18

[[difficulties]]
id = "hard"
label = "困难"
dimension = "qexed:temple_hard"
length = 480
extend_length = 220
region_length = 20000
path_width = 9
origin_x = 2048
origin_y = -52
origin_z = 2048
initial_speed = 0.26
max_speed = 0.50
speed_gain_per_meter = 0.00024
obstacle_start_distance = 36
segment_min_length = 9
segment_max_length = 16
gap_weight = 16
low_barrier_weight = 15
turn_weight = 14
double_barrier_weight = 18
coin_weight = 14
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_segments_always_have_a_survival_answer() {
        for difficulty in default_difficulties() {
            for seed in 1..=64u64 {
                let course = generate_course(&difficulty, seed, difficulty.prebuild_length());
                assert!(course.length >= difficulty.prebuild_length());
                for segment in &course.segments {
                    assert!(segment.length > 0);
                    assert!((-1..=1).contains(&segment.safe_lane));
                    assert!((-1..=1).contains(&segment.blocked_lane));
                    if segment.kind == SegmentKind::DoubleBarrier {
                        assert!(
                            (MIN_LANE..=MAX_LANE).any(|lane| lane != segment.blocked_lane),
                            "double barrier must leave a safe lane"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn obstacle_rules_match_temple_run_actions() {
        let difficulty = default_difficulties().remove(0);
        let mut session = TempleSession::new(
            &difficulty,
            (0, 64, 0),
            1,
            Course {
                length: 100,
                segments: vec![
                    Segment { start_z: 0, length: 20, kind: SegmentKind::Gap, safe_lane: 0, blocked_lane: 0 },
                    Segment { start_z: 20, length: 20, kind: SegmentKind::LowBarrier, safe_lane: 0, blocked_lane: 0 },
                ],
            },
        );
        assert_eq!(
            obstacle_failure(&session, 0.5, 64.0, 10.5, 1000, default_action_buffer_blocks(), default_action_late_blocks()),
            Some("没有跳过断路")
        );
        session.last_jump_distance = 2.0;
        assert_eq!(
            obstacle_failure(&session, 0.5, 64.0, 10.5, 1000, default_action_buffer_blocks(), default_action_late_blocks()),
            None
        );
        assert_eq!(
            obstacle_failure(&session, 0.5, 64.0, 30.5, 3000, default_action_buffer_blocks(), default_action_late_blocks()),
            Some("没有滑过低障碍")
        );
        session.last_slide_distance = 22.0;
        assert_eq!(
            obstacle_failure(&session, 0.5, 64.0, 30.5, 3000, default_action_buffer_blocks(), default_action_late_blocks()),
            None
        );
    }

    #[test]
    fn speed_increases_with_distance() {
        let difficulty = default_difficulties().remove(1);
        let mut session = TempleSession::new(&difficulty, (0, 64, 0), 1, generate_course(&difficulty, 1, 120));
        let start = session.speed_per_tick(50, default_max_tick_scale());
        session.distance = 500.0;
        let later = session.speed_per_tick(50, default_max_tick_scale());
        assert!(later > start);
        assert!(later <= difficulty.max_speed * default_max_tick_scale());
    }

    #[test]
    fn lane_input_uses_track_coordinates() {
        assert_eq!(shift_lane_on_track(0, 1), 1);
        assert_eq!(shift_lane_on_track(0, -1), -1);
        assert_eq!(shift_lane_on_track(1, 1), 1);
        assert_eq!(shift_lane_on_track(-1, -1), -1);
    }

    #[test]
    fn lane_smoothing_moves_monotonically_without_overshoot() {
        let controls = ControlConfig::default();
        let mut planned_x = 0.5;
        let target_x = 3.5;
        let mut previous_delta = 0.0;
        for _ in 0..20 {
            let delta = smooth_lane_delta(&mut planned_x, target_x, &controls);
            assert!(delta >= 0.0);
            assert!(delta <= controls.max_lane_delta + f64::EPSILON);
            assert!(planned_x <= target_x + f64::EPSILON);
            previous_delta = delta;
        }
        assert!(planned_x > 3.0);
        assert!(previous_delta < controls.max_lane_delta);
    }

    #[test]
    fn actual_position_is_limited_towards_planned_lane() {
        let controls = ControlConfig::default();
        let delta = smooth_actual_delta(0.5, 3.5, &controls);
        assert_eq!(delta, controls.max_lane_delta);
        let small_delta = smooth_actual_delta(3.48, 3.5, &controls);
        assert_eq!(small_delta, 0.0);
    }

    #[test]
    fn target_lane_does_not_hide_actual_wall_collision() {
        let difficulty = default_difficulties().remove(0);
        let mut session = TempleSession::new(
            &difficulty,
            (0, 64, 0),
            1,
            Course {
                length: 40,
                segments: vec![Segment { start_z: 0, length: 20, kind: SegmentKind::LaneBarrier, safe_lane: 0, blocked_lane: 0 }],
            },
        );
        session.target_lane = 1;
        assert_eq!(
            obstacle_failure(&session, 0.5, 64.0, 10.5, 1000, default_action_buffer_blocks(), default_action_late_blocks()),
            Some("撞上石墙")
        );
    }

    #[test]
    fn lane_barrier_uses_collision_width_not_rounding_only() {
        let difficulty = default_difficulties().remove(0);
        let session = TempleSession::new(
            &difficulty,
            (0, 64, 0),
            1,
            Course {
                length: 40,
                segments: vec![Segment { start_z: 0, length: 20, kind: SegmentKind::LaneBarrier, safe_lane: 1, blocked_lane: 0 }],
            },
        );
        assert_eq!(
            obstacle_failure(&session, 2.0, 64.0, 10.5, 3000, default_action_buffer_blocks(), default_action_late_blocks()),
            None
        );
    }

    #[test]
    fn turn_hints_are_not_fatal_without_real_turning_path() {
        let difficulty = default_difficulties().remove(0);
        let session = TempleSession::new(
            &difficulty,
            (0, 64, 0),
            1,
            Course {
                length: 40,
                segments: vec![Segment { start_z: 0, length: 20, kind: SegmentKind::TurnLeft, safe_lane: 0, blocked_lane: 0 }],
            },
        );
        assert_eq!(
            obstacle_failure(&session, 0.5, 64.0, 10.5, 3000, default_action_buffer_blocks(), default_action_late_blocks()),
            None
        );
    }

    #[test]
    fn monster_distance_is_clamped_to_ten_blocks() {
        let config = Config::default();
        let difficulty = config.difficulty("easy").unwrap().clone();
        let session = TempleSession::new(&difficulty, (0, 64, 0), 42, generate_course(&difficulty, 42, 120));
        let mut state = RuntimeSession {
            session,
            trace: VecDeque::new(),
            monster: TracePoint { x: 0.0, y: 64.0, z: -100.0 },
            last_trace: TracePoint { x: 0.0, y: 64.0, z: 0.0 },
            planned_x: 0.0,
            last_bossbar_bucket: -1,
            spawned: true,
            pending_build: None,
        };
        state.trace.push_back(TracePoint { x: 0.0, y: 64.0, z: 0.0 });
        let player = TracePoint { x: 0.0, y: 64.0, z: 30.0 };
        move_monster_for_test(&config, &mut state, player);
        assert!(trace_distance(state.monster, player) <= MONSTER_MAX_DISTANCE + 0.01);
    }

    /// 测试专用：与 move_monster 相同的追踪逻辑，但不触碰 SDK。
    fn move_monster_for_test(config: &Config, state: &mut RuntimeSession, player: TracePoint) {
        let target = trace_target(&state.trace, player).unwrap_or(player);
        let current_distance = trace_distance(state.monster, player);
        if current_distance > MONSTER_MAX_DISTANCE {
            state.monster = point_towards(player, state.monster, MONSTER_TARGET_DISTANCE);
        } else {
            let speed = if current_distance > MONSTER_TARGET_DISTANCE {
                config.monster.catchup_speed
            } else {
                config.monster.speed
            };
            state.monster = step_towards(state.monster, target, speed);
            let after = trace_distance(state.monster, player);
            if after > MONSTER_MAX_DISTANCE {
                state.monster = point_towards(player, state.monster, MONSTER_TARGET_DISTANCE);
            }
        }
    }
}

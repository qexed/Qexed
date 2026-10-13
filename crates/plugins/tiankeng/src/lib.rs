//! tiankeng：v4 移植 —— 天坑塌缩玩法。
//!
//! v4：commands / npc_mutations / npc_interact / player_tick / config_reload /
//!     join/leave 导出。v6：ChatCommandEvent（cancel 接管）、NpcMutationsEvent（Mutex 回写）、
//!     NpcInteractEvent、PlayerTickEvent（休眠就绪）、ConfigReloadEvent、join/leave。
//! WorldEdit 区块注册 + world_set_blocks 批量建场；实例网格 fnv1a64 定位。

use qexed_plugin_api::payload::{NpcMutationOp, NpcUpsert, PlayerAction as Pa};
use qexed_plugin_api::services::WorldEditRegionSpec;
use qexed_plugin_api::{
    ChatCommandEvent, ConfigReloadEvent, EventContext, LogLevel, NpcInteractEvent,
    NpcMutationsEvent, PlayerJoinEvent, PlayerLeaveEvent, PlayerTickEvent,
};
use qexed_plugin_sdk::{InitFuture, PluginContext};
use serde::{Deserialize, Serialize};

const COMMAND_NAME: &str = "tiankeng";
const NPC_KEY: &str = "tiankeng_guide";
const NPC_EVENT: &str = "tiankeng";
const SESSION_PREFIX: &str = "tiankeng/session";
const INSTANCE_GRID_WIDTH: u64 = 4096;

qexed_plugin_sdk::declare_simple_plugin!("tiankeng", "Tiankeng", "0.1.0", |ctx: PluginContext| -> InitFuture {
    Box::pin(async move {
        let plugin_id: &'static str = ctx.id();
        let sdk = ctx.sdk();
        let _ = sdk.config_load_or_create(DEFAULT_CONFIG);
        let config = load_config(&sdk);
        register_lobby(&sdk, &config);
        build_lobby(&sdk, &config);
        ctx.register_command(COMMAND_NAME);
        ctx.log(LogLevel::Info, "tiankeng initialized");

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
                let actions = if !config.enable {
                    vec![message("天坑玩法已关闭")]
                } else {
                    match ev.argument.split_whitespace().next().unwrap_or_default() {
                        "" | "menu" | "select" => vec![Pa::OpenMenu { menu: config.menu_id.clone() }],
                        "start" | "new" | "restart" => start_game(&sdk, &config, &player.uuid),
                        "status" | "info" => status_response(&sdk, &config, &player.uuid),
                        "lobby" | "leave" | "spawn" => return_to_lobby(&config, &player.uuid),
                        _ => vec![message("用法: /tiankeng menu | /tiankeng start | /tiankeng status | /tiankeng lobby")],
                    }
                };
                sdk.perform_all(&actions.iter().map(|a| (player, a.clone())).collect::<Vec<_>>());
            })
        });

        ctx.register::<ConfigReloadEvent>(move |api, ev, _| {
            let plugin_id = plugin_id;
            Box::pin(async move {
                let path = ev.payload.path.replace('\\', "/");
                if !(path.ends_with("tiankeng/config.toml") || path.ends_with("config.toml") && path.contains("tiankeng")) {
                    return;
                }
                let sdk = qexed_plugin_sdk::Sdk::new(api, plugin_id);
                let config = load_config(&sdk);
                register_lobby(&sdk, &config);
                build_lobby(&sdk, &config);
                api.log(LogLevel::Info, "tiankeng config reloaded");
            })
        });

        ctx.register::<PlayerJoinEvent>(move |api, ev, _| {
            let plugin_id = plugin_id;
            Box::pin(async move {
                let sdk = qexed_plugin_sdk::Sdk::new(api, plugin_id);
                let _ = sdk.storage_delete(&session_key(&ev.player.uuid));
            })
        });

        ctx.register::<PlayerLeaveEvent>(move |api, ev, _| {
            let plugin_id = plugin_id;
            Box::pin(async move {
                let sdk = qexed_plugin_sdk::Sdk::new(api, plugin_id);
                let _ = sdk.storage_delete(&session_key(&ev.player.uuid));
            })
        });

        // NPC 群体维护（宿主接线后生效）
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
                        name: "天坑".to_string(),
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

        // 对局推进（宿主 player_tick 接线后生效）
        ctx.register::<PlayerTickEvent>(move |api, ev, _| {
            let plugin_id = plugin_id;
            Box::pin(async move {
                let sdk = qexed_plugin_sdk::Sdk::new(api, plugin_id);
                let config = load_config(&sdk);
                if !config.enable || ev.dimension != "qexed:tiankeng_game" {
                    return;
                }
                let Some(mut session): Option<TiankengSession> = sdk.storage_get_json(&session_key(&ev.player.uuid)) else {
                    return;
                };
                if ev.dimension != session.dimension {
                    return;
                }
                let now = now_millis();
                let player = &ev.player;
                if ev.position.y < f64::from(session.origin_y - config.arena.fall_depth.max(2)) {
                    let _ = sdk.storage_delete(&session_key(&player.uuid));
                    sdk.perform_all(&finish_actions(&config, false, elapsed_seconds(now, session.started_at_ms)).iter().map(|a| (player, a.clone())).collect::<Vec<_>>());
                    return;
                }
                if session.current_radius <= config.arena.min_radius.max(1)
                    && now >= session.next_collapse_at_ms.saturating_add(config.arena.win_hold_ms)
                {
                    let _ = sdk.storage_delete(&session_key(&player.uuid));
                    sdk.perform_all(&finish_actions(&config, true, elapsed_seconds(now, session.started_at_ms)).iter().map(|a| (player, a.clone())).collect::<Vec<_>>());
                    return;
                }
                if now < session.next_collapse_at_ms || session.current_radius <= config.arena.min_radius.max(1) {
                    return;
                }
                collapse_outer_ring(&sdk, &config, &mut session);
                session.next_collapse_at_ms = now.saturating_add(config.arena.collapse_interval_ms.max(500));
                let current_radius = session.current_radius;
                let _ = sdk.storage_set_json(&session_key(&player.uuid), &session);
                sdk.perform(player, Pa::SystemMessage {
                    text: format!("天坑继续塌陷，剩余半径 {current_radius} 格"),
                    translate: String::new(),
                    with: Vec::new(),
                    overlay: true,
                });
            })
        });
    })
});

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TiankengSession {
    dimension: String,
    origin_x: i32,
    origin_y: i32,
    origin_z: i32,
    current_radius: i32,
    started_at_ms: i64,
    next_collapse_at_ms: i64,
}

fn start_game(sdk: &qexed_plugin_sdk::Sdk, config: &Config, player_uuid: &str) -> Vec<Pa> {
    let origin = instance_origin(config, player_uuid);
    register_arena(sdk, config, player_uuid, origin);
    if !build_arena(sdk, config, origin) {
        return vec![message("天坑场地生成失败，请稍后重试")];
    }
    let now = now_millis();
    let session = TiankengSession {
        dimension: config.arena.dimension.clone(),
        origin_x: origin.0,
        origin_y: origin.1,
        origin_z: origin.2,
        current_radius: config.arena.radius.max(config.arena.min_radius.max(1)),
        started_at_ms: now,
        next_collapse_at_ms: now
            .saturating_add(config.arena.start_delay_ms)
            .saturating_add(config.arena.collapse_interval_ms.max(500)),
    };
    let _ = sdk.storage_set_json(&session_key(player_uuid), &session);
    vec![
        message(format!("{} 天坑开始，QQ群 {}", config.server_name, config.qq_group)),
        Pa::SetPlayersVisible { visible: false },
        Pa::Teleport {
            dimension: config.arena.dimension.clone(),
            x: f64::from(origin.0) + 0.5,
            y: f64::from(origin.1),
            z: f64::from(origin.2) + 0.5,
            yaw: Some(0.0),
            pitch: Some(0.0),
        },
    ]
}

fn status_response(sdk: &qexed_plugin_sdk::Sdk, config: &Config, player_uuid: &str) -> Vec<Pa> {
    let Some(session): Option<TiankengSession> = sdk.storage_get_json(&session_key(player_uuid)) else {
        return vec![message("当前没有进行中的天坑对局")];
    };
    let next = session.next_collapse_at_ms.saturating_sub(now_millis()) / 1000;
    vec![message(format!(
        "天坑: 剩余半径 {}，下次塌陷约 {} 秒后，目标半径 {}",
        session.current_radius,
        next,
        config.arena.min_radius.max(1)
    ))]
}

fn return_to_lobby(config: &Config, player_uuid: &str) -> Vec<Pa> {
    let _ = player_uuid;
    let mut actions = vec![
        Pa::SetPlayersVisible { visible: true },
        Pa::Teleport {
            dimension: config.lobby.dimension.clone(),
            x: config.lobby.spawn_x,
            y: config.lobby.spawn_y,
            z: config.lobby.spawn_z,
            yaw: Some(config.lobby.spawn_yaw),
            pitch: Some(config.lobby.spawn_pitch),
        },
        message("已返回天坑大厅"),
    ];
    if config.use_proxy_lobby {
        actions.push(Pa::ProxyConnect { server: config.lobby_server.clone(), message: String::new() });
    }
    actions
}

fn finish_actions(config: &Config, won: bool, seconds: u64) -> Vec<Pa> {
    let text = if won {
        format!("挑战成功，用时 {seconds} 秒")
    } else {
        format!("挑战失败，坚持了 {seconds} 秒")
    };
    vec![
        Pa::SetPlayersVisible { visible: true },
        Pa::Teleport {
            dimension: config.lobby.dimension.clone(),
            x: config.lobby.spawn_x,
            y: config.lobby.spawn_y,
            z: config.lobby.spawn_z,
            yaw: Some(config.lobby.spawn_yaw),
            pitch: Some(config.lobby.spawn_pitch),
        },
        message(text),
        Pa::OpenMenu { menu: config.menu_id.clone() },
    ]
}

fn register_lobby(sdk: &qexed_plugin_sdk::Sdk, config: &Config) {
    let y = config.lobby.floor_y;
    let _ = sdk.world_register_edit_region(&WorldEditRegionSpec {
        id: "tiankeng_lobby".to_string(),
        dimension: config.lobby.dimension.clone(),
        min: (-12, y - 2, -12),
        max: (12, y + 6, 12),
        allow_player_break: false,
        allow_player_place: false,
        allow_plugin_write: true,
        runtime_only: true,
    });
}

fn build_lobby(sdk: &qexed_plugin_sdk::Sdk, config: &Config) {
    let dim = config.lobby.dimension.as_str();
    let y = config.lobby.floor_y;
    let mut blocks = Vec::new();
    for x in -6..=6 {
        for z in -6..=6 {
            blocks.push((dim, (x, y - 1, z), "minecraft:smooth_stone"));
            for air_y in y..=y + 4 {
                blocks.push((dim, (x, air_y, z), "minecraft:air"));
            }
        }
    }
    for x in -3..=3 {
        blocks.push((dim, (x, y - 1, 3), "minecraft:sea_lantern"));
    }
    let _ = sdk.world_set_blocks(&blocks);
}

fn register_arena(sdk: &qexed_plugin_sdk::Sdk, config: &Config, player_uuid: &str, origin: (i32, i32, i32)) {
    let radius = config.arena.radius.max(4);
    let id = format!("tiankeng_{}", compact_uuid(player_uuid));
    let _ = sdk.world_register_edit_region(&WorldEditRegionSpec {
        id,
        dimension: config.arena.dimension.clone(),
        min: (origin.0 - radius - 5, origin.1 - 10, origin.2 - radius - 5),
        max: (origin.0 + radius + 5, origin.1 + 8, origin.2 + radius + 5),
        allow_player_break: false,
        allow_player_place: false,
        allow_plugin_write: true,
        runtime_only: true,
    });
}

fn build_arena(sdk: &qexed_plugin_sdk::Sdk, config: &Config, origin: (i32, i32, i32)) -> bool {
    let radius = config.arena.radius.max(4);
    let dim = config.arena.dimension.as_str();
    let floor_y = origin.1 - 1;
    let mut blocks = Vec::new();
    for x in -radius - 4..=radius + 4 {
        for z in -radius - 4..=radius + 4 {
            for y in floor_y - 3..=floor_y + 8 {
                blocks.push((dim, (origin.0 + x, y, origin.2 + z), "minecraft:air"));
            }
        }
    }
    let radius_sq = radius * radius;
    let border_sq = (radius + 1) * (radius + 1);
    for x in -radius - 1..=radius + 1 {
        for z in -radius - 1..=radius + 1 {
            let distance_sq = x * x + z * z;
            if distance_sq <= radius_sq {
                let block = if x == 0 && z == 0 {
                    config.arena.center_block.as_str()
                } else if distance_sq > (radius - 2).max(1) * (radius - 2).max(1) {
                    config.arena.warning_block.as_str()
                } else {
                    config.arena.floor_block.as_str()
                };
                blocks.push((dim, (origin.0 + x, floor_y, origin.2 + z), block));
                blocks.push((dim, (origin.0 + x, floor_y - 1, origin.2 + z), config.arena.support_block.as_str()));
            } else if distance_sq <= border_sq {
                blocks.push((dim, (origin.0 + x, floor_y, origin.2 + z), config.arena.border_block.as_str()));
            }
        }
    }
    sdk.world_set_blocks(&blocks) > 0
}

fn collapse_outer_ring(sdk: &qexed_plugin_sdk::Sdk, config: &Config, session: &mut TiankengSession) {
    let radius = session.current_radius;
    let next_radius = (radius - 1).max(config.arena.min_radius.max(1));
    let floor_y = session.origin_y - 1;
    let dim = config.arena.dimension.as_str();
    let mut blocks = Vec::new();
    let radius_sq = radius * radius;
    let next_sq = next_radius * next_radius;
    for x in -radius..=radius {
        for z in -radius..=radius {
            let distance_sq = x * x + z * z;
            if distance_sq <= radius_sq && distance_sq > next_sq {
                blocks.push((dim, (session.origin_x + x, floor_y, session.origin_z + z), "minecraft:air"));
                blocks.push((dim, (session.origin_x + x, floor_y - 1, session.origin_z + z), "minecraft:air"));
            }
        }
    }
    let _ = sdk.world_set_blocks(&blocks);
    session.current_radius = next_radius;
}

fn session_key(player_uuid: &str) -> String {
    format!("{SESSION_PREFIX}/{player_uuid}")
}

fn message(text: impl Into<String>) -> Pa {
    Pa::SystemMessage { text: text.into(), translate: String::new(), with: Vec::new(), overlay: false }
}

fn elapsed_seconds(now: i64, started_at_ms: i64) -> u64 {
    u64::try_from(now.saturating_sub(started_at_ms).max(0) / 1000).unwrap_or(0)
}

fn instance_origin(config: &Config, player_uuid: &str) -> (i32, i32, i32) {
    let hash = fnv1a64(player_uuid.as_bytes());
    let spacing = i64::from(config.arena.instance_spacing.max(config.arena.radius * 3 + 16));
    let x_index = (hash % INSTANCE_GRID_WIDTH) as i64;
    let z_index = ((hash / INSTANCE_GRID_WIDTH) % INSTANCE_GRID_WIDTH) as i64;
    (
        clamp_i64_to_i32(i64::from(config.arena.origin_x) + x_index * spacing),
        config.arena.origin_y,
        clamp_i64_to_i32(i64::from(config.arena.origin_z) + z_index * spacing),
    )
}

fn compact_uuid(value: &str) -> String {
    value.chars().filter(|ch| ch.is_ascii_hexdigit()).take(16).collect()
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

fn clamp_i64_to_i32(value: i64) -> i32 {
    value.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn load_config(sdk: &qexed_plugin_sdk::Sdk) -> Config {
    sdk.read_config()
        .ok()
        .flatten()
        .and_then(|content| {
            content
                .parse::<toml_edit::DocumentMut>()
                .ok()
                .and_then(|doc| qexed_toml::from_document::<Config>(doc).ok())
        })
        .unwrap_or_else(default_config)
}

fn default_config() -> Config {
    let json = serde_json::to_string(&ConfigSeed::default()).expect("serialize seed");
    serde_json::from_str(&json).expect("default tiankeng config is valid")
}

/// 全 Default 种子（字段序与 Config 一致，驱动 serde 默认值）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct ConfigSeed {
    #[serde(default = "d_enable")]
    enable: bool,
    #[serde(default = "d_server_name")]
    server_name: String,
    #[serde(default = "d_qq_group")]
    qq_group: String,
    #[serde(default = "d_lobby_server")]
    lobby_server: String,
    #[serde(default = "d_false")]
    use_proxy_lobby: bool,
    #[serde(default = "d_menu_id")]
    menu_id: String,
    #[serde(default)]
    lobby: LobbyConfig,
    #[serde(default)]
    arena: ArenaConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Config {
    enable: bool,
    server_name: String,
    qq_group: String,
    lobby_server: String,
    use_proxy_lobby: bool,
    menu_id: String,
    lobby: LobbyConfig,
    arena: ArenaConfig,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct LobbyConfig {
    #[serde(default = "d_dimension")]
    dimension: String,
    #[serde(default = "d_lobby_floor_y")]
    floor_y: i32,
    #[serde(default = "d_spawn_x")]
    spawn_x: f64,
    #[serde(default = "d_spawn_y")]
    spawn_y: f64,
    #[serde(default = "d_spawn_z")]
    spawn_z: f64,
    #[serde(default = "d_zero_f32")]
    spawn_yaw: f32,
    #[serde(default = "d_zero_f32")]
    spawn_pitch: f32,
    #[serde(default = "d_spawn_x")]
    npc_x: f64,
    #[serde(default = "d_spawn_y")]
    npc_y: f64,
    #[serde(default = "d_npc_z")]
    npc_z: f64,
    #[serde(default = "d_180")]
    npc_yaw: f32,
    #[serde(default = "d_zero_f32")]
    npc_pitch: f32,
    #[serde(default = "d_npc_type")]
    npc_entity_type: String,
    #[serde(default = "d_npc_display")]
    npc_display_name: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct ArenaConfig {
    #[serde(default = "d_dimension")]
    dimension: String,
    #[serde(default = "d_origin")]
    origin_x: i32,
    #[serde(default = "d_arena_y")]
    origin_y: i32,
    #[serde(default = "d_origin")]
    origin_z: i32,
    #[serde(default = "d_spacing")]
    instance_spacing: i32,
    #[serde(default = "d_radius")]
    radius: i32,
    #[serde(default = "d_min_radius")]
    min_radius: i32,
    #[serde(default = "d_fall_depth")]
    fall_depth: i32,
    #[serde(default = "d_start_delay")]
    start_delay_ms: i64,
    #[serde(default = "d_interval")]
    collapse_interval_ms: i64,
    #[serde(default = "d_win_hold")]
    win_hold_ms: i64,
    #[serde(default = "d_floor_block")]
    floor_block: String,
    #[serde(default = "d_support_block")]
    support_block: String,
    #[serde(default = "d_warning_block")]
    warning_block: String,
    #[serde(default = "d_border_block")]
    border_block: String,
    #[serde(default = "d_center_block")]
    center_block: String,
}

fn d_enable() -> bool { true }
fn d_false() -> bool { false }
fn d_server_name() -> String { "浅屿闲游".to_string() }
fn d_qq_group() -> String { "722632621".to_string() }
fn d_lobby_server() -> String { "lobby_1".to_string() }
fn d_menu_id() -> String { "tiankeng_menu".to_string() }
fn d_dimension() -> String { "qexed:tiankeng_lobby".to_string() }
fn d_npc_type() -> String { "minecraft:villager".to_string() }
fn d_npc_display() -> String { r#"{"text":"天坑入口","color":"gold"}"#.to_string() }
fn d_lobby_floor_y() -> i32 { -52 }
fn d_spawn_x() -> f64 { 0.5 }
fn d_spawn_y() -> f64 { -52.0 }
fn d_spawn_z() -> f64 { 0.5 }
fn d_npc_z() -> f64 { 3.5 }
fn d_zero_f32() -> f32 { 0.0 }
fn d_180() -> f32 { 180.0 }
fn d_origin() -> i32 { 2048 }
fn d_arena_y() -> i32 { -52 }
fn d_spacing() -> i32 { 96 }
fn d_radius() -> i32 { 13 }
fn d_min_radius() -> i32 { 2 }
fn d_fall_depth() -> i32 { 7 }
fn d_start_delay() -> i64 { 3500 }
fn d_interval() -> i64 { 3500 }
fn d_win_hold() -> i64 { 8000 }
fn d_floor_block() -> String { "minecraft:smooth_stone".to_string() }
fn d_support_block() -> String { "minecraft:deepslate".to_string() }
fn d_warning_block() -> String { "minecraft:yellow_concrete".to_string() }
fn d_border_block() -> String { "minecraft:blackstone".to_string() }
fn d_center_block() -> String { "minecraft:sea_lantern".to_string() }

const DEFAULT_CONFIG: &str = r#"enable = true
server_name = "浅屿闲游"
qq_group = "722632621"
lobby_server = "lobby_1"
use_proxy_lobby = false
menu_id = "tiankeng_menu"

[lobby]
dimension = "qexed:tiankeng_lobby"
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
npc_display_name = "{\"text\":\"天坑入口\",\"color\":\"gold\"}"

[arena]
dimension = "qexed:tiankeng_game"
origin_x = 2048
origin_y = -52
origin_z = 2048
instance_spacing = 96
radius = 13
min_radius = 2
fall_depth = 7
start_delay_ms = 3500
collapse_interval_ms = 3500
win_hold_ms = 8000
floor_block = "minecraft:smooth_stone"
support_block = "minecraft:deepslate"
warning_block = "minecraft:yellow_concrete"
border_block = "minecraft:blackstone"
center_block = "minecraft:sea_lantern"
"#;

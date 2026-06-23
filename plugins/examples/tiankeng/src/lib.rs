use qexed_plugin_sdk::{
    ConfigReloadPayload, NpcInteractPayload, NpcMutationOp, NpcMutationResponse, NpcUpsert,
    PlayerAction, PlayerPayload, PlayerTickPayload, PluginCommandDefinition, PluginCommandQuery,
    PluginCommandResponse, WorldEditRegion, config_load_or_create, config_read_to_string,
    storage_delete, storage_get_typed, storage_set_typed, time_millis, world_register_edit_region,
    world_set_blocks,
};
use serde::{Deserialize, Serialize};

qexed_plugin_sdk::qexed_plugin_memory!();

const CONFIG_PATH: &str = "config.toml";
const COMMAND_NAME: &str = "tiankeng";
const NPC_KEY: &str = "tiankeng:guide";
const NPC_EVENT: &str = "tiankeng";
const SESSION_PREFIX: &str = "tiankeng/session/";
const INSTANCE_GRID_WIDTH: u64 = 4096;

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_priority() -> i32 {
    285
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_init() {
    let _ = config_load_or_create(CONFIG_PATH, DEFAULT_CONFIG);
    let config = load_config();
    register_lobby(&config);
    build_lobby(&config);
    qexed_plugin_sdk::log("tiankeng initialized");
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_config_reload(ptr: i32, len: i32) {
    let Some(payload) =
        (unsafe { qexed_plugin_sdk::decode_payload::<ConfigReloadPayload>(ptr, len) })
    else {
        return;
    };
    let path = payload.path.replace('\\', "/");
    if path.ends_with("tiankeng/config.toml") || path.ends_with(CONFIG_PATH) {
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
        description_key: "qexed.plugin.tiankeng.command.description".to_string(),
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
    qexed_plugin_sdk::response_ptr_len(&open_game_menu(&load_config()))
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_player_tick(ptr: i32, len: i32) -> i64 {
    let Some(payload) =
        (unsafe { qexed_plugin_sdk::decode_payload::<PlayerTickPayload>(ptr, len) })
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

    let now = time_millis();
    if payload.position.y < f64::from(session.origin_y - config.arena.fall_depth.max(2)) {
        let _ = storage_delete(&session_key(&payload.player.uuid));
        return qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse {
            handled: true,
            actions: finish_actions(&config, false, elapsed_seconds(now, session.started_at_ms)),
        });
    }

    if session.current_radius <= config.arena.min_radius.max(1)
        && now
            >= session
                .next_collapse_at_ms
                .saturating_add(config.arena.win_hold_ms)
    {
        let _ = storage_delete(&session_key(&payload.player.uuid));
        return qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse {
            handled: true,
            actions: finish_actions(&config, true, elapsed_seconds(now, session.started_at_ms)),
        });
    }

    if now < session.next_collapse_at_ms || session.current_radius <= config.arena.min_radius.max(1)
    {
        return qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse::default());
    }

    collapse_outer_ring(&config, &mut session);
    session.next_collapse_at_ms = now.saturating_add(config.arena.collapse_interval_ms.max(500));
    let current_radius = session.current_radius;
    let _ = storage_set_typed(&session_key(&payload.player.uuid), &session);
    qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse {
        handled: true,
        actions: vec![PlayerAction::SystemMessage {
            text: format!("天坑继续塌陷，剩余半径 {current_radius} 格"),
            translate: String::new(),
            with: Vec::new(),
            overlay: true,
        }],
    })
}

fn handle_command(config: &Config, payload: &PluginCommandQuery) -> PluginCommandResponse {
    if !config.enable {
        return PluginCommandResponse {
            handled: true,
            actions: vec![message("天坑玩法已关闭")],
        };
    }

    match payload
        .argument
        .split_whitespace()
        .next()
        .unwrap_or_default()
    {
        "" | "menu" | "select" => open_game_menu(config),
        "start" | "new" | "restart" => start_game(config, &payload.player.uuid),
        "status" | "info" => status_response(config, &payload.player.uuid),
        "lobby" | "leave" | "spawn" => return_to_lobby(config, &payload.player.uuid),
        _ => PluginCommandResponse {
            handled: true,
            actions: vec![message(
                "用法: /tiankeng menu | /tiankeng start | /tiankeng status | /tiankeng lobby",
            )],
        },
    }
}

fn open_game_menu(config: &Config) -> PluginCommandResponse {
    PluginCommandResponse {
        handled: true,
        actions: vec![PlayerAction::OpenMenu {
            menu: config.menu_id.clone(),
        }],
    }
}

fn start_game(config: &Config, player_uuid: &str) -> PluginCommandResponse {
    let origin = instance_origin(config, player_uuid);
    register_arena(config, player_uuid, origin);
    if !build_arena(config, origin) {
        return PluginCommandResponse {
            handled: true,
            actions: vec![message("天坑场地生成失败，请稍后重试")],
        };
    }

    let now = time_millis();
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
    let _ = storage_set_typed(&session_key(player_uuid), &session);

    PluginCommandResponse {
        handled: true,
        actions: vec![
            message(format!(
                "{} 天坑开始，QQ群 {}",
                config.server_name, config.qq_group
            )),
            PlayerAction::SetPlayersVisible { visible: false },
            PlayerAction::Teleport {
                dimension: config.arena.dimension.clone(),
                x: f64::from(origin.0) + 0.5,
                y: f64::from(origin.1),
                z: f64::from(origin.2) + 0.5,
                yaw: Some(0.0),
                pitch: Some(0.0),
            },
        ],
    }
}

fn status_response(config: &Config, player_uuid: &str) -> PluginCommandResponse {
    let Some(session) = active_session(player_uuid) else {
        return PluginCommandResponse {
            handled: true,
            actions: vec![message("当前没有进行中的天坑对局")],
        };
    };
    let now = time_millis();
    let next = session.next_collapse_at_ms.saturating_sub(now) / 1000;
    PluginCommandResponse {
        handled: true,
        actions: vec![message(format!(
            "天坑: 剩余半径 {}，下次塌陷约 {} 秒后，目标半径 {}",
            session.current_radius,
            next,
            config.arena.min_radius.max(1)
        ))],
    }
}

fn return_to_lobby(config: &Config, player_uuid: &str) -> PluginCommandResponse {
    let _ = storage_delete(&session_key(player_uuid));
    let mut actions = vec![
        PlayerAction::SetPlayersVisible { visible: true },
        PlayerAction::Teleport {
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

fn finish_actions(config: &Config, won: bool, seconds: u64) -> Vec<PlayerAction> {
    let text = if won {
        format!("挑战成功，用时 {seconds} 秒")
    } else {
        format!("挑战失败，坚持了 {seconds} 秒")
    };
    vec![
        PlayerAction::SetPlayersVisible { visible: true },
        PlayerAction::Teleport {
            dimension: config.lobby.dimension.clone(),
            x: config.lobby.spawn_x,
            y: config.lobby.spawn_y,
            z: config.lobby.spawn_z,
            yaw: Some(config.lobby.spawn_yaw),
            pitch: Some(config.lobby.spawn_pitch),
        },
        message(text),
        PlayerAction::OpenMenu {
            menu: config.menu_id.clone(),
        },
    ]
}

fn register_lobby(config: &Config) {
    let y = config.lobby.floor_y;
    let _ = world_register_edit_region(&WorldEditRegion {
        id: "tiankeng_lobby",
        dimension: &config.lobby.dimension,
        min: (-12, y - 2, -12),
        max: (12, y + 6, 12),
        allow_player_break: false,
        allow_player_place: false,
        allow_plugin_write: true,
        runtime_only: true,
    });
}

fn build_lobby(config: &Config) {
    let mut blocks = Vec::new();
    let y = config.lobby.floor_y;
    for x in -6..=6 {
        for z in -6..=6 {
            blocks.push((
                config.lobby.dimension.as_str(),
                (x, y - 1, z),
                "minecraft:smooth_stone",
            ));
            for air_y in y..=y + 4 {
                blocks.push((
                    config.lobby.dimension.as_str(),
                    (x, air_y, z),
                    "minecraft:air",
                ));
            }
        }
    }
    for x in -3..=3 {
        blocks.push((
            config.lobby.dimension.as_str(),
            (x, y - 1, 3),
            "minecraft:sea_lantern",
        ));
    }
    let _ = world_set_blocks(blocks);
}

fn register_arena(config: &Config, player_uuid: &str, origin: (i32, i32, i32)) {
    let radius = config.arena.radius.max(4);
    let id = format!("tiankeng_{}", compact_uuid(player_uuid));
    let _ = world_register_edit_region(&WorldEditRegion {
        id: &id,
        dimension: &config.arena.dimension,
        min: (origin.0 - radius - 5, origin.1 - 10, origin.2 - radius - 5),
        max: (origin.0 + radius + 5, origin.1 + 8, origin.2 + radius + 5),
        allow_player_break: false,
        allow_player_place: false,
        allow_plugin_write: true,
        runtime_only: true,
    });
}

fn build_arena(config: &Config, origin: (i32, i32, i32)) -> bool {
    let radius = config.arena.radius.max(4);
    let floor_y = origin.1 - 1;
    let mut blocks = Vec::new();
    for x in -radius - 4..=radius + 4 {
        for z in -radius - 4..=radius + 4 {
            for y in floor_y - 3..=floor_y + 8 {
                blocks.push((
                    config.arena.dimension.as_str(),
                    (origin.0 + x, y, origin.2 + z),
                    "minecraft:air",
                ));
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
                blocks.push((
                    config.arena.dimension.as_str(),
                    (origin.0 + x, floor_y, origin.2 + z),
                    block,
                ));
                blocks.push((
                    config.arena.dimension.as_str(),
                    (origin.0 + x, floor_y - 1, origin.2 + z),
                    config.arena.support_block.as_str(),
                ));
            } else if distance_sq <= border_sq {
                blocks.push((
                    config.arena.dimension.as_str(),
                    (origin.0 + x, floor_y, origin.2 + z),
                    config.arena.border_block.as_str(),
                ));
            }
        }
    }
    world_set_blocks(blocks)
}

fn collapse_outer_ring(config: &Config, session: &mut TiankengSession) {
    let radius = session.current_radius;
    let next_radius = (radius - 1).max(config.arena.min_radius.max(1));
    let floor_y = session.origin_y - 1;
    let mut blocks = Vec::new();
    let radius_sq = radius * radius;
    let next_sq = next_radius * next_radius;
    for x in -radius..=radius {
        for z in -radius..=radius {
            let distance_sq = x * x + z * z;
            if distance_sq <= radius_sq && distance_sq > next_sq {
                blocks.push((
                    config.arena.dimension.as_str(),
                    (session.origin_x + x, floor_y, session.origin_z + z),
                    "minecraft:air",
                ));
                blocks.push((
                    config.arena.dimension.as_str(),
                    (session.origin_x + x, floor_y - 1, session.origin_z + z),
                    "minecraft:air",
                ));
            }
        }
    }
    let _ = world_set_blocks(blocks);
    session.current_radius = next_radius;
}

fn active_session(player_uuid: &str) -> Option<TiankengSession> {
    storage_get_typed(&session_key(player_uuid))
}

fn session_key(player_uuid: &str) -> String {
    format!("{SESSION_PREFIX}{player_uuid}")
}

fn message(text: impl Into<String>) -> PlayerAction {
    PlayerAction::SystemMessage {
        text: text.into(),
        translate: String::new(),
        with: Vec::new(),
        overlay: false,
    }
}

fn elapsed_seconds(now: i64, started_at_ms: i64) -> u64 {
    let elapsed = now.saturating_sub(started_at_ms).max(0) / 1000;
    u64::try_from(elapsed).unwrap_or(0)
}

fn instance_origin(config: &Config, player_uuid: &str) -> (i32, i32, i32) {
    let hash = fnv1a64(player_uuid.as_bytes());
    let spacing = i64::from(
        config
            .arena
            .instance_spacing
            .max(config.arena.radius * 3 + 16),
    );
    let x_index = (hash % INSTANCE_GRID_WIDTH) as i64;
    let z_index = ((hash / INSTANCE_GRID_WIDTH) % INSTANCE_GRID_WIDTH) as i64;
    (
        clamp_i64_to_i32(i64::from(config.arena.origin_x) + x_index * spacing),
        config.arena.origin_y,
        clamp_i64_to_i32(i64::from(config.arena.origin_z) + z_index * spacing),
    )
}

fn compact_uuid(value: &str) -> String {
    value
        .chars()
        .filter(|ch| ch.is_ascii_hexdigit())
        .take(16)
        .collect()
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

fn load_config() -> Config {
    config_read_to_string(CONFIG_PATH)
        .and_then(|content| toml::from_str(&content).ok())
        .unwrap_or_else(default_config)
}

fn default_config() -> Config {
    toml::from_str(DEFAULT_CONFIG).expect("default tiankeng config is valid")
}

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

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LobbyConfig {
    dimension: String,
    floor_y: i32,
    spawn_x: f64,
    spawn_y: f64,
    spawn_z: f64,
    spawn_yaw: f32,
    spawn_pitch: f32,
    npc_x: f64,
    npc_y: f64,
    npc_z: f64,
    npc_yaw: f32,
    npc_pitch: f32,
    npc_entity_type: String,
    npc_display_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ArenaConfig {
    dimension: String,
    origin_x: i32,
    origin_y: i32,
    origin_z: i32,
    instance_spacing: i32,
    radius: i32,
    min_radius: i32,
    fall_depth: i32,
    start_delay_ms: i64,
    collapse_interval_ms: i64,
    win_hold_ms: i64,
    floor_block: String,
    support_block: String,
    warning_block: String,
    border_block: String,
    center_block: String,
}

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

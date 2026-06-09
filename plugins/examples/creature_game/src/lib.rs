use qexed_plugin_sdk::{
    ConfigReloadPayload, NpcInteractPayload, NpcMutationOp, NpcMutationResponse, NpcUpsert,
    PlayerAction, PlayerPayload, PluginCommandDefinition, PluginCommandQuery,
    PluginCommandResponse, RuntimeEntity, WorldEditRegion, config_load_or_create,
    config_read_to_string, entity_remove, entity_upsert, storage_set_typed, time_millis,
    world_register_edit_region, world_set_blocks,
};
use serde::{Deserialize, Serialize};

qexed_plugin_sdk::qexed_plugin_memory!();

const CONFIG_PATH: &str = "config.toml";
const COMMAND_NAME: &str = "creature";
const NPC_KEY: &str = "creature_game:guide";
const NPC_EVENT: &str = "creature_game";
const SESSION_PREFIX: &str = "creature_game/session/";
const ENTITY_CLEANUP_LIMIT: usize = 64;
const INSTANCE_GRID_WIDTH: u64 = 4096;

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_priority() -> i32 {
    260
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_init() {
    let _ = config_load_or_create(CONFIG_PATH, DEFAULT_CONFIG);
    let config = load_config();
    register_lobby(&config);
    qexed_plugin_sdk::log("creature_game initialized: 浅屿闲游 QQ 722632621");
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_config_reload(ptr: i32, len: i32) {
    let Some(payload) =
        (unsafe { qexed_plugin_sdk::decode_payload::<ConfigReloadPayload>(ptr, len) })
    else {
        return;
    };
    let path = payload.path.replace('\\', "/");
    if path.ends_with("creature_game/config.toml") || path.ends_with(CONFIG_PATH) {
        register_lobby(&load_config());
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_player_join(ptr: i32, len: i32) {
    let Some(payload) = (unsafe { qexed_plugin_sdk::decode_payload::<PlayerPayload>(ptr, len) })
    else {
        return;
    };
    let config = load_config();
    if config.enable {
        let _ = storage_set_typed(&session_key(&payload.uuid), &CreatureSession::lobby());
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_player_leave(ptr: i32, len: i32) {
    let Some(payload) = (unsafe { qexed_plugin_sdk::decode_payload::<PlayerPayload>(ptr, len) })
    else {
        return;
    };
    cleanup_player_entities(&payload.uuid);
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_commands(_ptr: i32, _len: i32) -> i64 {
    qexed_plugin_sdk::response_ptr_len(&PluginCommandDefinition {
        name: COMMAND_NAME.to_string(),
        description_key: "浅屿闲游 生物游戏".to_string(),
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
        "" | "menu" => open_game_menu(&config),
        "start" | "new" | "new_game" | "restart" => start_game(&config, &payload.player.uuid),
        "lobby" | "leave" | "spawn" => return_to_lobby(&config, &payload.player.uuid),
        _ => PluginCommandResponse {
            handled: true,
            actions: vec![message(
                "用法: /creature menu | /creature start | /creature lobby",
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
                name: "浅屿闲游".to_string(),
                display_name: "{\"text\":\"浅屿闲游\",\"color\":\"aqua\"}".to_string(),
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

fn open_game_menu(config: &Config) -> PluginCommandResponse {
    PluginCommandResponse {
        handled: true,
        actions: vec![PlayerAction::OpenMenu {
            menu: config.menu_id.clone(),
        }],
    }
}

fn start_game(config: &Config, player_uuid: &str) -> PluginCommandResponse {
    if !config.enable {
        return PluginCommandResponse::default();
    }

    cleanup_player_entities(player_uuid);
    let origin = instance_origin(config, player_uuid);
    register_arena(config, player_uuid, origin);
    if !build_arena(config, origin) {
        return PluginCommandResponse {
            handled: true,
            actions: vec![message("场地生成失败，请稍后重试")],
        };
    }
    spawn_creatures(config, player_uuid, origin);

    let session = CreatureSession {
        active: true,
        dimension: config.arena.dimension.clone(),
        origin_x: origin.0,
        origin_y: origin.1,
        origin_z: origin.2,
        started_at_ms: time_millis(),
    };
    let _ = storage_set_typed(&session_key(player_uuid), &session);

    PluginCommandResponse {
        handled: true,
        actions: vec![
            message(format!(
                "{} · 新游戏开始，QQ群 {}",
                config.server_name, config.qq_group
            )),
            PlayerAction::SetPlayersVisible { visible: false },
            PlayerAction::Teleport {
                dimension: config.arena.dimension.clone(),
                x: origin.0 as f64 + 0.5,
                y: origin.1 as f64 + 1.0,
                z: origin.2 as f64 + 0.5,
                yaw: Some(0.0),
                pitch: Some(0.0),
            },
        ],
    }
}

fn return_to_lobby(config: &Config, player_uuid: &str) -> PluginCommandResponse {
    cleanup_player_entities(player_uuid);
    let _ = storage_set_typed(&session_key(player_uuid), &CreatureSession::lobby());

    PluginCommandResponse {
        handled: true,
        actions: vec![
            message(format!("正在返回大厅: {}", config.lobby_server)),
            PlayerAction::SetPlayersVisible { visible: true },
            PlayerAction::ProxyConnect {
                server: config.lobby_server.clone(),
                message: String::new(),
            },
        ],
    }
}

fn register_lobby(config: &Config) {
    let y = config.lobby.floor_y;
    let _ = world_register_edit_region(&WorldEditRegion {
        id: "creature_game_lobby",
        dimension: &config.lobby.dimension,
        min: (-12, y - 1, -12),
        max: (12, y + 6, 12),
        allow_player_break: false,
        allow_player_place: false,
        allow_plugin_write: true,
        runtime_only: true,
    });
    let mut blocks = Vec::new();
    for x in -5..=5 {
        for z in -5..=5 {
            blocks.push((
                config.lobby.dimension.as_str(),
                (x, y - 1, z),
                "minecraft:polished_andesite",
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
    blocks.push((
        config.lobby.dimension.as_str(),
        (0, y, 0),
        "minecraft:sea_lantern",
    ));
    let _ = world_set_blocks(blocks);
}

fn register_arena(config: &Config, player_uuid: &str, origin: (i32, i32, i32)) {
    let radius = config.arena.radius.max(4);
    let id = format!("creature_game_{}", compact_uuid(player_uuid));
    let _ = world_register_edit_region(&WorldEditRegion {
        id: &id,
        dimension: &config.arena.dimension,
        min: (origin.0 - radius - 2, origin.1 - 2, origin.2 - radius - 2),
        max: (origin.0 + radius + 2, origin.1 + 8, origin.2 + radius + 2),
        allow_player_break: false,
        allow_player_place: false,
        allow_plugin_write: true,
        runtime_only: true,
    });
}

fn build_arena(config: &Config, origin: (i32, i32, i32)) -> bool {
    let radius = config.arena.radius.max(4);
    let mut blocks = Vec::new();
    for dx in -radius..=radius {
        for dz in -radius..=radius {
            let x = origin.0 + dx;
            let z = origin.2 + dz;
            blocks.push((
                config.arena.dimension.as_str(),
                (x, origin.1 - 1, z),
                config.arena.floor_block.as_str(),
            ));
            for dy in 0..=5 {
                blocks.push((
                    config.arena.dimension.as_str(),
                    (x, origin.1 + dy, z),
                    "minecraft:air",
                ));
            }
            if dx.abs() == radius || dz.abs() == radius {
                blocks.push((
                    config.arena.dimension.as_str(),
                    (x, origin.1, z),
                    config.arena.wall_block.as_str(),
                ));
            }
        }
    }
    for (dx, dz) in [
        (-radius + 1, -radius + 1),
        (-radius + 1, radius - 1),
        (radius - 1, -radius + 1),
        (radius - 1, radius - 1),
    ] {
        blocks.push((
            config.arena.dimension.as_str(),
            (origin.0 + dx, origin.1, origin.2 + dz),
            "minecraft:sea_lantern",
        ));
    }
    world_set_blocks(blocks)
}

fn spawn_creatures(config: &Config, player_uuid: &str, origin: (i32, i32, i32)) {
    let count = config.arena.creature_count.clamp(1, ENTITY_CLEANUP_LIMIT);
    for index in 0..count {
        let entity_type = if config.creatures.is_empty() {
            "minecraft:pig"
        } else {
            config.creatures[index % config.creatures.len()].as_str()
        };
        let offset = creature_offset(index as i32);
        let _ = entity_upsert(&RuntimeEntity {
            key: &creature_key(player_uuid, index),
            dimension: &config.arena.dimension,
            entity_type,
            x: origin.0 as f64 + offset.0 as f64 + 0.5,
            y: origin.1 as f64,
            z: origin.2 as f64 + offset.1 as f64 + 0.5,
            yaw: 0.0,
            pitch: 0.0,
            display_name: "{\"text\":\"浅屿闲游生物\",\"color\":\"green\"}",
            ai: "vanilla",
            ai_params_json: "{}",
            auto_jump: true,
        });
    }
}

fn cleanup_player_entities(player_uuid: &str) {
    for index in 0..ENTITY_CLEANUP_LIMIT {
        let _ = entity_remove(&creature_key(player_uuid, index));
    }
}

fn creature_offset(index: i32) -> (i32, i32) {
    const OFFSETS: &[(i32, i32)] = &[
        (-3, -3),
        (0, -4),
        (3, -3),
        (-4, 0),
        (4, 0),
        (-3, 3),
        (0, 4),
        (3, 3),
    ];
    OFFSETS[index.rem_euclid(OFFSETS.len() as i32) as usize]
}

fn instance_origin(config: &Config, player_uuid: &str) -> (i32, i32, i32) {
    let slot = stable_hash(player_uuid) % (INSTANCE_GRID_WIDTH * INSTANCE_GRID_WIDTH);
    let grid_x = (slot % INSTANCE_GRID_WIDTH) as i32;
    let grid_z = (slot / INSTANCE_GRID_WIDTH) as i32;
    let spacing = config
        .arena
        .instance_spacing
        .max(config.arena.radius.max(4) * 2 + 8);
    (
        config.arena.origin_x + grid_x * spacing,
        config.arena.origin_y,
        config.arena.origin_z + grid_z * spacing,
    )
}

fn stable_hash(value: &str) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x1000_0000_01b3);
    }
    hash
}

fn creature_key(player_uuid: &str, index: usize) -> String {
    format!("creature_game/{}/{}", compact_uuid(player_uuid), index)
}

fn session_key(player_uuid: &str) -> String {
    format!("{SESSION_PREFIX}{}", compact_uuid(player_uuid))
}

fn compact_uuid(player_uuid: &str) -> String {
    player_uuid
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .collect()
}

fn message(text: impl Into<String>) -> PlayerAction {
    PlayerAction::SystemMessage {
        text: text.into(),
        translate: String::new(),
        with: Vec::new(),
        overlay: false,
    }
}

fn load_config() -> Config {
    config_read_to_string(CONFIG_PATH)
        .and_then(|content| toml::from_str::<Config>(&content).ok())
        .unwrap_or_default()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CreatureSession {
    active: bool,
    dimension: String,
    origin_x: i32,
    origin_y: i32,
    origin_z: i32,
    started_at_ms: i64,
}

impl CreatureSession {
    fn lobby() -> Self {
        Self {
            active: false,
            dimension: String::new(),
            origin_x: 0,
            origin_y: 0,
            origin_z: 0,
            started_at_ms: 0,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct Config {
    #[serde(default = "default_true")]
    enable: bool,
    #[serde(default = "default_server_name")]
    server_name: String,
    #[serde(default = "default_qq_group")]
    qq_group: String,
    #[serde(default = "default_menu_id")]
    menu_id: String,
    #[serde(default = "default_lobby_server")]
    lobby_server: String,
    #[serde(default)]
    lobby: LobbyConfig,
    #[serde(default)]
    arena: ArenaConfig,
    #[serde(default = "default_creatures")]
    creatures: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enable: default_true(),
            server_name: default_server_name(),
            qq_group: default_qq_group(),
            menu_id: default_menu_id(),
            lobby_server: default_lobby_server(),
            lobby: LobbyConfig::default(),
            arena: ArenaConfig::default(),
            creatures: default_creatures(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct LobbyConfig {
    #[serde(default = "default_lobby_dimension")]
    dimension: String,
    #[serde(default = "default_lobby_floor_y")]
    floor_y: i32,
    #[serde(default = "default_lobby_y")]
    npc_y: f64,
    #[serde(default)]
    npc_x: f64,
    #[serde(default = "default_npc_z")]
    npc_z: f64,
    #[serde(default = "default_npc_yaw")]
    npc_yaw: f32,
    #[serde(default)]
    npc_pitch: f32,
    #[serde(default = "default_npc_entity_type")]
    npc_entity_type: String,
}

impl Default for LobbyConfig {
    fn default() -> Self {
        Self {
            dimension: default_lobby_dimension(),
            floor_y: default_lobby_floor_y(),
            npc_y: default_lobby_y(),
            npc_x: 0.0,
            npc_z: default_npc_z(),
            npc_yaw: default_npc_yaw(),
            npc_pitch: 0.0,
            npc_entity_type: default_npc_entity_type(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct ArenaConfig {
    #[serde(default = "default_arena_dimension")]
    dimension: String,
    #[serde(default = "default_arena_origin_x")]
    origin_x: i32,
    #[serde(default = "default_arena_origin_y")]
    origin_y: i32,
    #[serde(default = "default_arena_origin_z")]
    origin_z: i32,
    #[serde(default = "default_instance_spacing")]
    instance_spacing: i32,
    #[serde(default = "default_arena_radius")]
    radius: i32,
    #[serde(default = "default_creature_count")]
    creature_count: usize,
    #[serde(default = "default_floor_block")]
    floor_block: String,
    #[serde(default = "default_wall_block")]
    wall_block: String,
}

impl Default for ArenaConfig {
    fn default() -> Self {
        Self {
            dimension: default_arena_dimension(),
            origin_x: default_arena_origin_x(),
            origin_y: default_arena_origin_y(),
            origin_z: default_arena_origin_z(),
            instance_spacing: default_instance_spacing(),
            radius: default_arena_radius(),
            creature_count: default_creature_count(),
            floor_block: default_floor_block(),
            wall_block: default_wall_block(),
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

fn default_menu_id() -> String {
    "creature_game".to_string()
}

fn default_lobby_server() -> String {
    "lobby_1".to_string()
}

fn default_lobby_dimension() -> String {
    "qexed:creature_lobby".to_string()
}

fn default_lobby_floor_y() -> i32 {
    -52
}

fn default_lobby_y() -> f64 {
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

fn default_arena_dimension() -> String {
    "qexed:creature_game".to_string()
}

fn default_arena_origin_x() -> i32 {
    2048
}

fn default_arena_origin_y() -> i32 {
    -52
}

fn default_arena_origin_z() -> i32 {
    2048
}

fn default_instance_spacing() -> i32 {
    96
}

fn default_arena_radius() -> i32 {
    10
}

fn default_creature_count() -> usize {
    8
}

fn default_floor_block() -> String {
    "minecraft:grass_block".to_string()
}

fn default_wall_block() -> String {
    "minecraft:oak_fence".to_string()
}

fn default_creatures() -> Vec<String> {
    vec![
        "minecraft:pig".to_string(),
        "minecraft:cow".to_string(),
        "minecraft:sheep".to_string(),
        "minecraft:chicken".to_string(),
        "minecraft:zombie".to_string(),
        "minecraft:skeleton".to_string(),
        "minecraft:spider".to_string(),
        "minecraft:creeper".to_string(),
    ]
}

const DEFAULT_CONFIG: &str = r#"enable = true
server_name = "浅屿闲游"
qq_group = "722632621"
menu_id = "creature_game"
lobby_server = "lobby_1"
creatures = [
    "minecraft:pig",
    "minecraft:cow",
    "minecraft:sheep",
    "minecraft:chicken",
    "minecraft:zombie",
    "minecraft:skeleton",
    "minecraft:spider",
    "minecraft:creeper",
]

[lobby]
dimension = "qexed:creature_lobby"
floor_y = -52
npc_x = 0.0
npc_y = -52.0
npc_z = 3.5
npc_yaw = 180.0
npc_pitch = 0.0
npc_entity_type = "minecraft:villager"

[arena]
dimension = "qexed:creature_game"
origin_x = 2048
origin_y = -52
origin_z = 2048
instance_spacing = 96
radius = 10
creature_count = 8
floor_block = "minecraft:grass_block"
wall_block = "minecraft:oak_fence"
"#;

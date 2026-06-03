use std::collections::BTreeMap;

use qexed_config::{public::mongodb::MongoConfig, public::mysql::MysqlConfig};
use serde::{Deserialize, Serialize, de};

#[derive(Debug, Serialize, Deserialize)]
pub struct Server {
    pub ip: String,

    pub online: bool,

    pub max_player: i32,

    pub display_players: bool,

    pub online_mode: bool,

    #[serde(default)]
    pub log_level: ServerLogLevel,

    #[serde(default = "default_mojang_cache_path")]
    pub mojang_cache_path: String,

    #[serde(default)]
    pub lan_discovery: LanDiscovery,

    pub network_compression_threshold: isize,

    #[serde(default, skip)]
    pub proxy: bool,

    #[serde(default, skip)]
    pub proxy_protocol: ForwardingMode,

    #[serde(default, skip)]
    pub proxy_server_id: String,

    #[serde(default, skip)]
    pub proxy_token: String,

    #[serde(default = "default_proxy_online_mode", skip)]
    pub proxy_online_mode: bool,

    pub max_port_connections: u16,

    pub rate_limit_window_secs: u64,

    pub rate_limit_max_attempts: u32,

    #[serde(default)]
    pub click_detection: ClickDetection,

    #[serde(default)]
    pub gameplay: Gameplay,

    pub motd: Vec<String>,

    #[serde(default)]
    pub code_of_conduct: bool,

    #[serde(default, skip)]
    pub world: World,

    #[serde(default)]
    pub player_data: PlayerData,

    #[serde(default)]
    pub player_messages: PlayerMessages,

    #[serde(default)]
    pub player_audit: PlayerAudit,

    #[serde(default)]
    pub content_filter: ContentFilter,

    #[serde(default)]
    pub permissions: Permissions,

    #[serde(default)]
    pub resource_pack: ResourcePack,

    #[serde(default)]
    pub entities: Entities,

    #[serde(default)]
    pub scoreboard: Scoreboard,

    #[serde(default)]
    pub menus: Menus,

    #[serde(default)]
    pub entity_rendering: EntityRendering,

    #[serde(default)]
    pub placeholders: Placeholders,

    #[serde(default)]
    pub lobby: Lobby,

    pub favicon: String,
}

impl Default for Server {
    fn default() -> Self {
        Self {
            ip: "0.0.0.0:25565".to_owned(),
            online: false,
            max_player: -1,
            display_players: true,
            online_mode: true,
            log_level: ServerLogLevel::default(),
            mojang_cache_path: default_mojang_cache_path(),
            lan_discovery: LanDiscovery::default(),
            network_compression_threshold: 256,
            proxy: false,
            proxy_protocol: ForwardingMode::QTunnel,
            proxy_server_id: String::new(),
            proxy_token: nanoid::nanoid!(),
            proxy_online_mode: default_proxy_online_mode(),
            rate_limit_window_secs: 60,
            rate_limit_max_attempts: 6,
            click_detection: ClickDetection::default(),
            gameplay: Gameplay::default(),
            motd: vec![
                "Qexed服务器awa".to_string()
            ],
            code_of_conduct: false,
            world: World::default(),
            player_data: PlayerData::default(),
            player_messages: PlayerMessages::default(),
            player_audit: PlayerAudit::default(),
            content_filter: ContentFilter::default(),
            permissions: Permissions::default(),
            resource_pack: ResourcePack::default(),
            entities: Entities::default(),
            scoreboard: Scoreboard::default(),
            menus: Menus::default(),
            entity_rendering: EntityRendering::default(),
            placeholders: Placeholders::default(),
            lobby: Lobby::default(),
            favicon: "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAEAAAABACAYAAACqaXHeAAAACXBIWXMAAA9hAAAPYQGoP6dpAAACtklEQVR42u2ay0rDQBSGJ2EQCipqERU3SkFQQUERRJSCuHDrQvcu3Powbn0DH6IIohQKIi26ELRF8FLxAlbsyksTmTC2yVwyk3ZizmySkjaT/zvnP5mT1PpuDJTgYaOEDwAAAAAAAAAAAAAAAAAAAAAAAABI5MAyX7YsS3lC07pvLCs+jAAd4DpqARXxia8BpsOzk5r6QgB0iTfZOnbU0TO9bthRCIhT0cRRpX4U4v2yUnUezJokrA10iReZX7XWYN0CdNUO0Wj7BUzm+rGJvpSJKn2c/M7ZikKQArC/1RqVnYPvjon3gyELwWp+N+j3QyJ8cHYNTU30uPuvz1V3W8yd/IHAmph3UToLqOi5uAAc8dnNDU/0eeW95SSf10UPQlgAQZEPC0U0kzAv5R3xtPDFhRQaHc+4+7flK1R5SqHba30W0HUHoe2gVAOIeFo4EZ8v1Bt7dTSzuuTCCqoHKqmvAoRAYM2PWdF3PH9eeQwUvzyf8WpB6cZiimve6rrdqmYMcylMCh6d8seFO088GbkiZkaB5ftOd4yYl/601x/KvylPxBN7DPUidC+Yjn4FTtQqYazBswETgCNueHygIR41xL94wi8ua2gk/eEer771ofvTI7SX/wrl7043Tszb4O6ijda3s6746bFu1J8e8rLCEX92WHL3afG8KDdHsB0AWHNw1wEOhJG5FTQ52uV+fqk9+grnpTHP68YCIBDoEZTuMguhZiBGA5CdTHYl2I5nCEHnhldj/1mcyBrDCABBdaEd/YUdx6jpPI8xAHQWQJl+w6gMoK0QNhNkmy3jLCCyihRprCJ5JqialrINjEhEVd8VYNPEs+4MUSynjXwsLmMJ7W+GTIh+OxsmOy7iY7cUjoN4aIaiAhCX6AcWQdX1eJz+TYbjfPFQAwAAAAAAAAAAAACl8QOub9TOwLTmGwAAAABJRU5ErkJggg==".to_string(),
            max_port_connections: u16::MAX,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct Lobby {
    #[serde(default)]
    pub enable: bool,

    #[serde(default = "default_lobby_protect_world")]
    pub protect_world: bool,

    #[serde(default = "default_lobby_menu_title")]
    pub menu_title: String,

    #[serde(default = "default_lobby_menu_rows")]
    pub menu_rows: u8,

    #[serde(default)]
    pub navigator: LobbyNavigator,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub menu_items: Vec<LobbyMenuItem>,

    #[serde(default)]
    pub broadcast: LobbyBroadcast,

    #[serde(default)]
    pub boss_bar: LobbyBossBar,

    #[serde(default)]
    pub health_check: LobbyHealthCheck,
}

impl Default for Lobby {
    fn default() -> Self {
        Self {
            enable: false,
            protect_world: default_lobby_protect_world(),
            menu_title: default_lobby_menu_title(),
            menu_rows: default_lobby_menu_rows(),
            navigator: LobbyNavigator::default(),
            menu_items: Vec::new(),
            broadcast: LobbyBroadcast::default(),
            boss_bar: LobbyBossBar::default(),
            health_check: LobbyHealthCheck::default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct LobbyHealthCheck {
    #[serde(default = "default_lobby_health_check_interval_secs")]
    pub interval_secs: u64,

    #[serde(default = "default_lobby_health_check_timeout_ms")]
    pub timeout_ms: u64,
}

impl Default for LobbyHealthCheck {
    fn default() -> Self {
        Self {
            interval_secs: default_lobby_health_check_interval_secs(),
            timeout_ms: default_lobby_health_check_timeout_ms(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct LobbyBossBar {
    #[serde(default)]
    pub enable: bool,

    #[serde(default = "default_lobby_boss_bar_title")]
    pub title: String,

    #[serde(default)]
    pub color: LobbyBossBarColor,

    #[serde(default)]
    pub overlay: LobbyBossBarOverlay,

    #[serde(default)]
    pub darken_screen: bool,

    #[serde(default)]
    pub play_music: bool,

    #[serde(default)]
    pub create_world_fog: bool,
}

impl Default for LobbyBossBar {
    fn default() -> Self {
        Self {
            enable: false,
            title: default_lobby_boss_bar_title(),
            color: LobbyBossBarColor::default(),
            overlay: LobbyBossBarOverlay::default(),
            darken_screen: false,
            play_music: false,
            create_world_fog: false,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LobbyBossBarColor {
    Pink,
    Blue,
    Red,
    #[default]
    Green,
    Yellow,
    Purple,
    White,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LobbyBossBarOverlay {
    #[default]
    Progress,
    #[serde(rename = "notched_6")]
    Notched6,
    #[serde(rename = "notched_10")]
    Notched10,
    #[serde(rename = "notched_12")]
    Notched12,
    #[serde(rename = "notched_20")]
    Notched20,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct LobbyBroadcast {
    #[serde(default)]
    pub enable: bool,

    #[serde(default = "default_lobby_broadcast_interval_secs")]
    pub interval_secs: u64,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub messages: Vec<String>,
}

impl Default for LobbyBroadcast {
    fn default() -> Self {
        Self {
            enable: false,
            interval_secs: default_lobby_broadcast_interval_secs(),
            messages: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct LobbyNavigator {
    #[serde(default = "default_lobby_navigator_enable")]
    pub enable: bool,

    #[serde(default = "default_lobby_navigator_slot")]
    pub slot: u8,

    #[serde(default = "default_lobby_navigator_item")]
    pub item: String,

    #[serde(default = "default_lobby_navigator_name")]
    pub name: String,
}

impl Default for LobbyNavigator {
    fn default() -> Self {
        Self {
            enable: default_lobby_navigator_enable(),
            slot: default_lobby_navigator_slot(),
            item: default_lobby_navigator_item(),
            name: default_lobby_navigator_name(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct LobbyMenuItem {
    #[serde(default)]
    pub slot: u8,

    #[serde(default = "default_lobby_menu_item_item")]
    pub item: String,

    #[serde(default = "default_lobby_menu_item_unknown_item")]
    pub unknown_item: String,

    #[serde(default = "default_lobby_menu_item_offline_item")]
    pub offline_item: String,

    #[serde(default = "default_lobby_menu_item_disabled_item")]
    pub disabled_item: String,

    #[serde(default = "default_lobby_menu_item_maintenance_item")]
    pub maintenance_item: String,

    #[serde(default)]
    pub name: String,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lore: Vec<String>,

    #[serde(default)]
    pub action: LobbyAction,
}

impl Default for LobbyMenuItem {
    fn default() -> Self {
        Self {
            slot: 0,
            item: default_lobby_menu_item_item(),
            unknown_item: default_lobby_menu_item_unknown_item(),
            offline_item: default_lobby_menu_item_offline_item(),
            disabled_item: default_lobby_menu_item_disabled_item(),
            maintenance_item: default_lobby_menu_item_maintenance_item(),
            name: String::new(),
            lore: Vec::new(),
            action: LobbyAction::default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct LobbyAction {
    #[serde(default)]
    pub kind: LobbyActionKind,

    #[serde(default)]
    pub target: String,

    #[serde(default)]
    pub message: String,
}

impl Default for LobbyAction {
    fn default() -> Self {
        Self {
            kind: LobbyActionKind::None,
            target: String::new(),
            message: String::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LobbyActionKind {
    #[default]
    None,
    OpenMenu,
    Transfer,
    Message,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct Menus {
    #[serde(default)]
    pub enable: bool,

    #[serde(default)]
    pub reset_inventory_on_join: bool,

    #[serde(default = "default_menus_fixed_slots_only")]
    pub fixed_slots_only: bool,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hotbar_items: Vec<MenuHotbarItem>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub chests: Vec<ChestMenu>,
}

impl Default for Menus {
    fn default() -> Self {
        Self {
            enable: false,
            reset_inventory_on_join: false,
            fixed_slots_only: default_menus_fixed_slots_only(),
            hotbar_items: Vec::new(),
            chests: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct MenuHotbarItem {
    #[serde(default)]
    pub slot: u8,

    #[serde(default = "default_menu_item_item")]
    pub item: String,

    #[serde(default)]
    pub name: String,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lore: Vec<String>,

    #[serde(default)]
    pub action: MenuAction,
}

impl Default for MenuHotbarItem {
    fn default() -> Self {
        Self {
            slot: 0,
            item: default_menu_item_item(),
            name: String::new(),
            lore: Vec::new(),
            action: MenuAction::default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct ChestMenu {
    #[serde(default)]
    pub id: String,

    #[serde(default = "default_chest_menu_title")]
    pub title: String,

    #[serde(default = "default_chest_menu_rows")]
    pub rows: u8,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub items: Vec<MenuItem>,
}

impl Default for ChestMenu {
    fn default() -> Self {
        Self {
            id: String::new(),
            title: default_chest_menu_title(),
            rows: default_chest_menu_rows(),
            items: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct MenuItem {
    #[serde(default)]
    pub slot: u8,

    #[serde(default = "default_menu_item_item")]
    pub item: String,

    #[serde(default)]
    pub name: String,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lore: Vec<String>,

    #[serde(default)]
    pub action: MenuAction,
}

impl Default for MenuItem {
    fn default() -> Self {
        Self {
            slot: 0,
            item: default_menu_item_item(),
            name: String::new(),
            lore: Vec::new(),
            action: MenuAction::default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct MenuAction {
    #[serde(default)]
    pub kind: MenuActionKind,

    #[serde(default)]
    pub target: String,

    #[serde(default)]
    pub message: String,
}

impl Default for MenuAction {
    fn default() -> Self {
        Self {
            kind: MenuActionKind::None,
            target: String::new(),
            message: String::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MenuActionKind {
    #[default]
    None,
    OpenMenu,
    Transfer,
    Command,
    Message,
    HidePlayers,
    ShowPlayers,
    TogglePlayers,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct EntityRendering {
    #[serde(default = "default_entity_render_distance")]
    pub default_distance: f64,

    #[serde(default = "default_player_render_distance")]
    pub player_distance: f64,

    #[serde(default = "default_npc_render_distance")]
    pub npc_distance: f64,

    #[serde(default = "default_hologram_render_distance")]
    pub hologram_distance: f64,

    #[serde(default = "default_item_render_distance")]
    pub item_distance: f64,

    #[serde(default = "default_item_merge_radius")]
    pub item_merge_radius: f64,

    #[serde(default = "default_item_merge_max_stack")]
    pub item_merge_max_stack: i32,

    #[serde(default = "default_entity_stack_threshold")]
    pub stack_threshold: usize,

    #[serde(default = "default_entity_stack_radius")]
    pub stack_radius: f64,
}

impl Default for EntityRendering {
    fn default() -> Self {
        Self {
            default_distance: default_entity_render_distance(),
            player_distance: default_player_render_distance(),
            npc_distance: default_npc_render_distance(),
            hologram_distance: default_hologram_render_distance(),
            item_distance: default_item_render_distance(),
            item_merge_radius: default_item_merge_radius(),
            item_merge_max_stack: default_item_merge_max_stack(),
            stack_threshold: default_entity_stack_threshold(),
            stack_radius: default_entity_stack_radius(),
        }
    }
}

fn default_chest_menu_title() -> String {
    "Menu".to_string()
}

fn default_chest_menu_rows() -> u8 {
    3
}

fn default_menu_item_item() -> String {
    "minecraft:paper".to_string()
}

fn default_menus_fixed_slots_only() -> bool {
    true
}

fn default_entity_render_distance() -> f64 {
    64.0
}

fn default_player_render_distance() -> f64 {
    64.0
}

fn default_npc_render_distance() -> f64 {
    64.0
}

fn default_hologram_render_distance() -> f64 {
    64.0
}

fn default_item_render_distance() -> f64 {
    32.0
}

fn default_item_merge_radius() -> f64 {
    2.0
}

fn default_item_merge_max_stack() -> i32 {
    64
}

fn default_entity_stack_threshold() -> usize {
    20
}

fn default_entity_stack_radius() -> f64 {
    4.0
}

fn default_lobby_protect_world() -> bool {
    true
}

fn default_lobby_menu_title() -> String {
    "Server Selector".to_string()
}

fn default_lobby_menu_rows() -> u8 {
    3
}

fn default_lobby_navigator_enable() -> bool {
    true
}

fn default_lobby_navigator_slot() -> u8 {
    4
}

fn default_lobby_navigator_item() -> String {
    "minecraft:compass".to_string()
}

fn default_lobby_navigator_name() -> String {
    "Server Selector".to_string()
}

fn default_lobby_menu_item_item() -> String {
    "minecraft:paper".to_string()
}

fn default_lobby_menu_item_unknown_item() -> String {
    "minecraft:gray_dye".to_string()
}

fn default_lobby_menu_item_offline_item() -> String {
    "minecraft:barrier".to_string()
}

fn default_lobby_menu_item_disabled_item() -> String {
    "minecraft:barrier".to_string()
}

fn default_lobby_menu_item_maintenance_item() -> String {
    "minecraft:barrier".to_string()
}

fn default_lobby_broadcast_interval_secs() -> u64 {
    60
}

fn default_lobby_boss_bar_title() -> String {
    "Welcome to Qexed".to_string()
}

fn default_lobby_health_check_interval_secs() -> u64 {
    15
}

fn default_lobby_health_check_timeout_ms() -> u64 {
    600
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct Scoreboard {
    #[serde(default)]
    pub enable: bool,

    #[serde(default = "default_scoreboard_objective")]
    pub objective: String,

    #[serde(default = "default_scoreboard_title")]
    pub title: String,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lines: Vec<String>,
}

impl Default for Scoreboard {
    fn default() -> Self {
        Self {
            enable: false,
            objective: default_scoreboard_objective(),
            title: default_scoreboard_title(),
            lines: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct Placeholders {
    #[serde(default = "default_placeholders_enable")]
    pub enable: bool,
}

impl Default for Placeholders {
    fn default() -> Self {
        Self {
            enable: default_placeholders_enable(),
        }
    }
}

fn default_placeholders_enable() -> bool {
    true
}

fn default_scoreboard_objective() -> String {
    "qexed".to_string()
}

fn default_scoreboard_title() -> String {
    "Qexed".to_string()
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Entities {
    #[serde(default)]
    pub enable: bool,

    #[serde(default = "default_entities_dimension")]
    pub dimension: String,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub list: Vec<Entity>,

    #[serde(default)]
    pub spawning: EntitySpawning,
}

impl Default for Entities {
    fn default() -> Self {
        Self {
            enable: false,
            dimension: default_entities_dimension(),
            list: Vec::new(),
            spawning: EntitySpawning::default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Npcs {
    #[serde(default)]
    pub enable: bool,

    #[serde(default = "default_entities_dimension")]
    pub dimension: String,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub list: Vec<Npc>,
}

impl Default for Npcs {
    fn default() -> Self {
        Self {
            enable: false,
            dimension: default_entities_dimension(),
            list: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Npc {
    #[serde(default)]
    pub id: String,

    #[serde(default)]
    pub name: String,

    #[serde(default)]
    pub display_name: String,

    #[serde(default)]
    pub entity_type: String,

    #[serde(default)]
    pub skin_textures: String,

    #[serde(default)]
    pub skin_signature: String,

    #[serde(default)]
    pub skin_player_id: String,

    #[serde(default)]
    pub x: f64,

    #[serde(default)]
    pub y: f64,

    #[serde(default)]
    pub z: f64,

    #[serde(default)]
    pub yaw: f32,

    #[serde(default)]
    pub pitch: f32,

    #[serde(default)]
    pub on_ground: bool,

    #[serde(default)]
    pub look_at_players: bool,

    #[serde(default = "default_npc_main_hand_event")]
    pub main_hand_event: String,

    #[serde(default = "default_npc_off_hand_event")]
    pub off_hand_event: String,

    #[serde(default = "default_npc_attack_event")]
    pub attack_event: String,

    #[serde(default)]
    pub actions: NpcActions,
}

impl Default for Npc {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            display_name: String::new(),
            entity_type: String::new(),
            skin_textures: String::new(),
            skin_signature: String::new(),
            skin_player_id: String::new(),
            x: 0.0,
            y: 64.0,
            z: 0.0,
            yaw: 0.0,
            pitch: 0.0,
            on_ground: true,
            look_at_players: false,
            main_hand_event: default_npc_main_hand_event(),
            off_hand_event: default_npc_off_hand_event(),
            attack_event: default_npc_attack_event(),
            actions: NpcActions::default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, Default)]
pub struct NpcActions {
    #[serde(default)]
    pub main_hand: MenuAction,

    #[serde(default)]
    pub off_hand: MenuAction,

    #[serde(default)]
    pub attack: MenuAction,
}

fn default_entities_dimension() -> String {
    "minecraft:overworld".to_string()
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Entity {
    #[serde(default)]
    pub id: String,

    #[serde(default)]
    pub kind: EntityKind,

    #[serde(default = "default_entity_type")]
    pub entity_type: String,

    #[serde(default)]
    pub name: String,

    #[serde(default)]
    pub display_name: String,

    #[serde(default)]
    pub skin_textures: String,

    #[serde(default)]
    pub skin_signature: String,

    #[serde(default)]
    pub skin_player_id: String,

    #[serde(default)]
    pub x: f64,

    #[serde(default)]
    pub y: f64,

    #[serde(default)]
    pub z: f64,

    #[serde(default)]
    pub yaw: f32,

    #[serde(default)]
    pub pitch: f32,

    #[serde(default)]
    pub on_ground: bool,

    #[serde(default)]
    pub data: i32,

    #[serde(default)]
    pub ai: String,

    #[serde(default)]
    pub ai_params: BTreeMap<String, serde_json::Value>,

    #[serde(default = "default_entity_auto_jump")]
    pub auto_jump: bool,

    #[serde(default)]
    pub look_at_players: bool,

    #[serde(default = "default_npc_main_hand_event")]
    pub main_hand_event: String,

    #[serde(default = "default_npc_off_hand_event")]
    pub off_hand_event: String,

    #[serde(default = "default_npc_attack_event")]
    pub attack_event: String,
}

impl Default for Entity {
    fn default() -> Self {
        Self {
            id: String::new(),
            kind: EntityKind::default(),
            entity_type: default_entity_type(),
            name: String::new(),
            display_name: String::new(),
            skin_textures: String::new(),
            skin_signature: String::new(),
            skin_player_id: String::new(),
            x: 0.0,
            y: 64.0,
            z: 0.0,
            yaw: 0.0,
            pitch: 0.0,
            on_ground: true,
            data: 0,
            ai: String::new(),
            ai_params: BTreeMap::new(),
            auto_jump: default_entity_auto_jump(),
            look_at_players: false,
            main_hand_event: default_npc_main_hand_event(),
            off_hand_event: default_npc_off_hand_event(),
            attack_event: default_npc_attack_event(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityKind {
    #[default]
    Entity,
    Npc,
    Hologram,
}

fn default_entity_type() -> String {
    "minecraft:armor_stand".to_string()
}

fn default_entity_auto_jump() -> bool {
    true
}

fn default_npc_main_hand_event() -> String {
    "interact".to_string()
}

fn default_npc_off_hand_event() -> String {
    "interact_off_hand".to_string()
}

fn default_npc_attack_event() -> String {
    "attack".to_string()
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct EntitySpawning {
    #[serde(default)]
    pub enable: bool,

    #[serde(default = "default_entity_spawn_tick_interval_ms")]
    pub tick_interval_ms: u64,

    #[serde(default = "default_entity_ai_tick_interval_ms")]
    pub ai_tick_interval_ms: u64,

    #[serde(default = "default_entity_spawn_global_cap")]
    pub global_cap: usize,

    #[serde(default = "default_entity_spawn_per_dimension_cap")]
    pub per_dimension_cap: usize,

    #[serde(default = "default_entity_spawn_per_type_cap")]
    pub per_type_cap: usize,

    #[serde(default = "default_entity_spawn_max_per_tick")]
    pub max_spawn_per_tick: usize,

    #[serde(default = "default_entity_spawn_player_activation_range")]
    pub player_activation_range: f64,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rules: Vec<EntitySpawnRule>,
}

impl Default for EntitySpawning {
    fn default() -> Self {
        Self {
            enable: false,
            tick_interval_ms: default_entity_spawn_tick_interval_ms(),
            ai_tick_interval_ms: default_entity_ai_tick_interval_ms(),
            global_cap: default_entity_spawn_global_cap(),
            per_dimension_cap: default_entity_spawn_per_dimension_cap(),
            per_type_cap: default_entity_spawn_per_type_cap(),
            max_spawn_per_tick: default_entity_spawn_max_per_tick(),
            player_activation_range: default_entity_spawn_player_activation_range(),
            rules: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct EntitySpawnRule {
    #[serde(default)]
    pub id: String,

    #[serde(default = "default_entity_spawn_rule_enable")]
    pub enable: bool,

    #[serde(default)]
    pub dimension: String,

    #[serde(default = "default_entity_spawn_rule_entity_type")]
    pub entity_type: String,

    #[serde(default = "default_entity_spawn_rule_weight")]
    pub weight: u32,

    #[serde(default = "default_entity_spawn_rule_cap")]
    pub cap: usize,

    #[serde(default)]
    pub tick_interval_ms: u64,

    #[serde(default = "default_entity_spawn_rule_spawn_chance")]
    pub spawn_chance: f64,

    #[serde(default = "default_entity_spawn_rule_min_players")]
    pub min_players: usize,

    #[serde(default)]
    pub max_players: usize,

    #[serde(default)]
    pub activation_range: f64,

    #[serde(default = "default_entity_spawn_rule_require_ground")]
    pub require_ground: bool,

    #[serde(default = "default_entity_spawn_rule_require_air")]
    pub require_air: bool,

    #[serde(default = "default_entity_spawn_rule_position_attempts")]
    pub position_attempts: u32,

    #[serde(default)]
    pub name: String,

    #[serde(default)]
    pub display_name: String,

    #[serde(default)]
    pub ai: String,

    #[serde(default)]
    pub ai_params: BTreeMap<String, serde_json::Value>,

    #[serde(default = "default_entity_auto_jump")]
    pub auto_jump: bool,

    #[serde(default)]
    pub data: i32,

    #[serde(default = "default_entity_spawn_rule_on_ground")]
    pub on_ground: bool,

    #[serde(default = "default_entity_spawn_min_x")]
    pub min_x: f64,

    #[serde(default = "default_entity_spawn_max_x")]
    pub max_x: f64,

    #[serde(default = "default_entity_spawn_min_y")]
    pub min_y: f64,

    #[serde(default = "default_entity_spawn_max_y")]
    pub max_y: f64,

    #[serde(default = "default_entity_spawn_min_z")]
    pub min_z: f64,

    #[serde(default = "default_entity_spawn_max_z")]
    pub max_z: f64,
}

impl Default for EntitySpawnRule {
    fn default() -> Self {
        Self {
            id: String::new(),
            enable: default_entity_spawn_rule_enable(),
            dimension: String::new(),
            entity_type: default_entity_spawn_rule_entity_type(),
            weight: default_entity_spawn_rule_weight(),
            cap: default_entity_spawn_rule_cap(),
            tick_interval_ms: 0,
            spawn_chance: default_entity_spawn_rule_spawn_chance(),
            min_players: default_entity_spawn_rule_min_players(),
            max_players: 0,
            activation_range: 0.0,
            require_ground: default_entity_spawn_rule_require_ground(),
            require_air: default_entity_spawn_rule_require_air(),
            position_attempts: default_entity_spawn_rule_position_attempts(),
            name: String::new(),
            display_name: String::new(),
            ai: String::new(),
            ai_params: BTreeMap::new(),
            auto_jump: default_entity_auto_jump(),
            data: 0,
            on_ground: default_entity_spawn_rule_on_ground(),
            min_x: default_entity_spawn_min_x(),
            max_x: default_entity_spawn_max_x(),
            min_y: default_entity_spawn_min_y(),
            max_y: default_entity_spawn_max_y(),
            min_z: default_entity_spawn_min_z(),
            max_z: default_entity_spawn_max_z(),
        }
    }
}

fn default_entity_spawn_tick_interval_ms() -> u64 {
    1000
}

fn default_entity_ai_tick_interval_ms() -> u64 {
    50
}

fn default_entity_spawn_global_cap() -> usize {
    70
}

fn default_entity_spawn_per_dimension_cap() -> usize {
    70
}

fn default_entity_spawn_per_type_cap() -> usize {
    20
}

fn default_entity_spawn_max_per_tick() -> usize {
    4
}

fn default_entity_spawn_player_activation_range() -> f64 {
    64.0
}

fn default_entity_spawn_rule_enable() -> bool {
    true
}

fn default_entity_spawn_rule_entity_type() -> String {
    "minecraft:zombie".to_string()
}

fn default_entity_spawn_rule_weight() -> u32 {
    100
}

fn default_entity_spawn_rule_cap() -> usize {
    20
}

fn default_entity_spawn_rule_spawn_chance() -> f64 {
    1.0
}

fn default_entity_spawn_rule_min_players() -> usize {
    1
}

fn default_entity_spawn_rule_require_ground() -> bool {
    false
}

fn default_entity_spawn_rule_require_air() -> bool {
    false
}

fn default_entity_spawn_rule_position_attempts() -> u32 {
    8
}

fn default_entity_spawn_rule_on_ground() -> bool {
    true
}

fn default_entity_spawn_min_x() -> f64 {
    -16.0
}

fn default_entity_spawn_max_x() -> f64 {
    16.0
}

fn default_entity_spawn_min_y() -> f64 {
    64.0
}

fn default_entity_spawn_max_y() -> f64 {
    64.0
}

fn default_entity_spawn_min_z() -> f64 {
    -16.0
}

fn default_entity_spawn_max_z() -> f64 {
    16.0
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct ClickDetection {
    #[serde(default = "default_click_detection_enable")]
    pub enable: bool,

    #[serde(default = "default_click_detection_window_ms")]
    pub window_ms: u64,

    #[serde(default = "default_click_detection_max_clicks")]
    pub max_clicks: u32,

    #[serde(default)]
    pub cancel_actions: bool,
}

impl Default for ClickDetection {
    fn default() -> Self {
        Self {
            enable: default_click_detection_enable(),
            window_ms: default_click_detection_window_ms(),
            max_clicks: default_click_detection_max_clicks(),
            cancel_actions: false,
        }
    }
}

fn default_click_detection_enable() -> bool {
    true
}

fn default_click_detection_window_ms() -> u64 {
    1000
}

fn default_click_detection_max_clicks() -> u32 {
    18
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct Gameplay {
    #[serde(default = "default_gameplay_block_updates")]
    pub block_updates: bool,

    #[serde(default = "default_gameplay_crafting_table")]
    pub crafting_table: bool,

    #[serde(default = "default_gameplay_furnace")]
    pub furnace: bool,

    #[serde(default = "default_gameplay_crafting")]
    pub crafting: bool,

    #[serde(default = "default_gameplay_durability")]
    pub durability: bool,

    #[serde(default = "default_gameplay_combat")]
    pub combat: bool,

    #[serde(default = "default_gameplay_oxygen")]
    pub oxygen: bool,

    #[serde(default = "default_gameplay_sounds")]
    pub sounds: bool,

    #[serde(default = "default_gameplay_advancements")]
    pub advancements: bool,

    #[serde(default = "default_gameplay_enchantments")]
    pub enchantments: bool,

    #[serde(default = "default_gameplay_potion_effects")]
    pub potion_effects: bool,

    #[serde(default = "default_gameplay_furnace_tick_ms")]
    pub furnace_tick_ms: u64,

    #[serde(default = "default_gameplay_oxygen_tick_ms")]
    pub oxygen_tick_ms: u64,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub custom_advancements: Vec<CustomAdvancement>,
}

impl Default for Gameplay {
    fn default() -> Self {
        Self {
            block_updates: default_gameplay_block_updates(),
            crafting_table: default_gameplay_crafting_table(),
            furnace: default_gameplay_furnace(),
            crafting: default_gameplay_crafting(),
            durability: default_gameplay_durability(),
            combat: default_gameplay_combat(),
            oxygen: default_gameplay_oxygen(),
            sounds: default_gameplay_sounds(),
            advancements: default_gameplay_advancements(),
            enchantments: default_gameplay_enchantments(),
            potion_effects: default_gameplay_potion_effects(),
            furnace_tick_ms: default_gameplay_furnace_tick_ms(),
            oxygen_tick_ms: default_gameplay_oxygen_tick_ms(),
            custom_advancements: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct CustomAdvancement {
    #[serde(default)]
    pub id: String,

    #[serde(default = "default_custom_advancement_title")]
    pub title: String,

    #[serde(default = "default_custom_advancement_description")]
    pub description: String,

    #[serde(default = "default_custom_advancement_icon")]
    pub icon: String,

    #[serde(default)]
    pub trigger: CustomAdvancementTrigger,

    #[serde(default = "default_custom_advancement_toast")]
    pub toast: bool,
}

impl Default for CustomAdvancement {
    fn default() -> Self {
        Self {
            id: String::new(),
            title: default_custom_advancement_title(),
            description: default_custom_advancement_description(),
            icon: default_custom_advancement_icon(),
            trigger: CustomAdvancementTrigger::default(),
            toast: default_custom_advancement_toast(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CustomAdvancementTrigger {
    #[default]
    Join,
    Craft,
    Smelt,
    Mine,
    Attack,
    Kill,
    EnterWater,
}

fn default_gameplay_block_updates() -> bool {
    true
}

fn default_gameplay_crafting_table() -> bool {
    true
}

fn default_gameplay_furnace() -> bool {
    true
}

fn default_gameplay_crafting() -> bool {
    true
}

fn default_gameplay_durability() -> bool {
    true
}

fn default_gameplay_combat() -> bool {
    true
}

fn default_gameplay_oxygen() -> bool {
    true
}

fn default_gameplay_sounds() -> bool {
    true
}

fn default_gameplay_advancements() -> bool {
    true
}

fn default_gameplay_enchantments() -> bool {
    true
}

fn default_gameplay_potion_effects() -> bool {
    true
}

fn default_gameplay_furnace_tick_ms() -> u64 {
    50
}

fn default_gameplay_oxygen_tick_ms() -> u64 {
    1000
}

fn default_custom_advancement_title() -> String {
    "Qexed".to_string()
}

fn default_custom_advancement_description() -> String {
    "Custom advancement".to_string()
}

fn default_custom_advancement_icon() -> String {
    "minecraft:stone".to_string()
}

fn default_custom_advancement_toast() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct ResourcePack {
    #[serde(default)]
    pub enable: bool,

    #[serde(default)]
    pub source: ResourcePackSource,

    #[serde(default = "default_resource_pack_id")]
    pub id: uuid::Uuid,

    #[serde(default)]
    pub url: String,

    #[serde(default = "default_resource_pack_path")]
    pub path: String,

    #[serde(default = "default_resource_pack_download_bind")]
    pub download_bind: String,

    #[serde(default)]
    pub download_host: String,

    #[serde(default)]
    pub object_storage: ResourcePackObjectStorage,

    #[serde(default)]
    pub hash: String,

    #[serde(default)]
    pub required: bool,

    #[serde(default)]
    pub prompt: String,

    #[serde(default = "default_resource_pack_disconnect_message")]
    pub disconnect_message: String,
}

impl Default for ResourcePack {
    fn default() -> Self {
        Self {
            enable: false,
            source: ResourcePackSource::default(),
            id: default_resource_pack_id(),
            url: String::new(),
            path: default_resource_pack_path(),
            download_bind: default_resource_pack_download_bind(),
            download_host: String::new(),
            object_storage: ResourcePackObjectStorage::default(),
            hash: String::new(),
            required: false,
            prompt: String::new(),
            disconnect_message: default_resource_pack_disconnect_message(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourcePackSource {
    #[default]
    Url,
    Local,
    ObjectStorage,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct ResourcePackObjectStorage {
    #[serde(default)]
    pub provider: ResourcePackObjectStorageProvider,

    #[serde(default)]
    pub public_base_url: String,

    #[serde(default)]
    pub endpoint: String,

    #[serde(default)]
    pub bucket: String,

    #[serde(default = "default_resource_pack_object_key")]
    pub object_key: String,

    #[serde(default)]
    pub force_path_style: bool,
}

impl Default for ResourcePackObjectStorage {
    fn default() -> Self {
        Self {
            provider: ResourcePackObjectStorageProvider::default(),
            public_base_url: String::new(),
            endpoint: String::new(),
            bucket: String::new(),
            object_key: default_resource_pack_object_key(),
            force_path_style: false,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourcePackObjectStorageProvider {
    #[default]
    Generic,
    TencentCos,
    TencentEo,
    HuaweiObs,
    HuaweiCdn,
    AliyunOss,
    AwsS3,
}

fn default_resource_pack_id() -> uuid::Uuid {
    uuid::Uuid::from_u128(0x11111111_2222_3333_4444_555555555555)
}

fn default_resource_pack_path() -> String {
    "resourcepacks/server.zip".to_string()
}

fn default_resource_pack_object_key() -> String {
    "resourcepacks/server.zip".to_string()
}

fn default_resource_pack_download_bind() -> String {
    "0.0.0.0:25566".to_string()
}

fn default_resource_pack_disconnect_message() -> String {
    "This server requires its resource pack.".to_string()
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct PlayerMessages {
    #[serde(default = "default_player_messages_enable")]
    pub enable: bool,

    #[serde(default = "default_chat_rate_limit_window_secs")]
    pub chat_rate_limit_window_secs: u64,

    #[serde(default = "default_chat_rate_limit_max_messages")]
    pub chat_rate_limit_max_messages: u32,

    #[serde(default = "default_chat_max_length")]
    pub chat_max_length: usize,

    #[serde(default = "default_player_join_message")]
    pub join: String,

    #[serde(default = "default_player_leave_message")]
    pub leave: String,
}

impl Default for PlayerMessages {
    fn default() -> Self {
        Self {
            enable: default_player_messages_enable(),
            chat_rate_limit_window_secs: default_chat_rate_limit_window_secs(),
            chat_rate_limit_max_messages: default_chat_rate_limit_max_messages(),
            chat_max_length: default_chat_max_length(),
            join: default_player_join_message(),
            leave: default_player_leave_message(),
        }
    }
}

fn default_player_messages_enable() -> bool {
    true
}

fn default_player_join_message() -> String {
    "{player} joined the server".to_string()
}

fn default_player_leave_message() -> String {
    "{player} left the server".to_string()
}

fn default_chat_rate_limit_window_secs() -> u64 {
    2
}

fn default_chat_rate_limit_max_messages() -> u32 {
    5
}

fn default_chat_max_length() -> usize {
    256
}

fn default_proxy_online_mode() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct PlayerAudit {
    #[serde(default)]
    pub enable: bool,

    #[serde(default)]
    pub storage: PlayerAuditStorage,

    #[serde(default = "default_player_audit_file_path")]
    pub file_path: String,

    #[serde(default)]
    pub events: PlayerAuditEvents,
}

impl Default for PlayerAudit {
    fn default() -> Self {
        Self {
            enable: false,
            storage: PlayerAuditStorage::default(),
            file_path: default_player_audit_file_path(),
            events: PlayerAuditEvents::default(),
        }
    }
}

fn default_player_audit_file_path() -> String {
    "logs/player_audit.log".to_string()
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PlayerAuditStorage {
    #[default]
    File,
    Stdout,
    FileAndStdout,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct PlayerAuditEvents {
    #[serde(default = "default_player_audit_track_block_place")]
    pub block_place: bool,

    #[serde(default = "default_player_audit_track_block_break")]
    pub block_break: bool,

    #[serde(default = "default_player_audit_track_item_switch")]
    pub item_switch: bool,

    #[serde(default = "default_player_audit_track_command")]
    pub command: bool,
}

impl Default for PlayerAuditEvents {
    fn default() -> Self {
        Self {
            block_place: default_player_audit_track_block_place(),
            block_break: default_player_audit_track_block_break(),
            item_switch: default_player_audit_track_item_switch(),
            command: default_player_audit_track_command(),
        }
    }
}

fn default_player_audit_track_block_place() -> bool {
    true
}

fn default_player_audit_track_block_break() -> bool {
    true
}

fn default_player_audit_track_item_switch() -> bool {
    true
}

fn default_player_audit_track_command() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct ContentFilter {
    #[serde(default)]
    pub enable: bool,

    #[serde(default)]
    pub engine: ContentFilterEngine,

    #[serde(default)]
    pub words: Vec<String>,

    #[serde(default)]
    pub knowledge_path: String,

    #[serde(default)]
    pub api_url: String,

    #[serde(default)]
    pub api_token: String,

    #[serde(default = "default_content_filter_replacement")]
    pub replacement: String,

    #[serde(default = "default_content_filter_block_message")]
    pub block_message: String,
}

impl Default for ContentFilter {
    fn default() -> Self {
        Self {
            enable: false,
            engine: ContentFilterEngine::default(),
            words: Vec::new(),
            knowledge_path: String::new(),
            api_url: String::new(),
            api_token: String::new(),
            replacement: default_content_filter_replacement(),
            block_message: default_content_filter_block_message(),
        }
    }
}

fn default_content_filter_replacement() -> String {
    "***".to_string()
}

fn default_content_filter_block_message() -> String {
    "Your message was blocked by the server content filter.".to_string()
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentFilterEngine {
    #[default]
    Fixed,
    Knowledge,
    Api,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Permissions {
    #[serde(default)]
    pub engine: PermissionEngine,

    #[serde(default = "default_permissions_local_path")]
    pub local_path: String,

    #[serde(default = "default_permissions_table_prefix")]
    pub table_prefix: String,

    #[serde(default = "default_permissions_server")]
    pub server: String,

    #[serde(default = "default_permissions_world")]
    pub world: String,

    #[serde(default = "default_permissions_default_group")]
    pub default_group: String,

    #[serde(default = "default_permissions_allow_by_default")]
    pub allow_by_default: bool,

    #[serde(default = "default_permissions_denied_message")]
    pub denied_message: String,

    #[serde(default)]
    pub mysql: MysqlConfig,
}

impl Default for Permissions {
    fn default() -> Self {
        Self {
            engine: PermissionEngine::default(),
            local_path: default_permissions_local_path(),
            table_prefix: default_permissions_table_prefix(),
            server: default_permissions_server(),
            world: default_permissions_world(),
            default_group: default_permissions_default_group(),
            allow_by_default: default_permissions_allow_by_default(),
            denied_message: default_permissions_denied_message(),
            mysql: MysqlConfig {
                username: "luckperms".to_string(),
                password: nanoid::nanoid!(),
                database: "minecraft".to_string(),
                ..MysqlConfig::default()
            },
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionEngine {
    #[default]
    Local,
    LuckpermsMysql,
}

fn default_permissions_local_path() -> String {
    "config/qexed_permissions.toml".to_string()
}

fn default_permissions_table_prefix() -> String {
    "luckperms_".to_string()
}

fn default_permissions_server() -> String {
    "global".to_string()
}

fn default_permissions_world() -> String {
    "global".to_string()
}

fn default_permissions_default_group() -> String {
    "default".to_string()
}

fn default_permissions_allow_by_default() -> bool {
    false
}

fn default_permissions_denied_message() -> String {
    "You do not have permission to use this command.".to_string()
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct PlayerData {
    #[serde(default = "default_player_data_enable")]
    pub enable: bool,

    #[serde(default = "default_player_data_autosave_interval_secs")]
    pub autosave_interval_secs: u64,

    #[serde(default)]
    pub engine: PlayerDataEngine,

    #[serde(default = "default_player_data_collection")]
    pub collection: String,

    #[serde(default = "default_player_data_table")]
    pub table: String,

    #[serde(default)]
    pub mongodb: MongoConfig,

    #[serde(default)]
    pub mysql: MysqlConfig,
}

impl Default for PlayerData {
    fn default() -> Self {
        Self {
            enable: default_player_data_enable(),
            autosave_interval_secs: default_player_data_autosave_interval_secs(),
            engine: PlayerDataEngine::default(),
            collection: default_player_data_collection(),
            table: default_player_data_table(),
            mongodb: MongoConfig {
                username: Some("qexed".to_string()),
                password: Some(nanoid::nanoid!()),
                database: "qexed".to_string(),
                app_name: Some("qexed".to_string()),
                auth_source: Some("admin".to_string()),
                ..MongoConfig::default()
            },
            mysql: MysqlConfig {
                username: "qexed".to_string(),
                password: nanoid::nanoid!(),
                database: "qexed".to_string(),
                ..MysqlConfig::default()
            },
        }
    }
}

fn default_player_data_enable() -> bool {
    true
}

fn default_player_data_autosave_interval_secs() -> u64 {
    300
}

fn default_player_data_collection() -> String {
    "players".to_string()
}

fn default_player_data_table() -> String {
    "qexed_players".to_string()
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PlayerDataEngine {
    #[default]
    Vanilla,
    Mongodb,
    Mysql,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ServerLogLevel {
    Trace,
    Debug,
    #[default]
    Info,
    Warn,
    Error,
    Off,
}

fn default_mojang_cache_path() -> String {
    "cache/mojang".to_string()
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LanDiscovery {
    pub enable: bool,

    pub interval_ms: u64,
}

impl Default for LanDiscovery {
    fn default() -> Self {
        Self {
            enable: false,
            interval_ms: 1500,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct World {
    #[serde(default)]
    pub default_dimension: String,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub worlds: Vec<WorldStorage>,

    #[serde(default = "default_world_path")]
    pub path: String,

    #[serde(default)]
    pub read_only: bool,

    #[serde(default)]
    pub generator: WorldGenerator,

    #[serde(default = "default_world_generator_preset")]
    pub generator_preset: String,

    #[serde(default)]
    pub seed: i64,

    #[serde(default)]
    pub game_mode: GameMode,

    #[serde(default = "default_spawn_protection_radius")]
    pub spawn_protection_radius: i32,

    #[serde(default = "default_world_default_dimension")]
    pub dimension: String,

    #[serde(default = "default_world_dimension_type")]
    pub dimension_type: String,

    pub view_distance: i32,

    #[serde(default = "default_chunk_load_parallelism")]
    pub chunk_load_parallelism: usize,

    #[serde(default = "default_chunk_update_delay_ms")]
    pub chunk_update_delay_ms: u64,

    pub simulation_distance: i32,

    #[serde(default)]
    pub light: LightMode,

    #[serde(default)]
    pub light_algorithm: LightAlgorithm,

    #[serde(default)]
    pub precompiled_chunks: PrecompiledChunks,

    pub spawn: Spawn,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub instances: Vec<WorldInstance>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ore_pits: Vec<WorldOrePit>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub edit_regions: Vec<WorldEditRegion>,
}

impl Default for World {
    fn default() -> Self {
        Self {
            default_dimension: default_world_default_dimension(),
            worlds: vec![WorldStorage::default()],
            path: default_world_path(),
            read_only: false,
            generator: WorldGenerator::default(),
            generator_preset: default_world_generator_preset(),
            seed: 0,
            game_mode: GameMode::default(),
            spawn_protection_radius: default_spawn_protection_radius(),
            dimension: default_world_default_dimension(),
            dimension_type: default_world_dimension_type(),
            view_distance: 3,
            chunk_load_parallelism: default_chunk_load_parallelism(),
            chunk_update_delay_ms: default_chunk_update_delay_ms(),
            simulation_distance: 3,
            light: LightMode::default(),
            light_algorithm: LightAlgorithm::default(),
            precompiled_chunks: PrecompiledChunks::default(),
            spawn: Spawn::default(),
            instances: Vec::new(),
            ore_pits: Vec::new(),
            edit_regions: Vec::new(),
        }
    }
}

impl World {
    pub fn default_play_dimension(&self) -> String {
        let default_dimension = self.default_dimension.trim();
        if !default_dimension.is_empty() {
            return default_dimension.to_string();
        }
        let legacy_dimension = self.dimension.trim();
        if !legacy_dimension.is_empty() {
            return legacy_dimension.to_string();
        }
        self.worlds
            .iter()
            .find_map(|world| {
                let dimension = world.dimension.trim();
                (!dimension.is_empty()).then(|| dimension.to_string())
            })
            .unwrap_or_else(default_world_default_dimension)
    }

    pub fn configured_dimension_names(&self) -> Vec<String> {
        let mut dimensions = Vec::new();
        push_unique_dimension(&mut dimensions, self.default_play_dimension());
        for world in &self.worlds {
            push_unique_dimension(&mut dimensions, world.dimension.trim());
        }
        if self.worlds.is_empty() {
            push_unique_dimension(&mut dimensions, self.dimension.trim());
        }
        for instance in &self.instances {
            push_unique_dimension(&mut dimensions, instance.dimension.trim());
        }
        dimensions
    }

    pub fn dimension_type_for(&self, dimension: &str) -> Option<String> {
        let dimension = dimension.trim();
        if dimension.is_empty() {
            return None;
        }
        for world in &self.worlds {
            if world.dimension.trim() == dimension {
                let dimension_type = world.dimension_type.trim();
                if !dimension_type.is_empty() {
                    return Some(dimension_type.to_string());
                }
            }
        }
        if self.worlds.is_empty() && self.dimension.trim() == dimension {
            let dimension_type = self.dimension_type.trim();
            if !dimension_type.is_empty() {
                return Some(dimension_type.to_string());
            }
        }
        None
    }
}

fn push_unique_dimension(dimensions: &mut Vec<String>, dimension: impl AsRef<str>) {
    let dimension = dimension.as_ref().trim();
    if !dimension.is_empty() && !dimensions.iter().any(|existing| existing == dimension) {
        dimensions.push(dimension.to_string());
    }
}

impl qexed_config::tool::AppConfigTrait for World {
    const PATH: &'static str = "/";
    const NAME: &'static str = "world";
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorldGenerator {
    #[default]
    Empty,
    VanillaFlat,
    VanillaNoise,
}

fn default_world_generator_preset() -> String {
    "minecraft:classic_flat".to_string()
}

fn default_world_path() -> String {
    "world".to_string()
}

fn default_world_default_dimension() -> String {
    "minecraft:overworld".to_string()
}

fn default_world_dimension_type() -> String {
    "minecraft:overworld".to_string()
}

fn default_chunk_load_parallelism() -> usize {
    4
}

fn default_chunk_update_delay_ms() -> u64 {
    1000
}

fn default_spawn_protection_radius() -> i32 {
    16
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct WorldStorage {
    #[serde(default)]
    pub id: String,

    #[serde(default = "default_world_default_dimension")]
    pub dimension: String,

    #[serde(default = "default_world_dimension_type")]
    pub dimension_type: String,

    #[serde(default = "default_world_path")]
    pub path: String,
}

impl Default for WorldStorage {
    fn default() -> Self {
        Self {
            id: "overworld".to_string(),
            dimension: default_world_default_dimension(),
            dimension_type: default_world_dimension_type(),
            path: default_world_path(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct WorldInstance {
    #[serde(default)]
    pub id: String,

    #[serde(default)]
    pub dimension: String,

    #[serde(default = "default_world_instance_source_dimension")]
    pub source_dimension: String,

    #[serde(default)]
    pub path: String,

    #[serde(default = "default_world_instance_copy_on_write")]
    pub copy_on_write: bool,
}

impl Default for WorldInstance {
    fn default() -> Self {
        Self {
            id: String::new(),
            dimension: String::new(),
            source_dimension: default_world_instance_source_dimension(),
            path: String::new(),
            copy_on_write: default_world_instance_copy_on_write(),
        }
    }
}

fn default_world_instance_source_dimension() -> String {
    "minecraft:overworld".to_string()
}

fn default_world_instance_copy_on_write() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct WorldOrePit {
    #[serde(default)]
    pub id: String,

    #[serde(default = "default_world_ore_pit_enable")]
    pub enable: bool,

    #[serde(default = "default_world_default_dimension")]
    pub dimension: String,

    pub min_x: i32,

    pub max_x: i32,

    pub min_y: i32,

    pub max_y: i32,

    pub min_z: i32,

    pub max_z: i32,

    #[serde(default = "default_world_ore_pit_tick_interval_ms")]
    pub tick_interval_ms: u64,

    #[serde(default = "default_world_ore_pit_initial_refill")]
    pub initial_refill: bool,

    #[serde(default = "default_world_ore_pit_max_blocks_per_tick")]
    pub max_blocks_per_tick: usize,

    #[serde(default)]
    pub teleport_players_to_surface_on_refill: bool,

    #[serde(default = "default_world_ore_pit_replace_air")]
    pub replace_air: bool,

    #[serde(default = "default_world_ore_pit_replace_generated")]
    pub replace_generated: bool,

    #[serde(default)]
    pub only_break_generated: bool,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocks: Vec<WorldOrePitBlock>,
}

impl Default for WorldOrePit {
    fn default() -> Self {
        Self {
            id: String::new(),
            enable: default_world_ore_pit_enable(),
            dimension: default_world_default_dimension(),
            min_x: 0,
            max_x: 0,
            min_y: 0,
            max_y: 0,
            min_z: 0,
            max_z: 0,
            tick_interval_ms: default_world_ore_pit_tick_interval_ms(),
            initial_refill: default_world_ore_pit_initial_refill(),
            max_blocks_per_tick: default_world_ore_pit_max_blocks_per_tick(),
            teleport_players_to_surface_on_refill: false,
            replace_air: default_world_ore_pit_replace_air(),
            replace_generated: default_world_ore_pit_replace_generated(),
            only_break_generated: false,
            blocks: Vec::new(),
        }
    }
}

impl WorldOrePit {
    pub fn contains(&self, dimension: &str, x: i32, y: i32, z: i32) -> bool {
        self.dimension.trim() == dimension.trim()
            && contains_axis(x, self.min_x, self.max_x)
            && contains_axis(y, self.min_y, self.max_y)
            && contains_axis(z, self.min_z, self.max_z)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct WorldOrePitBlock {
    #[serde(default = "default_world_ore_pit_block")]
    pub block: String,

    #[serde(default = "default_world_ore_pit_block_weight")]
    pub weight: u32,
}

impl Default for WorldOrePitBlock {
    fn default() -> Self {
        Self {
            block: default_world_ore_pit_block(),
            weight: default_world_ore_pit_block_weight(),
        }
    }
}

fn default_world_ore_pit_enable() -> bool {
    true
}

fn default_world_ore_pit_tick_interval_ms() -> u64 {
    300_000
}

fn default_world_ore_pit_initial_refill() -> bool {
    true
}

fn default_world_ore_pit_max_blocks_per_tick() -> usize {
    64
}

fn default_world_ore_pit_replace_air() -> bool {
    true
}

fn default_world_ore_pit_replace_generated() -> bool {
    true
}

fn default_world_ore_pit_block() -> String {
    "minecraft:stone".to_string()
}

fn default_world_ore_pit_block_weight() -> u32 {
    1
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct WorldEditRegion {
    #[serde(default)]
    pub id: String,

    #[serde(default = "default_world_default_dimension")]
    pub dimension: String,

    pub min_x: i32,

    pub max_x: i32,

    pub min_y: i32,

    pub max_y: i32,

    pub min_z: i32,

    pub max_z: i32,

    #[serde(default = "default_world_edit_region_allow_player_break")]
    pub allow_player_break: bool,

    #[serde(default = "default_world_edit_region_allow_player_place")]
    pub allow_player_place: bool,

    #[serde(default = "default_world_edit_region_allow_plugin_write")]
    pub allow_plugin_write: bool,

    #[serde(default = "default_world_edit_region_runtime_only")]
    pub runtime_only: bool,
}

impl Default for WorldEditRegion {
    fn default() -> Self {
        Self {
            id: String::new(),
            dimension: default_world_default_dimension(),
            min_x: 0,
            max_x: 0,
            min_y: 0,
            max_y: 0,
            min_z: 0,
            max_z: 0,
            allow_player_break: default_world_edit_region_allow_player_break(),
            allow_player_place: default_world_edit_region_allow_player_place(),
            allow_plugin_write: default_world_edit_region_allow_plugin_write(),
            runtime_only: default_world_edit_region_runtime_only(),
        }
    }
}

impl WorldEditRegion {
    pub fn contains(&self, dimension: &str, x: i32, y: i32, z: i32) -> bool {
        self.dimension.trim() == dimension.trim()
            && contains_axis(x, self.min_x, self.max_x)
            && contains_axis(y, self.min_y, self.max_y)
            && contains_axis(z, self.min_z, self.max_z)
    }
}

fn contains_axis(value: i32, first: i32, second: i32) -> bool {
    let min = first.min(second);
    let max = first.max(second);
    (min..=max).contains(&value)
}

fn default_world_edit_region_allow_player_break() -> bool {
    true
}

fn default_world_edit_region_allow_player_place() -> bool {
    true
}

fn default_world_edit_region_allow_plugin_write() -> bool {
    true
}

fn default_world_edit_region_runtime_only() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct PrecompiledChunks {
    #[serde(default)]
    pub enable: bool,

    #[serde(default = "default_precompiled_chunks_light")]
    pub light: bool,

    #[serde(default = "default_precompiled_chunks_max_cached_packets")]
    pub max_cached_packets: usize,

    #[serde(default = "default_precompiled_chunks_max_cached_packet_bytes")]
    pub max_cached_packet_bytes: usize,

    #[serde(default = "default_precompiled_chunks_block_state_cache_limit")]
    pub block_state_cache_limit: usize,
}

impl Default for PrecompiledChunks {
    fn default() -> Self {
        Self {
            enable: false,
            light: default_precompiled_chunks_light(),
            max_cached_packets: default_precompiled_chunks_max_cached_packets(),
            max_cached_packet_bytes: default_precompiled_chunks_max_cached_packet_bytes(),
            block_state_cache_limit: default_precompiled_chunks_block_state_cache_limit(),
        }
    }
}

fn default_precompiled_chunks_light() -> bool {
    true
}

fn default_precompiled_chunks_max_cached_packets() -> usize {
    256
}

fn default_precompiled_chunks_max_cached_packet_bytes() -> usize {
    16 * 1024 * 1024
}

fn default_precompiled_chunks_block_state_cache_limit() -> usize {
    65_536
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GameMode {
    #[default]
    Survival,
    Creative,
    Adventure,
    Spectator,
}

impl GameMode {
    pub fn protocol_id(self) -> u8 {
        match self {
            Self::Survival => 0,
            Self::Creative => 1,
            Self::Adventure => 2,
            Self::Spectator => 3,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum LightMode {
    Static,
    Dynamic,
    Fixed(u8),
}

impl Default for LightMode {
    fn default() -> Self {
        Self::Static
    }
}

impl Serialize for LightMode {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Self::Static => serializer.serialize_str("static"),
            Self::Dynamic => serializer.serialize_str("dynamic"),
            Self::Fixed(level) => serializer.serialize_u8(*level),
        }
    }
}

impl<'de> Deserialize<'de> for LightMode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = LightMode;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str(r#""static", "dynamic", or an integer brightness from 0 to 15"#)
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                match value.trim().to_ascii_lowercase().as_str() {
                    "static" => Ok(LightMode::Static),
                    "dynamic" => Ok(LightMode::Dynamic),
                    other => other
                        .parse::<u8>()
                        .map_err(|_| E::custom(format!("unknown light mode: {value}")))
                        .and_then(fixed_light_mode),
                }
            }

            fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                let value = u8::try_from(value)
                    .map_err(|_| E::custom(format!("brightness out of range 0..=15: {value}")))?;
                fixed_light_mode(value)
            }

            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                let value = u8::try_from(value)
                    .map_err(|_| E::custom(format!("brightness out of range 0..=15: {value}")))?;
                fixed_light_mode(value)
            }
        }

        fn fixed_light_mode<E>(value: u8) -> Result<LightMode, E>
        where
            E: de::Error,
        {
            if value <= 15 {
                Ok(LightMode::Fixed(value))
            } else {
                Err(E::custom(format!(
                    "brightness out of range 0..=15: {value}"
                )))
            }
        }

        deserializer.deserialize_any(Visitor)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LightAlgorithm {
    #[default]
    Fast,
    RayTrace,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Spawn {
    pub x: f64,

    pub y: f64,

    pub z: f64,

    pub yaw: f32,

    pub pitch: f32,
}

impl Default for Spawn {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            yaw: 0.0,
            pitch: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq)]
pub enum ForwardingMode {
    Default,
    QTunnel,
    Victory,
    Velocity,
    BungeeCord,
    None,
}

impl Default for ForwardingMode {
    fn default() -> Self {
        ForwardingMode::Default
    }
}

impl std::fmt::Display for ForwardingMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ForwardingMode::Default => write!(f, "Default"),
            ForwardingMode::QTunnel => write!(f, "QTunnel"),
            ForwardingMode::Victory => write!(f, "Victory"),
            ForwardingMode::Velocity => write!(f, "Velocity"),
            ForwardingMode::BungeeCord => write!(f, "BungeeCord"),
            ForwardingMode::None => write!(f, "None"),
        }
    }
}

impl std::str::FromStr for ForwardingMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "default" => Ok(ForwardingMode::Default),
            "qtunnel" => Ok(ForwardingMode::QTunnel),
            "victory" => Ok(ForwardingMode::Victory),
            "velocity" => Ok(ForwardingMode::Velocity),
            "bungeecord" => Ok(ForwardingMode::BungeeCord),
            "none" => Ok(ForwardingMode::None),
            _ => Err(format!("鏈煡鐨勮浆鍙戞ā寮? {}", s)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ContentFilter, ContentFilterEngine, CustomAdvancementTrigger, Entities, EntityKind,
        ForwardingMode, GameMode, Gameplay, LightAlgorithm, LightMode, LobbyActionKind,
        LobbyBossBarColor, LobbyBossBarOverlay, MenuActionKind, Npcs, PermissionEngine,
        Permissions, PlayerAudit, PlayerAuditStorage, PlayerData, PlayerDataEngine, PlayerMessages,
        PrecompiledChunks, ResourcePack, ResourcePackObjectStorageProvider, ResourcePackSource,
        Server, World, WorldGenerator,
    };

    #[test]
    fn parses_velocity_forwarding_mode() {
        #[derive(serde::Deserialize)]
        struct ProxyConfig {
            proxy_protocol: ForwardingMode,
        }

        let config: ProxyConfig = toml::from_str(r#"proxy_protocol = "Velocity""#).unwrap();
        assert_eq!(config.proxy_protocol, ForwardingMode::Velocity);
        assert_eq!(
            "velocity".parse::<ForwardingMode>().unwrap(),
            ForwardingMode::Velocity
        );
    }

    #[test]
    fn parses_world_light_string_modes() {
        let static_world: World = toml::from_str(
            r#"
path = "world"
dimension = "minecraft:overworld"
dimension_type = "minecraft:overworld"
view_distance = 3
chunk_load_parallelism = 4
simulation_distance = 3
light = "static"
game_mode = "creative"
spawn_protection_radius = 0

[spawn]
x = 0.0
y = 0.0
z = 0.0
yaw = 0.0
pitch = 0.0
"#,
        )
        .unwrap();
        assert_eq!(static_world.light, LightMode::Static);
        assert_eq!(static_world.game_mode, GameMode::Creative);
        assert_eq!(static_world.spawn_protection_radius, 0);

        let dynamic_world: World = toml::from_str(
            r#"
path = "world"
dimension = "minecraft:overworld"
dimension_type = "minecraft:overworld"
view_distance = 3
chunk_load_parallelism = 4
simulation_distance = 3
light = "dynamic"

[spawn]
x = 0.0
y = 0.0
z = 0.0
yaw = 0.0
pitch = 0.0
"#,
        )
        .unwrap();
        assert_eq!(dynamic_world.light, LightMode::Dynamic);
        assert_eq!(dynamic_world.game_mode, GameMode::Survival);
        assert_eq!(dynamic_world.spawn_protection_radius, 16);
    }

    #[test]
    fn parses_world_light_fixed_brightness() {
        let world: World = toml::from_str(
            r#"
path = "world"
dimension = "minecraft:overworld"
dimension_type = "minecraft:overworld"
view_distance = 3
chunk_load_parallelism = 4
simulation_distance = 3
light = 12

[spawn]
x = 0.0
y = 0.0
z = 0.0
yaw = 0.0
pitch = 0.0
"#,
        )
        .unwrap();
        assert_eq!(world.light, LightMode::Fixed(12));
    }

    #[test]
    fn parses_world_light_algorithm() {
        let world: World = toml::from_str(
            r#"
path = "world"
dimension = "minecraft:overworld"
dimension_type = "minecraft:overworld"
view_distance = 3
chunk_load_parallelism = 6
simulation_distance = 3
light = "static"
light_algorithm = "ray_trace"

[spawn]
x = 0.0
y = 0.0
z = 0.0
yaw = 0.0
pitch = 0.0
"#,
        )
        .unwrap();
        assert_eq!(world.light_algorithm, LightAlgorithm::RayTrace);
        assert_eq!(world.chunk_load_parallelism, 6);
    }

    #[test]
    fn parses_world_generator_settings() {
        let world: World = toml::from_str(
            r#"
path = "world"
read_only = true
generator = "vanilla_flat"
generator_preset = "minecraft:classic_flat"
seed = 12345
dimension = "minecraft:overworld"
dimension_type = "minecraft:overworld"
view_distance = 3
chunk_load_parallelism = 6
simulation_distance = 3
light = "static"
light_algorithm = "fast"

[precompiled_chunks]
enable = true
light = false
max_cached_packets = 128
max_cached_packet_bytes = 8388608
block_state_cache_limit = 4096

[spawn]
x = 0.0
y = 0.0
z = 0.0
yaw = 0.0
pitch = 0.0
"#,
        )
        .unwrap();

        assert!(world.read_only);
        assert_eq!(world.generator, WorldGenerator::VanillaFlat);
        assert_eq!(world.generator_preset, "minecraft:classic_flat");
        assert_eq!(world.seed, 12345);
        assert_eq!(
            world.precompiled_chunks,
            PrecompiledChunks {
                enable: true,
                light: false,
                max_cached_packets: 128,
                max_cached_packet_bytes: 8_388_608,
                block_state_cache_limit: 4096,
            }
        );
    }

    #[test]
    fn parses_multi_world_storage_without_required_overworld() {
        let world: World = toml::from_str(
            r#"
default_dimension = "qexed:mine_a"
path = "worlds"
generator = "vanilla_noise"
generator_preset = "minecraft:overworld"
seed = 20260530
game_mode = "creative"
spawn_protection_radius = 0
dimension = "qexed:mine_a"
dimension_type = "minecraft:overworld"
view_distance = 4
chunk_load_parallelism = 4
simulation_distance = 4
light = "static"
light_algorithm = "fast"

[spawn]
x = 8.5
y = 80.0
z = 8.5
yaw = 180.0
pitch = 0.0

[[worlds]]
id = "mine_template"
dimension = "qexed:mine_template"
dimension_type = "minecraft:overworld"
path = "worlds/mine_template"

[[instances]]
id = "mine_a"
dimension = "qexed:mine_a"
source_dimension = "qexed:mine_template"
path = "worlds/mine_a"
copy_on_write = true
"#,
        )
        .unwrap();

        assert_eq!(world.default_play_dimension(), "qexed:mine_a");
        assert_eq!(
            world.configured_dimension_names(),
            vec![
                "qexed:mine_a".to_string(),
                "qexed:mine_template".to_string()
            ]
        );
        assert_eq!(
            world.dimension_type_for("qexed:mine_template").as_deref(),
            Some("minecraft:overworld")
        );
    }

    #[test]
    fn parses_world_edit_regions() {
        let world: World = toml::from_str(
            r#"
path = "world"
read_only = true
dimension = "minecraft:overworld"
dimension_type = "minecraft:overworld"
view_distance = 3
chunk_load_parallelism = 4
simulation_distance = 3
light = "static"

[spawn]
x = 0.0
y = 64.0
z = 0.0
yaw = 0.0
pitch = 0.0

[[edit_regions]]
id = "mine_a"
dimension = "minecraft:overworld"
min_x = -32
max_x = -1
min_y = 8
max_y = 16
min_z = -32
max_z = -1
allow_player_break = true
allow_player_place = false
allow_plugin_write = true
runtime_only = true
"#,
        )
        .unwrap();

        assert_eq!(world.edit_regions.len(), 1);
        let region = &world.edit_regions[0];
        assert_eq!(region.id, "mine_a");
        assert!(region.allow_player_break);
        assert!(!region.allow_player_place);
        assert!(region.allow_plugin_write);
        assert!(region.runtime_only);
        assert!(region.contains("minecraft:overworld", -32, 8, -32));
        assert!(region.contains("minecraft:overworld", -1, 16, -1));
        assert!(!region.contains("minecraft:overworld", 0, 16, -1));
    }

    #[test]
    fn parses_world_ore_pits() {
        let world: World = toml::from_str(
            r#"
path = "world"
read_only = true
dimension = "minecraft:overworld"
dimension_type = "minecraft:overworld"
view_distance = 3
chunk_load_parallelism = 4
simulation_distance = 3
light = "static"

[spawn]
x = 0.0
y = 64.0
z = 0.0
yaw = 0.0
pitch = 0.0

[[ore_pits]]
id = "mine_a"
dimension = "qexed:mine_a"
min_x = 36
max_x = 54
min_y = -50
max_y = -30
min_z = 6
max_z = 24
tick_interval_ms = 500
initial_refill = true
max_blocks_per_tick = 512
teleport_players_to_surface_on_refill = true
replace_air = true
replace_generated = true
only_break_generated = true

[[ore_pits.blocks]]
block = "minecraft:stone"
weight = 60

[[ore_pits.blocks]]
block = "minecraft:diamond_ore"
weight = 1
"#,
        )
        .unwrap();

        assert_eq!(world.ore_pits.len(), 1);
        let pit = &world.ore_pits[0];
        assert_eq!(pit.id, "mine_a");
        assert_eq!(pit.dimension, "qexed:mine_a");
        assert!(pit.initial_refill);
        assert_eq!(pit.max_blocks_per_tick, 512);
        assert!(pit.teleport_players_to_surface_on_refill);
        assert!(pit.only_break_generated);
        assert_eq!(pit.blocks.len(), 2);
        assert!(pit.contains("qexed:mine_a", 36, -50, 6));
        assert!(!pit.contains("qexed:mine_b", 36, -50, 6));
    }

    #[test]
    fn parses_player_data_settings() {
        let player_data: PlayerData = toml::from_str(
            r#"
enable = false
autosave_interval_secs = 120
engine = "mysql"
collection = "qexed_players"
table = "qexed_player_data"

[mongodb]
host = "127.0.0.1"
database = "qexed_player_test"

[mysql]
host = "127.0.0.1"
username = "qexed"
password = "qexed"
database = "qexed_player_test"
"#,
        )
        .unwrap();

        assert!(!player_data.enable);
        assert_eq!(player_data.autosave_interval_secs, 120);
        assert_eq!(player_data.engine, PlayerDataEngine::Mysql);
        assert_eq!(player_data.collection, "qexed_players");
        assert_eq!(player_data.table, "qexed_player_data");
        assert_eq!(player_data.mongodb.database, "qexed_player_test");
        assert_eq!(player_data.mysql.username, "qexed");
    }

    #[test]
    fn parses_player_message_settings() {
        let player_messages: PlayerMessages = toml::from_str(
            r#"
enable = true
chat_rate_limit_window_secs = 3
chat_rate_limit_max_messages = 7
chat_max_length = 128
join = "{player} joined"
leave = "{player} left"
"#,
        )
        .unwrap();

        assert!(player_messages.enable);
        assert_eq!(player_messages.chat_rate_limit_window_secs, 3);
        assert_eq!(player_messages.chat_rate_limit_max_messages, 7);
        assert_eq!(player_messages.chat_max_length, 128);
        assert_eq!(player_messages.join, "{player} joined");
        assert_eq!(player_messages.leave, "{player} left");
    }

    #[test]
    fn parses_gameplay_settings() {
        let gameplay: Gameplay = toml::from_str(
            r#"
block_updates = false
crafting_table = true
furnace = true
crafting = true
durability = true
combat = true
oxygen = true
sounds = true
advancements = true
enchantments = true
potion_effects = true
furnace_tick_ms = 100
oxygen_tick_ms = 500

[[custom_advancements]]
id = "qexed:first_mine"
title = "First Mine"
description = "Break a block"
icon = "minecraft:iron_pickaxe"
trigger = "mine"
toast = true
"#,
        )
        .unwrap();

        assert!(!gameplay.block_updates);
        assert!(gameplay.crafting_table);
        assert_eq!(gameplay.furnace_tick_ms, 100);
        assert_eq!(gameplay.oxygen_tick_ms, 500);
        assert_eq!(gameplay.custom_advancements.len(), 1);
        assert_eq!(
            gameplay.custom_advancements[0].trigger,
            CustomAdvancementTrigger::Mine
        );
    }

    #[test]
    fn parses_player_audit_settings() {
        let audit: PlayerAudit = toml::from_str(
            r#"
enable = true
storage = "file_and_stdout"
file_path = "logs/audit/player-events.log"

[events]
block_place = true
block_break = true
item_switch = true
command = false
"#,
        )
        .unwrap();

        assert!(audit.enable);
        assert_eq!(audit.storage, PlayerAuditStorage::FileAndStdout);
        assert_eq!(audit.file_path, "logs/audit/player-events.log");
        assert!(audit.events.block_place);
        assert!(audit.events.block_break);
        assert!(audit.events.item_switch);
        assert!(!audit.events.command);
    }

    #[test]
    fn parses_content_filter_settings() {
        let content_filter: ContentFilter = toml::from_str(
            r#"
enable = true
engine = "knowledge"
words = ["bad"]
knowledge_path = "config/sensitive_words.txt"
replacement = "***"
block_message = "blocked"
"#,
        )
        .unwrap();

        assert!(content_filter.enable);
        assert_eq!(content_filter.engine, ContentFilterEngine::Knowledge);
        assert_eq!(content_filter.words, vec!["bad"]);
        assert_eq!(content_filter.knowledge_path, "config/sensitive_words.txt");
    }

    #[test]
    fn parses_permission_settings() {
        let permissions: Permissions = toml::from_str(
            r#"
engine = "luckperms_mysql"
local_path = "config/local_permissions.toml"
table_prefix = "luckperms_"
server = "survival"
world = "world"
default_group = "member"
allow_by_default = false
denied_message = "denied"

[mysql]
ip = "127.0.0.1"
username = "luckperms"
password = "secret"
database = "minecraft"
"#,
        )
        .unwrap();

        assert_eq!(permissions.engine, PermissionEngine::LuckpermsMysql);
        assert_eq!(permissions.local_path, "config/local_permissions.toml");
        assert_eq!(permissions.table_prefix, "luckperms_");
        assert_eq!(permissions.server, "survival");
        assert_eq!(permissions.world, "world");
        assert_eq!(permissions.default_group, "member");
        assert!(!permissions.allow_by_default);
        assert_eq!(permissions.denied_message, "denied");
        assert_eq!(permissions.mysql.username, "luckperms");
    }

    #[test]
    fn parses_resource_pack_settings() {
        let resource_pack: ResourcePack = toml::from_str(
            r#"
enable = true
source = "local"
id = "00112233-4455-6677-8899-aabbccddeeff"
url = "https://example.com/qexed.zip"
path = "resourcepacks/test.zip"
download_bind = "127.0.0.1:25566"
download_host = "example.org"
hash = "0123456789abcdef0123456789abcdef01234567"
required = true
prompt = "Install server resources"
disconnect_message = "Resource pack required"

[object_storage]
provider = "tencent_eo"
public_base_url = "https://packs.example.com"
endpoint = "cos.ap-guangzhou.myqcloud.com"
bucket = "qexed-1250000000"
object_key = "minecraft/server.zip"
force_path_style = false
"#,
        )
        .unwrap();

        assert!(resource_pack.enable);
        assert_eq!(
            resource_pack.id,
            uuid::Uuid::from_u128(0x00112233_4455_6677_8899_aabbccddeeff)
        );
        assert_eq!(resource_pack.source, ResourcePackSource::Local);
        assert_eq!(resource_pack.url, "https://example.com/qexed.zip");
        assert_eq!(resource_pack.path, "resourcepacks/test.zip");
        assert_eq!(resource_pack.download_bind, "127.0.0.1:25566");
        assert_eq!(resource_pack.download_host, "example.org");
        assert_eq!(
            resource_pack.object_storage.provider,
            ResourcePackObjectStorageProvider::TencentEo
        );
        assert_eq!(
            resource_pack.object_storage.public_base_url,
            "https://packs.example.com"
        );
        assert_eq!(
            resource_pack.object_storage.endpoint,
            "cos.ap-guangzhou.myqcloud.com"
        );
        assert_eq!(resource_pack.object_storage.bucket, "qexed-1250000000");
        assert_eq!(
            resource_pack.object_storage.object_key,
            "minecraft/server.zip"
        );
        assert!(resource_pack.required);
        assert_eq!(resource_pack.prompt, "Install server resources");
    }

    #[test]
    fn parses_static_entity_settings() {
        let server: Server = toml::from_str(
            r#"
ip = "0.0.0.0:25565"
online = false
max_player = -1
display_players = true
online_mode = false
network_compression_threshold = 256
max_port_connections = 65535
rate_limit_window_secs = 60
rate_limit_max_attempts = 6
motd = ["Welcome"]
code_of_conduct = false
favicon = ""

[world]
path = "world"
dimension = "minecraft:overworld"
dimension_type = "minecraft:overworld"
view_distance = 3
chunk_load_parallelism = 4
simulation_distance = 3
light = "static"
light_algorithm = "fast"

[world.spawn]
x = 0.0
y = 64.0
z = 0.0
yaw = 0.0
pitch = 0.0

[entities]
enable = true
dimension = "minecraft:overworld"

[[entities.list]]
id = "spawn-guide"
kind = "npc"
name = "Guide"
skin_player_id = "Notch"
x = 1.0
y = 65.0
z = 2.0
yaw = 90.0
pitch = 0.0
on_ground = true

[[entities.list]]
id = "marker"
kind = "entity"
entity_type = "minecraft:armor_stand"
x = 3.0
y = 64.0
z = 4.0

[[entities.list]]
id = "welcome-title"
kind = "hologram"
name = "Welcome"
x = 0.0
y = 67.0
z = 0.0
"#,
        )
        .unwrap();

        assert!(server.entities.enable);
        assert_eq!(server.entities.list.len(), 3);
        assert_eq!(server.entities.list[0].kind, EntityKind::Npc);
        assert_eq!(server.entities.list[0].name, "Guide");
        assert_eq!(server.entities.list[0].skin_player_id, "Notch");
        assert_eq!(server.entities.list[1].kind, EntityKind::Entity);
        assert_eq!(server.entities.list[1].entity_type, "minecraft:armor_stand");
        assert_eq!(server.entities.list[2].kind, EntityKind::Hologram);
        assert_eq!(server.entities.list[2].name, "Welcome");
    }

    #[test]
    fn parses_entity_spawning_settings() {
        let entities: Entities = toml::from_str(
            r#"
enable = true
dimension = "qexed:mine_a"

[spawning]
enable = true
tick_interval_ms = 500
ai_tick_interval_ms = 100
global_cap = 30
per_dimension_cap = 20
per_type_cap = 8
max_spawn_per_tick = 3
player_activation_range = 48.0

[[spawning.rules]]
id = "mine_zombies"
dimension = "qexed:mine_a"
entity_type = "minecraft:zombie"
weight = 80
cap = 6
tick_interval_ms = 5000
spawn_chance = 0.5
min_players = 1
max_players = 8
activation_range = 24.0
require_ground = true
require_air = true
position_attempts = 12
name = "Mine Zombie"
display_name = "{\"text\":\"Mine Zombie\"}"
ai = "follow_nearest_player"
auto_jump = true
min_x = -8.0
max_x = 8.0
min_y = 64.0
max_y = 64.0
min_z = -8.0
max_z = 8.0

[spawning.rules.ai_params]
iq = 114514
profile = "aggressive"
"#,
        )
        .unwrap();

        assert!(entities.spawning.enable);
        assert_eq!(entities.spawning.tick_interval_ms, 500);
        assert_eq!(entities.spawning.ai_tick_interval_ms, 100);
        assert_eq!(entities.spawning.global_cap, 30);
        assert_eq!(entities.spawning.per_dimension_cap, 20);
        assert_eq!(entities.spawning.per_type_cap, 8);
        assert_eq!(entities.spawning.max_spawn_per_tick, 3);
        assert_eq!(entities.spawning.player_activation_range, 48.0);
        assert_eq!(entities.spawning.rules.len(), 1);
        assert_eq!(entities.spawning.rules[0].id, "mine_zombies");
        assert_eq!(entities.spawning.rules[0].entity_type, "minecraft:zombie");
        assert_eq!(entities.spawning.rules[0].ai, "follow_nearest_player");
        assert!(entities.spawning.rules[0].auto_jump);
        assert_eq!(entities.spawning.rules[0].tick_interval_ms, 5000);
        assert_eq!(entities.spawning.rules[0].spawn_chance, 0.5);
        assert_eq!(entities.spawning.rules[0].min_players, 1);
        assert_eq!(entities.spawning.rules[0].max_players, 8);
        assert_eq!(entities.spawning.rules[0].activation_range, 24.0);
        assert!(entities.spawning.rules[0].require_ground);
        assert!(entities.spawning.rules[0].require_air);
        assert_eq!(entities.spawning.rules[0].position_attempts, 12);
        assert_eq!(
            entities.spawning.rules[0].ai_params["iq"].as_i64(),
            Some(114514)
        );
        assert_eq!(
            entities.spawning.rules[0].ai_params["profile"].as_str(),
            Some("aggressive")
        );
    }

    #[test]
    fn parses_npc_settings() {
        let npcs: Npcs = toml::from_str(
            r#"
enable = true
dimension = "minecraft:overworld"

[[list]]
id = "survival_npc"
name = "sv1"
display_name = "鍘熺増鐢熷瓨"
entity_type = "minecraft:zombie"
skin_player_id = "MHF_Grass"
x = -14.5
y = 9.0
z = -17.5
yaw = 180.0
pitch = 0.0
on_ground = true
look_at_players = true

[list.actions.main_hand]
kind = "transfer"
target = "survival_1"
message = "姝ｅ湪浼犻€佸埌鍘熺増鐢熷瓨..."
"#,
        )
        .unwrap();

        assert!(npcs.enable);
        assert_eq!(npcs.list.len(), 1);
        assert_eq!(npcs.list[0].id, "survival_npc");
        assert_eq!(npcs.list[0].entity_type, "minecraft:zombie");
        assert_eq!(npcs.list[0].skin_player_id, "MHF_Grass");
        assert!(npcs.list[0].look_at_players);
        assert_eq!(
            npcs.list[0].actions.main_hand.kind,
            MenuActionKind::Transfer
        );
        assert_eq!(npcs.list[0].actions.main_hand.target, "survival_1");
    }

    #[test]
    fn parses_scoreboard_settings() {
        let server: Server = toml::from_str(
            r#"
ip = "0.0.0.0:25565"
online = false
max_player = -1
display_players = true
online_mode = false
network_compression_threshold = 256
max_port_connections = 65535
rate_limit_window_secs = 60
rate_limit_max_attempts = 6
motd = ["Welcome"]
code_of_conduct = false
favicon = ""

[world]
path = "world"
dimension = "minecraft:overworld"
dimension_type = "minecraft:overworld"
view_distance = 3
chunk_load_parallelism = 4
simulation_distance = 3
light = "static"
light_algorithm = "fast"

[world.spawn]
x = 0.0
y = 64.0
z = 0.0
yaw = 0.0
pitch = 0.0

[scoreboard]
enable = true
objective = "qexed"
title = "Qexed"
lines = ["online", "world"]
"#,
        )
        .unwrap();

        assert!(server.scoreboard.enable);
        assert_eq!(server.scoreboard.objective, "qexed");
        assert_eq!(server.scoreboard.title, "Qexed");
        assert_eq!(server.scoreboard.lines, vec!["online", "world"]);
    }

    #[test]
    fn parses_lobby_settings() {
        let server: Server = toml::from_str(
            r#"
ip = "0.0.0.0:25565"
online = false
max_player = -1
display_players = true
online_mode = false
network_compression_threshold = 256
max_port_connections = 65535
rate_limit_window_secs = 60
rate_limit_max_attempts = 6
motd = ["Welcome"]
code_of_conduct = false
favicon = ""

[world]
path = "world"
dimension = "minecraft:overworld"
dimension_type = "minecraft:overworld"
view_distance = 3
chunk_load_parallelism = 4
simulation_distance = 3
light = "static"
light_algorithm = "fast"

[world.spawn]
x = 0.0
y = 64.0
z = 0.0
yaw = 0.0
pitch = 0.0

[lobby]
enable = true
protect_world = true
menu_title = "Games"
menu_rows = 3

[lobby.navigator]
enable = true
slot = 4
item = "minecraft:compass"
name = "Games"

[[lobby.menu_items]]
slot = 13
item = "minecraft:diamond"
unknown_item = "minecraft:clock"
offline_item = "minecraft:red_wool"
disabled_item = "minecraft:gray_wool"
maintenance_item = "minecraft:orange_wool"
name = "Survival"
lore = ["Status: {status_label}", "{status_description}"]

[lobby.menu_items.action]
kind = "transfer"
target = "survival"
message = "Connecting"

[lobby.broadcast]
enable = true
interval_secs = 30
messages = ["Welcome", "Choose a server"]

[lobby.boss_bar]
enable = true
title = "Lobby"
color = "blue"
overlay = "notched_10"
darken_screen = true
play_music = false
create_world_fog = true

[lobby.health_check]
interval_secs = 5
timeout_ms = 250
"#,
        )
        .unwrap();

        assert!(server.lobby.enable);
        assert_eq!(server.lobby.menu_title, "Games");
        assert_eq!(server.lobby.navigator.slot, 4);
        assert_eq!(
            server.lobby.menu_items[0].action.kind,
            LobbyActionKind::Transfer
        );
        assert_eq!(server.lobby.menu_items[0].unknown_item, "minecraft:clock");
        assert_eq!(
            server.lobby.menu_items[0].offline_item,
            "minecraft:red_wool"
        );
        assert_eq!(
            server.lobby.menu_items[0].disabled_item,
            "minecraft:gray_wool"
        );
        assert_eq!(
            server.lobby.menu_items[0].maintenance_item,
            "minecraft:orange_wool"
        );
        assert_eq!(
            server.lobby.menu_items[0].lore,
            vec!["Status: {status_label}", "{status_description}"]
        );
        assert_eq!(server.lobby.menu_items[0].action.target, "survival");
        assert!(server.lobby.broadcast.enable);
        assert_eq!(server.lobby.broadcast.interval_secs, 30);
        assert_eq!(server.lobby.broadcast.messages.len(), 2);
        assert!(server.lobby.boss_bar.enable);
        assert_eq!(server.lobby.boss_bar.title, "Lobby");
        assert_eq!(server.lobby.boss_bar.color, LobbyBossBarColor::Blue);
        assert_eq!(
            server.lobby.boss_bar.overlay,
            LobbyBossBarOverlay::Notched10
        );
        assert!(server.lobby.boss_bar.darken_screen);
        assert!(!server.lobby.boss_bar.play_music);
        assert!(server.lobby.boss_bar.create_world_fog);
        assert_eq!(server.lobby.health_check.interval_secs, 5);
        assert_eq!(server.lobby.health_check.timeout_ms, 250);
    }

    #[test]
    fn parses_menu_settings() {
        let menus: super::Menus = toml::from_str(
            r#"
enable = true
reset_inventory_on_join = true
fixed_slots_only = true

[[hotbar_items]]
slot = 4
item = "minecraft:compass"
name = "Menu"
lore = ["Open menu"]

[hotbar_items.action]
kind = "open_menu"
target = "main"

[[chests]]
id = "main"
title = "Server Menu"
rows = 3

[[chests.items]]
slot = 10
item = "minecraft:ender_pearl"
name = "Survival"

[chests.items.action]
kind = "transfer"
target = "survival"
"#,
        )
        .unwrap();

        assert!(menus.enable);
        assert!(menus.reset_inventory_on_join);
        assert!(menus.fixed_slots_only);
        assert_eq!(menus.hotbar_items[0].slot, 4);
        assert_eq!(
            menus.hotbar_items[0].action.kind,
            super::MenuActionKind::OpenMenu
        );
        assert_eq!(menus.chests[0].id, "main");
        assert_eq!(
            menus.chests[0].items[0].action.kind,
            super::MenuActionKind::Transfer
        );
        assert_eq!(menus.chests[0].items[0].action.target, "survival");
    }

    #[test]
    fn parses_menu_command_action() {
        let action: super::MenuAction = toml::from_str(
            r#"
kind = "command"
target = "prison lottery"
"#,
        )
        .unwrap();

        assert_eq!(action.kind, super::MenuActionKind::Command);
        assert_eq!(action.target, "prison lottery");
    }

    #[test]
    fn parses_entity_rendering_settings() {
        let rendering: super::EntityRendering = toml::from_str(
            r#"
default_distance = 48.0
player_distance = 32.0
npc_distance = 64.0
hologram_distance = 96.0
item_distance = 16.0
item_merge_radius = 3.5
item_merge_max_stack = 32
stack_threshold = 12
stack_radius = 6.5
"#,
        )
        .unwrap();

        assert_eq!(rendering.default_distance, 48.0);
        assert_eq!(rendering.player_distance, 32.0);
        assert_eq!(rendering.npc_distance, 64.0);
        assert_eq!(rendering.hologram_distance, 96.0);
        assert_eq!(rendering.item_distance, 16.0);
        assert_eq!(rendering.item_merge_radius, 3.5);
        assert_eq!(rendering.item_merge_max_stack, 32);
        assert_eq!(rendering.stack_threshold, 12);
        assert_eq!(rendering.stack_radius, 6.5);
    }
}

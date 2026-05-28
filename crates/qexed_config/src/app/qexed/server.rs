use qexed_config::{public::mongodb::MongoConfig, public::mysql::MysqlConfig};
use qexed_config_macros::AutoDoc;
use rust_i18n::t;
use serde::{Deserialize, Serialize, de};

#[derive(Debug, Serialize, Deserialize, AutoDoc)]
pub struct Server {
    #[AutoDoc(key = "config.qexed.server.ip")]
    pub ip: String,

    #[AutoDoc(
        key = "config.qexed.server.online",
        warning = "config.qexed.server.warning.online"
    )]
    pub online: bool,

    #[AutoDoc(key = "config.qexed.server.max_player")]
    pub max_player: i32,

    #[AutoDoc(key = "config.qexed.server.display_players")]
    pub display_players: bool,

    #[AutoDoc(
        key = "config.qexed.server.online_mode",
        warning = "config.qexed.server.warning.online_mode"
    )]
    pub online_mode: bool,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.lan_discovery", sub)]
    pub lan_discovery: LanDiscovery,

    #[AutoDoc(key = "config.qexed.server.network_compression_threshold")]
    pub network_compression_threshold: isize,

    #[AutoDoc(key = "config.qexed.server.proxy")]
    pub proxy: bool,

    #[AutoDoc(key = "config.qexed.server.proxy_protocol")]
    pub proxy_protocol: ForwardingMode,

    #[AutoDoc(
        key = "config.qexed.server.proxy_token",
        warning = "config.qexed.server.warning.proxy_token",
        sensitive,
        default_display = "<stored in .secrets>"
    )]
    pub proxy_token: String,

    #[AutoDoc(key = "config.qexed.server.max_port_connections")]
    pub max_port_connections: u16,

    #[AutoDoc(key = "config.qexed.server.rate_limit_window_secs")]
    pub rate_limit_window_secs: u64,

    #[AutoDoc(key = "config.qexed.server.rate_limit_max_attempts")]
    pub rate_limit_max_attempts: u32,

    #[AutoDoc(key = "config.qexed.server.motd")]
    pub motd: Vec<String>,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.code_of_conduct")]
    pub code_of_conduct: bool,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.world", sub)]
    pub world: World,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.player_data", sub)]
    pub player_data: PlayerData,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.player_messages", sub)]
    pub player_messages: PlayerMessages,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.content_filter", sub)]
    pub content_filter: ContentFilter,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.permissions", sub)]
    pub permissions: Permissions,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.resource_pack", sub)]
    pub resource_pack: ResourcePack,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.entities", sub)]
    pub entities: Entities,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.scoreboard", sub)]
    pub scoreboard: Scoreboard,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.lobby", sub)]
    pub lobby: Lobby,

    #[AutoDoc(key = "config.qexed.server.favicon")]
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
            lan_discovery: LanDiscovery::default(),
            network_compression_threshold: 256,
            proxy: false,
            proxy_protocol: ForwardingMode::QTunnel,
            proxy_token: nanoid::nanoid!(),
            rate_limit_window_secs: 60,
            rate_limit_max_attempts: 6,
            motd: vec![
                t!("qexed_config.config.server.motd1").to_string(),
                t!("qexed_config.config.server.motd2").to_string(),
            ],
            code_of_conduct: false,
            world: World::default(),
            player_data: PlayerData::default(),
            player_messages: PlayerMessages::default(),
            content_filter: ContentFilter::default(),
            permissions: Permissions::default(),
            resource_pack: ResourcePack::default(),
            entities: Entities::default(),
            scoreboard: Scoreboard::default(),
            lobby: Lobby::default(),
            favicon: "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAEAAAABACAYAAACqaXHeAAAACXBIWXMAAA9hAAAPYQGoP6dpAAACtklEQVR42u2ay0rDQBSGJ2EQCipqERU3SkFQQUERRJSCuHDrQvcu3Powbn0DH6IIohQKIi26ELRF8FLxAlbsyksTmTC2yVwyk3ZizmySkjaT/zvnP5mT1PpuDJTgYaOEDwAAAAAAAAAAAAAAAAAAAAAAAABI5MAyX7YsS3lC07pvLCs+jAAd4DpqARXxia8BpsOzk5r6QgB0iTfZOnbU0TO9bthRCIhT0cRRpX4U4v2yUnUezJokrA10iReZX7XWYN0CdNUO0Wj7BUzm+rGJvpSJKn2c/M7ZikKQArC/1RqVnYPvjon3gyELwWp+N+j3QyJ8cHYNTU30uPuvz1V3W8yd/IHAmph3UToLqOi5uAAc8dnNDU/0eeW95SSf10UPQlgAQZEPC0U0kzAv5R3xtPDFhRQaHc+4+7flK1R5SqHba30W0HUHoe2gVAOIeFo4EZ8v1Bt7dTSzuuTCCqoHKqmvAoRAYM2PWdF3PH9eeQwUvzyf8WpB6cZiimve6rrdqmYMcylMCh6d8seFO088GbkiZkaB5ftOd4yYl/601x/KvylPxBN7DPUidC+Yjn4FTtQqYazBswETgCNueHygIR41xL94wi8ua2gk/eEer771ofvTI7SX/wrl7043Tszb4O6ijda3s6746bFu1J8e8rLCEX92WHL3afG8KDdHsB0AWHNw1wEOhJG5FTQ52uV+fqk9+grnpTHP68YCIBDoEZTuMguhZiBGA5CdTHYl2I5nCEHnhldj/1mcyBrDCABBdaEd/YUdx6jpPI8xAHQWQJl+w6gMoK0QNhNkmy3jLCCyihRprCJ5JqialrINjEhEVd8VYNPEs+4MUSynjXwsLmMJ7W+GTIh+OxsmOy7iY7cUjoN4aIaiAhCX6AcWQdX1eJz+TYbjfPFQAwAAAAAAAAAAAACl8QOub9TOwLTmGwAAAABJRU5ErkJggg==".to_string(),
            max_port_connections: u16::MAX,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, AutoDoc, PartialEq, Eq)]
pub struct Lobby {
    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.lobby.enable")]
    pub enable: bool,

    #[serde(default = "default_lobby_protect_world")]
    #[AutoDoc(key = "config.qexed.server.lobby.protect_world")]
    pub protect_world: bool,

    #[serde(default = "default_lobby_menu_title")]
    #[AutoDoc(key = "config.qexed.server.lobby.menu_title")]
    pub menu_title: String,

    #[serde(default = "default_lobby_menu_rows")]
    #[AutoDoc(key = "config.qexed.server.lobby.menu_rows")]
    pub menu_rows: u8,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.lobby.navigator", sub)]
    pub navigator: LobbyNavigator,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[AutoDoc(key = "config.qexed.server.lobby.servers")]
    pub servers: Vec<LobbyServer>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[AutoDoc(key = "config.qexed.server.lobby.menu_items")]
    pub menu_items: Vec<LobbyMenuItem>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[AutoDoc(key = "config.qexed.server.lobby.npc_actions")]
    pub npc_actions: Vec<LobbyNpcAction>,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.lobby.broadcast", sub)]
    pub broadcast: LobbyBroadcast,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.lobby.boss_bar", sub)]
    pub boss_bar: LobbyBossBar,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.lobby.health_check", sub)]
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
            servers: Vec::new(),
            menu_items: Vec::new(),
            npc_actions: Vec::new(),
            broadcast: LobbyBroadcast::default(),
            boss_bar: LobbyBossBar::default(),
            health_check: LobbyHealthCheck::default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, AutoDoc, PartialEq, Eq)]
pub struct LobbyHealthCheck {
    #[serde(default = "default_lobby_health_check_interval_secs")]
    #[AutoDoc(key = "config.qexed.server.lobby.health_check.interval_secs")]
    pub interval_secs: u64,

    #[serde(default = "default_lobby_health_check_timeout_ms")]
    #[AutoDoc(key = "config.qexed.server.lobby.health_check.timeout_ms")]
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

#[derive(Debug, Clone, Deserialize, Serialize, AutoDoc, PartialEq, Eq)]
pub struct LobbyBossBar {
    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.lobby.boss_bar.enable")]
    pub enable: bool,

    #[serde(default = "default_lobby_boss_bar_title")]
    #[AutoDoc(key = "config.qexed.server.lobby.boss_bar.title")]
    pub title: String,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.lobby.boss_bar.color")]
    pub color: LobbyBossBarColor,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.lobby.boss_bar.overlay")]
    pub overlay: LobbyBossBarOverlay,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.lobby.boss_bar.darken_screen")]
    pub darken_screen: bool,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.lobby.boss_bar.play_music")]
    pub play_music: bool,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.lobby.boss_bar.create_world_fog")]
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

#[derive(Debug, Clone, Deserialize, Serialize, AutoDoc, PartialEq, Eq)]
pub struct LobbyBroadcast {
    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.lobby.broadcast.enable")]
    pub enable: bool,

    #[serde(default = "default_lobby_broadcast_interval_secs")]
    #[AutoDoc(key = "config.qexed.server.lobby.broadcast.interval_secs")]
    pub interval_secs: u64,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[AutoDoc(key = "config.qexed.server.lobby.broadcast.messages")]
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

#[derive(Debug, Clone, Deserialize, Serialize, AutoDoc, PartialEq, Eq)]
pub struct LobbyNavigator {
    #[serde(default = "default_lobby_navigator_enable")]
    #[AutoDoc(key = "config.qexed.server.lobby.navigator.enable")]
    pub enable: bool,

    #[serde(default = "default_lobby_navigator_slot")]
    #[AutoDoc(key = "config.qexed.server.lobby.navigator.slot")]
    pub slot: u8,

    #[serde(default = "default_lobby_navigator_item")]
    #[AutoDoc(key = "config.qexed.server.lobby.navigator.item")]
    pub item: String,

    #[serde(default = "default_lobby_navigator_name")]
    #[AutoDoc(key = "config.qexed.server.lobby.navigator.name")]
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

#[derive(Debug, Clone, Deserialize, Serialize, AutoDoc, PartialEq, Eq)]
pub struct LobbyServer {
    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.lobby.servers.id")]
    pub id: String,

    #[serde(default = "default_lobby_server_enable")]
    #[AutoDoc(key = "config.qexed.server.lobby.servers.enable")]
    pub enable: bool,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.lobby.servers.maintenance")]
    pub maintenance: bool,

    #[serde(default = "default_lobby_server_maintenance_message")]
    #[AutoDoc(key = "config.qexed.server.lobby.servers.maintenance_message")]
    pub maintenance_message: String,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.lobby.servers.name")]
    pub name: String,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.lobby.servers.host")]
    pub host: String,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.lobby.servers.port")]
    pub port: u16,
}

impl Default for LobbyServer {
    fn default() -> Self {
        Self {
            id: String::new(),
            enable: default_lobby_server_enable(),
            maintenance: false,
            maintenance_message: default_lobby_server_maintenance_message(),
            name: String::new(),
            host: String::new(),
            port: 0,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, AutoDoc, PartialEq, Eq)]
pub struct LobbyMenuItem {
    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.lobby.menu_items.slot")]
    pub slot: u8,

    #[serde(default = "default_lobby_menu_item_item")]
    #[AutoDoc(key = "config.qexed.server.lobby.menu_items.item")]
    pub item: String,

    #[serde(default = "default_lobby_menu_item_unknown_item")]
    #[AutoDoc(key = "config.qexed.server.lobby.menu_items.unknown_item")]
    pub unknown_item: String,

    #[serde(default = "default_lobby_menu_item_offline_item")]
    #[AutoDoc(key = "config.qexed.server.lobby.menu_items.offline_item")]
    pub offline_item: String,

    #[serde(default = "default_lobby_menu_item_disabled_item")]
    #[AutoDoc(key = "config.qexed.server.lobby.menu_items.disabled_item")]
    pub disabled_item: String,

    #[serde(default = "default_lobby_menu_item_maintenance_item")]
    #[AutoDoc(key = "config.qexed.server.lobby.menu_items.maintenance_item")]
    pub maintenance_item: String,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.lobby.menu_items.name")]
    pub name: String,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[AutoDoc(key = "config.qexed.server.lobby.menu_items.lore")]
    pub lore: Vec<String>,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.lobby.menu_items.action")]
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

#[derive(Debug, Clone, Deserialize, Serialize, AutoDoc, PartialEq, Eq)]
pub struct LobbyNpcAction {
    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.lobby.npc_actions.entity")]
    pub entity: String,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.lobby.npc_actions.action")]
    pub action: LobbyAction,
}

#[derive(Debug, Clone, Deserialize, Serialize, AutoDoc, PartialEq, Eq)]
pub struct LobbyAction {
    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.lobby.action.kind")]
    pub kind: LobbyActionKind,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.lobby.action.target")]
    pub target: String,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.lobby.action.message")]
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

fn default_lobby_server_enable() -> bool {
    true
}

fn default_lobby_server_maintenance_message() -> String {
    "Server is under maintenance.".to_string()
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

#[derive(Debug, Clone, Deserialize, Serialize, AutoDoc, PartialEq, Eq)]
pub struct Scoreboard {
    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.scoreboard.enable")]
    pub enable: bool,

    #[serde(default = "default_scoreboard_objective")]
    #[AutoDoc(key = "config.qexed.server.scoreboard.objective")]
    pub objective: String,

    #[serde(default = "default_scoreboard_title")]
    #[AutoDoc(key = "config.qexed.server.scoreboard.title")]
    pub title: String,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[AutoDoc(key = "config.qexed.server.scoreboard.lines")]
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

fn default_scoreboard_objective() -> String {
    "qexed".to_string()
}

fn default_scoreboard_title() -> String {
    "Qexed".to_string()
}

#[derive(Debug, Clone, Deserialize, Serialize, AutoDoc, PartialEq)]
pub struct Entities {
    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.entities.enable")]
    pub enable: bool,

    #[serde(default = "default_entities_dimension")]
    #[AutoDoc(key = "config.qexed.server.entities.dimension")]
    pub dimension: String,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[AutoDoc(key = "config.qexed.server.entities.list")]
    pub list: Vec<Entity>,
}

impl Default for Entities {
    fn default() -> Self {
        Self {
            enable: false,
            dimension: default_entities_dimension(),
            list: Vec::new(),
        }
    }
}

fn default_entities_dimension() -> String {
    "minecraft:overworld".to_string()
}

#[derive(Debug, Clone, Deserialize, Serialize, AutoDoc, PartialEq)]
pub struct Entity {
    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.entities.list.id")]
    pub id: String,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.entities.list.kind")]
    pub kind: EntityKind,

    #[serde(default = "default_entity_type")]
    #[AutoDoc(key = "config.qexed.server.entities.list.entity_type")]
    pub entity_type: String,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.entities.list.name")]
    pub name: String,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.entities.list.display_name")]
    pub display_name: String,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.entities.list.skin_textures")]
    pub skin_textures: String,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.entities.list.skin_signature")]
    pub skin_signature: String,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.entities.list.skin_player_id")]
    pub skin_player_id: String,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.entities.list.x")]
    pub x: f64,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.entities.list.y")]
    pub y: f64,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.entities.list.z")]
    pub z: f64,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.entities.list.yaw")]
    pub yaw: f32,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.entities.list.pitch")]
    pub pitch: f32,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.entities.list.on_ground")]
    pub on_ground: bool,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.entities.list.data")]
    pub data: i32,
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

#[derive(Debug, Clone, Deserialize, Serialize, AutoDoc, PartialEq, Eq)]
pub struct ResourcePack {
    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.resource_pack.enable")]
    pub enable: bool,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.resource_pack.source")]
    pub source: ResourcePackSource,

    #[serde(default = "default_resource_pack_id")]
    #[AutoDoc(key = "config.qexed.server.resource_pack.id")]
    pub id: uuid::Uuid,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.resource_pack.url")]
    pub url: String,

    #[serde(default = "default_resource_pack_path")]
    #[AutoDoc(key = "config.qexed.server.resource_pack.path")]
    pub path: String,

    #[serde(default = "default_resource_pack_download_bind")]
    #[AutoDoc(key = "config.qexed.server.resource_pack.download_bind")]
    pub download_bind: String,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.resource_pack.download_host")]
    pub download_host: String,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.resource_pack.object_storage", sub)]
    pub object_storage: ResourcePackObjectStorage,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.resource_pack.hash")]
    pub hash: String,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.resource_pack.required")]
    pub required: bool,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.resource_pack.prompt")]
    pub prompt: String,

    #[serde(default = "default_resource_pack_disconnect_message")]
    #[AutoDoc(key = "config.qexed.server.resource_pack.disconnect_message")]
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

#[derive(Debug, Clone, Deserialize, Serialize, AutoDoc, PartialEq, Eq)]
pub struct ResourcePackObjectStorage {
    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.resource_pack.object_storage.provider")]
    pub provider: ResourcePackObjectStorageProvider,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.resource_pack.object_storage.public_base_url")]
    pub public_base_url: String,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.resource_pack.object_storage.endpoint")]
    pub endpoint: String,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.resource_pack.object_storage.bucket")]
    pub bucket: String,

    #[serde(default = "default_resource_pack_object_key")]
    #[AutoDoc(key = "config.qexed.server.resource_pack.object_storage.object_key")]
    pub object_key: String,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.resource_pack.object_storage.force_path_style")]
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

#[derive(Debug, Clone, Deserialize, Serialize, AutoDoc, PartialEq, Eq)]
pub struct PlayerMessages {
    #[serde(default = "default_player_messages_enable")]
    #[AutoDoc(key = "config.qexed.server.player_messages.enable")]
    pub enable: bool,

    #[serde(default = "default_player_join_message")]
    #[AutoDoc(key = "config.qexed.server.player_messages.join")]
    pub join: String,

    #[serde(default = "default_player_leave_message")]
    #[AutoDoc(key = "config.qexed.server.player_messages.leave")]
    pub leave: String,
}

impl Default for PlayerMessages {
    fn default() -> Self {
        Self {
            enable: default_player_messages_enable(),
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

#[derive(Debug, Clone, Deserialize, Serialize, AutoDoc, PartialEq, Eq)]
pub struct ContentFilter {
    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.content_filter.enable")]
    pub enable: bool,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.content_filter.engine")]
    pub engine: ContentFilterEngine,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.content_filter.words")]
    pub words: Vec<String>,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.content_filter.knowledge_path")]
    pub knowledge_path: String,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.content_filter.api_url")]
    pub api_url: String,

    #[serde(default)]
    #[AutoDoc(
        key = "config.qexed.server.content_filter.api_token",
        sensitive,
        default_display = "<stored in .secrets>"
    )]
    pub api_token: String,

    #[serde(default = "default_content_filter_replacement")]
    #[AutoDoc(key = "config.qexed.server.content_filter.replacement")]
    pub replacement: String,

    #[serde(default = "default_content_filter_block_message")]
    #[AutoDoc(key = "config.qexed.server.content_filter.block_message")]
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

#[derive(Debug, Clone, Deserialize, Serialize, AutoDoc, PartialEq)]
pub struct Permissions {
    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.permissions.engine")]
    pub engine: PermissionEngine,

    #[serde(default = "default_permissions_local_path")]
    #[AutoDoc(key = "config.qexed.server.permissions.local_path")]
    pub local_path: String,

    #[serde(default = "default_permissions_table_prefix")]
    #[AutoDoc(key = "config.qexed.server.permissions.table_prefix")]
    pub table_prefix: String,

    #[serde(default = "default_permissions_server")]
    #[AutoDoc(key = "config.qexed.server.permissions.server")]
    pub server: String,

    #[serde(default = "default_permissions_world")]
    #[AutoDoc(key = "config.qexed.server.permissions.world")]
    pub world: String,

    #[serde(default = "default_permissions_default_group")]
    #[AutoDoc(key = "config.qexed.server.permissions.default_group")]
    pub default_group: String,

    #[serde(default = "default_permissions_allow_by_default")]
    #[AutoDoc(key = "config.qexed.server.permissions.allow_by_default")]
    pub allow_by_default: bool,

    #[serde(default = "default_permissions_denied_message")]
    #[AutoDoc(key = "config.qexed.server.permissions.denied_message")]
    pub denied_message: String,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.permissions.mysql", sub)]
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
    true
}

fn default_permissions_denied_message() -> String {
    "You do not have permission to use this command.".to_string()
}

#[derive(Debug, Clone, Deserialize, Serialize, AutoDoc, PartialEq)]
pub struct PlayerData {
    #[serde(default = "default_player_data_enable")]
    #[AutoDoc(key = "config.qexed.server.player_data.enable")]
    pub enable: bool,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.player_data.engine")]
    pub engine: PlayerDataEngine,

    #[serde(default = "default_player_data_collection")]
    #[AutoDoc(key = "config.qexed.server.player_data.collection")]
    pub collection: String,

    #[serde(default = "default_player_data_table")]
    #[AutoDoc(key = "config.qexed.server.player_data.table")]
    pub table: String,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.player_data.mongodb", sub)]
    pub mongodb: MongoConfig,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.player_data.mysql", sub)]
    pub mysql: MysqlConfig,
}

impl Default for PlayerData {
    fn default() -> Self {
        Self {
            enable: default_player_data_enable(),
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

#[derive(Debug, Clone, Deserialize, Serialize, AutoDoc)]
pub struct LanDiscovery {
    #[AutoDoc(key = "config.qexed.server.lan_discovery.enable")]
    pub enable: bool,

    #[AutoDoc(key = "config.qexed.server.lan_discovery.interval_ms")]
    pub interval_ms: u64,
}

impl Default for LanDiscovery {
    fn default() -> Self {
        Self {
            enable: true,
            interval_ms: 1500,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, AutoDoc)]
pub struct World {
    #[AutoDoc(key = "config.qexed.server.world.path")]
    pub path: String,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.world.read_only")]
    pub read_only: bool,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.world.generator")]
    pub generator: WorldGenerator,

    #[serde(default = "default_world_generator_preset")]
    #[AutoDoc(key = "config.qexed.server.world.generator_preset")]
    pub generator_preset: String,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.world.seed")]
    pub seed: i64,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.world.game_mode")]
    pub game_mode: GameMode,

    #[serde(default = "default_spawn_protection_radius")]
    #[AutoDoc(key = "config.qexed.server.world.spawn_protection_radius")]
    pub spawn_protection_radius: i32,

    #[AutoDoc(key = "config.qexed.server.world.dimension")]
    pub dimension: String,

    #[AutoDoc(key = "config.qexed.server.world.dimension_type")]
    pub dimension_type: String,

    #[AutoDoc(key = "config.qexed.server.world.view_distance")]
    pub view_distance: i32,

    #[serde(default = "default_chunk_load_parallelism")]
    #[AutoDoc(key = "config.qexed.server.world.chunk_load_parallelism")]
    pub chunk_load_parallelism: usize,

    #[AutoDoc(key = "config.qexed.server.world.simulation_distance")]
    pub simulation_distance: i32,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.world.light")]
    pub light: LightMode,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.world.light_algorithm")]
    pub light_algorithm: LightAlgorithm,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.world.gpu", sub)]
    pub gpu: WorldGpu,

    #[AutoDoc(key = "config.qexed.server.world.spawn", sub)]
    pub spawn: Spawn,
}

impl Default for World {
    fn default() -> Self {
        Self {
            path: "world".to_string(),
            read_only: false,
            generator: WorldGenerator::default(),
            generator_preset: default_world_generator_preset(),
            seed: 0,
            game_mode: GameMode::default(),
            spawn_protection_radius: default_spawn_protection_radius(),
            dimension: "minecraft:overworld".to_string(),
            dimension_type: "minecraft:overworld".to_string(),
            view_distance: 3,
            chunk_load_parallelism: default_chunk_load_parallelism(),
            simulation_distance: 3,
            light: LightMode::default(),
            light_algorithm: LightAlgorithm::default(),
            gpu: WorldGpu::default(),
            spawn: Spawn::default(),
        }
    }
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

fn default_chunk_load_parallelism() -> usize {
    4
}

fn default_spawn_protection_radius() -> i32 {
    16
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

#[derive(Debug, Clone, Deserialize, Serialize, AutoDoc, PartialEq, Eq)]
pub struct WorldGpu {
    #[serde(default = "default_world_gpu_enable")]
    #[AutoDoc(key = "config.qexed.server.world.gpu.enable")]
    pub enable: bool,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.world.gpu.device")]
    pub device: GpuDeviceSelector,
}

impl Default for WorldGpu {
    fn default() -> Self {
        Self {
            enable: default_world_gpu_enable(),
            device: GpuDeviceSelector::default(),
        }
    }
}

fn default_world_gpu_enable() -> bool {
    false
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GpuDeviceSelector {
    Auto,
    Discrete,
    Integrated,
    Cpu,
    Index(usize),
}

impl Default for GpuDeviceSelector {
    fn default() -> Self {
        Self::Discrete
    }
}

impl Serialize for GpuDeviceSelector {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Self::Auto => serializer.serialize_str("auto"),
            Self::Discrete => serializer.serialize_str("discrete"),
            Self::Integrated => serializer.serialize_str("integrated"),
            Self::Cpu => serializer.serialize_str("cpu"),
            Self::Index(index) => serializer.serialize_u64(*index as u64),
        }
    }
}

impl<'de> Deserialize<'de> for GpuDeviceSelector {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = GpuDeviceSelector;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str(r#""auto", "discrete", "integrated", "cpu", or a GPU index"#)
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                match value.trim().to_ascii_lowercase().as_str() {
                    "auto" => Ok(GpuDeviceSelector::Auto),
                    "discrete" => Ok(GpuDeviceSelector::Discrete),
                    "integrated" => Ok(GpuDeviceSelector::Integrated),
                    "cpu" => Ok(GpuDeviceSelector::Cpu),
                    other => other
                        .parse::<usize>()
                        .map(GpuDeviceSelector::Index)
                        .map_err(|_| E::custom(format!("unknown GPU device selector: {value}"))),
                }
            }

            fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                usize::try_from(value)
                    .map(GpuDeviceSelector::Index)
                    .map_err(|_| E::custom(format!("GPU index out of range: {value}")))
            }

            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                usize::try_from(value)
                    .map(GpuDeviceSelector::Index)
                    .map_err(|_| E::custom(format!("GPU index out of range: {value}")))
            }
        }

        deserializer.deserialize_any(Visitor)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, AutoDoc)]
pub struct Spawn {
    #[AutoDoc(key = "config.qexed.server.world.spawn.x")]
    pub x: f64,

    #[AutoDoc(key = "config.qexed.server.world.spawn.y")]
    pub y: f64,

    #[AutoDoc(key = "config.qexed.server.world.spawn.z")]
    pub z: f64,

    #[AutoDoc(key = "config.qexed.server.world.spawn.yaw")]
    pub yaw: f32,

    #[AutoDoc(key = "config.qexed.server.world.spawn.pitch")]
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
            _ => Err(format!("未知的转发模式: {}", s)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ContentFilter, ContentFilterEngine, EntityKind, ForwardingMode, GameMode,
        GpuDeviceSelector, LightAlgorithm, LightMode, LobbyActionKind, LobbyBossBarColor,
        LobbyBossBarOverlay, PermissionEngine, Permissions, PlayerData, PlayerDataEngine,
        PlayerMessages, ResourcePack, ResourcePackObjectStorageProvider, ResourcePackSource,
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
    }

    #[test]
    fn parses_world_gpu_settings() {
        let world: World = toml::from_str(
            r#"
path = "world"
dimension = "minecraft:overworld"
dimension_type = "minecraft:overworld"
view_distance = 3
chunk_load_parallelism = 6
simulation_distance = 3
light = "static"
light_algorithm = "fast"

[gpu]
enable = true
device = "integrated"

[spawn]
x = 0.0
y = 0.0
z = 0.0
yaw = 0.0
pitch = 0.0
"#,
        )
        .unwrap();

        assert!(world.gpu.enable);
        assert_eq!(world.gpu.device, GpuDeviceSelector::Integrated);
    }

    #[test]
    fn parses_world_gpu_index() {
        let world: World = toml::from_str(
            r#"
path = "world"
dimension = "minecraft:overworld"
dimension_type = "minecraft:overworld"
view_distance = 3
chunk_load_parallelism = 6
simulation_distance = 3
light = "static"
light_algorithm = "fast"

[gpu]
device = 1

[spawn]
x = 0.0
y = 0.0
z = 0.0
yaw = 0.0
pitch = 0.0
"#,
        )
        .unwrap();

        assert!(!world.gpu.enable);
        assert_eq!(world.gpu.device, GpuDeviceSelector::Index(1));
    }

    #[test]
    fn parses_player_data_settings() {
        let player_data: PlayerData = toml::from_str(
            r#"
enable = false
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
join = "{player} joined"
leave = "{player} left"
"#,
        )
        .unwrap();

        assert!(player_messages.enable);
        assert_eq!(player_messages.join, "{player} joined");
        assert_eq!(player_messages.leave, "{player} left");
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
proxy = false
proxy_protocol = "QTunnel"
proxy_token = "secret"
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
    fn parses_scoreboard_settings() {
        let server: Server = toml::from_str(
            r#"
ip = "0.0.0.0:25565"
online = false
max_player = -1
display_players = true
online_mode = false
network_compression_threshold = 256
proxy = false
proxy_protocol = "QTunnel"
proxy_token = "secret"
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
proxy = false
proxy_protocol = "QTunnel"
proxy_token = "secret"
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

[[lobby.servers]]
id = "survival"
enable = true
maintenance = false
maintenance_message = "Survival is restarting."
name = "Survival"
host = "127.0.0.1"
port = 25566

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

[[lobby.npc_actions]]
entity = "spawn-guide"

[lobby.npc_actions.action]
kind = "open_menu"

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
        assert_eq!(server.lobby.servers[0].id, "survival");
        assert!(server.lobby.servers[0].enable);
        assert!(!server.lobby.servers[0].maintenance);
        assert_eq!(
            server.lobby.servers[0].maintenance_message,
            "Survival is restarting."
        );
        assert_eq!(server.lobby.servers[0].port, 25566);
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
        assert_eq!(
            server.lobby.npc_actions[0].action.kind,
            LobbyActionKind::OpenMenu
        );
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
}

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
    #[AutoDoc(key = "config.qexed.server.log_level")]
    pub log_level: ServerLogLevel,

    #[serde(default = "default_mojang_cache_path")]
    #[AutoDoc(key = "config.qexed.server.mojang_cache_path")]
    pub mojang_cache_path: String,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.lan_discovery", sub)]
    pub lan_discovery: LanDiscovery,

    #[AutoDoc(key = "config.qexed.server.network_compression_threshold")]
    pub network_compression_threshold: isize,

    #[AutoDoc(key = "config.qexed.server.proxy")]
    pub proxy: bool,

    #[AutoDoc(key = "config.qexed.server.proxy_protocol")]
    pub proxy_protocol: ForwardingMode,

    #[serde(default)]
    #[AutoDoc(
        key = "config.qexed.server.proxy_server_id",
        warning = "config.qexed.server.warning.proxy_server_id"
    )]
    pub proxy_server_id: String,

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

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.click_detection", sub)]
    pub click_detection: ClickDetection,

    #[AutoDoc(key = "config.qexed.server.motd")]
    pub motd: Vec<String>,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.code_of_conduct")]
    pub code_of_conduct: bool,

    #[serde(default, skip)]
    #[AutoDoc(key = "config.qexed.server.world", sub)]
    pub world: World,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.player_data", sub)]
    pub player_data: PlayerData,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.player_messages", sub)]
    pub player_messages: PlayerMessages,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.player_audit", sub)]
    pub player_audit: PlayerAudit,

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
    #[AutoDoc(key = "config.qexed.server.menus", sub)]
    pub menus: Menus,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.entity_rendering", sub)]
    pub entity_rendering: EntityRendering,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.placeholders", sub)]
    pub placeholders: Placeholders,

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
            log_level: ServerLogLevel::default(),
            mojang_cache_path: default_mojang_cache_path(),
            lan_discovery: LanDiscovery::default(),
            network_compression_threshold: 256,
            proxy: false,
            proxy_protocol: ForwardingMode::QTunnel,
            proxy_server_id: String::new(),
            proxy_token: nanoid::nanoid!(),
            rate_limit_window_secs: 60,
            rate_limit_max_attempts: 6,
            click_detection: ClickDetection::default(),
            motd: vec![
                t!("qexed_config.config.server.motd1").to_string(),
                t!("qexed_config.config.server.motd2").to_string(),
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
    #[AutoDoc(key = "config.qexed.server.lobby.servers", sub)]
    pub servers: Vec<LobbyServer>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[AutoDoc(key = "config.qexed.server.lobby.menu_items", sub)]
    pub menu_items: Vec<LobbyMenuItem>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[AutoDoc(key = "config.qexed.server.lobby.npc_actions", sub)]
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
    #[AutoDoc(key = "config.qexed.server.lobby.menu_items.action", sub)]
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
    #[AutoDoc(key = "config.qexed.server.lobby.npc_actions.action", sub)]
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

#[derive(Debug, Clone, Deserialize, Serialize, AutoDoc, PartialEq, Eq)]
pub struct Menus {
    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.menus.enable")]
    pub enable: bool,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.menus.reset_inventory_on_join")]
    pub reset_inventory_on_join: bool,

    #[serde(default = "default_menus_fixed_slots_only")]
    #[AutoDoc(key = "config.qexed.server.menus.fixed_slots_only")]
    pub fixed_slots_only: bool,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[AutoDoc(key = "config.qexed.server.menus.hotbar_items", sub)]
    pub hotbar_items: Vec<MenuHotbarItem>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[AutoDoc(key = "config.qexed.server.menus.chests", sub)]
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

#[derive(Debug, Clone, Deserialize, Serialize, AutoDoc, PartialEq, Eq)]
pub struct MenuHotbarItem {
    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.menus.hotbar_items.slot")]
    pub slot: u8,

    #[serde(default = "default_menu_item_item")]
    #[AutoDoc(key = "config.qexed.server.menus.hotbar_items.item")]
    pub item: String,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.menus.hotbar_items.name")]
    pub name: String,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[AutoDoc(key = "config.qexed.server.menus.hotbar_items.lore")]
    pub lore: Vec<String>,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.menus.hotbar_items.action", sub)]
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

#[derive(Debug, Clone, Deserialize, Serialize, AutoDoc, PartialEq, Eq)]
pub struct ChestMenu {
    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.menus.chests.id")]
    pub id: String,

    #[serde(default = "default_chest_menu_title")]
    #[AutoDoc(key = "config.qexed.server.menus.chests.title")]
    pub title: String,

    #[serde(default = "default_chest_menu_rows")]
    #[AutoDoc(key = "config.qexed.server.menus.chests.rows")]
    pub rows: u8,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[AutoDoc(key = "config.qexed.server.menus.chests.items", sub)]
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

#[derive(Debug, Clone, Deserialize, Serialize, AutoDoc, PartialEq, Eq)]
pub struct MenuItem {
    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.menus.chests.items.slot")]
    pub slot: u8,

    #[serde(default = "default_menu_item_item")]
    #[AutoDoc(key = "config.qexed.server.menus.chests.items.item")]
    pub item: String,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.menus.chests.items.name")]
    pub name: String,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[AutoDoc(key = "config.qexed.server.menus.chests.items.lore")]
    pub lore: Vec<String>,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.menus.chests.items.action", sub)]
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

#[derive(Debug, Clone, Deserialize, Serialize, AutoDoc, PartialEq, Eq)]
pub struct MenuAction {
    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.menus.action.kind")]
    pub kind: MenuActionKind,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.menus.action.target")]
    pub target: String,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.menus.action.message")]
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
    Message,
    HidePlayers,
    ShowPlayers,
    TogglePlayers,
}

#[derive(Debug, Clone, Deserialize, Serialize, AutoDoc, PartialEq)]
pub struct EntityRendering {
    #[serde(default = "default_entity_render_distance")]
    #[AutoDoc(key = "config.qexed.server.entity_rendering.default_distance")]
    pub default_distance: f64,

    #[serde(default = "default_player_render_distance")]
    #[AutoDoc(key = "config.qexed.server.entity_rendering.player_distance")]
    pub player_distance: f64,

    #[serde(default = "default_npc_render_distance")]
    #[AutoDoc(key = "config.qexed.server.entity_rendering.npc_distance")]
    pub npc_distance: f64,

    #[serde(default = "default_hologram_render_distance")]
    #[AutoDoc(key = "config.qexed.server.entity_rendering.hologram_distance")]
    pub hologram_distance: f64,

    #[serde(default = "default_item_render_distance")]
    #[AutoDoc(key = "config.qexed.server.entity_rendering.item_distance")]
    pub item_distance: f64,

    #[serde(default = "default_entity_stack_threshold")]
    #[AutoDoc(key = "config.qexed.server.entity_rendering.stack_threshold")]
    pub stack_threshold: usize,

    #[serde(default = "default_entity_stack_radius")]
    #[AutoDoc(key = "config.qexed.server.entity_rendering.stack_radius")]
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

#[derive(Debug, Clone, Deserialize, Serialize, AutoDoc, PartialEq, Eq)]
pub struct Placeholders {
    #[serde(default = "default_placeholders_enable")]
    #[AutoDoc(key = "config.qexed.server.placeholders.enable")]
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

#[derive(Debug, Clone, Deserialize, Serialize, AutoDoc, PartialEq)]
pub struct Entities {
    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.entities.enable")]
    pub enable: bool,

    #[serde(default = "default_entities_dimension")]
    #[AutoDoc(key = "config.qexed.server.entities.dimension")]
    pub dimension: String,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[AutoDoc(key = "config.qexed.server.entities.list", sub)]
    pub list: Vec<Entity>,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.entities.spawning", sub)]
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

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.entities.list.ai")]
    pub ai: String,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.entities.list.look_at_players")]
    pub look_at_players: bool,

    #[serde(default = "default_npc_main_hand_event")]
    #[AutoDoc(key = "config.qexed.server.entities.list.main_hand_event")]
    pub main_hand_event: String,

    #[serde(default = "default_npc_off_hand_event")]
    #[AutoDoc(key = "config.qexed.server.entities.list.off_hand_event")]
    pub off_hand_event: String,

    #[serde(default = "default_npc_attack_event")]
    #[AutoDoc(key = "config.qexed.server.entities.list.attack_event")]
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

fn default_npc_main_hand_event() -> String {
    "interact".to_string()
}

fn default_npc_off_hand_event() -> String {
    "interact_off_hand".to_string()
}

fn default_npc_attack_event() -> String {
    "attack".to_string()
}

#[derive(Debug, Clone, Deserialize, Serialize, AutoDoc, PartialEq)]
pub struct EntitySpawning {
    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.entities.spawning.enable")]
    pub enable: bool,

    #[serde(default = "default_entity_spawn_tick_interval_ms")]
    #[AutoDoc(key = "config.qexed.server.entities.spawning.tick_interval_ms")]
    pub tick_interval_ms: u64,

    #[serde(default = "default_entity_ai_tick_interval_ms")]
    #[AutoDoc(key = "config.qexed.server.entities.spawning.ai_tick_interval_ms")]
    pub ai_tick_interval_ms: u64,

    #[serde(default = "default_entity_spawn_global_cap")]
    #[AutoDoc(key = "config.qexed.server.entities.spawning.global_cap")]
    pub global_cap: usize,

    #[serde(default = "default_entity_spawn_per_dimension_cap")]
    #[AutoDoc(key = "config.qexed.server.entities.spawning.per_dimension_cap")]
    pub per_dimension_cap: usize,

    #[serde(default = "default_entity_spawn_per_type_cap")]
    #[AutoDoc(key = "config.qexed.server.entities.spawning.per_type_cap")]
    pub per_type_cap: usize,

    #[serde(default = "default_entity_spawn_max_per_tick")]
    #[AutoDoc(key = "config.qexed.server.entities.spawning.max_spawn_per_tick")]
    pub max_spawn_per_tick: usize,

    #[serde(default = "default_entity_spawn_player_activation_range")]
    #[AutoDoc(key = "config.qexed.server.entities.spawning.player_activation_range")]
    pub player_activation_range: f64,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[AutoDoc(key = "config.qexed.server.entities.spawning.rules", sub)]
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

#[derive(Debug, Clone, Deserialize, Serialize, AutoDoc, PartialEq)]
pub struct EntitySpawnRule {
    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.entities.spawning.rules.id")]
    pub id: String,

    #[serde(default = "default_entity_spawn_rule_enable")]
    #[AutoDoc(key = "config.qexed.server.entities.spawning.rules.enable")]
    pub enable: bool,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.entities.spawning.rules.dimension")]
    pub dimension: String,

    #[serde(default = "default_entity_spawn_rule_entity_type")]
    #[AutoDoc(key = "config.qexed.server.entities.spawning.rules.entity_type")]
    pub entity_type: String,

    #[serde(default = "default_entity_spawn_rule_weight")]
    #[AutoDoc(key = "config.qexed.server.entities.spawning.rules.weight")]
    pub weight: u32,

    #[serde(default = "default_entity_spawn_rule_cap")]
    #[AutoDoc(key = "config.qexed.server.entities.spawning.rules.cap")]
    pub cap: usize,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.entities.spawning.rules.name")]
    pub name: String,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.entities.spawning.rules.display_name")]
    pub display_name: String,

    #[serde(default = "default_entity_spawn_rule_ai")]
    #[AutoDoc(key = "config.qexed.server.entities.spawning.rules.ai")]
    pub ai: String,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.entities.spawning.rules.data")]
    pub data: i32,

    #[serde(default = "default_entity_spawn_rule_on_ground")]
    #[AutoDoc(key = "config.qexed.server.entities.spawning.rules.on_ground")]
    pub on_ground: bool,

    #[serde(default = "default_entity_spawn_min_x")]
    #[AutoDoc(key = "config.qexed.server.entities.spawning.rules.min_x")]
    pub min_x: f64,

    #[serde(default = "default_entity_spawn_max_x")]
    #[AutoDoc(key = "config.qexed.server.entities.spawning.rules.max_x")]
    pub max_x: f64,

    #[serde(default = "default_entity_spawn_min_y")]
    #[AutoDoc(key = "config.qexed.server.entities.spawning.rules.min_y")]
    pub min_y: f64,

    #[serde(default = "default_entity_spawn_max_y")]
    #[AutoDoc(key = "config.qexed.server.entities.spawning.rules.max_y")]
    pub max_y: f64,

    #[serde(default = "default_entity_spawn_min_z")]
    #[AutoDoc(key = "config.qexed.server.entities.spawning.rules.min_z")]
    pub min_z: f64,

    #[serde(default = "default_entity_spawn_max_z")]
    #[AutoDoc(key = "config.qexed.server.entities.spawning.rules.max_z")]
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
            name: String::new(),
            display_name: String::new(),
            ai: default_entity_spawn_rule_ai(),
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
    200
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

fn default_entity_spawn_rule_ai() -> String {
    "random_stroll".to_string()
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

#[derive(Debug, Clone, Deserialize, Serialize, AutoDoc, PartialEq, Eq)]
pub struct ClickDetection {
    #[serde(default = "default_click_detection_enable")]
    #[AutoDoc(key = "config.qexed.server.click_detection.enable")]
    pub enable: bool,

    #[serde(default = "default_click_detection_window_ms")]
    #[AutoDoc(key = "config.qexed.server.click_detection.window_ms")]
    pub window_ms: u64,

    #[serde(default = "default_click_detection_max_clicks")]
    #[AutoDoc(key = "config.qexed.server.click_detection.max_clicks")]
    pub max_clicks: u32,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.click_detection.cancel_actions")]
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
pub struct PlayerAudit {
    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.player_audit.enable")]
    pub enable: bool,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.player_audit.storage")]
    pub storage: PlayerAuditStorage,

    #[serde(default = "default_player_audit_file_path")]
    #[AutoDoc(key = "config.qexed.server.player_audit.file_path")]
    pub file_path: String,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.player_audit.events", sub)]
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

#[derive(Debug, Clone, Deserialize, Serialize, AutoDoc, PartialEq, Eq)]
pub struct PlayerAuditEvents {
    #[serde(default = "default_player_audit_track_block_place")]
    #[AutoDoc(key = "config.qexed.server.player_audit.events.block_place")]
    pub block_place: bool,

    #[serde(default = "default_player_audit_track_block_break")]
    #[AutoDoc(key = "config.qexed.server.player_audit.events.block_break")]
    pub block_break: bool,

    #[serde(default = "default_player_audit_track_item_switch")]
    #[AutoDoc(key = "config.qexed.server.player_audit.events.item_switch")]
    pub item_switch: bool,

    #[serde(default = "default_player_audit_track_command")]
    #[AutoDoc(key = "config.qexed.server.player_audit.events.command")]
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
    false
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
    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.world.default_dimension")]
    pub default_dimension: String,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[AutoDoc(key = "config.qexed.server.world.worlds", sub)]
    pub worlds: Vec<WorldStorage>,

    #[serde(default = "default_world_path")]
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

    #[serde(default = "default_world_default_dimension")]
    #[AutoDoc(key = "config.qexed.server.world.dimension")]
    pub dimension: String,

    #[serde(default = "default_world_dimension_type")]
    #[AutoDoc(key = "config.qexed.server.world.dimension_type")]
    pub dimension_type: String,

    #[AutoDoc(key = "config.qexed.server.world.view_distance")]
    pub view_distance: i32,

    #[serde(default = "default_chunk_load_parallelism")]
    #[AutoDoc(key = "config.qexed.server.world.chunk_load_parallelism")]
    pub chunk_load_parallelism: usize,

    #[serde(default = "default_chunk_update_delay_ms")]
    #[AutoDoc(key = "config.qexed.server.world.chunk_update_delay_ms")]
    pub chunk_update_delay_ms: u64,

    #[AutoDoc(key = "config.qexed.server.world.simulation_distance")]
    pub simulation_distance: i32,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.world.light")]
    pub light: LightMode,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.world.light_algorithm")]
    pub light_algorithm: LightAlgorithm,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.world.precompiled_chunks", sub)]
    pub precompiled_chunks: PrecompiledChunks,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.world.gpu", sub)]
    pub gpu: WorldGpu,

    #[AutoDoc(key = "config.qexed.server.world.spawn", sub)]
    pub spawn: Spawn,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[AutoDoc(key = "config.qexed.server.world.instances", sub)]
    pub instances: Vec<WorldInstance>,
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
            gpu: WorldGpu::default(),
            spawn: Spawn::default(),
            instances: Vec::new(),
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

    fn config_file_description(lang: &str, config_file: &str, _root_path: Option<&str>) -> String {
        rust_i18n::t!(
            "autodoc.file_description.world",
            locale = lang,
            file = config_file
        )
        .to_string()
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

#[derive(Debug, Clone, Deserialize, Serialize, AutoDoc, PartialEq, Eq)]
pub struct WorldStorage {
    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.world.worlds.id")]
    pub id: String,

    #[serde(default = "default_world_default_dimension")]
    #[AutoDoc(key = "config.qexed.server.world.worlds.dimension")]
    pub dimension: String,

    #[serde(default = "default_world_dimension_type")]
    #[AutoDoc(key = "config.qexed.server.world.worlds.dimension_type")]
    pub dimension_type: String,

    #[serde(default = "default_world_path")]
    #[AutoDoc(key = "config.qexed.server.world.worlds.path")]
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

#[derive(Debug, Clone, Deserialize, Serialize, AutoDoc, PartialEq, Eq)]
pub struct WorldInstance {
    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.world.instances.id")]
    pub id: String,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.world.instances.dimension")]
    pub dimension: String,

    #[serde(default = "default_world_instance_source_dimension")]
    #[AutoDoc(key = "config.qexed.server.world.instances.source_dimension")]
    pub source_dimension: String,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.world.instances.path")]
    pub path: String,

    #[serde(default = "default_world_instance_copy_on_write")]
    #[AutoDoc(key = "config.qexed.server.world.instances.copy_on_write")]
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

#[derive(Debug, Clone, Deserialize, Serialize, AutoDoc, PartialEq, Eq)]
pub struct PrecompiledChunks {
    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.world.precompiled_chunks.enable")]
    pub enable: bool,

    #[serde(default = "default_precompiled_chunks_light")]
    #[AutoDoc(key = "config.qexed.server.world.precompiled_chunks.light")]
    pub light: bool,

    #[serde(default = "default_precompiled_chunks_max_cached_packets")]
    #[AutoDoc(key = "config.qexed.server.world.precompiled_chunks.max_cached_packets")]
    pub max_cached_packets: usize,

    #[serde(default = "default_precompiled_chunks_max_cached_packet_bytes")]
    #[AutoDoc(key = "config.qexed.server.world.precompiled_chunks.max_cached_packet_bytes")]
    pub max_cached_packet_bytes: usize,

    #[serde(default = "default_precompiled_chunks_block_state_cache_limit")]
    #[AutoDoc(key = "config.qexed.server.world.precompiled_chunks.block_state_cache_limit")]
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
        ContentFilter, ContentFilterEngine, Entities, EntityKind, ForwardingMode, GameMode,
        GpuDeviceSelector, LightAlgorithm, LightMode, LobbyActionKind, LobbyBossBarColor,
        LobbyBossBarOverlay, PermissionEngine, Permissions, PlayerAudit, PlayerAuditStorage,
        PlayerData, PlayerDataEngine, PlayerMessages, PrecompiledChunks, ResourcePack,
        ResourcePackObjectStorageProvider, ResourcePackSource, Server, World, WorldGenerator,
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
proxy = false
proxy_protocol = "QTunnel"
proxy_server_id = ""
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
name = "Mine Zombie"
display_name = "{\"text\":\"Mine Zombie\"}"
ai = "follow_nearest_player"
min_x = -8.0
max_x = 8.0
min_y = 64.0
max_y = 64.0
min_z = -8.0
max_z = 8.0
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
proxy_server_id = ""
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
proxy_server_id = ""
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
    fn parses_entity_rendering_settings() {
        let rendering: super::EntityRendering = toml::from_str(
            r#"
default_distance = 48.0
player_distance = 32.0
npc_distance = 64.0
hologram_distance = 96.0
item_distance = 16.0
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
        assert_eq!(rendering.stack_threshold, 12);
        assert_eq!(rendering.stack_radius, 6.5);
    }
}

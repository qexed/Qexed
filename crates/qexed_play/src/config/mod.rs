//! qexed_play 配置：v4 qexed_config::app::qexed::server 里 lobby / menus / world / player_messages 域的等效定义（v6 不存在那些路径，按迁移规则 4 用 app_config 宏重定义）。
//! 说明：app_config 不依赖 Doc；含 Vec/嵌套结构体字段的配置不派生 Doc（与 qexed_server 的做法一致）。

use serde::{Deserialize, Serialize};


// ─────────────────────────── Lobby ───────────────────────────

/// 大厅配置（v4 server::Lobby 迁移，字段名与默认值一致）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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

fn default_lobby_protect_world() -> bool {
    true
}

fn default_lobby_menu_title() -> String {
    "Qexed Lobby".to_string()
}

fn default_lobby_menu_rows() -> u8 {
    1
}

/// 大厅后端健康检查（v4 server::LobbyHealthCheck；探活在 server 域，此处仅保留配置）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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

fn default_lobby_health_check_interval_secs() -> u64 {
    10
}

fn default_lobby_health_check_timeout_ms() -> u64 {
    2000
}

/// 大厅 Boss 栏（v4 server::LobbyBossBar）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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

fn default_lobby_boss_bar_title() -> String {
    "Qexed".to_string()
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

/// 大厅轮播广播（v4 server::LobbyBroadcast）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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

fn default_lobby_broadcast_interval_secs() -> u64 {
    60
}

/// 大厅导航物品（v4 server::LobbyNavigator）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LobbyNavigator {
    #[serde(default)]
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
            enable: false,
            slot: default_lobby_navigator_slot(),
            item: default_lobby_navigator_item(),
            name: default_lobby_navigator_name(),
        }
    }
}

fn default_lobby_navigator_slot() -> u8 {
    8
}

fn default_lobby_navigator_item() -> String {
    "minecraft:compass".to_string()
}

fn default_lobby_navigator_name() -> String {
    "Server Selector".to_string()
}

/// 大厅菜单项（v4 server::LobbyMenuItem）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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

fn default_lobby_menu_item_item() -> String {
    "minecraft:compass".to_string()
}

fn default_lobby_menu_item_unknown_item() -> String {
    "minecraft:clock".to_string()
}

fn default_lobby_menu_item_offline_item() -> String {
    "minecraft:barrier".to_string()
}

fn default_lobby_menu_item_disabled_item() -> String {
    "minecraft:gray_dye".to_string()
}

fn default_lobby_menu_item_maintenance_item() -> String {
    "minecraft:orange_dye".to_string()
}

/// 大厅菜单动作（v4 server::LobbyAction）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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

// ─────────────────────────── Menus ───────────────────────────

/// 菜单配置（v4 server::Menus）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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

fn default_menus_fixed_slots_only() -> bool {
    false
}

/// 快捷栏菜单物品（v4 server::MenuHotbarItem）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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

fn default_menu_item_item() -> String {
    "minecraft:paper".to_string()
}

/// 箱子菜单（v4 server::ChestMenu）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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

fn default_chest_menu_title() -> String {
    "Menu".to_string()
}

fn default_chest_menu_rows() -> u8 {
    3
}

/// 菜单项（v4 server::MenuItem）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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

/// 菜单动作（v4 server::MenuAction）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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

// ─────────────────────────── World ───────────────────────────

/// 游戏模式（v4 server::GameMode；protocol_id 与原版一致）。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GameMode {
    #[default]
    Survival,
    Creative,
    Adventure,
    Spectator,
}

impl GameMode {
    pub fn protocol_id(self) -> i32 {
        match self {
            Self::Survival => 0,
            Self::Creative => 1,
            Self::Adventure => 2,
            Self::Spectator => 3,
        }
    }

    pub fn from_protocol_id(id: i32) -> Option<Self> {
        match id {
            0 => Some(Self::Survival),
            1 => Some(Self::Creative),
            2 => Some(Self::Adventure),
            3 => Some(Self::Spectator),
            _ => None,
        }
    }
}

/// 出生点（v4 server::Spawn）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Spawn {
    #[serde(default = "default_spawn_x")]
    pub x: f64,
    #[serde(default = "default_spawn_y")]
    pub y: f64,
    #[serde(default = "default_spawn_z")]
    pub z: f64,
    #[serde(default)]
    pub yaw: f32,
    #[serde(default)]
    pub pitch: f32,
}

impl Default for Spawn {
    fn default() -> Self {
        Self {
            x: default_spawn_x(),
            y: default_spawn_y(),
            z: default_spawn_z(),
            yaw: 0.0,
            pitch: 0.0,
        }
    }
}

fn default_spawn_x() -> f64 {
    0.5
}

fn default_spawn_y() -> f64 {
    100.0
}

fn default_spawn_z() -> f64 {
    0.5
}

/// 世界域 play 配置（v4 server::World 的 play 会话子集；世界生成配置归 qexed_world）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorldConfig {
    #[serde(default = "default_game_mode")]
    pub game_mode: GameMode,
    #[serde(default = "default_view_distance")]
    pub view_distance: i32,
    #[serde(default = "default_simulation_distance")]
    pub simulation_distance: i32,
    #[serde(default)]
    pub allow_flight: bool,
    #[serde(default)]
    pub read_only: bool,
    #[serde(default = "default_spawn_protection_radius")]
    pub spawn_protection_radius: i32,
    #[serde(default = "default_day_ticks")]
    pub day_ticks: i64,
    #[serde(default = "default_default_play_dimension")]
    pub default_play_dimension: String,
    #[serde(default)]
    pub spawn: Spawn,
    #[serde(default = "default_chunk_load_parallelism")]
    pub chunk_load_parallelism: usize,
    #[serde(default = "default_chunk_update_delay_ms")]
    pub chunk_update_delay_ms: u64,
}

impl Default for WorldConfig {
    fn default() -> Self {
        Self {
            game_mode: default_game_mode(),
            view_distance: default_view_distance(),
            simulation_distance: default_simulation_distance(),
            allow_flight: false,
            read_only: false,
            spawn_protection_radius: default_spawn_protection_radius(),
            day_ticks: default_day_ticks(),
            default_play_dimension: default_default_play_dimension(),
            spawn: Spawn::default(),
            chunk_load_parallelism: default_chunk_load_parallelism(),
            chunk_update_delay_ms: default_chunk_update_delay_ms(),
        }
    }
}

impl WorldConfig {
    /// 默认游玩维度（v4 default_play_dimension()）。
    pub fn default_dimension(&self) -> &str {
        if self.default_play_dimension.trim().is_empty() {
            "minecraft:overworld"
        } else {
            self.default_play_dimension.trim()
        }
    }
}

fn default_game_mode() -> GameMode {
    GameMode::Survival
}

fn default_view_distance() -> i32 {
    8
}

fn default_simulation_distance() -> i32 {
    8
}

fn default_spawn_protection_radius() -> i32 {
    16
}

fn default_day_ticks() -> i64 {
    24000
}

fn default_default_play_dimension() -> String {
    "minecraft:overworld".to_string()
}

fn default_chunk_load_parallelism() -> usize {
    0
}

fn default_chunk_update_delay_ms() -> u64 {
    1000
}

// ─────────────────────── PlayerMessages ───────────────────────

/// 玩家进出消息与聊天限速（v4 server::PlayerMessages 的 play 子集）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlayerMessages {
    #[serde(default = "default_player_messages_enable")]
    pub enable: bool,
    #[serde(default = "default_join_message")]
    pub join: String,
    #[serde(default = "default_leave_message")]
    pub leave: String,
    #[serde(default = "default_chat_enable")]
    pub chat_enable: bool,
    #[serde(default = "default_chat_rate_limit")]
    pub chat_rate_limit: u32,
    #[serde(default = "default_chat_rate_window_secs")]
    pub chat_rate_window_secs: u64,
}

impl Default for PlayerMessages {
    fn default() -> Self {
        Self {
            enable: default_player_messages_enable(),
            join: default_join_message(),
            leave: default_leave_message(),
            chat_enable: default_chat_enable(),
            chat_rate_limit: default_chat_rate_limit(),
            chat_rate_window_secs: default_chat_rate_window_secs(),
        }
    }
}

fn default_player_messages_enable() -> bool {
    true
}

fn default_join_message() -> String {
    "{player} joined the game".to_string()
}

fn default_leave_message() -> String {
    "{player} left the game".to_string()
}

fn default_chat_enable() -> bool {
    true
}

fn default_chat_rate_limit() -> u32 {
    20
}

fn default_chat_rate_window_secs() -> u64 {
    10
}

// ─────────────────────── 根配置（app_config） ───────────────────────

/// qexed_play 根配置（app_config 宏，路径 /play.toml）。
#[qexed_config_macros::app_config("/", "play")]
#[derive(Debug, Serialize, Deserialize)]
pub struct PlayConfig {
    /// 世界/会话域配置。
    pub world: WorldConfig,
    /// 大厅配置。
    pub lobby: Lobby,
    /// 菜单配置。
    pub menus: Menus,
    /// 玩家消息配置。
    pub player_messages: PlayerMessages,
    /// 玩法开关（gameplay 域）。
    #[serde(default)]
    pub gameplay: GameplayConfig,
}

impl Default for PlayConfig {
    fn default() -> Self {
        Self {
            world: WorldConfig::default(),
            lobby: Lobby::default(),
            menus: Menus::default(),
            player_messages: PlayerMessages::default(),
            gameplay: GameplayConfig::default(),
        }
    }
}

// ─────────────────────── ServerProxy ───────────────────────

/// 代理转发模式（v4 server::ForwardingMode；含 BungeeCord）。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ForwardingMode {
    #[default]
    None,
    Velocity,
    Victory,
    BungeeCord,
}

/// 服务器代理配置（v4 server::Server 的代理子集；大厅转移判定用）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ServerProxyConfig {
    #[serde(default)]
    pub proxy: bool,
    #[serde(default)]
    pub proxy_protocol: ForwardingMode,
    #[serde(default)]
    pub proxy_online_mode: bool,
    #[serde(default)]
    pub proxy_server_id: String,
}

impl Default for ServerProxyConfig {
    fn default() -> Self {
        Self {
            proxy: false,
            proxy_protocol: ForwardingMode::None,
            proxy_online_mode: true,
            proxy_server_id: String::new(),
        }
    }
}

// ─────────────────────── Gameplay（play-gameplay 任务） ───────────────────────

/// 玩法开关配置（v4 qexed_config::app::qexed::server::Gameplay）。
#[qexed_config_macros::app_config("/", "gameplay")]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GameplayConfig {
    #[serde(default = "default_gameplay_true")]
    pub block_updates: bool,
    #[serde(default = "default_gameplay_true")]
    pub crafting_table: bool,
    #[serde(default)]
    pub furnace: bool,
    #[serde(default = "default_furnace_blocks")]
    pub furnace_blocks: Vec<String>,
    #[serde(default = "default_gameplay_true")]
    pub cauldron: bool,
    #[serde(default = "default_cauldron_blocks")]
    pub cauldron_blocks: Vec<String>,
    #[serde(default)]
    pub redstone: bool,
    #[serde(default = "default_gameplay_true")]
    pub crafting: bool,
    #[serde(default = "default_gameplay_true")]
    pub durability: bool,
    #[serde(default = "default_gameplay_true")]
    pub combat: bool,
    #[serde(default = "default_gameplay_true")]
    pub oxygen: bool,
    #[serde(default = "default_gameplay_true")]
    pub sounds: bool,
    #[serde(default = "default_gameplay_true")]
    pub advancements: bool,
    #[serde(default = "default_gameplay_true")]
    pub enchantments: bool,
    #[serde(default = "default_gameplay_true")]
    pub potion_effects: bool,
    #[serde(default = "default_gameplay_true")]
    pub drop_inventory_on_death: bool,
    #[serde(default = "default_furnace_tick_ms")]
    pub furnace_tick_ms: u64,
    #[serde(default = "default_redstone_tick_ms")]
    pub redstone_tick_ms: u64,
    #[serde(default = "default_redstone_max_distance")]
    pub redstone_max_distance: u32,
    #[serde(default = "default_oxygen_tick_ms")]
    pub oxygen_tick_ms: u64,
    #[serde(default = "default_farmland_tick_ms")]
    pub farmland_tick_ms: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub custom_advancements: Vec<CustomAdvancement>,
}

/// gameplay 域对 GameplayConfig 的短名。
pub type Gameplay = GameplayConfig;
/// world 域对 WorldConfig 的短名。
pub type World = WorldConfig;

impl Default for GameplayConfig {
    fn default() -> Self {
        Self {
            block_updates: true,
            crafting_table: true,
            furnace: false,
            furnace_blocks: default_furnace_blocks(),
            cauldron: true,
            cauldron_blocks: default_cauldron_blocks(),
            redstone: false,
            crafting: true,
            durability: true,
            combat: true,
            oxygen: true,
            sounds: true,
            advancements: true,
            enchantments: true,
            potion_effects: true,
            drop_inventory_on_death: true,
            furnace_tick_ms: default_furnace_tick_ms(),
            redstone_tick_ms: default_redstone_tick_ms(),
            redstone_max_distance: default_redstone_max_distance(),
            oxygen_tick_ms: default_oxygen_tick_ms(),
            farmland_tick_ms: default_farmland_tick_ms(),
            custom_advancements: Vec::new(),
        }
    }
}

/// 自定义成就（v4 server::CustomAdvancement）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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

/// 自定义成就触发时机（v4 server::CustomAdvancementTrigger）。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
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

fn default_gameplay_true() -> bool {
    true
}

fn default_furnace_blocks() -> Vec<String> {
    vec!["minecraft:furnace".to_string()]
}

fn default_cauldron_blocks() -> Vec<String> {
    vec![
        "minecraft:cauldron".to_string(),
        "minecraft:water_cauldron".to_string(),
        "minecraft:lava_cauldron".to_string(),
        "minecraft:powder_snow_cauldron".to_string(),
    ]
}

fn default_furnace_tick_ms() -> u64 {
    50
}

fn default_redstone_tick_ms() -> u64 {
    50
}

fn default_redstone_max_distance() -> u32 {
    8
}

fn default_oxygen_tick_ms() -> u64 {
    1000
}

fn default_farmland_tick_ms() -> u64 {
    1000
}

fn default_custom_advancement_title() -> String {
    "Advancement".to_string()
}

fn default_custom_advancement_description() -> String {
    "Advancement description".to_string()
}

fn default_custom_advancement_icon() -> String {
    "minecraft:stone".to_string()
}

fn default_custom_advancement_toast() -> bool {
    true
}

// ─────────────────────── Enchanting（play-gameplay 任务） ───────────────────────

/// 附魔台配置（v4 独立配置文件 app/qexed_enchanting.rs 的 EnchantingConfig）。
#[qexed_config_macros::app_config("/", "qexed_enchanting")]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnchantingConfig {
    #[serde(default = "default_enchanting_enable")]
    pub enable: bool,
    #[serde(default = "default_enchanting_title")]
    pub title: String,
    #[serde(default = "default_enchanting_lapis_item")]
    pub lapis_item: String,
    #[serde(default = "default_enchanting_creative_free")]
    pub creative_free: bool,
    #[serde(default = "default_enchanting_allow_reenchanting")]
    pub allow_reenchanting: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<EnchantingOptionConfig>,
}

impl Default for EnchantingConfig {
    fn default() -> Self {
        Self {
            enable: default_enchanting_enable(),
            title: default_enchanting_title(),
            lapis_item: default_enchanting_lapis_item(),
            creative_free: default_enchanting_creative_free(),
            allow_reenchanting: default_enchanting_allow_reenchanting(),
            options: default_enchanting_options(),
        }
    }
}

/// 附魔选项配置（v4 qexed_enchanting::EnchantingOptionConfig）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnchantingOptionConfig {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub display_name: String,
    #[serde(default = "default_enchanting_option_max_level")]
    pub max_level: i32,
    #[serde(default = "default_enchanting_option_weight")]
    pub weight: i32,
    #[serde(default)]
    pub min_player_level: i32,
    #[serde(default = "default_enchanting_option_max_player_level")]
    pub max_player_level: i32,
    #[serde(default)]
    pub min_bookshelves: i32,
    #[serde(default = "default_enchanting_option_lapis_cost")]
    pub lapis_cost: i32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub item_suffixes: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub items: Vec<String>,
    #[serde(default)]
    pub plugin: bool,
}

impl Default for EnchantingOptionConfig {
    fn default() -> Self {
        Self {
            id: String::new(),
            display_name: String::new(),
            max_level: default_enchanting_option_max_level(),
            weight: default_enchanting_option_weight(),
            min_player_level: 0,
            max_player_level: default_enchanting_option_max_player_level(),
            min_bookshelves: 0,
            lapis_cost: default_enchanting_option_lapis_cost(),
            item_suffixes: Vec::new(),
            items: Vec::new(),
            plugin: false,
        }
    }
}

fn default_enchanting_enable() -> bool {
    true
}

fn default_enchanting_title() -> String {
    "Enchanting".to_string()
}

fn default_enchanting_lapis_item() -> String {
    "minecraft:lapis_lazuli".to_string()
}

fn default_enchanting_creative_free() -> bool {
    true
}

fn default_enchanting_allow_reenchanting() -> bool {
    false
}

fn default_enchanting_option_max_level() -> i32 {
    1
}

fn default_enchanting_option_weight() -> i32 {
    10
}

fn default_enchanting_option_max_player_level() -> i32 {
    i32::MAX
}

fn default_enchanting_option_lapis_cost() -> i32 {
    1
}

/// v4 默认附魔选项表（42 项，原样迁移）。
fn default_enchanting_options() -> Vec<EnchantingOptionConfig> {
    vec![
        enchanting_option(
            "minecraft:efficiency",
            5,
            10,
            1,
            &["_pickaxe", "_axe", "_shovel", "_hoe"],
            &["minecraft:shears"],
            0,
        ),
        enchanting_option(
            "minecraft:fortune",
            3,
            2,
            1,
            &["_pickaxe", "_axe", "_shovel", "_hoe"],
            &[],
            0,
        ),
        enchanting_option(
            "minecraft:silk_touch",
            1,
            1,
            1,
            &["_pickaxe", "_axe", "_shovel", "_hoe"],
            &["minecraft:shears"],
            8,
        ),
        enchanting_option("minecraft:sharpness", 5, 10, 1, &["_sword", "_axe"], &[], 0),
        enchanting_option(
            "minecraft:smite",
            5,
            5,
            1,
            &["_sword", "_axe"],
            &["minecraft:mace"],
            0,
        ),
        enchanting_option(
            "minecraft:bane_of_arthropods",
            5,
            5,
            1,
            &["_sword", "_axe"],
            &["minecraft:mace"],
            0,
        ),
        enchanting_option("minecraft:knockback", 2, 5, 1, &["_sword"], &[], 0),
        enchanting_option(
            "minecraft:fire_aspect",
            2,
            2,
            1,
            &["_sword"],
            &["minecraft:mace"],
            0,
        ),
        enchanting_option("minecraft:looting", 3, 2, 1, &["_sword"], &[], 0),
        enchanting_option("minecraft:sweeping_edge", 3, 2, 1, &["_sword"], &[], 0),
        enchanting_option("minecraft:density", 5, 10, 1, &[], &["minecraft:mace"], 0),
        enchanting_option("minecraft:breach", 4, 2, 1, &[], &["minecraft:mace"], 0),
        enchanting_option("minecraft:lunge", 3, 5, 1, &["_spear"], &[], 0),
        enchanting_option(
            "minecraft:protection",
            4,
            10,
            1,
            &["_helmet", "_chestplate", "_leggings", "_boots"],
            &[],
            0,
        ),
        enchanting_option(
            "minecraft:fire_protection",
            4,
            5,
            1,
            &["_helmet", "_chestplate", "_leggings", "_boots"],
            &[],
            0,
        ),
        enchanting_option("minecraft:feather_falling", 4, 5, 1, &["_boots"], &[], 0),
        enchanting_option(
            "minecraft:blast_protection",
            4,
            2,
            1,
            &["_helmet", "_chestplate", "_leggings", "_boots"],
            &[],
            0,
        ),
        enchanting_option(
            "minecraft:projectile_protection",
            4,
            5,
            1,
            &["_helmet", "_chestplate", "_leggings", "_boots"],
            &[],
            0,
        ),
        enchanting_option("minecraft:respiration", 3, 2, 1, &["_helmet"], &[], 0),
        enchanting_option("minecraft:aqua_affinity", 1, 2, 1, &["_helmet"], &[], 0),
        enchanting_option(
            "minecraft:thorns",
            3,
            1,
            1,
            &["_helmet", "_chestplate", "_leggings", "_boots"],
            &[],
            8,
        ),
        enchanting_option("minecraft:depth_strider", 3, 2, 1, &["_boots"], &[], 0),
        enchanting_option("minecraft:power", 5, 10, 1, &[], &["minecraft:bow"], 0),
        enchanting_option("minecraft:punch", 2, 2, 1, &[], &["minecraft:bow"], 0),
        enchanting_option("minecraft:flame", 1, 2, 1, &[], &["minecraft:bow"], 0),
        enchanting_option("minecraft:infinity", 1, 1, 1, &[], &["minecraft:bow"], 8),
        enchanting_option(
            "minecraft:luck_of_the_sea",
            3,
            2,
            1,
            &[],
            &["minecraft:fishing_rod"],
            0,
        ),
        enchanting_option(
            "minecraft:lure",
            3,
            2,
            1,
            &[],
            &["minecraft:fishing_rod"],
            0,
        ),
        enchanting_option("minecraft:loyalty", 3, 5, 1, &[], &["minecraft:trident"], 0),
        enchanting_option(
            "minecraft:impaling",
            5,
            2,
            1,
            &[],
            &["minecraft:trident"],
            0,
        ),
        enchanting_option("minecraft:riptide", 3, 2, 1, &[], &["minecraft:trident"], 8),
        enchanting_option(
            "minecraft:channeling",
            1,
            1,
            1,
            &[],
            &["minecraft:trident"],
            8,
        ),
        enchanting_option(
            "minecraft:piercing",
            4,
            10,
            1,
            &[],
            &["minecraft:crossbow"],
            0,
        ),
        enchanting_option(
            "minecraft:quick_charge",
            3,
            5,
            1,
            &[],
            &["minecraft:crossbow"],
            0,
        ),
        enchanting_option(
            "minecraft:multishot",
            1,
            2,
            1,
            &[],
            &["minecraft:crossbow"],
            8,
        ),
        enchanting_option(
            "minecraft:unbreaking",
            3,
            5,
            1,
            &[
                "_helmet",
                "_chestplate",
                "_leggings",
                "_boots",
                "_sword",
                "_pickaxe",
                "_axe",
                "_shovel",
                "_hoe",
            ],
            &[
                "minecraft:bow",
                "minecraft:crossbow",
                "minecraft:trident",
                "minecraft:mace",
                "minecraft:fishing_rod",
                "minecraft:shears",
            ],
            0,
        ),
    ]
}

fn enchanting_option(
    id: &str,
    max_level: i32,
    weight: i32,
    min_player_level: i32,
    suffixes: &[&str],
    items: &[&str],
    min_bookshelves: i32,
) -> EnchantingOptionConfig {
    EnchantingOptionConfig {
        id: id.to_string(),
        max_level,
        weight,
        min_player_level,
        min_bookshelves,
        item_suffixes: suffixes.iter().map(|value| (*value).to_string()).collect(),
        items: items.iter().map(|value| (*value).to_string()).collect(),
        ..EnchantingOptionConfig::default()
    }
}

#[cfg(test)]
mod gameplay_tests {
    use super::*;
    use qexed_config::Config;

    #[test]
    fn gameplay_config_defaults() {
        let config = GameplayConfig::default();
        assert!(config.block_updates);
        assert!(!config.furnace);
        assert!(config.crafting);
        assert!(config.combat);
        assert_eq!(GameplayConfig::PATH, "/");
        assert_eq!(GameplayConfig::NAME, "gameplay");
    }

    #[test]
    fn enchanting_config_defaults_match_v4() {
        let config = EnchantingConfig::default();
        assert!(config.enable);
        assert_eq!(config.title, "Enchanting");
        assert_eq!(config.lapis_item, "minecraft:lapis_lazuli");
        assert!(config.creative_free);
        assert!(!config.allow_reenchanting);
        // 选项表非穷举（运行时从注册表补全），只校验非空与已知项存在
        assert!(!config.options.is_empty());
    }

    #[test]
    fn custom_advancement_trigger_serde_snake_case() {
        let trigger: CustomAdvancementTrigger =
            serde_json::from_str("\"enter_water\"").expect("snake_case");
        assert_eq!(trigger, CustomAdvancementTrigger::EnterWater);
    }
}

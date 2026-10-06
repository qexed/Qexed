//! 管理后端能力面（服务器真实状态的 trait 抽象）。
//!
//! ManagementServer 只做协议编解码与会话管理；全部状态读写经此 trait
//! 转发给 qexed 组装层实现（players/warden/配置/存档）。

use crate::model::{
    AllowlistEntry, Difficulty, GameMode, IpBan, KickRequest, OperatorEntry, PlayerRef,
    ServerState, SystemMessage, TypedGameRule, UserBan,
};
use crate::Result;

/// 同步读 + 异步写的管理能力集。
///
/// 设计约定：读写均为非阻塞快照语义——后端实现自行加锁，
/// 协议层不假设一致性跨度（每个调用独立生效）。
pub trait ManagementBackend: Send + Sync {
    // ── allowlist ──
    fn allowlist(&self) -> Result<Vec<AllowlistEntry>>;
    fn allowlist_set(&self, players: Vec<AllowlistEntry>) -> Result<Vec<AllowlistEntry>>;
    fn allowlist_add(&self, add: Vec<AllowlistEntry>) -> Result<Vec<AllowlistEntry>>;
    fn allowlist_remove(&self, remove: Vec<AllowlistEntry>) -> Result<Vec<AllowlistEntry>>;
    fn allowlist_clear(&self) -> Result<Vec<AllowlistEntry>>;
    fn use_allowlist(&self) -> Result<bool>;
    fn set_use_allowlist(&self, use_it: bool) -> Result<bool>;
    fn enforce_allowlist(&self) -> Result<bool>;
    fn set_enforce_allowlist(&self, enforce: bool) -> Result<bool>;

    // ── bans ──
    fn bans(&self) -> Result<Vec<UserBan>>;
    fn bans_set(&self, bans: Vec<UserBan>) -> Result<Vec<UserBan>>;
    fn bans_add(&self, add: Vec<UserBan>) -> Result<Vec<UserBan>>;
    fn bans_remove(&self, remove: Vec<PlayerRef>) -> Result<Vec<UserBan>>;
    fn bans_clear(&self) -> Result<Vec<UserBan>>;

    // ── ip_bans ──
    fn ip_bans(&self) -> Result<Vec<IpBan>>;
    fn ip_bans_set(&self, bans: Vec<IpBan>) -> Result<Vec<IpBan>>;
    fn ip_bans_add(&self, add: Vec<IpBan>) -> Result<Vec<IpBan>>;
    fn ip_bans_remove(&self, ips: Vec<String>) -> Result<Vec<IpBan>>;
    fn ip_bans_clear(&self) -> Result<Vec<IpBan>>;

    // ── players ──
    fn players(&self) -> Result<Vec<PlayerRef>>;
    fn players_kick(&self, kick: Vec<KickRequest>) -> Result<Vec<PlayerRef>>;

    // ── operators ──
    fn operators(&self) -> Result<Vec<OperatorEntry>>;
    fn operators_set(&self, ops: Vec<OperatorEntry>) -> Result<Vec<OperatorEntry>>;
    fn operators_add(&self, add: Vec<OperatorEntry>) -> Result<Vec<OperatorEntry>>;
    fn operators_remove(&self, remove: Vec<PlayerRef>) -> Result<Vec<OperatorEntry>>;
    fn operators_clear(&self) -> Result<Vec<OperatorEntry>>;
    fn operator_user_permission_level(&self) -> Result<i32>;
    fn set_operator_user_permission_level(&self, level: i32) -> Result<i32>;

    // ── server ──
    fn server_status(&self) -> Result<ServerState>;
    fn server_save(&self, flush: bool) -> Result<bool>;
    fn server_stop(&self) -> Result<bool>;
    fn server_system_message(&self, message: SystemMessage) -> Result<bool>;

    // ── serversettings ──
    fn autosave(&self) -> Result<bool>;
    fn set_autosave(&self, enable: bool) -> Result<bool>;
    fn difficulty(&self) -> Result<Difficulty>;
    fn set_difficulty(&self, difficulty: Difficulty) -> Result<Difficulty>;
    fn max_players(&self) -> Result<i32>;
    fn set_max_players(&self, max: i32) -> Result<i32>;
    fn motd(&self) -> Result<String>;
    fn set_motd(&self, message: String) -> Result<String>;
    fn spawn_protection_radius(&self) -> Result<i32>;
    fn set_spawn_protection_radius(&self, radius: i32) -> Result<i32>;
    fn force_game_mode(&self) -> Result<bool>;
    fn set_force_game_mode(&self, force: bool) -> Result<bool>;
    fn game_mode(&self) -> Result<GameMode>;
    fn set_game_mode(&self, mode: GameMode) -> Result<GameMode>;
    fn view_distance(&self) -> Result<i32>;
    fn set_view_distance(&self, distance: i32) -> Result<i32>;
    fn simulation_distance(&self) -> Result<i32>;
    fn set_simulation_distance(&self, distance: i32) -> Result<i32>;
    fn allow_flight(&self) -> Result<bool>;
    fn set_allow_flight(&self, allowed: bool) -> Result<bool>;
    fn player_idle_timeout(&self) -> Result<i32>;
    fn set_player_idle_timeout(&self, seconds: i32) -> Result<i32>;
    fn pause_when_empty_seconds(&self) -> Result<i32>;
    fn set_pause_when_empty_seconds(&self, seconds: i32) -> Result<i32>;
    fn hide_online_players(&self) -> Result<bool>;
    fn set_hide_online_players(&self, hide: bool) -> Result<bool>;
    fn status_replies(&self) -> Result<bool>;
    fn set_status_replies(&self, enable: bool) -> Result<bool>;
    fn entity_broadcast_range(&self) -> Result<i32>;
    fn set_entity_broadcast_range(&self, percentage_points: i32) -> Result<i32>;
    fn status_heartbeat_interval(&self) -> Result<i64>;
    fn set_status_heartbeat_interval(&self, seconds: i64) -> Result<i64>;
    fn accept_transfers(&self) -> Result<bool>;
    fn set_accept_transfers(&self, accept: bool) -> Result<bool>;

    // ── gamerules ──
    fn gamerules(&self) -> Result<Vec<TypedGameRule>>;
    fn gamerule_update(&self, rule: crate::model::UntypedGameRule) -> Result<TypedGameRule>;
}

/// operators_remove 的参数别名（Vec<PlayerRef>）。
pub type PlayerRefs = Vec<PlayerRef>;

/// 空实现（协议自测/未接线时用；全部返回 NOT_IMPLEMENTED 语义错误）。
pub struct NoBackend;

macro_rules! unimplemented_fn {
    ($($name:ident($($arg:ident: $ty:ty),*) -> $ret:ty),* $(,)?) => {
        $(
            fn $name(&self, $($arg: $ty),*) -> crate::Result<$ret> {
                Err(crate::ManagementError::msg(concat!("method not wired: ", stringify!($name))))
            }
        )*
    };
}

#[allow(unused_variables)]
impl ManagementBackend for NoBackend {
    unimplemented_fn! {
        allowlist() -> Vec<AllowlistEntry>,
        allowlist_set(players: Vec<AllowlistEntry>) -> Vec<AllowlistEntry>,
        allowlist_add(add: Vec<AllowlistEntry>) -> Vec<AllowlistEntry>,
        allowlist_remove(remove: Vec<AllowlistEntry>) -> Vec<AllowlistEntry>,
        allowlist_clear() -> Vec<AllowlistEntry>,
        use_allowlist() -> bool,
        set_use_allowlist(use_it: bool) -> bool,
        enforce_allowlist() -> bool,
        set_enforce_allowlist(enforce: bool) -> bool,
        bans() -> Vec<UserBan>,
        bans_set(bans: Vec<UserBan>) -> Vec<UserBan>,
        bans_add(add: Vec<UserBan>) -> Vec<UserBan>,
        bans_remove(remove: Vec<PlayerRef>) -> Vec<UserBan>,
        bans_clear() -> Vec<UserBan>,
        ip_bans() -> Vec<IpBan>,
        ip_bans_set(bans: Vec<IpBan>) -> Vec<IpBan>,
        ip_bans_add(add: Vec<IpBan>) -> Vec<IpBan>,
        ip_bans_remove(ips: Vec<String>) -> Vec<IpBan>,
        ip_bans_clear() -> Vec<IpBan>,
        players() -> Vec<PlayerRef>,
        players_kick(kick: Vec<KickRequest>) -> Vec<PlayerRef>,
        operators() -> Vec<OperatorEntry>,
        operators_set(ops: Vec<OperatorEntry>) -> Vec<OperatorEntry>,
        operators_add(add: Vec<OperatorEntry>) -> Vec<OperatorEntry>,
        operators_remove(remove: Vec<PlayerRef>) -> Vec<OperatorEntry>,
        operators_clear() -> Vec<OperatorEntry>,
        operator_user_permission_level() -> i32,
        set_operator_user_permission_level(level: i32) -> i32,
        server_status() -> ServerState,
        server_save(flush: bool) -> bool,
        server_stop() -> bool,
        server_system_message(message: SystemMessage) -> bool,
        autosave() -> bool,
        set_autosave(enable: bool) -> bool,
        difficulty() -> Difficulty,
        set_difficulty(difficulty: Difficulty) -> Difficulty,
        max_players() -> i32,
        set_max_players(max: i32) -> i32,
        motd() -> String,
        set_motd(message: String) -> String,
        spawn_protection_radius() -> i32,
        set_spawn_protection_radius(radius: i32) -> i32,
        force_game_mode() -> bool,
        set_force_game_mode(force: bool) -> bool,
        game_mode() -> GameMode,
        set_game_mode(mode: GameMode) -> GameMode,
        view_distance() -> i32,
        set_view_distance(distance: i32) -> i32,
        simulation_distance() -> i32,
        set_simulation_distance(distance: i32) -> i32,
        allow_flight() -> bool,
        set_allow_flight(allowed: bool) -> bool,
        player_idle_timeout() -> i32,
        set_player_idle_timeout(seconds: i32) -> i32,
        pause_when_empty_seconds() -> i32,
        set_pause_when_empty_seconds(seconds: i32) -> i32,
        hide_online_players() -> bool,
        set_hide_online_players(hide: bool) -> bool,
        status_replies() -> bool,
        set_status_replies(enable: bool) -> bool,
        entity_broadcast_range() -> i32,
        set_entity_broadcast_range(percentage_points: i32) -> i32,
        status_heartbeat_interval() -> i64,
        set_status_heartbeat_interval(seconds: i64) -> i64,
        accept_transfers() -> bool,
        set_accept_transfers(accept: bool) -> bool,
        gamerules() -> Vec<TypedGameRule>,
        gamerule_update(rule: crate::model::UntypedGameRule) -> TypedGameRule,
    }
}

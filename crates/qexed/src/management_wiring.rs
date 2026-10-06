//! 管理协议的组装层后端：桥 PlayerManager / WardenManager / 配置。

use std::sync::Arc;

use qexed_management::model::{
    AllowlistEntry, Difficulty, GameMode, IpBan, KickRequest, OperatorEntry, PlayerRef,
    ServerState, SystemMessage, TypedGameRule, UserBan,
};
use qexed_management::{ManagementBackend, Result};
use qexed_player::PlayerManager;
use qexed_server::warden::WardenManager;

macro_rules! wiring_todo {
    ($($name:ident($($arg:ident: $ty:ty),*) -> $ret:ty),* $(,)?) => {
        $(
            fn $name(&self, $($arg: $ty),*) -> Result<$ret> {
                Err(qexed_management::ManagementError::msg(concat!("not wired: ", stringify!($name))))
            }
        )*
    };
}


pub struct QexedManagementBackend {
    pub players: Arc<PlayerManager>,
    pub warden: Arc<WardenManager>,
    pub motd: Arc<std::sync::Mutex<String>>,
    pub version_name: &'static str,
    pub protocol_version: i32,
}

impl QexedManagementBackend {
    fn snapshot_players(&self) -> Vec<PlayerRef> {
        self.players
            .online_names()
            .into_iter()
            .map(|name| PlayerRef::by_name(name))
            .collect()
    }
}

impl ManagementBackend for QexedManagementBackend {
    fn players(&self) -> Result<Vec<PlayerRef>> {
        Ok(self.snapshot_players())
    }

    fn server_status(&self) -> Result<ServerState> {
        Ok(ServerState {
            started: true,
            players: self.snapshot_players(),
            version: qexed_management::VersionInfo {
                name: self.version_name.to_string(),
                protocol: self.protocol_version,
            },
        })
    }

    fn motd(&self) -> Result<String> {
        Ok(self.motd.lock().expect("motd poisoned").clone())
    }

    fn set_motd(&self, message: String) -> Result<String> {
        *self.motd.lock().expect("motd poisoned") = message.clone();
        Ok(message)
    }

    fn difficulty(&self) -> Result<Difficulty> { Ok(Difficulty::Normal) }
    fn game_mode(&self) -> Result<GameMode> { Ok(GameMode::Survival) }
    fn autosave(&self) -> Result<bool> { Ok(true) }
    fn view_distance(&self) -> Result<i32> { Ok(8) }
    fn simulation_distance(&self) -> Result<i32> { Ok(8) }
    fn max_players(&self) -> Result<i32> { Ok(-1) }
    fn allow_flight(&self) -> Result<bool> { Ok(false) }
    fn use_allowlist(&self) -> Result<bool> { Ok(false) }
    fn enforce_allowlist(&self) -> Result<bool> { Ok(false) }
    fn hide_online_players(&self) -> Result<bool> { Ok(false) }
    fn status_replies(&self) -> Result<bool> { Ok(true) }
    fn accept_transfers(&self) -> Result<bool> { Ok(false) }
    fn spawn_protection_radius(&self) -> Result<i32> { Ok(0) }
    fn force_game_mode(&self) -> Result<bool> { Ok(false) }
    fn player_idle_timeout(&self) -> Result<i32> { Ok(0) }
    fn pause_when_empty_seconds(&self) -> Result<i32> { Ok(0) }
    fn entity_broadcast_range(&self) -> Result<i32> { Ok(100) }
    fn operator_user_permission_level(&self) -> Result<i32> { Ok(2) }
    fn status_heartbeat_interval(&self) -> Result<i64> { Ok(10) }
    fn gamerules(&self) -> Result<Vec<TypedGameRule>> { Ok(Vec::new()) }
    fn bans(&self) -> Result<Vec<UserBan>> { Ok(Vec::new()) }

    fn server_stop(&self) -> Result<bool> {
        Err(qexed_management::ManagementError::msg("stop not wired"))
    }
    fn server_save(&self, _flush: bool) -> Result<bool> {
        Err(qexed_management::ManagementError::msg("save not wired"))
    }
    fn players_kick(&self, _kick: Vec<KickRequest>) -> Result<Vec<PlayerRef>> {
        Err(qexed_management::ManagementError::msg("kick not wired"))
    }

    wiring_todo! {
        allowlist() -> Vec<AllowlistEntry>,
        allowlist_set(players: Vec<AllowlistEntry>) -> Vec<AllowlistEntry>,
        allowlist_add(add: Vec<AllowlistEntry>) -> Vec<AllowlistEntry>,
        allowlist_remove(remove: Vec<AllowlistEntry>) -> Vec<AllowlistEntry>,
        allowlist_clear() -> Vec<AllowlistEntry>,
        set_use_allowlist(use_it: bool) -> bool,
        set_enforce_allowlist(enforce: bool) -> bool,
        bans_set(bans: Vec<UserBan>) -> Vec<UserBan>,
        bans_add(add: Vec<UserBan>) -> Vec<UserBan>,
        bans_remove(remove: Vec<PlayerRef>) -> Vec<UserBan>,
        bans_clear() -> Vec<UserBan>,
        ip_bans() -> Vec<IpBan>,
        ip_bans_set(bans: Vec<IpBan>) -> Vec<IpBan>,
        ip_bans_add(add: Vec<IpBan>) -> Vec<IpBan>,
        ip_bans_remove(ips: Vec<String>) -> Vec<IpBan>,
        ip_bans_clear() -> Vec<IpBan>,
        operators() -> Vec<OperatorEntry>,
        operators_set(ops: Vec<OperatorEntry>) -> Vec<OperatorEntry>,
        operators_add(add: Vec<OperatorEntry>) -> Vec<OperatorEntry>,
        operators_remove(remove: Vec<PlayerRef>) -> Vec<OperatorEntry>,
        operators_clear() -> Vec<OperatorEntry>,
        set_operator_user_permission_level(level: i32) -> i32,
        server_system_message(message: SystemMessage) -> bool,
        set_autosave(enable: bool) -> bool,
        set_difficulty(difficulty: Difficulty) -> Difficulty,
        set_max_players(max: i32) -> i32,
        set_spawn_protection_radius(radius: i32) -> i32,
        set_force_game_mode(force: bool) -> bool,
        set_game_mode(mode: GameMode) -> GameMode,
        set_view_distance(distance: i32) -> i32,
        set_simulation_distance(distance: i32) -> i32,
        set_allow_flight(allowed: bool) -> bool,
        set_player_idle_timeout(seconds: i32) -> i32,
        set_pause_when_empty_seconds(seconds: i32) -> i32,
        set_hide_online_players(hide: bool) -> bool,
        set_status_replies(enable: bool) -> bool,
        set_entity_broadcast_range(percentage_points: i32) -> i32,
        set_accept_transfers(accept: bool) -> bool,
        set_status_heartbeat_interval(seconds: i64) -> i64,
        gamerule_update(rule: qexed_management::model::UntypedGameRule) -> TypedGameRule,
    }
}

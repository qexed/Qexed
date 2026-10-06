//! 玩家离场守卫（v4 play/session.rs 迁移）。
//!
//! v6 适配：PlayerManager 来自 qexed_player、PluginManager 来自 qexed_plugins，
//! 插件事件 payload 需要 OnlinePlayer -> PlayerPayload 的转换。

// play-gameplay 接线前部分 API 暂未被本 crate 引用（供其改造 runtime.rs 时使用）。
#![allow(dead_code)]


use qexed_player::{OnlinePlayer, PlayerManager};
use qexed_plugins::{
    PluginManager,
    api::{PlayerPayload, PlayerPayloadOwned, PlayerPositionPayload},
};

pub struct PlayerLeaveGuard<'a> {
    players: &'a PlayerManager,
    plugins: &'a PluginManager,
    player: OnlinePlayer,
    active: bool,
}

impl<'a> PlayerLeaveGuard<'a> {
    pub fn new(
        players: &'a PlayerManager,
        plugins: &'a PluginManager,
        player: OnlinePlayer,
    ) -> Self {
        Self {
            players,
            plugins,
            player,
            active: true,
        }
    }

    pub fn leave(mut self) {
        self.leave_inner();
        self.active = false;
    }

    fn leave_inner(&self) {
        self.plugins
            .remove_geyser_player_info(&self.player.profile.uuid.to_string(), &self.player.profile.username);
        self.plugins.emit_player_leave(&player_payload(&self.player));
        self.players.leave(self.player.profile.uuid);
    }
}

impl Drop for PlayerLeaveGuard<'_> {
    fn drop(&mut self) {
        if self.active {
            self.leave_inner();
        }
    }
}

/// OnlinePlayer -> 插件事件 payload（v4 直接传 OnlinePlayer；v6 插件 ABI 用序列化 payload）。
pub fn player_payload(player: &OnlinePlayer) -> PlayerPayload {
    PlayerPayload {
        uuid: player.profile.uuid.to_string(),
        username: player.profile.username.clone(),
        entity_id: player.entity_id,
        language: player.language.clone(),
        dimension: player.dimension.clone(),
    }
}

/// OnlinePlayer -> 插件事件 owned payload（v4 qexed_plugin_api::player_payload_owned）。
pub fn player_payload_owned(player: &OnlinePlayer) -> PlayerPayloadOwned {
    PlayerPayloadOwned {
        uuid: player.profile.uuid.to_string(),
        username: player.profile.username.clone(),
        entity_id: player.entity_id,
        language: player.language.clone(),
        dimension: player.dimension.clone(),
    }
}

/// EntityPosition -> 插件事件位置 payload（v4 qexed_plugin_api::player_position_payload）。
pub fn player_position_payload(position: qexed_protocol::types::EntityPosition) -> PlayerPositionPayload {
    PlayerPositionPayload {
        x: position.x,
        y: position.y,
        z: position.z,
        yaw: position.yaw,
        pitch: position.pitch,
        on_ground: position.on_ground,
    }
}

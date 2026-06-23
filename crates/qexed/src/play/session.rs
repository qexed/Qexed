use crate::players::{OnlinePlayer, PlayerManager};

pub(super) struct PlayerLeaveGuard<'a> {
    players: &'a PlayerManager,
    plugins: &'a crate::plugins::PluginManager,
    player: OnlinePlayer,
    active: bool,
}

impl<'a> PlayerLeaveGuard<'a> {
    pub(super) fn new(
        players: &'a PlayerManager,
        plugins: &'a crate::plugins::PluginManager,
        player: OnlinePlayer,
    ) -> Self {
        Self {
            players,
            plugins,
            player,
            active: true,
        }
    }

    pub(super) fn leave(mut self) {
        self.leave_inner();
        self.active = false;
    }

    fn leave_inner(&self) {
        self.plugins.remove_geyser_player_info(&self.player);
        self.plugins.emit_player_leave(&self.player);
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

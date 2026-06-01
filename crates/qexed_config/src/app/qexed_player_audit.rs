use serde::{Deserialize, Serialize};

use crate::app::qexed::server::PlayerAudit;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QexedPlayerAudit {
    pub player_audit: PlayerAudit,
}

impl Default for QexedPlayerAudit {
    fn default() -> Self {
        Self {
            player_audit: PlayerAudit::default(),
        }
    }
}

impl qexed_config::tool::AppConfigTrait for QexedPlayerAudit {
    const PATH: &'static str = "/";
    const NAME: &'static str = "qexed_player_audit";
}

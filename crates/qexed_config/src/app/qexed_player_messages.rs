use serde::{Deserialize, Serialize};

use crate::app::qexed::server::PlayerMessages;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QexedPlayerMessages {
    pub player_messages: PlayerMessages,
}

impl Default for QexedPlayerMessages {
    fn default() -> Self {
        Self {
            player_messages: PlayerMessages::default(),
        }
    }
}

impl qexed_config::tool::AppConfigTrait for QexedPlayerMessages {
    const PATH: &'static str = "/";
    const NAME: &'static str = "qexed_player_messages";
}

use serde::{Deserialize, Serialize};

use crate::app::qexed::server::PlayerData;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QexedPlayerData {
    pub player_data: PlayerData,
}

impl Default for QexedPlayerData {
    fn default() -> Self {
        Self {
            player_data: PlayerData::default(),
        }
    }
}

impl qexed_config::tool::AppConfigTrait for QexedPlayerData {
    const PATH: &'static str = "/";
    const NAME: &'static str = "qexed_player_data";
}

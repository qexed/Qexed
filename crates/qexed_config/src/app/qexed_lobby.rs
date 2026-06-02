use serde::{Deserialize, Serialize};

use crate::app::qexed::server::Lobby;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QexedLobby {
    pub lobby: Lobby,
}

impl Default for QexedLobby {
    fn default() -> Self {
        Self {
            lobby: Lobby::default(),
        }
    }
}

impl qexed_config::tool::AppConfigTrait for QexedLobby {
    const PATH: &'static str = "/";
    const NAME: &'static str = "qexed_lobby";
}

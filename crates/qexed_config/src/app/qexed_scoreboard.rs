use serde::{Deserialize, Serialize};

use crate::app::qexed::server::Scoreboard;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QexedScoreboard {
    pub scoreboard: Scoreboard,
}

impl Default for QexedScoreboard {
    fn default() -> Self {
        Self {
            scoreboard: Scoreboard::default(),
        }
    }
}

impl qexed_config::tool::AppConfigTrait for QexedScoreboard {
    const PATH: &'static str = "/";
    const NAME: &'static str = "qexed_scoreboard";
}

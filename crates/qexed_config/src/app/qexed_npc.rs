use serde::{Deserialize, Serialize};

use crate::app::qexed::server::Npcs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QexedNpc {
    pub npcs: Npcs,
}

impl Default for QexedNpc {
    fn default() -> Self {
        Self {
            npcs: Npcs::default(),
        }
    }
}

impl qexed_config::tool::AppConfigTrait for QexedNpc {
    const PATH: &'static str = "/";
    const NAME: &'static str = "qexed_npc";
}

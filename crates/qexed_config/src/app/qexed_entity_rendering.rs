use serde::{Deserialize, Serialize};

use crate::app::qexed::server::EntityRendering;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QexedEntityRendering {
    pub entity_rendering: EntityRendering,
}

impl Default for QexedEntityRendering {
    fn default() -> Self {
        Self {
            entity_rendering: EntityRendering::default(),
        }
    }
}

impl qexed_config::tool::AppConfigTrait for QexedEntityRendering {
    const PATH: &'static str = "/";
    const NAME: &'static str = "qexed_entity_rendering";
}

use serde::{Deserialize, Serialize};

use crate::app::qexed::server::Placeholders;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QexedPlaceholders {
    pub placeholders: Placeholders,
}

impl Default for QexedPlaceholders {
    fn default() -> Self {
        Self {
            placeholders: Placeholders::default(),
        }
    }
}

impl qexed_config::tool::AppConfigTrait for QexedPlaceholders {
    const PATH: &'static str = "/";
    const NAME: &'static str = "qexed_placeholders";
}

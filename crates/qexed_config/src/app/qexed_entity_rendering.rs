use qexed_config_macros::AutoDoc;
use serde::{Deserialize, Serialize};

use crate::app::qexed::server::EntityRendering;

#[derive(Debug, Clone, Serialize, Deserialize, AutoDoc)]
pub struct QexedEntityRendering {
    #[AutoDoc(key = "config.qexed.server.entity_rendering", sub)]
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

    fn config_file_description(lang: &str, config_file: &str, _root_path: Option<&str>) -> String {
        rust_i18n::t!(
            "autodoc.file_description.qexed.entity_rendering",
            locale = lang,
            file = config_file
        )
        .to_string()
    }
}

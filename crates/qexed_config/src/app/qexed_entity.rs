use qexed_config_macros::AutoDoc;
use serde::{Deserialize, Serialize};

use crate::app::qexed::server::Entities;

#[derive(Debug, Clone, Serialize, Deserialize, AutoDoc)]
pub struct QexedEntity {
    #[AutoDoc(key = "config.qexed.server.entities", sub)]
    pub entities: Entities,
}

impl Default for QexedEntity {
    fn default() -> Self {
        Self {
            entities: Entities::default(),
        }
    }
}

impl qexed_config::tool::AppConfigTrait for QexedEntity {
    const PATH: &'static str = "/";
    const NAME: &'static str = "qexed_entity";

    fn config_file_description(lang: &str, config_file: &str, _root_path: Option<&str>) -> String {
        rust_i18n::t!(
            "autodoc.file_description.qexed.entities",
            locale = lang,
            file = config_file
        )
        .to_string()
    }
}

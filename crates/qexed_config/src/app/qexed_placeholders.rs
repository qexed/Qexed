use qexed_config_macros::AutoDoc;
use serde::{Deserialize, Serialize};

use crate::app::qexed::server::Placeholders;

#[derive(Debug, Clone, Serialize, Deserialize, AutoDoc)]
pub struct QexedPlaceholders {
    #[AutoDoc(key = "config.qexed.server.placeholders", sub)]
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

    fn config_file_description(lang: &str, config_file: &str, _root_path: Option<&str>) -> String {
        rust_i18n::t!(
            "autodoc.file_description.qexed.placeholders",
            locale = lang,
            file = config_file
        )
        .to_string()
    }
}

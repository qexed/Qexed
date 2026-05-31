use qexed_config_macros::AutoDoc;
use serde::{Deserialize, Serialize};

use crate::app::qexed::server::Npcs;

#[derive(Debug, Clone, Serialize, Deserialize, AutoDoc)]
pub struct QexedNpc {
    #[AutoDoc(key = "config.qexed.server.npcs", sub)]
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

    fn config_file_description(lang: &str, config_file: &str, _root_path: Option<&str>) -> String {
        rust_i18n::t!(
            "autodoc.file_description.qexed.npc",
            locale = lang,
            file = config_file
        )
        .to_string()
    }
}

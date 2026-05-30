use qexed_config_macros::AutoDoc;
use serde::{Deserialize, Serialize};

use crate::app::qexed::server::PlayerAudit;

#[derive(Debug, Clone, Serialize, Deserialize, AutoDoc)]
pub struct QexedPlayerAudit {
    #[AutoDoc(key = "config.qexed.server.player_audit", sub)]
    pub player_audit: PlayerAudit,
}

impl Default for QexedPlayerAudit {
    fn default() -> Self {
        Self {
            player_audit: PlayerAudit::default(),
        }
    }
}

impl qexed_config::tool::AppConfigTrait for QexedPlayerAudit {
    const PATH: &'static str = "/";
    const NAME: &'static str = "qexed_player_audit";

    fn config_file_description(lang: &str, config_file: &str, _root_path: Option<&str>) -> String {
        rust_i18n::t!(
            "autodoc.file_description.qexed.player_audit",
            locale = lang,
            file = config_file
        )
        .to_string()
    }
}

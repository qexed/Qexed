use qexed_config_macros::AutoDoc;
use serde::{Deserialize, Serialize};

use crate::app::qexed::server::Scoreboard;

#[derive(Debug, Clone, Serialize, Deserialize, AutoDoc)]
pub struct QexedScoreboard {
    #[AutoDoc(key = "config.qexed.server.scoreboard", sub)]
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

    fn config_file_description(lang: &str, config_file: &str, _root_path: Option<&str>) -> String {
        rust_i18n::t!(
            "autodoc.file_description.qexed.scoreboard",
            locale = lang,
            file = config_file
        )
        .to_string()
    }
}

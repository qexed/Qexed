use qexed_config_macros::AutoDoc;
use serde::{Deserialize, Serialize};

use crate::app::qexed::server::PlayerData;

#[derive(Debug, Clone, Serialize, Deserialize, AutoDoc)]
pub struct QexedPlayerData {
    #[AutoDoc(key = "config.qexed.server.player_data", sub)]
    pub player_data: PlayerData,
}

impl Default for QexedPlayerData {
    fn default() -> Self {
        Self {
            player_data: PlayerData::default(),
        }
    }
}

impl qexed_config::tool::AppConfigTrait for QexedPlayerData {
    const PATH: &'static str = "/";
    const NAME: &'static str = "qexed_player_data";

    fn config_file_description(lang: &str, config_file: &str, _root_path: Option<&str>) -> String {
        rust_i18n::t!(
            "autodoc.file_description.qexed.player_data",
            locale = lang,
            file = config_file
        )
        .to_string()
    }
}

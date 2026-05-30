use qexed_config_macros::AutoDoc;
use serde::{Deserialize, Serialize};

use crate::app::qexed::server::PlayerMessages;

#[derive(Debug, Clone, Serialize, Deserialize, AutoDoc)]
pub struct QexedPlayerMessages {
    #[AutoDoc(key = "config.qexed.server.player_messages", sub)]
    pub player_messages: PlayerMessages,
}

impl Default for QexedPlayerMessages {
    fn default() -> Self {
        Self {
            player_messages: PlayerMessages::default(),
        }
    }
}

impl qexed_config::tool::AppConfigTrait for QexedPlayerMessages {
    const PATH: &'static str = "/";
    const NAME: &'static str = "qexed_player_messages";

    fn config_file_description(lang: &str, config_file: &str, _root_path: Option<&str>) -> String {
        rust_i18n::t!(
            "autodoc.file_description.qexed.player_messages",
            locale = lang,
            file = config_file
        )
        .to_string()
    }
}

use qexed_config_macros::AutoDoc;
use serde::{Deserialize, Serialize};

use crate::app::qexed::server::Lobby;

#[derive(Debug, Clone, Serialize, Deserialize, AutoDoc)]
pub struct QexedLobby {
    #[AutoDoc(key = "config.qexed.server.lobby", sub)]
    pub lobby: Lobby,
}

impl Default for QexedLobby {
    fn default() -> Self {
        Self {
            lobby: Lobby::default(),
        }
    }
}

impl qexed_config::tool::AppConfigTrait for QexedLobby {
    const PATH: &'static str = "/";
    const NAME: &'static str = "qexed_lobby";

    fn obsolete_root_paths() -> &'static [&'static str] {
        &["lobby.servers"]
    }

    fn config_file_description(lang: &str, config_file: &str, _root_path: Option<&str>) -> String {
        rust_i18n::t!(
            "autodoc.file_description.qexed.lobby",
            locale = lang,
            file = config_file
        )
        .to_string()
    }
}

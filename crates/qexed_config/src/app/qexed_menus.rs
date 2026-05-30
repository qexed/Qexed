use qexed_config_macros::AutoDoc;
use serde::{Deserialize, Serialize};

use crate::app::qexed::server::Menus;

#[derive(Debug, Clone, Serialize, Deserialize, AutoDoc)]
pub struct QexedMenus {
    #[AutoDoc(key = "config.qexed.server.menus", sub)]
    pub menus: Menus,
}

impl Default for QexedMenus {
    fn default() -> Self {
        Self {
            menus: Menus::default(),
        }
    }
}

impl qexed_config::tool::AppConfigTrait for QexedMenus {
    const PATH: &'static str = "/";
    const NAME: &'static str = "qexed_menus";

    fn config_file_description(lang: &str, config_file: &str, _root_path: Option<&str>) -> String {
        rust_i18n::t!(
            "autodoc.file_description.qexed.menus",
            locale = lang,
            file = config_file
        )
        .to_string()
    }
}

use serde::{Deserialize, Serialize};

use crate::app::qexed::server::Menus;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QexedMenus {
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
}

use serde::{Deserialize, Serialize};

use crate::tool::AppConfigTrait;
#[derive(Debug, Serialize, Deserialize)]
pub struct PlayerList {
    pub version: i32,

    pub max_player: i32, //

    pub auto_update: bool,
}
impl Default for PlayerList {
    fn default() -> Self {
        Self {
            version: 0,
            max_player: 20,
            auto_update: false,
        }
    }
}
impl AppConfigTrait for PlayerList {
    const PATH: &'static str = "./config/qexed_player_list/";

    const NAME: &'static str = "config";
}

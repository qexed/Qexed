use crate::tool::AppConfigTrait;
use serde::{Deserialize, Serialize};
#[derive(Debug, Serialize, Deserialize)]
pub struct PlayerConfig {
    pub version: i32,
}
impl Default for PlayerConfig {
    fn default() -> Self {
        Self { version: 0 }
    }
}
impl AppConfigTrait for PlayerConfig {
    const PATH: &'static str = "./config/qexed_entity/player/";

    const NAME: &'static str = "config";
}

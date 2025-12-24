use serde::{Deserialize, Serialize};
use crate::tool::AppConfigTrait;

#[derive(Debug, Serialize, Deserialize)]
pub struct GameLogicConfig {
    pub version: i32,

}
impl Default for GameLogicConfig {
    fn default() -> Self {
        Self {
            version: 0,

        }
    }
}
impl AppConfigTrait for GameLogicConfig {
    const PATH: &'static str = "./config/qexed_logic/";

    const NAME: &'static str = "config";
}

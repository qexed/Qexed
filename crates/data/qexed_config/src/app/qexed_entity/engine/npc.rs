use crate::tool::AppConfigTrait;
use serde::{Deserialize, Serialize};
#[derive(Debug, Serialize, Deserialize)]
pub struct NpcConfig {
    pub version: i32,
    pub enable: bool,
}
impl Default for NpcConfig {
    fn default() -> Self {
        Self {
            version: 0,
            enable: true,
        }
    }
}
impl AppConfigTrait for NpcConfig {
    const PATH: &'static str = "./config/qexed_entity/npc/";

    const NAME: &'static str = "config";
}

use crate::tool::AppConfigTrait;
use serde::{Deserialize, Serialize};
pub mod engine;
#[derive(Debug, Serialize, Deserialize)]
pub struct EntityConfig {
    pub version: i32,
    pub player: engine::player::PlayerConfig,
    pub npc: engine::npc::NpcConfig,
}
impl Default for EntityConfig {
    fn default() -> Self {
        Self {
            version: 0,
            player: Default::default(),
            npc: Default::default(),
        }
    }
}
impl AppConfigTrait for EntityConfig {
    const PATH: &'static str = "./config/qexed_entity/";

    const NAME: &'static str = "config";
}

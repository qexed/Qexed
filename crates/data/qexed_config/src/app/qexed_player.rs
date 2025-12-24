use serde::{Deserialize, Serialize};

use crate::tool::AppConfigTrait;
#[derive(Debug, Serialize, Deserialize,Clone)]
pub struct PlayerConfig {
    pub version: i32,
    pub player_save:PlayerSave,
    
}
#[derive(Debug, Serialize, Deserialize,Clone,Default)]
pub struct PlayerSave{
    // 是否启用
    // 若禁用则不再读取而是每次都是新建玩家
    pub enable: bool,
}
impl Default for PlayerConfig {
    fn default() -> Self {
        Self {
            version: 0,
            player_save: Default::default(),

        }
    }
}
impl AppConfigTrait for PlayerConfig {
    const PATH: &'static str = "./config/qexed_player/";
    const NAME: &'static str = "config";
}

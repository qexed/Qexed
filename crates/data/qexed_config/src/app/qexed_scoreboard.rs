use serde::{Deserialize, Serialize};

use crate::tool::AppConfigTrait;
#[derive(Debug, Serialize, Deserialize,Clone)]
pub struct ScoreBoardConfig {
    pub version: i32,
    pub line:Vec<String>,
}
impl Default for ScoreBoardConfig {
    fn default() -> Self {
        Self {
            version: 0,
            line:vec![
                "Qexed服务器".to_string(),
                "".to_string(),
                "玩家: §a{player}".to_string(),
                "区域: §a大厅1".to_string(),
                "本区在线: §a{online}".to_string(),
                "".to_string(),
                "§eQQ群:627495509".to_string(),
            ]
        }
    }
}
impl AppConfigTrait for ScoreBoardConfig {
    const PATH: &'static str = "./config/qexed_scoreboard/";
    const NAME: &'static str = "config";
}

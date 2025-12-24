use serde::{Deserialize, Serialize};

use crate::tool::AppConfigTrait;
#[derive(Debug, Serialize, Deserialize,Clone)]
pub struct QexedWorldMiniConfig {
    pub version: i32,
    
}
impl Default for QexedWorldMiniConfig {
    fn default() -> Self {
        Self {
            version: 0,

        }
    }
}
impl AppConfigTrait for QexedWorldMiniConfig {
    const PATH: &'static str = "./config/qexed_world/mini/";
    const NAME: &'static str = "config";
}

use serde::{Deserialize, Serialize};

use crate::tool::AppConfigTrait;
#[derive(Debug, Serialize, Deserialize,Clone)]
pub struct ChatConfig {
    pub version: i32,

}
impl Default for ChatConfig {
    fn default() -> Self {
        Self {
            version: 0,

        }
    }
}
impl AppConfigTrait for ChatConfig {
    const PATH: &'static str = "./config/qexed_chat/";
    const NAME: &'static str = "config";
}

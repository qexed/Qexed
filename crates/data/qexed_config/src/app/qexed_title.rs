use serde::{Deserialize, Serialize};

use crate::tool::AppConfigTrait;
#[derive(Debug, Serialize, Deserialize,Clone)]
pub struct TitleConfig {
    pub version: i32,
    
}
impl Default for TitleConfig {
    fn default() -> Self {
        Self {
            version: 0,
        }
    }
}
impl AppConfigTrait for TitleConfig {
    const PATH: &'static str = "./config/qexed_title/";
    const NAME: &'static str = "config";
}

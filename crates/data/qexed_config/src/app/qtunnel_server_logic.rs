use serde::{Deserialize, Serialize};
use crate::tool::AppConfigTrait;

#[derive(Debug, Serialize, Deserialize)]
pub struct ServerLogicConfig {
    pub version: i32,

}
impl Default for ServerLogicConfig {
    fn default() -> Self {
        Self {
            version: 0,
        }
    }
}
impl AppConfigTrait for ServerLogicConfig {
    const PATH: &'static str = "./config/qtunnel_server_logic/";

    const NAME: &'static str = "config";
}

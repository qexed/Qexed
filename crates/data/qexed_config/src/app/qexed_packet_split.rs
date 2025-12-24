use serde::{Deserialize, Serialize};

use crate::tool::AppConfigTrait;
#[derive(Debug, Serialize, Deserialize,Clone)]
pub struct PacketSplitConfig {
    pub version: i32,

}
impl Default for PacketSplitConfig {
    fn default() -> Self {
        Self {
            version: 0,

        }
    }
}
impl AppConfigTrait for PacketSplitConfig {
    const PATH: &'static str = "./config/qexed_packet_split/";
    const NAME: &'static str = "config";
}

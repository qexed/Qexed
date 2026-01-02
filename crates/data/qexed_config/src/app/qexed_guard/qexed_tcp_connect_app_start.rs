use serde::{Deserialize, Serialize};

use crate::tool::AppConfigTrait;

#[derive(Debug,Clone, Serialize, Deserialize)]
pub struct TcpConnectStart {
    pub version: i32,
    pub check_connect_too_spped_time:i64,
    
}


impl Default for TcpConnectStart {
    fn default() -> Self {
        Self {
            version: 0,
            check_connect_too_spped_time:60,
        }
    }
}
impl AppConfigTrait for TcpConnectStart {
    const PATH: &'static str = "./config/qexed_guard/qexed_player_list/";

    const NAME: &'static str = "start";
}

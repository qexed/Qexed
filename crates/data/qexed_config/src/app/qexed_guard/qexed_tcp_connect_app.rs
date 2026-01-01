use serde::{Deserialize, Serialize};

use crate::tool::AppConfigTrait;

#[derive(Debug,Clone, Serialize, Deserialize)]
pub struct TcpConnect {
    pub version: i32,
    
}


impl Default for TcpConnect {
    fn default() -> Self {
        Self {
            version: 0,
            
        }
    }
}
impl AppConfigTrait for TcpConnect {
    const PATH: &'static str = "./config/qexed_guard/qexed_player_list/";

    const NAME: &'static str = "config";
}

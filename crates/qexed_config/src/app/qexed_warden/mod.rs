
// 典狱长(封禁管理)
pub mod data;
use serde::{Deserialize, Serialize};
use crate::tool::AppConfigTrait;
#[derive(Debug, Serialize, Deserialize)]
pub struct QexedWarden {
    pub version: i32,
    pub data:data::Data,
}
impl Default for QexedWarden {
    fn default() -> Self {
        Self { 
            version: Default::default(),
            data:Default::default(),
        }
    }
}
impl AppConfigTrait for QexedWarden {
    const PATH: &'static str = "./config/";

    const NAME: &'static str = "qexed_warden";
}

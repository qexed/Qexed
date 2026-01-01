use serde::{Deserialize, Serialize};

use crate::tool::AppConfigTrait;

pub mod data;
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct WardonConfig {
    pub version: i32,
    pub data:data::Data,
}
impl AppConfigTrait for WardonConfig {
    const PATH: &'static str = "./config/qexed_guard/";
    const NAME: &'static str = "config";
}

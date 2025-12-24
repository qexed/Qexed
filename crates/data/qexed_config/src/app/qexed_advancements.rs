use serde::{Deserialize, Serialize};

use crate::tool::AppConfigTrait;
#[derive(Debug, Serialize, Deserialize,Clone)]
pub struct AdvancementsConfig {
    pub version: i32,
    pub advancements:Vec<AdvancementMapping>,
}
impl Default for AdvancementsConfig {
    fn default() -> Self {
        Self {
            version: 0,
            advancements:vec![]

        }
    }
}
impl AppConfigTrait for AdvancementsConfig {
    const PATH: &'static str = "./config/qexed_advancements/";
    const NAME: &'static str = "config";
}
#[derive(Debug, Serialize, Deserialize,Clone)]
pub struct AdvancementMapping{
    
}
use std::i32;

use serde::{Deserialize, Serialize};

use crate::tool::AppConfigTrait;
#[derive(Debug, Serialize, Deserialize)]
pub struct EntityIdAllocator {
    pub version: i32,
    pub start_id: i32,
    pub max_entity_id: i32,
}
impl Default for EntityIdAllocator {
    fn default() -> Self {
        Self {
            version: 0,
            start_id:0,
            max_entity_id:i32::MAX,
        }
    }
}
impl AppConfigTrait for EntityIdAllocator {
    const PATH: &'static str = "./config/qexed_entity_id_allocator/";

    const NAME: &'static str = "config";
}

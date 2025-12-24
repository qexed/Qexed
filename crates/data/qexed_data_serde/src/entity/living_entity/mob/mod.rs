pub mod pathfinder_mob;
pub mod neutral_mob;
use std::collections::HashMap;
use serde::{Deserialize,Serialize,};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mob {
    #[serde(flatten)]
    pub living_entity:super::LivingEntity,
    #[serde(rename = "CanPickUpLoot")]
    pub can_pick_up_loot:bool,
    #[serde(rename = "DeathLootTable")]
    pub death_loot_table:Option<String>,
    #[serde(rename = "DeathLootTableSeed")]
    pub death_loot_table_seed:Option<i64>,

    pub drop_chances:Option<HashMap<String,f32>>,

    pub home_pos:Option<Vec<i32>>,

    pub home_radius:Option<i32>,

    pub leash:Option<LeashTag>,
    #[serde(rename = "LeftHanded")]
    pub left_handed:bool,
    #[serde(rename = "NoAI")]
    pub no_ai:Option<bool>,
    #[serde(rename = "PersistenceRequired")]
    pub persistence_required:bool,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum LeashTag {
    /// 拴在栅栏上
    Fence(FenceLeash),
    
    /// 拴在实体上
    Entity(EntityLeash),
}

/// 拴在栅栏上的数据
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FenceLeash {
    pub position: Vec<i32>,  // X, Y, Z
}

/// 拴在实体上的数据
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntityLeash {
    pub uuid: Vec<String>,
}

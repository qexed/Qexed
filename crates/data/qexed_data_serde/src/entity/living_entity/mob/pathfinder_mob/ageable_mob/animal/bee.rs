use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bee {
    #[serde(flatten)]
    pub ageable_mob:crate::entity::living_entity::mob::pathfinder_mob::ageable_mob::AgeableMob,
    #[serde(flatten)]
    pub neutral_mob:crate::entity::living_entity::mob::neutral_mob::NeutralMob,
    #[serde(rename = "CannotEnterHiveTicks")]
    pub cannot_enter_hive_ticks:i32,
    #[serde(rename = "CropsGrownSincePollination")]
    pub crops_grown_since_pollination:i32,
    pub flower_pos:Option<Vec<i32>>,
    #[serde(rename = "HasNectar")]
    pub has_nectar:bool,
    #[serde(rename = "HasStung")]
    pub has_stung:bool,
    pub hive_pos:Option<Vec<i32>>,
    #[serde(rename = "TicksSincePollination")]
    pub ticks_since_pollination:i32,
}
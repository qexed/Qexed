pub mod animal;
use serde::{Deserialize,Serialize,};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgeableMob {
    #[serde(flatten)]
    pub pathfinder_mob:super::PathfinderMob,
    #[serde(rename = "Age")]
    pub age:i32,
    #[serde(rename = "ForcedAge")]
    pub forced_age:i32,
}
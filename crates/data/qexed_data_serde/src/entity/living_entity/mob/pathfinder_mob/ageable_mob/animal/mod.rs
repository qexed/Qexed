pub mod bee;
use serde::{Deserialize,Serialize,};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Animal {
    #[serde(flatten)]
    pub ageable_mob:super::AgeableMob,
    #[serde(rename = "InLove")]
    pub in_love:i32,
    #[serde(rename = "LoveCause")]
    pub love_cause:Option<Vec<i32>>

}
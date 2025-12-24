pub mod ageable_mob;
use serde::{Deserialize,Serialize,};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathfinderMob {
    #[serde(flatten)]
    pub mob:super::Mob,

}
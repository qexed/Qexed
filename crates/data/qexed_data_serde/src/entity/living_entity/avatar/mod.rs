pub mod player;
use serde::{Deserialize,  Serialize};

use crate::entity::living_entity::LivingEntity;
#[derive(Debug, Clone, Serialize, Deserialize,Default)]
pub struct Avatar {
    #[serde(flatten)]
    pub living_entity: LivingEntity
}

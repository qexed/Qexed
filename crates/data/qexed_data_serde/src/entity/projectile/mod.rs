pub mod throwable_projectile;
use serde::{Deserialize, Serialize,};

use crate::entity::Entity;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Projectile {
    #[serde(flatten)]
    pub entity: Entity,
}
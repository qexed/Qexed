pub mod thrown_enderpearl;
use serde::{Deserialize, Serialize};

use crate::entity::projectile::throwable_projectile::ThrowableProjectile;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThrowableItemProjectile {
    #[serde(flatten)]
    pub throwable_projectile: ThrowableProjectile,
}


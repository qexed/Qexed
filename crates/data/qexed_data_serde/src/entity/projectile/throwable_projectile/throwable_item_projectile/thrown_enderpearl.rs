use serde::{Deserialize, Serialize};

use crate::entity::projectile::throwable_projectile::throwable_item_projectile::ThrowableItemProjectile;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThrownEnderpearl{
    #[serde(flatten)]
    pub throwable_item_projectile :ThrowableItemProjectile
}
pub mod throwable_item_projectile;
use serde::{Deserialize, Serialize};

use crate::entity::projectile::Projectile;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThrowableProjectile {
    #[serde(flatten)]
    pub projectile: Projectile,
}


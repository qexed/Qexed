mod manager;
mod model;
mod registry;

pub use manager::{DroppedItemUpdate, EntityManager};
#[cfg(test)]
pub use qexed_entity::npc_profile_name;
pub use qexed_entity::{
    DroppedItemEntity, EntityIdAllocator, EntitySpawnRequest, ManagedEntity, ManagedEntityKind,
};
pub(crate) use registry::entity_type_id;

#[cfg(test)]
mod tests;

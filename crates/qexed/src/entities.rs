mod id;
mod manager;
mod model;
mod packets;
mod registry;

pub use id::EntityIdAllocator;
pub use manager::EntityManager;
pub use model::{DroppedItemEntity, EntitySpawnRequest, ManagedEntity, ManagedEntityKind};
pub(crate) use registry::entity_type_id;

#[cfg(test)]
mod tests;

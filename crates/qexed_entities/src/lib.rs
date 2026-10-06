pub mod block_shapes;
pub mod config;
pub mod context;
pub mod error;
pub mod id;
pub mod manager;
pub mod model;
pub mod model_helpers;
pub mod optimized_collision;
pub mod optimized_spatial;
pub mod packets;
pub mod pathfinding;
pub mod position;

#[cfg(test)]
mod tests;

/// 测试共用辅助（tests 与 integration_tests 共享）。
#[cfg(test)]
pub(crate) mod tests_support {
    use qexed_protocol::types::Slot;
    use qexed_packet::net_types::VarInt;

    pub(crate) fn simple_item(item_id: i32, count: i32) -> Slot {
        Slot {
            item_count: VarInt(count),
            item_id: Some(VarInt(item_id)),
            // 26.3：item_count > 0 时必须显式给出组件计数（0 = 无附加组件）。
            number_of_components_to_add: Some(VarInt(0)),
            number_of_components_to_remove: Some(VarInt(0)),
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod integration_tests;

pub use config::{
    Entities, Entity, EntityAiOverride, EntityKind, EntityRendering, EntitySpawnRule,
    EntitySpawning, SlimeChunkSpawning,
};
pub use context::{
    BlockCollisionShape, BlockUpdate, CustomEntityDefinition, EntityAiEntityPayload,
    EntityAiHost, EntityAiOperation, EntityAiPlayerPayload, EntityAiTickQuery, NoEntityAiHost,
    OnlinePlayerView, PlayerDamageKind, ProjectileHitPlayerEvent, ViewerSource, WorldAccess,
    WorldRulesAccess, WorldRulesSnapshot,
};
pub use id::EntityIdAllocator;
pub use manager::{DroppedItemUpdate, EntityManager, VisualProjectileSpawnRequest};
pub use model::{DroppedItemEntity, EntitySpawnRequest, ManagedEntity, ManagedEntityKind};
pub use block_shapes::ReportBlockShapes;
pub use packets::npc_profile_name;
pub use position::EntityPosition;
pub mod registry;

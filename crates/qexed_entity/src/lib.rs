use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EntityId(i32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EntityTypeId(i32);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntityKind {
    Player,
    Mob { type_id: EntityTypeId },
    Object { type_id: EntityTypeId },
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct EntityPosition {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct EntityVelocity {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct EntityPose {
    pub position: EntityPosition,
    pub velocity: EntityVelocity,
    pub yaw: f32,
    pub pitch: f32,
    pub on_ground: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entity {
    pub id: EntityId,
    pub uuid: uuid::Uuid,
    pub kind: EntityKind,
    pub pose: EntityPose,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntityIdAllocator {
    next: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntitySpawnSnapshot {
    pub id: EntityId,
    pub uuid: uuid::Uuid,
    pub kind: EntityKind,
    pub pose: EntityPose,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct EntityPoseSnapshot {
    pub id: EntityId,
    pub pose: EntityPose,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityDespawnSnapshot {
    pub id: EntityId,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EntityUpdateSnapshot {
    Spawn(EntitySpawnSnapshot),
    Despawn(EntityDespawnSnapshot),
    Pose(EntityPoseSnapshot),
}

pub type EntitySnapshot = EntitySpawnSnapshot;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InsertEntityError {
    DuplicateId(EntityId),
}

#[derive(Debug, Default, Clone)]
pub struct EntityStore {
    allocator: EntityIdAllocator,
    entities: BTreeMap<EntityId, Entity>,
    updates: Vec<EntityUpdateSnapshot>,
}

impl EntityId {
    pub const fn new(value: i32) -> Self {
        Self(value)
    }

    pub const fn as_i32(self) -> i32 {
        self.0
    }

    pub const fn get(self) -> i32 {
        self.as_i32()
    }
}

impl From<EntityId> for i32 {
    fn from(id: EntityId) -> Self {
        id.as_i32()
    }
}

impl fmt::Display for EntityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl EntityTypeId {
    pub const fn new(value: i32) -> Self {
        Self(value)
    }

    pub const fn as_i32(self) -> i32 {
        self.0
    }
}

impl From<i32> for EntityTypeId {
    fn from(value: i32) -> Self {
        Self::new(value)
    }
}

impl From<EntityTypeId> for i32 {
    fn from(id: EntityTypeId) -> Self {
        id.as_i32()
    }
}

impl Default for EntityPosition {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 64.0,
            z: 0.0,
        }
    }
}

impl Default for EntityVelocity {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        }
    }
}

impl Default for EntityPose {
    fn default() -> Self {
        Self {
            position: EntityPosition::default(),
            velocity: EntityVelocity::default(),
            yaw: 0.0,
            pitch: 0.0,
            on_ground: true,
        }
    }
}

impl EntityIdAllocator {
    pub const fn new() -> Self {
        Self { next: 1 }
    }

    pub const fn with_next(next: i32) -> Self {
        Self { next }
    }

    pub fn allocate(&mut self) -> EntityId {
        let id = EntityId(self.next);
        self.next = self.next.saturating_add(1).max(1);
        id
    }

    pub const fn next(&self) -> i32 {
        self.next
    }

    fn reserve_after(&mut self, id: EntityId) {
        self.next = self.next.max(id.as_i32().saturating_add(1));
    }
}

impl Default for EntityIdAllocator {
    fn default() -> Self {
        Self::new()
    }
}

impl Entity {
    pub fn new(id: EntityId, kind: EntityKind, pose: EntityPose) -> Self {
        Self::with_uuid(id, uuid::Uuid::new_v4(), kind, pose)
    }

    pub fn with_uuid(id: EntityId, uuid: uuid::Uuid, kind: EntityKind, pose: EntityPose) -> Self {
        Self {
            id,
            uuid,
            kind,
            pose,
        }
    }

    pub fn snapshot(&self) -> EntitySnapshot {
        EntitySnapshot {
            id: self.id,
            uuid: self.uuid,
            kind: self.kind.clone(),
            pose: self.pose,
        }
    }

    pub fn pose_snapshot(&self) -> EntityPoseSnapshot {
        EntityPoseSnapshot {
            id: self.id,
            pose: self.pose,
        }
    }
}

impl EntityStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn spawn(&mut self, kind: EntityKind, pose: EntityPose) -> EntityId {
        let id = self.allocator.allocate();
        let entity = Entity::new(id, kind, pose);
        self.insert_allocated(entity);
        id
    }

    pub fn spawn_with_uuid(
        &mut self,
        uuid: uuid::Uuid,
        kind: EntityKind,
        pose: EntityPose,
    ) -> EntityId {
        let id = self.allocator.allocate();
        let entity = Entity::with_uuid(id, uuid, kind, pose);
        self.insert_allocated(entity);
        id
    }

    pub fn insert(&mut self, entity: Entity) -> Result<(), InsertEntityError> {
        if self.entities.contains_key(&entity.id) {
            return Err(InsertEntityError::DuplicateId(entity.id));
        }
        self.allocator.reserve_after(entity.id);
        self.insert_allocated(entity);
        Ok(())
    }

    pub fn despawn(&mut self, id: EntityId) -> Option<Entity> {
        let entity = self.entities.remove(&id)?;
        self.updates
            .push(EntityUpdateSnapshot::Despawn(EntityDespawnSnapshot { id }));
        Some(entity)
    }

    pub fn update_pose(&mut self, id: EntityId, pose: EntityPose) -> bool {
        let Some(entity) = self.entities.get_mut(&id) else {
            return false;
        };
        entity.pose = pose;
        self.updates
            .push(EntityUpdateSnapshot::Pose(EntityPoseSnapshot { id, pose }));
        true
    }

    pub fn get(&self, id: EntityId) -> Option<&Entity> {
        self.entities.get(&id)
    }

    pub fn contains(&self, id: EntityId) -> bool {
        self.entities.contains_key(&id)
    }

    pub fn len(&self) -> usize {
        self.entities.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Entity> {
        self.entities.values()
    }

    pub fn snapshots(&self) -> Vec<EntitySnapshot> {
        self.iter().map(Entity::snapshot).collect()
    }

    pub fn updates(&self) -> &[EntityUpdateSnapshot] {
        &self.updates
    }

    pub fn drain_updates(&mut self) -> Vec<EntityUpdateSnapshot> {
        std::mem::take(&mut self.updates)
    }

    fn insert_allocated(&mut self, entity: Entity) {
        let snapshot = entity.snapshot();
        self.entities.insert(entity.id, entity);
        self.updates.push(EntityUpdateSnapshot::Spawn(snapshot));
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Entity, EntityId, EntityIdAllocator, EntityKind, EntityPose, EntityPosition, EntityStore,
        EntityTypeId, EntityUpdateSnapshot, EntityVelocity, InsertEntityError,
    };

    #[test]
    fn allocator_starts_at_one_and_advances() {
        let mut allocator = EntityIdAllocator::new();

        assert_eq!(allocator.allocate(), EntityId::new(1));
        assert_eq!(allocator.allocate(), EntityId::new(2));
        assert_eq!(allocator.next(), 3);
    }

    #[test]
    fn store_spawns_updates_and_despawns_entities() {
        let mut store = EntityStore::new();
        let pose = EntityPose {
            position: EntityPosition {
                x: 1.0,
                y: 65.0,
                z: -1.0,
            },
            velocity: EntityVelocity {
                x: 0.1,
                y: 0.0,
                z: 0.0,
            },
            yaw: 90.0,
            pitch: 10.0,
            on_ground: true,
        };

        let id = store.spawn(EntityKind::Player, pose);

        assert_eq!(id, EntityId::new(1));
        assert_eq!(store.len(), 1);
        assert_eq!(store.get(id).unwrap().pose, pose);
        assert!(matches!(store.updates()[0], EntityUpdateSnapshot::Spawn(_)));

        let updated = EntityPose {
            position: EntityPosition {
                x: 2.0,
                ..pose.position
            },
            ..pose
        };
        assert!(store.update_pose(id, updated));
        assert_eq!(store.get(id).unwrap().pose, updated);
        assert!(matches!(store.updates()[1], EntityUpdateSnapshot::Pose(_)));

        assert!(store.despawn(id).is_some());
        assert!(store.is_empty());
        assert!(matches!(
            store.updates()[2],
            EntityUpdateSnapshot::Despawn(_)
        ));
    }

    #[test]
    fn snapshots_are_stable_and_protocol_free() {
        let mut store = EntityStore::new();
        let uuid = uuid::Uuid::from_u128(1);
        let id = store.spawn_with_uuid(
            uuid,
            EntityKind::Mob {
                type_id: EntityTypeId::new(146),
            },
            EntityPose::default(),
        );

        let snapshots = store.snapshots();

        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].id, id);
        assert_eq!(snapshots[0].uuid, uuid);
        assert_eq!(
            snapshots[0].kind,
            EntityKind::Mob {
                type_id: EntityTypeId::new(146)
            }
        );
    }

    #[test]
    fn insert_rejects_duplicate_ids_and_reserves_next_id() {
        let mut store = EntityStore::new();
        let entity = Entity::with_uuid(
            EntityId::new(10),
            uuid::Uuid::from_u128(2),
            EntityKind::Object {
                type_id: EntityTypeId::new(1),
            },
            EntityPose::default(),
        );

        assert_eq!(store.insert(entity.clone()), Ok(()));
        assert_eq!(
            store.insert(entity),
            Err(InsertEntityError::DuplicateId(EntityId::new(10)))
        );

        let id = store.spawn(EntityKind::Player, EntityPose::default());
        assert_eq!(id, EntityId::new(11));
    }

    #[test]
    fn drain_updates_clears_pending_snapshots() {
        let mut store = EntityStore::new();

        store.spawn(EntityKind::Player, EntityPose::default());
        assert_eq!(store.drain_updates().len(), 1);
        assert!(store.updates().is_empty());
    }
}

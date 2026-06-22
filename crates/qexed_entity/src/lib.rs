use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EntityId(i32);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntityKind {
    Player,
    Mob { type_id: i32 },
    Object { type_id: i32 },
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
pub enum EntityEvent {
    Spawned(EntitySnapshot),
    Despawned(EntityId),
    PoseUpdated { id: EntityId, pose: EntityPose },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntitySnapshot {
    pub id: EntityId,
    pub uuid: uuid::Uuid,
    pub kind: EntityKind,
    pub pose: EntityPose,
}

#[derive(Debug, Default, Clone)]
pub struct EntityStore {
    allocator: EntityIdAllocator,
    entities: BTreeMap<EntityId, Entity>,
    events: Vec<EntityEvent>,
}

impl EntityId {
    pub const fn new(value: i32) -> Self {
        Self(value)
    }

    pub const fn get(self) -> i32 {
        self.0
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
}

impl Default for EntityIdAllocator {
    fn default() -> Self {
        Self::new()
    }
}

impl Entity {
    pub fn new(id: EntityId, kind: EntityKind, pose: EntityPose) -> Self {
        Self {
            id,
            uuid: uuid::Uuid::new_v4(),
            kind,
            pose,
        }
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
}

impl EntityStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn spawn(&mut self, kind: EntityKind, pose: EntityPose) -> EntityId {
        let id = self.allocator.allocate();
        self.insert(Entity::new(id, kind, pose));
        id
    }

    pub fn spawn_with_uuid(
        &mut self,
        uuid: uuid::Uuid,
        kind: EntityKind,
        pose: EntityPose,
    ) -> EntityId {
        let id = self.allocator.allocate();
        self.insert(Entity::with_uuid(id, uuid, kind, pose));
        id
    }

    pub fn insert(&mut self, entity: Entity) {
        self.allocator.next = self.allocator.next.max(entity.id.get().saturating_add(1));
        let snapshot = entity.snapshot();
        self.entities.insert(entity.id, entity);
        self.events.push(EntityEvent::Spawned(snapshot));
    }

    pub fn despawn(&mut self, id: EntityId) -> Option<Entity> {
        let entity = self.entities.remove(&id)?;
        self.events.push(EntityEvent::Despawned(id));
        Some(entity)
    }

    pub fn update_pose(&mut self, id: EntityId, pose: EntityPose) -> bool {
        let Some(entity) = self.entities.get_mut(&id) else {
            return false;
        };
        entity.pose = pose;
        self.events.push(EntityEvent::PoseUpdated { id, pose });
        true
    }

    pub fn get(&self, id: EntityId) -> Option<&Entity> {
        self.entities.get(&id)
    }

    pub fn len(&self) -> usize {
        self.entities.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
    }

    pub fn snapshots(&self) -> Vec<EntitySnapshot> {
        self.entities.values().map(Entity::snapshot).collect()
    }

    pub fn events(&self) -> &[EntityEvent] {
        &self.events
    }

    pub fn drain_events(&mut self) -> Vec<EntityEvent> {
        std::mem::take(&mut self.events)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        EntityEvent, EntityId, EntityIdAllocator, EntityKind, EntityPose, EntityPosition,
        EntityStore, EntityVelocity,
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
        assert!(matches!(store.events()[0], EntityEvent::Spawned(_)));

        let updated = EntityPose {
            position: EntityPosition {
                x: 2.0,
                ..pose.position
            },
            ..pose
        };
        assert!(store.update_pose(id, updated));
        assert_eq!(store.get(id).unwrap().pose, updated);
        assert!(matches!(store.events()[1], EntityEvent::PoseUpdated { .. }));

        assert!(store.despawn(id).is_some());
        assert!(store.is_empty());
        assert!(matches!(store.events()[2], EntityEvent::Despawned(_)));
    }

    #[test]
    fn snapshots_are_stable_and_protocol_free() {
        let mut store = EntityStore::new();
        let uuid = uuid::Uuid::from_u128(1);
        let id = store.spawn_with_uuid(
            uuid,
            EntityKind::Mob { type_id: 146 },
            EntityPose::default(),
        );

        let snapshots = store.snapshots();

        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].id, id);
        assert_eq!(snapshots[0].uuid, uuid);
        assert_eq!(snapshots[0].kind, EntityKind::Mob { type_id: 146 });
    }
}

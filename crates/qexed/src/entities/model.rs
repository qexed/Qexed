use qexed_protocol::to_client::play::add_entity::EntityPosition;

#[derive(Debug, Clone, PartialEq)]
pub struct ManagedEntity {
    pub key: String,
    pub entity_id: i32,
    pub uuid: uuid::Uuid,
    pub kind: ManagedEntityKind,
    pub entity_type: String,
    pub entity_type_id: i32,
    pub dimension: String,
    pub position: EntityPosition,
    pub name: String,
    pub display_name: String,
    pub skin_textures: String,
    pub skin_signature: String,
    pub data: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManagedEntityKind {
    Entity,
    Npc,
    Hologram,
}

#[derive(Debug, Clone)]
pub struct EntitySpawnRequest {
    pub key: String,
    pub kind: ManagedEntityKind,
    pub entity_type: String,
    pub dimension: String,
    pub position: EntityPosition,
    pub name: String,
    pub display_name: String,
    pub skin_textures: String,
    pub skin_signature: String,
    pub data: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DroppedItemEntity {
    pub entity_id: i32,
    pub uuid: uuid::Uuid,
    pub dimension: String,
    pub position: EntityPosition,
    pub item: qexed_protocol::types::Slot,
    pub pickup_ready_at: std::time::Instant,
}

pub(super) fn configured_entity_key(
    index: usize,
    config: &qexed_config::app::qexed::server::Entity,
) -> String {
    let id = config.id.trim();
    if id.is_empty() {
        format!("entity-{index}")
    } else {
        id.to_string()
    }
}

pub(super) fn stable_entity_uuid(key: &str) -> uuid::Uuid {
    uuid::Uuid::new_v3(
        &uuid::Uuid::NAMESPACE_OID,
        format!("qexed:entity:{key}").as_bytes(),
    )
}

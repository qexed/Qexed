use qexed_protocol::to_client::play::add_entity::EntityPosition;
use std::collections::BTreeMap;

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
    pub ai: String,
    pub ai_params: BTreeMap<String, serde_json::Value>,
    pub auto_jump: bool,
    pub spawn_rule: String,
    pub custom_type: String,
    pub look_at_players: bool,
    pub main_hand_event: String,
    pub off_hand_event: String,
    pub attack_event: String,
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
    pub ai: String,
    pub ai_params: BTreeMap<String, serde_json::Value>,
    pub auto_jump: bool,
    pub spawn_rule: String,
    pub custom_type: String,
    pub look_at_players: bool,
    pub main_hand_event: String,
    pub off_hand_event: String,
    pub attack_event: String,
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

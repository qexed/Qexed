use bytes::Bytes;
use qexed_protocol::to_client::play::{add_entity::EntityPosition, set_equipment::Equipment};
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub enum PlayerEvent {
    Joined(OnlinePlayer),
    Left {
        profile_id: uuid::Uuid,
        entity_id: i32,
        username: String,
        dimension: String,
    },
    Moved {
        profile_id: uuid::Uuid,
        entity_id: i32,
        dimension: String,
        position: EntityPosition,
    },
    DimensionChanged {
        profile_id: uuid::Uuid,
        entity_id: i32,
        old_dimension: String,
        player: OnlinePlayer,
    },
    Teleport {
        profile_id: uuid::Uuid,
        dimension: String,
        position: EntityPosition,
    },
    EquipmentChanged {
        profile_id: uuid::Uuid,
        entity_id: i32,
        dimension: String,
        slots: Vec<Equipment>,
    },
    BlockChanged {
        profile_id: uuid::Uuid,
        dimension: String,
        position: qexed_packet::net_types::Position,
        block_state: i32,
        light_update: Option<Bytes>,
    },
    ClientboundPackets {
        packets: Vec<Bytes>,
    },
}

#[derive(Debug, Clone)]
pub struct OnlinePlayer {
    pub profile: qexed_packet::net_types::GameProfile,
    pub entity_id: i32,
    pub position: EntityPosition,
    pub dimension: String,
    pub equipment: Vec<Equipment>,
    pub language: String,
}

#[derive(Debug)]
pub struct PlayerSession {
    pub player: OnlinePlayer,
    pub receiver: mpsc::UnboundedReceiver<PlayerEvent>,
}

#[derive(Debug)]
pub(super) struct PlayerHandle {
    pub(super) player: OnlinePlayer,
    pub(super) sender: mpsc::UnboundedSender<PlayerEvent>,
}

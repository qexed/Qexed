use bytes::Bytes;
use qexed_protocol::{
    to_client::play::set_equipment::EquipmentEntry,
    types::{EntityPosition, Slot},
};
use tokio::sync::mpsc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerDamageKind {
    Generic,
    MobAttack,
    Projectile,
    Explosion,
    Magic,
}

#[derive(Debug, Clone)]
pub struct BlockChange {
    pub position: qexed_packet::net_types::Position,
    pub block_state: i32,
}

#[derive(Debug, Clone)]
pub struct ProjectileHitPlayerEvent {
    pub shooter_profile_id: uuid::Uuid,
    pub target_profile_id: uuid::Uuid,
    pub projectile_entity_id: i32,
    pub projectile_kind: String,
    pub dimension: String,
    pub position: EntityPosition,
    pub configured_event: String,
    pub tag: String,
}

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
    Damage {
        profile_id: uuid::Uuid,
        amount: f32,
        kind: PlayerDamageKind,
        source_entity_id: i32,
        source_position: EntityPosition,
        knockback: f32,
    },
    PotionEffect {
        profile_id: uuid::Uuid,
        effect: String,
        amplifier: i32,
        duration_ticks: i32,
        source_entity_id: i32,
        source_position: EntityPosition,
        knockback: f32,
    },
    GameModeChanged {
        profile_id: uuid::Uuid,
        username: String,
        game_mode: i32,
    },
    GiveItem {
        profile_id: uuid::Uuid,
        item: Slot,
        item_name: String,
    },
    ProjectileHitPlayer(ProjectileHitPlayerEvent),
    Animation {
        profile_id: uuid::Uuid,
        entity_id: i32,
        dimension: String,
        action_id: u8,
    },
    SetEquipmentChanged {
        profile_id: uuid::Uuid,
        entity_id: i32,
        dimension: String,
        slots: Vec<EquipmentEntry>,
    },
    BlockChanged {
        profile_id: uuid::Uuid,
        dimension: String,
        position: qexed_packet::net_types::Position,
        block_state: i32,
        light_update: Option<Bytes>,
    },
    BlockChanges {
        profile_id: uuid::Uuid,
        dimension: String,
        changes: Vec<BlockChange>,
    },
    ClientboundPackets {
        packets: Vec<Bytes>,
    },
}

#[derive(Debug, Clone)]
pub struct OnlinePlayer {
    pub profile: qexed_packet::net_types::GameProfile,
    pub entity_id: i32,
    pub game_mode: i32,
    pub position: EntityPosition,
    pub dimension: String,
    pub equipment: Vec<EquipmentEntry>,
    pub language: String,
    pub displayed_skin_parts: u8,
}

#[derive(Debug)]
pub struct PlayerSession {
    pub player: OnlinePlayer,
    pub receiver: mpsc::UnboundedReceiver<PlayerEvent>,
}

#[derive(Debug)]
pub(crate) struct PlayerHandle {
    pub(crate) player: OnlinePlayer,
    pub(crate) sender: mpsc::UnboundedSender<PlayerEvent>,
}

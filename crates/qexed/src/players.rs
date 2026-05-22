use std::{
    collections::HashMap,
    sync::{
        Mutex,
        atomic::{AtomicI32, Ordering},
    },
};

use bytes::{Bytes, BytesMut};
use qexed_packet::{Packet, PacketCodec};
use qexed_protocol::to_client::play::{
    add_entity::{
        EntityPosition, EntityPositionSync, PlayerInfoRemove, RemoveEntities, RotateHead,
    },
    player_info_update::{PlayerInfoActions, PlayerInfoEntry, PlayerInfoUpdate},
};
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub enum PlayerEvent {
    Joined(OnlinePlayer),
    Left {
        profile_id: uuid::Uuid,
        entity_id: i32,
    },
    Moved {
        profile_id: uuid::Uuid,
        entity_id: i32,
        position: EntityPosition,
    },
}

#[derive(Debug, Clone)]
pub struct OnlinePlayer {
    pub profile: qexed_packet::net_types::GameProfile,
    pub entity_id: i32,
    pub position: EntityPosition,
}

#[derive(Debug)]
pub struct PlayerSession {
    pub player: OnlinePlayer,
    pub receiver: mpsc::UnboundedReceiver<PlayerEvent>,
}

#[derive(Debug, Default)]
pub struct PlayerManager {
    next_entity_id: AtomicI32,
    players: Mutex<HashMap<uuid::Uuid, PlayerHandle>>,
}

impl PlayerManager {
    pub fn new() -> Self {
        Self {
            next_entity_id: AtomicI32::new(1),
            players: Mutex::new(HashMap::new()),
        }
    }

    pub fn join(
        &self,
        profile: qexed_packet::net_types::GameProfile,
        position: EntityPosition,
    ) -> PlayerSession {
        let entity_id = self.next_entity_id.fetch_add(1, Ordering::Relaxed);
        let player = OnlinePlayer {
            profile,
            entity_id,
            position,
        };
        let (sender, receiver) = mpsc::unbounded_channel();

        let mut players = self.players.lock().expect("player manager poisoned");
        players.insert(
            player.profile.uuid,
            PlayerHandle {
                player: player.clone(),
                sender,
            },
        );
        broadcast_locked(
            &players,
            player.profile.uuid,
            PlayerEvent::Joined(player.clone()),
        );

        PlayerSession { player, receiver }
    }

    pub fn list_except(&self, profile_id: uuid::Uuid) -> Vec<OnlinePlayer> {
        self.players
            .lock()
            .expect("player manager poisoned")
            .iter()
            .filter_map(|(id, handle)| (*id != profile_id).then_some(handle.player.clone()))
            .collect()
    }

    pub fn update_position(&self, profile_id: uuid::Uuid, position: EntityPosition) {
        let mut players = self.players.lock().expect("player manager poisoned");
        let Some(handle) = players.get_mut(&profile_id) else {
            return;
        };
        handle.player.position = position;
        let entity_id = handle.player.entity_id;
        broadcast_locked(
            &players,
            profile_id,
            PlayerEvent::Moved {
                profile_id,
                entity_id,
                position,
            },
        );
    }

    pub fn leave(&self, profile_id: uuid::Uuid) {
        let mut players = self.players.lock().expect("player manager poisoned");
        let Some(handle) = players.remove(&profile_id) else {
            return;
        };
        broadcast_locked(
            &players,
            profile_id,
            PlayerEvent::Left {
                profile_id,
                entity_id: handle.player.entity_id,
            },
        );
    }
}

impl PlayerEvent {
    pub fn packets(&self, player_entity_type: i32) -> anyhow::Result<Vec<bytes::Bytes>> {
        match self {
            Self::Joined(player) => spawn_player_packets(player, player_entity_type),
            Self::Left {
                profile_id,
                entity_id,
            } => Ok(vec![
                packet_bytes(RemoveEntities::one(*entity_id))?,
                packet_bytes(PlayerInfoRemove::one(*profile_id))?,
            ]),
            Self::Moved {
                profile_id: _,
                entity_id,
                position,
            } => Ok(vec![
                packet_bytes(EntityPositionSync::from_position(*entity_id, *position))?,
                packet_bytes(RotateHead::new(*entity_id, position.yaw))?,
            ]),
        }
    }
}

pub fn spawn_player_packets(
    player: &OnlinePlayer,
    player_entity_type: i32,
) -> anyhow::Result<Vec<Bytes>> {
    Ok(vec![
        packet_bytes(PlayerInfoUpdate {
            actions: PlayerInfoActions::player_initializing(),
            entries: vec![PlayerInfoEntry::from_profile(&player.profile, 1)],
        })?,
        packet_bytes(
            qexed_protocol::to_client::play::add_entity::AddEntity::player(
                player.entity_id,
                player.profile.uuid,
                player_entity_type,
                player.position,
            ),
        )?,
        packet_bytes(RotateHead::new(player.entity_id, player.position.yaw))?,
    ])
}

fn packet_bytes<T: Packet>(packet: T) -> anyhow::Result<Bytes> {
    let mut buf = BytesMut::new();
    let mut writer = qexed_packet::PacketWriter::new(&mut buf);
    qexed_packet::net_types::VarInt(T::ID).serialize(&mut writer)?;
    packet.serialize(&mut writer)?;
    Ok(buf.freeze())
}

#[derive(Debug)]
struct PlayerHandle {
    player: OnlinePlayer,
    sender: mpsc::UnboundedSender<PlayerEvent>,
}

fn broadcast_locked(
    players: &HashMap<uuid::Uuid, PlayerHandle>,
    except: uuid::Uuid,
    event: PlayerEvent,
) {
    for (id, handle) in players {
        if *id != except {
            let _ = handle.sender.send(event.clone());
        }
    }
}

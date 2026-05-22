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
    set_equipment::{Equipment, SetEquipment},
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
    EquipmentChanged {
        profile_id: uuid::Uuid,
        entity_id: i32,
        slots: Vec<Equipment>,
    },
    BlockChanged {
        profile_id: uuid::Uuid,
        position: qexed_packet::net_types::Position,
        block_state: i32,
        light_update: Option<Bytes>,
    },
}

#[derive(Debug, Clone)]
pub struct OnlinePlayer {
    pub profile: qexed_packet::net_types::GameProfile,
    pub entity_id: i32,
    pub position: EntityPosition,
    pub equipment: Vec<Equipment>,
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
        equipment: Vec<Equipment>,
    ) -> PlayerSession {
        let entity_id = self.next_entity_id.fetch_add(1, Ordering::Relaxed);
        let player = OnlinePlayer {
            profile,
            entity_id,
            position,
            equipment,
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

    pub fn update_equipment(&self, profile_id: uuid::Uuid, slots: Vec<Equipment>) {
        let mut players = self.players.lock().expect("player manager poisoned");
        let Some(handle) = players.get_mut(&profile_id) else {
            return;
        };
        for slot in &slots {
            if let Some(existing) = handle
                .player
                .equipment
                .iter_mut()
                .find(|existing| existing.slot == slot.slot)
            {
                existing.item = slot.item.clone();
            } else {
                handle.player.equipment.push(slot.clone());
            }
        }
        let entity_id = handle.player.entity_id;
        broadcast_locked(
            &players,
            profile_id,
            PlayerEvent::EquipmentChanged {
                profile_id,
                entity_id,
                slots,
            },
        );
    }

    pub fn broadcast_block_changed(
        &self,
        profile_id: uuid::Uuid,
        position: qexed_packet::net_types::Position,
        block_state: i32,
        light_update: Option<Bytes>,
    ) {
        let players = self.players.lock().expect("player manager poisoned");
        broadcast_locked(
            &players,
            profile_id,
            PlayerEvent::BlockChanged {
                profile_id,
                position,
                block_state,
                light_update,
            },
        );
    }

    pub fn online_names(&self) -> Vec<String> {
        self.players
            .lock()
            .expect("player manager poisoned")
            .values()
            .map(|handle| handle.player.profile.username.clone())
            .collect()
    }

    pub fn online_count(&self) -> usize {
        self.players.lock().expect("player manager poisoned").len()
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
            Self::EquipmentChanged {
                profile_id: _,
                entity_id,
                slots,
            } => Ok(vec![packet_bytes(SetEquipment {
                entity_id: qexed_packet::net_types::VarInt(*entity_id),
                slots: slots.clone(),
            })?]),
            Self::BlockChanged {
                profile_id: _,
                position,
                block_state,
                light_update,
            } => {
                let mut packets = vec![packet_bytes(
                    qexed_protocol::to_client::play::block_update::BlockUpdate {
                        location: position.clone(),
                        block_state: qexed_packet::net_types::VarInt(*block_state),
                    },
                )?];
                if let Some(light_update) = light_update {
                    packets.push(light_update.clone());
                }
                Ok(packets)
            }
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
        packet_bytes(SetEquipment {
            entity_id: qexed_packet::net_types::VarInt(player.entity_id),
            slots: player.equipment.clone(),
        })?,
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

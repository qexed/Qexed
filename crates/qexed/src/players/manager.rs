use std::{collections::HashMap, sync::Mutex};

use bytes::Bytes;
use qexed_protocol::to_client::play::{add_entity::EntityPosition, set_equipment::Equipment};
use tokio::sync::mpsc;

use super::{OnlinePlayer, PlayerEvent, PlayerHandle, PlayerSession};

#[derive(Debug, Default)]
pub struct PlayerManager {
    entity_ids: std::sync::Arc<crate::entities::EntityIdAllocator>,
    players: Mutex<HashMap<uuid::Uuid, PlayerHandle>>,
}

impl PlayerManager {
    pub fn new(entity_ids: std::sync::Arc<crate::entities::EntityIdAllocator>) -> Self {
        Self {
            entity_ids,
            players: Mutex::new(HashMap::new()),
        }
    }

    pub fn join(
        &self,
        profile: qexed_packet::net_types::GameProfile,
        position: EntityPosition,
        equipment: Vec<Equipment>,
    ) -> PlayerSession {
        let entity_id = self.entity_ids.next();
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

    pub fn teleport_player(&self, profile_id: uuid::Uuid, position: EntityPosition) -> bool {
        let mut players = self.players.lock().expect("player manager poisoned");
        let Some(handle) = players.get_mut(&profile_id) else {
            return false;
        };
        handle.player.position = position;
        let entity_id = handle.player.entity_id;
        let _ = handle.sender.send(PlayerEvent::Teleport {
            profile_id,
            position,
        });
        broadcast_locked(
            &players,
            profile_id,
            PlayerEvent::Moved {
                profile_id,
                entity_id,
                position,
            },
        );
        true
    }

    pub fn player_by_name(&self, username: &str) -> Option<OnlinePlayer> {
        let username = username.trim();
        if username.is_empty() {
            return None;
        }
        self.players
            .lock()
            .expect("player manager poisoned")
            .values()
            .find(|handle| {
                handle
                    .player
                    .profile
                    .username
                    .eq_ignore_ascii_case(username)
            })
            .map(|handle| handle.player.clone())
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

    pub fn broadcast_packets(&self, packets: Vec<Bytes>) {
        let players = self.players.lock().expect("player manager poisoned");
        broadcast_all_locked(&players, PlayerEvent::ClientboundPackets { packets });
    }

    pub fn broadcast_packets_except(&self, profile_id: uuid::Uuid, packets: Vec<Bytes>) {
        let players = self.players.lock().expect("player manager poisoned");
        broadcast_locked(
            &players,
            profile_id,
            PlayerEvent::ClientboundPackets { packets },
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

    pub fn display_name(&self, profile_id: uuid::Uuid) -> String {
        self.players
            .lock()
            .expect("player manager poisoned")
            .get(&profile_id)
            .map(|handle| handle.player.profile.username.clone())
            .unwrap_or_else(|| profile_id.to_string())
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
                username: handle.player.profile.username,
            },
        );
    }
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

fn broadcast_all_locked(players: &HashMap<uuid::Uuid, PlayerHandle>, event: PlayerEvent) {
    for handle in players.values() {
        let _ = handle.sender.send(event.clone());
    }
}

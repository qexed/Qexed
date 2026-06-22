use std::{collections::HashMap, sync::Mutex};

use bytes::Bytes;
use qexed_protocol::to_client::play::{add_entity::EntityPosition, set_equipment::Equipment};
use tokio::sync::mpsc;

use crate::{
    OnlinePlayer, PlayerDamageKind, PlayerEvent, PlayerSession,
    model::{PlayerHandle, ProjectileHitPlayerEvent},
};

pub const DEFAULT_DISPLAYED_SKIN_PARTS: u8 = 0x7f;

#[derive(Debug, Default)]
pub struct PlayerManager {
    entity_ids: std::sync::Arc<qexed_entity::EntityIdAllocator>,
    players: Mutex<HashMap<uuid::Uuid, PlayerHandle>>,
}

impl PlayerManager {
    pub fn new(entity_ids: std::sync::Arc<qexed_entity::EntityIdAllocator>) -> Self {
        Self {
            entity_ids,
            players: Mutex::new(HashMap::new()),
        }
    }

    pub fn join(
        &self,
        profile: qexed_packet::net_types::GameProfile,
        position: EntityPosition,
        dimension: String,
        equipment: Vec<Equipment>,
        language: String,
    ) -> PlayerSession {
        self.join_with_skin_parts(
            profile,
            position,
            dimension,
            equipment,
            language,
            DEFAULT_DISPLAYED_SKIN_PARTS,
            0,
        )
    }

    pub fn join_with_skin_parts(
        &self,
        profile: qexed_packet::net_types::GameProfile,
        position: EntityPosition,
        dimension: String,
        equipment: Vec<Equipment>,
        language: String,
        displayed_skin_parts: u8,
        game_mode: i32,
    ) -> PlayerSession {
        self.join_with_entity_id(
            self.entity_ids.next(),
            profile,
            position,
            dimension,
            equipment,
            language,
            displayed_skin_parts,
            game_mode,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn join_with_entity_id(
        &self,
        entity_id: i32,
        profile: qexed_packet::net_types::GameProfile,
        position: EntityPosition,
        dimension: String,
        equipment: Vec<Equipment>,
        language: String,
        displayed_skin_parts: u8,
        game_mode: i32,
    ) -> PlayerSession {
        let player = OnlinePlayer {
            profile,
            entity_id,
            game_mode,
            position,
            dimension,
            equipment,
            language,
            displayed_skin_parts,
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
        let dimension = handle.player.dimension.clone();
        let entity_id = handle.player.entity_id;
        broadcast_locked(
            &players,
            profile_id,
            PlayerEvent::Moved {
                profile_id,
                entity_id,
                dimension,
                position,
            },
        );
    }

    pub fn update_position_and_dimension(
        &self,
        profile_id: uuid::Uuid,
        dimension: String,
        position: EntityPosition,
    ) {
        let mut players = self.players.lock().expect("player manager poisoned");
        let Some(handle) = players.get_mut(&profile_id) else {
            return;
        };
        let old_dimension = std::mem::replace(&mut handle.player.dimension, dimension.clone());
        handle.player.position = position;
        let changed_dimension = old_dimension != dimension;
        let entity_id = handle.player.entity_id;
        let player = handle.player.clone();
        let event = if changed_dimension {
            PlayerEvent::DimensionChanged {
                profile_id,
                entity_id,
                old_dimension,
                player,
            }
        } else {
            PlayerEvent::Moved {
                profile_id,
                entity_id,
                dimension,
                position,
            }
        };
        broadcast_locked(&players, profile_id, event);
    }

    pub fn teleport_player(
        &self,
        profile_id: uuid::Uuid,
        dimension: String,
        position: EntityPosition,
    ) -> bool {
        let mut players = self.players.lock().expect("player manager poisoned");
        let Some(handle) = players.get_mut(&profile_id) else {
            return false;
        };
        let old_dimension = std::mem::replace(&mut handle.player.dimension, dimension.clone());
        handle.player.position = position;
        let changed_dimension = old_dimension != dimension;
        let player = handle.player.clone();
        let entity_id = handle.player.entity_id;
        let _ = handle.sender.send(PlayerEvent::Teleport {
            profile_id,
            dimension: dimension.clone(),
            position,
        });
        let event = if changed_dimension {
            PlayerEvent::DimensionChanged {
                profile_id,
                entity_id,
                old_dimension,
                player,
            }
        } else {
            PlayerEvent::Moved {
                profile_id,
                entity_id,
                dimension,
                position,
            }
        };
        broadcast_locked(&players, profile_id, event);
        true
    }

    pub fn damage_player(
        &self,
        profile_id: uuid::Uuid,
        amount: f32,
        kind: PlayerDamageKind,
        source_entity_id: i32,
        source_position: EntityPosition,
        knockback: f32,
    ) -> bool {
        if amount <= 0.0 || !amount.is_finite() {
            return false;
        }
        let players = self.players.lock().expect("player manager poisoned");
        let Some(handle) = players.get(&profile_id) else {
            return false;
        };
        handle
            .sender
            .send(PlayerEvent::Damage {
                profile_id,
                amount,
                kind,
                source_entity_id,
                source_position,
                knockback: knockback.max(0.0),
            })
            .is_ok()
    }

    pub fn apply_potion_effect(
        &self,
        profile_id: uuid::Uuid,
        effect: impl Into<String>,
        amplifier: i32,
        duration_ticks: i32,
        source_entity_id: i32,
        source_position: EntityPosition,
        knockback: f32,
    ) -> bool {
        let effect = effect.into();
        if effect.trim().is_empty() || duration_ticks <= 0 {
            return false;
        }
        let players = self.players.lock().expect("player manager poisoned");
        let Some(handle) = players.get(&profile_id) else {
            return false;
        };
        handle
            .sender
            .send(PlayerEvent::PotionEffect {
                profile_id,
                effect,
                amplifier: amplifier.max(0),
                duration_ticks,
                source_entity_id,
                source_position,
                knockback: knockback.max(0.0),
            })
            .is_ok()
    }

    pub fn set_game_mode(&self, profile_id: uuid::Uuid, game_mode: i32) -> Option<OnlinePlayer> {
        let mut players = self.players.lock().expect("player manager poisoned");
        let player = {
            let handle = players.get_mut(&profile_id)?;
            handle.player.game_mode = game_mode;
            handle.player.clone()
        };
        broadcast_all_locked(
            &players,
            PlayerEvent::GameModeChanged {
                profile_id,
                username: player.profile.username.clone(),
                game_mode,
            },
        );
        Some(player)
    }

    pub fn give_item(
        &self,
        profile_id: uuid::Uuid,
        item: qexed_protocol::types::Slot,
        item_name: impl Into<String>,
    ) -> bool {
        let players = self.players.lock().expect("player manager poisoned");
        let Some(handle) = players.get(&profile_id) else {
            return false;
        };
        handle
            .sender
            .send(PlayerEvent::GiveItem {
                profile_id,
                item,
                item_name: item_name.into(),
            })
            .is_ok()
    }

    pub fn emit_projectile_hit_player(&self, event: ProjectileHitPlayerEvent) -> bool {
        let players = self.players.lock().expect("player manager poisoned");
        let Some(handle) = players.get(&event.shooter_profile_id) else {
            return false;
        };
        handle
            .sender
            .send(PlayerEvent::ProjectileHitPlayer(event))
            .is_ok()
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

    pub fn player_by_uuid(&self, profile_id: uuid::Uuid) -> Option<OnlinePlayer> {
        self.players
            .lock()
            .expect("player manager poisoned")
            .get(&profile_id)
            .map(|handle| handle.player.clone())
    }

    pub fn player_by_entity_id(&self, entity_id: i32) -> Option<OnlinePlayer> {
        self.players
            .lock()
            .expect("player manager poisoned")
            .values()
            .find(|handle| handle.player.entity_id == entity_id)
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
        let dimension = handle.player.dimension.clone();
        broadcast_locked(
            &players,
            profile_id,
            PlayerEvent::EquipmentChanged {
                profile_id,
                entity_id,
                dimension,
                slots,
            },
        );
    }

    pub fn broadcast_block_changed(
        &self,
        profile_id: uuid::Uuid,
        dimension: &str,
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
                dimension: dimension.to_string(),
                position,
                block_state,
                light_update,
            },
        );
    }

    pub fn broadcast_block_changes(
        &self,
        profile_id: uuid::Uuid,
        dimension: &str,
        changes: Vec<crate::BlockChange>,
    ) {
        if changes.is_empty() {
            return;
        }
        let players = self.players.lock().expect("player manager poisoned");
        broadcast_locked(
            &players,
            profile_id,
            PlayerEvent::BlockChanges {
                profile_id,
                dimension: dimension.to_string(),
                changes,
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

    pub fn send_packets_to(&self, profile_id: uuid::Uuid, packets: Vec<Bytes>) {
        let players = self.players.lock().expect("player manager poisoned");
        if let Some(handle) = players.get(&profile_id) {
            let _ = handle
                .sender
                .send(PlayerEvent::ClientboundPackets { packets });
        }
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
                dimension: handle.player.dimension,
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

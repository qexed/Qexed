use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicI32, Ordering},
    },
};

use anyhow::{Context, Result};
use bytes::Bytes;
use qexed_packet::net_types::{GameProfile, VarInt};
use qexed_protocol::{
    to_client::play::{
        add_entity::{
            AddEntity, EntityPosition, EntityPositionSync, PlayerInfoRemove, RemoveEntities,
            RotateHead,
        },
        player_info_update::{PlayerInfoActions, PlayerInfoEntry, PlayerInfoUpdate},
        set_entity_data::SetEntityData,
    },
    types::{EntityMetadata, EntityMetadataEnum, EntityMetadataSub},
};

const REGISTRIES_REPORT: &str = "assets/reports/registries.json";

#[derive(Debug)]
pub struct EntityIdAllocator {
    next_entity_id: AtomicI32,
}

impl Default for EntityIdAllocator {
    fn default() -> Self {
        Self::new(1)
    }
}

impl EntityIdAllocator {
    pub fn new(first_entity_id: i32) -> Self {
        Self {
            next_entity_id: AtomicI32::new(first_entity_id.max(1)),
        }
    }

    pub fn next(&self) -> i32 {
        self.next_entity_id.fetch_add(1, Ordering::Relaxed)
    }
}

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
    pub data: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManagedEntityKind {
    Entity,
    Npc,
}

#[derive(Debug)]
pub struct EntityManager {
    entity_ids: std::sync::Arc<EntityIdAllocator>,
    entities: Mutex<Vec<ManagedEntity>>,
}

impl EntityManager {
    pub fn from_config(
        config: &qexed_config::app::qexed::server::Entities,
        entity_ids: std::sync::Arc<EntityIdAllocator>,
    ) -> Result<Self> {
        let manager = Self {
            entity_ids,
            entities: Mutex::new(Vec::new()),
        };

        if !config.enable {
            return Ok(manager);
        }

        for (index, entity) in config.list.iter().enumerate() {
            manager.spawn_configured(index, &config.dimension, entity)?;
        }

        Ok(manager)
    }

    pub fn list_for_dimension(&self, dimension: &str) -> Vec<ManagedEntity> {
        self.entities
            .lock()
            .expect("entity manager poisoned")
            .iter()
            .filter(|entity| entity.dimension == dimension)
            .cloned()
            .collect()
    }

    pub fn spawn_packets_for_dimension(&self, dimension: &str) -> Result<Vec<Bytes>> {
        let mut packets = Vec::new();
        for entity in self.list_for_dimension(dimension) {
            packets.extend(entity.spawn_packets()?);
        }
        Ok(packets)
    }

    pub fn spawn(
        &self,
        players: &crate::players::PlayerManager,
        request: EntitySpawnRequest,
    ) -> Result<ManagedEntity> {
        let key = request.key.trim();
        if key.is_empty() {
            anyhow::bail!("entity id cannot be empty");
        }
        let entity_type = match request.kind {
            ManagedEntityKind::Entity => {
                let entity_type = request.entity_type.trim();
                if entity_type.is_empty() {
                    "minecraft:armor_stand".to_string()
                } else {
                    entity_type.to_string()
                }
            }
            ManagedEntityKind::Npc => "minecraft:player".to_string(),
        };
        let entity = ManagedEntity {
            entity_id: self.entity_ids.next(),
            uuid: stable_entity_uuid(key),
            key: key.to_string(),
            kind: request.kind,
            entity_type_id: entity_type_id(&entity_type)?,
            entity_type,
            dimension: request.dimension,
            position: request.position,
            name: request.name,
            data: request.data,
        };

        let packets = entity.spawn_packets()?;
        let mut entities = self.entities.lock().expect("entity manager poisoned");
        if entities.iter().any(|existing| existing.key == entity.key) {
            anyhow::bail!("duplicate entity id: {}", entity.key);
        }
        entities.push(entity.clone());
        drop(entities);
        players.broadcast_packets(packets);
        Ok(entity)
    }

    pub fn move_entity(
        &self,
        players: &crate::players::PlayerManager,
        key: &str,
        position: EntityPosition,
    ) -> Result<()> {
        let mut entities = self.entities.lock().expect("entity manager poisoned");
        let entity = entities
            .iter_mut()
            .find(|entity| entity.key == key)
            .with_context(|| format!("entity not found: {key}"))?;
        entity.position = position;
        let packets = entity.position_packets()?;
        drop(entities);
        players.broadcast_packets(packets);
        Ok(())
    }

    pub fn remove(&self, players: &crate::players::PlayerManager, key: &str) -> Result<()> {
        let mut entities = self.entities.lock().expect("entity manager poisoned");
        let index = entities
            .iter()
            .position(|entity| entity.key == key)
            .with_context(|| format!("entity not found: {key}"))?;
        let entity = entities.remove(index);
        let packets = entity.remove_packets()?;
        drop(entities);
        players.broadcast_packets(packets);
        Ok(())
    }

    fn spawn_configured(
        &self,
        index: usize,
        dimension: &str,
        config: &qexed_config::app::qexed::server::Entity,
    ) -> Result<ManagedEntity> {
        let key = configured_entity_key(index, config);
        let kind = match config.kind {
            qexed_config::app::qexed::server::EntityKind::Entity => ManagedEntityKind::Entity,
            qexed_config::app::qexed::server::EntityKind::Npc => ManagedEntityKind::Npc,
        };
        let entity_type = match kind {
            ManagedEntityKind::Entity => {
                let entity_type = config.entity_type.trim();
                if entity_type.is_empty() {
                    "minecraft:armor_stand"
                } else {
                    entity_type
                }
            }
            ManagedEntityKind::Npc => "minecraft:player",
        }
        .to_string();
        let entity_type_id = entity_type_id(&entity_type)?;
        let entity = ManagedEntity {
            uuid: stable_entity_uuid(&key),
            entity_id: self.entity_ids.next(),
            key,
            kind,
            entity_type,
            entity_type_id,
            dimension: dimension.to_string(),
            position: EntityPosition {
                x: config.x,
                y: config.y,
                z: config.z,
                yaw: config.yaw,
                pitch: config.pitch,
                on_ground: config.on_ground,
            },
            name: config.name.clone(),
            data: config.data,
        };

        let mut entities = self.entities.lock().expect("entity manager poisoned");
        if entities.iter().any(|existing| existing.key == entity.key) {
            anyhow::bail!("duplicate configured entity id: {}", entity.key);
        }
        entities.push(entity.clone());
        Ok(entity)
    }
}

#[derive(Debug, Clone)]
pub struct EntitySpawnRequest {
    pub key: String,
    pub kind: ManagedEntityKind,
    pub entity_type: String,
    pub dimension: String,
    pub position: EntityPosition,
    pub name: String,
    pub data: i32,
}

impl ManagedEntity {
    pub fn spawn_packets(&self) -> Result<Vec<Bytes>> {
        let mut packets = Vec::new();
        if self.kind == ManagedEntityKind::Npc {
            packets.push(crate::players::packet_bytes(PlayerInfoUpdate {
                actions: PlayerInfoActions::player_initializing(),
                entries: vec![npc_player_info_entry(&self.profile(), 0)],
            })?);
        }

        packets.push(crate::players::packet_bytes(AddEntity::new(
            self.entity_id,
            self.uuid,
            self.entity_type_id,
            self.position,
            self.data,
        ))?);
        packets.push(crate::players::packet_bytes(RotateHead::new(
            self.entity_id,
            self.position.yaw,
        ))?);

        if let Some(name) = self.display_name() {
            let metadata = named_entity_metadata(name);
            packets.push(crate::players::packet_bytes(SetEntityData {
                entity_id: VarInt(self.entity_id),
                metadata,
            })?);
        }

        Ok(packets)
    }

    pub fn remove_packets(&self) -> Result<Vec<Bytes>> {
        let mut packets = vec![crate::players::packet_bytes(RemoveEntities::one(
            self.entity_id,
        ))?];
        if self.kind == ManagedEntityKind::Npc {
            packets.push(crate::players::packet_bytes(PlayerInfoRemove::one(
                self.uuid,
            ))?);
        }
        Ok(packets)
    }

    pub fn position_packets(&self) -> Result<Vec<Bytes>> {
        Ok(vec![
            crate::players::packet_bytes(EntityPositionSync::from_position(
                self.entity_id,
                self.position,
            ))?,
            crate::players::packet_bytes(RotateHead::new(self.entity_id, self.position.yaw))?,
        ])
    }

    fn profile(&self) -> GameProfile {
        GameProfile {
            uuid: self.uuid,
            username: npc_profile_name(self.profile_name()),
            properties: Vec::new(),
        }
    }

    fn display_name(&self) -> Option<&str> {
        let name = self.name.trim();
        if !name.is_empty() {
            return Some(name);
        }
        if self.kind == ManagedEntityKind::Npc {
            Some(&self.key)
        } else {
            None
        }
    }

    fn profile_name(&self) -> &str {
        self.display_name().unwrap_or(&self.key)
    }
}

pub(crate) fn entity_type_id(name: &str) -> Result<i32> {
    entity_type_registry()
        .get(name)
        .copied()
        .with_context(|| format!("missing entity type registry id: {name}"))
}

fn entity_type_registry() -> &'static HashMap<String, i32> {
    static REGISTRY: OnceLock<HashMap<String, i32>> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        load_registry_id_map("minecraft:entity_type").unwrap_or_else(|err| {
            log::warn!("failed to load entity type registry ids: {err:#}");
            HashMap::from([
                ("minecraft:armor_stand".to_string(), 5),
                ("minecraft:player".to_string(), 155),
                ("minecraft:villager".to_string(), 139),
            ])
        })
    })
}

fn load_registry_id_map(registry_id: &str) -> Result<HashMap<String, i32>> {
    let path = workspace_root().join(REGISTRIES_REPORT);
    let content =
        std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    let value: serde_json::Value =
        serde_json::from_str(&content).with_context(|| format!("parse {}", path.display()))?;
    let entries = value
        .get(registry_id)
        .and_then(|registry| registry.get("entries"))
        .and_then(serde_json::Value::as_object)
        .with_context(|| format!("registry not found in {}: {registry_id}", path.display()))?;

    let mut ids = HashMap::new();
    for (name, value) in entries {
        let Some(id) = value
            .get("protocol_id")
            .and_then(serde_json::Value::as_i64)
            .and_then(|id| i32::try_from(id).ok())
        else {
            continue;
        };
        ids.insert(name.clone(), id);
    }

    Ok(ids)
}

fn configured_entity_key(
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

fn stable_entity_uuid(key: &str) -> uuid::Uuid {
    uuid::Uuid::new_v3(
        &uuid::Uuid::NAMESPACE_OID,
        format!("qexed:entity:{key}").as_bytes(),
    )
}

fn npc_player_info_entry(profile: &GameProfile, game_mode: i32) -> PlayerInfoEntry {
    let mut entry = PlayerInfoEntry::from_profile(profile, game_mode);
    entry.listed = false;
    entry
}

fn npc_profile_name(name: &str) -> String {
    let mut username = name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .take(16)
        .collect::<String>();

    while username.starts_with('_') {
        username.remove(0);
    }
    while username.ends_with('_') {
        username.pop();
    }
    if username.is_empty() {
        username.push_str("NPC");
    }
    while username.len() < 3 {
        username.push('_');
    }
    username
}

fn named_entity_metadata(name: &str) -> EntityMetadata {
    EntityMetadata {
        data: vec![
            EntityMetadataSub {
                index: 2,
                data: Some(EntityMetadataEnum::OptionTextComponent(Some(
                    text_component(name),
                ))),
            },
            EntityMetadataSub {
                index: 3,
                data: Some(EntityMetadataEnum::Boolean(true)),
            },
            EntityMetadataSub {
                index: 0xff,
                data: None,
            },
        ],
    }
}

fn text_component(text: impl Into<String>) -> qexed_protocol::types::TextComponent {
    let mut map = std::collections::HashMap::new();
    map.insert(
        "text".to_string(),
        qexed_nbt::Tag::String(std::sync::Arc::from(text.into())),
    );
    qexed_nbt::Tag::Compound(std::sync::Arc::new(map))
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

#[cfg(test)]
mod tests {
    use super::{
        EntityIdAllocator, EntityManager, ManagedEntityKind, entity_type_id, npc_profile_name,
    };

    #[test]
    fn entity_type_id_is_loaded_from_current_report() {
        assert_eq!(entity_type_id("minecraft:player").unwrap(), 155);
        assert_eq!(entity_type_id("minecraft:armor_stand").unwrap(), 5);
    }

    #[test]
    fn configured_entities_allocate_before_players() {
        let entity_ids = std::sync::Arc::new(EntityIdAllocator::new(1));
        let config = qexed_config::app::qexed::server::Entities {
            enable: true,
            dimension: "minecraft:overworld".to_string(),
            list: vec![
                qexed_config::app::qexed::server::Entity {
                    id: "spawn-guide".to_string(),
                    name: "Guide".to_string(),
                    kind: qexed_config::app::qexed::server::EntityKind::Npc,
                    ..Default::default()
                },
                qexed_config::app::qexed::server::Entity {
                    id: "marker".to_string(),
                    entity_type: "minecraft:armor_stand".to_string(),
                    ..Default::default()
                },
            ],
        };

        let manager = EntityManager::from_config(&config, entity_ids.clone()).unwrap();
        let entities = manager.list_for_dimension("minecraft:overworld");

        assert_eq!(entities.len(), 2);
        assert_eq!(entities[0].entity_id, 1);
        assert_eq!(entities[0].kind, ManagedEntityKind::Npc);
        assert_eq!(entities[1].entity_id, 2);
        assert_eq!(entity_ids.next(), 3);
    }

    #[test]
    fn npc_profile_name_is_minecraft_safe() {
        assert_eq!(npc_profile_name("向导 NPC!"), "NPC");
        assert_eq!(npc_profile_name("ab"), "ab_");
        assert_eq!(
            npc_profile_name("Guide_0123456789012345"),
            "Guide_0123456789"
        );
    }
}

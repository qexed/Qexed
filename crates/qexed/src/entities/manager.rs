use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use bytes::Bytes;
use qexed_protocol::to_client::play::add_entity::EntityPosition;
use serde::Deserialize;

use super::{
    DroppedItemEntity, EntityIdAllocator, EntitySpawnRequest, ManagedEntity, ManagedEntityKind,
    model::{configured_entity_key, stable_entity_uuid},
    registry::entity_type_id,
};

const ITEM_PICKUP_DELAY: Duration = Duration::from_millis(500);
const ITEM_PICKUP_RADIUS_XZ: f64 = 1.5;
const ITEM_PICKUP_RADIUS_Y: f64 = 1.5;

#[derive(Debug)]
pub struct EntityManager {
    entity_ids: Arc<EntityIdAllocator>,
    entities: Mutex<Vec<ManagedEntity>>,
    dropped_items: Mutex<Vec<DroppedItemEntity>>,
}

impl EntityManager {
    pub fn from_config(
        config: &qexed_config::app::qexed::server::Entities,
        entity_ids: Arc<EntityIdAllocator>,
    ) -> Result<Self> {
        let manager = Self {
            entity_ids,
            entities: Mutex::new(Vec::new()),
            dropped_items: Mutex::new(Vec::new()),
        };

        if !config.enable {
            return Ok(manager);
        }

        for (index, entity) in config.list.iter().enumerate() {
            manager.spawn_configured(index, &config.dimension, entity)?;
        }

        Ok(manager)
    }

    pub async fn from_config_with_skin_lookup(
        config: &qexed_config::app::qexed::server::Entities,
        entity_ids: Arc<EntityIdAllocator>,
    ) -> Result<Self> {
        let mut resolved = config.clone();
        if !resolved.enable {
            return Self::from_config(&resolved, entity_ids);
        }

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(8))
            .build()
            .context("failed to create mojang skin lookup client")?;

        for entity in &mut resolved.list {
            if entity.kind != qexed_config::app::qexed::server::EntityKind::Npc {
                continue;
            }
            if !entity.skin_textures.trim().is_empty() {
                continue;
            }
            let player_id = entity.skin_player_id.trim();
            if player_id.is_empty() {
                continue;
            }

            match resolve_skin_by_player_id(&client, player_id).await {
                Ok(Some(skin)) => {
                    entity.skin_textures = skin.value;
                    if let Some(signature) = skin.signature {
                        entity.skin_signature = signature;
                    }
                }
                Ok(None) => {
                    log::warn!(
                        "npc skin lookup returned no textures: id={}, npc_id={}",
                        player_id,
                        entity.id
                    );
                }
                Err(err) => {
                    log::warn!(
                        "npc skin lookup failed: id={}, npc_id={}, error={err:#}",
                        player_id,
                        entity.id
                    );
                }
            }
        }

        Self::from_config(&resolved, entity_ids)
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

    pub fn entity_by_runtime_id(&self, entity_id: i32) -> Option<ManagedEntity> {
        self.entities
            .lock()
            .expect("entity manager poisoned")
            .iter()
            .find(|entity| entity.entity_id == entity_id)
            .cloned()
    }

    pub fn spawn_packets_for_dimension(&self, dimension: &str) -> Result<Vec<Bytes>> {
        let mut packets = Vec::new();
        for entity in self.list_for_dimension(dimension) {
            packets.extend(entity.spawn_packets()?);
        }
        let item_entity_type = entity_type_id("minecraft:item")?;
        let dropped_items = self
            .dropped_items
            .lock()
            .expect("entity manager dropped items poisoned");
        for item in dropped_items
            .iter()
            .filter(|item| item.dimension == dimension)
        {
            packets.extend(item.spawn_packets(item_entity_type)?);
        }
        Ok(packets)
    }

    pub fn drop_item(
        &self,
        players: &crate::players::PlayerManager,
        actor: uuid::Uuid,
        dimension: &str,
        position: EntityPosition,
        item: qexed_protocol::types::Slot,
    ) -> Result<Option<DroppedItemEntity>> {
        if item.item_count.0 <= 0 {
            return Ok(None);
        }

        let entity = DroppedItemEntity {
            entity_id: self.entity_ids.next(),
            uuid: uuid::Uuid::new_v4(),
            dimension: dimension.to_string(),
            position,
            item,
            pickup_ready_at: Instant::now() + ITEM_PICKUP_DELAY,
        };
        let packets = entity.spawn_packets(entity_type_id("minecraft:item")?)?;
        self.dropped_items
            .lock()
            .expect("entity manager dropped items poisoned")
            .push(entity.clone());
        players.broadcast_packets_except(actor, packets);
        Ok(Some(entity))
    }

    pub fn collect_reachable_items(
        &self,
        dimension: &str,
        collector: EntityPosition,
    ) -> Result<Vec<DroppedItemEntity>> {
        let now = Instant::now();
        let mut dropped_items = self
            .dropped_items
            .lock()
            .expect("entity manager dropped items poisoned");
        let mut collected = Vec::new();
        let mut index = 0;
        while index < dropped_items.len() {
            if dropped_items[index].dimension == dimension
                && dropped_items[index].pickup_ready_at <= now
                && can_reach_item(collector, dropped_items[index].position)
            {
                collected.push(dropped_items.remove(index));
            } else {
                index += 1;
            }
        }
        Ok(collected)
    }

    pub fn restore_dropped_item(&self, item: DroppedItemEntity) {
        self.dropped_items
            .lock()
            .expect("entity manager dropped items poisoned")
            .push(item);
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
            ManagedEntityKind::Hologram => "minecraft:text_display".to_string(),
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
            display_name: request.display_name,
            skin_textures: request.skin_textures,
            skin_signature: request.skin_signature,
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
            qexed_config::app::qexed::server::EntityKind::Hologram => ManagedEntityKind::Hologram,
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
            ManagedEntityKind::Hologram => "minecraft:text_display",
        }
        .to_string();
        let entity_type_id = entity_type_id(&entity_type)?;
        let display_name = {
            let name = config.display_name.trim();
            if !name.is_empty() {
                name.to_string()
            } else {
                config.name.clone()
            }
        };
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
            display_name,
            skin_textures: config.skin_textures.clone(),
            skin_signature: config.skin_signature.clone(),
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

#[derive(Debug, Deserialize)]
struct MojangNameLookup {
    id: String,
}

#[derive(Debug, Deserialize)]
struct MojangProfile {
    properties: Vec<MojangProperty>,
}

#[derive(Debug, Deserialize)]
struct MojangProperty {
    name: String,
    value: String,
    #[serde(default)]
    signature: Option<String>,
}

async fn resolve_skin_by_player_id(
    client: &reqwest::Client,
    player_id: &str,
) -> Result<Option<MojangProperty>> {
    let Some(uuid) = resolve_uuid(client, player_id).await? else {
        return Ok(None);
    };

    let url =
        format!("https://sessionserver.mojang.com/session/minecraft/profile/{uuid}?unsigned=false");
    let response = client.get(url).send().await?;
    if response.status() == reqwest::StatusCode::NO_CONTENT
        || response.status() == reqwest::StatusCode::NOT_FOUND
    {
        return Ok(None);
    }
    let response = response.error_for_status()?;
    let profile: MojangProfile = response.json().await?;
    Ok(profile
        .properties
        .into_iter()
        .find(|property| property.name == "textures"))
}

async fn resolve_uuid(client: &reqwest::Client, player_id: &str) -> Result<Option<String>> {
    if let Some(uuid) = normalize_uuid(player_id) {
        return Ok(Some(uuid));
    }

    let name = player_id.trim();
    if name.is_empty() {
        return Ok(None);
    }

    let url = format!("https://api.mojang.com/users/profiles/minecraft/{name}");
    let response = client.get(url).send().await?;
    if response.status() == reqwest::StatusCode::NO_CONTENT
        || response.status() == reqwest::StatusCode::NOT_FOUND
    {
        return Ok(None);
    }
    let response = response.error_for_status()?;
    let data: MojangNameLookup = response.json().await?;
    Ok(Some(data.id))
}

fn normalize_uuid(input: &str) -> Option<String> {
    let id = input.trim();
    if id.is_empty() {
        return None;
    }
    let parsed = uuid::Uuid::parse_str(id).ok()?;
    Some(parsed.simple().to_string())
}

#[cfg(test)]
mod tests {
    use super::normalize_uuid;

    #[test]
    fn normalize_uuid_accepts_dashed_and_compact() {
        assert_eq!(
            normalize_uuid("069a79f4-44e9-4726-a5be-fca90e38aaf5"),
            Some("069a79f444e94726a5befca90e38aaf5".to_string())
        );
        assert_eq!(
            normalize_uuid("069a79f444e94726a5befca90e38aaf5"),
            Some("069a79f444e94726a5befca90e38aaf5".to_string())
        );
    }

    #[test]
    fn normalize_uuid_rejects_invalid_input() {
        assert_eq!(normalize_uuid(""), None);
        assert_eq!(normalize_uuid("not-a-uuid"), None);
    }
}

fn can_reach_item(collector: EntityPosition, item: EntityPosition) -> bool {
    (collector.x - item.x).abs() <= ITEM_PICKUP_RADIUS_XZ
        && (collector.y + 0.9 - item.y).abs() <= ITEM_PICKUP_RADIUS_Y
        && (collector.z - item.z).abs() <= ITEM_PICKUP_RADIUS_XZ
}

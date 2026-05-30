use std::{
    collections::{HashMap, HashSet},
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

    pub fn entity_by_key(&self, key: &str) -> Option<ManagedEntity> {
        self.entities
            .lock()
            .expect("entity manager poisoned")
            .iter()
            .find(|entity| entity.key == key)
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

    pub fn spawn_packets_for_view(
        &self,
        dimension: &str,
        viewer_position: EntityPosition,
        rendering: &qexed_config::app::qexed::server::EntityRendering,
    ) -> Result<Vec<Bytes>> {
        let mut packets = Vec::new();
        for entity in self.visible_entities_for_view(dimension, viewer_position, rendering) {
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
            .filter(|item| {
                within_render_distance(item.position, viewer_position, rendering.item_distance)
            })
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

        let entity = self.create_dropped_item(dimension, position, item);
        let packets = entity.spawn_packets(entity_type_id("minecraft:item")?)?;
        self.dropped_items
            .lock()
            .expect("entity manager dropped items poisoned")
            .push(entity.clone());
        players.broadcast_packets_except(actor, packets);
        Ok(Some(entity))
    }

    pub fn drop_item_with_rendering(
        &self,
        players: &crate::players::PlayerManager,
        actor: uuid::Uuid,
        dimension: &str,
        position: EntityPosition,
        item: qexed_protocol::types::Slot,
        rendering: &qexed_config::app::qexed::server::EntityRendering,
    ) -> Result<Option<DroppedItemEntity>> {
        let Some(entity) = self.drop_item_local(dimension, position, item)? else {
            return Ok(None);
        };
        let packets = entity.spawn_packets(entity_type_id("minecraft:item")?)?;
        for player in players.list_except(actor) {
            if player.dimension == dimension
                && within_render_distance(player.position, position, rendering.item_distance)
            {
                players.send_packets_to(player.profile.uuid, packets.clone());
            }
        }
        Ok(Some(entity))
    }

    pub fn send_spawn_to_rendered_viewers(
        &self,
        players: &crate::players::PlayerManager,
        rendering: &qexed_config::app::qexed::server::EntityRendering,
        entity: &ManagedEntity,
    ) -> Result<()> {
        self.refresh_managed_entities_for_viewers(players, rendering, &[entity.dimension.clone()])
    }

    pub fn refresh_managed_entities_for_viewers(
        &self,
        players: &crate::players::PlayerManager,
        rendering: &qexed_config::app::qexed::server::EntityRendering,
        dimensions: &[String],
    ) -> Result<()> {
        if dimensions.is_empty() {
            return Ok(());
        }

        let dimension_set = dimensions.iter().collect::<HashSet<_>>();
        for player in players.list_except(uuid::Uuid::nil()) {
            if !dimension_set.contains(&player.dimension) {
                continue;
            }
            players.send_packets_to(
                player.profile.uuid,
                self.managed_entity_view_packets(&player.dimension, player.position, rendering)?,
            );
        }
        Ok(())
    }

    pub fn send_move_to_rendered_viewers(
        &self,
        players: &crate::players::PlayerManager,
        rendering: &qexed_config::app::qexed::server::EntityRendering,
        entity: &ManagedEntity,
    ) -> Result<()> {
        self.refresh_managed_entities_for_viewers(players, rendering, &[entity.dimension.clone()])
    }

    pub fn update_look_at_npcs(
        &self,
        players: &crate::players::PlayerManager,
        rendering: &qexed_config::app::qexed::server::EntityRendering,
    ) -> Result<()> {
        let viewers = players.list_except(uuid::Uuid::nil());
        if viewers.is_empty() {
            return Ok(());
        }

        let mut updates = Vec::new();
        {
            let mut entities = self.entities.lock().expect("entity manager poisoned");
            for entity in entities.iter_mut() {
                if entity.kind != ManagedEntityKind::Npc || !entity.look_at_players {
                    continue;
                }
                let Some(target) = viewers
                    .iter()
                    .filter(|player| player.dimension == entity.dimension)
                    .filter(|player| {
                        within_render_distance(
                            entity.position,
                            player.position,
                            rendering.npc_distance,
                        )
                    })
                    .min_by(|left, right| {
                        horizontal_distance_sq(entity.position, left.position)
                            .total_cmp(&horizontal_distance_sq(entity.position, right.position))
                    })
                else {
                    continue;
                };

                let (yaw, pitch) = look_rotation(entity.position, target.position);
                if (entity.position.yaw - yaw).abs() < 0.5
                    && (entity.position.pitch - pitch).abs() < 0.5
                {
                    continue;
                }
                entity.position.yaw = yaw;
                entity.position.pitch = pitch;
                updates.push(entity.clone());
            }
        }

        for entity in updates {
            let packets = entity.position_packets()?;
            for player in &viewers {
                if player.dimension == entity.dimension
                    && within_render_distance(
                        entity.position,
                        player.position,
                        render_distance_for_entity(&entity, rendering),
                    )
                {
                    players.send_packets_to(player.profile.uuid, packets.clone());
                }
            }
        }
        Ok(())
    }

    pub fn send_remove_to_rendered_viewers(
        &self,
        players: &crate::players::PlayerManager,
        rendering: &qexed_config::app::qexed::server::EntityRendering,
        entity: &ManagedEntity,
    ) -> Result<()> {
        let remove_packets = entity.remove_packets()?;
        for player in players.list_except(uuid::Uuid::nil()) {
            if player.dimension == entity.dimension {
                players.send_packets_to(player.profile.uuid, remove_packets.clone());
            }
        }
        self.refresh_managed_entities_for_viewers(players, rendering, &[entity.dimension.clone()])
    }

    pub fn drop_item_local(
        &self,
        dimension: &str,
        position: EntityPosition,
        item: qexed_protocol::types::Slot,
    ) -> Result<Option<DroppedItemEntity>> {
        if item.item_count.0 <= 0 {
            return Ok(None);
        }

        let entity = self.create_dropped_item(dimension, position, item);
        self.dropped_items
            .lock()
            .expect("entity manager dropped items poisoned")
            .push(entity.clone());
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
        let entity = self.spawn_local(request)?;
        let packets = entity.spawn_packets()?;
        players.broadcast_packets(packets);
        Ok(entity)
    }

    pub fn spawn_local(&self, request: EntitySpawnRequest) -> Result<ManagedEntity> {
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
            look_at_players: request.look_at_players,
            main_hand_event: normalized_npc_event(&request.main_hand_event, "interact"),
            off_hand_event: normalized_npc_event(&request.off_hand_event, "interact_off_hand"),
            attack_event: normalized_npc_event(&request.attack_event, "attack"),
        };

        let mut entities = self.entities.lock().expect("entity manager poisoned");
        if entities.iter().any(|existing| existing.key == entity.key) {
            anyhow::bail!("duplicate entity id: {}", entity.key);
        }
        entities.push(entity.clone());
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

    pub fn move_entity_local(&self, key: &str, position: EntityPosition) -> Result<ManagedEntity> {
        let mut entities = self.entities.lock().expect("entity manager poisoned");
        let entity = entities
            .iter_mut()
            .find(|entity| entity.key == key)
            .with_context(|| format!("entity not found: {key}"))?;
        entity.position = position;
        Ok(entity.clone())
    }

    pub fn remove(&self, players: &crate::players::PlayerManager, key: &str) -> Result<()> {
        let entity = self.remove_local(key)?;
        let packets = entity.remove_packets()?;
        players.broadcast_packets(packets);
        Ok(())
    }

    pub fn remove_local(&self, key: &str) -> Result<ManagedEntity> {
        let mut entities = self.entities.lock().expect("entity manager poisoned");
        let index = entities
            .iter()
            .position(|entity| entity.key == key)
            .with_context(|| format!("entity not found: {key}"))?;
        Ok(entities.remove(index))
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
            look_at_players: config.look_at_players,
            main_hand_event: normalized_npc_event(&config.main_hand_event, "interact"),
            off_hand_event: normalized_npc_event(&config.off_hand_event, "interact_off_hand"),
            attack_event: normalized_npc_event(&config.attack_event, "attack"),
        };

        let mut entities = self.entities.lock().expect("entity manager poisoned");
        if entities.iter().any(|existing| existing.key == entity.key) {
            anyhow::bail!("duplicate configured entity id: {}", entity.key);
        }
        entities.push(entity.clone());
        Ok(entity)
    }

    fn visible_entities_for_view(
        &self,
        dimension: &str,
        viewer_position: EntityPosition,
        rendering: &qexed_config::app::qexed::server::EntityRendering,
    ) -> Vec<ManagedEntity> {
        let entities = self
            .entities
            .lock()
            .expect("entity manager poisoned")
            .iter()
            .filter(|entity| entity.dimension == dimension)
            .filter(|entity| {
                within_render_distance(
                    entity.position,
                    viewer_position,
                    render_distance_for_entity(entity, rendering),
                )
            })
            .cloned()
            .collect::<Vec<_>>();

        simplify_stacked_entities(entities, rendering)
    }

    fn managed_entity_view_packets(
        &self,
        dimension: &str,
        viewer_position: EntityPosition,
        rendering: &qexed_config::app::qexed::server::EntityRendering,
    ) -> Result<Vec<Bytes>> {
        let all_entities = self
            .entities
            .lock()
            .expect("entity manager poisoned")
            .iter()
            .filter(|entity| entity.dimension == dimension)
            .cloned()
            .collect::<Vec<_>>();
        let mut packets = Vec::new();
        for entity in &all_entities {
            packets.extend(entity.remove_packets()?);
        }
        for entity in simplify_stacked_entities(
            all_entities
                .into_iter()
                .filter(|entity| {
                    within_render_distance(
                        entity.position,
                        viewer_position,
                        render_distance_for_entity(entity, rendering),
                    )
                })
                .collect(),
            rendering,
        ) {
            packets.extend(entity.spawn_packets()?);
        }
        Ok(packets)
    }

    fn create_dropped_item(
        &self,
        dimension: &str,
        position: EntityPosition,
        item: qexed_protocol::types::Slot,
    ) -> DroppedItemEntity {
        DroppedItemEntity {
            entity_id: self.entity_ids.next(),
            uuid: uuid::Uuid::new_v4(),
            dimension: dimension.to_string(),
            position,
            item,
            pickup_ready_at: Instant::now() + ITEM_PICKUP_DELAY,
        }
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

fn render_distance_for_entity(
    entity: &ManagedEntity,
    rendering: &qexed_config::app::qexed::server::EntityRendering,
) -> f64 {
    match entity.kind {
        ManagedEntityKind::Entity => rendering.default_distance,
        ManagedEntityKind::Npc => rendering.npc_distance,
        ManagedEntityKind::Hologram => rendering.hologram_distance,
    }
}

fn within_render_distance(entity: EntityPosition, viewer: EntityPosition, distance: f64) -> bool {
    if distance <= 0.0 {
        return false;
    }
    let dx = entity.x - viewer.x;
    let dz = entity.z - viewer.z;
    (dx * dx + dz * dz) <= distance * distance
}

fn horizontal_distance_sq(left: EntityPosition, right: EntityPosition) -> f64 {
    let dx = left.x - right.x;
    let dz = left.z - right.z;
    dx * dx + dz * dz
}

fn look_rotation(from: EntityPosition, to: EntityPosition) -> (f32, f32) {
    let dx = to.x - from.x;
    let dy = (to.y + 1.62) - (from.y + 1.62);
    let dz = to.z - from.z;
    let horizontal = (dx * dx + dz * dz).sqrt().max(0.0001);
    let yaw = (dz.atan2(dx).to_degrees() - 90.0) as f32;
    let pitch = (-dy.atan2(horizontal).to_degrees()) as f32;
    (yaw, pitch)
}

fn normalized_npc_event(value: &str, fallback: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        fallback.to_string()
    } else {
        value.to_string()
    }
}

fn simplify_stacked_entities(
    entities: Vec<ManagedEntity>,
    rendering: &qexed_config::app::qexed::server::EntityRendering,
) -> Vec<ManagedEntity> {
    if rendering.stack_threshold <= 1 || rendering.stack_radius <= 0.0 {
        return entities;
    }

    let mut by_type: HashMap<String, Vec<usize>> = HashMap::new();
    for (index, entity) in entities.iter().enumerate() {
        if entity.kind == ManagedEntityKind::Entity {
            by_type
                .entry(entity.entity_type.clone())
                .or_default()
                .push(index);
        }
    }

    let mut hidden = HashSet::new();
    let mut stacked_names = HashMap::<usize, String>::new();
    let stack_radius_sq = rendering.stack_radius * rendering.stack_radius;

    for indexes in by_type.values() {
        for &base_index in indexes {
            if hidden.contains(&base_index) {
                continue;
            }
            let base = &entities[base_index];
            let mut group = vec![base_index];
            for &candidate_index in indexes {
                if candidate_index == base_index || hidden.contains(&candidate_index) {
                    continue;
                }
                let candidate = &entities[candidate_index];
                let dx = base.position.x - candidate.position.x;
                let dy = base.position.y - candidate.position.y;
                let dz = base.position.z - candidate.position.z;
                if dx * dx + dy * dy + dz * dz <= stack_radius_sq {
                    group.push(candidate_index);
                }
            }

            if group.len() < rendering.stack_threshold {
                continue;
            }
            let label = format!(
                "{}*{}",
                base.entity_type
                    .rsplit(':')
                    .next()
                    .unwrap_or(base.entity_type.as_str()),
                group.len()
            );
            stacked_names.insert(base_index, label);
            for index in group.into_iter().skip(1) {
                hidden.insert(index);
            }
        }
    }

    entities
        .into_iter()
        .enumerate()
        .filter_map(|(index, mut entity)| {
            if hidden.contains(&index) {
                return None;
            }
            if let Some(name) = stacked_names.remove(&index) {
                entity.display_name = name;
            }
            Some(entity)
        })
        .collect()
}

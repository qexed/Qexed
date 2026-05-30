use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use bytes::Bytes;
use qexed_packet::net_types::Position as BlockPosition;
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
const ENTITY_PHYSICS_WIDTH: f64 = 0.6;
const ENTITY_PHYSICS_HEIGHT: f64 = 1.95;
const ENTITY_GRAVITY_PER_TICK: f64 = 0.08;
const ENTITY_TERMINAL_VELOCITY: f64 = -3.92;
const ENTITY_GROUND_SNAP: f64 = 0.05;

#[derive(Debug)]
pub struct EntityManager {
    entity_ids: Arc<EntityIdAllocator>,
    entities: Mutex<Vec<ManagedEntity>>,
    dropped_items: Mutex<Vec<DroppedItemEntity>>,
    custom_entities: Mutex<HashMap<String, CustomEntityRegistration>>,
    spawn_sequence: Mutex<u64>,
    last_spawn_tick: Mutex<Option<Instant>>,
    last_ai_tick: Mutex<Option<Instant>>,
    last_rule_spawn_tick: Mutex<HashMap<String, Instant>>,
    entity_motion: Mutex<HashMap<String, EntityMotion>>,
}

#[derive(Debug, Clone)]
struct CustomEntityRegistration {
    entity_type: String,
    display_name: String,
    ai: String,
}

#[derive(Debug, Clone, Copy, Default)]
struct EntityMotion {
    velocity_y: f64,
}

#[derive(Debug, Clone, Copy, Default)]
struct EntityMovement {
    x: f64,
    y: f64,
    z: f64,
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
            custom_entities: Mutex::new(HashMap::new()),
            spawn_sequence: Mutex::new(0),
            last_spawn_tick: Mutex::new(None),
            last_ai_tick: Mutex::new(None),
            last_rule_spawn_tick: Mutex::new(HashMap::new()),
            entity_motion: Mutex::new(HashMap::new()),
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
            ai: request.ai,
            spawn_rule: request.spawn_rule,
            custom_type: request.custom_type,
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
        self.entity_motion
            .lock()
            .expect("entity motion state poisoned")
            .entry(entity.key.clone())
            .or_default();
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
        let entity = entities.remove(index);
        self.entity_motion
            .lock()
            .expect("entity motion state poisoned")
            .remove(&entity.key);
        Ok(entity)
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
            ai: config.ai.clone(),
            spawn_rule: String::new(),
            custom_type: String::new(),
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

    pub fn register_custom_entities(
        &self,
        definitions: impl IntoIterator<Item = crate::plugins::CustomEntityDefinition>,
    ) {
        let mut custom_entities = self
            .custom_entities
            .lock()
            .expect("entity custom registry poisoned");
        for definition in definitions {
            let id = definition.id.trim();
            let entity_type = definition.entity_type.trim();
            if id.is_empty() || entity_type.is_empty() {
                continue;
            }
            custom_entities.insert(
                id.to_string(),
                CustomEntityRegistration {
                    entity_type: entity_type.to_string(),
                    display_name: definition.display_name,
                    ai: definition.ai,
                },
            );
        }
    }

    pub fn spawn_from_rules(
        &self,
        players: &crate::players::PlayerManager,
        world: &crate::world::WorldManager,
        rendering: &qexed_config::app::qexed::server::EntityRendering,
        spawning: &qexed_config::app::qexed::server::EntitySpawning,
        default_dimension: &str,
    ) -> Result<usize> {
        if !self.should_run_spawn_tick(spawning.tick_interval_ms) {
            return Ok(0);
        }
        if !spawning.enable || spawning.max_spawn_per_tick == 0 || spawning.rules.is_empty() {
            return Ok(0);
        }

        let viewers = players.list_except(uuid::Uuid::nil());
        if viewers.is_empty() {
            return Ok(0);
        }

        let mut spawned = 0usize;
        for _ in 0..spawning.max_spawn_per_tick {
            if self.dynamic_count(None, None, None) >= spawning.global_cap {
                break;
            }
            let Some(rule) = pick_spawn_rule(spawning) else {
                break;
            };
            let rule_id = spawn_rule_id(rule);
            let dimension = spawn_rule_dimension(rule, default_dimension);
            if self.dynamic_count(Some(dimension), None, None) >= spawning.per_dimension_cap {
                continue;
            }
            let custom = self.custom_registration(&rule.entity_type);
            let counted_entity_type = custom
                .as_ref()
                .map(|registration| registration.entity_type.as_str())
                .unwrap_or(rule.entity_type.as_str());
            if self.dynamic_count(Some(dimension), Some(counted_entity_type), None)
                >= spawning.per_type_cap
            {
                continue;
            }
            if self.dynamic_count(Some(dimension), None, Some(&rule_id)) >= rule.cap {
                continue;
            }
            let activation_range =
                spawn_rule_activation_range(rule, spawning.player_activation_range);
            let active_players =
                spawn_rule_active_player_count(rule, dimension, &viewers, activation_range);
            if !spawn_rule_player_count_passes(rule, active_players)
                || !spawn_rule_chance_passes(rule)
                || !self.spawn_rule_interval_passes(&rule_id, rule)
            {
                continue;
            }

            let Some(position) = spawn_position_for_rule(rule, dimension, world) else {
                continue;
            };

            let entity = self.spawn_local(spawn_request_from_rule(
                rule,
                &rule_id,
                dimension,
                self.next_spawn_sequence(),
                custom,
                position,
            )?)?;
            self.mark_rule_spawned(&rule_id);
            self.send_spawn_to_rendered_viewers(players, rendering, &entity)?;
            spawned += 1;
        }

        Ok(spawned)
    }

    pub fn tick_ai(
        &self,
        players: &crate::players::PlayerManager,
        world: &crate::world::WorldManager,
        plugins: &crate::plugins::PluginManager,
        rendering: &qexed_config::app::qexed::server::EntityRendering,
        tick_ms: u64,
    ) -> Result<()> {
        if !self.should_run_ai_tick(tick_ms) {
            return Ok(());
        }
        let viewers = players.list_except(uuid::Uuid::nil());
        if viewers.is_empty() {
            return Ok(());
        }

        let mut updates = Vec::new();
        let mut removes = Vec::new();
        {
            let mut entities = self.entities.lock().expect("entity manager poisoned");
            for entity in entities.iter_mut() {
                if entity.kind != ManagedEntityKind::Entity {
                    continue;
                }

                let previous = entity.position;
                let mut movement = EntityMovement::default();
                let mut remove = false;
                let ai = ai_kind(&entity.ai);
                match ai {
                    EntityAiKind::None => {}
                    EntityAiKind::RandomStroll => {
                        movement = apply_random_stroll(entity, tick_ms);
                    }
                    EntityAiKind::LookAtPlayer => {
                        apply_look_at_nearest_player(entity, &viewers, rendering.default_distance);
                    }
                    EntityAiKind::FollowNearestPlayer => {
                        movement = apply_follow_nearest_player(entity, &viewers, tick_ms);
                    }
                    EntityAiKind::Plugin => {
                        for operation in plugins
                            .handle_entity_ai_tick(entity_ai_query(entity, &viewers, tick_ms))
                        {
                            match operation {
                                crate::plugins::EntityAiOperation::MoveDelta {
                                    x,
                                    y,
                                    z,
                                    yaw,
                                    pitch,
                                } => {
                                    movement.x += finite_or_zero(x).clamp(-4.0, 4.0);
                                    movement.y += finite_or_zero(y).clamp(-4.0, 4.0);
                                    movement.z += finite_or_zero(z).clamp(-4.0, 4.0);
                                    if let Some(yaw) = yaw.filter(|value| value.is_finite()) {
                                        entity.position.yaw = yaw;
                                    }
                                    if let Some(pitch) = pitch.filter(|value| value.is_finite()) {
                                        entity.position.pitch = pitch;
                                    }
                                }
                                crate::plugins::EntityAiOperation::LookAt { x, y, z } => {
                                    if x.is_finite() && y.is_finite() && z.is_finite() {
                                        let target = EntityPosition {
                                            x,
                                            y,
                                            z,
                                            yaw: 0.0,
                                            pitch: 0.0,
                                            on_ground: true,
                                        };
                                        let (yaw, pitch) = look_rotation(entity.position, target);
                                        entity.position.yaw = yaw;
                                        entity.position.pitch = pitch;
                                    }
                                }
                                crate::plugins::EntityAiOperation::Remove => {
                                    remove = true;
                                }
                            }
                        }
                    }
                }

                if !remove && should_apply_entity_physics(entity, ai) {
                    let mut motion = self
                        .entity_motion
                        .lock()
                        .expect("entity motion state poisoned");
                    let motion = motion.entry(entity.key.clone()).or_default();
                    apply_entity_physics(entity, world, motion, movement, tick_ms);
                }

                if remove {
                    removes.push(entity.key.clone());
                } else if position_changed(previous, entity.position) {
                    updates.push(entity.clone());
                }
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

        for key in removes {
            if let Ok(entity) = self.remove_local(&key) {
                self.send_remove_to_rendered_viewers(players, rendering, &entity)?;
            }
        }

        Ok(())
    }

    fn custom_registration(&self, id: &str) -> Option<CustomEntityRegistration> {
        self.custom_entities
            .lock()
            .expect("entity custom registry poisoned")
            .get(id.trim())
            .cloned()
    }

    fn dynamic_count(
        &self,
        dimension: Option<&str>,
        entity_type: Option<&str>,
        spawn_rule: Option<&str>,
    ) -> usize {
        self.entities
            .lock()
            .expect("entity manager poisoned")
            .iter()
            .filter(|entity| !entity.spawn_rule.is_empty())
            .filter(|entity| dimension.is_none_or(|dimension| entity.dimension == dimension))
            .filter(|entity| {
                entity_type.is_none_or(|entity_type| entity.entity_type == entity_type)
            })
            .filter(|entity| spawn_rule.is_none_or(|spawn_rule| entity.spawn_rule == spawn_rule))
            .count()
    }

    fn next_spawn_sequence(&self) -> u64 {
        let mut sequence = self.spawn_sequence.lock().expect("spawn sequence poisoned");
        *sequence = sequence.saturating_add(1);
        *sequence
    }

    fn should_run_spawn_tick(&self, interval_ms: u64) -> bool {
        should_run_tick(&self.last_spawn_tick, interval_ms)
    }

    fn should_run_ai_tick(&self, interval_ms: u64) -> bool {
        should_run_tick(&self.last_ai_tick, interval_ms)
    }

    fn spawn_rule_interval_passes(
        &self,
        rule_id: &str,
        rule: &qexed_config::app::qexed::server::EntitySpawnRule,
    ) -> bool {
        if rule.tick_interval_ms == 0 {
            return true;
        }
        let interval_ms = rule.tick_interval_ms;
        let interval = Duration::from_millis(interval_ms.max(50));
        let last_spawns = self
            .last_rule_spawn_tick
            .lock()
            .expect("entity rule spawn state poisoned");
        last_spawns
            .get(rule_id)
            .is_none_or(|last_spawn| Instant::now().duration_since(*last_spawn) >= interval)
    }

    fn mark_rule_spawned(&self, rule_id: &str) {
        self.last_rule_spawn_tick
            .lock()
            .expect("entity rule spawn state poisoned")
            .insert(rule_id.to_string(), Instant::now());
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

fn should_run_tick(last_tick: &Mutex<Option<Instant>>, interval_ms: u64) -> bool {
    let now = Instant::now();
    let interval = Duration::from_millis(interval_ms.max(50));
    let mut last_tick = last_tick.lock().expect("entity tick state poisoned");
    if last_tick.is_some_and(|last_tick| now.duration_since(last_tick) < interval) {
        return false;
    }
    *last_tick = Some(now);
    true
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EntityAiKind {
    None,
    RandomStroll,
    LookAtPlayer,
    FollowNearestPlayer,
    Plugin,
}

fn ai_kind(value: &str) -> EntityAiKind {
    let value = value.trim();
    if value.is_empty() {
        return EntityAiKind::None;
    }
    match value {
        "none" => EntityAiKind::None,
        "random_stroll" | "wander" => EntityAiKind::RandomStroll,
        "look_at_player" | "look_at_players" => EntityAiKind::LookAtPlayer,
        "follow_nearest_player" | "follow_player" => EntityAiKind::FollowNearestPlayer,
        value if value.starts_with("plugin:") => EntityAiKind::Plugin,
        _ => EntityAiKind::None,
    }
}

fn should_apply_entity_physics(entity: &ManagedEntity, ai: EntityAiKind) -> bool {
    ai != EntityAiKind::None || !entity.spawn_rule.is_empty()
}

fn apply_random_stroll(entity: &mut ManagedEntity, tick_ms: u64) -> EntityMovement {
    let mut rng = rand::thread_rng();
    if rand::Rng::gen_range(&mut rng, 0.0..1.0) > 0.35 {
        return EntityMovement::default();
    }
    let yaw = rand::Rng::gen_range(&mut rng, -180.0..180.0);
    let radians = f64::from(yaw).to_radians();
    let tick_scale = (tick_ms as f64 / 200.0).clamp(0.25, 2.0);
    let step = rand::Rng::gen_range(&mut rng, 0.08..0.22) * tick_scale;
    entity.position.yaw = yaw;
    EntityMovement {
        x: -radians.sin() * step,
        y: 0.0,
        z: radians.cos() * step,
    }
}

fn apply_look_at_nearest_player(
    entity: &mut ManagedEntity,
    viewers: &[crate::players::OnlinePlayer],
    range: f64,
) {
    let Some(target) = nearest_player(entity.position, &entity.dimension, viewers, range) else {
        return;
    };
    let (yaw, pitch) = look_rotation(entity.position, target.position);
    entity.position.yaw = yaw;
    entity.position.pitch = pitch;
}

fn apply_follow_nearest_player(
    entity: &mut ManagedEntity,
    viewers: &[crate::players::OnlinePlayer],
    tick_ms: u64,
) -> EntityMovement {
    const FOLLOW_RANGE: f64 = 32.0;
    const STOP_DISTANCE: f64 = 2.0;
    const BASE_STEP: f64 = 0.22;

    let Some(target) = nearest_player(entity.position, &entity.dimension, viewers, FOLLOW_RANGE)
    else {
        return EntityMovement::default();
    };
    let dx = target.position.x - entity.position.x;
    let dz = target.position.z - entity.position.z;
    let horizontal = (dx * dx + dz * dz).sqrt();
    let (yaw, pitch) = look_rotation(entity.position, target.position);
    entity.position.yaw = yaw;
    entity.position.pitch = pitch;
    if horizontal <= STOP_DISTANCE {
        return EntityMovement::default();
    }
    let tick_scale = (tick_ms as f64 / 200.0).clamp(0.25, 2.0);
    let step = (BASE_STEP * tick_scale).min(horizontal - STOP_DISTANCE);
    EntityMovement {
        x: dx / horizontal * step,
        y: 0.0,
        z: dz / horizontal * step,
    }
}

fn nearest_player<'a>(
    position: EntityPosition,
    dimension: &str,
    viewers: &'a [crate::players::OnlinePlayer],
    range: f64,
) -> Option<&'a crate::players::OnlinePlayer> {
    viewers
        .iter()
        .filter(|player| player.dimension == dimension)
        .filter(|player| within_render_distance(position, player.position, range))
        .min_by(|left, right| {
            horizontal_distance_sq(position, left.position)
                .total_cmp(&horizontal_distance_sq(position, right.position))
        })
}

fn apply_entity_physics(
    entity: &mut ManagedEntity,
    world: &crate::world::WorldManager,
    motion: &mut EntityMotion,
    movement: EntityMovement,
    tick_ms: u64,
) {
    let tick_scale = (tick_ms as f64 / 50.0).clamp(0.25, 4.0);
    let dx = movement.x.clamp(-4.0, 4.0);
    let dz = movement.z.clamp(-4.0, 4.0);
    let mut dy = movement.y.clamp(-4.0, 4.0);

    if entity_has_ground(world, &entity.dimension, entity.position) && motion.velocity_y <= 0.0 {
        entity.position.on_ground = true;
        motion.velocity_y = 0.0;
    } else {
        entity.position.on_ground = false;
        motion.velocity_y = (motion.velocity_y - ENTITY_GRAVITY_PER_TICK * tick_scale)
            .max(ENTITY_TERMINAL_VELOCITY);
        dy += motion.velocity_y * tick_scale;
    }

    if dy != 0.0 {
        move_entity_axis(entity, world, motion, 0.0, dy, 0.0);
    }
    if dx != 0.0 {
        move_entity_axis(entity, world, motion, dx, 0.0, 0.0);
    }
    if dz != 0.0 {
        move_entity_axis(entity, world, motion, 0.0, 0.0, dz);
    }

    if entity_has_ground(world, &entity.dimension, entity.position) && motion.velocity_y <= 0.0 {
        entity.position.on_ground = true;
        motion.velocity_y = 0.0;
    } else {
        entity.position.on_ground = false;
    }
}

fn move_entity_axis(
    entity: &mut ManagedEntity,
    world: &crate::world::WorldManager,
    motion: &mut EntityMotion,
    dx: f64,
    dy: f64,
    dz: f64,
) {
    let next_x = entity.position.x + dx;
    let next_y = entity.position.y + dy;
    let next_z = entity.position.z + dz;
    if entity_aabb_intersects_solid(
        world,
        &entity.dimension,
        next_x,
        next_y,
        next_z,
        ENTITY_PHYSICS_WIDTH,
        ENTITY_PHYSICS_HEIGHT,
    ) {
        if dy < 0.0 {
            entity.position.y = next_y.floor() + 1.0;
            entity.position.on_ground = true;
            motion.velocity_y = 0.0;
        } else if dy > 0.0 {
            motion.velocity_y = 0.0;
        }
        return;
    }

    entity.position.x = next_x;
    entity.position.y = next_y;
    entity.position.z = next_z;
}

fn entity_has_ground(
    world: &crate::world::WorldManager,
    dimension: &str,
    position: EntityPosition,
) -> bool {
    let below_y = (position.y - ENTITY_GROUND_SNAP).floor() as i32;
    let min_x = (position.x - ENTITY_PHYSICS_WIDTH / 2.0 + 0.001).floor() as i32;
    let max_x = (position.x + ENTITY_PHYSICS_WIDTH / 2.0 - 0.001).floor() as i32;
    let min_z = (position.z - ENTITY_PHYSICS_WIDTH / 2.0 + 0.001).floor() as i32;
    let max_z = (position.z + ENTITY_PHYSICS_WIDTH / 2.0 - 0.001).floor() as i32;
    for x in min_x..=max_x {
        for z in min_z..=max_z {
            if block_has_collision_at(world, dimension, x, below_y, z) {
                return true;
            }
        }
    }
    false
}

fn entity_aabb_intersects_solid(
    world: &crate::world::WorldManager,
    dimension: &str,
    x: f64,
    y: f64,
    z: f64,
    width: f64,
    height: f64,
) -> bool {
    let half_width = width / 2.0;
    let min_x = (x - half_width + 0.001).floor() as i32;
    let max_x = (x + half_width - 0.001).floor() as i32;
    let min_y = (y + 0.001).floor() as i32;
    let max_y = (y + height - 0.001).floor() as i32;
    let min_z = (z - half_width + 0.001).floor() as i32;
    let max_z = (z + half_width - 0.001).floor() as i32;

    for block_x in min_x..=max_x {
        for block_y in min_y..=max_y {
            for block_z in min_z..=max_z {
                if block_has_collision_at(world, dimension, block_x, block_y, block_z) {
                    return true;
                }
            }
        }
    }
    false
}

fn block_has_collision_at(
    world: &crate::world::WorldManager,
    dimension: &str,
    x: i32,
    y: i32,
    z: i32,
) -> bool {
    let position = BlockPosition { x, y, z };
    world
        .block_state_at(dimension, &position)
        .map(crate::inventory::block_has_collision)
        .unwrap_or(false)
}

fn entity_ai_query(
    entity: &ManagedEntity,
    viewers: &[crate::players::OnlinePlayer],
    tick_ms: u64,
) -> crate::plugins::EntityAiTickQuery {
    let nearby_players = viewers
        .iter()
        .filter(|player| player.dimension == entity.dimension)
        .filter(|player| within_render_distance(entity.position, player.position, 64.0))
        .map(|player| crate::plugins::EntityAiPlayerPayload {
            player: qexed_plugin_api::player_payload_owned(player),
            position: qexed_plugin_api::player_position_payload(player.position),
        })
        .collect();
    crate::plugins::EntityAiTickQuery {
        entity: crate::plugins::EntityAiEntityPayload {
            key: entity.key.clone(),
            entity_id: entity.entity_id,
            entity_type: entity.entity_type.clone(),
            custom_type: entity.custom_type.clone(),
            ai: entity.ai.clone(),
            spawn_rule: entity.spawn_rule.clone(),
            dimension: entity.dimension.clone(),
            position: qexed_plugin_api::player_position_payload(entity.position),
        },
        nearby_players,
        tick_ms,
    }
}

fn finite_or_zero(value: f64) -> f64 {
    if value.is_finite() { value } else { 0.0 }
}

fn position_changed(left: EntityPosition, right: EntityPosition) -> bool {
    (left.x - right.x).abs() > 0.0001
        || (left.y - right.y).abs() > 0.0001
        || (left.z - right.z).abs() > 0.0001
        || (left.yaw - right.yaw).abs() > 0.1
        || (left.pitch - right.pitch).abs() > 0.1
        || left.on_ground != right.on_ground
}

fn pick_spawn_rule(
    spawning: &qexed_config::app::qexed::server::EntitySpawning,
) -> Option<&qexed_config::app::qexed::server::EntitySpawnRule> {
    let candidates = spawning
        .rules
        .iter()
        .filter(|rule| rule.enable && rule.weight > 0 && rule.cap > 0)
        .collect::<Vec<_>>();
    let total = candidates.iter().fold(0u64, |total, rule| {
        total.saturating_add(u64::from(rule.weight))
    });
    if total == 0 {
        return None;
    }

    let mut pick = rand::Rng::gen_range(&mut rand::thread_rng(), 0..total);
    for rule in candidates {
        let weight = u64::from(rule.weight);
        if pick < weight {
            return Some(rule);
        }
        pick -= weight;
    }
    None
}

fn spawn_rule_id(rule: &qexed_config::app::qexed::server::EntitySpawnRule) -> String {
    let id = rule.id.trim();
    if !id.is_empty() {
        id.to_string()
    } else {
        rule.entity_type
            .trim()
            .trim_start_matches("minecraft:")
            .replace([':', '/', '\\', ' '], "_")
    }
}

fn spawn_rule_dimension<'a>(
    rule: &'a qexed_config::app::qexed::server::EntitySpawnRule,
    default_dimension: &'a str,
) -> &'a str {
    let dimension = rule.dimension.trim();
    if dimension.is_empty() {
        default_dimension
    } else {
        dimension
    }
}

fn spawn_rule_activation_range(
    rule: &qexed_config::app::qexed::server::EntitySpawnRule,
    global_activation_range: f64,
) -> f64 {
    if rule.activation_range > 0.0 {
        rule.activation_range
    } else {
        global_activation_range
    }
    .max(0.0)
}

fn spawn_rule_active_player_count(
    rule: &qexed_config::app::qexed::server::EntitySpawnRule,
    dimension: &str,
    viewers: &[crate::players::OnlinePlayer],
    activation_range: f64,
) -> usize {
    let min_x = rule.min_x.min(rule.max_x) - activation_range;
    let max_x = rule.min_x.max(rule.max_x) + activation_range;
    let min_z = rule.min_z.min(rule.max_z) - activation_range;
    let max_z = rule.min_z.max(rule.max_z) + activation_range;
    viewers
        .iter()
        .filter(|player| {
            player.dimension == dimension
                && player.position.x >= min_x
                && player.position.x <= max_x
                && player.position.z >= min_z
                && player.position.z <= max_z
        })
        .count()
}

fn spawn_rule_player_count_passes(
    rule: &qexed_config::app::qexed::server::EntitySpawnRule,
    active_players: usize,
) -> bool {
    active_players >= rule.min_players
        && (rule.max_players == 0 || active_players <= rule.max_players)
}

fn spawn_rule_chance_passes(rule: &qexed_config::app::qexed::server::EntitySpawnRule) -> bool {
    let chance = rule.spawn_chance.clamp(0.0, 1.0);
    chance >= 1.0 || rand::Rng::gen_range(&mut rand::thread_rng(), 0.0..1.0) < chance
}

fn spawn_position_for_rule(
    rule: &qexed_config::app::qexed::server::EntitySpawnRule,
    dimension: &str,
    world: &crate::world::WorldManager,
) -> Option<EntityPosition> {
    let attempts = if rule.require_air || rule.require_ground {
        rule.position_attempts.max(1).min(64)
    } else {
        1
    };
    for _ in 0..attempts {
        let position = EntityPosition {
            x: random_between(rule.min_x, rule.max_x),
            y: random_spawn_y(rule),
            z: random_between(rule.min_z, rule.max_z),
            yaw: rand::Rng::gen_range(&mut rand::thread_rng(), -180.0..180.0),
            pitch: 0.0,
            on_ground: rule.on_ground,
        };
        if spawn_position_passes(rule, dimension, world, position) {
            return Some(position);
        }
    }
    None
}

fn spawn_position_passes(
    rule: &qexed_config::app::qexed::server::EntitySpawnRule,
    dimension: &str,
    world: &crate::world::WorldManager,
    position: EntityPosition,
) -> bool {
    if rule.require_ground && !entity_has_ground(world, dimension, position) {
        return false;
    }
    if rule.require_air
        && entity_aabb_intersects_solid(
            world,
            dimension,
            position.x,
            position.y,
            position.z,
            ENTITY_PHYSICS_WIDTH,
            ENTITY_PHYSICS_HEIGHT,
        )
    {
        return false;
    }
    true
}

fn spawn_request_from_rule(
    rule: &qexed_config::app::qexed::server::EntitySpawnRule,
    rule_id: &str,
    dimension: &str,
    sequence: u64,
    custom: Option<CustomEntityRegistration>,
    position: EntityPosition,
) -> Result<EntitySpawnRequest> {
    let (entity_type, custom_type, default_display_name, default_ai) = match custom {
        Some(custom) => (
            custom.entity_type,
            rule.entity_type.trim().to_string(),
            custom.display_name,
            custom.ai,
        ),
        None => (
            rule.entity_type.trim().to_string(),
            String::new(),
            String::new(),
            String::new(),
        ),
    };
    let entity_type = if entity_type.trim().is_empty() {
        "minecraft:zombie".to_string()
    } else {
        entity_type
    };
    let name = if rule.name.trim().is_empty() {
        rule_id.to_string()
    } else {
        rule.name.clone()
    };
    let display_name = if !rule.display_name.trim().is_empty() {
        rule.display_name.clone()
    } else if !default_display_name.trim().is_empty() {
        default_display_name
    } else {
        name.clone()
    };
    let ai = if !rule.ai.trim().is_empty() {
        rule.ai.clone()
    } else {
        default_ai
    };
    Ok(EntitySpawnRequest {
        key: format!("spawn:{rule_id}:{sequence}"),
        kind: ManagedEntityKind::Entity,
        entity_type,
        dimension: dimension.to_string(),
        position,
        name,
        display_name,
        skin_textures: String::new(),
        skin_signature: String::new(),
        data: rule.data,
        ai,
        spawn_rule: rule_id.to_string(),
        custom_type,
        look_at_players: false,
        main_hand_event: "interact".to_string(),
        off_hand_event: "interact_off_hand".to_string(),
        attack_event: "attack".to_string(),
    })
}

fn random_between(left: f64, right: f64) -> f64 {
    let min = left.min(right);
    let max = left.max(right);
    if (max - min).abs() < f64::EPSILON {
        min
    } else {
        rand::Rng::gen_range(&mut rand::thread_rng(), min..=max)
    }
}

fn random_spawn_y(rule: &qexed_config::app::qexed::server::EntitySpawnRule) -> f64 {
    if !rule.require_ground && !rule.require_air {
        return random_between(rule.min_y, rule.max_y);
    }

    let min = rule.min_y.min(rule.max_y).ceil() as i32;
    let max = rule.min_y.max(rule.max_y).floor() as i32;
    if min > max {
        return random_between(rule.min_y, rule.max_y);
    }
    f64::from(rand::Rng::gen_range(&mut rand::thread_rng(), min..=max))
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

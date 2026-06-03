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
const DEFAULT_ITEM_MERGE_RADIUS: f64 = 2.0;
const DEFAULT_ITEM_MERGE_MAX_STACK: i32 = 64;
const ENTITY_PHYSICS_WIDTH: f64 = 0.6;
const ENTITY_PHYSICS_HEIGHT: f64 = 1.95;
const ENTITY_GRAVITY_PER_TICK: f64 = 0.08;
const ENTITY_TERMINAL_VELOCITY: f64 = -3.92;
const ENTITY_GROUND_SNAP: f64 = 0.05;
const ENTITY_MAX_STEP_HEIGHT: f64 = 0.6;
const ENTITY_AUTO_JUMP_VELOCITY: f64 = 0.42;
const ENTITY_HORIZONTAL_ACCELERATION: f64 = 0.12;
const ENTITY_HORIZONTAL_FRICTION: f64 = 0.72;
const ENTITY_MAX_HORIZONTAL_SPEED: f64 = 0.28;
const ENTITY_MAX_YAW_TURN_PER_TICK: f32 = 18.0;
const ENTITY_MAX_PITCH_TURN_PER_TICK: f32 = 12.0;
const COLLISION_BLOCK_CACHE_LIMIT: usize = 262_144;
const COLLISION_AABB_CACHE_LIMIT: usize = 65_536;
const ENTITY_TARGET_RESELECT_INTERVAL: Duration = Duration::from_millis(750);
const ENTITY_TARGET_RESELECT_JITTER_MS: u64 = 350;
const ENTITY_TARGET_SWITCH_ADVANTAGE: f64 = 0.65;
const ENTITY_TARGET_RESELECTS_PER_TICK: usize = 4;
const ENTITY_PATH_RECALC_INTERVAL: Duration = Duration::from_millis(1_500);
const ENTITY_PATH_RECALCS_PER_TICK: usize = 2;
const ENTITY_PATH_MAX_NODES: usize = 512;
const ENTITY_PATH_WAYPOINT_REACHED: f64 = 0.65;
const ENTITY_PATH_TARGET_REPLAN_DISTANCE_SQ: i32 = 16;
const ENTITY_DIRECT_FOLLOW_DISTANCE: f64 = 10.0;
const ENTITY_DIRECT_FOLLOW_SAMPLES_PER_BLOCK: f64 = 2.0;
const ENTITY_DEATH_REMOVE_DELAY: Duration = Duration::from_millis(1_000);
const ENTITY_AI_MAX_ENTITIES_PER_TICK: usize = 256;
const ENTITY_PLUGIN_AI_DEFAULT_INTERVAL: Duration = Duration::from_millis(500);
const ENTITY_PLUGIN_AI_CALLS_PER_TICK: usize = 16;
const ENTITY_PLUGIN_AI_NEARBY_RANGE: f64 = 64.0;

#[derive(Debug)]
pub struct EntityManager {
    entity_ids: Arc<EntityIdAllocator>,
    entities: Mutex<Vec<ManagedEntity>>,
    dropped_items: Mutex<Vec<DroppedItemEntity>>,
    custom_entities: Mutex<HashMap<String, CustomEntityRegistration>>,
    custom_entity_type_ids: Mutex<HashMap<String, i32>>,
    spawn_sequence: Mutex<u64>,
    last_spawn_tick: Mutex<Option<Instant>>,
    last_ai_tick: Mutex<Option<Instant>>,
    last_rule_spawn_tick: Mutex<HashMap<String, Instant>>,
    entity_motion: Mutex<HashMap<String, EntityMotion>>,
    entity_health: Mutex<HashMap<String, f32>>,
    entity_deaths: Mutex<HashMap<String, Instant>>,
    entity_targets: Mutex<HashMap<String, EntityTargetMemory>>,
    entity_paths: Mutex<HashMap<String, EntityPathMemory>>,
    entity_plugin_ai: Mutex<HashMap<String, EntityPluginAiMemory>>,
    collision_cache: Mutex<CollisionCache>,
    ai_cursor: Mutex<usize>,
}

#[derive(Debug, Clone)]
struct CustomEntityRegistration {
    id: String,
    shell_entity_type: String,
    entity_type_id: Option<i32>,
    display_name: String,
    ai: String,
    ai_params: std::collections::BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Copy, Default)]
struct EntityMotion {
    velocity_x: f64,
    velocity_y: f64,
    velocity_z: f64,
}

#[derive(Debug, Clone, Copy)]
struct EntityTargetMemory {
    player_id: uuid::Uuid,
    selected_at: Instant,
}

#[derive(Debug, Clone)]
struct EntityPathMemory {
    target_player_id: uuid::Uuid,
    target_block: BlockPosition,
    calculated_at: Instant,
    waypoints: Vec<BlockPosition>,
    cursor: usize,
}

#[derive(Debug, Clone)]
struct EntityPluginAiMemory {
    last_tick: Instant,
    operations: Vec<crate::plugins::EntityAiOperation>,
}

#[derive(Debug, Clone, Copy, Default)]
struct EntityMovement {
    x: f64,
    y: f64,
    z: f64,
}

#[derive(Debug)]
struct EntityAiTickUpdate {
    key: String,
    previous: EntityPosition,
    next: EntityPosition,
    motion: Option<EntityMotion>,
}

#[derive(Debug, Clone)]
pub struct EntityDamageResult {
    pub entity: ManagedEntity,
    pub killed: bool,
}

#[derive(Debug)]
struct EntityMotionTickUpdate {
    key: String,
    previous: EntityPosition,
    next: EntityMotion,
}

#[derive(Debug, Clone)]
pub enum DroppedItemUpdate {
    Spawned(DroppedItemEntity),
    Merged(DroppedItemEntity),
}

#[derive(Debug, Default)]
struct CollisionCache {
    world_epoch: u64,
    blocks: HashMap<CollisionBlockKey, Option<crate::inventory::BlockCollisionShape>>,
    aabbs: HashMap<CollisionAabbKey, bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct CollisionBlockKey {
    dimension: String,
    x: i32,
    y: i32,
    z: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct CollisionAabbKey {
    dimension: String,
    min_x: i32,
    max_x: i32,
    min_y: i32,
    max_y: i32,
    min_z: i32,
    max_z: i32,
}

#[derive(Debug, Clone, Copy)]
struct EntityAabb {
    min_x: f64,
    max_x: f64,
    min_y: f64,
    max_y: f64,
    min_z: f64,
    max_z: f64,
}

impl EntityAabb {
    fn new(x: f64, y: f64, z: f64, width: f64, height: f64) -> Self {
        let half_width = width / 2.0;
        Self {
            min_x: x - half_width + 0.001,
            max_x: x + half_width - 0.001,
            min_y: y + 0.001,
            max_y: y + height - 0.001,
            min_z: z - half_width + 0.001,
            max_z: z + half_width - 0.001,
        }
    }

    fn ground_probe(position: EntityPosition, width: f64) -> Self {
        let half_width = width / 2.0;
        Self {
            min_x: position.x - half_width + 0.001,
            max_x: position.x + half_width - 0.001,
            min_y: position.y - ENTITY_GROUND_SNAP,
            max_y: position.y + 0.001,
            min_z: position.z - half_width + 0.001,
            max_z: position.z + half_width - 0.001,
        }
    }

    fn block_min_x(self) -> i32 {
        self.min_x.floor() as i32
    }

    fn block_max_x(self) -> i32 {
        self.max_x.floor() as i32
    }

    fn block_min_y(self) -> i32 {
        self.min_y.floor() as i32
    }

    fn block_max_y(self) -> i32 {
        self.max_y.floor() as i32
    }

    fn block_min_z(self) -> i32 {
        self.min_z.floor() as i32
    }

    fn block_max_z(self) -> i32 {
        self.max_z.floor() as i32
    }

    fn cache_key(self, dimension: &str) -> CollisionAabbKey {
        CollisionAabbKey {
            dimension: dimension.to_string(),
            min_x: quantized_aabb_coord(self.min_x),
            max_x: quantized_aabb_coord(self.max_x),
            min_y: quantized_aabb_coord(self.min_y),
            max_y: quantized_aabb_coord(self.max_y),
            min_z: quantized_aabb_coord(self.min_z),
            max_z: quantized_aabb_coord(self.max_z),
        }
    }

    fn intersects_block_shape(
        self,
        block_x: i32,
        block_y: i32,
        block_z: i32,
        shape: crate::inventory::BlockCollisionShape,
    ) -> bool {
        let min_x = f64::from(block_x) + shape.min_x;
        let max_x = f64::from(block_x) + shape.max_x;
        let min_y = f64::from(block_y) + shape.min_y;
        let max_y = f64::from(block_y) + shape.max_y;
        let min_z = f64::from(block_z) + shape.min_z;
        let max_z = f64::from(block_z) + shape.max_z;
        self.max_x > min_x
            && self.min_x < max_x
            && self.max_y > min_y
            && self.min_y < max_y
            && self.max_z > min_z
            && self.min_z < max_z
    }
}

fn quantized_aabb_coord(value: f64) -> i32 {
    (value * 1024.0).round() as i32
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
            custom_entity_type_ids: Mutex::new(HashMap::new()),
            spawn_sequence: Mutex::new(0),
            last_spawn_tick: Mutex::new(None),
            last_ai_tick: Mutex::new(None),
            last_rule_spawn_tick: Mutex::new(HashMap::new()),
            entity_motion: Mutex::new(HashMap::new()),
            entity_health: Mutex::new(HashMap::new()),
            entity_deaths: Mutex::new(HashMap::new()),
            entity_targets: Mutex::new(HashMap::new()),
            entity_paths: Mutex::new(HashMap::new()),
            entity_plugin_ai: Mutex::new(HashMap::new()),
            collision_cache: Mutex::new(CollisionCache::default()),
            ai_cursor: Mutex::new(0),
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
    ) -> Result<Vec<DroppedItemUpdate>> {
        if item.item_count.0 <= 0 {
            return Ok(Vec::new());
        }

        let updates = self.drop_item_local_with_limits(
            dimension,
            position,
            item,
            DEFAULT_ITEM_MERGE_RADIUS,
            DEFAULT_ITEM_MERGE_MAX_STACK,
        )?;
        let packets = dropped_item_update_packets(&updates)?;
        players.broadcast_packets_except(actor, packets);
        Ok(updates)
    }

    pub fn drop_item_with_rendering(
        &self,
        players: &crate::players::PlayerManager,
        actor: uuid::Uuid,
        dimension: &str,
        position: EntityPosition,
        item: qexed_protocol::types::Slot,
        rendering: &qexed_config::app::qexed::server::EntityRendering,
    ) -> Result<Vec<DroppedItemUpdate>> {
        let updates = self.drop_item_local_with_limits(
            dimension,
            position,
            item,
            rendering.item_merge_radius,
            rendering.item_merge_max_stack,
        )?;
        if updates.is_empty() {
            return Ok(Vec::new());
        }
        let packets = dropped_item_update_packets(&updates)?;
        for player in players.list_except(actor) {
            if player.dimension == dimension
                && within_render_distance(player.position, position, rendering.item_distance)
            {
                players.send_packets_to(player.profile.uuid, packets.clone());
            }
        }
        Ok(updates)
    }

    pub fn send_spawn_to_rendered_viewers(
        &self,
        players: &crate::players::PlayerManager,
        rendering: &qexed_config::app::qexed::server::EntityRendering,
        entity: &ManagedEntity,
    ) -> Result<()> {
        let packets = entity.spawn_packets()?;
        for player in players.list_except(uuid::Uuid::nil()) {
            if entity_visible_to_player(entity, &player, rendering) {
                players.send_packets_to(player.profile.uuid, packets.clone());
            }
        }
        Ok(())
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
        let packets = entity.position_packets()?;
        for player in players.list_except(uuid::Uuid::nil()) {
            if entity_visible_to_player(entity, &player, rendering) {
                players.send_packets_to(player.profile.uuid, packets.clone());
            }
        }
        Ok(())
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

                let (target_yaw, target_pitch) = look_rotation(entity.position, target.position);
                let (yaw, pitch) = smooth_rotation(entity.position, target_yaw, target_pitch, 50);
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
            let motion = self
                .entity_motion
                .lock()
                .expect("entity motion state poisoned")
                .get(&entity.key)
                .copied()
                .unwrap_or_default();
            let packets = entity.position_packets_with_velocity(
                motion.velocity_x,
                motion.velocity_y,
                motion.velocity_z,
            )?;
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
            if entity_visible_to_player(entity, &player, rendering) {
                players.send_packets_to(player.profile.uuid, remove_packets.clone());
            }
        }
        Ok(())
    }

    #[cfg(test)]
    pub fn drop_item_local(
        &self,
        dimension: &str,
        position: EntityPosition,
        item: qexed_protocol::types::Slot,
    ) -> Result<Vec<DroppedItemUpdate>> {
        self.drop_item_local_with_limits(
            dimension,
            position,
            item,
            DEFAULT_ITEM_MERGE_RADIUS,
            DEFAULT_ITEM_MERGE_MAX_STACK,
        )
    }

    fn drop_item_local_with_limits(
        &self,
        dimension: &str,
        position: EntityPosition,
        mut item: qexed_protocol::types::Slot,
        merge_radius: f64,
        max_stack: i32,
    ) -> Result<Vec<DroppedItemUpdate>> {
        if item.item_count.0 <= 0 {
            return Ok(Vec::new());
        }

        let mut updates = Vec::new();
        let merge_radius_sq = merge_radius.max(0.0) * merge_radius.max(0.0);
        let max_stack = max_stack.max(1);
        let mut dropped_items = self
            .dropped_items
            .lock()
            .expect("entity manager dropped items poisoned");
        if merge_radius > 0.0 {
            for existing in dropped_items.iter_mut() {
                if item.item_count.0 <= 0 {
                    break;
                }
                if existing.dimension != dimension
                    || !crate::inventory::same_stack_kind(&existing.item, &item)
                    || existing.item.item_count.0 >= max_stack
                    || distance_sq(existing.position, position) > merge_radius_sq
                {
                    continue;
                }
                let moved = (max_stack - existing.item.item_count.0).min(item.item_count.0);
                if moved <= 0 {
                    continue;
                }
                existing.item.item_count.0 += moved;
                item.item_count.0 -= moved;
                updates.push(DroppedItemUpdate::Merged(existing.clone()));
            }
        }

        if item.item_count.0 > 0 {
            while item.item_count.0 > 0 {
                let count = item.item_count.0.min(max_stack);
                let mut stack = item.clone();
                stack.item_count.0 = count;
                item.item_count.0 -= count;
                let entity = self.create_dropped_item(dimension, position, stack);
                dropped_items.push(entity.clone());
                updates.push(DroppedItemUpdate::Spawned(entity));
            }
        }
        Ok(updates)
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

    pub fn settle_collectable_dropped_items<F>(
        &self,
        dimension: &str,
        collector: EntityPosition,
        mut settle: F,
    ) where
        F: FnMut(EntityPosition) -> EntityPosition,
    {
        let now = Instant::now();
        let mut dropped_items = self
            .dropped_items
            .lock()
            .expect("entity manager dropped items poisoned");
        for item in dropped_items.iter_mut() {
            if item.dimension == dimension
                && item.pickup_ready_at <= now
                && (collector.x - item.position.x).abs() <= ITEM_PICKUP_RADIUS_XZ
                && (collector.z - item.position.z).abs() <= ITEM_PICKUP_RADIUS_XZ
            {
                item.position = settle(item.position);
            }
        }
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
            ManagedEntityKind::Npc => normalized_npc_client_entity_type(key, &request.entity_type),
            ManagedEntityKind::Hologram => "minecraft:text_display".to_string(),
        };
        let entity = ManagedEntity {
            entity_id: self.entity_ids.next(),
            uuid: stable_entity_uuid(key),
            key: key.to_string(),
            kind: request.kind,
            entity_type_id: request
                .entity_type_id_override
                .map(Ok)
                .unwrap_or_else(|| entity_type_id(&entity_type))?,
            entity_type,
            dimension: request.dimension,
            position: request.position,
            name: request.name,
            display_name: request.display_name,
            skin_textures: request.skin_textures,
            skin_signature: request.skin_signature,
            data: request.data,
            ai: request.ai,
            ai_params: request.ai_params,
            auto_jump: request.auto_jump,
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
        self.entity_health
            .lock()
            .expect("entity health state poisoned")
            .entry(entity.key.clone())
            .or_insert_with(|| default_entity_health(&entity.entity_type));
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
        self.entity_health
            .lock()
            .expect("entity health state poisoned")
            .remove(&entity.key);
        self.entity_deaths
            .lock()
            .expect("entity death state poisoned")
            .remove(&entity.key);
        self.entity_targets
            .lock()
            .expect("entity target state poisoned")
            .remove(&entity.key);
        self.entity_paths
            .lock()
            .expect("entity path state poisoned")
            .remove(&entity.key);
        self.entity_plugin_ai
            .lock()
            .expect("entity plugin ai state poisoned")
            .remove(&entity.key);
        Ok(entity)
    }

    pub fn damage_managed_entity(
        &self,
        _players: &crate::players::PlayerManager,
        _rendering: &qexed_config::app::qexed::server::EntityRendering,
        entity_id: i32,
        damage: f32,
    ) -> Result<Option<EntityDamageResult>> {
        if damage <= 0.0 {
            return Ok(None);
        }
        let Some(entity) = self.entity_by_runtime_id(entity_id) else {
            return Ok(None);
        };
        if entity.kind != ManagedEntityKind::Entity {
            return Ok(None);
        }
        if self
            .entity_deaths
            .lock()
            .expect("entity death state poisoned")
            .contains_key(&entity.key)
        {
            return Ok(None);
        }

        let mut health = self
            .entity_health
            .lock()
            .expect("entity health state poisoned");
        let current = health
            .entry(entity.key.clone())
            .or_insert_with(|| default_entity_health(&entity.entity_type));
        *current = (*current - damage).max(0.0);
        let killed = *current <= 0.0;
        if killed {
            *current = 0.0;
        }
        drop(health);

        if killed {
            self.entity_deaths
                .lock()
                .expect("entity death state poisoned")
                .insert(
                    entity.key.clone(),
                    Instant::now() + ENTITY_DEATH_REMOVE_DELAY,
                );
        }

        Ok(Some(EntityDamageResult { entity, killed }))
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
                    "minecraft:armor_stand".to_string()
                } else {
                    entity_type.to_string()
                }
            }
            ManagedEntityKind::Npc => normalized_npc_client_entity_type(&key, &config.entity_type),
            ManagedEntityKind::Hologram => "minecraft:text_display".to_string(),
        };
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
            ai_params: config.ai_params.clone(),
            auto_jump: config.auto_jump,
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
            if id.is_empty() {
                continue;
            }
            let shell_entity_type = custom_entity_shell_type(&definition);
            let entity_type_id = self.custom_entity_type_id(id, &shell_entity_type, &definition);
            custom_entities.insert(
                id.to_string(),
                CustomEntityRegistration {
                    id: id.to_string(),
                    shell_entity_type,
                    entity_type_id,
                    display_name: definition.display_name,
                    ai: definition.ai,
                    ai_params: definition.ai_params,
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
            if custom.is_none() && !is_known_minecraft_entity_type(&rule.entity_type) {
                log::warn!(
                    "skip spawn rule with unknown entity type: rule={}, entity_type={}",
                    rule_id,
                    rule.entity_type
                );
                continue;
            }
            let counted_entity_type = custom
                .as_ref()
                .map(|registration| registration.id.as_str())
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

            let Some(position) = self.spawn_position_for_rule(rule, dimension, world) else {
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
        let now = Instant::now();
        for key in self.expired_death_keys(now) {
            if let Ok(entity) = self.remove_local(&key) {
                self.send_remove_to_rendered_viewers(players, rendering, &entity)?;
            }
        }

        let viewers = players.list_except(uuid::Uuid::nil());
        if viewers.is_empty() {
            return Ok(());
        }

        let mut updates = Vec::new();
        let mut removes = Vec::new();
        let mut tick_updates = Vec::new();
        let mut motion_updates = Vec::new();
        let dying_keys = self
            .entity_deaths
            .lock()
            .expect("entity death state poisoned")
            .keys()
            .cloned()
            .collect::<HashSet<_>>();
        let snapshot = self.active_ai_snapshot(&viewers, rendering, &dying_keys);
        {
            let mut motion = self
                .entity_motion
                .lock()
                .expect("entity motion state poisoned")
                .clone();
            let mut target_memory = self
                .entity_targets
                .lock()
                .expect("entity target state poisoned")
                .clone();
            let mut path_memory = self
                .entity_paths
                .lock()
                .expect("entity path state poisoned")
                .clone();
            let mut plugin_ai_memory = self
                .entity_plugin_ai
                .lock()
                .expect("entity plugin ai state poisoned")
                .clone();
            let mut collision_cache = std::mem::take(
                &mut *self
                    .collision_cache
                    .lock()
                    .expect("entity collision cache poisoned"),
            );
            collision_cache.sync_world_epoch(world.cache_epoch());
            let mut target_reselects = 0usize;
            let mut path_recalcs = 0usize;
            let mut plugin_ai_calls = 0usize;
            for mut entity in snapshot {
                let previous = entity.position;
                let mut movement = EntityMovement::default();
                let mut remove = false;
                let ai = ai_kind(&entity.ai);
                match ai {
                    EntityAiKind::None => {
                        target_memory.remove(&entity.key);
                        path_memory.remove(&entity.key);
                    }
                    EntityAiKind::RandomStroll => {
                        target_memory.remove(&entity.key);
                        path_memory.remove(&entity.key);
                        movement = apply_random_stroll(&mut entity, tick_ms);
                    }
                    EntityAiKind::LookAtPlayer => {
                        target_memory.remove(&entity.key);
                        path_memory.remove(&entity.key);
                        apply_look_at_nearest_player(
                            &mut entity,
                            &viewers,
                            rendering.default_distance,
                        );
                    }
                    EntityAiKind::FollowNearestPlayer => {
                        movement = apply_follow_nearest_player(
                            &mut entity,
                            &viewers,
                            tick_ms,
                            &mut target_memory,
                            &mut path_memory,
                            &mut target_reselects,
                            &mut path_recalcs,
                            world,
                            &mut collision_cache,
                            now,
                        );
                    }
                    EntityAiKind::Plugin => {
                        target_memory.remove(&entity.key);
                        path_memory.remove(&entity.key);
                        let operations = plugin_ai_operations(
                            &entity,
                            &viewers,
                            tick_ms,
                            plugins,
                            &mut plugin_ai_memory,
                            &mut plugin_ai_calls,
                            now,
                        );
                        apply_plugin_ai_operations(
                            &mut entity,
                            &mut movement,
                            &mut remove,
                            operations.as_slice(),
                        );
                    }
                }

                if remove {
                    target_memory.remove(&entity.key);
                    path_memory.remove(&entity.key);
                    plugin_ai_memory.remove(&entity.key);
                    removes.push(entity.key.clone());
                    continue;
                }

                let next_motion = if should_apply_entity_physics(&entity, ai) {
                    let motion = motion.entry(entity.key.clone()).or_default();
                    apply_entity_physics(
                        &mut entity,
                        world,
                        &mut collision_cache,
                        motion,
                        movement,
                        tick_ms,
                    );
                    let next_motion = *motion;
                    motion_updates.push(EntityMotionTickUpdate {
                        key: entity.key.clone(),
                        previous,
                        next: next_motion,
                    });
                    Some(next_motion)
                } else {
                    None
                };

                if position_changed(previous, entity.position) {
                    tick_updates.push(EntityAiTickUpdate {
                        key: entity.key.clone(),
                        previous,
                        next: entity.position,
                        motion: next_motion,
                    });
                }
            }
            *self
                .collision_cache
                .lock()
                .expect("entity collision cache poisoned") = collision_cache;
            *self
                .entity_targets
                .lock()
                .expect("entity target state poisoned") = target_memory;
            *self
                .entity_paths
                .lock()
                .expect("entity path state poisoned") = path_memory;
            *self
                .entity_plugin_ai
                .lock()
                .expect("entity plugin ai state poisoned") = plugin_ai_memory;
        }

        if !tick_updates.is_empty() || !motion_updates.is_empty() {
            let mut entities = self.entities.lock().expect("entity manager poisoned");
            let mut motion = self
                .entity_motion
                .lock()
                .expect("entity motion state poisoned");
            let entity_index = entities
                .iter()
                .enumerate()
                .map(|(index, entity)| (entity.key.clone(), index))
                .collect::<HashMap<_, _>>();
            for tick_update in tick_updates {
                let Some(index) = entity_index.get(&tick_update.key).copied() else {
                    continue;
                };
                let entity = &mut entities[index];
                if entity.kind != ManagedEntityKind::Entity
                    || position_changed(entity.position, tick_update.previous)
                {
                    continue;
                }
                entity.position = tick_update.next;
                let current_motion = if let Some(next_motion) = tick_update.motion {
                    motion.insert(entity.key.clone(), next_motion);
                    next_motion
                } else {
                    motion.get(&entity.key).copied().unwrap_or_default()
                };
                updates.push((entity.clone(), current_motion));
            }
            for motion_update in motion_updates {
                let Some(index) = entity_index.get(&motion_update.key).copied() else {
                    continue;
                };
                let entity = &entities[index];
                if entity.kind == ManagedEntityKind::Entity
                    && !position_changed(entity.position, motion_update.previous)
                {
                    motion.insert(motion_update.key, motion_update.next);
                }
            }
        }

        for (entity, motion) in updates {
            let packets = entity.position_packets_with_velocity(
                motion.velocity_x,
                motion.velocity_y,
                motion.velocity_z,
            )?;
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

    fn active_ai_snapshot(
        &self,
        viewers: &[crate::players::OnlinePlayer],
        rendering: &qexed_config::app::qexed::server::EntityRendering,
        dying_keys: &HashSet<String>,
    ) -> Vec<ManagedEntity> {
        let entities = self.entities.lock().expect("entity manager poisoned");
        let active_indexes = entities
            .iter()
            .enumerate()
            .filter(|(_, entity)| entity.kind == ManagedEntityKind::Entity)
            .filter(|(_, entity)| !dying_keys.contains(&entity.key))
            .filter(|(_, entity)| entity_has_active_viewer(entity, viewers, rendering))
            .map(|(index, _)| index)
            .collect::<Vec<_>>();

        if active_indexes.is_empty() {
            return Vec::new();
        }
        if active_indexes.len() <= ENTITY_AI_MAX_ENTITIES_PER_TICK {
            let mut cursor = self.ai_cursor.lock().expect("entity ai cursor poisoned");
            *cursor = 0;
            return active_indexes
                .into_iter()
                .map(|index| entities[index].clone())
                .collect();
        }

        let quota = ENTITY_AI_MAX_ENTITIES_PER_TICK.min(active_indexes.len());
        let mut cursor = self.ai_cursor.lock().expect("entity ai cursor poisoned");
        let start = *cursor % active_indexes.len();
        let snapshot = (0..quota)
            .map(|offset| active_indexes[(start + offset) % active_indexes.len()])
            .map(|index| entities[index].clone())
            .collect::<Vec<_>>();
        *cursor = (start + quota) % active_indexes.len();
        snapshot
    }

    fn expired_death_keys(&self, now: Instant) -> Vec<String> {
        let mut deaths = self
            .entity_deaths
            .lock()
            .expect("entity death state poisoned");
        let expired = deaths
            .iter()
            .filter_map(|(key, remove_at)| (*remove_at <= now).then(|| key.clone()))
            .collect::<Vec<_>>();
        for key in &expired {
            deaths.remove(key);
        }
        expired
    }

    fn custom_registration(&self, id: &str) -> Option<CustomEntityRegistration> {
        self.custom_entities
            .lock()
            .expect("entity custom registry poisoned")
            .get(id.trim())
            .cloned()
    }

    fn custom_entity_type_id(
        &self,
        id: &str,
        shell_entity_type: &str,
        definition: &crate::plugins::CustomEntityDefinition,
    ) -> Option<i32> {
        if let Some(registry_id) = definition.registry_id {
            return Some(registry_id);
        }
        if shell_entity_type != id {
            return entity_type_id(shell_entity_type).ok();
        }

        let mut ids = self
            .custom_entity_type_ids
            .lock()
            .expect("entity custom registry ids poisoned");
        if let Some(id) = ids.get(id) {
            return Some(*id);
        }
        let next_id = custom_entity_runtime_id(ids.len());
        ids.insert(id.to_string(), next_id);
        Some(next_id)
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

    fn spawn_position_for_rule(
        &self,
        rule: &qexed_config::app::qexed::server::EntitySpawnRule,
        dimension: &str,
        world: &crate::world::WorldManager,
    ) -> Option<EntityPosition> {
        let attempts = if rule.require_air || rule.require_ground {
            rule.position_attempts.max(1).min(64)
        } else {
            1
        };
        let mut collision_cache = self
            .collision_cache
            .lock()
            .expect("entity collision cache poisoned");
        collision_cache.sync_world_epoch(world.cache_epoch());
        for _ in 0..attempts {
            let position = EntityPosition {
                x: random_between(rule.min_x, rule.max_x),
                y: random_spawn_y(rule),
                z: random_between(rule.min_z, rule.max_z),
                yaw: rand::Rng::gen_range(&mut rand::thread_rng(), -180.0..180.0),
                pitch: 0.0,
                on_ground: rule.on_ground,
            };
            if spawn_position_passes(rule, dimension, world, &mut collision_cache, position) {
                return Some(position);
            }
        }
        None
    }
}

impl CollisionCache {
    fn sync_world_epoch(&mut self, world_epoch: u64) {
        if self.world_epoch == world_epoch {
            return;
        }
        self.world_epoch = world_epoch;
        self.blocks.clear();
        self.aabbs.clear();
    }

    fn block_collision_shape(
        &mut self,
        world: &crate::world::WorldManager,
        dimension: &str,
        x: i32,
        y: i32,
        z: i32,
    ) -> Option<crate::inventory::BlockCollisionShape> {
        let key = CollisionBlockKey {
            dimension: dimension.to_string(),
            x,
            y,
            z,
        };
        if let Some(shape) = self.blocks.get(&key) {
            return *shape;
        }
        let position = BlockPosition { x, y, z };
        let shape = world
            .cached_block_state_at(dimension, &position)
            .and_then(crate::inventory::block_collision_shape);
        if self.blocks.len() >= COLLISION_BLOCK_CACHE_LIMIT {
            self.blocks.clear();
        }
        self.blocks.insert(key, shape);
        shape
    }

    fn entity_aabb_intersects_solid(
        &mut self,
        world: &crate::world::WorldManager,
        dimension: &str,
        bounds: EntityAabb,
    ) -> bool {
        let key = bounds.cache_key(dimension);
        if let Some(collides) = self.aabbs.get(&key) {
            return *collides;
        }

        let mut collides = false;
        'blocks: for block_x in bounds.block_min_x()..=bounds.block_max_x() {
            for block_y in bounds.block_min_y()..=bounds.block_max_y() {
                for block_z in bounds.block_min_z()..=bounds.block_max_z() {
                    let Some(shape) =
                        self.block_collision_shape(world, dimension, block_x, block_y, block_z)
                    else {
                        continue;
                    };
                    if bounds.intersects_block_shape(block_x, block_y, block_z, shape) {
                        collides = true;
                        break 'blocks;
                    }
                }
            }
        }

        if self.aabbs.len() >= COLLISION_AABB_CACHE_LIMIT {
            self.aabbs.clear();
        }
        self.aabbs.insert(key, collides);
        collides
    }

    fn max_collision_top_for_aabb(
        &mut self,
        world: &crate::world::WorldManager,
        dimension: &str,
        bounds: EntityAabb,
    ) -> Option<f64> {
        let mut top = None::<f64>;
        for block_x in bounds.block_min_x()..=bounds.block_max_x() {
            for block_y in bounds.block_min_y()..=bounds.block_max_y() {
                for block_z in bounds.block_min_z()..=bounds.block_max_z() {
                    let Some(shape) =
                        self.block_collision_shape(world, dimension, block_x, block_y, block_z)
                    else {
                        continue;
                    };
                    if bounds.intersects_block_shape(block_x, block_y, block_z, shape) {
                        top = Some(
                            top.unwrap_or(f64::NEG_INFINITY)
                                .max(f64::from(block_y) + shape.max_y),
                        );
                    }
                }
            }
        }
        top
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
    use super::{
        EntityTargetMemory, normalize_uuid, preferred_follow_target, target_reselect_interval,
    };
    use std::{
        collections::HashMap,
        time::{Duration, Instant},
    };

    use qexed_protocol::to_client::play::add_entity::EntityPosition;

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

    #[test]
    fn preferred_follow_target_keeps_recent_target() {
        let entity = test_entity("steady_follower", 0.0);
        let current = test_player("Current", 10.0);
        let nearer = test_player("Nearer", -1.0);
        let now = Instant::now();
        let mut target_memory = HashMap::from([(
            entity.key.clone(),
            EntityTargetMemory {
                player_id: current.profile.uuid,
                selected_at: now,
            },
        )]);
        let mut target_reselects = 0;
        let viewers = [current.clone(), nearer];

        let selected = preferred_follow_target(
            &entity,
            &viewers,
            &mut target_memory,
            &mut target_reselects,
            now + Duration::from_millis(200),
            32.0,
        )
        .expect("target");

        assert_eq!(selected.profile.uuid, current.profile.uuid);
        assert_eq!(target_reselects, 0);
        assert_eq!(
            target_memory
                .get(&entity.key)
                .map(|memory| memory.player_id),
            Some(current.profile.uuid)
        );
    }

    #[test]
    fn preferred_follow_target_limits_reselects_per_tick() {
        let entity = test_entity("steady_follower", 0.0);
        let current = test_player("Current", 10.0);
        let nearer = test_player("Nearer", -1.0);
        let now = Instant::now();
        let mut target_memory = HashMap::from([(
            entity.key.clone(),
            EntityTargetMemory {
                player_id: current.profile.uuid,
                selected_at: now - target_reselect_interval(&entity.key),
            },
        )]);
        let mut target_reselects = super::ENTITY_TARGET_RESELECTS_PER_TICK;
        let viewers = [current.clone(), nearer];

        let selected = preferred_follow_target(
            &entity,
            &viewers,
            &mut target_memory,
            &mut target_reselects,
            now,
            32.0,
        )
        .expect("target");

        assert_eq!(selected.profile.uuid, current.profile.uuid);
        assert_eq!(target_reselects, super::ENTITY_TARGET_RESELECTS_PER_TICK);
        assert_eq!(
            target_memory
                .get(&entity.key)
                .map(|memory| memory.player_id),
            Some(current.profile.uuid)
        );
    }

    fn test_entity(key: &str, x: f64) -> super::ManagedEntity {
        super::ManagedEntity {
            key: key.to_string(),
            entity_id: 1,
            uuid: uuid::Uuid::new_v4(),
            kind: super::ManagedEntityKind::Entity,
            entity_type: "minecraft:zombie".to_string(),
            entity_type_id: 1,
            dimension: "minecraft:overworld".to_string(),
            position: EntityPosition {
                x,
                y: 64.0,
                z: 0.0,
                yaw: 0.0,
                pitch: 0.0,
                on_ground: true,
            },
            name: key.to_string(),
            display_name: key.to_string(),
            skin_textures: String::new(),
            skin_signature: String::new(),
            data: 0,
            ai: "follow_nearest_player".to_string(),
            ai_params: Default::default(),
            auto_jump: false,
            spawn_rule: String::new(),
            custom_type: String::new(),
            look_at_players: false,
            main_hand_event: "interact".to_string(),
            off_hand_event: "interact_off_hand".to_string(),
            attack_event: "attack".to_string(),
        }
    }

    fn test_player(username: &str, x: f64) -> crate::players::OnlinePlayer {
        crate::players::OnlinePlayer {
            profile: qexed_packet::net_types::GameProfile {
                uuid: uuid::Uuid::new_v4(),
                username: username.to_string(),
                properties: Vec::new(),
            },
            entity_id: 1,
            position: EntityPosition {
                x,
                y: 64.0,
                z: 0.0,
                yaw: 0.0,
                pitch: 0.0,
                on_ground: true,
            },
            dimension: "minecraft:overworld".to_string(),
            equipment: Vec::new(),
            language: "en_us".to_string(),
            displayed_skin_parts: crate::players::DEFAULT_DISPLAYED_SKIN_PARTS,
        }
    }
}

fn can_reach_item(collector: EntityPosition, item: EntityPosition) -> bool {
    (collector.x - item.x).abs() <= ITEM_PICKUP_RADIUS_XZ
        && (collector.y + 0.9 - item.y).abs() <= ITEM_PICKUP_RADIUS_Y
        && (collector.z - item.z).abs() <= ITEM_PICKUP_RADIUS_XZ
}

fn dropped_item_update_packets(updates: &[DroppedItemUpdate]) -> Result<Vec<Bytes>> {
    let item_entity_type = entity_type_id("minecraft:item")?;
    let mut packets = Vec::new();
    for update in updates {
        match update {
            DroppedItemUpdate::Spawned(entity) => {
                packets.extend(entity.spawn_packets(item_entity_type)?);
            }
            DroppedItemUpdate::Merged(entity) => {
                packets.extend(entity.metadata_packets()?);
            }
        }
    }
    Ok(packets)
}

fn distance_sq(left: EntityPosition, right: EntityPosition) -> f64 {
    let dx = left.x - right.x;
    let dy = left.y - right.y;
    let dz = left.z - right.z;
    dx * dx + dy * dy + dz * dz
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

fn default_entity_health(entity_type: &str) -> f32 {
    match entity_type {
        "minecraft:warden" => 500.0,
        "minecraft:ender_dragon" => 200.0,
        "minecraft:wither" => 300.0,
        "minecraft:iron_golem" => 100.0,
        "minecraft:ravager" => 100.0,
        "minecraft:ghast" => 10.0,
        "minecraft:slime" | "minecraft:magma_cube" => 16.0,
        "minecraft:chicken"
        | "minecraft:rabbit"
        | "minecraft:bat"
        | "minecraft:cod"
        | "minecraft:salmon"
        | "minecraft:tropical_fish"
        | "minecraft:pufferfish" => 6.0,
        "minecraft:sheep" | "minecraft:pig" | "minecraft:cow" | "minecraft:goat"
        | "minecraft:wolf" | "minecraft:cat" | "minecraft:ocelot" | "minecraft:fox" => 10.0,
        "minecraft:zombie"
        | "minecraft:husk"
        | "minecraft:drowned"
        | "minecraft:skeleton"
        | "minecraft:stray"
        | "minecraft:creeper"
        | "minecraft:spider"
        | "minecraft:cave_spider"
        | "minecraft:pillager"
        | "minecraft:vindicator"
        | "minecraft:witch"
        | "minecraft:piglin"
        | "minecraft:zombified_piglin" => 20.0,
        "minecraft:enderman" => 40.0,
        _ => 20.0,
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

fn smooth_rotation(
    from: EntityPosition,
    target_yaw: f32,
    target_pitch: f32,
    tick_ms: u64,
) -> (f32, f32) {
    let tick_scale = (tick_ms as f32 / 50.0).clamp(0.25, 4.0);
    (
        rotate_toward(
            from.yaw,
            target_yaw,
            ENTITY_MAX_YAW_TURN_PER_TICK * tick_scale,
        ),
        rotate_toward(
            from.pitch,
            target_pitch,
            ENTITY_MAX_PITCH_TURN_PER_TICK * tick_scale,
        ),
    )
}

fn rotate_toward(current: f32, target: f32, max_delta: f32) -> f32 {
    current + angle_delta(current, target).clamp(-max_delta, max_delta)
}

fn angle_delta(current: f32, target: f32) -> f32 {
    let mut delta = (target - current).rem_euclid(360.0);
    if delta > 180.0 {
        delta -= 360.0;
    }
    delta
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
    let target_yaw = rand::Rng::gen_range(&mut rng, -180.0..180.0);
    let (yaw, _) = smooth_rotation(entity.position, target_yaw, entity.position.pitch, tick_ms);
    let radians = f64::from(yaw).to_radians();
    let speed = rand::Rng::gen_range(&mut rng, 0.10..0.20);
    entity.position.yaw = yaw;
    EntityMovement {
        x: -radians.sin() * speed,
        y: 0.0,
        z: radians.cos() * speed,
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
    let (target_yaw, target_pitch) = look_rotation(entity.position, target.position);
    let (yaw, pitch) = smooth_rotation(entity.position, target_yaw, target_pitch, 50);
    entity.position.yaw = yaw;
    entity.position.pitch = pitch;
}

fn apply_follow_nearest_player(
    entity: &mut ManagedEntity,
    viewers: &[crate::players::OnlinePlayer],
    tick_ms: u64,
    target_memory: &mut HashMap<String, EntityTargetMemory>,
    path_memory: &mut HashMap<String, EntityPathMemory>,
    target_reselects: &mut usize,
    path_recalcs: &mut usize,
    world: &crate::world::WorldManager,
    collision_cache: &mut CollisionCache,
    now: Instant,
) -> EntityMovement {
    const FOLLOW_RANGE: f64 = 32.0;
    const STOP_DISTANCE: f64 = 2.0;
    const BASE_STEP: f64 = 0.22;

    let Some(target) = preferred_follow_target(
        entity,
        viewers,
        target_memory,
        target_reselects,
        now,
        FOLLOW_RANGE,
    ) else {
        target_memory.remove(&entity.key);
        path_memory.remove(&entity.key);
        return EntityMovement::default();
    };
    let target_dx = target.position.x - entity.position.x;
    let target_dz = target.position.z - entity.position.z;
    let target_horizontal = (target_dx * target_dx + target_dz * target_dz).sqrt();
    let (target_yaw, target_pitch) = look_rotation(entity.position, target.position);
    let (yaw, pitch) = smooth_rotation(entity.position, target_yaw, target_pitch, tick_ms);
    entity.position.yaw = yaw;
    entity.position.pitch = pitch;
    if target_horizontal <= STOP_DISTANCE {
        path_memory.remove(&entity.key);
        return EntityMovement::default();
    }

    let move_target = follow_move_target(
        entity,
        target,
        world,
        collision_cache,
        path_memory,
        path_recalcs,
        now,
    );
    let dx = move_target.x - entity.position.x;
    let dz = move_target.z - entity.position.z;
    let horizontal = (dx * dx + dz * dz).sqrt();
    if horizontal <= f64::EPSILON {
        return EntityMovement::default();
    }
    let speed = BASE_STEP.min(horizontal);
    EntityMovement {
        x: dx / horizontal * speed,
        y: 0.0,
        z: dz / horizontal * speed,
    }
}

fn follow_move_target(
    entity: &ManagedEntity,
    target: &crate::players::OnlinePlayer,
    world: &crate::world::WorldManager,
    collision_cache: &mut CollisionCache,
    path_memory: &mut HashMap<String, EntityPathMemory>,
    path_recalcs: &mut usize,
    now: Instant,
) -> EntityPosition {
    if let Some(position) = direct_follow_move_target(entity, target, world, collision_cache) {
        path_memory.remove(&entity.key);
        return position;
    }

    let target_block = position_block(target.position);
    let entity_block = position_block(entity.position);
    let should_recalculate = path_memory.get(&entity.key).is_none_or(|path| {
        path.target_player_id != target.profile.uuid
            || path.cursor >= path.waypoints.len()
            || (now.duration_since(path.calculated_at) >= ENTITY_PATH_RECALC_INTERVAL
                && block_horizontal_distance_sq(&path.target_block, &target_block)
                    >= ENTITY_PATH_TARGET_REPLAN_DISTANCE_SQ)
    });

    if should_recalculate && *path_recalcs < ENTITY_PATH_RECALCS_PER_TICK {
        *path_recalcs = path_recalcs.saturating_add(1);
        if let Some(path) =
            crate::play::pathfinding::find_path(crate::play::pathfinding::PathQuery {
                world,
                dimension: &entity.dimension,
                start: entity_block,
                goal: target_block.clone(),
                max_nodes: ENTITY_PATH_MAX_NODES,
                cached_only: true,
            })
        {
            let waypoints = path.into_iter().skip(1).collect::<Vec<_>>();
            if waypoints.is_empty() {
                path_memory.remove(&entity.key);
            } else {
                path_memory.insert(
                    entity.key.clone(),
                    EntityPathMemory {
                        target_player_id: target.profile.uuid,
                        target_block,
                        calculated_at: now,
                        waypoints,
                        cursor: 0,
                    },
                );
            }
        } else {
            path_memory.remove(&entity.key);
        }
    }

    if let Some(path) = path_memory.get_mut(&entity.key) {
        advance_path_cursor(path, entity.position);
        if let Some(waypoint) = path.waypoints.get(path.cursor) {
            return block_center_position(waypoint);
        }
    }

    target.position
}

fn direct_follow_move_target(
    entity: &ManagedEntity,
    target: &crate::players::OnlinePlayer,
    world: &crate::world::WorldManager,
    collision_cache: &mut CollisionCache,
) -> Option<EntityPosition> {
    let dx = target.position.x - entity.position.x;
    let dz = target.position.z - entity.position.z;
    let distance = (dx * dx + dz * dz).sqrt();
    if distance <= f64::EPSILON || distance > ENTITY_DIRECT_FOLLOW_DISTANCE {
        return None;
    }
    if (target.position.y.floor() - entity.position.y.floor()).abs() > 1.0 {
        return None;
    }

    let steps = (distance * ENTITY_DIRECT_FOLLOW_SAMPLES_PER_BLOCK)
        .ceil()
        .clamp(1.0, 24.0) as usize;
    for step in 1..=steps {
        let ratio = step as f64 / steps as f64;
        let position = BlockPosition {
            x: (entity.position.x + dx * ratio).floor() as i32,
            y: entity.position.y.floor() as i32,
            z: (entity.position.z + dz * ratio).floor() as i32,
        };
        if !direct_follow_block_is_walkable(world, collision_cache, &entity.dimension, &position) {
            return None;
        }
    }

    Some(target.position)
}

fn direct_follow_block_is_walkable(
    world: &crate::world::WorldManager,
    collision_cache: &mut CollisionCache,
    dimension: &str,
    feet: &BlockPosition,
) -> bool {
    const STEPABLE_COLLISION_HEIGHT: f64 = 0.6;
    let head = BlockPosition {
        x: feet.x,
        y: feet.y + 1,
        z: feet.z,
    };
    let below = BlockPosition {
        x: feet.x,
        y: feet.y - 1,
        z: feet.z,
    };
    let feet_shape =
        collision_cache.block_collision_shape(world, dimension, feet.x, feet.y, feet.z);
    let head_shape =
        collision_cache.block_collision_shape(world, dimension, head.x, head.y, head.z);
    let below_shape =
        collision_cache.block_collision_shape(world, dimension, below.x, below.y, below.z);

    let feet_clear = feet_shape.is_none_or(|shape| shape.max_y <= STEPABLE_COLLISION_HEIGHT);
    let head_clear = head_shape.is_none();
    let has_support = below_shape.is_some() || feet_shape.is_some_and(|shape| shape.max_y > 0.0);
    feet_clear && head_clear && has_support
}

fn advance_path_cursor(path: &mut EntityPathMemory, position: EntityPosition) {
    while let Some(waypoint) = path.waypoints.get(path.cursor) {
        let waypoint_position = block_center_position(waypoint);
        if horizontal_distance_sq(position, waypoint_position)
            > ENTITY_PATH_WAYPOINT_REACHED * ENTITY_PATH_WAYPOINT_REACHED
        {
            break;
        }
        path.cursor += 1;
    }
}

fn position_block(position: EntityPosition) -> BlockPosition {
    BlockPosition {
        x: position.x.floor() as i32,
        y: position.y.floor() as i32,
        z: position.z.floor() as i32,
    }
}

fn block_horizontal_distance_sq(left: &BlockPosition, right: &BlockPosition) -> i32 {
    let dx = left.x - right.x;
    let dz = left.z - right.z;
    dx * dx + dz * dz
}

fn block_center_position(position: &BlockPosition) -> EntityPosition {
    EntityPosition {
        x: f64::from(position.x) + 0.5,
        y: f64::from(position.y),
        z: f64::from(position.z) + 0.5,
        yaw: 0.0,
        pitch: 0.0,
        on_ground: true,
    }
}

fn preferred_follow_target<'a>(
    entity: &ManagedEntity,
    viewers: &'a [crate::players::OnlinePlayer],
    target_memory: &mut HashMap<String, EntityTargetMemory>,
    target_reselects: &mut usize,
    now: Instant,
    range: f64,
) -> Option<&'a crate::players::OnlinePlayer> {
    let remembered = target_memory.get(&entity.key).copied();
    let current = remembered.and_then(|memory| {
        viewers.iter().find(|player| {
            player.profile.uuid == memory.player_id
                && player.dimension == entity.dimension
                && within_render_distance(entity.position, player.position, range)
        })
    });
    let should_reselect = current.is_none()
        || remembered.is_none_or(|memory| {
            now.duration_since(memory.selected_at) >= target_reselect_interval(&entity.key)
        });

    if !should_reselect {
        return current;
    }
    if current.is_some() && *target_reselects >= ENTITY_TARGET_RESELECTS_PER_TICK {
        return current;
    }
    *target_reselects = target_reselects.saturating_add(1);

    let nearest = nearest_player(entity.position, &entity.dimension, viewers, range);
    let selected = match (current, nearest) {
        (Some(current), Some(nearest)) => {
            let current_distance = horizontal_distance_sq(entity.position, current.position);
            let nearest_distance = horizontal_distance_sq(entity.position, nearest.position);
            if nearest.profile.uuid != current.profile.uuid
                && nearest_distance < current_distance * ENTITY_TARGET_SWITCH_ADVANTAGE
            {
                nearest
            } else {
                current
            }
        }
        (Some(current), None) => current,
        (None, Some(nearest)) => nearest,
        (None, None) => {
            target_memory.remove(&entity.key);
            return None;
        }
    };
    target_memory.insert(
        entity.key.clone(),
        EntityTargetMemory {
            player_id: selected.profile.uuid,
            selected_at: now,
        },
    );
    Some(selected)
}

fn target_reselect_interval(entity_key: &str) -> Duration {
    ENTITY_TARGET_RESELECT_INTERVAL + Duration::from_millis(stable_jitter_ms(entity_key))
}

fn stable_jitter_ms(value: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash % (ENTITY_TARGET_RESELECT_JITTER_MS + 1)
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
    collision_cache: &mut CollisionCache,
    motion: &mut EntityMotion,
    movement: EntityMovement,
    tick_ms: u64,
) {
    let tick_scale = (tick_ms as f64 / 50.0).clamp(0.25, 4.0);
    let mut dy = movement.y.clamp(-4.0, 4.0);
    apply_horizontal_motion(motion, movement, tick_scale);
    let dx = (motion.velocity_x * tick_scale).clamp(-4.0, 4.0);
    let dz = (motion.velocity_z * tick_scale).clamp(-4.0, 4.0);

    if entity_has_ground(world, collision_cache, &entity.dimension, entity.position)
        && motion.velocity_y <= 0.0
    {
        entity.position.on_ground = true;
        motion.velocity_y = 0.0;
    } else {
        entity.position.on_ground = false;
        motion.velocity_y = (motion.velocity_y - ENTITY_GRAVITY_PER_TICK * tick_scale)
            .max(ENTITY_TERMINAL_VELOCITY);
        dy += motion.velocity_y * tick_scale;
    }

    if dy != 0.0 {
        move_entity_axis(entity, world, collision_cache, motion, 0.0, dy, 0.0);
    }
    if dx != 0.0 {
        move_entity_axis(entity, world, collision_cache, motion, dx, 0.0, 0.0);
    }
    if dz != 0.0 {
        move_entity_axis(entity, world, collision_cache, motion, 0.0, 0.0, dz);
    }

    if entity_has_ground(world, collision_cache, &entity.dimension, entity.position)
        && motion.velocity_y <= 0.0
    {
        entity.position.on_ground = true;
        motion.velocity_y = 0.0;
    } else {
        entity.position.on_ground = false;
    }
}

fn apply_horizontal_motion(motion: &mut EntityMotion, movement: EntityMovement, tick_scale: f64) {
    let target_x = movement
        .x
        .clamp(-ENTITY_MAX_HORIZONTAL_SPEED, ENTITY_MAX_HORIZONTAL_SPEED);
    let target_z = movement
        .z
        .clamp(-ENTITY_MAX_HORIZONTAL_SPEED, ENTITY_MAX_HORIZONTAL_SPEED);
    if target_x == 0.0 && target_z == 0.0 {
        let friction = ENTITY_HORIZONTAL_FRICTION.powf(tick_scale);
        motion.velocity_x *= friction;
        motion.velocity_z *= friction;
        if motion.velocity_x.abs() < 0.001 {
            motion.velocity_x = 0.0;
        }
        if motion.velocity_z.abs() < 0.001 {
            motion.velocity_z = 0.0;
        }
        return;
    }

    let acceleration = ENTITY_HORIZONTAL_ACCELERATION * tick_scale;
    motion.velocity_x = approach(motion.velocity_x, target_x, acceleration);
    motion.velocity_z = approach(motion.velocity_z, target_z, acceleration);
}

fn approach(current: f64, target: f64, max_delta: f64) -> f64 {
    current + (target - current).clamp(-max_delta, max_delta)
}

fn move_entity_axis(
    entity: &mut ManagedEntity,
    world: &crate::world::WorldManager,
    collision_cache: &mut CollisionCache,
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
        collision_cache,
        &entity.dimension,
        next_x,
        next_y,
        next_z,
        ENTITY_PHYSICS_WIDTH,
        ENTITY_PHYSICS_HEIGHT,
    ) {
        if dy == 0.0
            && (dx != 0.0 || dz != 0.0)
            && entity.position.on_ground
            && try_entity_step_up(entity, world, collision_cache, next_x, next_z)
        {
            return;
        }
        if entity_can_auto_jump(entity)
            && dy == 0.0
            && (dx != 0.0 || dz != 0.0)
            && entity.position.on_ground
            && can_entity_auto_jump(entity, world, collision_cache, dx, dz)
        {
            motion.velocity_y = ENTITY_AUTO_JUMP_VELOCITY;
            entity.position.y += ENTITY_AUTO_JUMP_VELOCITY;
            entity.position.on_ground = false;
            return;
        }
        if dx != 0.0 {
            motion.velocity_x = 0.0;
        }
        if dz != 0.0 {
            motion.velocity_z = 0.0;
        }
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

fn try_entity_step_up(
    entity: &mut ManagedEntity,
    world: &crate::world::WorldManager,
    collision_cache: &mut CollisionCache,
    next_x: f64,
    next_z: f64,
) -> bool {
    let forward = EntityAabb::new(
        next_x,
        entity.position.y,
        next_z,
        ENTITY_PHYSICS_WIDTH,
        ENTITY_PHYSICS_HEIGHT,
    );
    let Some(obstacle_top) =
        collision_cache.max_collision_top_for_aabb(world, &entity.dimension, forward)
    else {
        return false;
    };
    let step_height = obstacle_top - entity.position.y;
    if !(0.0..=ENTITY_MAX_STEP_HEIGHT).contains(&step_height) {
        return false;
    }
    let stepped_y = obstacle_top;
    if entity_aabb_intersects_solid_at(
        world,
        collision_cache,
        &entity.dimension,
        next_x,
        stepped_y,
        next_z,
    ) {
        return false;
    }
    entity.position.x = next_x;
    entity.position.y = stepped_y;
    entity.position.z = next_z;
    entity.position.on_ground = true;
    true
}

fn entity_can_auto_jump(entity: &ManagedEntity) -> bool {
    entity.auto_jump
}

fn can_entity_auto_jump(
    entity: &ManagedEntity,
    world: &crate::world::WorldManager,
    collision_cache: &mut CollisionCache,
    dx: f64,
    dz: f64,
) -> bool {
    let jump_y = entity.position.y + 1.0;
    !entity_aabb_intersects_solid_at(
        world,
        collision_cache,
        &entity.dimension,
        entity.position.x,
        jump_y,
        entity.position.z,
    ) && !entity_aabb_intersects_solid_at(
        world,
        collision_cache,
        &entity.dimension,
        entity.position.x + dx,
        jump_y,
        entity.position.z + dz,
    )
}

fn plugin_ai_operations(
    entity: &ManagedEntity,
    viewers: &[crate::players::OnlinePlayer],
    tick_ms: u64,
    plugins: &crate::plugins::PluginManager,
    plugin_ai_memory: &mut HashMap<String, EntityPluginAiMemory>,
    plugin_ai_calls: &mut usize,
    now: Instant,
) -> Vec<crate::plugins::EntityAiOperation> {
    let interval = plugin_ai_interval(entity);
    let cached = plugin_ai_memory.get(&entity.key);
    let should_call = cached.is_none_or(|memory| now.duration_since(memory.last_tick) >= interval);
    if should_call && *plugin_ai_calls < ENTITY_PLUGIN_AI_CALLS_PER_TICK {
        *plugin_ai_calls = plugin_ai_calls.saturating_add(1);
        let elapsed_ms = cached
            .map(|memory| now.duration_since(memory.last_tick).as_millis() as u64)
            .unwrap_or(tick_ms)
            .max(tick_ms)
            .min(10_000);
        let operations =
            plugins.handle_entity_ai_tick(entity_ai_query(entity, viewers, elapsed_ms));
        plugin_ai_memory.insert(
            entity.key.clone(),
            EntityPluginAiMemory {
                last_tick: now,
                operations: operations.clone(),
            },
        );
        return operations;
    }

    cached
        .map(|memory| memory.operations.clone())
        .unwrap_or_default()
}

fn plugin_ai_interval(entity: &ManagedEntity) -> Duration {
    let interval_ms = entity
        .ai_params
        .get("tick_interval_ms")
        .or_else(|| entity.ai_params.get("ai_tick_interval_ms"))
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(ENTITY_PLUGIN_AI_DEFAULT_INTERVAL.as_millis() as u64)
        .clamp(50, 10_000);
    Duration::from_millis(interval_ms)
}

fn apply_plugin_ai_operations(
    entity: &mut ManagedEntity,
    movement: &mut EntityMovement,
    remove: &mut bool,
    operations: &[crate::plugins::EntityAiOperation],
) {
    for operation in operations {
        match operation {
            crate::plugins::EntityAiOperation::MoveDelta {
                x,
                y,
                z,
                yaw,
                pitch,
            } => {
                movement.x += finite_or_zero(*x).clamp(-4.0, 4.0);
                movement.y += finite_or_zero(*y).clamp(-4.0, 4.0);
                movement.z += finite_or_zero(*z).clamp(-4.0, 4.0);
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
                        x: *x,
                        y: *y,
                        z: *z,
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
                *remove = true;
            }
        }
    }
}

fn entity_has_ground(
    world: &crate::world::WorldManager,
    collision_cache: &mut CollisionCache,
    dimension: &str,
    position: EntityPosition,
) -> bool {
    collision_cache.entity_aabb_intersects_solid(
        world,
        dimension,
        EntityAabb::ground_probe(position, ENTITY_PHYSICS_WIDTH),
    )
}

fn entity_aabb_intersects_solid(
    world: &crate::world::WorldManager,
    collision_cache: &mut CollisionCache,
    dimension: &str,
    x: f64,
    y: f64,
    z: f64,
    width: f64,
    height: f64,
) -> bool {
    collision_cache.entity_aabb_intersects_solid(
        world,
        dimension,
        EntityAabb::new(x, y, z, width, height),
    )
}

fn entity_aabb_intersects_solid_at(
    world: &crate::world::WorldManager,
    collision_cache: &mut CollisionCache,
    dimension: &str,
    x: f64,
    y: f64,
    z: f64,
) -> bool {
    entity_aabb_intersects_solid(
        world,
        collision_cache,
        dimension,
        x,
        y,
        z,
        ENTITY_PHYSICS_WIDTH,
        ENTITY_PHYSICS_HEIGHT,
    )
}

fn entity_ai_query(
    entity: &ManagedEntity,
    viewers: &[crate::players::OnlinePlayer],
    tick_ms: u64,
) -> crate::plugins::EntityAiTickQuery {
    let nearby_players = viewers
        .iter()
        .filter(|player| player.dimension == entity.dimension)
        .filter(|player| {
            within_render_distance(
                entity.position,
                player.position,
                ENTITY_PLUGIN_AI_NEARBY_RANGE,
            )
        })
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
            ai_params: entity.ai_params.clone(),
            dimension: entity.dimension.clone(),
            position: qexed_plugin_api::player_position_payload(entity.position),
        },
        nearby_players,
        tick_ms,
    }
}

fn entity_has_active_viewer(
    entity: &ManagedEntity,
    viewers: &[crate::players::OnlinePlayer],
    rendering: &qexed_config::app::qexed::server::EntityRendering,
) -> bool {
    let range = render_distance_for_entity(entity, rendering).max(32.0);
    viewers.iter().any(|player| {
        player.dimension == entity.dimension
            && within_render_distance(entity.position, player.position, range)
    })
}

fn entity_visible_to_player(
    entity: &ManagedEntity,
    player: &crate::players::OnlinePlayer,
    rendering: &qexed_config::app::qexed::server::EntityRendering,
) -> bool {
    player.dimension == entity.dimension
        && within_render_distance(
            entity.position,
            player.position,
            render_distance_for_entity(entity, rendering),
        )
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

fn spawn_position_passes(
    rule: &qexed_config::app::qexed::server::EntitySpawnRule,
    dimension: &str,
    world: &crate::world::WorldManager,
    collision_cache: &mut CollisionCache,
    position: EntityPosition,
) -> bool {
    if rule.require_ground && !entity_has_ground(world, collision_cache, dimension, position) {
        return false;
    }
    if rule.require_air
        && entity_aabb_intersects_solid(
            world,
            collision_cache,
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
    let (
        entity_type,
        entity_type_id,
        custom_type,
        default_display_name,
        default_ai,
        default_ai_params,
    ) = match custom {
        Some(custom) => (
            custom.shell_entity_type,
            custom.entity_type_id,
            custom.id,
            custom.display_name,
            custom.ai,
            custom.ai_params,
        ),
        None => (
            rule.entity_type.trim().to_string(),
            None,
            String::new(),
            String::new(),
            String::new(),
            std::collections::BTreeMap::new(),
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
    } else if !default_ai.trim().is_empty() {
        default_ai
    } else {
        "random_stroll".to_string()
    };
    let mut ai_params = default_ai_params;
    ai_params.extend(rule.ai_params.clone());
    Ok(EntitySpawnRequest {
        key: format!("spawn:{rule_id}:{sequence}"),
        kind: ManagedEntityKind::Entity,
        entity_type: entity_type.clone(),
        entity_type_id_override: entity_type_id,
        dimension: dimension.to_string(),
        position,
        name,
        display_name,
        skin_textures: String::new(),
        skin_signature: String::new(),
        data: rule.data,
        ai,
        ai_params,
        auto_jump: rule.auto_jump,
        spawn_rule: rule_id.to_string(),
        custom_type,
        look_at_players: false,
        main_hand_event: "interact".to_string(),
        off_hand_event: "interact_off_hand".to_string(),
        attack_event: "attack".to_string(),
    })
}

fn custom_entity_shell_type(definition: &crate::plugins::CustomEntityDefinition) -> String {
    let shell_entity_type = definition.shell_entity_type.trim();
    if !shell_entity_type.is_empty() {
        return shell_entity_type.to_string();
    }
    let legacy_entity_type = definition.entity_type.trim();
    if !legacy_entity_type.is_empty() {
        return legacy_entity_type.to_string();
    }
    definition.id.trim().to_string()
}

fn custom_entity_runtime_id(index: usize) -> i32 {
    10_000 + i32::try_from(index).unwrap_or(i32::MAX - 10_000)
}

fn normalized_npc_client_entity_type(key: &str, entity_type: &str) -> String {
    let entity_type = entity_type.trim();
    if entity_type.is_empty() {
        "minecraft:player".to_string()
    } else if entity_type_id(entity_type).is_ok() {
        entity_type.to_string()
    } else {
        log::warn!(
            "npc client entity type is unknown, falling back to minecraft:player: key={}, entity_type={}",
            key,
            entity_type
        );
        "minecraft:player".to_string()
    }
}

fn is_known_minecraft_entity_type(entity_type: &str) -> bool {
    let entity_type = entity_type.trim();
    !entity_type.is_empty()
        && entity_type.starts_with("minecraft:")
        && entity_type_id(entity_type).is_ok()
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

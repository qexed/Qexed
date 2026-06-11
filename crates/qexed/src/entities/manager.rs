use std::{
    collections::{BTreeMap, HashMap, HashSet},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use bytes::Bytes;
use qexed_packet::net_types::Position as BlockPosition;
use qexed_protocol::to_client::play::{add_entity::EntityPosition, entity_event::EntityEvent};
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
const ENTITY_FIRE_DAMAGE_INTERVAL_TICKS: i32 = 20;
const ENTITY_FIRE_DAMAGE: f32 = 1.0;
const ENTITY_DAYLIGHT_FIRE_TICKS: i32 = 160;
const ENTITY_IGNITE_EVENT_ID: u8 = 37;
const ENTITY_MELEE_ATTACK_INTERVAL_TICKS: i32 = 20;
const CREEPER_SWELL_TICKS: i32 = 30;
const CREEPER_EXPLOSION_RADIUS: f64 = 3.0;
const CREEPER_EXPLOSION_DAMAGE: f32 = 43.0;
const ENTITY_PROJECTILE_ARROW_AI: &str = "vanilla_projectile:arrow";
const ENTITY_PROJECTILE_SMALL_FIREBALL_AI: &str = "vanilla_projectile:small_fireball";
const ENTITY_PROJECTILE_FIREBALL_AI: &str = "vanilla_projectile:fireball";
const ENTITY_PROJECTILE_POTION_AI: &str = "vanilla_projectile:potion";
const ENTITY_PROJECTILE_SHULKER_BULLET_AI: &str = "vanilla_projectile:shulker_bullet";
const ENTITY_PROJECTILE_WIND_CHARGE_AI: &str = "vanilla_projectile:wind_charge";
const ENTITY_PROJECTILE_WITHER_SKULL_AI: &str = "vanilla_projectile:wither_skull";
const ENTITY_PROJECTILE_TRIDENT_AI: &str = "vanilla_projectile:trident";
const ENTITY_PROJECTILE_SNOWBALL_AI: &str = "vanilla_projectile:snowball";
const ENTITY_PROJECTILE_LIFETIME_TICKS: i32 = 1_200;
const ENTITY_PROJECTILE_GRAVITY_PER_TICK: f64 = 0.05;
const ENTITY_PROJECTILE_HIT_RADIUS: f64 = 0.7;
const ENTITY_ARROW_DEFAULT_DAMAGE: f32 = 4.0;
const ENTITY_ARROW_DEFAULT_SPEED: f64 = 1.6;
const ENTITY_TRIDENT_DEFAULT_DAMAGE: f32 = 8.0;
const ENTITY_TRIDENT_DEFAULT_SPEED: f64 = 1.9;
const ENTITY_SMALL_FIREBALL_DEFAULT_DAMAGE: f32 = 5.0;
const ENTITY_FIREBALL_DEFAULT_EXPLOSION_RADIUS: f64 = 2.0;
const ENTITY_FIREBALL_DEFAULT_EXPLOSION_DAMAGE: f32 = 17.0;
const ENTITY_SHULKER_BULLET_DAMAGE: f32 = 4.0;
const ENTITY_SHULKER_BULLET_LEVITATION_TICKS: i32 = 20 * 10;
const ENTITY_SHULKER_BULLET_HOMING_BLEND: f64 = 0.2;
const ENTITY_WIND_CHARGE_EXPLOSION_RADIUS: f64 = 2.0;
const ENTITY_WIND_CHARGE_EXPLOSION_DAMAGE: f32 = 2.0;
const ENTITY_WITHER_SKULL_EXPLOSION_RADIUS: f64 = 1.0;
const ENTITY_WITHER_SKULL_EXPLOSION_DAMAGE: f32 = 8.0;
const ENTITY_WITCH_INSTANT_DAMAGE_AMPLIFIER: i32 = 0;
const ENTITY_WITCH_SLOWNESS_DURATION_TICKS: i32 = 20 * 30;
const ENTITY_GUARDIAN_ATTACK_DURATION_TICKS: i32 = 80;
const ENTITY_GUARDIAN_MAGIC_DAMAGE: f32 = 6.0;
const ENTITY_ELDER_GUARDIAN_MAGIC_DAMAGE: f32 = 8.0;
const ENTITY_SLIME_JUMP_VELOCITY: f64 = 0.42;
const ENTITY_SLIME_JUMP_DELAY_TICKS: i32 = 10;
const ENTITY_SPIDER_CLIMB_VELOCITY: f64 = 0.20;
const ENTITY_ENDERMAN_TELEPORT_MIN_DISTANCE: f64 = 16.0;
const ENTITY_ENDERMAN_TELEPORT_ARRIVAL_DISTANCE: f64 = 4.0;
const ENTITY_ENDERMAN_TELEPORT_CHANCE: f64 = 0.05;
const ENTITY_ENDERMAN_WATER_DAMAGE: f32 = 1.0;
const ENTITY_ENDERMAN_WATER_TELEPORT_RADIUS: i32 = 16;
const ENTITY_OUT_OF_WATER_DAMAGE: f32 = 1.0;
const ENTITY_WATER_CONTACT_DAMAGE: f32 = 1.0;
const ENTITY_FLYING_VERTICAL_SPEED: f64 = 0.18;
const ENTITY_SWIMMING_VERTICAL_SPEED: f64 = 0.12;
const ENTITY_EVOKER_FANGS_DAMAGE: f32 = 6.0;
const ENTITY_WARDEN_SONIC_BOOM_DAMAGE: f32 = 10.0;
const ENTITY_DEFAULT_SUMMON_LIFETIME_TICKS: i32 = 40;

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
    entity_fire: Mutex<HashMap<String, EntityFireState>>,
    entity_attacks: Mutex<HashMap<String, EntityAttackState>>,
    entity_creepers: Mutex<HashMap<String, EntityCreeperState>>,
    entity_projectiles: Mutex<HashMap<String, EntityProjectileState>>,
    entity_summons: Mutex<HashMap<String, EntitySummonState>>,
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
    jump_cooldown_ticks: i32,
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

#[derive(Debug, Clone, Copy)]
struct EntityFireState {
    remaining_ticks: i32,
    damage_cooldown_ticks: i32,
}

#[derive(Debug, Clone, Copy, Default)]
struct EntityAttackState {
    cooldown_ticks: i32,
    charge_ticks: i32,
    target_profile_id: Option<uuid::Uuid>,
}

#[derive(Debug, Clone, Copy, Default)]
struct EntityCreeperState {
    swell_ticks: i32,
}

#[derive(Debug, Clone, Copy)]
struct EntitySummonState {
    remaining_ticks: i32,
}

#[derive(Debug, Clone, Copy)]
struct EntityProjectileState {
    source_entity_id: i32,
    kind: EntityProjectileKind,
    damage: f32,
    damage_kind: crate::players::PlayerDamageKind,
    knockback: f32,
    gravity_per_tick: f64,
    hit_radius: f64,
    remaining_ticks: i32,
    explosion_radius: f64,
    explosion_damage: f32,
    explosion_break_blocks: bool,
    splash_radius: f64,
    potion_effect: Option<EntityPotionEffect>,
    target_profile_id: Option<uuid::Uuid>,
}

#[derive(Debug, Clone, Copy, Default)]
struct EntityMovement {
    x: f64,
    y: f64,
    z: f64,
}

#[derive(Debug, Clone)]
struct EntityPlayerDamageRequest {
    target_profile_id: uuid::Uuid,
    amount: f32,
    kind: crate::players::PlayerDamageKind,
    source_entity_id: i32,
    source_position: EntityPosition,
    knockback: f32,
}

#[derive(Debug, Clone)]
struct EntityPlayerPotionEffectRequest {
    target_profile_id: uuid::Uuid,
    effect: &'static str,
    amplifier: i32,
    duration_ticks: i32,
    source_entity_id: i32,
    source_position: EntityPosition,
    knockback: f32,
}

#[derive(Debug, Clone)]
struct EntityEntityDamageRequest {
    target_entity_id: i32,
    amount: f32,
}

#[derive(Debug, Clone)]
struct EntityProjectileSpawnRequest {
    key: String,
    entity_type: String,
    dimension: String,
    position: EntityPosition,
    motion: EntityMotion,
    source_entity_id: i32,
    kind: EntityProjectileKind,
    damage: f32,
    damage_kind: crate::players::PlayerDamageKind,
    knockback: f32,
    gravity_per_tick: f64,
    hit_radius: f64,
    lifetime_ticks: i32,
    explosion_radius: f64,
    explosion_damage: f32,
    explosion_break_blocks: bool,
    splash_radius: f64,
    potion_effect: Option<EntityPotionEffect>,
    target_profile_id: Option<uuid::Uuid>,
}

#[derive(Debug, Clone)]
struct EntitySummonSpawnRequest {
    key: String,
    entity_type: String,
    dimension: String,
    position: EntityPosition,
    ai: String,
    ai_params: BTreeMap<String, serde_json::Value>,
    display_name: String,
    lifetime_ticks: Option<i32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EntityProjectileKind {
    Arrow,
    SmallFireball,
    Fireball,
    Potion,
    ShulkerBullet,
    WindCharge,
    WitherSkull,
    Trident,
    Snowball,
}

impl EntityProjectileKind {
    fn ai(self) -> &'static str {
        match self {
            Self::Arrow => ENTITY_PROJECTILE_ARROW_AI,
            Self::SmallFireball => ENTITY_PROJECTILE_SMALL_FIREBALL_AI,
            Self::Fireball => ENTITY_PROJECTILE_FIREBALL_AI,
            Self::Potion => ENTITY_PROJECTILE_POTION_AI,
            Self::ShulkerBullet => ENTITY_PROJECTILE_SHULKER_BULLET_AI,
            Self::WindCharge => ENTITY_PROJECTILE_WIND_CHARGE_AI,
            Self::WitherSkull => ENTITY_PROJECTILE_WITHER_SKULL_AI,
            Self::Trident => ENTITY_PROJECTILE_TRIDENT_AI,
            Self::Snowball => ENTITY_PROJECTILE_SNOWBALL_AI,
        }
    }

    fn entity_type(self) -> &'static str {
        match self {
            Self::Arrow => "minecraft:arrow",
            Self::SmallFireball => "minecraft:small_fireball",
            Self::Fireball => "minecraft:fireball",
            Self::Potion => "minecraft:splash_potion",
            Self::ShulkerBullet => "minecraft:shulker_bullet",
            Self::WindCharge => "minecraft:wind_charge",
            Self::WitherSkull => "minecraft:wither_skull",
            Self::Trident => "minecraft:trident",
            Self::Snowball => "minecraft:snowball",
        }
    }

    fn damage_kind(self) -> crate::players::PlayerDamageKind {
        match self {
            Self::Fireball | Self::WindCharge | Self::WitherSkull => {
                crate::players::PlayerDamageKind::Explosion
            }
            Self::Potion => crate::players::PlayerDamageKind::Magic,
            Self::Arrow
            | Self::SmallFireball
            | Self::ShulkerBullet
            | Self::Trident
            | Self::Snowball => crate::players::PlayerDamageKind::Projectile,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct EntityPotionEffect {
    effect: EntityPotionEffectKind,
    amplifier: i32,
    duration_ticks: i32,
}

#[derive(Debug, Clone, Copy)]
enum EntityPotionEffectKind {
    InstantDamage,
    Hunger,
    Levitation,
    Poison,
    Slowness,
    Weakness,
    Wither,
}

impl EntityPotionEffectKind {
    fn name(self) -> &'static str {
        match self {
            Self::InstantDamage => "minecraft:instant_damage",
            Self::Hunger => "minecraft:hunger",
            Self::Levitation => "minecraft:levitation",
            Self::Poison => "minecraft:poison",
            Self::Slowness => "minecraft:slowness",
            Self::Weakness => "minecraft:weakness",
            Self::Wither => "minecraft:wither",
        }
    }

    fn default_duration_ticks(self) -> i32 {
        match self {
            Self::InstantDamage => 1,
            Self::Hunger => 20 * 7,
            Self::Levitation => ENTITY_SHULKER_BULLET_LEVITATION_TICKS,
            Self::Poison => 20 * 45,
            Self::Slowness => ENTITY_WITCH_SLOWNESS_DURATION_TICKS,
            Self::Weakness => 20 * 30,
            Self::Wither => 20 * 10,
        }
    }
}

#[derive(Debug, Clone)]
struct EntityExplosionBlockRequest {
    dimension: String,
    center: EntityPosition,
    radius: f64,
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
            entity_fire: Mutex::new(HashMap::new()),
            entity_attacks: Mutex::new(HashMap::new()),
            entity_creepers: Mutex::new(HashMap::new()),
            entity_projectiles: Mutex::new(HashMap::new()),
            entity_summons: Mutex::new(HashMap::new()),
            collision_cache: Mutex::new(CollisionCache::default()),
            ai_cursor: Mutex::new(0),
        };

        if !config.enable {
            return Ok(manager);
        }

        for (index, entity) in config.list.iter().enumerate() {
            if entity.kind == qexed_config::app::qexed::server::EntityKind::Entity
                && Self::entity_type_disabled(config, configured_entity_type(entity))
            {
                log::debug!(
                    "skip disabled configured entity: id={}, entity_type={}",
                    entity.id,
                    configured_entity_type(entity)
                );
                continue;
            }
            manager.spawn_configured(index, &config.dimension, entity, config)?;
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

    pub fn entity_type_disabled(
        config: &qexed_config::app::qexed::server::Entities,
        entity_type: &str,
    ) -> bool {
        entity_type_disabled(&config.disabled_entity_types, entity_type)
    }

    pub fn apply_entity_ai_overrides(
        config: &qexed_config::app::qexed::server::Entities,
        entity_type: &str,
        ai: &mut String,
        ai_params: &mut BTreeMap<String, serde_json::Value>,
        auto_jump: &mut bool,
    ) {
        let Some(override_config) = entity_ai_override(config, entity_type) else {
            return;
        };
        if !override_config.ai.trim().is_empty() {
            *ai = override_config.ai.trim().to_string();
        }
        ai_params.extend(override_config.ai_params.clone());
        if let Some(value) = override_config.auto_jump {
            *auto_jump = value;
        }
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

    fn send_entity_event_to_rendered_viewers(
        &self,
        players: &crate::players::PlayerManager,
        rendering: &qexed_config::app::qexed::server::EntityRendering,
        entity: &ManagedEntity,
        event_id: u8,
    ) -> Result<()> {
        let packet =
            qexed_tcp_connect::PacketSink::<tokio::io::Sink>::build_send_packet(EntityEvent {
                entity_id: entity.entity_id,
                event_id,
            })?;
        for player in players.list_except(uuid::Uuid::nil()) {
            if entity_visible_to_player(entity, &player, rendering) {
                players.send_packets_to(player.profile.uuid, vec![packet.clone()]);
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
        let data = normalized_entity_data(&entity_type, request.data, &request.ai_params);
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
            data,
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
            .or_insert_with(|| default_entity_health_for_entity(&entity));
        Ok(entity)
    }

    fn spawn_projectile(
        &self,
        players: &crate::players::PlayerManager,
        rendering: &qexed_config::app::qexed::server::EntityRendering,
        request: EntityProjectileSpawnRequest,
    ) -> Result<()> {
        let entity = self.spawn_local(EntitySpawnRequest {
            key: request.key,
            kind: ManagedEntityKind::Entity,
            entity_type: request.entity_type,
            entity_type_id_override: None,
            dimension: request.dimension,
            position: request.position,
            name: "Projectile".to_string(),
            display_name: String::new(),
            skin_textures: String::new(),
            skin_signature: String::new(),
            data: 0,
            ai: request.kind.ai().to_string(),
            ai_params: BTreeMap::new(),
            auto_jump: false,
            spawn_rule: String::new(),
            custom_type: String::new(),
            look_at_players: false,
            main_hand_event: "interact".to_string(),
            off_hand_event: "interact_off_hand".to_string(),
            attack_event: "attack".to_string(),
        })?;
        self.entity_motion
            .lock()
            .expect("entity motion state poisoned")
            .insert(entity.key.clone(), request.motion);
        self.entity_projectiles
            .lock()
            .expect("entity projectile state poisoned")
            .insert(
                entity.key.clone(),
                EntityProjectileState {
                    source_entity_id: request.source_entity_id,
                    kind: request.kind,
                    damage: request.damage,
                    damage_kind: request.damage_kind,
                    knockback: request.knockback,
                    gravity_per_tick: request.gravity_per_tick,
                    hit_radius: request.hit_radius,
                    remaining_ticks: request.lifetime_ticks,
                    explosion_radius: request.explosion_radius,
                    explosion_damage: request.explosion_damage,
                    explosion_break_blocks: request.explosion_break_blocks,
                    splash_radius: request.splash_radius,
                    potion_effect: request.potion_effect,
                    target_profile_id: request.target_profile_id,
                },
            );
        self.send_spawn_to_rendered_viewers(players, rendering, &entity)?;

        let packets = entity.position_packets_with_velocity(
            request.motion.velocity_x,
            request.motion.velocity_y,
            request.motion.velocity_z,
        )?;
        for player in players.list_except(uuid::Uuid::nil()) {
            if entity_visible_to_player(&entity, &player, rendering) {
                players.send_packets_to(player.profile.uuid, packets.clone());
            }
        }
        Ok(())
    }

    fn spawn_summoned_entity(
        &self,
        players: &crate::players::PlayerManager,
        rendering: &qexed_config::app::qexed::server::EntityRendering,
        request: EntitySummonSpawnRequest,
    ) -> Result<()> {
        let mut ai_params = request.ai_params;
        if let Some(lifetime_ticks) = request.lifetime_ticks {
            ai_params.insert(
                "summon_lifetime_ticks".to_string(),
                serde_json::json!(lifetime_ticks),
            );
        }
        let entity = self.spawn_local(EntitySpawnRequest {
            key: request.key,
            kind: ManagedEntityKind::Entity,
            entity_type: request.entity_type,
            entity_type_id_override: None,
            dimension: request.dimension,
            position: request.position,
            name: request.display_name.clone(),
            display_name: request.display_name,
            skin_textures: String::new(),
            skin_signature: String::new(),
            data: 0,
            ai: request.ai,
            ai_params,
            auto_jump: true,
            spawn_rule: String::new(),
            custom_type: String::new(),
            look_at_players: false,
            main_hand_event: "interact".to_string(),
            off_hand_event: "interact_off_hand".to_string(),
            attack_event: "attack".to_string(),
        })?;
        if let Some(lifetime_ticks) = request.lifetime_ticks {
            self.entity_summons
                .lock()
                .expect("entity summon state poisoned")
                .insert(
                    entity.key.clone(),
                    EntitySummonState {
                        remaining_ticks: lifetime_ticks.max(1),
                    },
                );
        }
        self.send_spawn_to_rendered_viewers(players, rendering, &entity)?;
        Ok(())
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
        self.entity_fire
            .lock()
            .expect("entity fire state poisoned")
            .remove(&entity.key);
        self.entity_attacks
            .lock()
            .expect("entity attack state poisoned")
            .remove(&entity.key);
        self.entity_creepers
            .lock()
            .expect("entity creeper state poisoned")
            .remove(&entity.key);
        self.entity_projectiles
            .lock()
            .expect("entity projectile state poisoned")
            .remove(&entity.key);
        self.entity_summons
            .lock()
            .expect("entity summon state poisoned")
            .remove(&entity.key);
        Ok(entity)
    }

    pub fn damage_managed_entity(
        &self,
        players: &crate::players::PlayerManager,
        rendering: &qexed_config::app::qexed::server::EntityRendering,
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
            .or_insert_with(|| default_entity_health_for_entity(&entity));
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
            self.spawn_slime_split_children(players, rendering, &entity)?;
        }

        Ok(Some(EntityDamageResult { entity, killed }))
    }

    fn spawn_slime_split_children(
        &self,
        players: &crate::players::PlayerManager,
        rendering: &qexed_config::app::qexed::server::EntityRendering,
        entity: &ManagedEntity,
    ) -> Result<()> {
        if !entity_type_uses_slime_size(&entity.entity_type)
            || ai_param_bool(&entity.ai_params, "disable_slime_split", false)
        {
            return Ok(());
        }
        let size = slime_size_for_entity(entity);
        if size <= 1 {
            return Ok(());
        }
        let child_size = (size / 2).max(1);
        let random_count = rand::Rng::gen_range(&mut rand::thread_rng(), 2..=4);
        let split_count =
            ai_param_i32(&entity.ai_params, "slime_split_count", random_count).clamp(0, 8);
        let offset = (f64::from(size) * 0.255).max(0.25);
        for index in 0..split_count {
            let x_offset = f64::from(index % 2) * offset - offset * 0.5;
            let z_offset = f64::from(index / 2) * offset - offset * 0.5;
            let mut ai_params = entity.ai_params.clone();
            ai_params.insert("slime_size".to_string(), serde_json::json!(child_size));
            let mut position = entity.position;
            position.x += x_offset;
            position.y += 0.5;
            position.z += z_offset;
            position.yaw = rand::Rng::gen_range(&mut rand::thread_rng(), -180.0..180.0);
            position.pitch = 0.0;
            position.on_ground = false;
            let child = self.spawn_local(EntitySpawnRequest {
                key: format!("{}:split:{}", entity.key, self.next_spawn_sequence()),
                kind: ManagedEntityKind::Entity,
                entity_type: entity.entity_type.clone(),
                entity_type_id_override: Some(entity.entity_type_id),
                dimension: entity.dimension.clone(),
                position,
                name: entity.name.clone(),
                display_name: entity.display_name.clone(),
                skin_textures: entity.skin_textures.clone(),
                skin_signature: entity.skin_signature.clone(),
                data: child_size,
                ai: entity.ai.clone(),
                ai_params,
                auto_jump: entity.auto_jump,
                spawn_rule: entity.spawn_rule.clone(),
                custom_type: entity.custom_type.clone(),
                look_at_players: entity.look_at_players,
                main_hand_event: entity.main_hand_event.clone(),
                off_hand_event: entity.off_hand_event.clone(),
                attack_event: entity.attack_event.clone(),
            })?;
            self.send_spawn_to_rendered_viewers(players, rendering, &child)?;
        }
        Ok(())
    }

    pub fn ignite_managed_entity(
        &self,
        players: &crate::players::PlayerManager,
        rendering: &qexed_config::app::qexed::server::EntityRendering,
        entity_id: i32,
        fire_ticks: i32,
    ) -> Result<bool> {
        if fire_ticks <= 0 {
            return Ok(false);
        }
        let Some(entity) = self.entity_by_runtime_id(entity_id) else {
            return Ok(false);
        };
        if entity.kind != ManagedEntityKind::Entity {
            return Ok(false);
        }
        let inserted = ignite_entity_state(
            &mut self.entity_fire.lock().expect("entity fire state poisoned"),
            &entity.key,
            fire_ticks,
        );
        if inserted {
            self.send_entity_event_to_rendered_viewers(
                players,
                rendering,
                &entity,
                ENTITY_IGNITE_EVENT_ID,
            )?;
        }
        Ok(true)
    }

    #[cfg(test)]
    pub(crate) fn entity_health_for_tests(&self, key: &str) -> Option<f32> {
        self.entity_health
            .lock()
            .expect("entity health state poisoned")
            .get(key)
            .copied()
    }

    #[cfg(test)]
    pub(crate) fn entity_fire_ticks_for_tests(&self, key: &str) -> Option<i32> {
        self.entity_fire
            .lock()
            .expect("entity fire state poisoned")
            .get(key)
            .map(|state| state.remaining_ticks)
    }

    fn spawn_configured(
        &self,
        index: usize,
        dimension: &str,
        config: &qexed_config::app::qexed::server::Entity,
        entities_config: &qexed_config::app::qexed::server::Entities,
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
        let mut ai = config.ai.clone();
        let mut ai_params = config.ai_params.clone();
        let mut auto_jump = config.auto_jump;
        if kind == ManagedEntityKind::Entity {
            Self::apply_entity_ai_overrides(
                entities_config,
                &entity_type,
                &mut ai,
                &mut ai_params,
                &mut auto_jump,
            );
        }
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
            ai,
            ai_params,
            auto_jump,
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

    pub(crate) fn managed_entity_view_packets(
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
        self.spawn_from_rules_with_entity_config(
            players,
            world,
            rendering,
            spawning,
            default_dimension,
            None,
        )
    }

    pub fn spawn_from_rules_with_entity_config(
        &self,
        players: &crate::players::PlayerManager,
        world: &crate::world::WorldManager,
        rendering: &qexed_config::app::qexed::server::EntityRendering,
        spawning: &qexed_config::app::qexed::server::EntitySpawning,
        default_dimension: &str,
        entities_config: Option<&qexed_config::app::qexed::server::Entities>,
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
            if let Some(entities_config) = entities_config {
                let shell_entity_type = custom
                    .as_ref()
                    .map(|registration| registration.shell_entity_type.as_str())
                    .unwrap_or(rule.entity_type.as_str());
                if Self::entity_type_disabled(entities_config, &rule.entity_type)
                    || Self::entity_type_disabled(entities_config, shell_entity_type)
                {
                    continue;
                }
            }
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

            let spawn_entity_type = custom
                .as_ref()
                .map(|registration| registration.shell_entity_type.as_str())
                .unwrap_or(rule.entity_type.as_str());
            let Some(position) = self.spawn_position_for_rule(
                rule,
                dimension,
                world,
                &spawning.slime_chunks,
                spawn_entity_type,
            ) else {
                continue;
            };

            let mut spawn = spawn_request_from_rule(
                rule,
                &rule_id,
                dimension,
                self.next_spawn_sequence(),
                custom,
                position,
            )?;
            if let Some(entities_config) = entities_config {
                Self::apply_entity_ai_overrides(
                    entities_config,
                    &spawn.entity_type,
                    &mut spawn.ai,
                    &mut spawn.ai_params,
                    &mut spawn.auto_jump,
                );
            }

            let entity = self.spawn_local(spawn)?;
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
        self.tick_ai_with_world_rules(players, world, plugins, None, rendering, tick_ms)
    }

    pub fn tick_ai_with_world_rules(
        &self,
        players: &crate::players::PlayerManager,
        world: &crate::world::WorldManager,
        plugins: &crate::plugins::PluginManager,
        world_rules: Option<&crate::world::WorldRulesManager>,
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
        let mut fire_events = Vec::new();
        let mut fire_damage = Vec::new();
        let mut environment_damage = Vec::new();
        let mut player_damage_requests = Vec::new();
        let mut player_potion_effect_requests = Vec::new();
        let mut entity_damage_requests = Vec::new();
        let mut projectile_spawns = Vec::new();
        let mut summon_spawns = Vec::new();
        let mut explosion_block_requests = Vec::new();
        let dying_keys = self
            .entity_deaths
            .lock()
            .expect("entity death state poisoned")
            .keys()
            .cloned()
            .collect::<HashSet<_>>();
        let snapshot = self.active_ai_snapshot(&viewers, rendering, &dying_keys);
        let target_snapshot = snapshot.clone();
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
            let mut fire_state = self
                .entity_fire
                .lock()
                .expect("entity fire state poisoned")
                .clone();
            let mut attack_state = self
                .entity_attacks
                .lock()
                .expect("entity attack state poisoned")
                .clone();
            let mut creeper_state = self
                .entity_creepers
                .lock()
                .expect("entity creeper state poisoned")
                .clone();
            let mut projectile_state = self
                .entity_projectiles
                .lock()
                .expect("entity projectile state poisoned")
                .clone();
            let mut summon_state = self
                .entity_summons
                .lock()
                .expect("entity summon state poisoned")
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
                let mut remove = apply_summon_lifetime_tick(&entity, tick_ms, &mut summon_state);
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
                    EntityAiKind::Vanilla => {
                        movement = apply_vanilla_ai(
                            &mut entity,
                            &viewers,
                            &target_snapshot,
                            tick_ms,
                            &mut target_memory,
                            &mut path_memory,
                            &mut target_reselects,
                            &mut path_recalcs,
                            world,
                            &mut collision_cache,
                            now,
                            rendering.default_distance,
                        );
                    }
                    EntityAiKind::Projectile => {
                        target_memory.remove(&entity.key);
                        path_memory.remove(&entity.key);
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

                if ai == EntityAiKind::Vanilla {
                    remove |= apply_vanilla_ai_effects(
                        &entity,
                        &viewers,
                        &target_snapshot,
                        tick_ms,
                        &mut attack_state,
                        &mut creeper_state,
                        &mut player_damage_requests,
                        &mut player_potion_effect_requests,
                        &mut entity_damage_requests,
                        &mut projectile_spawns,
                        &mut summon_spawns,
                        &mut explosion_block_requests,
                    );
                } else {
                    attack_state.remove(&entity.key);
                    creeper_state.remove(&entity.key);
                }

                if remove {
                    target_memory.remove(&entity.key);
                    path_memory.remove(&entity.key);
                    plugin_ai_memory.remove(&entity.key);
                    attack_state.remove(&entity.key);
                    creeper_state.remove(&entity.key);
                    projectile_state.remove(&entity.key);
                    summon_state.remove(&entity.key);
                    removes.push(entity.key.clone());
                    continue;
                }

                let next_motion = if ai == EntityAiKind::Projectile {
                    let motion = motion.entry(entity.key.clone()).or_default();
                    remove |= apply_projectile_tick(
                        &mut entity,
                        world,
                        &mut collision_cache,
                        motion,
                        &mut projectile_state,
                        &viewers,
                        &target_snapshot,
                        tick_ms,
                        &mut player_damage_requests,
                        &mut player_potion_effect_requests,
                        &mut entity_damage_requests,
                        &mut explosion_block_requests,
                    );
                    let next_motion = *motion;
                    motion_updates.push(EntityMotionTickUpdate {
                        key: entity.key.clone(),
                        previous,
                        next: next_motion,
                    });
                    Some(next_motion)
                } else if should_apply_entity_physics(&entity, ai) {
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

                if remove {
                    target_memory.remove(&entity.key);
                    path_memory.remove(&entity.key);
                    plugin_ai_memory.remove(&entity.key);
                    attack_state.remove(&entity.key);
                    creeper_state.remove(&entity.key);
                    projectile_state.remove(&entity.key);
                    summon_state.remove(&entity.key);
                    removes.push(entity.key.clone());
                    continue;
                }

                apply_entity_fire_tick(
                    &entity,
                    world,
                    world_rules,
                    &mut fire_state,
                    tick_ms,
                    &mut fire_events,
                    &mut fire_damage,
                );
                if apply_enderman_water_reaction(
                    &mut entity,
                    world,
                    &mut collision_cache,
                    tick_ms,
                    &mut environment_damage,
                ) {
                    target_memory.remove(&entity.key);
                    path_memory.remove(&entity.key);
                }
                apply_water_contact_damage(&entity, world, tick_ms, &mut environment_damage);
                apply_out_of_water_damage(&entity, world, tick_ms, &mut environment_damage);

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
            *self.entity_fire.lock().expect("entity fire state poisoned") = fire_state;
            *self
                .entity_attacks
                .lock()
                .expect("entity attack state poisoned") = attack_state;
            *self
                .entity_creepers
                .lock()
                .expect("entity creeper state poisoned") = creeper_state;
            *self
                .entity_projectiles
                .lock()
                .expect("entity projectile state poisoned") = projectile_state;
            *self
                .entity_summons
                .lock()
                .expect("entity summon state poisoned") = summon_state;
        }

        for entity in fire_events {
            self.send_entity_event_to_rendered_viewers(
                players,
                rendering,
                &entity,
                ENTITY_IGNITE_EVENT_ID,
            )?;
        }
        for entity_id in fire_damage {
            let _ =
                self.damage_managed_entity(players, rendering, entity_id, ENTITY_FIRE_DAMAGE)?;
        }
        for (entity_id, amount) in environment_damage {
            let _ = self.damage_managed_entity(players, rendering, entity_id, amount)?;
        }
        for request in entity_damage_requests {
            let _ = self.damage_managed_entity(
                players,
                rendering,
                request.target_entity_id,
                request.amount,
            )?;
        }
        for request in player_damage_requests {
            players.damage_player(
                request.target_profile_id,
                request.amount,
                request.kind,
                request.source_entity_id,
                request.source_position,
                request.knockback,
            );
        }
        for request in player_potion_effect_requests {
            players.apply_potion_effect(
                request.target_profile_id,
                request.effect,
                request.amplifier,
                request.duration_ticks,
                request.source_entity_id,
                request.source_position,
                request.knockback,
            );
        }
        for request in projectile_spawns {
            self.spawn_projectile(players, rendering, request)?;
        }
        for request in summon_spawns {
            self.spawn_summoned_entity(players, rendering, request)?;
        }
        apply_explosion_block_requests(players, world, world_rules, explosion_block_requests)?;

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
        slime_chunks: &qexed_config::app::qexed::server::SlimeChunkSpawning,
        entity_type: &str,
    ) -> Option<EntityPosition> {
        let restrict_to_slime_chunks = slime_chunk_spawning_applies(slime_chunks, entity_type);
        let attempts = if rule.require_air || rule.require_ground || restrict_to_slime_chunks {
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
            if restrict_to_slime_chunks && !slime_chunk_allows_position(slime_chunks, position) {
                continue;
            }
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
            game_mode: 0,
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
        "minecraft:evoker" => 24.0,
        "minecraft:vex" => 14.0,
        "minecraft:guardian" => 30.0,
        "minecraft:elder_guardian" => 80.0,
        "minecraft:polar_bear" => 30.0,
        "minecraft:turtle" => 30.0,
        "minecraft:dolphin" => 10.0,
        "minecraft:axolotl" => 14.0,
        "minecraft:bee" => 10.0,
        "minecraft:snow_golem" => 4.0,
        "minecraft:slime" | "minecraft:magma_cube" => 16.0,
        "minecraft:chicken"
        | "minecraft:rabbit"
        | "minecraft:bat"
        | "minecraft:allay"
        | "minecraft:parrot"
        | "minecraft:cod"
        | "minecraft:salmon"
        | "minecraft:tropical_fish"
        | "minecraft:pufferfish"
        | "minecraft:tadpole" => 6.0,
        "minecraft:sheep"
        | "minecraft:pig"
        | "minecraft:cow"
        | "minecraft:goat"
        | "minecraft:wolf"
        | "minecraft:cat"
        | "minecraft:ocelot"
        | "minecraft:fox"
        | "minecraft:frog"
        | "minecraft:armadillo" => 10.0,
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

fn default_entity_health_for_entity(entity: &ManagedEntity) -> f32 {
    if entity_type_uses_slime_size(&entity.entity_type) {
        let size = slime_size_for_entity(entity);
        return (size * size) as f32;
    }
    default_entity_health(&entity.entity_type)
}

fn normalized_entity_data(
    entity_type: &str,
    data: i32,
    ai_params: &BTreeMap<String, serde_json::Value>,
) -> i32 {
    if entity_type_uses_slime_size(entity_type) {
        return slime_size_from_params(data, ai_params);
    }
    data
}

fn slime_size_for_entity(entity: &ManagedEntity) -> i32 {
    slime_size_from_params(entity.data, &entity.ai_params)
}

fn slime_size_from_params(data: i32, ai_params: &BTreeMap<String, serde_json::Value>) -> i32 {
    let configured = ai_param_i32(
        ai_params,
        "slime_size",
        ai_param_i32(ai_params, "size", data.max(0)),
    );
    if configured > 0 {
        configured.clamp(1, 127)
    } else {
        4
    }
}

fn entity_type_uses_slime_size(entity_type: &str) -> bool {
    matches!(
        normalized_entity_type(entity_type).as_str(),
        "minecraft:slime" | "minecraft:magma_cube"
    )
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

fn rotation_from_velocity(velocity_x: f64, velocity_y: f64, velocity_z: f64) -> (f32, f32) {
    let horizontal = (velocity_x * velocity_x + velocity_z * velocity_z)
        .sqrt()
        .max(0.0001);
    let yaw = (velocity_z.atan2(velocity_x).to_degrees() - 90.0) as f32;
    let pitch = (-velocity_y.atan2(horizontal).to_degrees()) as f32;
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

fn configured_entity_type(config: &qexed_config::app::qexed::server::Entity) -> &str {
    let entity_type = config.entity_type.trim();
    if entity_type.is_empty() {
        "minecraft:armor_stand"
    } else {
        entity_type
    }
}

fn entity_type_disabled(disabled: &[String], entity_type: &str) -> bool {
    let entity_type = normalized_entity_type(entity_type);
    disabled.iter().any(|disabled_type| {
        let disabled_type = disabled_type.trim();
        disabled_type == "*" || normalized_entity_type(disabled_type) == entity_type
    })
}

fn entity_ai_override<'a>(
    config: &'a qexed_config::app::qexed::server::Entities,
    entity_type: &str,
) -> Option<&'a qexed_config::app::qexed::server::EntityAiOverride> {
    let entity_type = normalized_entity_type(entity_type);
    config
        .ai_overrides
        .iter()
        .find(|override_config| normalized_entity_type(&override_config.entity_type) == entity_type)
}

fn normalized_entity_type(entity_type: &str) -> String {
    let entity_type = entity_type.trim();
    if entity_type.is_empty() {
        return String::new();
    }
    if entity_type.contains(':') {
        entity_type.to_string()
    } else {
        format!("minecraft:{entity_type}")
    }
}

fn ai_param_f64(params: &BTreeMap<String, serde_json::Value>, key: &str, fallback: f64) -> f64 {
    params
        .get(key)
        .and_then(|value| match value {
            serde_json::Value::Number(number) => number.as_f64(),
            serde_json::Value::String(value) => value.trim().parse().ok(),
            _ => None,
        })
        .filter(|value| value.is_finite())
        .unwrap_or(fallback)
}

fn ai_param_i32(params: &BTreeMap<String, serde_json::Value>, key: &str, fallback: i32) -> i32 {
    params
        .get(key)
        .and_then(|value| match value {
            serde_json::Value::Number(number) => number.as_i64(),
            serde_json::Value::String(value) => value.trim().parse().ok(),
            _ => None,
        })
        .and_then(|value| i32::try_from(value).ok())
        .unwrap_or(fallback)
}

fn ai_param_bool(params: &BTreeMap<String, serde_json::Value>, key: &str, fallback: bool) -> bool {
    params
        .get(key)
        .and_then(|value| match value {
            serde_json::Value::Bool(value) => Some(*value),
            serde_json::Value::String(value) => match value.trim() {
                "true" | "1" | "yes" | "on" => Some(true),
                "false" | "0" | "no" | "off" => Some(false),
                _ => None,
            },
            _ => None,
        })
        .unwrap_or(fallback)
}

fn ai_param_string(params: &BTreeMap<String, serde_json::Value>, key: &str) -> Option<String> {
    params.get(key).and_then(|value| match value {
        serde_json::Value::String(value) => Some(value.trim().to_string()),
        _ => None,
    })
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
    Vanilla,
    Projectile,
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
        "vanilla" | "minecraft:vanilla" => EntityAiKind::Vanilla,
        value if value.starts_with("vanilla:") || value.starts_with("minecraft:vanilla:") => {
            EntityAiKind::Vanilla
        }
        value if value.starts_with("vanilla_projectile:") => EntityAiKind::Projectile,
        value if value.starts_with("plugin:") => EntityAiKind::Plugin,
        _ => EntityAiKind::None,
    }
}

fn should_apply_entity_physics(entity: &ManagedEntity, ai: EntityAiKind) -> bool {
    ai != EntityAiKind::None || !entity.spawn_rule.is_empty()
}

fn vanilla_behavior_override(value: &str) -> Option<VanillaEntityBehavior> {
    match value.trim() {
        "none" | "static" => Some(VanillaEntityBehavior::Static),
        "passive" | "ambient" | "animal" => Some(VanillaEntityBehavior::Passive),
        "neutral" | "neutral_melee" => Some(VanillaEntityBehavior::NeutralMelee),
        "neutral_ranged" => Some(VanillaEntityBehavior::NeutralRanged),
        "hostile" | "hostile_melee" | "melee" => Some(VanillaEntityBehavior::HostileMelee),
        "hostile_ranged" | "ranged" => Some(VanillaEntityBehavior::HostileRanged),
        "spellcaster" | "caster" => Some(VanillaEntityBehavior::Spellcaster),
        "guardian" | "guardian_beam" => Some(VanillaEntityBehavior::GuardianBeam),
        "creeper" => Some(VanillaEntityBehavior::Creeper),
        _ => None,
    }
}

fn vanilla_entity_ai_profile(entity_type: &str) -> VanillaEntityAiProfile {
    let entity_type = normalized_entity_type(entity_type);
    let mut profile = VanillaEntityAiProfile {
        behavior: VanillaEntityBehavior::Passive,
        movement_mode: VanillaMovementMode::Ground,
        movement_speed: 0.20,
        vertical_speed: 0.0,
        follow_range: 16.0,
        stop_distance: 2.0,
        stroll_chance: 0.25,
        stroll_min_speed: 0.06,
        stroll_max_speed: 0.14,
        look_range: 6.0,
        attack_damage: 2.0,
        attack_range: 2.0,
        attack_interval_ticks: ENTITY_MELEE_ATTACK_INTERVAL_TICKS,
        attack_knockback: 0.35,
        melee_effect: None,
        melee_effect_chance: 1.0,
        ranged_attack_damage: ENTITY_ARROW_DEFAULT_DAMAGE,
        ranged_attack_range: 15.0,
        ranged_attack_interval_ticks: 40,
        projectile_speed: ENTITY_ARROW_DEFAULT_SPEED,
        projectile_gravity_per_tick: ENTITY_PROJECTILE_GRAVITY_PER_TICK,
        projectile_hit_radius: ENTITY_PROJECTILE_HIT_RADIUS,
        projectile_lifetime_ticks: ENTITY_PROJECTILE_LIFETIME_TICKS,
        projectile_explosion_radius: 0.0,
        projectile_explosion_damage: 0.0,
        projectile_break_blocks: true,
        projectile_splash_radius: 0.0,
        spell_damage: 0.0,
        summon_kind: None,
        summon_count: 0,
        summon_chance: 0.0,
        summon_lifetime_ticks: ENTITY_DEFAULT_SUMMON_LIFETIME_TICKS,
        guardian_attack_duration_ticks: ENTITY_GUARDIAN_ATTACK_DURATION_TICKS,
        guardian_magic_damage: ENTITY_GUARDIAN_MAGIC_DAMAGE,
        slime_jump_velocity: ENTITY_SLIME_JUMP_VELOCITY,
        slime_jump_delay_ticks: ENTITY_SLIME_JUMP_DELAY_TICKS,
        spider_climb_velocity: ENTITY_SPIDER_CLIMB_VELOCITY,
        enderman_teleport_min_distance: ENTITY_ENDERMAN_TELEPORT_MIN_DISTANCE,
        enderman_teleport_arrival_distance: ENTITY_ENDERMAN_TELEPORT_ARRIVAL_DISTANCE,
        enderman_teleport_chance: ENTITY_ENDERMAN_TELEPORT_CHANCE,
        creeper_swell_ticks: CREEPER_SWELL_TICKS,
        creeper_explosion_radius: CREEPER_EXPLOSION_RADIUS,
        creeper_explosion_damage: CREEPER_EXPLOSION_DAMAGE,
        creeper_break_blocks: true,
    };

    match entity_type.as_str() {
        "minecraft:armor_stand"
        | "minecraft:area_effect_cloud"
        | "minecraft:block_display"
        | "minecraft:boat"
        | "minecraft:chest_boat"
        | "minecraft:command_block_minecart"
        | "minecraft:dragon_fireball"
        | "minecraft:egg"
        | "minecraft:end_crystal"
        | "minecraft:ender_pearl"
        | "minecraft:evoker_fangs"
        | "minecraft:experience_bottle"
        | "minecraft:experience_orb"
        | "minecraft:eye_of_ender"
        | "minecraft:falling_block"
        | "minecraft:fireball"
        | "minecraft:firework_rocket"
        | "minecraft:fishing_bobber"
        | "minecraft:glow_item_frame"
        | "minecraft:interaction"
        | "minecraft:item"
        | "minecraft:item_display"
        | "minecraft:item_frame"
        | "minecraft:leash_knot"
        | "minecraft:lingering_potion"
        | "minecraft:llama_spit"
        | "minecraft:marker"
        | "minecraft:minecart"
        | "minecraft:painting"
        | "minecraft:player"
        | "minecraft:potion"
        | "minecraft:shulker_bullet"
        | "minecraft:small_fireball"
        | "minecraft:snowball"
        | "minecraft:spectral_arrow"
        | "minecraft:splash_potion"
        | "minecraft:text_display"
        | "minecraft:tnt"
        | "minecraft:tnt_minecart"
        | "minecraft:trident"
        | "minecraft:wind_charge"
        | "minecraft:wither_skull" => {
            profile.behavior = VanillaEntityBehavior::Static;
        }
        "minecraft:pig" | "minecraft:armadillo" => {
            profile.movement_speed = 0.25;
        }
        "minecraft:sheep" => {
            profile.movement_speed = 0.23;
        }
        "minecraft:chicken" | "minecraft:frog" => {
            profile.movement_speed = 0.25;
            profile.stroll_max_speed = 0.16;
        }
        "minecraft:cow" | "minecraft:mooshroom" => {
            profile.movement_speed = 0.20;
        }
        "minecraft:cod"
        | "minecraft:salmon"
        | "minecraft:tropical_fish"
        | "minecraft:pufferfish"
        | "minecraft:squid"
        | "minecraft:glow_squid"
        | "minecraft:tadpole" => {
            profile.movement_mode = VanillaMovementMode::Swimming;
            profile.movement_speed = 0.18;
            profile.vertical_speed = ENTITY_SWIMMING_VERTICAL_SPEED;
            profile.stroll_max_speed = 0.14;
        }
        "minecraft:dolphin" | "minecraft:axolotl" | "minecraft:turtle" => {
            profile.movement_mode = VanillaMovementMode::Swimming;
            profile.movement_speed = 0.24;
            profile.vertical_speed = ENTITY_SWIMMING_VERTICAL_SPEED;
            profile.stroll_max_speed = 0.18;
        }
        "minecraft:allay" | "minecraft:bat" | "minecraft:parrot" => {
            profile.movement_mode = VanillaMovementMode::Flying;
            profile.movement_speed = 0.22;
            profile.vertical_speed = ENTITY_FLYING_VERTICAL_SPEED;
            profile.stroll_max_speed = 0.18;
        }
        "minecraft:bee" => {
            profile.behavior = VanillaEntityBehavior::NeutralMelee;
            profile.movement_mode = VanillaMovementMode::Flying;
            profile.movement_speed = 0.24;
            profile.vertical_speed = ENTITY_FLYING_VERTICAL_SPEED;
            profile.follow_range = 22.0;
            profile.stop_distance = 1.4;
            profile.attack_damage = 2.0;
            profile.attack_range = 1.6;
            profile.melee_effect = Some(EntityPotionEffect {
                effect: EntityPotionEffectKind::Poison,
                amplifier: 0,
                duration_ticks: 20 * 10,
            });
        }
        "minecraft:rabbit" => {
            profile.movement_speed = 0.30;
            profile.stroll_max_speed = 0.20;
        }
        "minecraft:horse"
        | "minecraft:donkey"
        | "minecraft:mule"
        | "minecraft:skeleton_horse"
        | "minecraft:zombie_horse"
        | "minecraft:camel" => {
            profile.movement_speed = 0.22;
            profile.stroll_max_speed = 0.17;
        }
        "minecraft:fox" => {
            profile.behavior = VanillaEntityBehavior::NeutralMelee;
            profile.movement_speed = 0.30;
            profile.stroll_max_speed = 0.20;
            profile.follow_range = 16.0;
            profile.stop_distance = 1.4;
            profile.attack_damage = 2.0;
            profile.attack_range = 1.6;
        }
        "minecraft:ocelot" | "minecraft:cat" => {
            profile.movement_speed = 0.30;
            profile.stroll_max_speed = 0.20;
        }
        "minecraft:wolf" => {
            profile.behavior = VanillaEntityBehavior::NeutralMelee;
            profile.movement_speed = 0.30;
            profile.follow_range = 24.0;
            profile.stop_distance = 1.6;
            profile.attack_damage = 4.0;
            profile.attack_range = 1.8;
        }
        "minecraft:polar_bear" => {
            profile.behavior = VanillaEntityBehavior::NeutralMelee;
            profile.movement_speed = 0.25;
            profile.follow_range = 32.0;
            profile.stop_distance = 2.0;
            profile.attack_damage = 6.0;
            profile.attack_range = 2.2;
        }
        "minecraft:iron_golem" => {
            profile.behavior = VanillaEntityBehavior::NeutralMelee;
            profile.movement_speed = 0.25;
            profile.follow_range = 32.0;
            profile.stop_distance = 2.0;
            profile.attack_damage = 15.0;
            profile.attack_range = 2.4;
            profile.attack_knockback = 0.9;
        }
        "minecraft:snow_golem" => {
            profile.behavior = VanillaEntityBehavior::NeutralRanged;
            profile.movement_speed = 0.20;
            profile.follow_range = 16.0;
            profile.stop_distance = 8.0;
            profile.ranged_attack_range = 10.0;
            profile.ranged_attack_damage = 0.0;
            profile.projectile_speed = 1.5;
            profile.projectile_hit_radius = 0.5;
        }
        "minecraft:villager" | "minecraft:wandering_trader" => {
            profile.movement_speed = 0.20;
            profile.look_range = 8.0;
        }
        "minecraft:creeper" => {
            profile.behavior = VanillaEntityBehavior::Creeper;
            profile.movement_speed = 0.25;
            profile.follow_range = 16.0;
            profile.stop_distance = 1.6;
        }
        "minecraft:zombie" | "minecraft:zombie_villager" => {
            profile.behavior = VanillaEntityBehavior::HostileMelee;
            profile.movement_speed = 0.23;
            profile.follow_range = 35.0;
            profile.stop_distance = 1.7;
            profile.attack_damage = 3.0;
            profile.attack_range = 1.8;
        }
        "minecraft:drowned" => {
            profile.behavior = VanillaEntityBehavior::HostileMelee;
            profile.movement_mode = VanillaMovementMode::Swimming;
            profile.movement_speed = 0.23;
            profile.vertical_speed = ENTITY_SWIMMING_VERTICAL_SPEED;
            profile.follow_range = 35.0;
            profile.stop_distance = 1.7;
            profile.attack_damage = 3.0;
            profile.attack_range = 1.8;
        }
        "minecraft:husk" => {
            profile.behavior = VanillaEntityBehavior::HostileMelee;
            profile.movement_speed = 0.23;
            profile.follow_range = 35.0;
            profile.stop_distance = 1.7;
            profile.attack_damage = 3.0;
            profile.attack_range = 1.8;
            profile.melee_effect = Some(EntityPotionEffect {
                effect: EntityPotionEffectKind::Hunger,
                amplifier: 0,
                duration_ticks: EntityPotionEffectKind::Hunger.default_duration_ticks(),
            });
        }
        "minecraft:spider" => {
            profile.behavior = VanillaEntityBehavior::HostileMelee;
            profile.movement_speed = 0.30;
            profile.follow_range = 32.0;
            profile.stop_distance = 1.6;
            profile.attack_damage = 2.0;
            profile.attack_range = 1.8;
        }
        "minecraft:cave_spider" => {
            profile.behavior = VanillaEntityBehavior::HostileMelee;
            profile.movement_speed = 0.30;
            profile.follow_range = 32.0;
            profile.stop_distance = 1.6;
            profile.attack_damage = 2.0;
            profile.attack_range = 1.8;
            profile.melee_effect = Some(EntityPotionEffect {
                effect: EntityPotionEffectKind::Poison,
                amplifier: 0,
                duration_ticks: 20 * 7,
            });
        }
        "minecraft:enderman" => {
            profile.behavior = VanillaEntityBehavior::HostileMelee;
            profile.movement_speed = 0.30;
            profile.follow_range = 64.0;
            profile.stop_distance = 1.8;
            profile.attack_damage = 7.0;
            profile.attack_range = 2.0;
        }
        "minecraft:warden" => {
            profile.behavior = VanillaEntityBehavior::HostileMelee;
            profile.movement_speed = 0.30;
            profile.follow_range = 48.0;
            profile.stop_distance = 2.0;
            profile.attack_damage = 30.0;
            profile.attack_range = 2.6;
            profile.attack_knockback = 0.8;
            profile.spell_damage = ENTITY_WARDEN_SONIC_BOOM_DAMAGE;
            profile.ranged_attack_range = 20.0;
            profile.ranged_attack_interval_ticks = 40;
        }
        "minecraft:ender_dragon" => {
            profile.behavior = VanillaEntityBehavior::HostileMelee;
            profile.movement_mode = VanillaMovementMode::Flying;
            profile.movement_speed = 0.32;
            profile.vertical_speed = ENTITY_FLYING_VERTICAL_SPEED;
            profile.follow_range = 96.0;
            profile.stop_distance = 4.0;
            profile.attack_damage = 10.0;
            profile.attack_range = 5.0;
            profile.attack_knockback = 1.2;
        }
        "minecraft:ravager" | "minecraft:zoglin" => {
            profile.behavior = VanillaEntityBehavior::HostileMelee;
            profile.movement_speed = 0.30;
            profile.follow_range = 32.0;
            profile.stop_distance = 2.4;
            profile.attack_damage = 12.0;
            profile.attack_range = 2.8;
            profile.attack_knockback = 0.8;
        }
        "minecraft:hoglin"
        | "minecraft:piglin_brute"
        | "minecraft:vindicator"
        | "minecraft:silverfish"
        | "minecraft:endermite"
        | "minecraft:slime"
        | "minecraft:magma_cube" => {
            profile.behavior = VanillaEntityBehavior::HostileMelee;
            profile.movement_speed = 0.26;
            profile.follow_range = 32.0;
            profile.stop_distance = 1.8;
            profile.attack_damage = match entity_type.as_str() {
                "minecraft:piglin_brute" | "minecraft:vindicator" => 13.0,
                "minecraft:hoglin" => 6.0,
                "minecraft:wither_skeleton" => 8.0,
                _ => 2.0,
            };
            profile.attack_range = 2.0;
        }
        "minecraft:piglin" | "minecraft:zombified_piglin" => {
            profile.behavior = VanillaEntityBehavior::NeutralMelee;
            profile.movement_speed = 0.26;
            profile.follow_range = 32.0;
            profile.stop_distance = 1.8;
            profile.attack_damage = 5.0;
            profile.attack_range = 2.0;
        }
        "minecraft:evoker" => {
            profile.behavior = VanillaEntityBehavior::Spellcaster;
            profile.movement_speed = 0.25;
            profile.follow_range = 32.0;
            profile.stop_distance = 8.0;
            profile.look_range = 32.0;
            profile.ranged_attack_range = 12.0;
            profile.ranged_attack_interval_ticks = 40;
            profile.spell_damage = ENTITY_EVOKER_FANGS_DAMAGE;
            profile.summon_kind = Some(EntitySummonKind::Vex);
            profile.summon_count = 2;
            profile.summon_chance = 0.15;
            profile.summon_lifetime_ticks = 20 * 60;
        }
        "minecraft:vex" => {
            profile.behavior = VanillaEntityBehavior::HostileMelee;
            profile.movement_mode = VanillaMovementMode::Flying;
            profile.movement_speed = 0.28;
            profile.vertical_speed = ENTITY_FLYING_VERTICAL_SPEED;
            profile.follow_range = 32.0;
            profile.stop_distance = 1.4;
            profile.attack_damage = 9.0;
            profile.attack_range = 1.8;
        }
        "minecraft:wither_skeleton" => {
            profile.behavior = VanillaEntityBehavior::HostileMelee;
            profile.movement_speed = 0.26;
            profile.follow_range = 32.0;
            profile.stop_distance = 1.8;
            profile.attack_damage = 8.0;
            profile.attack_range = 2.0;
            profile.melee_effect = Some(EntityPotionEffect {
                effect: EntityPotionEffectKind::Wither,
                amplifier: 0,
                duration_ticks: EntityPotionEffectKind::Wither.default_duration_ticks(),
            });
        }
        "minecraft:skeleton"
        | "minecraft:stray"
        | "minecraft:bogged"
        | "minecraft:pillager"
        | "minecraft:illusioner" => {
            profile.behavior = VanillaEntityBehavior::HostileRanged;
            profile.movement_speed = 0.25;
            profile.follow_range = 32.0;
            profile.stop_distance = 12.0;
            profile.look_range = 32.0;
            profile.ranged_attack_range = 15.0;
            profile.ranged_attack_interval_ticks = 40;
            profile.ranged_attack_damage = ENTITY_ARROW_DEFAULT_DAMAGE;
            profile.projectile_speed = ENTITY_ARROW_DEFAULT_SPEED;
        }
        "minecraft:witch" => {
            profile.behavior = VanillaEntityBehavior::HostileRanged;
            profile.movement_speed = 0.25;
            profile.follow_range = 32.0;
            profile.stop_distance = 8.0;
            profile.look_range = 32.0;
            profile.ranged_attack_range = 12.0;
            profile.ranged_attack_interval_ticks = 60;
            profile.ranged_attack_damage = 0.0;
            profile.projectile_speed = 0.75;
            profile.projectile_splash_radius = 3.0;
        }
        "minecraft:blaze" => {
            profile.behavior = VanillaEntityBehavior::HostileRanged;
            profile.movement_mode = VanillaMovementMode::Flying;
            profile.movement_speed = 0.25;
            profile.vertical_speed = ENTITY_FLYING_VERTICAL_SPEED;
            profile.follow_range = 48.0;
            profile.stop_distance = 8.0;
            profile.look_range = 32.0;
            profile.ranged_attack_range = 16.0;
            profile.ranged_attack_interval_ticks = 40;
            profile.ranged_attack_damage = ENTITY_SMALL_FIREBALL_DEFAULT_DAMAGE;
            profile.projectile_speed = 1.0;
            profile.projectile_gravity_per_tick = 0.0;
            profile.projectile_hit_radius = 0.8;
        }
        "minecraft:ghast" => {
            profile.behavior = VanillaEntityBehavior::HostileRanged;
            profile.movement_mode = VanillaMovementMode::Flying;
            profile.movement_speed = 0.18;
            profile.vertical_speed = ENTITY_FLYING_VERTICAL_SPEED;
            profile.follow_range = 64.0;
            profile.stop_distance = 24.0;
            profile.look_range = 64.0;
            profile.ranged_attack_range = 64.0;
            profile.ranged_attack_interval_ticks = 60;
            profile.ranged_attack_damage = 0.0;
            profile.projectile_speed = 1.0;
            profile.projectile_gravity_per_tick = 0.0;
            profile.projectile_hit_radius = 1.0;
            profile.projectile_explosion_radius = ENTITY_FIREBALL_DEFAULT_EXPLOSION_RADIUS;
            profile.projectile_explosion_damage = ENTITY_FIREBALL_DEFAULT_EXPLOSION_DAMAGE;
        }
        "minecraft:shulker" => {
            profile.behavior = VanillaEntityBehavior::HostileRanged;
            profile.movement_mode = VanillaMovementMode::Flying;
            profile.movement_speed = 0.0;
            profile.vertical_speed = 0.0;
            profile.follow_range = 32.0;
            profile.stop_distance = 16.0;
            profile.look_range = 32.0;
            profile.ranged_attack_range = 16.0;
            profile.ranged_attack_interval_ticks = 60;
            profile.ranged_attack_damage = ENTITY_SHULKER_BULLET_DAMAGE;
            profile.projectile_speed = 0.5;
            profile.projectile_gravity_per_tick = 0.0;
            profile.projectile_hit_radius = 0.8;
        }
        "minecraft:breeze" => {
            profile.behavior = VanillaEntityBehavior::HostileRanged;
            profile.movement_speed = 0.30;
            profile.follow_range = 32.0;
            profile.stop_distance = 8.0;
            profile.look_range = 32.0;
            profile.ranged_attack_range = 16.0;
            profile.ranged_attack_interval_ticks = 40;
            profile.ranged_attack_damage = 0.0;
            profile.projectile_speed = 1.2;
            profile.projectile_gravity_per_tick = 0.0;
            profile.projectile_hit_radius = 0.9;
            profile.projectile_explosion_radius = ENTITY_WIND_CHARGE_EXPLOSION_RADIUS;
            profile.projectile_explosion_damage = ENTITY_WIND_CHARGE_EXPLOSION_DAMAGE;
            profile.projectile_break_blocks = false;
        }
        "minecraft:wither" => {
            profile.behavior = VanillaEntityBehavior::HostileRanged;
            profile.movement_mode = VanillaMovementMode::Flying;
            profile.movement_speed = 0.30;
            profile.vertical_speed = ENTITY_FLYING_VERTICAL_SPEED;
            profile.follow_range = 64.0;
            profile.stop_distance = 16.0;
            profile.look_range = 64.0;
            profile.ranged_attack_range = 64.0;
            profile.ranged_attack_interval_ticks = 40;
            profile.ranged_attack_damage = 0.0;
            profile.projectile_speed = 1.0;
            profile.projectile_gravity_per_tick = 0.0;
            profile.projectile_hit_radius = 1.0;
            profile.projectile_explosion_radius = ENTITY_WITHER_SKULL_EXPLOSION_RADIUS;
            profile.projectile_explosion_damage = ENTITY_WITHER_SKULL_EXPLOSION_DAMAGE;
        }
        "minecraft:guardian" | "minecraft:elder_guardian" => {
            profile.behavior = VanillaEntityBehavior::GuardianBeam;
            profile.movement_mode = VanillaMovementMode::Swimming;
            profile.movement_speed = 0.20;
            profile.vertical_speed = ENTITY_SWIMMING_VERTICAL_SPEED;
            profile.follow_range = 16.0;
            profile.stop_distance = 4.0;
            profile.look_range = 16.0;
            profile.ranged_attack_range = 16.0;
            profile.ranged_attack_interval_ticks = 60;
            profile.guardian_attack_duration_ticks = ENTITY_GUARDIAN_ATTACK_DURATION_TICKS;
            profile.guardian_magic_damage = if entity_type == "minecraft:elder_guardian" {
                ENTITY_ELDER_GUARDIAN_MAGIC_DAMAGE
            } else {
                ENTITY_GUARDIAN_MAGIC_DAMAGE
            };
        }
        "minecraft:phantom" => {
            profile.behavior = VanillaEntityBehavior::HostileMelee;
            profile.movement_mode = VanillaMovementMode::Flying;
            profile.movement_speed = 0.28;
            profile.vertical_speed = ENTITY_FLYING_VERTICAL_SPEED;
            profile.follow_range = 32.0;
            profile.stop_distance = 1.8;
            profile.attack_damage = 6.0;
            profile.attack_range = 2.0;
        }
        _ => {}
    }
    profile
}

fn apply_random_stroll(entity: &mut ManagedEntity, tick_ms: u64) -> EntityMovement {
    apply_random_stroll_with(entity, tick_ms, 0.35, 0.10, 0.20)
}

fn apply_random_stroll_with(
    entity: &mut ManagedEntity,
    tick_ms: u64,
    chance: f64,
    min_speed: f64,
    max_speed: f64,
) -> EntityMovement {
    let mut rng = rand::thread_rng();
    if rand::Rng::gen_range(&mut rng, 0.0..1.0) > chance.clamp(0.0, 1.0) {
        return EntityMovement::default();
    }
    let target_yaw = rand::Rng::gen_range(&mut rng, -180.0..180.0);
    let (yaw, _) = smooth_rotation(entity.position, target_yaw, entity.position.pitch, tick_ms);
    let radians = f64::from(yaw).to_radians();
    let min_speed = min_speed.max(0.0);
    let max_speed = max_speed.max(min_speed);
    let speed = if (max_speed - min_speed).abs() < f64::EPSILON {
        min_speed
    } else {
        rand::Rng::gen_range(&mut rng, min_speed..max_speed)
    };
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VanillaEntityBehavior {
    Static,
    Passive,
    NeutralMelee,
    NeutralRanged,
    HostileMelee,
    HostileRanged,
    Spellcaster,
    GuardianBeam,
    Creeper,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VanillaMovementMode {
    Ground,
    Swimming,
    Flying,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EntitySummonKind {
    EvokerFangs,
    Vex,
}

#[derive(Debug, Clone, Copy)]
struct VanillaEntityAiProfile {
    behavior: VanillaEntityBehavior,
    movement_mode: VanillaMovementMode,
    movement_speed: f64,
    vertical_speed: f64,
    follow_range: f64,
    stop_distance: f64,
    stroll_chance: f64,
    stroll_min_speed: f64,
    stroll_max_speed: f64,
    look_range: f64,
    attack_damage: f32,
    attack_range: f64,
    attack_interval_ticks: i32,
    attack_knockback: f32,
    melee_effect: Option<EntityPotionEffect>,
    melee_effect_chance: f64,
    ranged_attack_damage: f32,
    ranged_attack_range: f64,
    ranged_attack_interval_ticks: i32,
    projectile_speed: f64,
    projectile_gravity_per_tick: f64,
    projectile_hit_radius: f64,
    projectile_lifetime_ticks: i32,
    projectile_explosion_radius: f64,
    projectile_explosion_damage: f32,
    projectile_break_blocks: bool,
    projectile_splash_radius: f64,
    spell_damage: f32,
    summon_kind: Option<EntitySummonKind>,
    summon_count: i32,
    summon_chance: f64,
    summon_lifetime_ticks: i32,
    guardian_attack_duration_ticks: i32,
    guardian_magic_damage: f32,
    slime_jump_velocity: f64,
    slime_jump_delay_ticks: i32,
    spider_climb_velocity: f64,
    enderman_teleport_min_distance: f64,
    enderman_teleport_arrival_distance: f64,
    enderman_teleport_chance: f64,
    creeper_swell_ticks: i32,
    creeper_explosion_radius: f64,
    creeper_explosion_damage: f32,
    creeper_break_blocks: bool,
}

#[allow(clippy::too_many_arguments)]
fn apply_vanilla_ai(
    entity: &mut ManagedEntity,
    viewers: &[crate::players::OnlinePlayer],
    target_entities: &[ManagedEntity],
    tick_ms: u64,
    target_memory: &mut HashMap<String, EntityTargetMemory>,
    path_memory: &mut HashMap<String, EntityPathMemory>,
    target_reselects: &mut usize,
    path_recalcs: &mut usize,
    world: &crate::world::WorldManager,
    collision_cache: &mut CollisionCache,
    now: Instant,
    default_look_range: f64,
) -> EntityMovement {
    let profile = resolved_vanilla_ai_profile(entity, default_look_range);

    match profile.behavior {
        VanillaEntityBehavior::Static => {
            target_memory.remove(&entity.key);
            path_memory.remove(&entity.key);
            EntityMovement::default()
        }
        VanillaEntityBehavior::Passive => {
            target_memory.remove(&entity.key);
            path_memory.remove(&entity.key);
            apply_vanilla_passive_ai(entity, viewers, target_entities, tick_ms, world, profile)
        }
        VanillaEntityBehavior::NeutralMelee => {
            if !neutral_entity_is_angry(entity) {
                if has_vanilla_entity_target(entity, target_entities, profile.follow_range) {
                    return apply_vanilla_follow_movement(
                        entity,
                        viewers,
                        target_entities,
                        tick_ms,
                        target_memory,
                        path_memory,
                        target_reselects,
                        path_recalcs,
                        world,
                        collision_cache,
                        now,
                        profile,
                        profile.stop_distance,
                    );
                }
                target_memory.remove(&entity.key);
                path_memory.remove(&entity.key);
                return apply_vanilla_passive_ai(
                    entity,
                    viewers,
                    target_entities,
                    tick_ms,
                    world,
                    profile,
                );
            }
            apply_vanilla_follow_movement(
                entity,
                viewers,
                target_entities,
                tick_ms,
                target_memory,
                path_memory,
                target_reselects,
                path_recalcs,
                world,
                collision_cache,
                now,
                profile,
                profile.stop_distance,
            )
        }
        VanillaEntityBehavior::HostileMelee | VanillaEntityBehavior::Creeper => {
            if enderman_is_passive_to_players(entity, viewers, profile) {
                target_memory.remove(&entity.key);
                path_memory.remove(&entity.key);
                return apply_vanilla_passive_ai(
                    entity,
                    viewers,
                    target_entities,
                    tick_ms,
                    world,
                    profile,
                );
            }
            if try_apply_enderman_target_teleport(entity, viewers, world, collision_cache, profile)
            {
                target_memory.remove(&entity.key);
                path_memory.remove(&entity.key);
                return EntityMovement::default();
            }
            if let Some(movement) =
                vanilla_avoidance_movement(entity, target_entities, tick_ms, profile)
            {
                target_memory.remove(&entity.key);
                path_memory.remove(&entity.key);
                return movement;
            }
            apply_vanilla_follow_movement(
                entity,
                viewers,
                target_entities,
                tick_ms,
                target_memory,
                path_memory,
                target_reselects,
                path_recalcs,
                world,
                collision_cache,
                now,
                profile,
                profile.stop_distance,
            )
        }
        VanillaEntityBehavior::NeutralRanged => {
            if !neutral_entity_is_angry(entity)
                && !has_vanilla_entity_target(entity, target_entities, profile.follow_range)
            {
                target_memory.remove(&entity.key);
                path_memory.remove(&entity.key);
                return apply_vanilla_passive_ai(
                    entity,
                    viewers,
                    target_entities,
                    tick_ms,
                    world,
                    profile,
                );
            }
            apply_vanilla_follow_movement(
                entity,
                viewers,
                target_entities,
                tick_ms,
                target_memory,
                path_memory,
                target_reselects,
                path_recalcs,
                world,
                collision_cache,
                now,
                profile,
                profile.stop_distance.max(8.0),
            )
        }
        VanillaEntityBehavior::HostileRanged
        | VanillaEntityBehavior::Spellcaster
        | VanillaEntityBehavior::GuardianBeam => {
            if let Some(movement) =
                vanilla_avoidance_movement(entity, target_entities, tick_ms, profile)
            {
                target_memory.remove(&entity.key);
                path_memory.remove(&entity.key);
                return movement;
            }
            apply_vanilla_follow_movement(
                entity,
                viewers,
                target_entities,
                tick_ms,
                target_memory,
                path_memory,
                target_reselects,
                path_recalcs,
                world,
                collision_cache,
                now,
                profile,
                profile.stop_distance.max(8.0),
            )
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn apply_vanilla_follow_movement(
    entity: &mut ManagedEntity,
    viewers: &[crate::players::OnlinePlayer],
    target_entities: &[ManagedEntity],
    tick_ms: u64,
    target_memory: &mut HashMap<String, EntityTargetMemory>,
    path_memory: &mut HashMap<String, EntityPathMemory>,
    target_reselects: &mut usize,
    path_recalcs: &mut usize,
    world: &crate::world::WorldManager,
    collision_cache: &mut CollisionCache,
    now: Instant,
    profile: VanillaEntityAiProfile,
    stop_distance: f64,
) -> EntityMovement {
    if let Some(target) =
        nearest_vanilla_entity_target(entity, target_entities, profile.follow_range)
    {
        target_memory.remove(&entity.key);
        path_memory.remove(&entity.key);
        return apply_follow_position_movement(
            entity,
            target.position,
            tick_ms,
            world,
            profile,
            stop_distance,
        );
    }
    let mut movement = apply_follow_nearest_player_with(
        entity,
        viewers,
        tick_ms,
        target_memory,
        path_memory,
        target_reselects,
        path_recalcs,
        world,
        collision_cache,
        now,
        profile.follow_range,
        stop_distance,
        profile.movement_speed,
    );
    movement.y += vanilla_vertical_target_movement(entity, viewers, world, profile);
    movement
}

fn apply_follow_position_movement(
    entity: &mut ManagedEntity,
    target_position: EntityPosition,
    tick_ms: u64,
    world: &crate::world::WorldManager,
    profile: VanillaEntityAiProfile,
    stop_distance: f64,
) -> EntityMovement {
    let target_dx = target_position.x - entity.position.x;
    let target_dz = target_position.z - entity.position.z;
    let target_horizontal = (target_dx * target_dx + target_dz * target_dz).sqrt();
    let (target_yaw, target_pitch) = look_rotation(entity.position, target_position);
    let (yaw, pitch) = smooth_rotation(entity.position, target_yaw, target_pitch, tick_ms);
    entity.position.yaw = yaw;
    entity.position.pitch = pitch;
    if target_horizontal <= stop_distance.max(0.0) {
        return EntityMovement::default();
    }
    let speed = profile.movement_speed.max(0.0).min(target_horizontal);
    let mut movement = EntityMovement {
        x: target_dx / target_horizontal * speed,
        y: 0.0,
        z: target_dz / target_horizontal * speed,
    };
    movement.y += vanilla_vertical_position_movement(entity, target_position, world, profile);
    movement
}

fn apply_vanilla_passive_ai(
    entity: &mut ManagedEntity,
    viewers: &[crate::players::OnlinePlayer],
    target_entities: &[ManagedEntity],
    tick_ms: u64,
    world: &crate::world::WorldManager,
    profile: VanillaEntityAiProfile,
) -> EntityMovement {
    if let Some(movement) = vanilla_avoidance_movement(entity, target_entities, tick_ms, profile) {
        return movement;
    }
    let mut movement = apply_random_stroll_with(
        entity,
        tick_ms,
        profile.stroll_chance,
        profile.stroll_min_speed,
        profile.stroll_max_speed,
    );
    movement.y += vanilla_random_vertical_movement(entity, world, profile);
    if movement.x.abs() + movement.y.abs() + movement.z.abs() <= 0.0001 {
        apply_look_at_nearest_player(entity, viewers, profile.look_range);
    }
    movement
}

fn vanilla_avoidance_movement(
    entity: &mut ManagedEntity,
    target_entities: &[ManagedEntity],
    tick_ms: u64,
    profile: VanillaEntityAiProfile,
) -> Option<EntityMovement> {
    let threat = vanilla_avoidance_threat(entity, target_entities)?;
    let dx = entity.position.x - threat.position.x;
    let dz = entity.position.z - threat.position.z;
    let horizontal = (dx * dx + dz * dz).sqrt();
    if horizontal <= f64::EPSILON {
        return None;
    }
    let flee_target = EntityPosition {
        x: entity.position.x + dx / horizontal,
        y: entity.position.y,
        z: entity.position.z + dz / horizontal,
        yaw: 0.0,
        pitch: 0.0,
        on_ground: entity.position.on_ground,
    };
    let (target_yaw, target_pitch) = look_rotation(entity.position, flee_target);
    let (yaw, pitch) = smooth_rotation(entity.position, target_yaw, target_pitch, tick_ms);
    entity.position.yaw = yaw;
    entity.position.pitch = pitch;
    let speed = ai_param_f64(
        &entity.ai_params,
        "avoid_entities_speed",
        ai_param_f64(
            &entity.ai_params,
            "avoid_hostiles_speed",
            profile.movement_speed.max(profile.stroll_max_speed) * 1.25,
        ),
    )
    .clamp(0.0, ENTITY_MAX_HORIZONTAL_SPEED);
    Some(EntityMovement {
        x: dx / horizontal * speed,
        y: 0.0,
        z: dz / horizontal * speed,
    })
}

fn vanilla_avoidance_threat<'a>(
    entity: &ManagedEntity,
    target_entities: &'a [ManagedEntity],
) -> Option<&'a ManagedEntity> {
    if ai_param_bool(&entity.ai_params, "disable_avoid_entities", false)
        || ai_param_bool(&entity.ai_params, "disable_avoid_hostiles", false)
    {
        return None;
    }
    nearest_vanilla_avoidance_threat(entity, target_entities)
}

fn nearest_vanilla_avoidance_threat<'a>(
    entity: &ManagedEntity,
    candidates: &'a [ManagedEntity],
) -> Option<&'a ManagedEntity> {
    candidates
        .iter()
        .filter(|candidate| {
            candidate.kind == ManagedEntityKind::Entity
                && candidate.key != entity.key
                && candidate.dimension == entity.dimension
                && vanilla_avoidance_range(entity, candidate).is_some_and(|range| {
                    let configured = ai_param_f64(
                        &entity.ai_params,
                        "avoid_entities_range",
                        ai_param_f64(&entity.ai_params, "avoid_hostiles_range", range),
                    )
                    .max(0.0);
                    distance_sq(entity.position, candidate.position) <= configured * configured
                })
        })
        .min_by(|left, right| {
            distance_sq(entity.position, left.position)
                .total_cmp(&distance_sq(entity.position, right.position))
        })
}

fn vanilla_avoidance_range(entity: &ManagedEntity, threat: &ManagedEntity) -> Option<f64> {
    let entity_type = normalized_entity_type(&entity.entity_type);
    let threat_type = normalized_entity_type(&threat.entity_type);
    if entity_type_is_villager_like(&entity_type) && vanilla_entity_can_attack(threat, entity) {
        return Some(8.0);
    }
    match entity_type.as_str() {
        "minecraft:creeper"
            if matches!(threat_type.as_str(), "minecraft:cat" | "minecraft:ocelot") =>
        {
            Some(6.0)
        }
        "minecraft:phantom" if threat_type == "minecraft:cat" => Some(16.0),
        "minecraft:skeleton"
        | "minecraft:stray"
        | "minecraft:bogged"
        | "minecraft:wither_skeleton"
            if threat_type == "minecraft:wolf" =>
        {
            Some(6.0)
        }
        "minecraft:rabbit"
            if matches!(
                threat_type.as_str(),
                "minecraft:wolf" | "minecraft:fox" | "minecraft:cat" | "minecraft:ocelot"
            ) =>
        {
            Some(10.0)
        }
        _ => None,
    }
}

fn neutral_entity_is_angry(entity: &ManagedEntity) -> bool {
    ai_param_bool(
        &entity.ai_params,
        "angry",
        ai_param_bool(&entity.ai_params, "neutral_angry", false),
    )
}

fn enderman_is_passive_to_players(
    entity: &ManagedEntity,
    viewers: &[crate::players::OnlinePlayer],
    profile: VanillaEntityAiProfile,
) -> bool {
    normalized_entity_type(&entity.entity_type) == "minecraft:enderman"
        && !enderman_should_target_player(entity, viewers, profile)
}

fn enderman_should_target_player(
    entity: &ManagedEntity,
    viewers: &[crate::players::OnlinePlayer],
    profile: VanillaEntityAiProfile,
) -> bool {
    if neutral_entity_is_angry(entity) {
        return true;
    }
    if ai_param_bool(&entity.ai_params, "disable_enderman_stare_aggro", false) {
        return false;
    }
    nearest_enderman_staring_player(entity, viewers, profile.follow_range).is_some()
}

fn nearest_enderman_staring_player<'a>(
    entity: &ManagedEntity,
    viewers: &'a [crate::players::OnlinePlayer],
    range: f64,
) -> Option<&'a crate::players::OnlinePlayer> {
    let range = ai_param_f64(&entity.ai_params, "enderman_stare_range", range).max(0.0);
    let range_sq = range * range;
    viewers
        .iter()
        .filter(|player| {
            player.dimension == entity.dimension
                && player_can_be_attacked(player)
                && distance_sq(entity.position, player.position) <= range_sq
                && player_is_staring_at_enderman(player, entity)
        })
        .min_by(|left, right| {
            distance_sq(entity.position, left.position)
                .total_cmp(&distance_sq(entity.position, right.position))
        })
}

fn player_is_staring_at_enderman(
    player: &crate::players::OnlinePlayer,
    entity: &ManagedEntity,
) -> bool {
    let dx = entity.position.x - player.position.x;
    let dy = entity.position.y + 1.8 - (player.position.y + 1.62);
    let dz = entity.position.z - player.position.z;
    let distance = (dx * dx + dy * dy + dz * dz).sqrt();
    if distance <= f64::EPSILON {
        return true;
    }
    let (look_x, look_y, look_z) = look_vector(player.position.yaw, player.position.pitch);
    let dot = look_x * dx / distance + look_y * dy / distance + look_z * dz / distance;
    let threshold = ai_param_f64(&entity.ai_params, "enderman_stare_dot", 0.98).clamp(-1.0, 1.0);
    dot >= threshold
}

fn look_vector(yaw: f32, pitch: f32) -> (f64, f64, f64) {
    let yaw = f64::from(yaw + 90.0).to_radians();
    let pitch = f64::from(pitch).to_radians();
    let horizontal = pitch.cos();
    (yaw.cos() * horizontal, -pitch.sin(), yaw.sin() * horizontal)
}

fn vanilla_vertical_target_movement(
    entity: &ManagedEntity,
    viewers: &[crate::players::OnlinePlayer],
    world: &crate::world::WorldManager,
    profile: VanillaEntityAiProfile,
) -> f64 {
    if !vanilla_movement_mode_can_adjust_y(entity, world, profile) {
        return 0.0;
    }
    let Some(target) = nearest_player(
        entity.position,
        &entity.dimension,
        viewers,
        profile.follow_range,
    ) else {
        return 0.0;
    };
    let dy = target.position.y - entity.position.y;
    if dy.abs() <= 0.75 {
        return 0.0;
    }
    dy.signum() * profile.vertical_speed.min(dy.abs())
}

fn vanilla_vertical_position_movement(
    entity: &ManagedEntity,
    target_position: EntityPosition,
    world: &crate::world::WorldManager,
    profile: VanillaEntityAiProfile,
) -> f64 {
    if !vanilla_movement_mode_can_adjust_y(entity, world, profile) {
        return 0.0;
    }
    let dy = target_position.y - entity.position.y;
    if dy.abs() <= 0.75 {
        return 0.0;
    }
    dy.signum() * profile.vertical_speed.min(dy.abs())
}

fn vanilla_random_vertical_movement(
    entity: &ManagedEntity,
    world: &crate::world::WorldManager,
    profile: VanillaEntityAiProfile,
) -> f64 {
    if !vanilla_movement_mode_can_adjust_y(entity, world, profile) || profile.vertical_speed <= 0.0
    {
        return 0.0;
    }
    if rand::Rng::gen_range(&mut rand::thread_rng(), 0.0..1.0)
        > (profile.stroll_chance * 0.5).clamp(0.0, 1.0)
    {
        return 0.0;
    }
    rand::Rng::gen_range(
        &mut rand::thread_rng(),
        -profile.vertical_speed..=profile.vertical_speed,
    )
}

fn vanilla_movement_mode_can_adjust_y(
    entity: &ManagedEntity,
    world: &crate::world::WorldManager,
    profile: VanillaEntityAiProfile,
) -> bool {
    match profile.movement_mode {
        VanillaMovementMode::Ground => false,
        VanillaMovementMode::Flying => profile.vertical_speed > 0.0,
        VanillaMovementMode::Swimming => {
            profile.vertical_speed > 0.0
                && entity_position_touches_water(world, &entity.dimension, entity.position)
        }
    }
}

fn resolved_vanilla_ai_profile(
    entity: &ManagedEntity,
    default_look_range: f64,
) -> VanillaEntityAiProfile {
    let mut profile = vanilla_entity_ai_profile(&entity.entity_type);
    apply_slime_size_to_profile(entity, &mut profile);
    profile.behavior = ai_param_string(&entity.ai_params, "behavior")
        .and_then(|value| vanilla_behavior_override(&value))
        .unwrap_or(profile.behavior);
    profile.movement_mode = ai_param_string(&entity.ai_params, "movement_mode")
        .or_else(|| ai_param_string(&entity.ai_params, "locomotion"))
        .and_then(|value| vanilla_movement_mode_override(&value))
        .unwrap_or(profile.movement_mode);
    profile.movement_speed =
        ai_param_f64(&entity.ai_params, "movement_speed", profile.movement_speed)
            .clamp(0.0, ENTITY_MAX_HORIZONTAL_SPEED);
    profile.vertical_speed =
        ai_param_f64(&entity.ai_params, "vertical_speed", profile.vertical_speed)
            .clamp(0.0, ENTITY_MAX_HORIZONTAL_SPEED);
    profile.follow_range =
        ai_param_f64(&entity.ai_params, "follow_range", profile.follow_range).max(0.0);
    profile.stop_distance =
        ai_param_f64(&entity.ai_params, "stop_distance", profile.stop_distance).max(0.0);
    profile.stroll_chance =
        ai_param_f64(&entity.ai_params, "stroll_chance", profile.stroll_chance).clamp(0.0, 1.0);
    profile.stroll_min_speed = ai_param_f64(
        &entity.ai_params,
        "stroll_min_speed",
        profile.stroll_min_speed,
    )
    .max(0.0);
    profile.stroll_max_speed = ai_param_f64(
        &entity.ai_params,
        "stroll_max_speed",
        profile.stroll_max_speed,
    )
    .max(profile.stroll_min_speed);
    profile.look_range = ai_param_f64(
        &entity.ai_params,
        "look_range",
        profile.look_range.max(default_look_range.min(8.0)),
    )
    .max(0.0);
    profile.attack_damage = ai_param_f64(
        &entity.ai_params,
        "attack_damage",
        f64::from(profile.attack_damage),
    )
    .max(0.0) as f32;
    profile.attack_range =
        ai_param_f64(&entity.ai_params, "attack_range", profile.attack_range).max(0.0);
    profile.attack_interval_ticks = ai_param_i32(
        &entity.ai_params,
        "attack_interval_ticks",
        profile.attack_interval_ticks,
    )
    .clamp(1, 200);
    profile.attack_knockback = ai_param_f64(
        &entity.ai_params,
        "attack_knockback",
        f64::from(profile.attack_knockback),
    )
    .max(0.0) as f32;
    profile.melee_effect_chance = ai_param_f64(
        &entity.ai_params,
        "melee_effect_chance",
        profile.melee_effect_chance,
    )
    .clamp(0.0, 1.0);
    profile.melee_effect = resolved_melee_effect(&entity.ai_params, profile.melee_effect);
    profile.ranged_attack_damage = ai_param_f64(
        &entity.ai_params,
        "ranged_attack_damage",
        f64::from(profile.ranged_attack_damage),
    )
    .max(0.0) as f32;
    profile.ranged_attack_range = ai_param_f64(
        &entity.ai_params,
        "ranged_attack_range",
        profile.ranged_attack_range,
    )
    .max(0.0);
    profile.ranged_attack_interval_ticks = ai_param_i32(
        &entity.ai_params,
        "ranged_attack_interval_ticks",
        profile.ranged_attack_interval_ticks,
    )
    .clamp(1, 400);
    profile.projectile_speed = ai_param_f64(
        &entity.ai_params,
        "projectile_speed",
        profile.projectile_speed,
    )
    .max(0.1);
    profile.projectile_gravity_per_tick = ai_param_f64(
        &entity.ai_params,
        "projectile_gravity_per_tick",
        ai_param_f64(
            &entity.ai_params,
            "projectile_gravity",
            profile.projectile_gravity_per_tick,
        ),
    )
    .max(0.0);
    profile.projectile_hit_radius = ai_param_f64(
        &entity.ai_params,
        "projectile_hit_radius",
        profile.projectile_hit_radius,
    )
    .clamp(0.1, 8.0);
    profile.projectile_lifetime_ticks = ai_param_i32(
        &entity.ai_params,
        "projectile_lifetime_ticks",
        profile.projectile_lifetime_ticks,
    )
    .clamp(1, 20 * 60 * 5);
    profile.projectile_explosion_radius = ai_param_f64(
        &entity.ai_params,
        "projectile_explosion_radius",
        profile.projectile_explosion_radius,
    )
    .max(0.0);
    profile.projectile_explosion_damage = ai_param_f64(
        &entity.ai_params,
        "projectile_explosion_damage",
        f64::from(profile.projectile_explosion_damage),
    )
    .max(0.0) as f32;
    profile.projectile_break_blocks = ai_param_bool(
        &entity.ai_params,
        "projectile_break_blocks",
        ai_param_bool(
            &entity.ai_params,
            "break_blocks",
            profile.projectile_break_blocks,
        ),
    );
    profile.projectile_splash_radius = ai_param_f64(
        &entity.ai_params,
        "projectile_splash_radius",
        profile.projectile_splash_radius,
    )
    .max(0.0);
    profile.spell_damage = ai_param_f64(
        &entity.ai_params,
        "spell_damage",
        f64::from(profile.spell_damage),
    )
    .max(0.0) as f32;
    profile.summon_kind = ai_param_string(&entity.ai_params, "summon_kind")
        .or_else(|| ai_param_string(&entity.ai_params, "summon_entity_type"))
        .and_then(|value| entity_summon_kind(&value))
        .or(profile.summon_kind);
    profile.summon_count =
        ai_param_i32(&entity.ai_params, "summon_count", profile.summon_count).clamp(0, 8);
    profile.summon_chance =
        ai_param_f64(&entity.ai_params, "summon_chance", profile.summon_chance).clamp(0.0, 1.0);
    profile.summon_lifetime_ticks = ai_param_i32(
        &entity.ai_params,
        "summon_lifetime_ticks",
        profile.summon_lifetime_ticks,
    )
    .clamp(1, 20 * 60 * 10);
    profile.guardian_attack_duration_ticks = ai_param_i32(
        &entity.ai_params,
        "guardian_attack_duration_ticks",
        ai_param_i32(
            &entity.ai_params,
            "attack_duration_ticks",
            profile.guardian_attack_duration_ticks,
        ),
    )
    .clamp(1, 20 * 30);
    profile.guardian_magic_damage = ai_param_f64(
        &entity.ai_params,
        "guardian_magic_damage",
        f64::from(profile.guardian_magic_damage),
    )
    .max(0.0) as f32;
    profile.slime_jump_velocity = ai_param_f64(
        &entity.ai_params,
        "slime_jump_velocity",
        profile.slime_jump_velocity,
    )
    .max(0.0);
    profile.slime_jump_delay_ticks = ai_param_i32(
        &entity.ai_params,
        "slime_jump_delay_ticks",
        profile.slime_jump_delay_ticks,
    )
    .clamp(1, 200);
    profile.spider_climb_velocity = ai_param_f64(
        &entity.ai_params,
        "spider_climb_velocity",
        profile.spider_climb_velocity,
    )
    .max(0.0);
    profile.enderman_teleport_min_distance = ai_param_f64(
        &entity.ai_params,
        "enderman_teleport_min_distance",
        profile.enderman_teleport_min_distance,
    )
    .max(0.0);
    profile.enderman_teleport_arrival_distance = ai_param_f64(
        &entity.ai_params,
        "enderman_teleport_arrival_distance",
        profile.enderman_teleport_arrival_distance,
    )
    .max(0.0);
    profile.enderman_teleport_chance = ai_param_f64(
        &entity.ai_params,
        "enderman_teleport_chance",
        profile.enderman_teleport_chance,
    )
    .clamp(0.0, 1.0);
    profile.creeper_swell_ticks = ai_param_i32(
        &entity.ai_params,
        "creeper_swell_ticks",
        profile.creeper_swell_ticks,
    )
    .clamp(1, 200);
    profile.creeper_explosion_radius = ai_param_f64(
        &entity.ai_params,
        "creeper_explosion_radius",
        profile.creeper_explosion_radius,
    )
    .max(0.1);
    profile.creeper_explosion_damage = ai_param_f64(
        &entity.ai_params,
        "creeper_explosion_damage",
        f64::from(profile.creeper_explosion_damage),
    )
    .max(0.0) as f32;
    profile.creeper_break_blocks = ai_param_bool(
        &entity.ai_params,
        "creeper_break_blocks",
        ai_param_bool(
            &entity.ai_params,
            "break_blocks",
            profile.creeper_break_blocks,
        ),
    );
    apply_entity_specific_profile_params(entity, &mut profile);
    profile
}

fn vanilla_movement_mode_override(value: &str) -> Option<VanillaMovementMode> {
    match value.trim() {
        "ground" | "walk" | "walking" => Some(VanillaMovementMode::Ground),
        "swim" | "swimming" | "water" | "aquatic" => Some(VanillaMovementMode::Swimming),
        "fly" | "flying" | "air" => Some(VanillaMovementMode::Flying),
        _ => None,
    }
}

fn entity_summon_kind(value: &str) -> Option<EntitySummonKind> {
    match normalized_entity_type(value).as_str() {
        "minecraft:evoker_fangs" | "evoker_fangs" | "fangs" => Some(EntitySummonKind::EvokerFangs),
        "minecraft:vex" | "vex" => Some(EntitySummonKind::Vex),
        _ => None,
    }
}

fn apply_entity_specific_profile_params(
    entity: &ManagedEntity,
    profile: &mut VanillaEntityAiProfile,
) {
    if normalized_entity_type(&entity.entity_type) == "minecraft:drowned"
        && ai_param_bool(
            &entity.ai_params,
            "drowned_has_trident",
            ai_param_bool(&entity.ai_params, "has_trident", false),
        )
    {
        profile.behavior = VanillaEntityBehavior::HostileRanged;
        profile.stop_distance = profile.stop_distance.max(8.0);
        profile.ranged_attack_range = ai_param_f64(&entity.ai_params, "ranged_attack_range", 20.0);
        profile.ranged_attack_damage = ai_param_f64(
            &entity.ai_params,
            "ranged_attack_damage",
            f64::from(ENTITY_TRIDENT_DEFAULT_DAMAGE),
        )
        .max(0.0) as f32;
        profile.ranged_attack_interval_ticks =
            ai_param_i32(&entity.ai_params, "ranged_attack_interval_ticks", 60).clamp(1, 400);
        profile.projectile_speed = ai_param_f64(
            &entity.ai_params,
            "projectile_speed",
            ENTITY_TRIDENT_DEFAULT_SPEED,
        )
        .max(0.1);
    }
}

fn apply_slime_size_to_profile(entity: &ManagedEntity, profile: &mut VanillaEntityAiProfile) {
    if !entity_type_uses_slime_size(&entity.entity_type) {
        return;
    }
    let size = slime_size_for_entity(entity);
    profile.movement_speed = 0.2 + 0.1 * f64::from(size);
    let is_magma_cube = normalized_entity_type(&entity.entity_type) == "minecraft:magma_cube";
    profile.attack_damage = if is_magma_cube {
        (size + 2) as f32
    } else {
        size as f32
    };
    if is_magma_cube {
        profile.slime_jump_velocity = ENTITY_SLIME_JUMP_VELOCITY + f64::from(size) * 0.1;
        profile.slime_jump_delay_ticks = ENTITY_SLIME_JUMP_DELAY_TICKS * 4;
    }
}

fn resolved_melee_effect(
    params: &BTreeMap<String, serde_json::Value>,
    default_effect: Option<EntityPotionEffect>,
) -> Option<EntityPotionEffect> {
    if ai_param_bool(params, "disable_melee_effect", false) {
        return None;
    }
    let effect = ai_param_string(params, "melee_effect")
        .or_else(|| ai_param_string(params, "attack_effect"))
        .and_then(|value| potion_effect_kind(&value))
        .map(|effect| EntityPotionEffect {
            effect,
            amplifier: 0,
            duration_ticks: effect.default_duration_ticks(),
        })
        .or(default_effect)?;
    Some(EntityPotionEffect {
        effect: effect.effect,
        amplifier: ai_param_i32(params, "melee_effect_amplifier", effect.amplifier).clamp(0, 16),
        duration_ticks: ai_param_i32(params, "melee_effect_duration_ticks", effect.duration_ticks)
            .clamp(1, 20 * 60 * 10),
    })
}

fn try_apply_enderman_target_teleport(
    entity: &mut ManagedEntity,
    viewers: &[crate::players::OnlinePlayer],
    world: &crate::world::WorldManager,
    collision_cache: &mut CollisionCache,
    profile: VanillaEntityAiProfile,
) -> bool {
    if normalized_entity_type(&entity.entity_type) != "minecraft:enderman"
        || profile.enderman_teleport_chance <= 0.0
        || !enderman_should_target_player(entity, viewers, profile)
        || rand::Rng::gen_range(&mut rand::thread_rng(), 0.0..1.0)
            > profile.enderman_teleport_chance
    {
        return false;
    }
    let Some(target) = nearest_attackable_player(
        entity.position,
        &entity.dimension,
        viewers,
        profile.follow_range,
    ) else {
        return false;
    };
    let dx = target.position.x - entity.position.x;
    let dz = target.position.z - entity.position.z;
    let horizontal = (dx * dx + dz * dz).sqrt();
    if horizontal < profile.enderman_teleport_min_distance.max(0.0) || horizontal <= f64::EPSILON {
        return false;
    }

    let arrival_distance = profile
        .enderman_teleport_arrival_distance
        .min(horizontal)
        .max(0.0);
    let candidate_x = target.position.x - dx / horizontal * arrival_distance;
    let candidate_z = target.position.z - dz / horizontal * arrival_distance;
    let Some(mut landing) = find_entity_landing_position(
        world,
        collision_cache,
        &entity.dimension,
        candidate_x,
        target.position.y + 8.0,
        candidate_z,
        24,
    ) else {
        return false;
    };
    let (yaw, pitch) = look_rotation(landing, target.position);
    landing.yaw = yaw;
    landing.pitch = pitch;
    entity.position = landing;
    true
}

fn find_entity_landing_position(
    world: &crate::world::WorldManager,
    collision_cache: &mut CollisionCache,
    dimension: &str,
    x: f64,
    start_y: f64,
    z: f64,
    scan_down_blocks: i32,
) -> Option<EntityPosition> {
    let top_y = start_y.floor() as i32;
    let min_y = top_y.saturating_sub(scan_down_blocks.max(1));
    let block_x = x.floor() as i32;
    let block_z = z.floor() as i32;
    for support_y in (min_y..=top_y).rev() {
        let shape =
            collision_cache.block_collision_shape(world, dimension, block_x, support_y, block_z);
        let Some(shape) = shape else {
            continue;
        };
        let feet_y = f64::from(support_y) + shape.max_y;
        if entity_aabb_intersects_solid_at(world, collision_cache, dimension, x, feet_y, z) {
            continue;
        }
        let landing = EntityPosition {
            x,
            y: feet_y,
            z,
            yaw: 0.0,
            pitch: 0.0,
            on_ground: true,
        };
        if entity_position_touches_water(world, dimension, landing) {
            continue;
        }
        return Some(landing);
    }
    None
}

fn apply_enderman_water_reaction(
    entity: &mut ManagedEntity,
    world: &crate::world::WorldManager,
    collision_cache: &mut CollisionCache,
    tick_ms: u64,
    environment_damage: &mut Vec<(i32, f32)>,
) -> bool {
    if normalized_entity_type(&entity.entity_type) != "minecraft:enderman"
        || !entity_is_in_water(entity, world)
    {
        return false;
    }
    let water_damage = ai_param_f64(
        &entity.ai_params,
        "enderman_water_damage",
        f64::from(ENTITY_ENDERMAN_WATER_DAMAGE),
    )
    .max(0.0) as f32;
    if water_damage > 0.0 {
        environment_damage.push((
            entity.entity_id,
            water_damage * entity_tick_units(tick_ms) as f32,
        ));
    }
    if ai_param_bool(&entity.ai_params, "disable_enderman_water_teleport", false) {
        return false;
    }
    try_apply_enderman_water_escape_teleport(entity, world, collision_cache)
}

fn apply_out_of_water_damage(
    entity: &ManagedEntity,
    world: &crate::world::WorldManager,
    tick_ms: u64,
    environment_damage: &mut Vec<(i32, f32)>,
) {
    if !entity_type_takes_out_of_water_damage(&entity.entity_type)
        || ai_param_bool(&entity.ai_params, "disable_out_of_water_damage", false)
        || entity_is_in_water(entity, world)
    {
        return;
    }
    let damage = ai_param_f64(
        &entity.ai_params,
        "out_of_water_damage",
        f64::from(ENTITY_OUT_OF_WATER_DAMAGE),
    )
    .max(0.0) as f32;
    if damage <= 0.0 {
        return;
    }
    environment_damage.push((entity.entity_id, damage * entity_tick_units(tick_ms) as f32));
}

fn apply_water_contact_damage(
    entity: &ManagedEntity,
    world: &crate::world::WorldManager,
    tick_ms: u64,
    environment_damage: &mut Vec<(i32, f32)>,
) {
    if !entity_type_takes_water_contact_damage(&entity.entity_type)
        || ai_param_bool(&entity.ai_params, "disable_water_contact_damage", false)
        || !entity_is_in_water(entity, world)
    {
        return;
    }
    let damage = ai_param_f64(
        &entity.ai_params,
        "water_contact_damage",
        f64::from(ENTITY_WATER_CONTACT_DAMAGE),
    )
    .max(0.0) as f32;
    if damage <= 0.0 {
        return;
    }
    environment_damage.push((entity.entity_id, damage * entity_tick_units(tick_ms) as f32));
}

fn entity_type_takes_out_of_water_damage(entity_type: &str) -> bool {
    matches!(
        normalized_entity_type(entity_type).as_str(),
        "minecraft:cod"
            | "minecraft:salmon"
            | "minecraft:tropical_fish"
            | "minecraft:pufferfish"
            | "minecraft:tadpole"
            | "minecraft:squid"
            | "minecraft:glow_squid"
            | "minecraft:axolotl"
            | "minecraft:dolphin"
    )
}

fn entity_type_takes_water_contact_damage(entity_type: &str) -> bool {
    matches!(
        normalized_entity_type(entity_type).as_str(),
        "minecraft:blaze" | "minecraft:snow_golem"
    )
}

fn try_apply_enderman_water_escape_teleport(
    entity: &mut ManagedEntity,
    world: &crate::world::WorldManager,
    collision_cache: &mut CollisionCache,
) -> bool {
    let radius = ai_param_i32(
        &entity.ai_params,
        "enderman_water_teleport_radius",
        ENTITY_ENDERMAN_WATER_TELEPORT_RADIUS,
    )
    .clamp(1, 64);
    for distance in 1..=radius {
        for dx in -distance..=distance {
            for dz in -distance..=distance {
                if dx.abs() != distance && dz.abs() != distance {
                    continue;
                }
                let x = entity.position.x + f64::from(dx);
                let z = entity.position.z + f64::from(dz);
                let Some(mut landing) = find_entity_landing_position(
                    world,
                    collision_cache,
                    &entity.dimension,
                    x,
                    entity.position.y + 8.0,
                    z,
                    16,
                ) else {
                    continue;
                };
                landing.yaw = entity.position.yaw;
                landing.pitch = entity.position.pitch;
                entity.position = landing;
                return true;
            }
        }
    }
    false
}

fn apply_vanilla_ai_effects(
    entity: &ManagedEntity,
    viewers: &[crate::players::OnlinePlayer],
    target_entities: &[ManagedEntity],
    tick_ms: u64,
    attack_state: &mut HashMap<String, EntityAttackState>,
    creeper_state: &mut HashMap<String, EntityCreeperState>,
    player_damage_requests: &mut Vec<EntityPlayerDamageRequest>,
    player_potion_effect_requests: &mut Vec<EntityPlayerPotionEffectRequest>,
    entity_damage_requests: &mut Vec<EntityEntityDamageRequest>,
    projectile_spawns: &mut Vec<EntityProjectileSpawnRequest>,
    summon_spawns: &mut Vec<EntitySummonSpawnRequest>,
    explosion_block_requests: &mut Vec<EntityExplosionBlockRequest>,
) -> bool {
    let profile = resolved_vanilla_ai_profile(entity, 8.0);
    match profile.behavior {
        VanillaEntityBehavior::NeutralMelee if !neutral_entity_is_angry(entity) => {
            if has_vanilla_entity_target(entity, target_entities, profile.attack_range) {
                creeper_state.remove(&entity.key);
                apply_vanilla_melee_attack(
                    entity,
                    viewers,
                    target_entities,
                    tick_ms,
                    profile,
                    attack_state,
                    player_damage_requests,
                    player_potion_effect_requests,
                    entity_damage_requests,
                );
                return false;
            }
            attack_state.remove(&entity.key);
            creeper_state.remove(&entity.key);
            false
        }
        VanillaEntityBehavior::NeutralMelee | VanillaEntityBehavior::HostileMelee => {
            if enderman_is_passive_to_players(entity, viewers, profile) {
                attack_state.remove(&entity.key);
                creeper_state.remove(&entity.key);
                return false;
            }
            creeper_state.remove(&entity.key);
            apply_vanilla_melee_attack(
                entity,
                viewers,
                target_entities,
                tick_ms,
                profile,
                attack_state,
                player_damage_requests,
                player_potion_effect_requests,
                entity_damage_requests,
            );
            apply_vanilla_sonic_boom(
                entity,
                viewers,
                profile,
                attack_state,
                player_damage_requests,
            );
            false
        }
        VanillaEntityBehavior::Creeper => {
            attack_state.remove(&entity.key);
            apply_vanilla_creeper_swell(
                entity,
                viewers,
                tick_ms,
                profile,
                creeper_state,
                player_damage_requests,
                target_entities,
                entity_damage_requests,
                explosion_block_requests,
            )
        }
        VanillaEntityBehavior::HostileRanged => {
            creeper_state.remove(&entity.key);
            apply_vanilla_ranged_attack(
                entity,
                viewers,
                target_entities,
                tick_ms,
                profile,
                attack_state,
                entity_damage_requests,
                projectile_spawns,
            );
            false
        }
        VanillaEntityBehavior::NeutralRanged if !neutral_entity_is_angry(entity) => {
            if has_vanilla_entity_target(entity, target_entities, profile.ranged_attack_range) {
                creeper_state.remove(&entity.key);
                apply_vanilla_ranged_attack(
                    entity,
                    viewers,
                    target_entities,
                    tick_ms,
                    profile,
                    attack_state,
                    entity_damage_requests,
                    projectile_spawns,
                );
                return false;
            }
            attack_state.remove(&entity.key);
            creeper_state.remove(&entity.key);
            false
        }
        VanillaEntityBehavior::NeutralRanged => {
            creeper_state.remove(&entity.key);
            apply_vanilla_ranged_attack(
                entity,
                viewers,
                target_entities,
                tick_ms,
                profile,
                attack_state,
                entity_damage_requests,
                projectile_spawns,
            );
            false
        }
        VanillaEntityBehavior::Spellcaster => {
            creeper_state.remove(&entity.key);
            apply_vanilla_spellcaster(
                entity,
                viewers,
                tick_ms,
                profile,
                attack_state,
                player_damage_requests,
                summon_spawns,
            );
            false
        }
        VanillaEntityBehavior::GuardianBeam => {
            creeper_state.remove(&entity.key);
            apply_vanilla_guardian_beam(
                entity,
                viewers,
                tick_ms,
                profile,
                attack_state,
                player_damage_requests,
            );
            false
        }
        _ => {
            attack_state.remove(&entity.key);
            creeper_state.remove(&entity.key);
            false
        }
    }
}

fn apply_vanilla_melee_attack(
    entity: &ManagedEntity,
    viewers: &[crate::players::OnlinePlayer],
    target_entities: &[ManagedEntity],
    tick_ms: u64,
    profile: VanillaEntityAiProfile,
    attack_state: &mut HashMap<String, EntityAttackState>,
    player_damage_requests: &mut Vec<EntityPlayerDamageRequest>,
    player_potion_effect_requests: &mut Vec<EntityPlayerPotionEffectRequest>,
    entity_damage_requests: &mut Vec<EntityEntityDamageRequest>,
) {
    let state = attack_state.entry(entity.key.clone()).or_default();
    state.cooldown_ticks = state
        .cooldown_ticks
        .saturating_sub(entity_tick_units(tick_ms));
    if state.cooldown_ticks > 0 || profile.attack_damage <= 0.0 {
        return;
    }
    if let Some(target) =
        nearest_vanilla_entity_melee_target(entity, target_entities, profile.attack_range)
    {
        entity_damage_requests.push(EntityEntityDamageRequest {
            target_entity_id: target.entity_id,
            amount: profile.attack_damage,
        });
        state.cooldown_ticks = profile.attack_interval_ticks;
        return;
    }
    let Some(target) = nearest_attackable_player(
        entity.position,
        &entity.dimension,
        viewers,
        profile.attack_range,
    ) else {
        return;
    };
    player_damage_requests.push(EntityPlayerDamageRequest {
        target_profile_id: target.profile.uuid,
        amount: profile.attack_damage,
        kind: crate::players::PlayerDamageKind::MobAttack,
        source_entity_id: entity.entity_id,
        source_position: entity.position,
        knockback: profile.attack_knockback,
    });
    if let Some(effect) = profile.melee_effect.filter(|_| melee_effect_roll(profile)) {
        player_potion_effect_requests.push(EntityPlayerPotionEffectRequest {
            target_profile_id: target.profile.uuid,
            effect: effect.effect.name(),
            amplifier: effect.amplifier,
            duration_ticks: effect.duration_ticks,
            source_entity_id: entity.entity_id,
            source_position: entity.position,
            knockback: 0.0,
        });
    }
    state.cooldown_ticks = profile.attack_interval_ticks;
}

fn apply_vanilla_sonic_boom(
    entity: &ManagedEntity,
    viewers: &[crate::players::OnlinePlayer],
    profile: VanillaEntityAiProfile,
    attack_state: &mut HashMap<String, EntityAttackState>,
    player_damage_requests: &mut Vec<EntityPlayerDamageRequest>,
) {
    if profile.spell_damage <= 0.0
        || normalized_entity_type(&entity.entity_type) != "minecraft:warden"
    {
        return;
    }
    let state = attack_state.entry(entity.key.clone()).or_default();
    if state.cooldown_ticks > 0 {
        return;
    }
    let Some(target) = nearest_attackable_player_3d(
        entity.position,
        &entity.dimension,
        viewers,
        profile.ranged_attack_range,
    ) else {
        return;
    };
    if distance_sq(entity.position, target.position) <= profile.attack_range * profile.attack_range
    {
        return;
    }
    player_damage_requests.push(EntityPlayerDamageRequest {
        target_profile_id: target.profile.uuid,
        amount: profile.spell_damage,
        kind: crate::players::PlayerDamageKind::Magic,
        source_entity_id: entity.entity_id,
        source_position: entity.position,
        knockback: 1.0,
    });
    state.cooldown_ticks = profile.ranged_attack_interval_ticks;
}

fn melee_effect_roll(profile: VanillaEntityAiProfile) -> bool {
    if profile.melee_effect_chance >= 1.0 {
        return true;
    }
    if profile.melee_effect_chance <= 0.0 {
        return false;
    }
    rand::Rng::gen_range(&mut rand::thread_rng(), 0.0..1.0) <= profile.melee_effect_chance
}

fn apply_vanilla_guardian_beam(
    entity: &ManagedEntity,
    viewers: &[crate::players::OnlinePlayer],
    tick_ms: u64,
    profile: VanillaEntityAiProfile,
    attack_state: &mut HashMap<String, EntityAttackState>,
    player_damage_requests: &mut Vec<EntityPlayerDamageRequest>,
) {
    let state = attack_state.entry(entity.key.clone()).or_default();
    let elapsed_ticks = entity_tick_units(tick_ms);
    if state.cooldown_ticks > 0 {
        state.cooldown_ticks = state.cooldown_ticks.saturating_sub(elapsed_ticks);
        return;
    }
    if profile.guardian_magic_damage <= 0.0 {
        state.charge_ticks = 0;
        state.target_profile_id = None;
        return;
    }

    let target = state
        .target_profile_id
        .and_then(|profile_id| {
            attackable_player_by_id(
                profile_id,
                entity.position,
                &entity.dimension,
                viewers,
                profile.ranged_attack_range,
            )
        })
        .or_else(|| {
            nearest_attackable_player_3d(
                entity.position,
                &entity.dimension,
                viewers,
                profile.ranged_attack_range,
            )
        });
    let Some(target) = target else {
        state.charge_ticks = 0;
        state.target_profile_id = None;
        return;
    };

    state.target_profile_id = Some(target.profile.uuid);
    state.charge_ticks = state.charge_ticks.saturating_add(elapsed_ticks);
    if state.charge_ticks < profile.guardian_attack_duration_ticks {
        return;
    }

    player_damage_requests.push(EntityPlayerDamageRequest {
        target_profile_id: target.profile.uuid,
        amount: profile.guardian_magic_damage,
        kind: crate::players::PlayerDamageKind::Magic,
        source_entity_id: entity.entity_id,
        source_position: entity.position,
        knockback: 0.0,
    });
    state.charge_ticks = 0;
    state.target_profile_id = None;
    state.cooldown_ticks = profile.ranged_attack_interval_ticks;
}

fn apply_vanilla_spellcaster(
    entity: &ManagedEntity,
    viewers: &[crate::players::OnlinePlayer],
    tick_ms: u64,
    profile: VanillaEntityAiProfile,
    attack_state: &mut HashMap<String, EntityAttackState>,
    player_damage_requests: &mut Vec<EntityPlayerDamageRequest>,
    summon_spawns: &mut Vec<EntitySummonSpawnRequest>,
) {
    let state = attack_state.entry(entity.key.clone()).or_default();
    state.cooldown_ticks = state
        .cooldown_ticks
        .saturating_sub(entity_tick_units(tick_ms));
    if state.cooldown_ticks > 0 {
        return;
    }
    let Some(target) = nearest_attackable_player_3d(
        entity.position,
        &entity.dimension,
        viewers,
        profile.ranged_attack_range,
    ) else {
        return;
    };

    if profile.spell_damage > 0.0 {
        add_evoker_fangs_spell(
            entity,
            target,
            profile,
            player_damage_requests,
            summon_spawns,
        );
    }
    add_spell_summons(entity, profile, summon_spawns);
    state.cooldown_ticks = profile.ranged_attack_interval_ticks;
}

fn add_evoker_fangs_spell(
    entity: &ManagedEntity,
    target: &crate::players::OnlinePlayer,
    profile: VanillaEntityAiProfile,
    player_damage_requests: &mut Vec<EntityPlayerDamageRequest>,
    summon_spawns: &mut Vec<EntitySummonSpawnRequest>,
) {
    player_damage_requests.push(EntityPlayerDamageRequest {
        target_profile_id: target.profile.uuid,
        amount: profile.spell_damage,
        kind: crate::players::PlayerDamageKind::Magic,
        source_entity_id: entity.entity_id,
        source_position: entity.position,
        knockback: 0.25,
    });
    let mut position = target.position;
    position.yaw = entity.position.yaw;
    position.pitch = 0.0;
    summon_spawns.push(EntitySummonSpawnRequest {
        key: format!(
            "summon:{}:fangs:{}",
            entity.key,
            rand::RngCore::next_u64(&mut rand::thread_rng())
        ),
        entity_type: "minecraft:evoker_fangs".to_string(),
        dimension: entity.dimension.clone(),
        position,
        ai: "none".to_string(),
        ai_params: BTreeMap::new(),
        display_name: String::new(),
        lifetime_ticks: Some(ENTITY_DEFAULT_SUMMON_LIFETIME_TICKS),
    });
}

fn add_spell_summons(
    entity: &ManagedEntity,
    profile: VanillaEntityAiProfile,
    summon_spawns: &mut Vec<EntitySummonSpawnRequest>,
) {
    let Some(kind) = profile.summon_kind else {
        return;
    };
    if profile.summon_count <= 0 || profile.summon_chance <= 0.0 {
        return;
    }
    if rand::Rng::gen_range(&mut rand::thread_rng(), 0.0..1.0) > profile.summon_chance {
        return;
    }
    for index in 0..profile.summon_count {
        summon_spawns.push(spell_summon_request(entity, kind, profile, index));
    }
}

fn spell_summon_request(
    entity: &ManagedEntity,
    kind: EntitySummonKind,
    profile: VanillaEntityAiProfile,
    index: i32,
) -> EntitySummonSpawnRequest {
    let angle = rand::Rng::gen_range(&mut rand::thread_rng(), 0.0..std::f64::consts::TAU);
    let radius = rand::Rng::gen_range(&mut rand::thread_rng(), 1.0..3.0);
    let mut position = entity.position;
    position.x += angle.cos() * radius;
    position.z += angle.sin() * radius;
    position.y += if matches!(kind, EntitySummonKind::Vex) {
        1.0
    } else {
        0.0
    };
    position.yaw = rand::Rng::gen_range(&mut rand::thread_rng(), -180.0..180.0);
    position.pitch = 0.0;
    position.on_ground = !matches!(kind, EntitySummonKind::Vex);
    let (entity_type, ai, display_name) = match kind {
        EntitySummonKind::EvokerFangs => ("minecraft:evoker_fangs", "none", ""),
        EntitySummonKind::Vex => ("minecraft:vex", "vanilla", "Vex"),
    };
    EntitySummonSpawnRequest {
        key: format!(
            "summon:{}:{}:{}:{}",
            entity.key,
            entity_type.trim_start_matches("minecraft:"),
            index,
            rand::RngCore::next_u64(&mut rand::thread_rng())
        ),
        entity_type: entity_type.to_string(),
        dimension: entity.dimension.clone(),
        position,
        ai: ai.to_string(),
        ai_params: BTreeMap::new(),
        display_name: display_name.to_string(),
        lifetime_ticks: Some(profile.summon_lifetime_ticks),
    }
}

fn attackable_player_by_id<'a>(
    profile_id: uuid::Uuid,
    source: EntityPosition,
    dimension: &str,
    viewers: &'a [crate::players::OnlinePlayer],
    range: f64,
) -> Option<&'a crate::players::OnlinePlayer> {
    let range_sq = range * range;
    viewers.iter().find(|player| {
        player.profile.uuid == profile_id
            && player.dimension == dimension
            && player_can_be_attacked(player)
            && distance_sq(source, player.position) <= range_sq
    })
}

fn apply_vanilla_ranged_attack(
    entity: &ManagedEntity,
    viewers: &[crate::players::OnlinePlayer],
    target_entities: &[ManagedEntity],
    tick_ms: u64,
    profile: VanillaEntityAiProfile,
    attack_state: &mut HashMap<String, EntityAttackState>,
    entity_damage_requests: &mut Vec<EntityEntityDamageRequest>,
    projectile_spawns: &mut Vec<EntityProjectileSpawnRequest>,
) {
    let Some(projectile_kind) = vanilla_ranged_projectile_kind(entity, profile) else {
        attack_state.remove(&entity.key);
        return;
    };
    let state = attack_state.entry(entity.key.clone()).or_default();
    state.cooldown_ticks = state
        .cooldown_ticks
        .saturating_sub(entity_tick_units(tick_ms));
    if state.cooldown_ticks > 0 || !vanilla_ranged_attack_enabled(projectile_kind, profile) {
        return;
    }
    if let Some(target) =
        nearest_vanilla_entity_target(entity, target_entities, profile.ranged_attack_range)
    {
        let spawn = ranged_projectile_spawn_request_at(
            entity,
            target.position,
            None,
            None,
            projectile_kind,
            profile,
        );
        projectile_spawns.push(spawn);
        if profile.ranged_attack_damage > 0.0 {
            entity_damage_requests.push(EntityEntityDamageRequest {
                target_entity_id: target.entity_id,
                amount: profile.ranged_attack_damage,
            });
        }
        state.cooldown_ticks = profile.ranged_attack_interval_ticks;
        return;
    }
    let Some(target) = nearest_attackable_player_3d(
        entity.position,
        &entity.dimension,
        viewers,
        profile.ranged_attack_range,
    ) else {
        return;
    };
    let spawn = ranged_projectile_spawn_request(entity, target, projectile_kind, profile);
    projectile_spawns.push(spawn);
    state.cooldown_ticks = profile.ranged_attack_interval_ticks;
}

fn vanilla_ranged_attack_enabled(
    projectile_kind: EntityProjectileKind,
    profile: VanillaEntityAiProfile,
) -> bool {
    profile.ranged_attack_damage > 0.0
        || matches!(projectile_kind, EntityProjectileKind::Potion)
        || matches!(projectile_kind, EntityProjectileKind::Snowball)
        || (profile.projectile_explosion_radius > 0.0 && profile.projectile_explosion_damage > 0.0)
}

fn vanilla_ranged_projectile_kind(
    entity: &ManagedEntity,
    profile: VanillaEntityAiProfile,
) -> Option<EntityProjectileKind> {
    match normalized_entity_type(&entity.entity_type).as_str() {
        "minecraft:skeleton"
        | "minecraft:stray"
        | "minecraft:bogged"
        | "minecraft:pillager"
        | "minecraft:illusioner" => Some(EntityProjectileKind::Arrow),
        "minecraft:drowned"
            if ai_param_bool(
                &entity.ai_params,
                "drowned_has_trident",
                ai_param_bool(&entity.ai_params, "has_trident", false),
            ) =>
        {
            Some(EntityProjectileKind::Trident)
        }
        "minecraft:witch" => Some(EntityProjectileKind::Potion),
        "minecraft:blaze" => Some(EntityProjectileKind::SmallFireball),
        "minecraft:ghast" => Some(EntityProjectileKind::Fireball),
        "minecraft:shulker" => Some(EntityProjectileKind::ShulkerBullet),
        "minecraft:breeze" => Some(EntityProjectileKind::WindCharge),
        "minecraft:wither" => Some(EntityProjectileKind::WitherSkull),
        "minecraft:snow_golem" if profile.ranged_attack_damage >= 0.0 => {
            Some(EntityProjectileKind::Snowball)
        }
        _ => None,
    }
}

fn ranged_projectile_spawn_request(
    entity: &ManagedEntity,
    target: &crate::players::OnlinePlayer,
    projectile_kind: EntityProjectileKind,
    profile: VanillaEntityAiProfile,
) -> EntityProjectileSpawnRequest {
    ranged_projectile_spawn_request_at(
        entity,
        target.position,
        projectile_potion_effect(entity, target, projectile_kind),
        projectile_homing_target(projectile_kind, target),
        projectile_kind,
        profile,
    )
}

fn ranged_projectile_spawn_request_at(
    entity: &ManagedEntity,
    target: EntityPosition,
    potion_effect: Option<EntityPotionEffect>,
    homing_target: Option<uuid::Uuid>,
    projectile_kind: EntityProjectileKind,
    profile: VanillaEntityAiProfile,
) -> EntityProjectileSpawnRequest {
    let (source_y_offset, target_y_offset, arc_boost) = projectile_aim_offsets(projectile_kind);
    let source = EntityPosition {
        x: entity.position.x,
        y: entity.position.y + source_y_offset,
        z: entity.position.z,
        yaw: entity.position.yaw,
        pitch: entity.position.pitch,
        on_ground: false,
    };
    let target_position = EntityPosition {
        x: target.x,
        y: target.y + target_y_offset,
        z: target.z,
        yaw: target.yaw,
        pitch: target.pitch,
        on_ground: target.on_ground,
    };
    let dx = target_position.x - source.x;
    let dz = target_position.z - source.z;
    let horizontal = (dx * dx + dz * dz).sqrt();
    let dy = target_position.y - source.y + horizontal * arc_boost;
    let length = (dx * dx + dy * dy + dz * dz).sqrt().max(0.0001);
    let speed = profile.projectile_speed;
    let velocity_x = dx / length * speed;
    let velocity_y = dy / length * speed;
    let velocity_z = dz / length * speed;
    let (yaw, pitch) = rotation_from_velocity(velocity_x, velocity_y, velocity_z);
    EntityProjectileSpawnRequest {
        key: format!(
            "projectile:{}:{}",
            entity.key,
            rand::RngCore::next_u64(&mut rand::thread_rng())
        ),
        entity_type: projectile_kind.entity_type().to_string(),
        dimension: entity.dimension.clone(),
        position: EntityPosition {
            yaw,
            pitch,
            ..source
        },
        motion: EntityMotion {
            velocity_x,
            velocity_y,
            velocity_z,
            ..Default::default()
        },
        source_entity_id: entity.entity_id,
        kind: projectile_kind,
        damage: profile.ranged_attack_damage,
        damage_kind: projectile_kind.damage_kind(),
        knockback: profile.attack_knockback,
        gravity_per_tick: profile.projectile_gravity_per_tick,
        hit_radius: profile.projectile_hit_radius,
        lifetime_ticks: profile.projectile_lifetime_ticks,
        explosion_radius: profile.projectile_explosion_radius,
        explosion_damage: profile.projectile_explosion_damage,
        explosion_break_blocks: profile.projectile_break_blocks,
        splash_radius: profile.projectile_splash_radius,
        potion_effect,
        target_profile_id: homing_target,
    }
}

fn projectile_homing_target(
    projectile_kind: EntityProjectileKind,
    target: &crate::players::OnlinePlayer,
) -> Option<uuid::Uuid> {
    matches!(projectile_kind, EntityProjectileKind::ShulkerBullet).then_some(target.profile.uuid)
}

fn projectile_aim_offsets(projectile_kind: EntityProjectileKind) -> (f64, f64, f64) {
    match projectile_kind {
        EntityProjectileKind::Arrow => (1.45, 1.35, 0.03),
        EntityProjectileKind::Potion => (1.4, 0.8, 0.20),
        EntityProjectileKind::SmallFireball => (1.2, 1.0, 0.0),
        EntityProjectileKind::Fireball => (1.5, 1.0, 0.0),
        EntityProjectileKind::ShulkerBullet => (0.5, 1.0, 0.0),
        EntityProjectileKind::WindCharge => (1.0, 1.0, 0.0),
        EntityProjectileKind::WitherSkull => (2.4, 1.2, 0.0),
        EntityProjectileKind::Trident => (1.45, 1.35, 0.02),
        EntityProjectileKind::Snowball => (1.4, 1.0, 0.04),
    }
}

fn projectile_potion_effect(
    entity: &ManagedEntity,
    target: &crate::players::OnlinePlayer,
    projectile_kind: EntityProjectileKind,
) -> Option<EntityPotionEffect> {
    match projectile_kind {
        EntityProjectileKind::Potion => Some(witch_potion_effect(entity, target)),
        EntityProjectileKind::ShulkerBullet => Some(shulker_bullet_effect(entity)),
        _ => None,
    }
}

fn shulker_bullet_effect(entity: &ManagedEntity) -> EntityPotionEffect {
    EntityPotionEffect {
        effect: EntityPotionEffectKind::Levitation,
        amplifier: ai_param_i32(&entity.ai_params, "shulker_levitation_amplifier", 0).clamp(0, 16),
        duration_ticks: ai_param_i32(
            &entity.ai_params,
            "shulker_levitation_duration_ticks",
            EntityPotionEffectKind::Levitation.default_duration_ticks(),
        )
        .clamp(1, 20 * 60 * 10),
    }
}

fn witch_potion_effect(
    entity: &ManagedEntity,
    target: &crate::players::OnlinePlayer,
) -> EntityPotionEffect {
    let effect = ai_param_string(&entity.ai_params, "witch_potion_effect")
        .or_else(|| ai_param_string(&entity.ai_params, "potion_effect"))
        .and_then(|value| potion_effect_kind(&value))
        .unwrap_or_else(|| {
            if horizontal_distance_sq(entity.position, target.position) >= 64.0 {
                EntityPotionEffectKind::Slowness
            } else {
                EntityPotionEffectKind::InstantDamage
            }
        });
    let default_amplifier = match effect {
        EntityPotionEffectKind::InstantDamage => ENTITY_WITCH_INSTANT_DAMAGE_AMPLIFIER,
        _ => 0,
    };
    EntityPotionEffect {
        effect,
        amplifier: ai_param_i32(
            &entity.ai_params,
            "witch_potion_amplifier",
            default_amplifier,
        )
        .clamp(0, 16),
        duration_ticks: ai_param_i32(
            &entity.ai_params,
            "witch_potion_duration_ticks",
            effect.default_duration_ticks(),
        )
        .clamp(1, 20 * 60 * 10),
    }
}

fn potion_effect_kind(value: &str) -> Option<EntityPotionEffectKind> {
    match normalized_entity_type(value).as_str() {
        "minecraft:harming" | "minecraft:instant_damage" | "minecraft:damage" => {
            Some(EntityPotionEffectKind::InstantDamage)
        }
        "minecraft:hunger" => Some(EntityPotionEffectKind::Hunger),
        "minecraft:levitation" | "minecraft:levitate" => Some(EntityPotionEffectKind::Levitation),
        "minecraft:poison" => Some(EntityPotionEffectKind::Poison),
        "minecraft:slowness" | "minecraft:slow" => Some(EntityPotionEffectKind::Slowness),
        "minecraft:weakness" | "minecraft:weak" => Some(EntityPotionEffectKind::Weakness),
        "minecraft:wither" => Some(EntityPotionEffectKind::Wither),
        _ => None,
    }
}

fn apply_vanilla_creeper_swell(
    entity: &ManagedEntity,
    viewers: &[crate::players::OnlinePlayer],
    tick_ms: u64,
    profile: VanillaEntityAiProfile,
    creeper_state: &mut HashMap<String, EntityCreeperState>,
    player_damage_requests: &mut Vec<EntityPlayerDamageRequest>,
    target_entities: &[ManagedEntity],
    entity_damage_requests: &mut Vec<EntityEntityDamageRequest>,
    explosion_block_requests: &mut Vec<EntityExplosionBlockRequest>,
) -> bool {
    let elapsed_ticks = entity_tick_units(tick_ms);
    let state = creeper_state.entry(entity.key.clone()).or_default();
    let target = nearest_attackable_player(
        entity.position,
        &entity.dimension,
        viewers,
        profile.follow_range,
    );
    let target_distance_sq = target
        .map(|target| distance_sq(entity.position, target.position))
        .unwrap_or(f64::INFINITY);
    if target_distance_sq > 49.0 {
        state.swell_ticks = state.swell_ticks.saturating_sub(elapsed_ticks);
        if state.swell_ticks <= 0 {
            creeper_state.remove(&entity.key);
        }
        return false;
    }
    if vanilla_avoidance_threat(entity, target_entities).is_some() {
        state.swell_ticks = state.swell_ticks.saturating_sub(elapsed_ticks);
        if state.swell_ticks <= 0 {
            creeper_state.remove(&entity.key);
        }
        return false;
    }
    if state.swell_ticks <= 0 && target_distance_sq >= 9.0 {
        return false;
    }

    state.swell_ticks = state
        .swell_ticks
        .saturating_add(elapsed_ticks)
        .min(profile.creeper_swell_ticks);
    if state.swell_ticks < profile.creeper_swell_ticks {
        return false;
    }

    add_creeper_explosion_damage_requests(entity, viewers, profile, player_damage_requests);
    add_explosion_entity_damage_requests(
        entity.entity_id,
        entity.position,
        profile.creeper_explosion_radius,
        profile.creeper_explosion_damage,
        target_entities,
        entity_damage_requests,
    );
    if profile.creeper_break_blocks {
        explosion_block_requests.push(EntityExplosionBlockRequest {
            dimension: entity.dimension.clone(),
            center: entity.position,
            radius: profile.creeper_explosion_radius,
        });
    }
    creeper_state.remove(&entity.key);
    true
}

fn add_creeper_explosion_damage_requests(
    entity: &ManagedEntity,
    viewers: &[crate::players::OnlinePlayer],
    profile: VanillaEntityAiProfile,
    player_damage_requests: &mut Vec<EntityPlayerDamageRequest>,
) {
    for player in viewers {
        if player.dimension != entity.dimension || !player_can_be_attacked(player) {
            continue;
        }
        let distance = distance_sq(entity.position, player.position).sqrt();
        if distance > profile.creeper_explosion_radius {
            continue;
        }
        let exposure = (1.0 - distance / profile.creeper_explosion_radius).clamp(0.0, 1.0);
        let scaled_damage =
            ((exposure * exposure + exposure) * 0.5 * 7.0 * profile.creeper_explosion_radius * 2.0
                + 1.0) as f32;
        let amount = scaled_damage.min(profile.creeper_explosion_damage).max(0.0);
        if amount <= 0.0 {
            continue;
        }
        player_damage_requests.push(EntityPlayerDamageRequest {
            target_profile_id: player.profile.uuid,
            amount,
            kind: crate::players::PlayerDamageKind::Explosion,
            source_entity_id: entity.entity_id,
            source_position: entity.position,
            knockback: (exposure * 1.2) as f32,
        });
    }
}

fn add_explosion_entity_damage_requests(
    source_entity_id: i32,
    center: EntityPosition,
    radius: f64,
    max_damage: f32,
    target_entities: &[ManagedEntity],
    entity_damage_requests: &mut Vec<EntityEntityDamageRequest>,
) {
    if radius <= 0.0 || max_damage <= 0.0 {
        return;
    }
    let radius = radius.max(0.1);
    for target in target_entities {
        if target.entity_id == source_entity_id || target.kind != ManagedEntityKind::Entity {
            continue;
        }
        let distance = distance_sq(center, target.position).sqrt();
        if distance > radius {
            continue;
        }
        let exposure = (1.0 - distance / radius).clamp(0.0, 1.0);
        let amount = ((exposure * exposure + exposure) * 0.5 * 7.0 * radius * 2.0 + 1.0) as f32;
        let amount = amount.min(max_damage).max(0.0);
        if amount <= 0.0 {
            continue;
        }
        entity_damage_requests.push(EntityEntityDamageRequest {
            target_entity_id: target.entity_id,
            amount,
        });
    }
}

fn apply_explosion_block_requests(
    players: &crate::players::PlayerManager,
    world: &crate::world::WorldManager,
    world_rules: Option<&crate::world::WorldRulesManager>,
    requests: Vec<EntityExplosionBlockRequest>,
) -> Result<()> {
    for request in requests {
        if !explosion_block_updates_enabled(world_rules, &request.dimension) {
            continue;
        }
        let blocks = explosion_air_blocks(world, &request);
        let updates = world.place_blocks(&request.dimension, blocks)?;
        for update in updates {
            players.broadcast_block_changed(
                uuid::Uuid::nil(),
                &request.dimension,
                update.location,
                update.block_state.0,
                None,
            );
        }
    }
    Ok(())
}

fn explosion_block_updates_enabled(
    world_rules: Option<&crate::world::WorldRulesManager>,
    dimension: &str,
) -> bool {
    world_rules.is_none_or(|rules| rules.snapshot(dimension).block_updates)
}

fn explosion_air_blocks(
    world: &crate::world::WorldManager,
    request: &EntityExplosionBlockRequest,
) -> Vec<(BlockPosition, i32)> {
    let radius = request.radius.clamp(0.1, 16.0);
    let min_x = (request.center.x - radius).floor() as i32;
    let max_x = (request.center.x + radius).floor() as i32;
    let min_y = (request.center.y - radius).floor() as i32;
    let max_y = (request.center.y + radius).floor() as i32;
    let min_z = (request.center.z - radius).floor() as i32;
    let max_z = (request.center.z + radius).floor() as i32;
    let air = crate::inventory::air_block_state();
    let mut blocks = Vec::new();
    for x in min_x..=max_x {
        for y in min_y..=max_y {
            for z in min_z..=max_z {
                let block_center = EntityPosition {
                    x: f64::from(x) + 0.5,
                    y: f64::from(y) + 0.5,
                    z: f64::from(z) + 0.5,
                    yaw: 0.0,
                    pitch: 0.0,
                    on_ground: false,
                };
                if distance_sq(request.center, block_center) > radius * radius {
                    continue;
                }
                let position = BlockPosition { x, y, z };
                let current = world
                    .block_state_at(&request.dimension, &position)
                    .unwrap_or(air);
                if explosion_can_break_block_state(current) {
                    blocks.push((position, air));
                }
            }
        }
    }
    blocks
}

fn explosion_can_break_block_state(block_state: i32) -> bool {
    if crate::inventory::is_air_block_state(block_state) {
        return false;
    }
    let Some(name) = crate::inventory::block_name_for_state(block_state) else {
        return true;
    };
    !matches!(
        name.as_str(),
        "minecraft:bedrock"
            | "minecraft:barrier"
            | "minecraft:command_block"
            | "minecraft:chain_command_block"
            | "minecraft:repeating_command_block"
            | "minecraft:end_portal"
            | "minecraft:end_portal_frame"
            | "minecraft:structure_block"
            | "minecraft:jigsaw"
    )
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
    apply_follow_nearest_player_with(
        entity,
        viewers,
        tick_ms,
        target_memory,
        path_memory,
        target_reselects,
        path_recalcs,
        world,
        collision_cache,
        now,
        32.0,
        2.0,
        0.22,
    )
}

#[allow(clippy::too_many_arguments)]
fn apply_follow_nearest_player_with(
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
    follow_range: f64,
    stop_distance: f64,
    base_step: f64,
) -> EntityMovement {
    let Some(target) = preferred_follow_target(
        entity,
        viewers,
        target_memory,
        target_reselects,
        now,
        follow_range.max(0.0),
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
    if target_horizontal <= stop_distance.max(0.0) {
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
    let speed = base_step.max(0.0).min(horizontal);
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

fn nearest_attackable_player<'a>(
    position: EntityPosition,
    dimension: &str,
    viewers: &'a [crate::players::OnlinePlayer],
    range: f64,
) -> Option<&'a crate::players::OnlinePlayer> {
    let range_sq = range.max(0.0) * range.max(0.0);
    viewers
        .iter()
        .filter(|player| player.dimension == dimension && player_can_be_attacked(player))
        .filter(|player| (player.position.y - position.y).abs() <= 2.5)
        .filter(|player| horizontal_distance_sq(position, player.position) <= range_sq)
        .min_by(|left, right| {
            distance_sq(position, left.position).total_cmp(&distance_sq(position, right.position))
        })
}

fn nearest_attackable_player_3d<'a>(
    position: EntityPosition,
    dimension: &str,
    viewers: &'a [crate::players::OnlinePlayer],
    range: f64,
) -> Option<&'a crate::players::OnlinePlayer> {
    let range_sq = range.max(0.0) * range.max(0.0);
    viewers
        .iter()
        .filter(|player| player.dimension == dimension && player_can_be_attacked(player))
        .filter(|player| distance_sq(position, player.position) <= range_sq)
        .min_by(|left, right| {
            distance_sq(position, left.position).total_cmp(&distance_sq(position, right.position))
        })
}

fn has_vanilla_entity_target(
    attacker: &ManagedEntity,
    candidates: &[ManagedEntity],
    range: f64,
) -> bool {
    nearest_vanilla_entity_target(attacker, candidates, range).is_some()
}

fn nearest_vanilla_entity_target<'a>(
    attacker: &ManagedEntity,
    candidates: &'a [ManagedEntity],
    range: f64,
) -> Option<&'a ManagedEntity> {
    if ai_param_bool(&attacker.ai_params, "disable_entity_targets", false) {
        return None;
    }
    let range_sq = range.max(0.0) * range.max(0.0);
    candidates
        .iter()
        .filter(|target| {
            target.kind == ManagedEntityKind::Entity
                && target.key != attacker.key
                && target.dimension == attacker.dimension
                && vanilla_entity_can_attack(attacker, target)
                && distance_sq(attacker.position, target.position) <= range_sq
        })
        .min_by(|left, right| {
            distance_sq(attacker.position, left.position)
                .total_cmp(&distance_sq(attacker.position, right.position))
        })
}

fn nearest_vanilla_entity_melee_target<'a>(
    attacker: &ManagedEntity,
    candidates: &'a [ManagedEntity],
    range: f64,
) -> Option<&'a ManagedEntity> {
    if ai_param_bool(&attacker.ai_params, "disable_entity_targets", false) {
        return None;
    }
    let range_sq = range.max(0.0) * range.max(0.0);
    candidates
        .iter()
        .filter(|target| {
            target.kind == ManagedEntityKind::Entity
                && target.key != attacker.key
                && target.dimension == attacker.dimension
                && vanilla_entity_can_attack(attacker, target)
                && (target.position.y - attacker.position.y).abs() <= 2.5
                && horizontal_distance_sq(attacker.position, target.position) <= range_sq
        })
        .min_by(|left, right| {
            distance_sq(attacker.position, left.position)
                .total_cmp(&distance_sq(attacker.position, right.position))
        })
}

fn vanilla_entity_can_attack(attacker: &ManagedEntity, target: &ManagedEntity) -> bool {
    let attacker_type = normalized_entity_type(&attacker.entity_type);
    let target_type = normalized_entity_type(&target.entity_type);
    if target_type == "minecraft:armor_stand"
        || target_type == "minecraft:item"
        || target_type.ends_with("_display")
    {
        return false;
    }
    if entity_type_is_villager_like(&target_type) {
        return entity_type_attacks_villagers(&attacker_type);
    }
    if entity_type_is_golem(&target_type) {
        return entity_type_attacks_golems(&attacker_type);
    }
    if entity_type_is_golem(&attacker_type) || attacker_type == "minecraft:snow_golem" {
        return entity_type_is_golem_target(&target_type);
    }
    if entity_type_attacks_prey(&attacker_type, &target_type) {
        return true;
    }
    if neutral_entity_is_angry(attacker) && entity_type_is_neutral_predator(&attacker_type) {
        return entity_type_is_golem_target(&target_type);
    }
    false
}

fn entity_type_is_villager_like(entity_type: &str) -> bool {
    matches!(
        entity_type,
        "minecraft:villager" | "minecraft:wandering_trader"
    )
}

fn entity_type_is_golem(entity_type: &str) -> bool {
    matches!(entity_type, "minecraft:iron_golem" | "minecraft:snow_golem")
}

fn entity_type_attacks_villagers(entity_type: &str) -> bool {
    matches!(
        entity_type,
        "minecraft:zombie"
            | "minecraft:zombie_villager"
            | "minecraft:husk"
            | "minecraft:drowned"
            | "minecraft:vindicator"
            | "minecraft:evoker"
            | "minecraft:pillager"
            | "minecraft:ravager"
            | "minecraft:vex"
            | "minecraft:zoglin"
    )
}

fn entity_type_attacks_golems(entity_type: &str) -> bool {
    matches!(
        entity_type,
        "minecraft:zombie"
            | "minecraft:zombie_villager"
            | "minecraft:husk"
            | "minecraft:drowned"
            | "minecraft:vindicator"
            | "minecraft:evoker"
            | "minecraft:pillager"
            | "minecraft:ravager"
            | "minecraft:vex"
            | "minecraft:warden"
            | "minecraft:wither"
            | "minecraft:zoglin"
    )
}

fn entity_type_is_golem_target(entity_type: &str) -> bool {
    matches!(
        entity_type,
        "minecraft:zombie"
            | "minecraft:zombie_villager"
            | "minecraft:husk"
            | "minecraft:drowned"
            | "minecraft:skeleton"
            | "minecraft:stray"
            | "minecraft:bogged"
            | "minecraft:spider"
            | "minecraft:cave_spider"
            | "minecraft:witch"
            | "minecraft:slime"
            | "minecraft:magma_cube"
            | "minecraft:blaze"
            | "minecraft:guardian"
            | "minecraft:elder_guardian"
            | "minecraft:pillager"
            | "minecraft:vindicator"
            | "minecraft:evoker"
            | "minecraft:ravager"
            | "minecraft:vex"
            | "minecraft:phantom"
            | "minecraft:zoglin"
            | "minecraft:piglin_brute"
            | "minecraft:warden"
    )
}

fn entity_type_is_neutral_predator(entity_type: &str) -> bool {
    matches!(
        entity_type,
        "minecraft:wolf" | "minecraft:bee" | "minecraft:polar_bear"
    )
}

fn entity_type_attacks_prey(attacker_type: &str, target_type: &str) -> bool {
    match attacker_type {
        "minecraft:fox" => matches!(
            target_type,
            "minecraft:chicken"
                | "minecraft:rabbit"
                | "minecraft:cod"
                | "minecraft:salmon"
                | "minecraft:tropical_fish"
                | "minecraft:pufferfish"
        ),
        "minecraft:wolf" => matches!(
            target_type,
            "minecraft:sheep"
                | "minecraft:rabbit"
                | "minecraft:fox"
                | "minecraft:skeleton"
                | "minecraft:stray"
                | "minecraft:bogged"
                | "minecraft:wither_skeleton"
        ),
        _ => false,
    }
}

fn player_can_be_attacked(player: &crate::players::OnlinePlayer) -> bool {
    !matches!(player.game_mode, 1 | 3)
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
    let elapsed_ticks = entity_tick_units(tick_ms);
    let profile = resolved_vanilla_ai_profile(entity, 8.0);
    let ignores_gravity = entity_ignores_gravity_for_profile(entity, world, profile);
    motion.jump_cooldown_ticks = motion.jump_cooldown_ticks.saturating_sub(elapsed_ticks);
    apply_horizontal_motion(motion, movement, tick_scale);
    let dx = (motion.velocity_x * tick_scale).clamp(-4.0, 4.0);
    let dz = (motion.velocity_z * tick_scale).clamp(-4.0, 4.0);

    if ignores_gravity {
        entity.position.on_ground = false;
        apply_vertical_motion(motion, movement, tick_scale);
        dy = (motion.velocity_y * tick_scale).clamp(-4.0, 4.0);
    } else if entity_has_ground(world, collision_cache, &entity.dimension, entity.position)
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
    if !ignores_gravity && try_apply_slime_jump(entity, motion, movement) {
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

fn try_apply_slime_jump(
    entity: &mut ManagedEntity,
    motion: &mut EntityMotion,
    movement: EntityMovement,
) -> bool {
    if !entity_type_uses_slime_jump(&entity.entity_type)
        || !entity.position.on_ground
        || motion.jump_cooldown_ticks > 0
        || movement.x.abs() + movement.z.abs() <= 0.0001
    {
        return false;
    }
    let profile = resolved_vanilla_ai_profile(entity, 8.0);
    if profile.slime_jump_velocity <= 0.0 {
        return false;
    }
    motion.velocity_y = profile.slime_jump_velocity;
    motion.jump_cooldown_ticks = if movement.x.abs() + movement.z.abs() > 0.0001 {
        (profile.slime_jump_delay_ticks / 3).max(1)
    } else {
        profile.slime_jump_delay_ticks
    };
    entity.position.on_ground = false;
    true
}

fn entity_type_uses_slime_jump(entity_type: &str) -> bool {
    matches!(
        normalized_entity_type(entity_type).as_str(),
        "minecraft:slime" | "minecraft:magma_cube"
    )
}

#[allow(clippy::too_many_arguments)]
fn apply_projectile_tick(
    entity: &mut ManagedEntity,
    world: &crate::world::WorldManager,
    collision_cache: &mut CollisionCache,
    motion: &mut EntityMotion,
    projectile_state: &mut HashMap<String, EntityProjectileState>,
    viewers: &[crate::players::OnlinePlayer],
    target_entities: &[ManagedEntity],
    tick_ms: u64,
    player_damage_requests: &mut Vec<EntityPlayerDamageRequest>,
    player_potion_effect_requests: &mut Vec<EntityPlayerPotionEffectRequest>,
    entity_damage_requests: &mut Vec<EntityEntityDamageRequest>,
    explosion_block_requests: &mut Vec<EntityExplosionBlockRequest>,
) -> bool {
    let Some(state) = projectile_state.get_mut(&entity.key) else {
        return true;
    };
    let elapsed_ticks = entity_tick_units(tick_ms);
    state.remaining_ticks = state.remaining_ticks.saturating_sub(elapsed_ticks);
    if state.remaining_ticks <= 0 {
        return true;
    }

    let previous = entity.position;
    let tick_scale = (tick_ms as f64 / 50.0).clamp(0.25, 4.0);
    apply_shulker_bullet_homing(entity, motion, state, viewers, tick_scale);
    motion.velocity_y -= state.gravity_per_tick * tick_scale;
    entity.position.x += motion.velocity_x * tick_scale;
    entity.position.y += motion.velocity_y * tick_scale;
    entity.position.z += motion.velocity_z * tick_scale;
    let (yaw, pitch) =
        rotation_from_velocity(motion.velocity_x, motion.velocity_y, motion.velocity_z);
    entity.position.yaw = yaw;
    entity.position.pitch = pitch;
    entity.position.on_ground = false;

    if projectile_hits_solid_block(entity, world, collision_cache) {
        apply_projectile_impact(
            entity,
            state,
            viewers,
            None,
            None,
            player_damage_requests,
            player_potion_effect_requests,
            entity_damage_requests,
            target_entities,
            explosion_block_requests,
        );
        return true;
    }

    if let Some(target) = projectile_hit_entity(
        previous,
        entity.position,
        &entity.key,
        state.source_entity_id,
        &entity.dimension,
        target_entities,
        state.hit_radius,
    ) {
        apply_projectile_impact(
            entity,
            state,
            viewers,
            None,
            Some(target.entity_id),
            player_damage_requests,
            player_potion_effect_requests,
            entity_damage_requests,
            target_entities,
            explosion_block_requests,
        );
        return true;
    }

    let Some(target) = projectile_hit_player(
        previous,
        entity.position,
        &entity.dimension,
        viewers,
        state.hit_radius,
    ) else {
        return false;
    };
    apply_projectile_impact(
        entity,
        state,
        viewers,
        Some(target.profile.uuid),
        None,
        player_damage_requests,
        player_potion_effect_requests,
        entity_damage_requests,
        target_entities,
        explosion_block_requests,
    );
    true
}

fn apply_shulker_bullet_homing(
    entity: &ManagedEntity,
    motion: &mut EntityMotion,
    state: &EntityProjectileState,
    viewers: &[crate::players::OnlinePlayer],
    tick_scale: f64,
) {
    if !matches!(state.kind, EntityProjectileKind::ShulkerBullet) {
        return;
    }
    let Some(target_profile_id) = state.target_profile_id else {
        return;
    };
    let Some(target) = viewers.iter().find(|player| {
        player.profile.uuid == target_profile_id
            && player.dimension == entity.dimension
            && player_can_be_attacked(player)
    }) else {
        return;
    };

    let dx = target.position.x - entity.position.x;
    let dy = target.position.y + 1.0 - entity.position.y;
    let dz = target.position.z - entity.position.z;
    let target_distance = (dx * dx + dy * dy + dz * dz).sqrt();
    let speed = (motion.velocity_x * motion.velocity_x
        + motion.velocity_y * motion.velocity_y
        + motion.velocity_z * motion.velocity_z)
        .sqrt();
    if target_distance <= f64::EPSILON || speed <= f64::EPSILON {
        return;
    }

    let desired_x = dx / target_distance * speed;
    let desired_y = dy / target_distance * speed;
    let desired_z = dz / target_distance * speed;
    let blend = 1.0 - (1.0 - ENTITY_SHULKER_BULLET_HOMING_BLEND).powf(tick_scale);
    let next_x = motion.velocity_x + (desired_x - motion.velocity_x) * blend;
    let next_y = motion.velocity_y + (desired_y - motion.velocity_y) * blend;
    let next_z = motion.velocity_z + (desired_z - motion.velocity_z) * blend;
    let next_speed = (next_x * next_x + next_y * next_y + next_z * next_z).sqrt();
    if next_speed <= f64::EPSILON {
        return;
    }

    let scale = speed / next_speed;
    motion.velocity_x = next_x * scale;
    motion.velocity_y = next_y * scale;
    motion.velocity_z = next_z * scale;
}

fn apply_projectile_impact(
    entity: &ManagedEntity,
    state: &EntityProjectileState,
    viewers: &[crate::players::OnlinePlayer],
    hit_target_id: Option<uuid::Uuid>,
    hit_entity_id: Option<i32>,
    player_damage_requests: &mut Vec<EntityPlayerDamageRequest>,
    player_potion_effect_requests: &mut Vec<EntityPlayerPotionEffectRequest>,
    entity_damage_requests: &mut Vec<EntityEntityDamageRequest>,
    target_entities: &[ManagedEntity],
    explosion_block_requests: &mut Vec<EntityExplosionBlockRequest>,
) {
    if state.explosion_radius > 0.0 {
        add_projectile_explosion_damage_requests(entity, state, viewers, player_damage_requests);
        add_explosion_entity_damage_requests(
            state.source_entity_id,
            entity.position,
            state.explosion_radius,
            state.explosion_damage,
            target_entities,
            entity_damage_requests,
        );
        if state.explosion_break_blocks {
            explosion_block_requests.push(EntityExplosionBlockRequest {
                dimension: entity.dimension.clone(),
                center: entity.position,
                radius: state.explosion_radius,
            });
        }
        return;
    }

    if let Some(effect) = state.potion_effect {
        add_projectile_potion_effect_requests(
            entity,
            state,
            viewers,
            hit_target_id,
            effect,
            player_potion_effect_requests,
        );
        add_projectile_entity_potion_damage_requests(
            entity,
            state,
            hit_entity_id,
            effect,
            target_entities,
            entity_damage_requests,
        );
        if matches!(state.kind, EntityProjectileKind::Potion) {
            return;
        }
    }

    if state.damage <= 0.0 {
        return;
    }
    if let Some(target_entity_id) = hit_entity_id {
        entity_damage_requests.push(EntityEntityDamageRequest {
            target_entity_id,
            amount: state.damage,
        });
        return;
    }
    let Some(target_profile_id) = hit_target_id else {
        return;
    };
    let knockback = match state.kind {
        EntityProjectileKind::SmallFireball => state.knockback.max(0.25),
        _ => state.knockback,
    };
    player_damage_requests.push(EntityPlayerDamageRequest {
        target_profile_id,
        amount: state.damage,
        kind: state.damage_kind,
        source_entity_id: state.source_entity_id,
        source_position: entity.position,
        knockback,
    });
}

fn add_projectile_explosion_damage_requests(
    entity: &ManagedEntity,
    state: &EntityProjectileState,
    viewers: &[crate::players::OnlinePlayer],
    player_damage_requests: &mut Vec<EntityPlayerDamageRequest>,
) {
    if state.explosion_damage <= 0.0 {
        return;
    }
    let radius = state.explosion_radius.max(0.1);
    for player in viewers {
        if player.dimension != entity.dimension || !player_can_be_attacked(player) {
            continue;
        }
        let distance = distance_sq(entity.position, player.position).sqrt();
        if distance > radius {
            continue;
        }
        let exposure = (1.0 - distance / radius).clamp(0.0, 1.0);
        let amount = ((exposure * exposure + exposure) * 0.5 * 7.0 * radius * 2.0 + 1.0) as f32;
        let amount = amount.min(state.explosion_damage).max(0.0);
        if amount <= 0.0 {
            continue;
        }
        player_damage_requests.push(EntityPlayerDamageRequest {
            target_profile_id: player.profile.uuid,
            amount,
            kind: crate::players::PlayerDamageKind::Explosion,
            source_entity_id: state.source_entity_id,
            source_position: entity.position,
            knockback: (exposure * 1.2) as f32,
        });
    }
}

fn add_projectile_potion_effect_requests(
    entity: &ManagedEntity,
    state: &EntityProjectileState,
    viewers: &[crate::players::OnlinePlayer],
    hit_target_id: Option<uuid::Uuid>,
    effect: EntityPotionEffect,
    player_potion_effect_requests: &mut Vec<EntityPlayerPotionEffectRequest>,
) {
    if state.splash_radius <= 0.0 {
        let Some(target_profile_id) = hit_target_id else {
            return;
        };
        player_potion_effect_requests.push(EntityPlayerPotionEffectRequest {
            target_profile_id,
            effect: effect.effect.name(),
            amplifier: effect.amplifier,
            duration_ticks: effect.duration_ticks,
            source_entity_id: state.source_entity_id,
            source_position: entity.position,
            knockback: state.knockback,
        });
        return;
    }

    let radius = state.splash_radius.max(0.1);
    for player in viewers {
        if player.dimension != entity.dimension || !player_can_be_attacked(player) {
            continue;
        }
        let distance = distance_sq(entity.position, player.position).sqrt();
        if distance > radius {
            continue;
        }
        let intensity = (1.0 - distance / radius).clamp(0.25, 1.0);
        let duration_ticks = if matches!(effect.effect, EntityPotionEffectKind::InstantDamage) {
            1
        } else {
            ((effect.duration_ticks as f64) * intensity).round() as i32
        }
        .max(1);
        player_potion_effect_requests.push(EntityPlayerPotionEffectRequest {
            target_profile_id: player.profile.uuid,
            effect: effect.effect.name(),
            amplifier: effect.amplifier,
            duration_ticks,
            source_entity_id: state.source_entity_id,
            source_position: entity.position,
            knockback: state.knockback,
        });
    }
}

fn add_projectile_entity_potion_damage_requests(
    entity: &ManagedEntity,
    state: &EntityProjectileState,
    hit_entity_id: Option<i32>,
    effect: EntityPotionEffect,
    target_entities: &[ManagedEntity],
    entity_damage_requests: &mut Vec<EntityEntityDamageRequest>,
) {
    if !matches!(effect.effect, EntityPotionEffectKind::InstantDamage) {
        return;
    }
    let amount = instant_damage_amount(effect);
    if amount <= 0.0 {
        return;
    }
    if state.splash_radius <= 0.0 {
        if let Some(target_entity_id) = hit_entity_id {
            entity_damage_requests.push(EntityEntityDamageRequest {
                target_entity_id,
                amount,
            });
        }
        return;
    }

    let radius = state.splash_radius.max(0.1);
    for target in target_entities {
        if !projectile_can_hit_managed_entity(target, &entity.key, state.source_entity_id) {
            continue;
        }
        if target.dimension != entity.dimension {
            continue;
        }
        let distance = distance_sq(entity.position, target.position).sqrt();
        if distance > radius {
            continue;
        }
        let intensity = (1.0 - distance / radius).clamp(0.25, 1.0);
        entity_damage_requests.push(EntityEntityDamageRequest {
            target_entity_id: target.entity_id,
            amount: amount * intensity as f32,
        });
    }
}

fn instant_damage_amount(effect: EntityPotionEffect) -> f32 {
    6.0 * (effect.amplifier.max(0) + 1) as f32
}

fn projectile_hits_solid_block(
    entity: &ManagedEntity,
    world: &crate::world::WorldManager,
    collision_cache: &mut CollisionCache,
) -> bool {
    entity_aabb_intersects_solid(
        world,
        collision_cache,
        &entity.dimension,
        entity.position.x,
        entity.position.y,
        entity.position.z,
        0.25,
        0.25,
    )
}

fn projectile_hit_entity<'a>(
    previous: EntityPosition,
    current: EntityPosition,
    projectile_key: &str,
    source_entity_id: i32,
    dimension: &str,
    target_entities: &'a [ManagedEntity],
    hit_radius: f64,
) -> Option<&'a ManagedEntity> {
    let hit_radius = hit_radius.clamp(0.1, 8.0);
    target_entities
        .iter()
        .filter(|target| {
            projectile_can_hit_managed_entity(target, projectile_key, source_entity_id)
        })
        .filter(|target| target.dimension == dimension)
        .filter(|target| {
            let target_point = projectile_entity_hit_point(target);
            let radius = hit_radius + projectile_entity_collision_radius(target);
            point_segment_distance_sq(target_point, previous, current) <= radius * radius
        })
        .min_by(|left, right| {
            distance_sq(current, left.position).total_cmp(&distance_sq(current, right.position))
        })
}

fn projectile_can_hit_managed_entity(
    target: &ManagedEntity,
    projectile_key: &str,
    source_entity_id: i32,
) -> bool {
    if target.kind != ManagedEntityKind::Entity
        || target.key == projectile_key
        || target.entity_id == source_entity_id
        || ai_kind(&target.ai) == EntityAiKind::Projectile
    {
        return false;
    }
    let entity_type = normalized_entity_type(&target.entity_type);
    !matches!(entity_type.as_str(), "minecraft:item" | "minecraft:marker")
        && !entity_type.ends_with("_display")
}

fn projectile_entity_hit_point(target: &ManagedEntity) -> EntityPosition {
    EntityPosition {
        y: target.position.y + projectile_entity_hit_y_offset(target),
        ..target.position
    }
}

fn projectile_entity_hit_y_offset(target: &ManagedEntity) -> f64 {
    match normalized_entity_type(&target.entity_type).as_str() {
        "minecraft:slime" | "minecraft:magma_cube" => {
            0.25 * f64::from(slime_size_for_entity(target))
        }
        "minecraft:chicken"
        | "minecraft:rabbit"
        | "minecraft:silverfish"
        | "minecraft:endermite" => 0.35,
        "minecraft:spider" | "minecraft:cave_spider" => 0.55,
        "minecraft:ghast" => 2.0,
        "minecraft:ravager" | "minecraft:iron_golem" | "minecraft:warden" => 1.6,
        _ => 1.0,
    }
}

fn projectile_entity_collision_radius(target: &ManagedEntity) -> f64 {
    match normalized_entity_type(&target.entity_type).as_str() {
        "minecraft:ghast" => 2.0,
        "minecraft:ravager" | "minecraft:iron_golem" | "minecraft:warden" => 0.9,
        "minecraft:slime" | "minecraft:magma_cube" => {
            (0.25 * f64::from(slime_size_for_entity(target))).clamp(0.35, 2.0)
        }
        "minecraft:chicken"
        | "minecraft:rabbit"
        | "minecraft:silverfish"
        | "minecraft:endermite" => 0.35,
        _ => 0.6,
    }
}

fn projectile_hit_player<'a>(
    previous: EntityPosition,
    current: EntityPosition,
    dimension: &str,
    viewers: &'a [crate::players::OnlinePlayer],
    hit_radius: f64,
) -> Option<&'a crate::players::OnlinePlayer> {
    let hit_radius = hit_radius.clamp(0.1, 8.0);
    viewers
        .iter()
        .filter(|player| player.dimension == dimension && player_can_be_attacked(player))
        .filter(|player| {
            let target = EntityPosition {
                x: player.position.x,
                y: player.position.y + 1.0,
                z: player.position.z,
                yaw: player.position.yaw,
                pitch: player.position.pitch,
                on_ground: player.position.on_ground,
            };
            point_segment_distance_sq(target, previous, current) <= hit_radius * hit_radius
        })
        .min_by(|left, right| {
            distance_sq(current, left.position).total_cmp(&distance_sq(current, right.position))
        })
}

fn point_segment_distance_sq(
    point: EntityPosition,
    start: EntityPosition,
    end: EntityPosition,
) -> f64 {
    let sx = end.x - start.x;
    let sy = end.y - start.y;
    let sz = end.z - start.z;
    let length_sq = sx * sx + sy * sy + sz * sz;
    if length_sq <= f64::EPSILON {
        return distance_sq(point, start);
    }
    let px = point.x - start.x;
    let py = point.y - start.y;
    let pz = point.z - start.z;
    let t = ((px * sx + py * sy + pz * sz) / length_sq).clamp(0.0, 1.0);
    let closest = EntityPosition {
        x: start.x + sx * t,
        y: start.y + sy * t,
        z: start.z + sz * t,
        yaw: 0.0,
        pitch: 0.0,
        on_ground: false,
    };
    distance_sq(point, closest)
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

fn apply_vertical_motion(motion: &mut EntityMotion, movement: EntityMovement, tick_scale: f64) {
    let target_y = movement
        .y
        .clamp(-ENTITY_MAX_HORIZONTAL_SPEED, ENTITY_MAX_HORIZONTAL_SPEED);
    if target_y == 0.0 {
        let friction = ENTITY_HORIZONTAL_FRICTION.powf(tick_scale);
        motion.velocity_y *= friction;
        if motion.velocity_y.abs() < 0.001 {
            motion.velocity_y = 0.0;
        }
        return;
    }
    let acceleration = ENTITY_HORIZONTAL_ACCELERATION * tick_scale;
    motion.velocity_y = approach(motion.velocity_y, target_y, acceleration);
}

fn entity_ignores_gravity_for_profile(
    entity: &ManagedEntity,
    world: &crate::world::WorldManager,
    profile: VanillaEntityAiProfile,
) -> bool {
    match profile.movement_mode {
        VanillaMovementMode::Ground => false,
        VanillaMovementMode::Flying => true,
        VanillaMovementMode::Swimming => {
            entity_position_touches_water(world, &entity.dimension, entity.position)
        }
    }
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
        if dy == 0.0
            && (dx != 0.0 || dz != 0.0)
            && try_entity_wall_climb(entity, world, collision_cache, motion)
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

fn try_entity_wall_climb(
    entity: &mut ManagedEntity,
    world: &crate::world::WorldManager,
    collision_cache: &mut CollisionCache,
    motion: &mut EntityMotion,
) -> bool {
    if !entity_type_climbs_walls(&entity.entity_type) {
        return false;
    }
    let profile = resolved_vanilla_ai_profile(entity, 8.0);
    let climb_velocity = profile.spider_climb_velocity.clamp(0.0, 1.0);
    if climb_velocity <= 0.0 {
        return false;
    }
    let climb_y = entity.position.y + climb_velocity;
    if entity_aabb_intersects_solid_at(
        world,
        collision_cache,
        &entity.dimension,
        entity.position.x,
        climb_y,
        entity.position.z,
    ) {
        return false;
    }
    entity.position.y = climb_y;
    entity.position.on_ground = false;
    motion.velocity_y = climb_velocity;
    true
}

fn entity_type_climbs_walls(entity_type: &str) -> bool {
    matches!(
        normalized_entity_type(entity_type).as_str(),
        "minecraft:spider" | "minecraft:cave_spider"
    )
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

fn apply_entity_fire_tick(
    entity: &ManagedEntity,
    world: &crate::world::WorldManager,
    world_rules: Option<&crate::world::WorldRulesManager>,
    fire_state: &mut HashMap<String, EntityFireState>,
    tick_ms: u64,
    fire_events: &mut Vec<ManagedEntity>,
    fire_damage: &mut Vec<i32>,
) {
    if entity_should_ignite_from_daylight(entity, world, world_rules) {
        let inserted = ignite_entity_state(fire_state, &entity.key, ENTITY_DAYLIGHT_FIRE_TICKS);
        if inserted {
            fire_events.push(entity.clone());
        }
    }

    let Some(state) = fire_state.get_mut(&entity.key) else {
        return;
    };
    let elapsed_ticks = entity_tick_units(tick_ms);
    state.remaining_ticks = state.remaining_ticks.saturating_sub(elapsed_ticks);
    state.damage_cooldown_ticks = state.damage_cooldown_ticks.saturating_sub(elapsed_ticks);
    if state.remaining_ticks > 0 && state.damage_cooldown_ticks <= 0 {
        fire_damage.push(entity.entity_id);
        state.damage_cooldown_ticks = ENTITY_FIRE_DAMAGE_INTERVAL_TICKS;
    }
    if state.remaining_ticks <= 0 {
        fire_state.remove(&entity.key);
    }
}

fn ignite_entity_state(
    fire_state: &mut HashMap<String, EntityFireState>,
    key: &str,
    fire_ticks: i32,
) -> bool {
    let fire_ticks = fire_ticks.max(1);
    match fire_state.get_mut(key) {
        Some(state) => {
            state.remaining_ticks = state.remaining_ticks.max(fire_ticks);
            false
        }
        None => {
            fire_state.insert(
                key.to_string(),
                EntityFireState {
                    remaining_ticks: fire_ticks,
                    damage_cooldown_ticks: ENTITY_FIRE_DAMAGE_INTERVAL_TICKS,
                },
            );
            true
        }
    }
}

fn entity_tick_units(tick_ms: u64) -> i32 {
    i32::try_from(tick_ms.max(50).div_ceil(50))
        .unwrap_or(i32::MAX)
        .clamp(1, 100)
}

fn apply_summon_lifetime_tick(
    entity: &ManagedEntity,
    tick_ms: u64,
    summon_state: &mut HashMap<String, EntitySummonState>,
) -> bool {
    let Some(state) = summon_state.get_mut(&entity.key) else {
        return false;
    };
    state.remaining_ticks = state
        .remaining_ticks
        .saturating_sub(entity_tick_units(tick_ms));
    state.remaining_ticks <= 0
}

fn entity_should_ignite_from_daylight(
    entity: &ManagedEntity,
    world: &crate::world::WorldManager,
    world_rules: Option<&crate::world::WorldRulesManager>,
) -> bool {
    if !entity_type_burns_in_daylight(&entity.entity_type) {
        return false;
    }
    let Some(world_rules) = world_rules else {
        return false;
    };
    let snapshot = world_rules.snapshot(&entity.dimension);
    if !dimension_has_daylight(&snapshot.dimension_type) || !world_time_is_day(snapshot.time_value)
    {
        return false;
    }
    !entity_is_in_water(entity, world) && entity_has_sky_visibility(entity, world)
}

fn entity_type_burns_in_daylight(entity_type: &str) -> bool {
    matches!(
        normalized_entity_type(entity_type).as_str(),
        "minecraft:zombie"
            | "minecraft:zombie_villager"
            | "minecraft:drowned"
            | "minecraft:skeleton"
            | "minecraft:stray"
            | "minecraft:bogged"
            | "minecraft:phantom"
    )
}

fn dimension_has_daylight(dimension_type: &str) -> bool {
    matches!(dimension_type.trim(), "" | "minecraft:overworld")
}

fn world_time_is_day(time_value: i64) -> bool {
    let time_of_day = time_value.rem_euclid(24_000);
    (0..12_000).contains(&time_of_day)
}

fn entity_is_in_water(entity: &ManagedEntity, world: &crate::world::WorldManager) -> bool {
    entity_position_touches_water(world, &entity.dimension, entity.position)
}

fn entity_position_touches_water(
    world: &crate::world::WorldManager,
    dimension: &str,
    position: EntityPosition,
) -> bool {
    let x = position.x.floor() as i32;
    let z = position.z.floor() as i32;
    let feet_y = position.y.floor() as i32;
    let eye_y = (position.y + 1.62).floor() as i32;
    [feet_y, eye_y].into_iter().any(|y| {
        block_name_at(world, dimension, x, y, z).is_some_and(|name| {
            matches!(name.as_str(), "minecraft:water" | "minecraft:bubble_column")
        })
    })
}

fn entity_has_sky_visibility(entity: &ManagedEntity, world: &crate::world::WorldManager) -> bool {
    let x = entity.position.x.floor() as i32;
    let z = entity.position.z.floor() as i32;
    let start_y = (entity.position.y + ENTITY_PHYSICS_HEIGHT).floor() as i32 + 1;
    for y in start_y..=crate::world::WORLD_MAX_Y {
        let Some(name) = block_name_at(world, &entity.dimension, x, y, z) else {
            continue;
        };
        if !block_allows_daylight(&name) {
            return false;
        }
    }
    true
}

fn block_name_at(
    world: &crate::world::WorldManager,
    dimension: &str,
    x: i32,
    y: i32,
    z: i32,
) -> Option<String> {
    world
        .block_state_at(dimension, &BlockPosition { x, y, z })
        .and_then(crate::inventory::block_name_for_state)
}

fn block_allows_daylight(block_name: &str) -> bool {
    matches!(
        block_name,
        "minecraft:air"
            | "minecraft:cave_air"
            | "minecraft:void_air"
            | "minecraft:water"
            | "minecraft:bubble_column"
            | "minecraft:glass"
            | "minecraft:tinted_glass"
            | "minecraft:short_grass"
            | "minecraft:tall_grass"
            | "minecraft:fern"
            | "minecraft:large_fern"
            | "minecraft:vine"
            | "minecraft:seagrass"
            | "minecraft:tall_seagrass"
            | "minecraft:snow"
            | "minecraft:torch"
            | "minecraft:wall_torch"
            | "minecraft:fire"
            | "minecraft:soul_fire"
    )
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
    } else if is_known_minecraft_entity_type(&entity_type) {
        "vanilla".to_string()
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

fn slime_chunk_spawning_applies(
    config: &qexed_config::app::qexed::server::SlimeChunkSpawning,
    entity_type: &str,
) -> bool {
    config.enable
        && config.entity_types.iter().any(|configured| {
            normalize_minecraft_key(configured) == normalize_minecraft_key(entity_type)
        })
}

fn slime_chunk_allows_position(
    config: &qexed_config::app::qexed::server::SlimeChunkSpawning,
    position: EntityPosition,
) -> bool {
    let block_x = position.x.floor() as i32;
    let block_z = position.z.floor() as i32;
    is_vanilla_slime_chunk(
        config.seed,
        block_x.div_euclid(16),
        block_z.div_euclid(16),
        config.chance.max(1),
    )
}

fn is_vanilla_slime_chunk(seed: i64, chunk_x: i32, chunk_z: i32, chance: u32) -> bool {
    let chunk_x = i64::from(chunk_x);
    let chunk_z = i64::from(chunk_z);
    let mixed = seed
        .wrapping_add(chunk_x.wrapping_mul(chunk_x).wrapping_mul(4_987_142))
        .wrapping_add(chunk_x.wrapping_mul(5_947_611))
        .wrapping_add(chunk_z.wrapping_mul(chunk_z).wrapping_mul(4_392_871))
        .wrapping_add(chunk_z.wrapping_mul(389_711))
        ^ 987_234_911;
    java_random_next_int(mixed, chance.max(1)) == 0
}

fn java_random_next_int(seed: i64, bound: u32) -> u32 {
    let bound = bound.max(1);
    let mut state = ((seed as u64) ^ 0x5DEECE66D) & ((1_u64 << 48) - 1);
    if bound.is_power_of_two() {
        return ((u64::from(bound) * u64::from(java_random_next_bits(&mut state, 31))) >> 31)
            as u32;
    }

    loop {
        let bits = i64::from(java_random_next_bits(&mut state, 31));
        let value = bits % i64::from(bound);
        if bits - value + (i64::from(bound) - 1) >= 0 {
            return value as u32;
        }
    }
}

fn java_random_next_bits(state: &mut u64, bits: u32) -> u32 {
    *state = (*state).wrapping_mul(0x5DEECE66D).wrapping_add(0xB) & ((1_u64 << 48) - 1);
    (*state >> (48 - bits)) as u32
}

fn normalize_minecraft_key(value: &str) -> String {
    let value = value.trim().to_ascii_lowercase();
    if value.contains(':') {
        value
    } else {
        format!("minecraft:{value}")
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

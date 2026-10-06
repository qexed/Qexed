//! qexed_entities 配置模型。
//!
//! v4 的配置路径 `qexed_config::app::qexed::server::{Entities, Entity, ...}` 在 v6
//! 不存在（v6 配置统一在 qexed_config 核心）。这里以纯 serde 结构体保留 v4 字段语义
//! （字段名/默认值与 v4 一致），供 server 域组装时反序列化注入。
//! TODO(config): 待 server 域配置落地后，用 app_config 宏替换本模块并迁移字段文档。

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

fn default_entities_dimension() -> String {
    "minecraft:overworld".to_string()
}

fn default_entity_type() -> String {
    "minecraft:armor_stand".to_string()
}

fn default_entity_auto_jump() -> bool {
    true
}

fn default_npc_main_hand_event() -> String {
    "interact".to_string()
}

fn default_npc_off_hand_event() -> String {
    "interact_off_hand".to_string()
}

fn default_npc_attack_event() -> String {
    "attack".to_string()
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Entities {
    #[serde(default)]
    pub enable: bool,
    #[serde(default = "default_entities_dimension")]
    pub dimension: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub list: Vec<Entity>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub disabled_entity_types: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ai_overrides: Vec<EntityAiOverride>,
    #[serde(default)]
    pub spawning: EntitySpawning,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Entity {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub kind: EntityKind,
    #[serde(default = "default_entity_type")]
    pub entity_type: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub skin_textures: String,
    #[serde(default)]
    pub skin_signature: String,
    #[serde(default)]
    pub skin_player_id: String,
    #[serde(default)]
    pub x: f64,
    #[serde(default = "default_entity_y")]
    pub y: f64,
    #[serde(default)]
    pub z: f64,
    #[serde(default)]
    pub yaw: f32,
    #[serde(default)]
    pub pitch: f32,
    #[serde(default = "default_entity_on_ground")]
    pub on_ground: bool,
    #[serde(default)]
    pub data: i32,
    #[serde(default)]
    pub ai: String,
    #[serde(default)]
    pub ai_params: BTreeMap<String, serde_json::Value>,
    #[serde(default = "default_entity_auto_jump")]
    pub auto_jump: bool,
    #[serde(default)]
    pub look_at_players: bool,
    #[serde(default = "default_npc_main_hand_event")]
    pub main_hand_event: String,
    #[serde(default = "default_npc_off_hand_event")]
    pub off_hand_event: String,
    #[serde(default = "default_npc_attack_event")]
    pub attack_event: String,
}

fn default_entity_y() -> f64 {
    64.0
}

fn default_entity_on_ground() -> bool {
    true
}

impl Default for Entity {
    fn default() -> Self {
        Self {
            id: String::new(),
            kind: EntityKind::default(),
            entity_type: default_entity_type(),
            name: String::new(),
            display_name: String::new(),
            skin_textures: String::new(),
            skin_signature: String::new(),
            skin_player_id: String::new(),
            x: 0.0,
            y: default_entity_y(),
            z: 0.0,
            yaw: 0.0,
            pitch: 0.0,
            on_ground: default_entity_on_ground(),
            data: 0,
            ai: String::new(),
            ai_params: BTreeMap::new(),
            auto_jump: default_entity_auto_jump(),
            look_at_players: false,
            main_hand_event: default_npc_main_hand_event(),
            off_hand_event: default_npc_off_hand_event(),
            attack_event: default_npc_attack_event(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityKind {
    #[default]
    Entity,
    Npc,
    Hologram,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Default)]
pub struct EntityAiOverride {
    #[serde(default = "default_entity_type")]
    pub entity_type: String,
    #[serde(default)]
    pub ai: String,
    #[serde(default)]
    pub ai_params: BTreeMap<String, serde_json::Value>,
    #[serde(default)]
    pub auto_jump: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EntityRendering {
    #[serde(default = "default_entity_render_distance")]
    pub default_distance: f64,
    #[serde(default = "default_player_render_distance")]
    pub player_distance: f64,
    #[serde(default = "default_npc_render_distance")]
    pub npc_distance: f64,
    #[serde(default = "default_hologram_render_distance")]
    pub hologram_distance: f64,
    #[serde(default = "default_item_render_distance")]
    pub item_distance: f64,
    #[serde(default = "default_item_merge_radius")]
    pub item_merge_radius: f64,
    #[serde(default = "default_item_merge_max_stack")]
    pub item_merge_max_stack: i32,
    #[serde(default = "default_entity_stack_threshold")]
    pub stack_threshold: usize,
    #[serde(default = "default_entity_stack_radius")]
    pub stack_radius: f64,
}

impl Default for EntityRendering {
    fn default() -> Self {
        Self {
            default_distance: default_entity_render_distance(),
            player_distance: default_player_render_distance(),
            npc_distance: default_npc_render_distance(),
            hologram_distance: default_hologram_render_distance(),
            item_distance: default_item_render_distance(),
            item_merge_radius: default_item_merge_radius(),
            item_merge_max_stack: default_item_merge_max_stack(),
            stack_threshold: default_entity_stack_threshold(),
            stack_radius: default_entity_stack_radius(),
        }
    }
}

fn default_entity_render_distance() -> f64 {
    64.0
}

fn default_player_render_distance() -> f64 {
    64.0
}

fn default_npc_render_distance() -> f64 {
    64.0
}

fn default_hologram_render_distance() -> f64 {
    64.0
}

fn default_item_render_distance() -> f64 {
    32.0
}

fn default_item_merge_radius() -> f64 {
    2.0
}

fn default_item_merge_max_stack() -> i32 {
    64
}

fn default_entity_stack_threshold() -> usize {
    20
}

fn default_entity_stack_radius() -> f64 {
    4.0
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EntitySpawning {
    #[serde(default)]
    pub enable: bool,
    #[serde(default = "default_entity_spawn_tick_interval_ms")]
    pub tick_interval_ms: u64,
    #[serde(default = "default_entity_ai_tick_interval_ms")]
    pub ai_tick_interval_ms: u64,
    #[serde(default = "default_entity_spawn_global_cap")]
    pub global_cap: usize,
    #[serde(default = "default_entity_spawn_per_dimension_cap")]
    pub per_dimension_cap: usize,
    #[serde(default = "default_entity_spawn_per_type_cap")]
    pub per_type_cap: usize,
    #[serde(default = "default_entity_spawn_max_per_tick")]
    pub max_spawn_per_tick: usize,
    #[serde(default = "default_entity_spawn_player_activation_range")]
    pub player_activation_range: f64,
    #[serde(default)]
    pub slime_chunks: SlimeChunkSpawning,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rules: Vec<EntitySpawnRule>,
}

impl Default for EntitySpawning {
    fn default() -> Self {
        Self {
            enable: false,
            tick_interval_ms: default_entity_spawn_tick_interval_ms(),
            ai_tick_interval_ms: default_entity_ai_tick_interval_ms(),
            global_cap: default_entity_spawn_global_cap(),
            per_dimension_cap: default_entity_spawn_per_dimension_cap(),
            per_type_cap: default_entity_spawn_per_type_cap(),
            max_spawn_per_tick: default_entity_spawn_max_per_tick(),
            player_activation_range: default_entity_spawn_player_activation_range(),
            slime_chunks: SlimeChunkSpawning::default(),
            rules: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SlimeChunkSpawning {
    #[serde(default)]
    pub enable: bool,
    #[serde(default)]
    pub seed: i64,
    #[serde(default = "default_slime_chunk_spawn_chance")]
    pub chance: u32,
    #[serde(default = "default_slime_chunk_entity_types")]
    pub entity_types: Vec<String>,
}

impl Default for SlimeChunkSpawning {
    fn default() -> Self {
        Self {
            enable: false,
            seed: 0,
            chance: default_slime_chunk_spawn_chance(),
            entity_types: default_slime_chunk_entity_types(),
        }
    }
}

fn default_slime_chunk_spawn_chance() -> u32 {
    10
}

fn default_slime_chunk_entity_types() -> Vec<String> {
    vec!["minecraft:slime".to_string()]
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EntitySpawnRule {
    #[serde(default)]
    pub id: String,
    #[serde(default = "default_entity_spawn_rule_enable")]
    pub enable: bool,
    #[serde(default)]
    pub dimension: String,
    #[serde(default = "default_entity_spawn_rule_entity_type")]
    pub entity_type: String,
    #[serde(default = "default_entity_spawn_rule_weight")]
    pub weight: u32,
    #[serde(default = "default_entity_spawn_rule_cap")]
    pub cap: usize,
    #[serde(default)]
    pub tick_interval_ms: u64,
    #[serde(default = "default_entity_spawn_rule_spawn_chance")]
    pub spawn_chance: f64,
    #[serde(default = "default_entity_spawn_rule_min_players")]
    pub min_players: usize,
    #[serde(default)]
    pub max_players: usize,
    #[serde(default)]
    pub activation_range: f64,
    #[serde(default = "default_entity_spawn_rule_require_ground")]
    pub require_ground: bool,
    #[serde(default = "default_entity_spawn_rule_require_air")]
    pub require_air: bool,
    #[serde(default = "default_entity_spawn_rule_position_attempts")]
    pub position_attempts: u32,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub ai: String,
    #[serde(default)]
    pub ai_params: BTreeMap<String, serde_json::Value>,
    #[serde(default = "default_entity_auto_jump")]
    pub auto_jump: bool,
    #[serde(default)]
    pub data: i32,
    #[serde(default = "default_entity_spawn_rule_on_ground")]
    pub on_ground: bool,
    #[serde(default = "default_entity_spawn_min_x")]
    pub min_x: f64,
    #[serde(default = "default_entity_spawn_max_x")]
    pub max_x: f64,
    #[serde(default = "default_entity_spawn_min_y")]
    pub min_y: f64,
    #[serde(default = "default_entity_spawn_max_y")]
    pub max_y: f64,
    #[serde(default = "default_entity_spawn_min_z")]
    pub min_z: f64,
    #[serde(default = "default_entity_spawn_max_z")]
    pub max_z: f64,
}

impl Default for EntitySpawnRule {
    fn default() -> Self {
        Self {
            id: String::new(),
            enable: default_entity_spawn_rule_enable(),
            dimension: String::new(),
            entity_type: default_entity_spawn_rule_entity_type(),
            weight: default_entity_spawn_rule_weight(),
            cap: default_entity_spawn_rule_cap(),
            tick_interval_ms: 0,
            spawn_chance: default_entity_spawn_rule_spawn_chance(),
            min_players: default_entity_spawn_rule_min_players(),
            max_players: 0,
            activation_range: 0.0,
            require_ground: default_entity_spawn_rule_require_ground(),
            require_air: default_entity_spawn_rule_require_air(),
            position_attempts: default_entity_spawn_rule_position_attempts(),
            name: String::new(),
            display_name: String::new(),
            ai: String::new(),
            ai_params: BTreeMap::new(),
            auto_jump: default_entity_auto_jump(),
            data: 0,
            on_ground: default_entity_spawn_rule_on_ground(),
            min_x: default_entity_spawn_min_x(),
            max_x: default_entity_spawn_max_x(),
            min_y: default_entity_spawn_min_y(),
            max_y: default_entity_spawn_max_y(),
            min_z: default_entity_spawn_min_z(),
            max_z: default_entity_spawn_max_z(),
        }
    }
}

fn default_entity_spawn_tick_interval_ms() -> u64 {
    1000
}

fn default_entity_ai_tick_interval_ms() -> u64 {
    50
}

fn default_entity_spawn_global_cap() -> usize {
    70
}

fn default_entity_spawn_per_dimension_cap() -> usize {
    70
}

fn default_entity_spawn_per_type_cap() -> usize {
    20
}

fn default_entity_spawn_max_per_tick() -> usize {
    4
}

fn default_entity_spawn_player_activation_range() -> f64 {
    64.0
}

fn default_entity_spawn_rule_enable() -> bool {
    true
}

fn default_entity_spawn_rule_entity_type() -> String {
    "minecraft:zombie".to_string()
}

fn default_entity_spawn_rule_weight() -> u32 {
    100
}

fn default_entity_spawn_rule_cap() -> usize {
    20
}

fn default_entity_spawn_rule_spawn_chance() -> f64 {
    1.0
}

fn default_entity_spawn_rule_min_players() -> usize {
    1
}

fn default_entity_spawn_rule_require_ground() -> bool {
    false
}

fn default_entity_spawn_rule_require_air() -> bool {
    false
}

fn default_entity_spawn_rule_position_attempts() -> u32 {
    8
}

fn default_entity_spawn_rule_on_ground() -> bool {
    true
}

fn default_entity_spawn_min_x() -> f64 {
    -16.0
}

fn default_entity_spawn_max_x() -> f64 {
    16.0
}

fn default_entity_spawn_min_y() -> f64 {
    64.0
}

fn default_entity_spawn_max_y() -> f64 {
    64.0
}

fn default_entity_spawn_min_z() -> f64 {
    -16.0
}

fn default_entity_spawn_max_z() -> f64 {
    16.0
}

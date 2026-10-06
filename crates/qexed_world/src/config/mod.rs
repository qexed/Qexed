//! qexed_world 配置：v4 `qexed_config::app::qexed::server` 里 world 域的配置类型
//! （迁移规则 4：v6 的 qexed_config 无这些路径，用本 crate 定义）。
//!
//! - `WorldConfig`（`app_config` 宏 → `world.toml`）：v4 `server::World` 的 v6 对应。
//! - `LightMode`/`LightAlgorithm`/`WorldCluster`/`WorldClusterShard`/
//!   `WorldClusterMode`/`WorldClusterAxisSide`/`WorldSpawnPlatform`/
//!   `WorldStorage`/`WorldInstance`/`WorldOrePit`/`WorldOrePitBlock`/
//!   `WorldEditRegion`/`PrecompiledChunks`/`Spawn`/`WorldGenerator`/
//!   `GameMode`：v4 同名类型原样迁移（serde 语义不变）。
//! - `DimensionWorldRules`（`app_config` 宏 → `rules.toml`）：v4
//!   `qexed_config::app::qexed::world_rules::DimensionWorldRules`。
//!
//! 说明：含 Vec/嵌套结构体字段的配置不派生 Doc（Doc 宏不支持），与
//! qexed_server 的做法一致。

use serde::{Deserialize, Serialize};

/// 世界主配置（v4 qexed_config::app::qexed::server::World）。
#[qexed_config_macros::app_config("/", "world")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldConfig {
    #[serde(default = "default_dimension")]
    pub default_dimension: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub worlds: Vec<WorldStorage>,
    #[serde(default = "default_world_path")]
    pub path: String,
    #[serde(default)]
    pub read_only: bool,
    #[serde(default)]
    pub generator: WorldGenerator,
    #[serde(default = "default_world_generator_preset")]
    pub generator_preset: String,
    #[serde(default)]
    pub seed: i64,
    #[serde(default)]
    pub game_mode: GameMode,
    #[serde(default)]
    pub allow_flight: bool,
    #[serde(default = "default_spawn_protection_radius")]
    pub spawn_protection_radius: i32,
    #[serde(default = "default_dimension")]
    pub dimension: String,
    #[serde(default = "default_dimension_type")]
    pub dimension_type: String,
    #[serde(default = "default_view_distance")]
    pub view_distance: i32,
    #[serde(default = "default_chunk_load_parallelism")]
    pub chunk_load_parallelism: usize,
    #[serde(default = "default_chunk_update_delay_ms")]
    pub chunk_update_delay_ms: u64,
    #[serde(default = "default_view_distance")]
    pub simulation_distance: i32,
    #[serde(default)]
    pub light: LightMode,
    #[serde(default)]
    pub light_algorithm: LightAlgorithm,
    #[serde(default)]
    pub precompiled_chunks: PrecompiledChunks,
    #[serde(default)]
    pub cluster: WorldCluster,
    #[serde(default)]
    pub spawn_platform: WorldSpawnPlatform,
    #[serde(default)]
    pub spawn: Spawn,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub instances: Vec<WorldInstance>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ore_pits: Vec<WorldOrePit>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub edit_regions: Vec<WorldEditRegion>,
}

impl Default for WorldConfig {
    fn default() -> Self {
        Self {
            default_dimension: default_dimension(),
            worlds: vec![WorldStorage::default()],
            path: default_world_path(),
            read_only: false,
            generator: WorldGenerator::default(),
            generator_preset: default_world_generator_preset(),
            seed: 0,
            game_mode: GameMode::default(),
            allow_flight: false,
            spawn_protection_radius: default_spawn_protection_radius(),
            dimension: default_dimension(),
            dimension_type: default_dimension_type(),
            view_distance: default_view_distance(),
            chunk_load_parallelism: default_chunk_load_parallelism(),
            chunk_update_delay_ms: default_chunk_update_delay_ms(),
            simulation_distance: default_view_distance(),
            light: LightMode::default(),
            light_algorithm: LightAlgorithm::default(),
            precompiled_chunks: PrecompiledChunks::default(),
            cluster: WorldCluster::default(),
            spawn_platform: WorldSpawnPlatform::default(),
            spawn: Spawn::default(),
            instances: Vec::new(),
            ore_pits: Vec::new(),
            edit_regions: Vec::new(),
        }
    }
}

impl WorldConfig {
    /// v4 World::default_play_dimension 原样。
    pub fn default_play_dimension(&self) -> String {
        let default_dimension = self.default_dimension.trim();
        if !default_dimension.is_empty() {
            return default_dimension.to_string();
        }
        let legacy_dimension = self.dimension.trim();
        if !legacy_dimension.is_empty() {
            return legacy_dimension.to_string();
        }
        self.worlds
            .iter()
            .find_map(|world| {
                let dimension = world.dimension.trim();
                (!dimension.is_empty()).then(|| dimension.to_string())
            })
            .unwrap_or_else(default_world_dimension)
    }

    /// v4 World::configured_dimension_names 原样。
    pub fn configured_dimension_names(&self) -> Vec<String> {
        let mut dimensions = Vec::new();
        push_unique_dimension(&mut dimensions, self.default_play_dimension());
        for world in &self.worlds {
            push_unique_dimension(&mut dimensions, world.dimension.trim());
        }
        if self.worlds.is_empty() {
            push_unique_dimension(&mut dimensions, self.dimension.trim());
        }
        for instance in &self.instances {
            push_unique_dimension(&mut dimensions, instance.dimension.trim());
        }
        dimensions
    }

    /// v4 World::dimension_type_for 原样。
    pub fn dimension_type_for(&self, dimension: &str) -> Option<String> {
        let dimension = dimension.trim();
        if dimension.is_empty() {
            return None;
        }
        for world in &self.worlds {
            if world.dimension.trim() == dimension {
                let dimension_type = world.dimension_type.trim();
                if !dimension_type.is_empty() {
                    return Some(dimension_type.to_string());
                }
            }
        }
        if self.worlds.is_empty() && self.dimension.trim() == dimension {
            let dimension_type = self.dimension_type.trim();
            if !dimension_type.is_empty() {
                return Some(dimension_type.to_string());
            }
        }
        None
    }
}

/// 兼容别名：v4 类型名就叫 World（避免与 MC 世界概念混淆，v6 主名 WorldConfig）。
pub type World = WorldConfig;

fn push_unique_dimension(dimensions: &mut Vec<String>, dimension: impl AsRef<str>) {
    let dimension = dimension.as_ref().trim();
    if !dimension.is_empty() && !dimensions.iter().any(|existing| existing == dimension) {
        dimensions.push(dimension.to_string());
    }
}

/// 世界生成器类型（v4 server::WorldGenerator）。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorldGenerator {
    #[default]
    Empty,
    VanillaFlat,
    VanillaNoise,
}

/// 游戏模式（v4 server::GameMode）。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GameMode {
    #[default]
    Survival,
    Creative,
    Adventure,
    Spectator,
}

/// 光照模式（v4 server::LightMode；支持 "static"/"dynamic"/0-15 数字）。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum LightMode {
    #[default]
    Static,
    Dynamic,
    Fixed(u8),
}

impl Serialize for LightMode {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Self::Static => serializer.serialize_str("static"),
            Self::Dynamic => serializer.serialize_str("dynamic"),
            Self::Fixed(level) => serializer.serialize_u8(*level),
        }
    }
}

impl<'de> Deserialize<'de> for LightMode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct Visitor;

        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = LightMode;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str(r#""static", "dynamic", or an integer brightness from 0 to 15"#)
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                match value.trim().to_ascii_lowercase().as_str() {
                    "static" => Ok(LightMode::Static),
                    "dynamic" => Ok(LightMode::Dynamic),
                    other => other
                        .parse::<u8>()
                        .map_err(|_| E::custom(format!("unknown light mode: {value}")))
                        .and_then(fixed_light_mode),
                }
            }

            fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                let value = u8::try_from(value)
                    .map_err(|_| E::custom(format!("brightness out of range 0..=15: {value}")))?;
                fixed_light_mode(value)
            }

            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                let value = u8::try_from(value)
                    .map_err(|_| E::custom(format!("brightness out of range 0..=15: {value}")))?;
                fixed_light_mode(value)
            }
        }

        fn fixed_light_mode<E>(value: u8) -> Result<LightMode, E>
        where
            E: serde::de::Error,
        {
            if value <= 15 {
                Ok(LightMode::Fixed(value))
            } else {
                Err(E::custom(format!(
                    "brightness out of range 0..=15: {value}"
                )))
            }
        }

        deserializer.deserialize_any(Visitor)
    }
}

/// 光照算法（v4 server::LightAlgorithm）。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LightAlgorithm {
    #[default]
    Fast,
    RayTrace,
}

/// 世界集群（v4 server::WorldCluster）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorldCluster {
    #[serde(default)]
    pub enable: bool,
    #[serde(default = "default_world_cluster_mode")]
    pub mode: WorldClusterMode,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub shards: Vec<WorldClusterShard>,
}

impl Default for WorldCluster {
    fn default() -> Self {
        Self {
            enable: false,
            mode: default_world_cluster_mode(),
            shards: Vec::new(),
        }
    }
}

/// 集群模式（v4 server::WorldClusterMode）。
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorldClusterMode {
    #[default]
    Quadrant,
    Regions,
}

/// 集群分片（v4 server::WorldClusterShard）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorldClusterShard {
    pub id: String,
    #[serde(default)]
    pub endpoint: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x: Option<WorldClusterAxisSide>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub z: Option<WorldClusterAxisSide>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_chunk_x: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_chunk_x: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_chunk_z: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_chunk_z: Option<i32>,
}

/// 集群象限方向（v4 server::WorldClusterAxisSide）。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorldClusterAxisSide {
    Negative,
    Positive,
}

/// 出生平台（v4 server::WorldSpawnPlatform）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorldSpawnPlatform {
    #[serde(default)]
    pub enable: bool,
    #[serde(default = "default_dimension")]
    pub dimension: String,
    #[serde(default = "default_spawn_platform_block")]
    pub block: String,
    #[serde(default = "default_spawn_platform_y")]
    pub y: i32,
    #[serde(default = "default_spawn_platform_min_x")]
    pub min_x: i32,
    #[serde(default = "default_spawn_platform_max_x")]
    pub max_x: i32,
    #[serde(default = "default_spawn_platform_min_z")]
    pub min_z: i32,
    #[serde(default = "default_spawn_platform_max_z")]
    pub max_z: i32,
}

impl Default for WorldSpawnPlatform {
    fn default() -> Self {
        Self {
            enable: false,
            dimension: default_dimension(),
            block: default_spawn_platform_block(),
            y: default_spawn_platform_y(),
            min_x: default_spawn_platform_min_x(),
            max_x: default_spawn_platform_max_x(),
            min_z: default_spawn_platform_min_z(),
            max_z: default_spawn_platform_max_z(),
        }
    }
}

/// 多世界存储（v4 server::WorldStorage）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorldStorage {
    #[serde(default)]
    pub id: String,
    #[serde(default = "default_dimension")]
    pub dimension: String,
    #[serde(default = "default_dimension_type")]
    pub dimension_type: String,
    #[serde(default = "default_world_path")]
    pub path: String,
}

impl Default for WorldStorage {
    fn default() -> Self {
        Self {
            id: "overworld".to_string(),
            dimension: default_dimension(),
            dimension_type: default_dimension_type(),
            path: default_world_path(),
        }
    }
}

/// 世界实例（copy-on-write，v4 server::WorldInstance）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorldInstance {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub dimension: String,
    #[serde(default = "default_world_instance_source_dimension")]
    pub source_dimension: String,
    #[serde(default)]
    pub path: String,
    #[serde(default = "default_world_instance_copy_on_write")]
    pub copy_on_write: bool,
}

impl Default for WorldInstance {
    fn default() -> Self {
        Self {
            id: String::new(),
            dimension: String::new(),
            source_dimension: default_world_instance_source_dimension(),
            path: String::new(),
            copy_on_write: default_world_instance_copy_on_write(),
        }
    }
}

/// 矿坑（v4 server::WorldOrePit）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorldOrePit {
    #[serde(default)]
    pub id: String,
    #[serde(default = "default_world_ore_pit_enable")]
    pub enable: bool,
    #[serde(default = "default_dimension")]
    pub dimension: String,
    pub min_x: i32,
    pub max_x: i32,
    pub min_y: i32,
    pub max_y: i32,
    pub min_z: i32,
    pub max_z: i32,
    #[serde(default = "default_world_ore_pit_tick_interval_ms")]
    pub tick_interval_ms: u64,
    #[serde(default = "default_world_ore_pit_initial_refill")]
    pub initial_refill: bool,
    #[serde(default = "default_world_ore_pit_max_blocks_per_tick")]
    pub max_blocks_per_tick: usize,
    #[serde(default)]
    pub teleport_players_to_surface_on_refill: bool,
    #[serde(default = "default_world_ore_pit_replace_air")]
    pub replace_air: bool,
    #[serde(default = "default_world_ore_pit_replace_generated")]
    pub replace_generated: bool,
    #[serde(default)]
    pub only_break_generated: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocks: Vec<WorldOrePitBlock>,
}

impl Default for WorldOrePit {
    fn default() -> Self {
        Self {
            id: String::new(),
            enable: default_world_ore_pit_enable(),
            dimension: default_dimension(),
            min_x: 0,
            max_x: 0,
            min_y: 0,
            max_y: 0,
            min_z: 0,
            max_z: 0,
            tick_interval_ms: default_world_ore_pit_tick_interval_ms(),
            initial_refill: default_world_ore_pit_initial_refill(),
            max_blocks_per_tick: default_world_ore_pit_max_blocks_per_tick(),
            teleport_players_to_surface_on_refill: false,
            replace_air: default_world_ore_pit_replace_air(),
            replace_generated: default_world_ore_pit_replace_generated(),
            only_break_generated: false,
            blocks: Vec::new(),
        }
    }
}

impl WorldOrePit {
    /// v4 WorldOrePit::contains 原样。
    pub fn contains(&self, dimension: &str, x: i32, y: i32, z: i32) -> bool {
        self.dimension.trim() == dimension.trim()
            && contains_axis(x, self.min_x, self.max_x)
            && contains_axis(y, self.min_y, self.max_y)
            && contains_axis(z, self.min_z, self.max_z)
    }
}

/// 矿坑方块权重（v4 server::WorldOrePitBlock）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorldOrePitBlock {
    #[serde(default = "default_world_ore_pit_block")]
    pub block: String,
    #[serde(default = "default_world_ore_pit_block_weight")]
    pub weight: u32,
}

impl Default for WorldOrePitBlock {
    fn default() -> Self {
        Self {
            block: default_world_ore_pit_block(),
            weight: default_world_ore_pit_block_weight(),
        }
    }
}

/// 运行时可编辑区域（v4 server::WorldEditRegion）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorldEditRegion {
    #[serde(default)]
    pub id: String,
    #[serde(default = "default_dimension")]
    pub dimension: String,
    pub min_x: i32,
    pub max_x: i32,
    pub min_y: i32,
    pub max_y: i32,
    pub min_z: i32,
    pub max_z: i32,
    #[serde(default = "default_world_edit_region_allow_player_break")]
    pub allow_player_break: bool,
    #[serde(default = "default_world_edit_region_allow_player_place")]
    pub allow_player_place: bool,
    #[serde(default = "default_world_edit_region_allow_plugin_write")]
    pub allow_plugin_write: bool,
    #[serde(default = "default_world_edit_region_runtime_only")]
    pub runtime_only: bool,
}

impl Default for WorldEditRegion {
    fn default() -> Self {
        Self {
            id: String::new(),
            dimension: default_dimension(),
            min_x: 0,
            max_x: 0,
            min_y: 0,
            max_y: 0,
            min_z: 0,
            max_z: 0,
            allow_player_break: default_world_edit_region_allow_player_break(),
            allow_player_place: default_world_edit_region_allow_player_place(),
            allow_plugin_write: default_world_edit_region_allow_plugin_write(),
            runtime_only: default_world_edit_region_runtime_only(),
        }
    }
}

impl WorldEditRegion {
    /// v4 WorldEditRegion::contains 原样。
    pub fn contains(&self, dimension: &str, x: i32, y: i32, z: i32) -> bool {
        self.dimension.trim() == dimension.trim()
            && contains_axis(x, self.min_x, self.max_x)
            && contains_axis(y, self.min_y, self.max_y)
            && contains_axis(z, self.min_z, self.max_z)
    }
}

fn contains_axis(value: i32, first: i32, second: i32) -> bool {
    let min = first.min(second);
    let max = first.max(second);
    (min..=max).contains(&value)
}

/// 预编译区块（v4 server::PrecompiledChunks）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PrecompiledChunks {
    #[serde(default)]
    pub enable: bool,
    #[serde(default = "default_precompiled_chunks_light")]
    pub light: bool,
    #[serde(default = "default_precompiled_chunks_max_cached_packets")]
    pub max_cached_packets: usize,
    #[serde(default = "default_precompiled_chunks_max_cached_packet_bytes")]
    pub max_cached_packet_bytes: usize,
    #[serde(default = "default_precompiled_chunks_block_state_cache_limit")]
    pub block_state_cache_limit: usize,
}

impl Default for PrecompiledChunks {
    fn default() -> Self {
        Self {
            enable: false,
            light: default_precompiled_chunks_light(),
            max_cached_packets: default_precompiled_chunks_max_cached_packets(),
            max_cached_packet_bytes: default_precompiled_chunks_max_cached_packet_bytes(),
            block_state_cache_limit: default_precompiled_chunks_block_state_cache_limit(),
        }
    }
}

/// 出生点（v4 server::Spawn）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Spawn {
    #[serde(default)]
    pub x: f64,
    #[serde(default)]
    pub y: f64,
    #[serde(default)]
    pub z: f64,
    #[serde(default)]
    pub yaw: f32,
    #[serde(default)]
    pub pitch: f32,
}

impl Default for Spawn {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            yaw: 0.0,
            pitch: 0.0,
        }
    }
}

/// 维度世界规则（v4 qexed_config::app::qexed::world_rules::DimensionWorldRules）。
///
/// v4 通过 AppConfigTrait::load_or_create_default(None, Some(false), Some(dir)) 按
/// 维度目录读写；v6 的 Config trait 无自定义目录参数，rules.rs 里改为按目录直接
/// 读写 TOML 文件（文件名 rules.toml、结构不变），保持 v4 语义。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DimensionWorldRules {
    #[serde(default = "default_dimension")]
    pub dimension: String,
    #[serde(default = "default_dimension_type")]
    pub dimension_type: String,
    #[serde(default)]
    pub read_only: bool,
    #[serde(default = "default_block_updates")]
    pub block_updates: bool,
    #[serde(default)]
    pub light: LightMode,
    #[serde(default)]
    pub light_algorithm: LightAlgorithm,
    #[serde(default)]
    pub time: DimensionTimeRule,
}

impl Default for DimensionWorldRules {
    fn default() -> Self {
        Self {
            dimension: default_dimension(),
            dimension_type: default_dimension_type(),
            read_only: false,
            block_updates: default_block_updates(),
            light: LightMode::default(),
            light_algorithm: LightAlgorithm::default(),
            time: DimensionTimeRule::default(),
        }
    }
}

/// 维度时间规则（v4 world_rules::DimensionTimeRule）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DimensionTimeRule {
    #[serde(default)]
    pub value: i64,
    #[serde(default)]
    pub fixed: Option<i64>,
    #[serde(default = "default_daylight_cycle")]
    pub daylight_cycle: bool,
    #[serde(default = "default_tick_step")]
    pub tick_step: i64,
}

impl Default for DimensionTimeRule {
    fn default() -> Self {
        Self {
            value: 0,
            fixed: None,
            daylight_cycle: default_daylight_cycle(),
            tick_step: default_tick_step(),
        }
    }
}

// ---- 默认值函数（v4 原样） ----

fn default_dimension() -> String {
    "minecraft:overworld".to_string()
}

fn default_dimension_type() -> String {
    "minecraft:overworld".to_string()
}

/// v4 default_world_default_dimension（与 default_dimension 同值；独立命名避免与
/// WorldConfig::default_play_dimension 内的局部变量遮蔽冲突）。
fn default_world_dimension() -> String {
    "minecraft:overworld".to_string()
}

fn default_world_path() -> String {
    "world".to_string()
}

fn default_world_generator_preset() -> String {
    "minecraft:classic_flat".to_string()
}

fn default_view_distance() -> i32 {
    3
}

fn default_chunk_load_parallelism() -> usize {
    4
}

fn default_chunk_update_delay_ms() -> u64 {
    1000
}

fn default_spawn_protection_radius() -> i32 {
    16
}

fn default_world_cluster_mode() -> WorldClusterMode {
    WorldClusterMode::Quadrant
}

fn default_spawn_platform_block() -> String {
    "minecraft:grass_block".to_string()
}

fn default_spawn_platform_y() -> i32 {
    254
}

fn default_spawn_platform_min_x() -> i32 {
    -16
}

fn default_spawn_platform_max_x() -> i32 {
    15
}

fn default_spawn_platform_min_z() -> i32 {
    -16
}

fn default_spawn_platform_max_z() -> i32 {
    15
}

fn default_world_instance_source_dimension() -> String {
    "minecraft:overworld".to_string()
}

fn default_world_instance_copy_on_write() -> bool {
    true
}

fn default_world_ore_pit_enable() -> bool {
    true
}

fn default_world_ore_pit_tick_interval_ms() -> u64 {
    300_000
}

fn default_world_ore_pit_initial_refill() -> bool {
    true
}

fn default_world_ore_pit_max_blocks_per_tick() -> usize {
    64
}

fn default_world_ore_pit_replace_air() -> bool {
    true
}

fn default_world_ore_pit_replace_generated() -> bool {
    true
}

fn default_world_ore_pit_block() -> String {
    "minecraft:stone".to_string()
}

fn default_world_ore_pit_block_weight() -> u32 {
    1
}

fn default_world_edit_region_allow_player_break() -> bool {
    true
}

fn default_world_edit_region_allow_player_place() -> bool {
    true
}

fn default_world_edit_region_allow_plugin_write() -> bool {
    true
}

fn default_world_edit_region_runtime_only() -> bool {
    true
}

fn default_precompiled_chunks_light() -> bool {
    true
}

fn default_precompiled_chunks_max_cached_packets() -> usize {
    256
}

fn default_precompiled_chunks_max_cached_packet_bytes() -> usize {
    16 * 1024 * 1024
}

fn default_precompiled_chunks_block_state_cache_limit() -> usize {
    65_536
}

fn default_block_updates() -> bool {
    true
}

fn default_daylight_cycle() -> bool {
    true
}

fn default_tick_step() -> i64 {
    1
}

// app_config 生成的常量在 tests::app_config_constants_are_stable 里断言。

#[cfg(test)]
mod tests {
    use super::*;
    use qexed_config::Config;

    #[test]
    fn app_config_constants_are_stable() {
        assert_eq!(WorldConfig::PATH, "/");
        assert_eq!(WorldConfig::NAME, "world");
    }

    #[test]
    fn world_config_defaults() {
        let config = WorldConfig::default();
        assert_eq!(config.path, "world");
        assert_eq!(config.default_dimension, "minecraft:overworld");
        assert_eq!(config.light, LightMode::Static);
        assert_eq!(config.light_algorithm, LightAlgorithm::Fast);
        assert_eq!(config.generator, WorldGenerator::Empty);
    }

    #[test]
    fn light_mode_roundtrip_dynamic() {
        let config = WorldConfig {
            light: LightMode::Dynamic,
            light_algorithm: LightAlgorithm::RayTrace,
            ..WorldConfig::default()
        };
        let parsed: WorldConfig =
            qexed_toml::from_document(qexed_toml::to_document(&config).unwrap()).unwrap();
        assert_eq!(parsed.light, LightMode::Dynamic);
        assert_eq!(parsed.light_algorithm, LightAlgorithm::RayTrace);
    }

    #[test]
    fn light_mode_roundtrip_fixed() {
        let config = WorldConfig {
            light: LightMode::Fixed(7),
            ..WorldConfig::default()
        };
        let parsed: WorldConfig =
            qexed_toml::from_document(qexed_toml::to_document(&config).unwrap()).unwrap();
        assert_eq!(parsed.light, LightMode::Fixed(7));
    }

    #[test]
    fn dimension_rules_roundtrip_toml() {
        let rules = DimensionWorldRules::default();
        let parsed: DimensionWorldRules = qexed_toml::from_document(
            qexed_toml::to_document(&rules).unwrap(),
        )
        .unwrap();
        assert_eq!(parsed.dimension, rules.dimension);
        assert_eq!(parsed.time.tick_step, 1);
    }
}

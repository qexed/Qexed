//! v4 world 模块根：世界尺寸常量与子模块声明。
//!
//! - generator（include! 扁平命名空间）：world-features 落地 features 系列与
//!   最小内核；world-core 补齐完整 core/dimensions/nbt（生成管线）与 manager。
//! - light / region / chunk_nbt / optimized_blocks / rules / ore_pits / cluster：
//!   world-misc 任务迁移（本文件常量为其共享）。
#![allow(dead_code)]

pub mod chunk_nbt;
pub mod generator;
mod light;
mod optimized_blocks;
mod manager;
mod ore_pits;
pub mod region;
mod rules;
mod vanilla_noise;

pub mod cluster;

#[allow(unused_imports)]
pub(crate) use cluster::ClusterRouter;
// TODO(world-cluster): ClusteredWorldGenerator 半适配版在 target/migration/cluster_rpc_partial.rs，
// 待 generator trait 定稿后接回
#[cfg(test)]
pub(crate) use light::empty_chunk_section_bytes;
#[allow(unused_imports)]
pub(crate) use light::{
    Light, LightDampeningNeighborhood, LIGHT_ARRAY_BYTES, block_light_dampening_index,
    empty_chunk_packet, empty_heightmaps, light_for_mode, light_from_layers,
    light_section_index, light_update_data, replace_sky_light, section_count,
    sky_light_from_dampening, sky_light_from_neighbourhood, write_empty_section,
    write_fixed_long_array,
};
pub use light::{WorldLightAlgorithm, WorldLightMode};
pub use ore_pits::{OrePitBlockUpdate, OrePitManager};
pub use rules::WorldRulesManager;

pub use manager::{PrecompiledChunkSettings, RuntimeEditRegion, WorldManager, WorldSession};

const OVERWORLD_HEIGHT: i32 = 384;
const SECTION_HEIGHT: i32 = 16;
const LIGHT_SECTION_COUNT: usize = (OVERWORLD_HEIGHT / SECTION_HEIGHT + 2) as usize;
pub(crate) const WORLD_MIN_Y: i32 = -64;
pub(crate) const WORLD_MAX_Y: i32 = WORLD_MIN_Y + OVERWORLD_HEIGHT - 1;
pub(crate) const WORLD_SECTION_COUNT: usize = (OVERWORLD_HEIGHT / SECTION_HEIGHT) as usize;
const CHUNK_DAMPENING_LEN: usize = 16 * WORLD_SECTION_COUNT * 16 * 16;
pub(crate) const WORLD_MIN_SECTION_Y: i32 = WORLD_MIN_Y / SECTION_HEIGHT;
pub(crate) const MIN_LIGHT_SECTION_Y: i32 = WORLD_MIN_SECTION_Y - 1;
const AIR_BLOCK_STATE_ID: i32 = 0;
const PLAINS_BIOME_ID: i32 = 40;

/// v4 world::light::section_count，原样（light::section_count 的再导出已足够，
/// 保留此路径兼容 v4 引用 world::section_count() 的代码）。
pub(crate) fn section_count_alias() -> i32 {
    section_count()
}

#[cfg(test)]
mod tests;

//! v4 `world/generator.rs` 的 v6 对应：通过 include! 维持 v4 的扁平命名空间。
//!
//! v4 的 generator.rs include 了 25 个文件（constants/core/terrain/dimensions/
//! carvers/sampling/features*/support/nbt/tests）共享一个命名空间，features 系列对
//! core/support 的引用不带任何路径前缀，逐文件拆 module 需要改数千处引用，因此
//! 保留 include! 结构。
//!
//! - world-features 任务：features 系列 + 最小内核（constants/sampling/support/
//!   terrain/vanilla_noise/carvers）。
//! - world-misc 任务：chunk_nbt（区块 NBT ↔ 网络包）与 block_registry 别名。
//! - world-core 任务：core.rs / dimensions.rs / nbt.rs 完整版（WorldChunkGenerator
//!   trait、SpawnPlatform/Flat/VanillaNoise 生成器、区块生成管线），删除过渡期的
//!   noise_settings.rs / nbt_layers.rs 子集与临时 trait 定义。
//!
//! 协议适配（26.3）：`MapChunk` → `level_chunk_with_light::LevelChunkWithLight`
//! （GeneratedChunk.packet 字段类型，其余结构不变）；错误 anyhow → WorldError。

#![allow(dead_code)]

use std::{
    collections::{HashMap, HashSet, VecDeque},
    path::{Path, PathBuf},
    sync::{Arc, Condvar, Mutex},
    time::{Duration, Instant},
};

use qexed_nbt::{ListHeader, Tag, tag_id};
use rayon::prelude::*;
use serde::Deserialize;

use crate::config::{WorldConfig, WorldGenerator as WorldGeneratorKind};
use crate::world::{
    CHUNK_DAMPENING_LEN, SECTION_HEIGHT, WORLD_MAX_Y, WORLD_MIN_SECTION_Y, WORLD_MIN_Y,
    WorldLightAlgorithm, chunk_nbt, empty_chunk_packet, section_count,
};
use qexed_protocol::to_client::play::level_chunk_with_light::LevelChunkWithLight;

pub(crate) mod vanilla_noise_adapter {
    //! generator 命名空间内以 `vanilla_noise::` 前缀访问 vanilla noise（v4 中它是
    //! world 模块的兄弟模块，这里 re-export 保持 include 文件里的路径不变）。
    pub(crate) use crate::world::vanilla_noise::*;
}

// 让 include 文件里不带前缀的 vanilla_noise 引用也能解析。
use vanilla_noise_adapter as vanilla_noise;

include!("constants.rs");
include!("sampling.rs");
include!("terrain.rs");
include!("support.rs");
include!("carvers.rs");
include!("nbt.rs");
include!("core.rs");
include!("dimensions.rs");
include!("features.rs");
include!("features_lakes.rs");
include!("features_geodes.rs");
include!("features_cave.rs");
include!("features_aquatic.rs");
include!("features_lush.rs");
include!("features_surface.rs");
include!("features_mushrooms.rs");
include!("features_dripstone.rs");
include!("features_sculk.rs");
include!("features_structures.rs");
include!("features_vegetation.rs");
include!("features_columns.rs");
include!("features_trees.rs");
include!("features_common.rs");
include!("features_ores.rs");

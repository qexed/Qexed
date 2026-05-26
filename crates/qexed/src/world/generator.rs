use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use qexed_config::app::qexed::server::{
    World as WorldConfig, WorldGenerator as WorldGeneratorConfig, WorldGpu,
};
use qexed_nbt::{ListHeader, Tag, tag_id};
use qexed_packet::net_types::{OptionalNbt, VarInt};
use qexed_protocol::to_client::play::map_chunk::{BlockEntities, MapChunk};
use rayon::prelude::*;
use serde::Deserialize;

use super::{
    CHUNK_DAMPENING_LEN, SECTION_HEIGHT, WORLD_MAX_Y, WORLD_MIN_SECTION_Y, WORLD_MIN_Y,
    WorldLightAlgorithm, chunk_nbt, empty_chunk_packet, gpu_worldgen, section_count, vanilla_noise,
};

include!("generator/constants.rs");
include!("generator/core.rs");
include!("generator/terrain.rs");
include!("generator/carvers.rs");
include!("generator/sampling.rs");
include!("generator/features.rs");
include!("generator/features_lakes.rs");
include!("generator/features_geodes.rs");
include!("generator/features_cave.rs");
include!("generator/features_aquatic.rs");
include!("generator/features_lush.rs");
include!("generator/features_surface.rs");
include!("generator/features_mushrooms.rs");
include!("generator/features_dripstone.rs");
include!("generator/features_sculk.rs");
include!("generator/features_structures.rs");
include!("generator/features_vegetation.rs");
include!("generator/features_columns.rs");
include!("generator/features_trees.rs");
include!("generator/features_common.rs");
include!("generator/features_ores.rs");
include!("generator/support.rs");
include!("generator/nbt.rs");
include!("generator/tests.rs");

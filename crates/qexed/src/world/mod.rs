pub mod chunk_nbt;
pub mod region;

use anyhow::{Context, Result};
use bytes::BytesMut;
use qexed_packet::{PacketCodec, PacketWriter, net_types::VarInt};
use qexed_protocol::to_client::play::{
    block_update::BlockUpdate,
    light_update::{LightUpdate, LightUpdateData},
    map_chunk::{Chunk, Heightmaps, LIGHT_ARRAY_BYTES, Light, LightArray, MapChunk},
};
use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, AtomicUsize, Ordering},
    },
};

const OVERWORLD_HEIGHT: i32 = 384;
const SECTION_HEIGHT: i32 = 16;
const LIGHT_SECTION_COUNT: usize = (OVERWORLD_HEIGHT / SECTION_HEIGHT + 2) as usize;
pub(crate) const WORLD_MIN_Y: i32 = -64;
pub(crate) const WORLD_MAX_Y: i32 = WORLD_MIN_Y + OVERWORLD_HEIGHT - 1;
pub(crate) const WORLD_SECTION_COUNT: usize = (OVERWORLD_HEIGHT / SECTION_HEIGHT) as usize;
const CHUNK_DAMPENING_LEN: usize = 16 * WORLD_SECTION_COUNT * 16 * 16;
pub(crate) const WORLD_MIN_SECTION_Y: i32 = WORLD_MIN_Y / SECTION_HEIGHT;
const MIN_LIGHT_SECTION_Y: i32 = WORLD_MIN_SECTION_Y - 1;
const AIR_BLOCK_STATE_ID: i32 = 0;
const PLAINS_BIOME_ID: i32 = 40;

#[derive(Clone, Debug)]
pub struct WorldManager {
    save_path: std::path::PathBuf,
    light_mode: WorldLightMode,
    light_algorithm: WorldLightAlgorithm,
    placed_blocks: Arc<Mutex<std::collections::HashMap<BlockKey, i32>>>,
    chunk_light_dampening: Arc<Mutex<std::collections::HashMap<ChunkKey, Vec<u8>>>>,
    active_sessions: Arc<AtomicUsize>,
    cache_epoch: Arc<AtomicU64>,
}

impl WorldManager {
    pub fn new(save_path: impl Into<std::path::PathBuf>) -> Self {
        Self::with_light_mode(
            save_path,
            WorldLightMode::default(),
            WorldLightAlgorithm::default(),
        )
    }

    pub fn with_light_mode(
        save_path: impl Into<std::path::PathBuf>,
        light_mode: WorldLightMode,
        light_algorithm: WorldLightAlgorithm,
    ) -> Self {
        Self {
            save_path: save_path.into(),
            light_mode,
            light_algorithm,
            placed_blocks: Default::default(),
            chunk_light_dampening: Default::default(),
            active_sessions: Default::default(),
            cache_epoch: Default::default(),
        }
    }

    pub fn save_path(&self) -> &std::path::Path {
        &self.save_path
    }

    pub fn begin_session(&self) -> WorldSession {
        self.active_sessions.fetch_add(1, Ordering::AcqRel);
        WorldSession {
            world: self.clone(),
            active: true,
        }
    }

    pub fn cache_epoch(&self) -> u64 {
        self.cache_epoch.load(Ordering::Acquire)
    }

    #[cfg(test)]
    fn cached_light_chunk_count(&self) -> usize {
        self.chunk_light_dampening
            .lock()
            .expect("world light cache poisoned")
            .len()
    }

    pub fn network_chunk(&self, dimension: &str, chunk_x: i32, chunk_z: i32) -> Result<MapChunk> {
        self.network_chunk_inner(dimension, chunk_x, chunk_z, None)
    }

    pub fn network_chunk_for_session(
        &self,
        dimension: &str,
        chunk_x: i32,
        chunk_z: i32,
        cache_epoch: u64,
    ) -> Result<MapChunk> {
        if !self.cache_epoch_is_active(cache_epoch) {
            anyhow::bail!("chunk load cancelled because world session expired");
        }
        self.network_chunk_inner(dimension, chunk_x, chunk_z, Some(cache_epoch))
    }

    fn network_chunk_inner(
        &self,
        dimension: &str,
        chunk_x: i32,
        chunk_z: i32,
        cache_epoch: Option<u64>,
    ) -> Result<MapChunk> {
        if let Some(chunk) = self.load_region_chunk(dimension, chunk_x, chunk_z)? {
            match chunk_nbt::network_chunk_and_light_dampening_from_region(
                chunk_x,
                chunk_z,
                &chunk,
                WorldLightAlgorithm::Fast,
            ) {
                Ok((mut packet, dampening)) => {
                    self.remember_chunk_light_dampening(
                        dimension,
                        chunk_x,
                        chunk_z,
                        dampening,
                        cache_epoch,
                    );
                    if let WorldLightMode::Fixed(_) = self.light_mode {
                        packet.light = self.chunk_light();
                    } else {
                        let algorithm_light =
                            self.calculated_chunk_light(dimension, chunk_x, chunk_z, cache_epoch);
                        replace_sky_light(&mut packet.light, algorithm_light);
                    }
                    log::debug!(
                        "loaded saved chunk as network chunk: dimension={dimension}, chunk=({chunk_x}, {chunk_z})"
                    );
                    return Ok(packet);
                }
                Err(err) => {
                    log::warn!(
                        "failed to convert saved chunk, falling back to empty chunk: dimension={dimension}, chunk=({chunk_x}, {chunk_z}), error={err:#}"
                    );
                }
            }
        }

        log::trace!("生成空世界区块: dimension={dimension}, chunk=({chunk_x}, {chunk_z})");
        self.remember_chunk_light_dampening(
            dimension,
            chunk_x,
            chunk_z,
            vec![0; CHUNK_DAMPENING_LEN],
            cache_epoch,
        );
        Ok(empty_chunk_packet(chunk_x, chunk_z, self.light_mode))
    }

    pub fn dynamic_light_enabled(&self) -> bool {
        self.light_mode == WorldLightMode::Dynamic
    }

    pub fn light_update(&self, dimension: &str, chunk_x: i32, chunk_z: i32) -> LightUpdate {
        LightUpdate {
            chunk_x: VarInt(chunk_x),
            chunk_z: VarInt(chunk_z),
            light: light_update_data(self.chunk_light_for_update(dimension, chunk_x, chunk_z)),
        }
    }

    fn chunk_light(&self) -> Light {
        light_for_mode(self.light_mode)
    }

    pub fn load_region_chunk(
        &self,
        dimension: &str,
        chunk_x: i32,
        chunk_z: i32,
    ) -> Result<Option<region::ChunkData>> {
        log::trace!(
            "从存档读取区块: dimension={dimension}, chunk=({chunk_x}, {chunk_z}), root={}",
            self.save_path.display()
        );
        let region_path = self.dimension_region_path(dimension).join(format!(
            "r.{}.{}.mca",
            floor_div(chunk_x, 32),
            floor_div(chunk_z, 32)
        ));
        if !region_path.exists() {
            return Ok(None);
        }

        let region = region::AnvilRegion::from_file(&region_path)
            .with_context(|| format!("读取区域文件失败: {}", region_path.display()))?;
        region
            .read_chunk(chunk_x, chunk_z)
            .with_context(|| format!("读取区块失败: {chunk_x}, {chunk_z}"))
    }

    pub fn write_region_chunk(
        &self,
        dimension: &str,
        chunk_x: i32,
        chunk_z: i32,
        chunk: region::ChunkData,
    ) -> Result<()> {
        log::trace!(
            "写入区块到存档: dimension={dimension}, chunk=({chunk_x}, {chunk_z}), root={}",
            self.save_path.display()
        );
        let region_dir = self.dimension_region_path(dimension);
        std::fs::create_dir_all(&region_dir)
            .with_context(|| format!("创建区域目录失败: {}", region_dir.display()))?;

        let region_path = region_dir.join(format!(
            "r.{}.{}.mca",
            floor_div(chunk_x, 32),
            floor_div(chunk_z, 32)
        ));
        let mut region = if region_path.exists() {
            region::AnvilRegion::from_file(&region_path)?
        } else {
            region::AnvilRegion::new(&region_path)
        };
        region.write_chunk(chunk_x, chunk_z, chunk)?;
        region.save()?;
        Ok(())
    }

    pub fn ensure_storage(&self, dimension: &str) -> Result<()> {
        std::fs::create_dir_all(self.dimension_region_path(dimension))
            .with_context(|| format!("创建世界存档目录失败: {}", self.save_path.display()))
    }

    pub fn place_block(
        &self,
        dimension: &str,
        position: qexed_packet::net_types::Position,
        block_state: i32,
    ) {
        self.placed_blocks
            .lock()
            .expect("world block store poisoned")
            .insert(BlockKey::new(dimension, &position), block_state);
        self.mark_placed_block_light_dampening(
            dimension,
            &position,
            if block_state == AIR_BLOCK_STATE_ID {
                0
            } else {
                15
            },
        );
    }

    pub fn block_state_at(
        &self,
        dimension: &str,
        position: &qexed_packet::net_types::Position,
    ) -> Option<i32> {
        self.placed_blocks
            .lock()
            .expect("world block store poisoned")
            .get(&BlockKey::new(dimension, position))
            .copied()
    }

    pub fn placed_block_updates(
        &self,
        dimension: &str,
        chunk_x: i32,
        chunk_z: i32,
    ) -> Vec<BlockUpdate> {
        self.placed_blocks
            .lock()
            .expect("world block store poisoned")
            .iter()
            .filter_map(|(key, block_state)| {
                (key.dimension == dimension
                    && key.x.div_euclid(16) == chunk_x
                    && key.z.div_euclid(16) == chunk_z)
                    .then(|| BlockUpdate {
                        location: qexed_packet::net_types::Position {
                            x: key.x,
                            y: key.y,
                            z: key.z,
                        },
                        block_state: VarInt(*block_state),
                    })
            })
            .collect()
    }

    fn dimension_region_path(&self, dimension: &str) -> std::path::PathBuf {
        match dimension {
            "minecraft:the_nether" => self.save_path.join("DIM-1").join("region"),
            "minecraft:the_end" => self.save_path.join("DIM1").join("region"),
            _ => self.save_path.join("region"),
        }
    }

    fn remember_chunk_light_dampening(
        &self,
        dimension: &str,
        chunk_x: i32,
        chunk_z: i32,
        dampening: Vec<u8>,
        cache_epoch: Option<u64>,
    ) {
        let mut chunks = self
            .chunk_light_dampening
            .lock()
            .expect("world light cache poisoned");
        if cache_epoch.is_some_and(|epoch| !self.cache_epoch_is_active(epoch)) {
            return;
        }
        chunks.insert(ChunkKey::new(dimension, chunk_x, chunk_z), dampening);
    }

    fn mark_placed_block_light_dampening(
        &self,
        dimension: &str,
        position: &qexed_packet::net_types::Position,
        dampening: u8,
    ) {
        if !(WORLD_MIN_Y..=WORLD_MAX_Y).contains(&position.y) {
            return;
        }

        let chunk_x = position.x.div_euclid(16);
        let chunk_z = position.z.div_euclid(16);
        let local_x = position.x.rem_euclid(16) as usize;
        let local_z = position.z.rem_euclid(16) as usize;
        let mut chunks = self
            .chunk_light_dampening
            .lock()
            .expect("world light cache poisoned");
        let light_dampening = chunks
            .entry(ChunkKey::new(dimension, chunk_x, chunk_z))
            .or_insert_with(|| vec![0; CHUNK_DAMPENING_LEN]);
        light_dampening[block_light_dampening_index(local_x, position.y, local_z)] =
            dampening.min(15);
    }

    fn chunk_light_for_update(&self, dimension: &str, chunk_x: i32, chunk_z: i32) -> Light {
        match self.light_mode {
            WorldLightMode::Static | WorldLightMode::Fixed(_) => self.chunk_light(),
            WorldLightMode::Dynamic => {
                self.calculated_chunk_light(dimension, chunk_x, chunk_z, None)
            }
        }
    }

    fn calculated_chunk_light(
        &self,
        dimension: &str,
        chunk_x: i32,
        chunk_z: i32,
        cache_epoch: Option<u64>,
    ) -> Light {
        if let WorldLightMode::Fixed(_) = self.light_mode {
            return self.chunk_light();
        }

        match self.light_algorithm {
            WorldLightAlgorithm::Fast => {
                let neighbourhood =
                    self.chunk_light_neighbourhood(dimension, chunk_x, chunk_z, cache_epoch);
                sky_light_from_neighbourhood(&neighbourhood, WorldLightAlgorithm::Fast)
            }
            WorldLightAlgorithm::RayTrace => {
                let neighbourhood =
                    self.chunk_light_neighbourhood(dimension, chunk_x, chunk_z, cache_epoch);
                sky_light_from_neighbourhood(&neighbourhood, WorldLightAlgorithm::RayTrace)
            }
        }
    }

    fn chunk_light_neighbourhood(
        &self,
        dimension: &str,
        center_x: i32,
        center_z: i32,
        cache_epoch: Option<u64>,
    ) -> LightDampeningNeighborhood {
        let mut neighbourhood = LightDampeningNeighborhood::default();
        for offset_z in -1..=1 {
            for offset_x in -1..=1 {
                let chunk_x = center_x + offset_x;
                let chunk_z = center_z + offset_z;
                if let Some(dampening) = self.cached_or_load_chunk_light_dampening(
                    dimension,
                    chunk_x,
                    chunk_z,
                    cache_epoch,
                ) {
                    neighbourhood.set(offset_x, offset_z, dampening);
                }
            }
        }
        neighbourhood
    }

    fn cached_or_load_chunk_light_dampening(
        &self,
        dimension: &str,
        chunk_x: i32,
        chunk_z: i32,
        cache_epoch: Option<u64>,
    ) -> Option<Vec<u8>> {
        let key = ChunkKey::new(dimension, chunk_x, chunk_z);
        if let Some(dampening) = self
            .chunk_light_dampening
            .lock()
            .expect("world light cache poisoned")
            .get(&key)
            .cloned()
        {
            return Some(dampening);
        }

        let chunk = match self.load_region_chunk(dimension, chunk_x, chunk_z) {
            Ok(Some(chunk)) => chunk,
            Ok(None) => return Some(vec![0; CHUNK_DAMPENING_LEN]),
            Err(err) => {
                log::warn!(
                    "failed to load neighbour chunk light data: dimension={dimension}, chunk=({chunk_x}, {chunk_z}), error={err:#}"
                );
                return None;
            }
        };

        match chunk_nbt::light_dampening_from_region(&chunk) {
            Ok(dampening) => {
                self.remember_chunk_light_dampening(
                    dimension,
                    chunk_x,
                    chunk_z,
                    dampening.clone(),
                    cache_epoch,
                );
                Some(dampening)
            }
            Err(err) => {
                log::warn!(
                    "failed to parse neighbour chunk light data: dimension={dimension}, chunk=({chunk_x}, {chunk_z}), error={err:#}"
                );
                None
            }
        }
    }

    fn cache_epoch_is_active(&self, cache_epoch: u64) -> bool {
        self.cache_epoch.load(Ordering::Acquire) == cache_epoch
            && self.active_sessions.load(Ordering::Acquire) > 0
    }

    fn end_session(&self) {
        if self.active_sessions.fetch_sub(1, Ordering::AcqRel) != 1 {
            return;
        }

        self.cache_epoch.fetch_add(1, Ordering::AcqRel);
        let mut chunks = self
            .chunk_light_dampening
            .lock()
            .expect("world light cache poisoned");
        let cleared = chunks.len();
        *chunks = std::collections::HashMap::new();
        if cleared > 0 {
            log::debug!("已清理世界区块光照缓存: chunks={cleared}");
        }
    }
}

#[derive(Debug)]
pub struct WorldSession {
    world: WorldManager,
    active: bool,
}

impl Drop for WorldSession {
    fn drop(&mut self) {
        if self.active {
            self.world.end_session();
            self.active = false;
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct BlockKey {
    dimension: String,
    x: i32,
    y: i32,
    z: i32,
}

impl BlockKey {
    fn new(dimension: &str, position: &qexed_packet::net_types::Position) -> Self {
        Self {
            dimension: dimension.to_string(),
            x: position.x,
            y: position.y,
            z: position.z,
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct ChunkKey {
    dimension: String,
    x: i32,
    z: i32,
}

impl ChunkKey {
    fn new(dimension: &str, x: i32, z: i32) -> Self {
        Self {
            dimension: dimension.to_string(),
            x,
            z,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorldLightMode {
    Static,
    Dynamic,
    Fixed(u8),
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum WorldLightAlgorithm {
    #[default]
    Fast,
    RayTrace,
}

impl From<&qexed_config::app::qexed::server::LightAlgorithm> for WorldLightAlgorithm {
    fn from(value: &qexed_config::app::qexed::server::LightAlgorithm) -> Self {
        match value {
            qexed_config::app::qexed::server::LightAlgorithm::Fast => Self::Fast,
            qexed_config::app::qexed::server::LightAlgorithm::RayTrace => Self::RayTrace,
        }
    }
}

#[derive(Clone, Debug)]
struct LightDampeningNeighborhood {
    chunks: [Option<Vec<u8>>; 9],
}

impl Default for LightDampeningNeighborhood {
    fn default() -> Self {
        Self {
            chunks: std::array::from_fn(|_| None),
        }
    }
}

impl LightDampeningNeighborhood {
    fn single(center: &[u8]) -> Self {
        let mut neighbourhood = Self::default();
        neighbourhood.set(0, 0, center.to_vec());
        neighbourhood
    }

    fn set(&mut self, offset_x: i32, offset_z: i32, dampening: Vec<u8>) {
        if let Some(index) = Self::chunk_index(offset_x, offset_z) {
            self.chunks[index] = Some(dampening);
        }
    }

    fn dampening_at(&self, x: i32, y: i32, z: i32) -> Option<u8> {
        if !(WORLD_MIN_Y..=WORLD_MAX_Y).contains(&y) {
            return None;
        }

        let chunk_offset_x = x.div_euclid(16);
        let chunk_offset_z = z.div_euclid(16);
        let local_x = x.rem_euclid(16) as usize;
        let local_z = z.rem_euclid(16) as usize;
        let chunk = self.chunks[Self::chunk_index(chunk_offset_x, chunk_offset_z)?].as_ref()?;
        chunk
            .get(block_light_dampening_index(local_x, y, local_z))
            .copied()
    }

    fn center(&self) -> Option<&[u8]> {
        self.chunks[Self::chunk_index(0, 0).expect("center chunk index")]
            .as_ref()
            .map(Vec::as_slice)
    }

    fn chunk_index(offset_x: i32, offset_z: i32) -> Option<usize> {
        ((-1..=1).contains(&offset_x) && (-1..=1).contains(&offset_z))
            .then_some(((offset_z + 1) * 3 + (offset_x + 1)) as usize)
    }
}

impl Default for WorldLightMode {
    fn default() -> Self {
        Self::Static
    }
}

impl From<&qexed_config::app::qexed::server::LightMode> for WorldLightMode {
    fn from(value: &qexed_config::app::qexed::server::LightMode) -> Self {
        match value {
            qexed_config::app::qexed::server::LightMode::Static => Self::Static,
            qexed_config::app::qexed::server::LightMode::Dynamic => Self::Dynamic,
            qexed_config::app::qexed::server::LightMode::Fixed(level) => Self::Fixed(*level),
        }
    }
}

pub(crate) fn empty_chunk_packet(
    chunk_x: i32,
    chunk_z: i32,
    light_mode: WorldLightMode,
) -> MapChunk {
    MapChunk {
        chunk_x,
        chunk_z,
        data: Chunk {
            heightmaps: empty_heightmaps(),
            data: empty_chunk_section_bytes(),
            block_entities: Vec::new(),
        },
        light: light_for_mode(light_mode),
    }
}

pub(crate) fn empty_heightmaps() -> Vec<Heightmaps> {
    vec![
        Heightmaps {
            type_id: VarInt(1),
            data: vec![0; 37],
        },
        Heightmaps {
            type_id: VarInt(4),
            data: vec![0; 37],
        },
        Heightmaps {
            type_id: VarInt(5),
            data: vec![0; 37],
        },
    ]
}

pub(crate) fn empty_light() -> Light {
    light_for_mode(WorldLightMode::Static)
}

pub(crate) fn light_for_mode(mode: WorldLightMode) -> Light {
    let (sky_level, block_level) = match mode {
        WorldLightMode::Static | WorldLightMode::Dynamic => (15, 0),
        WorldLightMode::Fixed(level) => {
            let level = level.min(15);
            (level, level)
        }
    };

    Light {
        sky_light_mask: filled_light_mask(sky_level),
        block_light_mask: filled_light_mask(block_level),
        empty_sky_light_mask: empty_filled_light_mask(sky_level),
        empty_block_light_mask: empty_filled_light_mask(block_level),
        sky_light_arrays: filled_light_arrays(sky_level),
        block_light_arrays: filled_light_arrays(block_level),
    }
}

pub(crate) fn sky_light_from_dampening(
    block_dampening: &[u8],
    algorithm: WorldLightAlgorithm,
) -> Light {
    if block_dampening.len() != CHUNK_DAMPENING_LEN {
        return light_for_mode(WorldLightMode::Static);
    }

    sky_light_from_neighbourhood(
        &LightDampeningNeighborhood::single(block_dampening),
        algorithm,
    )
}

fn sky_light_from_neighbourhood(
    neighbourhood: &LightDampeningNeighborhood,
    algorithm: WorldLightAlgorithm,
) -> Light {
    let Some(center) = neighbourhood.center() else {
        return light_for_mode(WorldLightMode::Static);
    };

    if center.len() != CHUNK_DAMPENING_LEN {
        return light_for_mode(WorldLightMode::Static);
    }

    let sky_values = match algorithm {
        WorldLightAlgorithm::Fast => fast_sky_light_values_from_neighbourhood(neighbourhood),
        WorldLightAlgorithm::RayTrace => ray_trace_sky_light_values(neighbourhood),
    };

    light_from_sky_values(&sky_values)
}

fn fast_sky_light_values_from_neighbourhood(neighbourhood: &LightDampeningNeighborhood) -> Vec<u8> {
    let Some(center) = neighbourhood.center() else {
        return vec![0_u8; CHUNK_DAMPENING_LEN];
    };
    let neighbourhood_sky = fast_neighbourhood_sky_values(neighbourhood);
    let mut sky_values = neighbourhood_sky
        [LightDampeningNeighborhood::chunk_index(0, 0).expect("center chunk index")]
    .clone()
    .unwrap_or_else(|| fast_sky_light_values(center));
    seed_neighbour_sky_light(center, &neighbourhood_sky, &mut sky_values, None);
    sky_values
}

fn fast_sky_light_values(block_dampening: &[u8]) -> Vec<u8> {
    let mut sky_values = vec![0_u8; block_dampening.len()];
    let mut queue = VecDeque::new();
    for x in 0..16 {
        for z in 0..16 {
            let mut light = 15_u8;
            for y in (WORLD_MIN_Y..=WORLD_MAX_Y).rev() {
                let index = block_light_dampening_index(x, y, z);
                if light > sky_values[index] {
                    sky_values[index] = light;
                    queue.push_back((x, y, z));
                }
                light = light.saturating_sub(block_dampening[index].min(15));
            }
        }
    }
    spread_sky_light(block_dampening, &mut sky_values, queue);
    sky_values
}

fn ray_trace_sky_light_values(neighbourhood: &LightDampeningNeighborhood) -> Vec<u8> {
    let Some(center) = neighbourhood.center() else {
        return vec![0_u8; CHUNK_DAMPENING_LEN];
    };
    let neighbourhood_sky = fast_neighbourhood_sky_values(neighbourhood);
    let mut sky_values = neighbourhood_sky
        [LightDampeningNeighborhood::chunk_index(0, 0).expect("center chunk index")]
    .clone()
    .unwrap_or_else(|| fast_sky_light_values(center));
    seed_neighbour_sky_light(center, &neighbourhood_sky, &mut sky_values, Some(15));
    let rays = [
        (0, 1, 0),
        (-1, 1, 0),
        (1, 1, 0),
        (0, 1, -1),
        (0, 1, 1),
        (-1, 1, -1),
        (-1, 1, 1),
        (1, 1, -1),
        (1, 1, 1),
    ];

    let mut queue = VecDeque::new();
    for x in 0..16 {
        for z in 0..16 {
            for y in WORLD_MIN_Y..=WORLD_MAX_Y {
                let index = block_light_dampening_index(x, y, z);
                let old = sky_values[index];
                let mut best = old;
                for (dx, dy, dz) in rays {
                    best = best.max(trace_sky_ray(
                        neighbourhood,
                        &neighbourhood_sky,
                        x,
                        y,
                        z,
                        dx,
                        dy,
                        dz,
                    ));
                    if best == 15 {
                        break;
                    }
                }
                if best > old {
                    sky_values[index] = best;
                    queue.push_back((x, y, z));
                }
            }
        }
    }
    spread_sky_light(center, &mut sky_values, queue);

    sky_values
}

fn fast_neighbourhood_sky_values(
    neighbourhood: &LightDampeningNeighborhood,
) -> [Option<Vec<u8>>; 9] {
    std::array::from_fn(|index| {
        neighbourhood.chunks[index]
            .as_ref()
            .map(|dampening| fast_sky_light_values(dampening))
    })
}

fn seed_neighbour_sky_light(
    center_dampening: &[u8],
    neighbourhood_sky: &[Option<Vec<u8>>; 9],
    sky_values: &mut [u8],
    missing_source_light: Option<u8>,
) {
    let mut queue = VecDeque::new();
    for y in WORLD_MIN_Y..=WORLD_MAX_Y {
        for z in 0..16 {
            seed_neighbour_sky_cell(
                center_dampening,
                neighbourhood_sky,
                sky_values,
                &mut queue,
                (0, y, z),
                (-1, y, z as i32),
                missing_source_light,
            );
            seed_neighbour_sky_cell(
                center_dampening,
                neighbourhood_sky,
                sky_values,
                &mut queue,
                (15, y, z),
                (16, y, z as i32),
                missing_source_light,
            );
        }
        for x in 0..16 {
            seed_neighbour_sky_cell(
                center_dampening,
                neighbourhood_sky,
                sky_values,
                &mut queue,
                (x, y, 0),
                (x as i32, y, -1),
                missing_source_light,
            );
            seed_neighbour_sky_cell(
                center_dampening,
                neighbourhood_sky,
                sky_values,
                &mut queue,
                (x, y, 15),
                (x as i32, y, 16),
                missing_source_light,
            );
        }
    }
    spread_sky_light(center_dampening, sky_values, queue);
}

fn seed_neighbour_sky_cell(
    center_dampening: &[u8],
    neighbourhood_sky: &[Option<Vec<u8>>; 9],
    sky_values: &mut [u8],
    queue: &mut VecDeque<(usize, i32, usize)>,
    local: (usize, i32, usize),
    source: (i32, i32, i32),
    missing_source_light: Option<u8>,
) {
    let Some(source_light) =
        sky_at(neighbourhood_sky, source.0, source.1, source.2).or(missing_source_light)
    else {
        return;
    };
    if source_light <= 1 {
        return;
    }

    let index = block_light_dampening_index(local.0, local.1, local.2);
    let dampening = center_dampening[index].min(15).max(1);
    let candidate = source_light.saturating_sub(dampening);
    if candidate > sky_values[index] {
        sky_values[index] = candidate;
        queue.push_back(local);
    }
}

fn trace_sky_ray(
    neighbourhood: &LightDampeningNeighborhood,
    neighbourhood_sky: &[Option<Vec<u8>>; 9],
    x: usize,
    y: i32,
    z: usize,
    dx: i32,
    dy: i32,
    dz: i32,
) -> u8 {
    let mut cx = x as i32;
    let mut cy = y;
    let mut cz = z as i32;
    let mut loss = 0_u8;

    loop {
        cx += dx;
        cy += dy;
        cz += dz;
        if cy > WORLD_MAX_Y {
            return 15_u8.saturating_sub(loss);
        }
        if cy < WORLD_MIN_Y {
            return 0;
        }

        let Some(dampening) = neighbourhood.dampening_at(cx, cy, cz) else {
            return 15_u8.saturating_sub(loss);
        };
        let dampening = dampening.min(15);
        loss = loss.saturating_add(dampening);
        if loss >= 15 {
            return 0;
        }
        if let Some(light) = sky_at(neighbourhood_sky, cx, cy, cz) {
            if light > loss {
                return light.saturating_sub(loss);
            }
        }
    }
}

fn sky_at(neighbourhood_sky: &[Option<Vec<u8>>; 9], x: i32, y: i32, z: i32) -> Option<u8> {
    if !(WORLD_MIN_Y..=WORLD_MAX_Y).contains(&y) {
        return None;
    }

    let chunk_offset_x = x.div_euclid(16);
    let chunk_offset_z = z.div_euclid(16);
    let local_x = x.rem_euclid(16) as usize;
    let local_z = z.rem_euclid(16) as usize;
    let chunk = neighbourhood_sky
        [LightDampeningNeighborhood::chunk_index(chunk_offset_x, chunk_offset_z)?]
    .as_ref()?;
    chunk
        .get(block_light_dampening_index(local_x, y, local_z))
        .copied()
}

fn light_from_sky_values(sky_values: &[u8]) -> Light {
    let mut sky_layers = vec![LightArray::default(); LIGHT_SECTION_COUNT];
    for x in 0..16 {
        for z in 0..16 {
            for y in WORLD_MIN_Y..=WORLD_MAX_Y {
                let section_index = light_section_index(y.div_euclid(SECTION_HEIGHT));
                if let Some(section_index) = section_index {
                    let light = sky_values[block_light_dampening_index(x, y, z)];
                    set_light_value(&mut sky_layers[section_index], x, y, z, light);
                }
            }
            set_light_column(&mut sky_layers[LIGHT_SECTION_COUNT - 1], x, z, 15);
        }
    }
    light_from_layers(sky_layers, vec![LightArray::default(); LIGHT_SECTION_COUNT])
}

fn spread_sky_light(
    block_dampening: &[u8],
    sky_values: &mut [u8],
    mut queue: VecDeque<(usize, i32, usize)>,
) {
    while let Some((x, y, z)) = queue.pop_front() {
        let source = sky_values[block_light_dampening_index(x, y, z)];
        if source <= 1 {
            continue;
        }

        for (nx, ny, nz) in light_neighbours(x, y, z) {
            let index = block_light_dampening_index(nx, ny, nz);
            let dampening = block_dampening[index].min(15).max(1);
            let candidate = source.saturating_sub(dampening);
            if candidate > sky_values[index] {
                sky_values[index] = candidate;
                queue.push_back((nx, ny, nz));
            }
        }
    }
}

fn light_neighbours(x: usize, y: i32, z: usize) -> impl Iterator<Item = (usize, i32, usize)> {
    [
        (x > 0).then(|| (x - 1, y, z)),
        (x < 15).then(|| (x + 1, y, z)),
        (y > WORLD_MIN_Y).then(|| (x, y - 1, z)),
        (y < WORLD_MAX_Y).then(|| (x, y + 1, z)),
        (z > 0).then(|| (x, y, z - 1)),
        (z < 15).then(|| (x, y, z + 1)),
    ]
    .into_iter()
    .flatten()
}

fn light_update_data(light: Light) -> LightUpdateData {
    LightUpdateData {
        sky_light_mask: light.sky_light_mask,
        block_light_mask: light.block_light_mask,
        empty_sky_light_mask: light.empty_sky_light_mask,
        empty_block_light_mask: light.empty_block_light_mask,
        sky_light_arrays: light.sky_light_arrays,
        block_light_arrays: light.block_light_arrays,
    }
}

fn replace_sky_light(target: &mut Light, source: Light) {
    target.sky_light_mask = source.sky_light_mask;
    target.empty_sky_light_mask = source.empty_sky_light_mask;
    target.sky_light_arrays = source.sky_light_arrays;
}

fn filled_light_mask(level: u8) -> qexed_packet::net_types::Bitset {
    if level == 0 {
        qexed_packet::net_types::Bitset(Vec::new())
    } else {
        full_light_section_mask()
    }
}

fn empty_filled_light_mask(level: u8) -> qexed_packet::net_types::Bitset {
    if level == 0 {
        full_light_section_mask()
    } else {
        qexed_packet::net_types::Bitset(Vec::new())
    }
}

fn full_light_section_mask() -> qexed_packet::net_types::Bitset {
    qexed_packet::net_types::Bitset(vec![(1_u64 << LIGHT_SECTION_COUNT) - 1])
}

fn filled_light_arrays(level: u8) -> Vec<LightArray> {
    if level == 0 {
        Vec::new()
    } else {
        let packed = (level.min(15) & 0x0f) | ((level.min(15) & 0x0f) << 4);
        vec![LightArray([packed; LIGHT_ARRAY_BYTES]); LIGHT_SECTION_COUNT]
    }
}

pub(crate) fn light_from_layers(
    sky_layers: Vec<LightArray>,
    block_layers: Vec<LightArray>,
) -> Light {
    let (sky_light_mask, empty_sky_light_mask, sky_light_arrays) = split_light_layers(sky_layers);
    let (block_light_mask, empty_block_light_mask, block_light_arrays) =
        split_light_layers(block_layers);

    Light {
        sky_light_mask,
        block_light_mask,
        empty_sky_light_mask,
        empty_block_light_mask,
        sky_light_arrays,
        block_light_arrays,
    }
}

fn split_light_layers(
    layers: Vec<LightArray>,
) -> (
    qexed_packet::net_types::Bitset,
    qexed_packet::net_types::Bitset,
    Vec<LightArray>,
) {
    let mut light_mask = 0_u64;
    let mut empty_mask = 0_u64;
    let mut arrays = Vec::new();

    for (index, layer) in layers.into_iter().enumerate() {
        if layer.0.iter().all(|value| *value == 0) {
            empty_mask |= 1_u64 << index;
        } else {
            light_mask |= 1_u64 << index;
            arrays.push(layer);
        }
    }

    (
        qexed_packet::net_types::Bitset(non_empty_bitset_words(light_mask)),
        qexed_packet::net_types::Bitset(non_empty_bitset_words(empty_mask)),
        arrays,
    )
}

fn non_empty_bitset_words(mask: u64) -> Vec<u64> {
    if mask == 0 { Vec::new() } else { vec![mask] }
}

pub(crate) fn block_light_dampening_index(x: usize, y: i32, z: usize) -> usize {
    debug_assert!(x < 16);
    debug_assert!(z < 16);
    debug_assert!((WORLD_MIN_Y..=WORLD_MAX_Y).contains(&y));
    ((y - WORLD_MIN_Y) as usize * 16 + z) * 16 + x
}

pub(crate) fn light_section_index(section_y: i32) -> Option<usize> {
    let index = section_y - MIN_LIGHT_SECTION_Y;
    (0..LIGHT_SECTION_COUNT as i32)
        .contains(&index)
        .then_some(index as usize)
}

fn set_light_column(layer: &mut LightArray, x: usize, z: usize, level: u8) {
    for y in 0..16 {
        set_section_light(layer, x, y, z, level);
    }
}

fn set_light_value(layer: &mut LightArray, x: usize, y: i32, z: usize, level: u8) {
    set_section_light(layer, x, y.rem_euclid(SECTION_HEIGHT) as usize, z, level);
}

fn set_section_light(layer: &mut LightArray, x: usize, y: usize, z: usize, level: u8) {
    let block_index = y * 16 * 16 + z * 16 + x;
    let byte_index = block_index >> 1;
    let level = level.min(15);
    if block_index & 1 == 0 {
        layer.0[byte_index] = (layer.0[byte_index] & 0xf0) | level;
    } else {
        layer.0[byte_index] = (layer.0[byte_index] & 0x0f) | (level << 4);
    }
}

fn empty_chunk_section_bytes() -> Vec<u8> {
    let mut bytes = BytesMut::new();
    let mut writer = PacketWriter::new(&mut bytes);

    for _ in 0..section_count() {
        write_empty_section(&mut writer).expect("empty section packet data is infallible");
    }

    bytes.to_vec()
}

pub(crate) fn write_empty_section(writer: &mut PacketWriter) -> Result<()> {
    0_i16.serialize(writer)?;
    0_i16.serialize(writer)?;
    write_single_value_palette(writer, AIR_BLOCK_STATE_ID)?;
    write_single_value_palette(writer, PLAINS_BIOME_ID)?;
    Ok(())
}

pub(crate) fn write_single_value_palette(
    writer: &mut PacketWriter,
    registry_id: i32,
) -> Result<()> {
    0_u8.serialize(writer)?;
    VarInt(registry_id).serialize(writer)?;
    write_fixed_long_array(writer, &[])
}

pub(crate) fn write_fixed_long_array(writer: &mut PacketWriter, values: &[u64]) -> Result<()> {
    for value in values {
        value.serialize(writer)?;
    }
    Ok(())
}

pub(crate) fn section_count() -> i32 {
    OVERWORLD_HEIGHT / SECTION_HEIGHT
}

fn floor_div(value: i32, divisor: i32) -> i32 {
    value.div_euclid(divisor)
}

#[cfg(test)]
mod tests {
    use qexed_packet::{Packet, PacketCodec};

    use super::{
        LIGHT_SECTION_COUNT, WORLD_MAX_Y, WorldLightAlgorithm, WorldLightMode, WorldManager,
        block_light_dampening_index, empty_chunk_section_bytes, light_for_mode, section_count,
        sky_light_from_dampening, sky_light_from_neighbourhood,
    };
    use qexed_protocol::to_client::play::map_chunk::LIGHT_ARRAY_BYTES;

    #[test]
    fn empty_chunk_has_all_overworld_sections() {
        let bytes = empty_chunk_section_bytes();
        assert_eq!(bytes.len(), section_count() as usize * 8);
    }

    #[test]
    fn empty_network_chunk_serializes() {
        let manager = WorldManager::new("world");
        let chunk = manager.network_chunk("minecraft:overworld", 0, 0).unwrap();
        let mut payload = bytes::BytesMut::new();
        let mut writer = qexed_packet::PacketWriter::new(&mut payload);

        chunk.serialize(&mut writer).unwrap();

        assert!(!payload.is_empty());
    }

    #[test]
    fn static_light_sends_full_sky_light_layers() {
        let light = light_for_mode(WorldLightMode::Static);

        assert_eq!(light.sky_light_arrays.len(), LIGHT_SECTION_COUNT);
        assert!(
            light
                .sky_light_arrays
                .iter()
                .all(|layer| layer.0.len() == LIGHT_ARRAY_BYTES)
        );
        assert!(
            light
                .sky_light_arrays
                .iter()
                .flat_map(|layer| layer.0)
                .all(|value| value == 0xff)
        );
        assert!(light.block_light_arrays.is_empty());
    }

    #[test]
    fn fixed_light_sends_constant_sky_and_block_light() {
        let light = light_for_mode(WorldLightMode::Fixed(7));

        assert_eq!(light.sky_light_arrays.len(), LIGHT_SECTION_COUNT);
        assert_eq!(light.block_light_arrays.len(), LIGHT_SECTION_COUNT);
        assert!(
            light
                .sky_light_arrays
                .iter()
                .flat_map(|layer| layer.0)
                .all(|value| value == 0x77)
        );
        assert!(
            light
                .block_light_arrays
                .iter()
                .flat_map(|layer| layer.0)
                .all(|value| value == 0x77)
        );
    }

    #[test]
    fn light_layers_serialize_with_minecraft_byte_array_lengths() {
        let light = light_for_mode(WorldLightMode::Static);
        let mut payload = bytes::BytesMut::new();
        let mut writer = qexed_packet::PacketWriter::new(&mut payload);

        light.serialize(&mut writer).unwrap();

        let masks_len = 9 + 1 + 1 + 9;
        let per_layer_len_prefix = 2;
        let sky_arrays_len = 1 + LIGHT_SECTION_COUNT * (per_layer_len_prefix + LIGHT_ARRAY_BYTES);
        let block_arrays_len = 1;
        assert_eq!(payload.len(), masks_len + sky_arrays_len + block_arrays_len);
    }

    #[test]
    fn sky_light_dampens_through_water_column() {
        let mut dampening = vec![0; 16 * super::OVERWORLD_HEIGHT as usize * 16];
        dampening[block_light_dampening_index(0, WORLD_MAX_Y, 0)] = 1;
        let light = sky_light_from_dampening(&dampening, WorldLightAlgorithm::Fast);

        assert_eq!(
            light.sky_light_mask.0[0].count_ones() as usize,
            light.sky_light_arrays.len()
        );
        assert_eq!(
            nibble_at(&light.sky_light_arrays[LIGHT_SECTION_COUNT - 2], 0),
            15
        );
        assert_eq!(
            nibble_at(&light.sky_light_arrays[LIGHT_SECTION_COUNT - 3], 0),
            14
        );
    }

    #[test]
    fn sky_light_spreads_under_solid_block() {
        let y = 80;
        let mut dampening = vec![0; 16 * super::OVERWORLD_HEIGHT as usize * 16];
        dampening[block_light_dampening_index(8, y + 1, 8)] = 15;
        let light = sky_light_from_dampening(&dampening, WorldLightAlgorithm::Fast);
        let layers = layers_by_mask(&light);
        let under_block = light_at(&layers, 8, y, 8);
        let adjacent_open = light_at(&layers, 7, y, 8);

        assert!(under_block > 0);
        assert!(under_block < adjacent_open);
    }

    #[test]
    fn ray_trace_sky_light_can_enter_from_diagonal_opening() {
        let y = 80;
        let mut dampening = vec![0; 16 * super::OVERWORLD_HEIGHT as usize * 16];
        for block_y in y + 1..=WORLD_MAX_Y {
            dampening[block_light_dampening_index(8, block_y, 8)] = 15;
        }
        let fast = sky_light_from_dampening(&dampening, WorldLightAlgorithm::Fast);
        let traced = sky_light_from_dampening(&dampening, WorldLightAlgorithm::RayTrace);
        let fast_layers = layers_by_mask(&fast);
        let traced_layers = layers_by_mask(&traced);

        assert!(light_at(&traced_layers, 8, y, 8) > light_at(&fast_layers, 8, y, 8));
    }

    #[test]
    fn ray_trace_sky_light_uses_neighbouring_chunk_openings() {
        let y = 80;
        let mut center = vec![0; super::CHUNK_DAMPENING_LEN];
        let east = vec![0; super::CHUNK_DAMPENING_LEN];
        let mut blocked_east = vec![0; super::CHUNK_DAMPENING_LEN];
        for block_y in y + 1..=WORLD_MAX_Y {
            for x in 0..16 {
                for z in 0..16 {
                    center[block_light_dampening_index(x, block_y, z)] = 15;
                    blocked_east[block_light_dampening_index(x, block_y, z)] = 15;
                }
            }
        }

        let mut blocked = super::LightDampeningNeighborhood::single(&center);
        blocked.set(1, 0, blocked_east);
        let blocked = sky_light_from_neighbourhood(&blocked, WorldLightAlgorithm::RayTrace);
        let mut neighbourhood = super::LightDampeningNeighborhood::single(&center);
        neighbourhood.set(1, 0, east);
        let traced = sky_light_from_neighbourhood(&neighbourhood, WorldLightAlgorithm::RayTrace);
        let blocked_layers = layers_by_mask(&blocked);
        let traced_layers = layers_by_mask(&traced);

        assert!(light_at(&traced_layers, 15, y, 8) > light_at(&blocked_layers, 15, y, 8));
    }

    #[test]
    fn fast_sky_light_uses_neighbouring_chunk_edges() {
        let y = 80;
        let mut center = vec![0; super::CHUNK_DAMPENING_LEN];
        let east = vec![0; super::CHUNK_DAMPENING_LEN];
        let mut blocked_east = vec![0; super::CHUNK_DAMPENING_LEN];
        for block_y in y + 1..=WORLD_MAX_Y {
            for x in 0..16 {
                for z in 0..16 {
                    center[block_light_dampening_index(x, block_y, z)] = 15;
                    blocked_east[block_light_dampening_index(x, block_y, z)] = 15;
                }
            }
        }

        let mut blocked = super::LightDampeningNeighborhood::single(&center);
        blocked.set(1, 0, blocked_east);
        let blocked = sky_light_from_neighbourhood(&blocked, WorldLightAlgorithm::Fast);
        let mut neighbourhood = super::LightDampeningNeighborhood::single(&center);
        neighbourhood.set(1, 0, east);
        let fast = sky_light_from_neighbourhood(&neighbourhood, WorldLightAlgorithm::Fast);
        let blocked_layers = layers_by_mask(&blocked);
        let fast_layers = layers_by_mask(&fast);

        assert!(
            light_at_or_zero(&fast_layers, 15, y, 8) > light_at_or_zero(&blocked_layers, 15, y, 8)
        );
    }

    #[test]
    fn world_manager_writes_region_chunks() {
        let dir = tempfile::tempdir().unwrap();
        let manager = WorldManager::new(dir.path());
        let chunk = super::region::ChunkData::zlib(b"chunk").unwrap();

        manager
            .write_region_chunk("minecraft:overworld", 0, 0, chunk)
            .unwrap();

        let loaded = manager
            .load_region_chunk("minecraft:overworld", 0, 0)
            .unwrap()
            .unwrap();
        assert_eq!(loaded.decompress().unwrap(), b"chunk");
    }

    #[test]
    fn ending_last_world_session_clears_chunk_light_cache() {
        let manager = WorldManager::new("world");
        let session = manager.begin_session();
        let epoch = manager.cache_epoch();

        manager.remember_chunk_light_dampening(
            "minecraft:overworld",
            0,
            0,
            vec![0; super::CHUNK_DAMPENING_LEN],
            Some(epoch),
        );
        assert_eq!(manager.cached_light_chunk_count(), 1);

        drop(session);

        assert_eq!(manager.cached_light_chunk_count(), 0);
        manager.remember_chunk_light_dampening(
            "minecraft:overworld",
            1,
            0,
            vec![0; super::CHUNK_DAMPENING_LEN],
            Some(epoch),
        );
        assert_eq!(manager.cached_light_chunk_count(), 0);
    }

    fn nibble_at(
        layer: &qexed_protocol::to_client::play::map_chunk::LightArray,
        index: usize,
    ) -> u8 {
        let value = layer.0[index / 2];
        if index % 2 == 0 {
            value & 0x0f
        } else {
            value >> 4
        }
    }

    fn layers_by_mask(
        light: &qexed_protocol::to_client::play::map_chunk::Light,
    ) -> Vec<Option<&qexed_protocol::to_client::play::map_chunk::LightArray>> {
        let mut layers = vec![None; LIGHT_SECTION_COUNT];
        let mask = light.sky_light_mask.0.first().copied().unwrap_or_default();
        let mut array_index = 0;
        for (section_index, slot) in layers.iter_mut().enumerate() {
            if mask & (1_u64 << section_index) != 0 {
                *slot = light.sky_light_arrays.get(array_index);
                array_index += 1;
            }
        }
        layers
    }

    fn light_at(
        layers: &[Option<&qexed_protocol::to_client::play::map_chunk::LightArray>],
        x: usize,
        y: i32,
        z: usize,
    ) -> u8 {
        let section_index = y.div_euclid(super::SECTION_HEIGHT) - super::MIN_LIGHT_SECTION_Y;
        let layer = layers[section_index as usize].expect("expected light layer");
        let local_y = y.rem_euclid(super::SECTION_HEIGHT) as usize;
        let block_index = local_y * 16 * 16 + z * 16 + x;
        nibble_at(layer, block_index)
    }

    fn light_at_or_zero(
        layers: &[Option<&qexed_protocol::to_client::play::map_chunk::LightArray>],
        x: usize,
        y: i32,
        z: usize,
    ) -> u8 {
        let section_index = y.div_euclid(super::SECTION_HEIGHT) - super::MIN_LIGHT_SECTION_Y;
        let Some(layer) = layers[section_index as usize] else {
            return 0;
        };
        let local_y = y.rem_euclid(super::SECTION_HEIGHT) as usize;
        let block_index = local_y * 16 * 16 + z * 16 + x;
        nibble_at(layer, block_index)
    }
}

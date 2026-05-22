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

const OVERWORLD_HEIGHT: i32 = 384;
const SECTION_HEIGHT: i32 = 16;
const LIGHT_SECTION_COUNT: usize = (OVERWORLD_HEIGHT / SECTION_HEIGHT + 2) as usize;
pub(crate) const WORLD_MIN_Y: i32 = -64;
pub(crate) const WORLD_MAX_Y: i32 = WORLD_MIN_Y + OVERWORLD_HEIGHT - 1;
pub(crate) const WORLD_SECTION_COUNT: usize = (OVERWORLD_HEIGHT / SECTION_HEIGHT) as usize;
pub(crate) const WORLD_MIN_SECTION_Y: i32 = WORLD_MIN_Y / SECTION_HEIGHT;
const MIN_LIGHT_SECTION_Y: i32 = WORLD_MIN_SECTION_Y - 1;
const AIR_BLOCK_STATE_ID: i32 = 0;
const PLAINS_BIOME_ID: i32 = 40;

#[derive(Clone, Debug)]
pub struct WorldManager {
    save_path: std::path::PathBuf,
    light_mode: WorldLightMode,
    placed_blocks: std::sync::Arc<std::sync::Mutex<std::collections::HashMap<BlockKey, i32>>>,
    chunk_light_dampening:
        std::sync::Arc<std::sync::Mutex<std::collections::HashMap<ChunkKey, Vec<u8>>>>,
}

impl WorldManager {
    pub fn new(save_path: impl Into<std::path::PathBuf>) -> Self {
        Self::with_light_mode(save_path, WorldLightMode::default())
    }

    pub fn with_light_mode(
        save_path: impl Into<std::path::PathBuf>,
        light_mode: WorldLightMode,
    ) -> Self {
        Self {
            save_path: save_path.into(),
            light_mode,
            placed_blocks: Default::default(),
            chunk_light_dampening: Default::default(),
        }
    }

    pub fn save_path(&self) -> &std::path::Path {
        &self.save_path
    }

    pub fn network_chunk(&self, dimension: &str, chunk_x: i32, chunk_z: i32) -> Result<MapChunk> {
        if let Some(chunk) = self.load_region_chunk(dimension, chunk_x, chunk_z)? {
            match chunk_nbt::network_chunk_and_light_dampening_from_region(chunk_x, chunk_z, &chunk)
            {
                Ok((mut packet, dampening)) => {
                    self.remember_chunk_light_dampening(dimension, chunk_x, chunk_z, dampening);
                    if let WorldLightMode::Fixed(_) = self.light_mode {
                        packet.light = self.chunk_light();
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
            vec![0; 16 * WORLD_SECTION_COUNT * 16 * 16],
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
    ) {
        self.chunk_light_dampening
            .lock()
            .expect("world light cache poisoned")
            .insert(ChunkKey::new(dimension, chunk_x, chunk_z), dampening);
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
            .or_insert_with(|| vec![0; 16 * WORLD_SECTION_COUNT * 16 * 16]);
        light_dampening[block_light_dampening_index(local_x, position.y, local_z)] =
            dampening.min(15);
    }

    fn chunk_light_for_update(&self, dimension: &str, chunk_x: i32, chunk_z: i32) -> Light {
        match self.light_mode {
            WorldLightMode::Static | WorldLightMode::Fixed(_) => self.chunk_light(),
            WorldLightMode::Dynamic => self
                .chunk_light_dampening
                .lock()
                .expect("world light cache poisoned")
                .get(&ChunkKey::new(dimension, chunk_x, chunk_z))
                .map(|dampening| sky_light_from_dampening(dampening))
                .unwrap_or_else(|| self.chunk_light()),
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

pub(crate) fn sky_light_from_dampening(block_dampening: &[u8]) -> Light {
    if block_dampening.len() != 16 * OVERWORLD_HEIGHT as usize * 16 {
        return light_for_mode(WorldLightMode::Static);
    }

    let mut sky_layers = vec![LightArray::default(); LIGHT_SECTION_COUNT];
    for x in 0..16 {
        for z in 0..16 {
            let mut light = 15_u8;
            for y in (WORLD_MIN_Y..=WORLD_MAX_Y).rev() {
                let section_index = light_section_index(y.div_euclid(SECTION_HEIGHT));
                if let Some(section_index) = section_index {
                    set_light_value(&mut sky_layers[section_index], x, y, z, light);
                }
                light = light
                    .saturating_sub(block_dampening[block_light_dampening_index(x, y, z)].min(15));
            }
            set_light_column(&mut sky_layers[LIGHT_SECTION_COUNT - 1], x, z, 15);
        }
    }

    light_from_layers(sky_layers, vec![LightArray::default(); LIGHT_SECTION_COUNT])
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

fn light_from_layers(sky_layers: Vec<LightArray>, block_layers: Vec<LightArray>) -> Light {
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

fn light_section_index(section_y: i32) -> Option<usize> {
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
        LIGHT_SECTION_COUNT, WORLD_MAX_Y, WorldLightMode, WorldManager,
        block_light_dampening_index, empty_chunk_section_bytes, light_for_mode, section_count,
        sky_light_from_dampening,
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
        let light = sky_light_from_dampening(&dampening);

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
}

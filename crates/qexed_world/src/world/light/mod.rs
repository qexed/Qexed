//! 世界光照：v4 `world/light.rs` 的 v6 迁移。
//!
//! 协议适配（26.3）：v4 把光照挂在 `map_chunk::MapChunk.light`；v6 区块包是
//! `level_chunk_with_light::LevelChunkWithLight`，其 `light_data` 为
//! `LightUpdatePacketData`——BIT_SET 编码从 long 数组改为
//! `VarInt 长度 + 字节串`（BitSet.toByteArray），光照层为 `Vec<ByteArray>`。
//! 本模块内部保留 v4 的位掩码 + 光照层表示（`Light`），并提供到
//! `light_update::LightUpdateData`（光照更新包）与
//! `LightUpdatePacketData`（区块包内嵌光照）的转换。
//!
//! 错误适配：anyhow → `crate::error::WorldError`。

use std::collections::VecDeque;

use qexed_packet::{
    net_types::{ByteArray, VarInt, Bitset},
    PacketCodec, PacketWriter,
};
use qexed_protocol::to_client::play::{
    level_chunk_with_light::LightUpdatePacketData, light_update::LightArray,
    light_update::LightUpdateData,
};

use super::{
    AIR_BLOCK_STATE_ID, CHUNK_DAMPENING_LEN, LIGHT_SECTION_COUNT, MIN_LIGHT_SECTION_Y,
    OVERWORLD_HEIGHT, PLAINS_BIOME_ID, SECTION_HEIGHT, WORLD_MAX_Y, WORLD_MIN_Y,
};

/// 每层光照数组字节数（16*16*16 半字节）。
pub const LIGHT_ARRAY_BYTES: usize = 2048;

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

impl From<&crate::config::LightAlgorithm> for WorldLightAlgorithm {
    fn from(value: &crate::config::LightAlgorithm) -> Self {
        match value {
            crate::config::LightAlgorithm::Fast => Self::Fast,
            crate::config::LightAlgorithm::RayTrace => Self::RayTrace,
        }
    }
}

/// 内部光照表示：v4 `map_chunk::Light` 的等价物（26.3 无独立类型）。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Light {
    pub sky_light_mask: Bitset,
    pub block_light_mask: Bitset,
    pub empty_sky_light_mask: Bitset,
    pub empty_block_light_mask: Bitset,
    pub sky_light_arrays: Vec<LightArray>,
    pub block_light_arrays: Vec<LightArray>,
}

impl From<Light> for LightUpdateData {
    fn from(light: Light) -> Self {
        Self {
            sky_light_mask: light.sky_light_mask,
            block_light_mask: light.block_light_mask,
            empty_sky_light_mask: light.empty_sky_light_mask,
            empty_block_light_mask: light.empty_block_light_mask,
            sky_light_arrays: light.sky_light_arrays,
            block_light_arrays: light.block_light_arrays,
        }
    }
}

impl From<Light> for LightUpdatePacketData {
    fn from(light: Light) -> Self {
        Self {
            sky_y_mask: bitset_to_byte_array(&light.sky_light_mask),
            block_y_mask: bitset_to_byte_array(&light.block_light_mask),
            empty_sky_y_mask: bitset_to_byte_array(&light.empty_sky_light_mask),
            empty_block_y_mask: bitset_to_byte_array(&light.empty_block_light_mask),
            sky_updates: light
                .sky_light_arrays
                .into_iter()
                .map(|layer| ByteArray(layer.0))
                .collect(),
            block_updates: light
                .block_light_arrays
                .into_iter()
                .map(|layer| ByteArray(layer.0))
                .collect(),
        }
    }
}

/// Java BitSet.toByteArray()：每字节 8 位，第 (8i+j) 位落在第 i 字节的第 j 位。
/// 最高置位字节之后的内容省略；空集为空串。
fn bitset_to_byte_array(bitset: &Bitset) -> ByteArray {
    let mut words = bitset.0.clone();
    while words.last().is_some_and(|word| *word == 0) {
        words.pop();
    }
    let Some((&last, _)) = words.split_last() else {
        return ByteArray(Vec::new());
    };
    // Java BitSet.toByteArray：只写到最高置位字节（非整个 long 字）。
    let highest_bit = words.len() * 64 - last.leading_zeros() as usize;
    let mut bytes = vec![0_u8; highest_bit.div_ceil(8)];
    for (word_index, word) in words.iter().enumerate() {
        for bit in 0..64 {
            if word & (1_u64 << bit) != 0 {
                let absolute = word_index * 64 + bit;
                bytes[absolute / 8] |= 1 << (absolute % 8);
            }
        }
    }
    ByteArray(bytes)
}

#[derive(Clone, Debug)]
pub(crate) struct LightDampeningNeighborhood {
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
    pub(crate) fn single(center: &[u8]) -> Self {
        let mut neighbourhood = Self::default();
        neighbourhood.set(0, 0, center.to_vec());
        neighbourhood
    }

    pub(crate) fn set(&mut self, offset_x: i32, offset_z: i32, dampening: Vec<u8>) {
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

    pub(crate) fn chunks(&self) -> &[Option<Vec<u8>>; 9] {
        &self.chunks
    }

    fn center(&self) -> Option<&[u8]> {
        self.chunks[Self::chunk_index(0, 0).expect("center chunk index")]
            .as_ref()
            .map(Vec::as_slice)
    }

    pub(crate) fn chunk_index(offset_x: i32, offset_z: i32) -> Option<usize> {
        ((-1..=1).contains(&offset_x) && (-1..=1).contains(&offset_z))
            .then_some(((offset_z + 1) * 3 + (offset_x + 1)) as usize)
    }
}

impl Default for WorldLightMode {
    fn default() -> Self {
        Self::Static
    }
}

impl From<&crate::config::LightMode> for WorldLightMode {
    fn from(value: &crate::config::LightMode) -> Self {
        match value {
            crate::config::LightMode::Static => Self::Static,
            crate::config::LightMode::Dynamic => Self::Dynamic,
            crate::config::LightMode::Fixed(level) => Self::Fixed(*level),
        }
    }
}

pub(crate) fn empty_chunk_packet(
    chunk_x: i32,
    chunk_z: i32,
    light_mode: WorldLightMode,
) -> qexed_protocol::to_client::play::level_chunk_with_light::LevelChunkWithLight {
    use qexed_protocol::to_client::play::level_chunk_with_light::{
        Heightmap, LevelChunkPacketData,
    };

    qexed_protocol::to_client::play::level_chunk_with_light::LevelChunkWithLight {
        x: chunk_x,
        z: chunk_z,
        chunk_data: LevelChunkPacketData {
            heightmaps: vec![
                Heightmap {
                    kind: VarInt(1),
                    data: vec![0; 37],
                },
                Heightmap {
                    kind: VarInt(4),
                    data: vec![0; 37],
                },
                Heightmap {
                    kind: VarInt(5),
                    data: vec![0; 37],
                },
            ],
            buffer: ByteArray(empty_chunk_section_bytes()),
            block_entities: Vec::new(),
        },
        light_data: light_for_mode(light_mode).into(),
    }
}

pub(crate) fn empty_heightmaps(
) -> Vec<qexed_protocol::to_client::play::level_chunk_with_light::Heightmap> {
    use qexed_protocol::to_client::play::level_chunk_with_light::Heightmap;
    vec![
        Heightmap {
            kind: VarInt(1),
            data: vec![0; 37],
        },
        Heightmap {
            kind: VarInt(4),
            data: vec![0; 37],
        },
        Heightmap {
            kind: VarInt(5),
            data: vec![0; 37],
        },
    ]
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

pub(crate) fn sky_light_from_neighbourhood(
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

/// 空光照层（v4 LightArray 为 [u8; 2048] 定长数组，Default 即全零；v6 的
/// LightArray(Vec<u8>) 派生 Default 是空 Vec，必须显式分配 2048 字节）。
pub(crate) fn empty_light_layer() -> LightArray {
    LightArray(vec![0; LIGHT_ARRAY_BYTES])
}

pub(crate) fn light_from_sky_values(sky_values: &[u8]) -> Light {
    let mut sky_layers = vec![empty_light_layer(); LIGHT_SECTION_COUNT];
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
    light_from_layers(sky_layers, vec![empty_light_layer(); LIGHT_SECTION_COUNT])
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

pub(crate) fn light_update_data(light: Light) -> LightUpdateData {
    light.into()
}

pub(crate) fn replace_sky_light(target: &mut Light, source: Light) {
    target.sky_light_mask = source.sky_light_mask;
    target.empty_sky_light_mask = source.empty_sky_light_mask;
    target.sky_light_arrays = source.sky_light_arrays;
}

fn filled_light_mask(level: u8) -> Bitset {
    if level == 0 {
        Bitset(Vec::new())
    } else {
        full_light_section_mask()
    }
}

fn empty_filled_light_mask(level: u8) -> Bitset {
    if level == 0 {
        full_light_section_mask()
    } else {
        Bitset(Vec::new())
    }
}

fn full_light_section_mask() -> Bitset {
    Bitset(vec![(1_u64 << LIGHT_SECTION_COUNT) - 1])
}

fn filled_light_arrays(level: u8) -> Vec<LightArray> {
    if level == 0 {
        Vec::new()
    } else {
        let packed = (level.min(15) & 0x0f) | ((level.min(15) & 0x0f) << 4);
        vec![LightArray(vec![packed; LIGHT_ARRAY_BYTES]); LIGHT_SECTION_COUNT]
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

fn split_light_layers(layers: Vec<LightArray>) -> (Bitset, Bitset, Vec<LightArray>) {
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
        Bitset(non_empty_bitset_words(light_mask)),
        Bitset(non_empty_bitset_words(empty_mask)),
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

pub(crate) fn empty_chunk_section_bytes() -> Vec<u8> {
    let mut bytes = bytes::BytesMut::new();
    let mut writer = PacketWriter::new(&mut bytes);

    for _ in 0..section_count() {
        write_empty_section(&mut writer).expect("empty section packet data is infallible");
    }

    bytes.to_vec()
}

pub(crate) fn write_empty_section(writer: &mut PacketWriter) -> crate::error::Result<()> {
    0_i16.serialize(writer)?;
    0_i16.serialize(writer)?;
    write_single_value_palette(writer, AIR_BLOCK_STATE_ID)?;
    write_single_value_palette(writer, PLAINS_BIOME_ID)?;
    Ok(())
}

pub(crate) fn write_single_value_palette(
    writer: &mut PacketWriter,
    registry_id: i32,
) -> crate::error::Result<()> {
    0_u8.serialize(writer)?;
    VarInt(registry_id).serialize(writer)?;
    write_fixed_long_array(writer, &[])
}

pub(crate) fn write_fixed_long_array(
    writer: &mut PacketWriter,
    values: &[u64],
) -> crate::error::Result<()> {
    for value in values {
        value.serialize(writer)?;
    }
    Ok(())
}

pub(crate) fn section_count() -> i32 {
    OVERWORLD_HEIGHT / SECTION_HEIGHT
}

#[cfg(test)]
mod tests;

//! v4 `world/chunk_nbt.rs` 的 v6 迁移：区块 NBT ↔ 网络区块包的转换。
//!
//! 协议适配（26.3）：v4 产出 `map_chunk::MapChunk`；v6 区块包是
//! `level_chunk_with_light::LevelChunkWithLight`：
//! - 字段 chunk_x/chunk_z → x/z；`Chunk{heightmaps,data,block_entities}` →
//!   `LevelChunkPacketData{heightmaps: Vec<Heightmap>, buffer: ByteArray, block_entities}`；
//! - `Heightmaps{type_id,data}` → `Heightmap{kind,data}`；
//! - `BlockEntities{xz: u8, y: u16}` → `BlockEntityInfo{packed_xz: i8, y: i16}`；
//! - `light: Light` → `light_data`（`Light` 内部表示 → `LightUpdatePacketData`，
//!   BIT_SET 由 long 数组改字节串）。
//!
//! 注册表复用：`registry`（v4 chunk_nbt/registry.rs 完整版）从
//! `qexed_mojang_data::registry_sync` 加载；generator 的 nbt_layers.rs /
//! noise_settings.rs 经 generator 里的 `block_registry` 别名使用同一份（world-features
//! 任务期间的子集版已并入本模块的 registry）。
//!
//! 错误适配：anyhow → `crate::error::WorldError`。

use std::{collections::HashMap, sync::Arc};

use bytes::BytesMut;
use qexed_nbt::{ListHeader, Tag, tag_id};
use qexed_packet::{
    net_types::Position,
    PacketCodec, PacketWriter,
    net_types::{OptionalNbt, VarInt},
};
use qexed_protocol::to_client::play::level_chunk_with_light::{
    BlockEntityInfo, Heightmap, LevelChunkPacketData, LevelChunkWithLight,
};

use super::{
    Light, WorldLightAlgorithm, LIGHT_ARRAY_BYTES, block_light_dampening_index,
    light_from_layers, light_section_index, section_count, sky_light_from_dampening,
    write_empty_section, write_fixed_long_array,
};
use crate::error::WorldError;
use crate::world::region::ChunkData;

mod nbt;
pub(crate) mod registry;

use nbt::{
    byte_array, compound, int_field, list_items, long_array, string_field, string_properties,
};
#[allow(unused_imports)]
pub(crate) use registry::{
    BlockStateDefinition, block_state, default_block_state, default_block_state_id,
    default_block_state_id_if_known,
};
#[allow(unused_imports)]
use registry::block_state_entry;
use registry::{
    biome_registry, block_entity_type_id, block_state_registry, has_fluid, is_air_block,
    light_dampening, normalize_identifier, state_key,
};

const MIN_SECTION_Y: i32 = -4;
const BLOCK_ENTRY_COUNT: usize = 16 * 16 * 16;
const BIOME_ENTRY_COUNT: usize = 4 * 4 * 4;
const AIR_BLOCK_STATE_ID: i32 = 0;
const PLAINS_BIOME_ID: i32 = 40;
const DATA_VERSION: i32 = 4790;

/// 测试/内部用：从 region 区块构造网络区块包（默认光照算法）。
#[cfg(test)]
pub(crate) fn network_chunk_from_region_pub(
    chunk_x: i32,
    chunk_z: i32,
    chunk: &ChunkData,
) -> crate::error::Result<LevelChunkWithLight> {
    network_chunk_from_region(chunk_x, chunk_z, chunk)
}

#[cfg(test)]
fn network_chunk_from_region(
    chunk_x: i32,
    chunk_z: i32,
    chunk: &ChunkData,
) -> crate::error::Result<LevelChunkWithLight> {
    Ok(network_chunk_and_light_dampening_from_region(
        chunk_x,
        chunk_z,
        chunk,
        WorldLightAlgorithm::default(),
    )?
    .0)
}

pub fn network_chunk_and_light_dampening_from_region(
    chunk_x: i32,
    chunk_z: i32,
    chunk: &ChunkData,
    light_algorithm: WorldLightAlgorithm,
) -> crate::error::Result<(LevelChunkWithLight, Vec<u8>)> {
    let raw = chunk.decompress()?;
    let (_, root) = qexed_nbt::from_slice(&raw)?;
    network_chunk_and_light_dampening_from_nbt(chunk_x, chunk_z, &root, light_algorithm)
}

pub fn light_dampening_from_region(chunk: &ChunkData) -> crate::error::Result<Vec<u8>> {
    let raw = chunk.decompress()?;
    let (_, root) = qexed_nbt::from_slice(&raw)?;
    light_dampening_from_nbt(&root)
}

pub fn block_state_at_from_region(
    chunk: &ChunkData,
    position: &Position,
) -> crate::error::Result<Option<i32>> {
    let raw = chunk.decompress()?;
    let (_, root) = qexed_nbt::from_slice(&raw)?;
    block_state_at_from_nbt(&root, position)
}

pub fn set_block_state_in_region(
    chunk_x: i32,
    chunk_z: i32,
    existing: Option<&ChunkData>,
    position: &Position,
    block_state: i32,
    fallback_block_state: Option<i32>,
) -> crate::error::Result<ChunkData> {
    let root = if let Some(existing) = existing {
        let raw = existing.decompress()?;
        let (_, root) = qexed_nbt::from_slice(&raw)?;
        root
    } else {
        minimal_chunk_root(chunk_x, chunk_z)
    };

    let updated = set_block_state_in_nbt(
        &root,
        chunk_x,
        chunk_z,
        position,
        block_state,
        fallback_block_state,
    )?;
    let raw = qexed_nbt::to_vec("", &updated)?;
    ChunkData::zlib(&raw)
}

pub fn set_block_states_in_region(
    chunk_x: i32,
    chunk_z: i32,
    existing: Option<&ChunkData>,
    blocks: &[(Position, i32, Option<i32>)],
) -> crate::error::Result<ChunkData> {
    let mut root = if let Some(existing) = existing {
        let raw = existing.decompress()?;
        let (_, root) = qexed_nbt::from_slice(&raw)?;
        root
    } else {
        minimal_chunk_root(chunk_x, chunk_z)
    };

    for (position, block_state, fallback_block_state) in blocks {
        root = set_block_state_in_nbt(
            &root,
            chunk_x,
            chunk_z,
            position,
            *block_state,
            *fallback_block_state,
        )?;
    }

    let raw = qexed_nbt::to_vec("", &root)?;
    ChunkData::zlib(&raw)
}

pub fn region_chunk_from_nbt(
    chunk_x: i32,
    chunk_z: i32,
    root: &Tag,
) -> crate::error::Result<ChunkData> {
    let root = normalized_chunk_root(root, chunk_x, chunk_z)?;
    let raw = qexed_nbt::to_vec("", &root)?;
    ChunkData::zlib(&raw)
}

#[cfg(test)]
fn network_chunk_from_nbt(
    chunk_x: i32,
    chunk_z: i32,
    root: &Tag,
) -> crate::error::Result<LevelChunkWithLight> {
    Ok(network_chunk_and_light_dampening_from_nbt(
        chunk_x,
        chunk_z,
        root,
        WorldLightAlgorithm::default(),
    )?
    .0)
}

pub fn network_chunk_and_light_dampening_from_nbt(
    chunk_x: i32,
    chunk_z: i32,
    root: &Tag,
    light_algorithm: WorldLightAlgorithm,
) -> crate::error::Result<(LevelChunkWithLight, Vec<u8>)> {
    let root = compound(root)
        .ok_or_else(|| crate::error::WorldError::msg("chunk root is not a compound"))?;
    let sections = sections_by_y(root);
    let section_data = chunk_section_bytes(&sections)?;
    let block_dampening = chunk_light_dampening(&sections)?;
    let light: Light = saved_light(&sections)
        .unwrap_or_else(|| sky_light_from_dampening(&block_dampening, light_algorithm));
    let packet = LevelChunkWithLight {
        x: chunk_x,
        z: chunk_z,
        chunk_data: LevelChunkPacketData {
            heightmaps: heightmaps(root),
            buffer: qexed_packet::net_types::ByteArray(section_data),
            block_entities: block_entities(root, chunk_x, chunk_z),
        },
        light_data: light.into(),
    };
    Ok((packet, block_dampening))
}

pub fn light_dampening_from_nbt(root: &Tag) -> crate::error::Result<Vec<u8>> {
    let root = compound(root)
        .ok_or_else(|| crate::error::WorldError::msg("chunk root is not a compound"))?;
    let sections = sections_by_y(root);
    chunk_light_dampening(&sections)
}

pub fn block_state_at_from_nbt(
    root: &Tag,
    position: &Position,
) -> crate::error::Result<Option<i32>> {
    let root = compound(root)
        .ok_or_else(|| crate::error::WorldError::msg("chunk root is not a compound"))?;
    let section_y = position.y.div_euclid(16);
    let Some(section) = sections_by_y(root).get(&section_y).copied() else {
        return Ok(None);
    };
    let values = block_values(section.get("block_states"))?;
    let index = block_index(position);
    let block = values
        .blocks
        .get(index)
        .copied()
        .ok_or_else(|| crate::error::WorldError::msg("block index out of section bounds"))?;
    let block_state = values
        .global_ids
        .get(index)
        .copied()
        .ok_or_else(|| crate::error::WorldError::msg("block index out of section bounds"))?;
    Ok((!block.is_air).then_some(block_state))
}

pub fn set_block_state_in_nbt(
    root: &Tag,
    chunk_x: i32,
    chunk_z: i32,
    position: &Position,
    block_state: i32,
    fallback_block_state: Option<i32>,
) -> crate::error::Result<Tag> {
    let mut root = compound(root)
        .ok_or_else(|| crate::error::WorldError::msg("chunk root is not a compound"))?
        .clone();
    root.insert("DataVersion".to_string(), Tag::Int(DATA_VERSION));
    root.insert("xPos".to_string(), Tag::Int(chunk_x));
    root.insert("yPos".to_string(), Tag::Int(MIN_SECTION_Y));
    root.insert("zPos".to_string(), Tag::Int(chunk_z));

    let section_y = position.y.div_euclid(16);
    let mut sections = root
        .get("sections")
        .and_then(|tag| list_items(Some(tag)))
        .map(|items| items.to_vec())
        .unwrap_or_default();
    let section_index = sections.iter().position(|section| {
        compound(section).and_then(|fields| int_field(fields, "Y")) == Some(section_y)
    });

    let mut section = match section_index {
        Some(index) => compound(&sections[index])
            .ok_or_else(|| {
                crate::error::WorldError::msg(format!(
                    "section y={section_y} is not a compound"
                ))
            })?
            .clone(),
        None => empty_section(
            section_y,
            fallback_block_state.unwrap_or(AIR_BLOCK_STATE_ID),
        ),
    };

    let mut values = block_values(section.get("block_states"))?.global_ids;
    values[block_index(position)] = block_state;
    section.insert("block_states".to_string(), block_states_tag(&values)?);
    section
        .entry("biomes".to_string())
        .or_insert_with(|| biome_states_tag("minecraft:plains"));

    let section_tag = Tag::Compound(Arc::new(section));
    if let Some(index) = section_index {
        sections[index] = section_tag;
    } else {
        sections.push(section_tag);
        sections.sort_by_key(|section| {
            compound(section)
                .and_then(|fields| int_field(fields, "Y"))
                .unwrap_or(i32::MAX)
        });
    }
    root.insert("sections".to_string(), list_tag(tag_id::COMPOUND, sections));
    remove_block_entity_at(&mut root, position);

    normalized_chunk_root(&Tag::Compound(Arc::new(root)), chunk_x, chunk_z)
}

fn chunk_section_bytes(
    sections: &HashMap<i32, &HashMap<String, Tag>>,
) -> crate::error::Result<Vec<u8>> {
    let mut bytes = BytesMut::new();
    let mut writer = PacketWriter::new(&mut bytes);

    for section_y in MIN_SECTION_Y..MIN_SECTION_Y + section_count() {
        if let Some(section) = sections.get(&section_y) {
            write_section(&mut writer, section).map_err(|err| {
                crate::error::WorldError::msg(format!(
                    "serialize section y={section_y}: {err}"
                ))
            })?;
        } else {
            write_empty_section(&mut writer)?;
        }
    }

    Ok(bytes.to_vec())
}

fn chunk_light_dampening(
    sections: &HashMap<i32, &HashMap<String, Tag>>,
) -> crate::error::Result<Vec<u8>> {
    let mut dampening = vec![0; 16 * super::WORLD_SECTION_COUNT * 16 * 16];

    for section_y in MIN_SECTION_Y..MIN_SECTION_Y + section_count() {
        let Some(section) = sections.get(&section_y) else {
            continue;
        };
        let values = block_values(section.get("block_states"))?;
        for (index, block) in values.blocks.iter().enumerate() {
            if block.light_dampening == 0 {
                continue;
            }

            let local_y = index / (16 * 16);
            let z = (index / 16) % 16;
            let x = index % 16;
            let world_y = section_y * 16 + local_y as i32;
            let dampening_index = block_light_dampening_index(x, world_y, z);
            dampening[dampening_index] = block.light_dampening;
        }
    }

    Ok(dampening)
}

fn saved_light(sections: &HashMap<i32, &HashMap<String, Tag>>) -> Option<Light> {

    let mut sky_layers =
        vec![super::light::empty_light_layer(); super::LIGHT_SECTION_COUNT];
    let mut block_layers =
        vec![super::light::empty_light_layer(); super::LIGHT_SECTION_COUNT];
    let mut found_sky = false;
    let mut found_block = false;

    for (&section_y, section) in sections {
        let Some(section_index) = light_section_index(section_y) else {
            continue;
        };

        if let Some(layer) = light_array(section.get("SkyLight")) {
            sky_layers[section_index] = layer;
            found_sky = true;
        }
        if let Some(layer) = light_array(section.get("BlockLight")) {
            block_layers[section_index] = layer;
            found_block = true;
        }
    }

    (found_sky || found_block).then(|| light_from_layers(sky_layers, block_layers))
}

fn light_array(
    tag: Option<&Tag>,
) -> Option<qexed_protocol::to_client::play::light_update::LightArray> {
    let bytes = byte_array(tag?)?;
    if bytes.len() != LIGHT_ARRAY_BYTES {
        log::warn!(
            "{}",
            qexed_language::t("qexed.world.chunk_nbt.invalid_light_layer")
                .replace("%{got}", &bytes.len().to_string())
                .replace("%{expected}", &LIGHT_ARRAY_BYTES.to_string())
        );
        return None;
    }

    let mut layer =
        qexed_protocol::to_client::play::light_update::LightArray(vec![0; LIGHT_ARRAY_BYTES]);
    for (target, source) in layer.0.iter_mut().zip(bytes.iter()) {
        *target = *source as u8;
    }
    Some(layer)
}

fn write_section(
    writer: &mut PacketWriter,
    section: &HashMap<String, Tag>,
) -> crate::error::Result<()> {
    let blocks = block_values(section.get("block_states"))?;
    let biomes = biome_values(section.get("biomes"))?;

    (blocks.non_empty_count as i16)
        .serialize(writer)
        .map_err(WorldError::Packet)?;
    (blocks.fluid_count as i16)
        .serialize(writer)
        .map_err(WorldError::Packet)?;
    write_paletted_container(writer, &blocks.global_ids, PaletteKind::Block)?;
    write_paletted_container(writer, &biomes, PaletteKind::Biome)?;
    Ok(())
}

fn block_values(tag: Option<&Tag>) -> crate::error::Result<BlockValues> {
    let Some(container) = tag.and_then(compound) else {
        return Ok(BlockValues::empty());
    };

    let palette = block_palette(container);
    let indices = unpack_indices(
        container.get("data"),
        palette.len(),
        BLOCK_ENTRY_COUNT,
        PaletteKind::Block,
    )?;

    let mut global_ids = Vec::with_capacity(BLOCK_ENTRY_COUNT);
    let mut blocks = Vec::with_capacity(BLOCK_ENTRY_COUNT);
    let mut non_empty_count = 0usize;
    let mut fluid_count = 0usize;
    for index in indices {
        let entry = palette.get(index).ok_or_else(|| {
            crate::error::WorldError::msg(format!(
                "block palette index {index} out of range for palette size {}",
                palette.len()
            ))
        })?;
        if !entry.is_air {
            non_empty_count += 1;
        }
        if entry.has_fluid {
            fluid_count += 1;
        }
        global_ids.push(entry.id);
        blocks.push(*entry);
    }

    Ok(BlockValues {
        global_ids,
        blocks,
        non_empty_count,
        fluid_count,
    })
}

fn biome_values(tag: Option<&Tag>) -> crate::error::Result<Vec<i32>> {
    let Some(container) = tag.and_then(compound) else {
        return Ok(vec![PLAINS_BIOME_ID; BIOME_ENTRY_COUNT]);
    };

    let palette = biome_palette(container);
    let indices = unpack_indices(
        container.get("data"),
        palette.len(),
        BIOME_ENTRY_COUNT,
        PaletteKind::Biome,
    )?;

    indices
        .into_iter()
        .map(|index| {
            palette.get(index).copied().ok_or_else(|| {
                crate::error::WorldError::msg(format!(
                    "biome palette index {index} out of range for palette size {}",
                    palette.len()
                ))
            })
        })
        .collect()
}

fn block_palette(container: &HashMap<String, Tag>) -> Vec<BlockPaletteEntry> {
    let Some(entries) = list_items(container.get("palette")) else {
        return vec![BlockPaletteEntry::air()];
    };

    let palette = entries
        .iter()
        .filter_map(|entry| match entry {
            Tag::Compound(compound) => Some(block_palette_entry(compound)),
            _ => None,
        })
        .collect::<Vec<_>>();

    if palette.is_empty() {
        vec![BlockPaletteEntry::air()]
    } else {
        palette
    }
}

fn block_palette_entry(entry: &HashMap<String, Tag>) -> BlockPaletteEntry {
    let name = string_field(entry, "Name").unwrap_or("minecraft:air");
    let properties = string_properties(entry.get("Properties"));
    let key = state_key(name, &properties);
    let registry = block_state_registry();
    let id = registry
        .id_by_state
        .get(&key)
        .copied()
        .or_else(|| {
            if properties.is_empty() {
                registry
                    .default_state_by_name
                    .get(&normalize_identifier(name))
                    .map(|state| state.id)
            } else {
                None
            }
        })
        .unwrap_or_else(|| {
            log::warn!(
                "{}",
                qexed_language::t("qexed.world.chunk_nbt.unknown_block_state")
                    .replace("%{key}", &key)
            );
            AIR_BLOCK_STATE_ID
        });
    let block_type = registry
        .metadata_by_name
        .get(name)
        .map(|metadata| metadata.block_type.as_str());

    let has_fluid = has_fluid(name, &properties);
    BlockPaletteEntry {
        id,
        is_air: id == AIR_BLOCK_STATE_ID || is_air_block(name),
        has_fluid,
        light_dampening: light_dampening(name, block_type, has_fluid),
    }
}

fn biome_palette(container: &HashMap<String, Tag>) -> Vec<i32> {
    let Some(entries) = list_items(container.get("palette")) else {
        return vec![PLAINS_BIOME_ID];
    };

    let palette = entries
        .iter()
        .filter_map(|entry| match entry {
            Tag::String(name) => Some(
                biome_registry()
                    .id_by_name
                    .get(normalize_identifier(name).as_str())
                    .copied()
                    .unwrap_or_else(|| {
                        log::warn!(
                            "{}",
                            qexed_language::t("qexed.world.chunk_nbt.unknown_biome")
                                .replace("%{biome}", name)
                        );
                        PLAINS_BIOME_ID
                    }),
            ),
            _ => None,
        })
        .collect::<Vec<_>>();

    if palette.is_empty() {
        vec![PLAINS_BIOME_ID]
    } else {
        palette
    }
}

fn unpack_indices(
    data_tag: Option<&Tag>,
    palette_len: usize,
    entry_count: usize,
    kind: PaletteKind,
) -> crate::error::Result<Vec<usize>> {
    if palette_len == 0 {
        return Ok(vec![0; entry_count]);
    }

    let bits = storage_bits(kind, palette_len);
    if bits == 0 {
        return Ok(vec![0; entry_count]);
    }

    let data = data_tag.and_then(long_array).ok_or_else(|| {
        crate::error::WorldError::msg(format!(
            "missing paletted container data for palette size {palette_len}"
        ))
    })?;
    let values_per_long = 64 / bits;
    let required_len = entry_count.div_ceil(values_per_long);
    if data.len() != required_len {
        return Err(crate::error::WorldError::msg(format!(
            "invalid paletted container data length: got {}, expected {}",
            data.len(),
            required_len
        )));
    }

    let mask = (1_u64 << bits) - 1;
    let mut values = Vec::with_capacity(entry_count);
    for index in 0..entry_count {
        let cell = index / values_per_long;
        let bit_offset = (index % values_per_long) * bits;
        let value = ((data[cell] as u64) >> bit_offset) & mask;
        let value = usize::try_from(value)
            .map_err(|_| crate::error::WorldError::msg("palette index does not fit usize"))?;
        if value >= palette_len {
            return Err(crate::error::WorldError::msg(format!(
                "palette index {value} out of range for palette size {palette_len}"
            )));
        }
        values.push(value);
    }

    Ok(values)
}

fn write_paletted_container(
    writer: &mut PacketWriter,
    global_ids: &[i32],
    kind: PaletteKind,
) -> crate::error::Result<()> {
    let (palette, local_values) = local_palette(global_ids);
    let (bits, global_palette) = network_config(kind, palette.len());
    (bits as u8)
        .serialize(writer)
        .map_err(|err| WorldError::Packet(err))?;

    if bits == 0 {
        VarInt(palette.first().copied().unwrap_or(default_id(kind)))
            .serialize(writer)
            .map_err(WorldError::Packet)?;
        return write_fixed_long_array(writer, &[]);
    }

    let packed_values = if global_palette {
        pack_values(global_ids, bits).map_err(WorldError::Packet)?
    } else {
        VarInt(palette.len() as i32)
            .serialize(writer)
            .map_err(WorldError::Packet)?;
        for id in &palette {
            VarInt(*id).serialize(writer).map_err(WorldError::Packet)?;
        }
        let local_values = local_values
            .into_iter()
            .map(|value| {
                i32::try_from(value).map_err(|_| {
                    WorldError::msg("local palette index does not fit i32")
                })
            })
            .collect::<crate::error::Result<Vec<_>>>()?;
        pack_values(&local_values, bits).map_err(WorldError::Packet)?
    };

    write_fixed_long_array(writer, &packed_values)
}

fn block_states_tag(global_ids: &[i32]) -> crate::error::Result<Tag> {
    if global_ids.len() != BLOCK_ENTRY_COUNT {
        return Err(crate::error::WorldError::msg(format!(
            "invalid block state count: got {}, expected {}",
            global_ids.len(),
            BLOCK_ENTRY_COUNT
        )));
    }

    let (palette, local_values) = local_palette(global_ids);
    let palette_tags = palette
        .iter()
        .map(|id| block_state_tag(*id))
        .collect::<Vec<_>>();
    let data = (palette.len() > 1)
        .then(|| {
            let bits = storage_bits(PaletteKind::Block, palette.len());
            let local_values = local_values
                .into_iter()
                .map(|value| {
                    i32::try_from(value).map_err(|_| {
                        crate::error::WorldError::msg("palette index does not fit i32")
                    })
                })
                .collect::<crate::error::Result<Vec<_>>>()?;
            pack_values(&local_values, bits).map_err(WorldError::Packet)
        })
        .transpose()?
        .map(|values| values.into_iter().map(|value| value as i64).collect());
    Ok(paletted_container_tag(palette_tags, data))
}

fn block_state_tag(block_state: i32) -> Tag {
    let state = block_state_entry(block_state);
    let mut fields = HashMap::new();
    fields.insert("Name".to_string(), Tag::String(Arc::from(state.name)));
    if !state.properties.is_empty() {
        fields.insert(
            "Properties".to_string(),
            Tag::Compound(Arc::new(
                state
                    .properties
                    .into_iter()
                    .map(|(name, value)| (name, Tag::String(Arc::from(value))))
                    .collect(),
            )),
        );
    }
    Tag::Compound(Arc::new(fields))
}

fn biome_states_tag(biome: &str) -> Tag {
    paletted_container_tag(vec![Tag::String(Arc::from(biome.to_string()))], None)
}

fn paletted_container_tag(palette: Vec<Tag>, data: Option<Vec<i64>>) -> Tag {
    let mut fields = HashMap::new();
    fields.insert(
        "palette".to_string(),
        Tag::List(
            ListHeader {
                tag_id: palette.first().map(Tag::tag_id).unwrap_or(tag_id::END),
                length: palette.len() as i32,
            },
            Arc::from(palette),
        ),
    );
    if let Some(data) = data {
        fields.insert("data".to_string(), Tag::LongArray(Arc::from(data)));
    }
    Tag::Compound(Arc::new(fields))
}

fn local_palette(values: &[i32]) -> (Vec<i32>, Vec<usize>) {
    let mut palette = Vec::new();
    let mut index_by_value = HashMap::new();
    let mut local_values = Vec::with_capacity(values.len());

    for value in values {
        let next_index = palette.len();
        let index = *index_by_value.entry(*value).or_insert_with(|| {
            palette.push(*value);
            next_index
        });
        local_values.push(index);
    }

    (palette, local_values)
}

fn pack_values(values: &[i32], bits: usize) -> qexed_packet::Result<Vec<u64>> {
    if bits == 0 {
        return Ok(Vec::new());
    }

    let values_per_long = 64 / bits;
    let mut packed = vec![0_u64; values.len().div_ceil(values_per_long)];
    let mask = (1_u64 << bits) - 1;

    for (index, value) in values.iter().enumerate() {
        if *value < 0 {
            return Err(qexed_packet::PacketError::msg(format!(
                "negative paletted value: {value}"
            )));
        }

        let value = *value as u64;
        if value > mask {
            return Err(qexed_packet::PacketError::msg(format!(
                "paletted value {value} exceeds {bits} bits"
            )));
        }

        let cell = index / values_per_long;
        let bit_offset = (index % values_per_long) * bits;
        packed[cell] |= value << bit_offset;
    }

    Ok(packed)
}

fn network_config(kind: PaletteKind, palette_len: usize) -> (usize, bool) {
    let entry_bits = ceil_log2(palette_len);
    match kind {
        PaletteKind::Block => match entry_bits {
            0 => (0, false),
            1..=4 => (4, false),
            5..=8 => (entry_bits, false),
            _ => (block_state_registry().global_bits.max(entry_bits), true),
        },
        PaletteKind::Biome => match entry_bits {
            0 => (0, false),
            1..=3 => (entry_bits, false),
            _ => (biome_registry().global_bits.max(entry_bits), true),
        },
    }
}

fn storage_bits(kind: PaletteKind, palette_len: usize) -> usize {
    let entry_bits = ceil_log2(palette_len);
    match kind {
        PaletteKind::Block => match entry_bits {
            0 => 0,
            1..=4 => 4,
            _ => entry_bits,
        },
        PaletteKind::Biome => entry_bits,
    }
}

fn heightmaps(root: &HashMap<String, Tag>) -> Vec<Heightmap> {
    let Some(heightmaps) = root.get("Heightmaps").and_then(compound) else {
        return super::empty_heightmaps();
    };

    [
        ("WORLD_SURFACE", 1),
        ("MOTION_BLOCKING", 4),
        ("MOTION_BLOCKING_NO_LEAVES", 5),
    ]
    .into_iter()
    .map(|(name, type_id)| Heightmap {
        kind: VarInt(type_id),
        data: heightmaps
            .get(name)
            .and_then(long_array)
            .map(|values| values.to_vec())
            .unwrap_or_else(|| vec![0; 37]),
    })
    .collect()
}

fn block_entities(
    root: &HashMap<String, Tag>,
    chunk_x: i32,
    chunk_z: i32,
) -> Vec<BlockEntityInfo> {
    let chunk_min_x = chunk_x * 16;
    let chunk_min_z = chunk_z * 16;
    let mut entities = root
        .get("block_entities")
        .and_then(|tag| list_items(Some(tag)))
        .into_iter()
        .flatten()
        .filter_map(|item| block_entity(item, chunk_min_x, chunk_min_z))
        .collect::<Vec<_>>();
    entities.sort_by_key(|entity| (entity.y, entity.packed_xz));
    entities
}

fn block_entity(item: &Tag, chunk_min_x: i32, chunk_min_z: i32) -> Option<BlockEntityInfo> {
    let fields = compound(item)?;
    let id = string_field(fields, "id")?;
    let entity_type = block_entity_type_id(id).or_else(|| {
        log::warn!(
            "{}",
            qexed_language::t("qexed.world.chunk_nbt.unknown_block_entity")
                .replace("%{id}", id)
        );
        None
    })?;
    let x = int_field(fields, "x")?;
    let y = int_field(fields, "y")?;
    let z = int_field(fields, "z")?;
    let local_x = local_chunk_coord(x, chunk_min_x)?;
    let local_z = local_chunk_coord(z, chunk_min_z)?;
    Some(BlockEntityInfo {
        // 26.3：packed_xz 为 i8（(x << 4) | z，本地坐标 0-15），y 为 i16。
        packed_xz: (((local_x as i8) << 4) | local_z as i8) as i8,
        y: y as i16,
        block_entity_type: VarInt(entity_type),
        tag: OptionalNbt(block_entity_update_tag(fields)),
    })
}

fn local_chunk_coord(world_coord: i32, chunk_min: i32) -> Option<usize> {
    let local = world_coord - chunk_min;
    (0..16).contains(&local).then_some(local as usize)
}

fn block_entity_update_tag(fields: &HashMap<String, Tag>) -> Option<Tag> {
    let update_fields = fields
        .iter()
        .filter(|(key, _)| !matches!(key.as_str(), "id" | "x" | "y" | "z"))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect::<HashMap<_, _>>();
    (!update_fields.is_empty()).then(|| Tag::Compound(Arc::new(update_fields)))
}

fn minimal_chunk_root(chunk_x: i32, chunk_z: i32) -> Tag {
    Tag::Compound(Arc::new(HashMap::from([
        ("DataVersion".to_string(), Tag::Int(DATA_VERSION)),
        ("xPos".to_string(), Tag::Int(chunk_x)),
        ("yPos".to_string(), Tag::Int(MIN_SECTION_Y)),
        ("zPos".to_string(), Tag::Int(chunk_z)),
        ("LastUpdate".to_string(), Tag::Long(0)),
        ("InhabitedTime".to_string(), Tag::Long(0)),
        (
            "Status".to_string(),
            Tag::String(Arc::from("minecraft:full")),
        ),
        (
            "sections".to_string(),
            list_tag(tag_id::COMPOUND, Vec::new()),
        ),
        ("block_entities".to_string(), empty_compound_list()),
        ("block_ticks".to_string(), empty_compound_list()),
        ("fluid_ticks".to_string(), empty_compound_list()),
        ("PostProcessing".to_string(), empty_list()),
        ("Heightmaps".to_string(), empty_heightmaps_tag()),
        ("structures".to_string(), empty_compound()),
    ])))
}

fn normalized_chunk_root(
    root: &Tag,
    chunk_x: i32,
    chunk_z: i32,
) -> crate::error::Result<Tag> {
    let mut root = compound(root)
        .ok_or_else(|| crate::error::WorldError::msg("chunk root is not a compound"))?
        .clone();
    root.insert("DataVersion".to_string(), Tag::Int(DATA_VERSION));
    root.insert("xPos".to_string(), Tag::Int(chunk_x));
    root.insert("yPos".to_string(), Tag::Int(MIN_SECTION_Y));
    root.insert("zPos".to_string(), Tag::Int(chunk_z));
    root.entry("LastUpdate".to_string()).or_insert(Tag::Long(0));
    root.entry("InhabitedTime".to_string())
        .or_insert(Tag::Long(0));
    root.entry("Status".to_string())
        .or_insert_with(|| Tag::String(Arc::from("minecraft:full")));
    root.entry("sections".to_string())
        .or_insert_with(|| list_tag(tag_id::COMPOUND, Vec::new()));
    root.entry("Heightmaps".to_string())
        .or_insert_with(empty_heightmaps_tag);
    root.entry("block_entities".to_string())
        .or_insert_with(empty_compound_list);
    root.entry("block_ticks".to_string())
        .or_insert_with(empty_compound_list);
    root.entry("fluid_ticks".to_string())
        .or_insert_with(empty_compound_list);
    root.entry("PostProcessing".to_string())
        .or_insert_with(empty_list);
    root.entry("structures".to_string())
        .or_insert_with(empty_compound);
    Ok(Tag::Compound(Arc::new(root)))
}

fn empty_section(section_y: i32, fallback_block_state: i32) -> HashMap<String, Tag> {
    let values = vec![fallback_block_state; BLOCK_ENTRY_COUNT];
    HashMap::from([
        ("Y".to_string(), Tag::Byte(section_y as i8)),
        (
            "block_states".to_string(),
            block_states_tag(&values).expect("fallback section block palette is valid"),
        ),
        ("biomes".to_string(), biome_states_tag("minecraft:plains")),
    ])
}

fn empty_heightmaps_tag() -> Tag {
    Tag::Compound(Arc::new(HashMap::from([
        (
            "WORLD_SURFACE".to_string(),
            Tag::LongArray(Arc::from(vec![0_i64; 37])),
        ),
        (
            "MOTION_BLOCKING".to_string(),
            Tag::LongArray(Arc::from(vec![0_i64; 37])),
        ),
        (
            "MOTION_BLOCKING_NO_LEAVES".to_string(),
            Tag::LongArray(Arc::from(vec![0_i64; 37])),
        ),
    ])))
}

fn empty_compound() -> Tag {
    Tag::Compound(Arc::new(HashMap::new()))
}

fn empty_compound_list() -> Tag {
    list_tag(tag_id::COMPOUND, Vec::new())
}

fn empty_list() -> Tag {
    list_tag(tag_id::END, Vec::new())
}

fn list_tag(tag_id: u8, items: Vec<Tag>) -> Tag {
    Tag::List(
        ListHeader {
            tag_id,
            length: items.len() as i32,
        },
        Arc::from(items),
    )
}

fn remove_block_entity_at(root: &mut HashMap<String, Tag>, position: &Position) {
    let Some(items) = root
        .get("block_entities")
        .and_then(|tag| list_items(Some(tag)))
    else {
        return;
    };
    let filtered = items
        .iter()
        .filter(|item| !block_entity_is_at(item, position))
        .cloned()
        .collect::<Vec<_>>();
    root.insert(
        "block_entities".to_string(),
        list_tag(tag_id::COMPOUND, filtered),
    );
}

fn block_entity_is_at(item: &Tag, position: &Position) -> bool {
    let Some(fields) = compound(item) else {
        return false;
    };
    int_field(fields, "x") == Some(position.x)
        && int_field(fields, "y") == Some(position.y)
        && int_field(fields, "z") == Some(position.z)
}

fn sections_by_y(root: &HashMap<String, Tag>) -> HashMap<i32, &HashMap<String, Tag>> {
    let mut sections = HashMap::new();
    let Some(items) = list_items(root.get("sections")) else {
        return sections;
    };

    for item in items {
        let Some(section) = compound(item) else {
            continue;
        };
        let Some(y) = int_field(section, "Y") else {
            continue;
        };
        sections.insert(y, section);
    }

    sections
}

fn block_index(position: &Position) -> usize {
    let local_x = position.x.rem_euclid(16) as usize;
    let local_y = position.y.rem_euclid(16) as usize;
    let local_z = position.z.rem_euclid(16) as usize;
    (local_y * 16 + local_z) * 16 + local_x
}

fn default_id(kind: PaletteKind) -> i32 {
    match kind {
        PaletteKind::Block => AIR_BLOCK_STATE_ID,
        PaletteKind::Biome => PLAINS_BIOME_ID,
    }
}

fn ceil_log2(count: usize) -> usize {
    if count <= 1 {
        0
    } else {
        usize::BITS as usize - (count - 1).leading_zeros() as usize
    }
}

#[derive(Clone, Copy)]
enum PaletteKind {
    Block,
    Biome,
}

struct BlockValues {
    global_ids: Vec<i32>,
    blocks: Vec<BlockPaletteEntry>,
    non_empty_count: usize,
    fluid_count: usize,
}

impl BlockValues {
    fn empty() -> Self {
        Self {
            global_ids: vec![AIR_BLOCK_STATE_ID; BLOCK_ENTRY_COUNT],
            blocks: vec![BlockPaletteEntry::air(); BLOCK_ENTRY_COUNT],
            non_empty_count: 0,
            fluid_count: 0,
        }
    }
}

#[derive(Clone, Copy)]
struct BlockPaletteEntry {
    id: i32,
    is_air: bool,
    has_fluid: bool,
    light_dampening: u8,
}

impl BlockPaletteEntry {
    fn air() -> Self {
        Self {
            id: AIR_BLOCK_STATE_ID,
            is_air: true,
            has_fluid: false,
            light_dampening: 0,
        }
    }
}

pub fn all_section_block_states(root: &Tag) -> crate::error::Result<Vec<SectionBlockStates>> {
    let root = compound(root)
        .ok_or_else(|| crate::error::WorldError::msg("not a compound"))?;
    let sections = sections_by_y(root);
    let mut result = Vec::with_capacity(sections.len());
    for (&sy, section) in &sections {
        let values = block_values(section.get("block_states"))?;
        result.push(SectionBlockStates {
            section_y: sy,
            states: values.global_ids,
        });
    }
    Ok(result)
}

/// 方块碰撞近似提示（ore_pits 撤离目标用）：光照抑制 15 视为实体方块。
///
/// TODO(play)：play crate 的 collision_block_states 注册表迁移后替换。
pub fn block_collision_hint(block_state: i32) -> bool {
    let entry = block_state_entry(block_state);
    registry::light_dampening(
        &entry.name,
        block_state_registry()
            .metadata_by_name
            .get(entry.name.as_str())
            .map(|metadata| metadata.block_type.as_str()),
        false,
    ) == 15
}

pub struct SectionBlockStates {
    pub section_y: i32,
    pub states: Vec<i32>,
}

pub fn fluid_positions_from_region(
    chunk_x: i32,
    chunk_z: i32,
    chunk: &ChunkData,
) -> crate::error::Result<Vec<(Position, i32)>> {
    let raw = chunk.decompress()?;
    let (_, root) = qexed_nbt::from_slice(&raw)?;
    fluid_positions_from_nbt(chunk_x, chunk_z, &root)
}

pub fn fluid_positions_from_nbt(
    chunk_x: i32,
    chunk_z: i32,
    root: &Tag,
) -> crate::error::Result<Vec<(Position, i32)>> {
    let root = compound(root)
        .ok_or_else(|| crate::error::WorldError::msg("chunk root is not a compound"))?;
    let sections = sections_by_y(root);
    let mut result = Vec::new();
    for (&section_y, section) in &sections {
        let values = block_values(section.get("block_states"))?;
        for (index, block_state) in values.global_ids.iter().copied().enumerate() {
            let entry = block_state_entry(block_state);
            if !matches!(entry.name.as_str(), "minecraft:water" | "minecraft:lava") {
                continue;
            }
            let local_x = (index & 15) as i32;
            let local_z = ((index >> 4) & 15) as i32;
            let local_y = ((index >> 8) & 15) as i32;
            result.push((
                Position {
                    x: chunk_x * 16 + local_x,
                    y: section_y * 16 + local_y,
                    z: chunk_z * 16 + local_z,
                },
                block_state,
            ));
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests;

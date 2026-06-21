use std::{collections::HashMap, sync::Arc};

use anyhow::{Context, Result};
use bytes::BytesMut;
use qexed_nbt::{ListHeader, Tag, tag_id};
use qexed_packet::{PacketCodec, PacketWriter, net_types::OptionalNbt, net_types::VarInt};
use qexed_protocol::to_client::play::map_chunk::{
    BlockEntities, Chunk, Heightmaps, LIGHT_ARRAY_BYTES, Light, LightArray, MapChunk,
};

use super::{WorldLightAlgorithm, empty_heightmaps, section_count};
use crate::light::{
    block_light_dampening_index, light_from_layers, light_section_index, sky_light_from_dampening,
    write_empty_section, write_fixed_long_array,
};
use crate::region::ChunkData;

mod nbt;
mod registry;

use nbt::{
    byte_array, compound, int_field, list_items, long_array, string_field, string_properties,
};
#[allow(unused_imports)]
pub use registry::{
    BlockStateDefinition, block_state, block_state_entry, default_block_state,
    default_block_state_id, default_block_state_id_if_known,
};
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

#[cfg(test)]
#[allow(dead_code)]
fn network_chunk_from_region(chunk_x: i32, chunk_z: i32, chunk: &ChunkData) -> Result<MapChunk> {
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
) -> Result<(MapChunk, Vec<u8>)> {
    let raw = chunk.decompress().context("decompress chunk nbt")?;
    let (_, root) = qexed_nbt::from_slice(&raw).context("parse chunk nbt")?;
    network_chunk_and_light_dampening_from_nbt(chunk_x, chunk_z, &root, light_algorithm)
}

pub fn light_dampening_from_region(chunk: &ChunkData) -> Result<Vec<u8>> {
    let raw = chunk.decompress().context("decompress chunk nbt")?;
    let (_, root) = qexed_nbt::from_slice(&raw).context("parse chunk nbt")?;
    light_dampening_from_nbt(&root)
}

pub fn block_state_at_from_region(
    chunk: &ChunkData,
    position: &qexed_packet::net_types::Position,
) -> Result<Option<i32>> {
    let raw = chunk.decompress().context("decompress chunk nbt")?;
    let (_, root) = qexed_nbt::from_slice(&raw).context("parse chunk nbt")?;
    block_state_at_from_nbt(&root, position)
}

pub fn set_block_state_in_region(
    chunk_x: i32,
    chunk_z: i32,
    existing: Option<&ChunkData>,
    position: &qexed_packet::net_types::Position,
    block_state: i32,
    fallback_block_state: Option<i32>,
) -> Result<ChunkData> {
    let root = if let Some(existing) = existing {
        let raw = existing.decompress().context("decompress chunk nbt")?;
        let (_, root) = qexed_nbt::from_slice(&raw).context("parse chunk nbt")?;
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
    let raw = qexed_nbt::to_vec("", &updated).context("serialize chunk nbt")?;
    ChunkData::zlib(&raw).context("compress chunk nbt")
}

pub fn set_block_states_in_region(
    chunk_x: i32,
    chunk_z: i32,
    existing: Option<&ChunkData>,
    blocks: &[(qexed_packet::net_types::Position, i32, Option<i32>)],
) -> Result<ChunkData> {
    let mut root = if let Some(existing) = existing {
        let raw = existing.decompress().context("decompress chunk nbt")?;
        let (_, root) = qexed_nbt::from_slice(&raw).context("parse chunk nbt")?;
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

    let raw = qexed_nbt::to_vec("", &root).context("serialize chunk nbt")?;
    ChunkData::zlib(&raw).context("compress chunk nbt")
}

pub fn region_chunk_from_nbt(chunk_x: i32, chunk_z: i32, root: &Tag) -> Result<ChunkData> {
    let root = normalized_chunk_root(root, chunk_x, chunk_z)?;
    let raw = qexed_nbt::to_vec("", &root).context("serialize chunk nbt")?;
    ChunkData::zlib(&raw).context("compress chunk nbt")
}

#[cfg(test)]
#[allow(dead_code)]
fn network_chunk_from_nbt(chunk_x: i32, chunk_z: i32, root: &Tag) -> Result<MapChunk> {
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
) -> Result<(MapChunk, Vec<u8>)> {
    let root = compound(root).context("chunk root is not a compound")?;
    let sections = sections_by_y(root);
    let section_data = chunk_section_bytes(&sections)?;
    let block_dampening = chunk_light_dampening(&sections)?;
    let light = saved_light(&sections)
        .unwrap_or_else(|| sky_light_from_dampening(&block_dampening, light_algorithm));
    let packet = MapChunk {
        chunk_x,
        chunk_z,
        data: Chunk {
            heightmaps: heightmaps(root),
            data: section_data,
            block_entities: block_entities(root, chunk_x, chunk_z),
        },
        light,
    };
    Ok((packet, block_dampening))
}

pub fn light_dampening_from_nbt(root: &Tag) -> Result<Vec<u8>> {
    let root = compound(root).context("chunk root is not a compound")?;
    let sections = sections_by_y(root);
    chunk_light_dampening(&sections)
}

pub fn block_state_at_from_nbt(
    root: &Tag,
    position: &qexed_packet::net_types::Position,
) -> Result<Option<i32>> {
    let root = compound(root).context("chunk root is not a compound")?;
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
        .context("block index out of section bounds")?;
    let block_state = values
        .global_ids
        .get(index)
        .copied()
        .context("block index out of section bounds")?;
    Ok((!block.is_air).then_some(block_state))
}

pub fn set_block_state_in_nbt(
    root: &Tag,
    chunk_x: i32,
    chunk_z: i32,
    position: &qexed_packet::net_types::Position,
    block_state: i32,
    fallback_block_state: Option<i32>,
) -> Result<Tag> {
    let mut root = compound(root)
        .context("chunk root is not a compound")?
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
            .with_context(|| format!("section y={section_y} is not a compound"))?
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

fn chunk_section_bytes(sections: &HashMap<i32, &HashMap<String, Tag>>) -> Result<Vec<u8>> {
    let mut bytes = BytesMut::new();
    let mut writer = PacketWriter::new(&mut bytes);

    for section_y in MIN_SECTION_Y..MIN_SECTION_Y + section_count() {
        if let Some(section) = sections.get(&section_y) {
            write_section(&mut writer, section)
                .with_context(|| format!("serialize section y={section_y}"))?;
        } else {
            write_empty_section(&mut writer)?;
        }
    }

    Ok(bytes.to_vec())
}

fn chunk_light_dampening(sections: &HashMap<i32, &HashMap<String, Tag>>) -> Result<Vec<u8>> {
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
    let mut sky_layers = vec![LightArray::default(); super::LIGHT_SECTION_COUNT];
    let mut block_layers = vec![LightArray::default(); super::LIGHT_SECTION_COUNT];
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

fn light_array(tag: Option<&Tag>) -> Option<LightArray> {
    let bytes = byte_array(tag?)?;
    if bytes.len() != LIGHT_ARRAY_BYTES {
        log::warn!(
            "invalid saved light layer length: got {}, expected {}",
            bytes.len(),
            LIGHT_ARRAY_BYTES
        );
        return None;
    }

    let mut layer = LightArray::default();
    for (target, source) in layer.0.iter_mut().zip(bytes.iter()) {
        *target = *source as u8;
    }
    Some(layer)
}

fn write_section(writer: &mut PacketWriter, section: &HashMap<String, Tag>) -> Result<()> {
    let blocks = block_values(section.get("block_states"))?;
    let biomes = biome_values(section.get("biomes"))?;

    (blocks.non_empty_count as i16).serialize(writer)?;
    (blocks.fluid_count as i16).serialize(writer)?;
    write_paletted_container(writer, &blocks.global_ids, PaletteKind::Block)?;
    write_paletted_container(writer, &biomes, PaletteKind::Biome)?;
    Ok(())
}

fn block_values(tag: Option<&Tag>) -> Result<BlockValues> {
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
        let entry = palette.get(index).with_context(|| {
            format!(
                "block palette index {index} out of range for palette size {}",
                palette.len()
            )
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

fn biome_values(tag: Option<&Tag>) -> Result<Vec<i32>> {
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
            palette.get(index).copied().with_context(|| {
                format!(
                    "biome palette index {index} out of range for palette size {}",
                    palette.len()
                )
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
            log::warn!("unknown block state in saved chunk, using air: {key}");
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
                        log::warn!("unknown biome in saved chunk, using plains: {name}");
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
) -> Result<Vec<usize>> {
    if palette_len == 0 {
        return Ok(vec![0; entry_count]);
    }

    let bits = storage_bits(kind, palette_len);
    if bits == 0 {
        return Ok(vec![0; entry_count]);
    }

    let data = data_tag.and_then(long_array).with_context(|| {
        format!("missing paletted container data for palette size {palette_len}")
    })?;
    let values_per_long = 64 / bits;
    let required_len = entry_count.div_ceil(values_per_long);
    if data.len() != required_len {
        anyhow::bail!(
            "invalid paletted container data length: got {}, expected {}",
            data.len(),
            required_len
        );
    }

    let mask = (1_u64 << bits) - 1;
    let mut values = Vec::with_capacity(entry_count);
    for index in 0..entry_count {
        let cell = index / values_per_long;
        let bit_offset = (index % values_per_long) * bits;
        let value = ((data[cell] as u64) >> bit_offset) & mask;
        let value = usize::try_from(value).context("palette index does not fit usize")?;
        if value >= palette_len {
            anyhow::bail!("palette index {value} out of range for palette size {palette_len}");
        }
        values.push(value);
    }

    Ok(values)
}

fn write_paletted_container(
    writer: &mut PacketWriter,
    global_ids: &[i32],
    kind: PaletteKind,
) -> Result<()> {
    let (palette, local_values) = local_palette(global_ids);
    let (bits, global_palette) = network_config(kind, palette.len());
    (bits as u8).serialize(writer)?;

    if bits == 0 {
        VarInt(palette.first().copied().unwrap_or(default_id(kind))).serialize(writer)?;
        return write_fixed_long_array(writer, &[]);
    }

    let packed_values = if global_palette {
        pack_values(global_ids, bits)?
    } else {
        VarInt(palette.len() as i32).serialize(writer)?;
        for id in &palette {
            VarInt(*id).serialize(writer)?;
        }
        let local_values = local_values
            .into_iter()
            .map(|value| i32::try_from(value).context("local palette index does not fit i32"))
            .collect::<Result<Vec<_>>>()?;
        pack_values(&local_values, bits)?
    };

    write_fixed_long_array(writer, &packed_values)
}

fn block_states_tag(global_ids: &[i32]) -> Result<Tag> {
    if global_ids.len() != BLOCK_ENTRY_COUNT {
        anyhow::bail!(
            "invalid block state count: got {}, expected {}",
            global_ids.len(),
            BLOCK_ENTRY_COUNT
        );
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
                .map(|value| i32::try_from(value).context("palette index does not fit i32"))
                .collect::<Result<Vec<_>>>()?;
            pack_values(&local_values, bits)
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

fn pack_values(values: &[i32], bits: usize) -> Result<Vec<u64>> {
    if bits == 0 {
        return Ok(Vec::new());
    }

    let values_per_long = 64 / bits;
    let mut packed = vec![0_u64; values.len().div_ceil(values_per_long)];
    let mask = (1_u64 << bits) - 1;

    for (index, value) in values.iter().enumerate() {
        if *value < 0 {
            anyhow::bail!("negative paletted value: {value}");
        }

        let value = *value as u64;
        if value > mask {
            anyhow::bail!("paletted value {value} exceeds {bits} bits");
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

fn heightmaps(root: &HashMap<String, Tag>) -> Vec<Heightmaps> {
    let Some(heightmaps) = root.get("Heightmaps").and_then(compound) else {
        return empty_heightmaps();
    };

    [
        ("WORLD_SURFACE", 1),
        ("MOTION_BLOCKING", 4),
        ("MOTION_BLOCKING_NO_LEAVES", 5),
    ]
    .into_iter()
    .map(|(name, type_id)| Heightmaps {
        type_id: VarInt(type_id),
        data: heightmaps
            .get(name)
            .and_then(long_array)
            .map(|values| values.iter().map(|value| *value as u64).collect())
            .unwrap_or_else(|| vec![0; 37]),
    })
    .collect()
}

fn block_entities(root: &HashMap<String, Tag>, chunk_x: i32, chunk_z: i32) -> Vec<BlockEntities> {
    let chunk_min_x = chunk_x * 16;
    let chunk_min_z = chunk_z * 16;
    let mut entities = root
        .get("block_entities")
        .and_then(|tag| list_items(Some(tag)))
        .into_iter()
        .flatten()
        .filter_map(|item| block_entity(item, chunk_min_x, chunk_min_z))
        .collect::<Vec<_>>();
    entities.sort_by_key(|entity| (entity.y, entity.xz));
    entities
}

fn block_entity(item: &Tag, chunk_min_x: i32, chunk_min_z: i32) -> Option<BlockEntities> {
    let fields = compound(item)?;
    let id = string_field(fields, "id")?;
    let entity_type = block_entity_type_id(id).or_else(|| {
        log::warn!("unknown saved block entity type, skipping: {id}");
        None
    })?;
    let x = int_field(fields, "x")?;
    let y = int_field(fields, "y")?;
    let z = int_field(fields, "z")?;
    let local_x = local_chunk_coord(x, chunk_min_x)?;
    let local_z = local_chunk_coord(z, chunk_min_z)?;
    Some(BlockEntities {
        xz: ((local_x as u8) << 4) | local_z as u8,
        y: y as u16,
        entity_type: VarInt(entity_type),
        nbt: OptionalNbt(block_entity_update_tag(fields)),
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

fn normalized_chunk_root(root: &Tag, chunk_x: i32, chunk_z: i32) -> Result<Tag> {
    let mut root = compound(root)
        .context("chunk root is not a compound")?
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

fn remove_block_entity_at(
    root: &mut HashMap<String, Tag>,
    position: &qexed_packet::net_types::Position,
) {
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

fn block_entity_is_at(item: &Tag, position: &qexed_packet::net_types::Position) -> bool {
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

fn block_index(position: &qexed_packet::net_types::Position) -> usize {
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

pub fn all_section_block_states(root: &Tag) -> Result<Vec<SectionBlockStates>> {
    let root = compound(root).context("not a compound")?;
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
pub struct SectionBlockStates {
    pub section_y: i32,
    pub states: Vec<i32>,
}

pub fn fluid_positions_from_region(
    chunk_x: i32,
    chunk_z: i32,
    chunk: &ChunkData,
) -> Result<Vec<(qexed_packet::net_types::Position, i32)>> {
    let raw = chunk.decompress().context("decompress chunk nbt")?;
    let (_, root) = qexed_nbt::from_slice(&raw).context("parse chunk nbt")?;
    fluid_positions_from_nbt(chunk_x, chunk_z, &root)
}

pub fn fluid_positions_from_nbt(
    chunk_x: i32,
    chunk_z: i32,
    root: &Tag,
) -> Result<Vec<(qexed_packet::net_types::Position, i32)>> {
    let root = compound(root).context("chunk root is not a compound")?;
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
                qexed_packet::net_types::Position {
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

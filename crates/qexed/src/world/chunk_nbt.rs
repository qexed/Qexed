use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::OnceLock,
};

use anyhow::{Context, Result};
use bytes::BytesMut;
use qexed_nbt::Tag;
use qexed_packet::{PacketCodec, PacketWriter, net_types::VarInt};
use qexed_protocol::to_client::play::map_chunk::{
    Chunk, Heightmaps, LIGHT_ARRAY_BYTES, Light, LightArray, MapChunk,
};

use super::{
    WorldLightAlgorithm, block_light_dampening_index, empty_heightmaps, light_from_layers,
    light_section_index, section_count, sky_light_from_dampening, write_empty_section,
    write_fixed_long_array,
};
use crate::world::region::ChunkData;

const MIN_SECTION_Y: i32 = -4;
const BLOCK_ENTRY_COUNT: usize = 16 * 16 * 16;
const BIOME_ENTRY_COUNT: usize = 4 * 4 * 4;
const AIR_BLOCK_STATE_ID: i32 = 0;
const PLAINS_BIOME_ID: i32 = 40;
const BLOCKS_REPORT: &str = "assets/reports/blocks.json";
const BIOME_REGISTRY_DIR: &str = "assets/decompiled_source/src/data/minecraft/worldgen/biome";

pub fn network_chunk_from_region(
    chunk_x: i32,
    chunk_z: i32,
    chunk: &ChunkData,
) -> Result<MapChunk> {
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

pub fn network_chunk_from_nbt(chunk_x: i32, chunk_z: i32, root: &Tag) -> Result<MapChunk> {
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
            block_entities: Vec::new(),
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
    let id = registry.id_by_state.get(&key).copied().unwrap_or_else(|| {
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

fn block_state_registry() -> &'static BlockStateRegistry {
    static REGISTRY: OnceLock<BlockStateRegistry> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        load_block_state_registry().unwrap_or_else(|err| {
            log::warn!("failed to load block state registry report: {err:#}");
            BlockStateRegistry::fallback()
        })
    })
}

fn biome_registry() -> &'static BiomeRegistry {
    static REGISTRY: OnceLock<BiomeRegistry> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        load_biome_registry().unwrap_or_else(|err| {
            log::warn!("failed to load biome registry from assets: {err:#}");
            BiomeRegistry::fallback()
        })
    })
}

fn load_block_state_registry() -> Result<BlockStateRegistry> {
    let path = workspace_root().join(BLOCKS_REPORT);
    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("read block report {}", path.display()))?;
    let value: serde_json::Value =
        serde_json::from_str(&content).with_context(|| format!("parse {}", path.display()))?;
    let blocks = value
        .as_object()
        .with_context(|| format!("block report root is not object: {}", path.display()))?;

    let mut id_by_state = HashMap::new();
    let mut metadata_by_name = HashMap::new();
    let mut max_id = AIR_BLOCK_STATE_ID;
    for (name, block) in blocks {
        let block_type = block
            .get("definition")
            .and_then(|definition| definition.get("type"))
            .and_then(serde_json::Value::as_str)
            .map(normalize_identifier)
            .unwrap_or_else(|| "minecraft:block".to_string());
        metadata_by_name.insert(name.clone(), BlockMetadata { block_type });

        let Some(states) = block.get("states").and_then(serde_json::Value::as_array) else {
            continue;
        };

        for state in states {
            let Some(id) = state.get("id").and_then(serde_json::Value::as_i64) else {
                continue;
            };
            let Ok(id) = i32::try_from(id) else {
                continue;
            };
            let properties = json_string_properties(state.get("properties"));
            id_by_state.insert(state_key(name, &properties), id);
            max_id = max_id.max(id);
        }
    }

    if id_by_state.is_empty() {
        anyhow::bail!("block report contains no block states");
    }

    Ok(BlockStateRegistry {
        id_by_state,
        metadata_by_name,
        global_bits: ceil_log2((max_id as usize) + 1).max(1),
    })
}

fn load_biome_registry() -> Result<BiomeRegistry> {
    let root = workspace_root().join(BIOME_REGISTRY_DIR);
    let mut files = json_files(&root)?;
    files.sort();

    let mut id_by_name = HashMap::new();
    for (index, path) in files.iter().enumerate() {
        let id = entry_id_from_path(&root, path)?;
        id_by_name.insert(id, index as i32);
    }

    if id_by_name.is_empty() {
        anyhow::bail!("biome registry contains no entries");
    }

    Ok(BiomeRegistry {
        global_bits: ceil_log2(id_by_name.len()).max(1),
        id_by_name,
    })
}

fn json_files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    collect_json_files(root, &mut files)?;
    Ok(files)
}

fn collect_json_files(dir: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    for entry in std::fs::read_dir(dir).with_context(|| format!("read dir {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_json_files(&path, files)?;
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("json") {
            files.push(path);
        }
    }
    Ok(())
}

fn entry_id_from_path(root: &Path, path: &Path) -> Result<String> {
    let id = path
        .strip_prefix(root)?
        .with_extension("")
        .to_string_lossy()
        .replace('\\', "/");
    Ok(format!("minecraft:{id}"))
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn json_string_properties(value: Option<&serde_json::Value>) -> Vec<(String, String)> {
    let Some(properties) = value.and_then(serde_json::Value::as_object) else {
        return Vec::new();
    };

    let mut properties = properties
        .iter()
        .filter_map(|(key, value)| {
            value
                .as_str()
                .map(|value| (key.to_string(), value.to_string()))
        })
        .collect::<Vec<_>>();
    properties.sort_by(|left, right| left.0.cmp(&right.0));
    properties
}

fn string_properties(value: Option<&Tag>) -> Vec<(String, String)> {
    let Some(properties) = value.and_then(compound) else {
        return Vec::new();
    };

    let mut properties = properties
        .iter()
        .filter_map(|(key, value)| match value {
            Tag::String(value) => Some((key.clone(), value.to_string())),
            _ => None,
        })
        .collect::<Vec<_>>();
    properties.sort_by(|left, right| left.0.cmp(&right.0));
    properties
}

fn state_key(name: &str, properties: &[(String, String)]) -> String {
    let mut key = normalize_identifier(name);
    key.push('|');
    for (name, value) in properties {
        key.push_str(name);
        key.push('=');
        key.push_str(value);
        key.push(';');
    }
    key
}

fn normalize_identifier(value: &str) -> String {
    if value.contains(':') {
        value.to_string()
    } else {
        format!("minecraft:{value}")
    }
}

fn is_air_block(name: &str) -> bool {
    matches!(
        name,
        "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air"
    )
}

fn has_fluid(name: &str, properties: &[(String, String)]) -> bool {
    name == "minecraft:water"
        || name == "minecraft:lava"
        || is_always_water_filled_block(name)
        || properties
            .iter()
            .any(|(key, value)| key == "waterlogged" && value == "true")
}

fn is_always_water_filled_block(name: &str) -> bool {
    matches!(
        name,
        "minecraft:bubble_column"
            | "minecraft:kelp"
            | "minecraft:kelp_plant"
            | "minecraft:seagrass"
            | "minecraft:tall_seagrass"
    )
}

fn light_dampening(name: &str, block_type: Option<&str>, has_fluid: bool) -> u8 {
    if has_fluid || is_one_light_dampening_block_type(block_type) {
        1
    } else if is_air_block(name) || is_zero_light_dampening_block_type(block_type, name) {
        0
    } else {
        15
    }
}

fn is_one_light_dampening_block_type(block_type: Option<&str>) -> bool {
    matches!(
        block_type,
        Some(
            "minecraft:liquid"
                | "minecraft:mangrove_leaves"
                | "minecraft:tinted_particle_leaves"
                | "minecraft:untinted_particle_leaves"
        )
    )
}

fn is_zero_light_dampening_block_type(block_type: Option<&str>, name: &str) -> bool {
    matches!(
        block_type,
        Some(
            "minecraft:air"
                | "minecraft:barrier"
                | "minecraft:bamboo_sapling"
                | "minecraft:bamboo_stalk"
                | "minecraft:big_dripleaf"
                | "minecraft:big_dripleaf_stem"
                | "minecraft:button"
                | "minecraft:cave_vines"
                | "minecraft:cave_vines_plant"
                | "minecraft:cross_collision"
                | "minecraft:door"
                | "minecraft:end_portal"
                | "minecraft:fence"
                | "minecraft:fence_gate"
                | "minecraft:fire"
                | "minecraft:flower_pot"
                | "minecraft:glow_lichen"
                | "minecraft:hanging_moss"
                | "minecraft:iron_bars"
                | "minecraft:ladder"
                | "minecraft:light"
                | "minecraft:mossy_carpet"
                | "minecraft:nether_sprouts"
                | "minecraft:pressure_plate"
                | "minecraft:seagrass"
                | "minecraft:sea_pickle"
                | "minecraft:short_dry_grass"
                | "minecraft:small_dripleaf"
                | "minecraft:snow_layer"
                | "minecraft:tall_dry_grass"
                | "minecraft:tall_grass"
                | "minecraft:torch"
                | "minecraft:transparent"
                | "minecraft:trapdoor"
                | "minecraft:twisting_vines"
                | "minecraft:twisting_vines_plant"
                | "minecraft:vine"
                | "minecraft:void"
                | "minecraft:wall_banner"
                | "minecraft:wall_hanging_sign"
                | "minecraft:wall_sign"
                | "minecraft:wall_skull"
                | "minecraft:wall_torch"
                | "minecraft:weeping_vines"
                | "minecraft:weeping_vines_plant"
        )
    ) || matches!(
        name,
        "minecraft:structure_void"
            | "minecraft:glass"
            | "minecraft:ice"
            | "minecraft:packed_ice"
            | "minecraft:blue_ice"
    )
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

fn compound(tag: &Tag) -> Option<&HashMap<String, Tag>> {
    match tag {
        Tag::Compound(compound) => Some(compound),
        _ => None,
    }
}

fn list_items(tag: Option<&Tag>) -> Option<&[Tag]> {
    match tag {
        Some(Tag::List(_, items)) => Some(items),
        _ => None,
    }
}

fn long_array(tag: &Tag) -> Option<&[i64]> {
    match tag {
        Tag::LongArray(values) => Some(values),
        _ => None,
    }
}

fn byte_array(tag: &Tag) -> Option<&[i8]> {
    match tag {
        Tag::ByteArray(values) => Some(values),
        _ => None,
    }
}

fn string_field<'a>(compound: &'a HashMap<String, Tag>, name: &str) -> Option<&'a str> {
    match compound.get(name) {
        Some(Tag::String(value)) => Some(value),
        _ => None,
    }
}

fn int_field(compound: &HashMap<String, Tag>, name: &str) -> Option<i32> {
    match compound.get(name) {
        Some(Tag::Byte(value)) => Some(i32::from(*value)),
        Some(Tag::Short(value)) => Some(i32::from(*value)),
        Some(Tag::Int(value)) => Some(*value),
        Some(Tag::Long(value)) => i32::try_from(*value).ok(),
        _ => None,
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

struct BlockStateRegistry {
    id_by_state: HashMap<String, i32>,
    metadata_by_name: HashMap<String, BlockMetadata>,
    global_bits: usize,
}

struct BlockMetadata {
    block_type: String,
}

impl BlockStateRegistry {
    fn fallback() -> Self {
        let mut id_by_state = HashMap::new();
        id_by_state.insert("minecraft:air|".to_string(), AIR_BLOCK_STATE_ID);
        id_by_state.insert("minecraft:stone|".to_string(), 1);
        id_by_state.insert("minecraft:water|level=0;".to_string(), 86);
        id_by_state.insert("minecraft:lava|level=0;".to_string(), 102);
        let mut metadata_by_name = HashMap::new();
        metadata_by_name.insert(
            "minecraft:air".to_string(),
            BlockMetadata {
                block_type: "minecraft:air".to_string(),
            },
        );
        metadata_by_name.insert(
            "minecraft:stone".to_string(),
            BlockMetadata {
                block_type: "minecraft:block".to_string(),
            },
        );
        metadata_by_name.insert(
            "minecraft:water".to_string(),
            BlockMetadata {
                block_type: "minecraft:liquid".to_string(),
            },
        );
        metadata_by_name.insert(
            "minecraft:lava".to_string(),
            BlockMetadata {
                block_type: "minecraft:liquid".to_string(),
            },
        );
        Self {
            id_by_state,
            metadata_by_name,
            global_bits: 14,
        }
    }
}

struct BiomeRegistry {
    id_by_name: HashMap<String, i32>,
    global_bits: usize,
}

impl BiomeRegistry {
    fn fallback() -> Self {
        let mut id_by_name = HashMap::new();
        id_by_name.insert("minecraft:plains".to_string(), PLAINS_BIOME_ID);
        Self {
            id_by_name,
            global_bits: 6,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use qexed_nbt::{ListHeader, tag_id};
    use qexed_packet::Packet;

    use super::*;

    #[test]
    fn converts_single_value_saved_section() {
        let root = chunk_root(vec![section(
            0,
            paletted_container(vec![block_state("minecraft:stone", &[])], None),
            paletted_container(vec![string("minecraft:plains")], None),
        )]);

        let packet = network_chunk_from_nbt(0, 0, &root).unwrap();
        let section_offset = ((0 - MIN_SECTION_Y) as usize) * 8;

        assert_eq!(packet.chunk_x, 0);
        assert_eq!(packet.chunk_z, 0);
        assert_eq!(
            &packet.data.data[section_offset..section_offset + 4],
            &[0x10, 0x00, 0x00, 0x00]
        );
    }

    #[test]
    fn converts_multi_value_saved_section() {
        let mut values = vec![0_i32; BLOCK_ENTRY_COUNT];
        values[0] = 1;
        let data = pack_values(&values, 4)
            .unwrap()
            .into_iter()
            .map(|value| value as i64)
            .collect();
        let root = chunk_root(vec![section(
            0,
            paletted_container(
                vec![
                    block_state("minecraft:air", &[]),
                    block_state("minecraft:stone", &[]),
                ],
                Some(data),
            ),
            paletted_container(vec![string("minecraft:plains")], None),
        )]);

        let packet = network_chunk_from_nbt(0, 0, &root).unwrap();
        let section_offset = 4 * 8;

        assert!(packet.data.data.len() > super::section_count() as usize * 8);
        assert_eq!(
            &packet.data.data[section_offset..section_offset + 4],
            &[0x00, 0x01, 0x00, 0x00]
        );
    }

    #[test]
    fn converts_region_chunk_payload() {
        let root = chunk_root(vec![section(
            0,
            paletted_container(vec![block_state("minecraft:stone", &[])], None),
            paletted_container(vec![string("minecraft:plains")], None),
        )]);
        let raw = qexed_nbt::to_vec("", &root).unwrap();
        let chunk = ChunkData::zlib(&raw).unwrap();
        let packet = network_chunk_from_region(0, 0, &chunk).unwrap();
        let mut payload = bytes::BytesMut::new();
        let mut writer = qexed_packet::PacketWriter::new(&mut payload);

        packet.serialize(&mut writer).unwrap();

        assert!(!payload.is_empty());
    }

    #[test]
    fn water_plant_counts_as_fluid_and_light_dampening() {
        let root = chunk_root(vec![section(
            0,
            paletted_container(vec![block_state("minecraft:seagrass", &[])], None),
            paletted_container(vec![string("minecraft:plains")], None),
        )]);

        let (packet, dampening) =
            network_chunk_and_light_dampening_from_nbt(0, 0, &root, WorldLightAlgorithm::Fast)
                .unwrap();
        let section_offset = ((0 - MIN_SECTION_Y) as usize) * 8;
        let world_y = 0;

        assert_eq!(
            &packet.data.data[section_offset..section_offset + 4],
            &[0x10, 0x00, 0x10, 0x00]
        );
        assert_eq!(dampening[block_light_dampening_index(0, world_y, 0)], 1);
    }

    #[test]
    fn waterlogged_block_counts_as_fluid_dampening() {
        assert!(has_fluid(
            "minecraft:sea_pickle",
            &[("waterlogged".to_string(), "true".to_string())]
        ));
        assert_eq!(
            light_dampening("minecraft:sea_pickle", Some("minecraft:sea_pickle"), true),
            1
        );
        assert_eq!(
            light_dampening("minecraft:sea_pickle", Some("minecraft:sea_pickle"), false),
            0
        );
    }

    #[test]
    fn leaves_dampen_sky_light_like_minecraft() {
        assert_eq!(
            light_dampening(
                "minecraft:oak_leaves",
                Some("minecraft:tinted_particle_leaves"),
                false
            ),
            1
        );
        assert_eq!(
            light_dampening(
                "minecraft:mangrove_leaves",
                Some("minecraft:mangrove_leaves"),
                false
            ),
            1
        );
    }

    #[test]
    fn transparent_block_types_use_report_metadata() {
        assert_eq!(
            light_dampening("minecraft:vine", Some("minecraft:vine"), false),
            0
        );
        assert_eq!(
            light_dampening(
                "minecraft:glow_lichen",
                Some("minecraft:glow_lichen"),
                false
            ),
            0
        );
        assert_eq!(
            light_dampening("minecraft:glass_pane", Some("minecraft:iron_bars"), false),
            0
        );
        assert_eq!(
            light_dampening("minecraft:iron_chain", Some("minecraft:chain"), false),
            15
        );
        assert_eq!(
            light_dampening("minecraft:iron_chain", Some("minecraft:chain"), true),
            1
        );
    }

    #[test]
    fn saved_light_layers_are_used_when_present() {
        let root = chunk_root(vec![section_with_light(
            0,
            paletted_container(vec![block_state("minecraft:air", &[])], None),
            paletted_container(vec![string("minecraft:plains")], None),
            Some(vec![0xff_u8; LIGHT_ARRAY_BYTES]),
            Some(vec![0x77_u8; LIGHT_ARRAY_BYTES]),
        )]);

        let (packet, _) =
            network_chunk_and_light_dampening_from_nbt(0, 0, &root, WorldLightAlgorithm::Fast)
                .unwrap();

        assert_eq!(packet.light.sky_light_arrays.len(), 1);
        assert_eq!(packet.light.block_light_arrays.len(), 1);
        assert_eq!(packet.light.sky_light_arrays[0].0[0], 0xff);
        assert_eq!(packet.light.block_light_arrays[0].0[0], 0x77);
    }

    fn chunk_root(sections: Vec<Tag>) -> Tag {
        compound_tag([
            (
                "sections",
                Tag::List(
                    ListHeader {
                        tag_id: tag_id::COMPOUND,
                        length: sections.len() as i32,
                    },
                    Arc::from(sections),
                ),
            ),
            (
                "Heightmaps",
                compound_tag([
                    ("WORLD_SURFACE", Tag::LongArray(Arc::from(vec![0_i64; 37]))),
                    (
                        "MOTION_BLOCKING",
                        Tag::LongArray(Arc::from(vec![0_i64; 37])),
                    ),
                    (
                        "MOTION_BLOCKING_NO_LEAVES",
                        Tag::LongArray(Arc::from(vec![0_i64; 37])),
                    ),
                ]),
            ),
        ])
    }

    fn section(y: i8, block_states: Tag, biomes: Tag) -> Tag {
        section_with_light(y, block_states, biomes, None, None)
    }

    fn section_with_light(
        y: i8,
        block_states: Tag,
        biomes: Tag,
        sky_light: Option<Vec<u8>>,
        block_light: Option<Vec<u8>>,
    ) -> Tag {
        let mut section = HashMap::new();
        section.insert("Y".to_string(), Tag::Byte(y));
        section.insert("block_states".to_string(), block_states);
        section.insert("biomes".to_string(), biomes);
        if let Some(sky_light) = sky_light {
            section.insert(
                "SkyLight".to_string(),
                Tag::byte_array_from_u8_slice(&sky_light),
            );
        }
        if let Some(block_light) = block_light {
            section.insert(
                "BlockLight".to_string(),
                Tag::byte_array_from_u8_slice(&block_light),
            );
        }
        Tag::Compound(Arc::new(section))
    }

    fn paletted_container(palette: Vec<Tag>, data: Option<Vec<i64>>) -> Tag {
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

    fn block_state(name: &str, properties: &[(&str, &str)]) -> Tag {
        let mut fields = HashMap::new();
        fields.insert("Name".to_string(), string(name));
        if !properties.is_empty() {
            let properties = properties
                .iter()
                .map(|(name, value)| (name.to_string(), string(value)))
                .collect();
            fields.insert(
                "Properties".to_string(),
                Tag::Compound(Arc::new(properties)),
            );
        }
        Tag::Compound(Arc::new(fields))
    }

    fn compound_tag<I>(fields: I) -> Tag
    where
        I: IntoIterator<Item = (&'static str, Tag)>,
    {
        Tag::Compound(Arc::new(
            fields
                .into_iter()
                .map(|(name, value)| (name.to_string(), value))
                .collect(),
        ))
    }

    fn string(value: &str) -> Tag {
        Tag::String(Arc::from(value))
    }
}

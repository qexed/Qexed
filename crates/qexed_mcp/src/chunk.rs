use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{Context, Result};
use qexed_nbt::{ListHeader, Tag, tag_id};

use crate::region::ChunkData;

pub const WORLD_MIN_Y: i32 = -64;
pub const WORLD_MAX_Y: i32 = 319;
const WORLD_MIN_SECTION_Y: i32 = WORLD_MIN_Y / 16;
const SECTION_HEIGHT: i32 = 16;
const WORLD_SECTION_COUNT: usize = 24;
const BLOCK_ENTRY_COUNT: usize = 16 * 16 * 16;
const BIOME_ENTRY_COUNT: usize = 4 * 4 * 4;
const AIR_BLOCK_STATE_ID: i32 = 0;
const PLAINS_BIOME_ID: i32 = 40;
const DATA_VERSION: i32 = 4790;

/// A rectangular region of blocks read from the world.
#[derive(Debug, Clone)]
pub struct BlockRegion {
    pub min_x: i32,
    pub min_y: i32,
    pub min_z: i32,
    pub size_x: usize,
    pub size_y: usize,
    pub size_z: usize,
    /// Flat array of block_state_ids, indexed by [y * size_x * size_z + z * size_x + x]
    pub blocks: Vec<i32>,
}

/// Read all block states in a 3D region from the world save.
pub fn read_blocks_region(
    save_path: &std::path::Path,
    dimension: &str,
    min_x: i32,
    min_y: i32,
    min_z: i32,
    max_x: i32,
    max_y: i32,
    max_z: i32,
) -> Result<BlockRegion> {
    let size_x = (max_x - min_x + 1).max(0) as usize;
    let size_y = (max_y - min_y + 1).max(0) as usize;
    let size_z = (max_z - min_z + 1).max(0) as usize;
    let total = size_x * size_y * size_z;
    let mut blocks = vec![AIR_BLOCK_STATE_ID; total];

    // Iterate over chunks that overlap the region
    let min_cx = min_x.div_euclid(16);
    let max_cx = max_x.div_euclid(16);
    let min_cz = min_z.div_euclid(16);
    let max_cz = max_z.div_euclid(16);

    for cx in min_cx..=max_cx {
        for cz in min_cz..=max_cz {
            let region_dir = dimension_region_path(save_path, dimension);
            let region_path = region_dir.join(crate::region::region_file_name(cx, cz));

            let chunk_data = if region_path.exists() {
                let region =
                    crate::region::AnvilRegion::from_file(&region_path).with_context(|| {
                        format!("failed to open region file: {}", region_path.display())
                    })?;
                region.read_chunk(cx, cz)?
            } else {
                None
            };

            if let Some(chunk) = &chunk_data {
                let raw = chunk.decompress()?;
                let (_, root) = qexed_nbt::from_slice(&raw)?;
                copy_chunk_blocks_to_region(
                    &root,
                    cx,
                    cz,
                    &mut blocks,
                    min_x,
                    min_y,
                    min_z,
                    size_x,
                    size_y,
                    size_z,
                )?;
            }
        }
    }

    Ok(BlockRegion {
        min_x,
        min_y,
        min_z,
        size_x,
        size_y,
        size_z,
        blocks,
    })
}

fn copy_chunk_blocks_to_region(
    root: &Tag,
    cx: i32,
    cz: i32,
    blocks: &mut [i32],
    min_x: i32,
    min_y: i32,
    min_z: i32,
    size_x: usize,
    size_y: usize,
    size_z: usize,
) -> Result<()> {
    let root = compound(root).context("chunk root is not a compound")?;
    let sections = list_items(root.get("sections"));

    let chunk_min_x = cx * 16;
    let chunk_min_z = cz * 16;

    for section_tag in sections.iter().flat_map(|s| s.iter()) {
        let section = compound(section_tag).context("section is not a compound")?;
        let section_y = int_field(section, "Y").unwrap_or(0);
        let block_states = section.get("block_states");

        let global_ids = match block_states {
            Some(bs) => block_global_ids(bs)?,
            None => continue,
        };

        for local_y in 0..16_i32 {
            let world_y = section_y * 16 + local_y;
            if world_y < min_y || world_y > min_y + size_y as i32 - 1 {
                continue;
            }
            for local_z in 0..16_i32 {
                let world_z = chunk_min_z + local_z;
                if world_z < min_z || world_z > min_z + size_z as i32 - 1 {
                    continue;
                }
                for local_x in 0..16_i32 {
                    let world_x = chunk_min_x + local_x;
                    if world_x < min_x || world_x > min_x + size_x as i32 - 1 {
                        continue;
                    }

                    let idx = (local_y * 256 + local_z * 16 + local_x) as usize;
                    let block_state = global_ids.get(idx).copied().unwrap_or(AIR_BLOCK_STATE_ID);

                    let out_idx = (world_y - min_y) as usize * size_x * size_z
                        + (world_z - min_z) as usize * size_x
                        + (world_x - min_x) as usize;
                    if out_idx < blocks.len() {
                        blocks[out_idx] = block_state;
                    }
                }
            }
        }
    }

    Ok(())
}

fn block_global_ids(block_states_tag: &Tag) -> Result<Vec<i32>> {
    let container = compound(block_states_tag).context("block_states is not a compound")?;
    let palette = block_palette(container);
    let indices = unpack_indices(container.get("data"), palette.len(), BLOCK_ENTRY_COUNT)?;

    let mut global_ids = Vec::with_capacity(BLOCK_ENTRY_COUNT);
    for index in indices {
        let id = palette.get(index).copied().unwrap_or(AIR_BLOCK_STATE_ID);
        global_ids.push(id);
    }
    Ok(global_ids)
}

fn block_palette(container: &HashMap<String, Tag>) -> Vec<i32> {
    let Some(entries) = list_items(container.get("palette")) else {
        return vec![AIR_BLOCK_STATE_ID];
    };

    let palette: Vec<i32> = entries
        .iter()
        .map(|entry| match entry {
            Tag::Compound(compound) => {
                let name = string_field(compound, "Name").unwrap_or("minecraft:air");
                // Try to look up by properties, fallback to default
                let properties = string_properties(compound.get("Properties"));
                crate::registry::block_state_id(name, &properties).unwrap_or(AIR_BLOCK_STATE_ID)
            }
            _ => AIR_BLOCK_STATE_ID,
        })
        .collect();

    if palette.is_empty() {
        vec![AIR_BLOCK_STATE_ID]
    } else {
        palette
    }
}

/// Create a minimal empty chunk NBT tag.
pub fn minimal_chunk_root(chunk_x: i32, chunk_z: i32) -> Tag {
    let mut root = HashMap::new();
    root.insert("DataVersion".to_string(), Tag::Int(DATA_VERSION));
    root.insert("xPos".to_string(), Tag::Int(chunk_x));
    root.insert("yPos".to_string(), Tag::Int(WORLD_MIN_SECTION_Y));
    root.insert("zPos".to_string(), Tag::Int(chunk_z));
    root.insert(
        "sections".to_string(),
        Tag::List(
            ListHeader {
                tag_id: tag_id::COMPOUND,
                length: 0,
            },
            Arc::from([]),
        ),
    );
    root.insert("Heightmaps".to_string(), empty_heightmaps());
    root.insert(
        "block_entities".to_string(),
        Tag::List(
            ListHeader {
                tag_id: tag_id::COMPOUND,
                length: 0,
            },
            Arc::from([]),
        ),
    );
    let mut structures = HashMap::new();
    structures.insert(
        "References".to_string(),
        Tag::Compound(Arc::new(HashMap::new())),
    );
    structures.insert(
        "starts".to_string(),
        Tag::Compound(Arc::new(HashMap::new())),
    );
    root.insert(
        "structures".to_string(),
        Tag::Compound(Arc::new(structures)),
    );
    root.insert("isLightOn".to_string(), Tag::Byte(1));
    Tag::Compound(Arc::new(root))
}

/// Set block states in a region chunk, creating it from scratch if needed.
pub fn set_block_states_in_chunk(
    existing: Option<&ChunkData>,
    chunk_x: i32,
    chunk_z: i32,
    blocks: &[(i32, i32, i32, i32)], // (x, y, z, block_state_id)
) -> Result<ChunkData> {
    let root = if let Some(chunk) = existing {
        let raw = chunk.decompress().context("decompress chunk nbt")?;
        let (_, root) = qexed_nbt::from_slice(&raw).context("parse chunk nbt")?;
        root
    } else {
        minimal_chunk_root(chunk_x, chunk_z)
    };

    let mut root = compound(&root)
        .context("chunk root is not a compound")?
        .clone();
    root.insert("DataVersion".to_string(), Tag::Int(DATA_VERSION));
    root.insert("xPos".to_string(), Tag::Int(chunk_x));
    root.insert("yPos".to_string(), Tag::Int(WORLD_MIN_SECTION_Y));
    root.insert("zPos".to_string(), Tag::Int(chunk_z));

    let mut sections = root
        .get("sections")
        .and_then(|tag| list_items(Some(tag)))
        .map(|items| items.to_vec())
        .unwrap_or_default();

    let mut blocks_by_section: HashMap<i32, Vec<(i32, i32, i32, i32)>> = HashMap::new();
    for &(x, y, z, block_state) in blocks {
        blocks_by_section
            .entry(y.div_euclid(16))
            .or_default()
            .push((x, y, z, block_state));
    }

    for (section_y, section_blocks) in blocks_by_section {
        let section_index = sections.iter().position(|section| {
            compound(section).and_then(|fields| int_field(fields, "Y")) == Some(section_y)
        });

        let mut section = match section_index {
            Some(index) => compound(&sections[index])
                .with_context(|| format!("section y={section_y} is not a compound"))?
                .clone(),
            None => empty_section(section_y, AIR_BLOCK_STATE_ID),
        };

        let mut values = match section.get("block_states") {
            Some(bs) => block_global_ids(bs)?,
            None => vec![AIR_BLOCK_STATE_ID; BLOCK_ENTRY_COUNT],
        };

        for (x, y, z, block_state) in section_blocks {
            let local_x = x.rem_euclid(16) as usize;
            let local_y = y.rem_euclid(16) as usize;
            let local_z = z.rem_euclid(16) as usize;
            let index = local_y * 256 + local_z * 16 + local_x;
            values[index] = block_state;
        }

        section.insert("block_states".to_string(), block_states_tag(&values)?);
        section
            .entry("biomes".to_string())
            .or_insert_with(|| biome_states_tag());

        let section_tag = Tag::Compound(Arc::new(section));
        if let Some(index) = section_index {
            sections[index] = section_tag;
        } else {
            sections.push(section_tag);
        }
    }

    sections.sort_by_key(|section| {
        compound(section)
            .and_then(|fields| int_field(fields, "Y"))
            .unwrap_or(i32::MAX)
    });
    root.insert(
        "sections".to_string(),
        Tag::List(
            ListHeader {
                tag_id: tag_id::COMPOUND,
                length: sections.len() as i32,
            },
            Arc::from(sections),
        ),
    );
    update_heightmaps(&mut root);

    let root = Tag::Compound(Arc::new(root));
    let raw = qexed_nbt::to_vec("", &root).context("serialize chunk nbt")?;
    ChunkData::zlib(&raw).context("compress chunk nbt")
}

fn block_states_tag(block_ids: &[i32]) -> Result<Tag> {
    // Build palette from unique block_ids
    let mut palette_vec: Vec<i32> = Vec::new();
    let mut palette_map: HashMap<i32, usize> = HashMap::new();
    let mut values = vec![0i64; BLOCK_ENTRY_COUNT];

    for (i, &id) in block_ids.iter().enumerate() {
        let next = palette_vec.len();
        let idx = *palette_map.entry(id).or_insert_with(|| {
            palette_vec.push(id);
            next
        });
        values[i] = idx as i64;
    }

    let palette_tags: Vec<Tag> = palette_vec
        .iter()
        .map(|&id| {
            let entry = crate::registry::block_state_entry(id);
            let mut fields = HashMap::new();
            fields.insert("Name".to_string(), Tag::String(Arc::from(entry.name)));
            if !entry.properties.is_empty() {
                let props: HashMap<String, Tag> = entry
                    .properties
                    .iter()
                    .map(|(k, v)| (k.clone(), Tag::String(Arc::from(v.clone()))))
                    .collect();
                fields.insert("Properties".to_string(), Tag::Compound(Arc::new(props)));
            }
            Tag::Compound(Arc::new(fields))
        })
        .collect();

    let mut container = HashMap::new();
    container.insert(
        "palette".to_string(),
        Tag::List(
            ListHeader {
                tag_id: tag_id::COMPOUND,
                length: palette_tags.len() as i32,
            },
            Arc::from(palette_tags),
        ),
    );

    if palette_vec.len() > 1 {
        let bits = ceil_log2(palette_vec.len()).max(4);
        let packed = pack_long_values(&values, bits);
        container.insert("data".to_string(), Tag::LongArray(Arc::from(packed)));
    }

    Ok(Tag::Compound(Arc::new(container)))
}

fn biome_states_tag() -> Tag {
    let mut container = HashMap::new();
    container.insert(
        "palette".to_string(),
        Tag::List(
            ListHeader {
                tag_id: tag_id::STRING,
                length: 1,
            },
            Arc::from([Tag::String(Arc::from("minecraft:plains".to_string()))]),
        ),
    );
    Tag::Compound(Arc::new(container))
}

fn empty_section(section_y: i32, default_state: i32) -> HashMap<String, Tag> {
    let values = vec![default_state; BLOCK_ENTRY_COUNT];
    let mut section = HashMap::new();
    section.insert("Y".to_string(), Tag::Byte(section_y as i8));
    section.insert(
        "block_states".to_string(),
        block_states_tag(&values).unwrap_or_else(|_| {
            // Fallback: single-entry palette with the default state
            let mut container = HashMap::new();
            container.insert(
                "palette".to_string(),
                Tag::List(
                    ListHeader {
                        tag_id: tag_id::COMPOUND,
                        length: 1,
                    },
                    Arc::from([Tag::Compound(Arc::new({
                        let mut f = HashMap::new();
                        f.insert(
                            "Name".to_string(),
                            Tag::String(Arc::from("minecraft:air".to_string())),
                        );
                        f
                    }))]),
                ),
            );
            Tag::Compound(Arc::new(container))
        }),
    );
    section.insert("biomes".to_string(), biome_states_tag());
    section
}

fn empty_heightmaps() -> Tag {
    let empty = Tag::LongArray(Arc::from(vec![0i64; 37]));
    let mut hm = HashMap::new();
    hm.insert("WORLD_SURFACE".to_string(), empty.clone());
    hm.insert("MOTION_BLOCKING".to_string(), empty.clone());
    hm.insert("MOTION_BLOCKING_NO_LEAVES".to_string(), empty);
    Tag::Compound(Arc::new(hm))
}

fn update_heightmaps(root: &mut HashMap<String, Tag>) {
    // Compute simple heightmap: for each x,z column, find highest non-air block
    let mut heights = vec![0i32; 256]; // 16x16

    let sections = list_items(root.get("sections"));
    for section_tag in sections.iter().flat_map(|s| s.iter()) {
        let section = compound(section_tag);
        let Some(section) = section else {
            continue;
        };
        let section_y = int_field(section, "Y").unwrap_or(0);
        let ids = match section.get("block_states") {
            Some(bs) => block_global_ids(bs).unwrap_or_default(),
            None => continue,
        };

        for local_y in 0..16_i32 {
            let world_y = section_y * 16 + local_y;
            for z in 0..16 {
                for x in 0..16 {
                    let idx = (local_y * 256 + z * 16 + x) as usize;
                    if idx < ids.len() && ids[idx] != AIR_BLOCK_STATE_ID {
                        let col = (z * 16 + x) as usize;
                        heights[col] = heights[col].max(world_y + 1);
                    }
                }
            }
        }
    }

    let packed = pack_long_values_32(&heights, 9); // 9 bits for max height ~512
    let tag = Tag::LongArray(Arc::from(packed));
    let mut hm = HashMap::new();
    hm.insert("WORLD_SURFACE".to_string(), tag.clone());
    hm.insert("MOTION_BLOCKING".to_string(), tag.clone());
    hm.insert("MOTION_BLOCKING_NO_LEAVES".to_string(), tag);
    root.insert("Heightmaps".to_string(), Tag::Compound(Arc::new(hm)));
}

/// Fill a region with a single block state.
pub fn fill_region_chunk(
    existing: Option<&ChunkData>,
    chunk_x: i32,
    chunk_z: i32,
    min_x: i32,
    min_y: i32,
    min_z: i32,
    max_x: i32,
    max_y: i32,
    max_z: i32,
    block_state: i32,
) -> Result<ChunkData> {
    // Collect all positions in this chunk that fall within the region
    let chunk_min_x = chunk_x * 16;
    let chunk_max_x = chunk_min_x + 15;
    let chunk_min_z = chunk_z * 16;
    let chunk_max_z = chunk_min_z + 15;

    let x_start = min_x.max(chunk_min_x);
    let x_end = max_x.min(chunk_max_x);
    let z_start = min_z.max(chunk_min_z);
    let z_end = max_z.min(chunk_max_z);

    let mut blocks = Vec::new();
    for y in min_y..=max_y {
        if y < WORLD_MIN_Y || y > WORLD_MAX_Y {
            continue;
        }
        for z in z_start..=z_end {
            for x in x_start..=x_end {
                blocks.push((x, y, z, block_state));
            }
        }
    }

    set_block_states_in_chunk(existing, chunk_x, chunk_z, &blocks)
}

fn unpack_indices(
    data: Option<&Tag>,
    palette_len: usize,
    entry_count: usize,
) -> Result<Vec<usize>> {
    if palette_len <= 1 {
        return Ok(vec![0; entry_count]);
    }

    let bits = ceil_log2(palette_len).max(4);
    let values_per_long = 64 / bits;
    let mask = (1_u64 << bits) - 1;

    let longs = match data {
        Some(Tag::LongArray(arr)) => arr.as_ref(),
        _ => anyhow::bail!("missing block data array"),
    };

    let mut indices = Vec::with_capacity(entry_count);
    for i in 0..entry_count {
        let cell = i / values_per_long;
        let offset = (i % values_per_long) * bits;
        if cell < longs.len() {
            let value = ((longs[cell] as u64) >> offset) & mask;
            indices.push(value as usize);
        } else {
            indices.push(0);
        }
    }
    Ok(indices)
}

fn pack_long_values(values: &[i64], bits: usize) -> Vec<i64> {
    let values_per_long = 64 / bits;
    let mask = (1_u64 << bits) - 1;
    let mut packed = vec![0_u64; values.len().div_ceil(values_per_long)];

    for (index, value) in values.iter().enumerate() {
        let cell = index / values_per_long;
        let bit_offset = (index % values_per_long) * bits;
        packed[cell] |= ((*value as u64) & mask) << bit_offset;
    }

    packed.into_iter().map(|v| v as i64).collect()
}

fn pack_long_values_32(values: &[i32], bits: usize) -> Vec<i64> {
    let values_per_long = 64 / bits;
    let mask = (1_u64 << bits) - 1;
    let mut packed = vec![0_u64; values.len().div_ceil(values_per_long)];

    for (index, value) in values.iter().enumerate() {
        let cell = index / values_per_long;
        let bit_offset = (index % values_per_long) * bits;
        packed[cell] |= ((*value as u64) & mask) << bit_offset;
    }

    packed.into_iter().map(|v| v as i64).collect()
}

fn ceil_log2(count: usize) -> usize {
    if count <= 1 {
        0
    } else {
        usize::BITS as usize - (count - 1).leading_zeros() as usize
    }
}

// --- NBT helper functions ---

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

fn string_properties(value: Option<&Tag>) -> Vec<(String, String)> {
    let Some(properties) = value.and_then(compound) else {
        return Vec::new();
    };

    let mut props: Vec<(String, String)> = properties
        .iter()
        .filter_map(|(key, value)| match value {
            Tag::String(value) => Some((key.clone(), value.to_string())),
            _ => None,
        })
        .collect();
    props.sort_by(|left, right| left.0.cmp(&right.0));
    props
}

pub fn dimension_region_path(save_path: &std::path::Path, dimension: &str) -> std::path::PathBuf {
    match dimension {
        "minecraft:the_nether" => save_path.join("DIM-1").join("region"),
        "minecraft:the_end" => save_path.join("DIM1").join("region"),
        _ => save_path.join("region"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sparse_write_into_new_section_keeps_other_blocks_air() {
        crate::registry::load_registry(std::path::Path::new("../../assets/reports/blocks.json"))
            .expect("block registry should load");
        let chunk = set_block_states_in_chunk(None, 0, 0, &[(1, 80, 1, 6996)])
            .expect("sparse write should create a valid chunk");
        let raw = chunk.decompress().expect("chunk should decompress");
        let (_, root) = qexed_nbt::from_slice(&raw).expect("chunk NBT should parse");

        let written = read_block_state(&root, 0, 0, 1, 80, 1);
        let untouched = read_block_state(&root, 0, 0, 2, 80, 1);

        assert_eq!(written, 6996);
        assert_eq!(untouched, AIR_BLOCK_STATE_ID);
    }

    #[test]
    fn batch_write_preserves_existing_blocks_and_overwrites_same_position() {
        crate::registry::load_registry(std::path::Path::new("../../assets/reports/blocks.json"))
            .expect("block registry should load");
        let chunk = set_block_states_in_chunk(None, 0, 0, &[(1, 80, 1, 6996)])
            .expect("initial write should create chunk");
        let chunk =
            set_block_states_in_chunk(Some(&chunk), 0, 0, &[(2, 80, 1, 15), (1, 80, 1, 15044)])
                .expect("second write should update chunk");
        let raw = chunk.decompress().expect("chunk should decompress");
        let (_, root) = qexed_nbt::from_slice(&raw).expect("chunk NBT should parse");

        assert_eq!(read_block_state(&root, 0, 0, 1, 80, 1), 15044);
        assert_eq!(read_block_state(&root, 0, 0, 2, 80, 1), 15);
        assert_eq!(read_block_state(&root, 0, 0, 3, 80, 1), AIR_BLOCK_STATE_ID);
    }

    #[test]
    fn unknown_palette_entries_keep_indices_aligned() {
        crate::registry::load_registry(std::path::Path::new("../../assets/reports/blocks.json"))
            .expect("block registry should load");

        let block_states = Tag::Compound(Arc::new(HashMap::from([
            (
                "palette".to_string(),
                Tag::List(
                    ListHeader {
                        tag_id: tag_id::COMPOUND,
                        length: 2,
                    },
                    Arc::from([
                        block_state_tag("minecraft:not_a_real_block"),
                        block_state_tag("minecraft:oak_planks"),
                    ]),
                ),
            ),
            (
                "data".to_string(),
                Tag::LongArray(Arc::from(pack_long_values(&[1_i64; BLOCK_ENTRY_COUNT], 4))),
            ),
        ])));

        let ids = block_global_ids(&block_states).expect("block states should decode");
        assert_eq!(ids[0], 15);
    }

    fn read_block_state(root: &Tag, chunk_x: i32, chunk_z: i32, x: i32, y: i32, z: i32) -> i32 {
        let root = compound(root).expect("root compound");
        let section_y = y.div_euclid(16);
        let sections = list_items(root.get("sections")).expect("sections list");
        let section = sections
            .iter()
            .find_map(|section| {
                let fields = compound(section)?;
                (int_field(fields, "Y") == Some(section_y)).then_some(fields)
            })
            .expect("section exists");
        let ids = block_global_ids(section.get("block_states").expect("block states"))
            .expect("block states decode");
        let local_x = (x - chunk_x * 16).rem_euclid(16) as usize;
        let local_y = y.rem_euclid(16) as usize;
        let local_z = (z - chunk_z * 16).rem_euclid(16) as usize;
        ids[local_y * 256 + local_z * 16 + local_x]
    }

    fn block_state_tag(name: &str) -> Tag {
        Tag::Compound(Arc::new(HashMap::from([(
            "Name".to_string(),
            Tag::String(Arc::from(name.to_string())),
        )])))
    }
}

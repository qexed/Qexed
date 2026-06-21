use std::{collections::HashMap, sync::Arc};

use anyhow::{Context as _, Result};
use qexed_nbt::{ListHeader, Tag, tag_id};

use crate::{
    constants::{BLOCKS_PER_SECTION, DATA_VERSION, HEIGHTMAP_BITS, SECTION_HEIGHT},
    registry::{BlockIds, read_blocks_report},
    util::{ceil_log2, column_index, normalize_identifier},
};
pub(crate) fn chunk_root(
    chunk_x: i32,
    chunk_z: i32,
    min_y: i32,
    height: i32,
    columns: &[Vec<i32>],
    biomes: &[&str],
    heightmap: &[i32],
    ocean_floor_heightmap: &[i32],
) -> Result<Tag> {
    let mut root = HashMap::new();
    root.insert("DataVersion".to_string(), Tag::Int(DATA_VERSION));
    root.insert("xPos".to_string(), Tag::Int(chunk_x));
    root.insert("yPos".to_string(), Tag::Int(min_y / SECTION_HEIGHT));
    root.insert("zPos".to_string(), Tag::Int(chunk_z));
    root.insert("LastUpdate".to_string(), Tag::Long(0));
    root.insert("InhabitedTime".to_string(), Tag::Long(0));
    root.insert(
        "Status".to_string(),
        Tag::String(Arc::from("minecraft:full")),
    );
    root.insert(
        "sections".to_string(),
        sections_tag(min_y, height, columns, biomes)?,
    );
    root.insert(
        "Heightmaps".to_string(),
        heightmaps_tag(min_y, heightmap, ocean_floor_heightmap),
    );
    root.insert("block_entities".to_string(), empty_list_tag());
    root.insert("block_ticks".to_string(), empty_compound_list_tag());
    root.insert("fluid_ticks".to_string(), empty_compound_list_tag());
    root.insert(
        "PostProcessing".to_string(),
        empty_post_processing_tag(height / SECTION_HEIGHT),
    );
    root.insert("structures".to_string(), structures_tag());
    root.insert("isLightOn".to_string(), Tag::Byte(1));
    Ok(Tag::Compound(Arc::new(root)))
}

fn sections_tag(min_y: i32, height: i32, columns: &[Vec<i32>], biomes: &[&str]) -> Result<Tag> {
    let section_count = height / SECTION_HEIGHT;
    let mut sections = Vec::with_capacity(section_count as usize);
    for section in 0..section_count {
        let section_y = min_y / SECTION_HEIGHT + section;
        let mut blocks = Vec::with_capacity(BLOCKS_PER_SECTION);
        let section_min_y = section_y * SECTION_HEIGHT;
        for local_y in 0..16 {
            let y_index = (section_min_y + local_y - min_y) as usize;
            for local_z in 0..16 {
                for local_x in 0..16 {
                    blocks.push(columns[column_index(local_x, local_z)][y_index]);
                }
            }
        }

        let mut section_tag = HashMap::new();
        section_tag.insert("Y".to_string(), Tag::Byte(section_y as i8));
        section_tag.insert(
            "block_states".to_string(),
            paletted_block_container(&blocks)?,
        );
        let biome_start = section as usize * 4 * 4 * 4;
        let biome_end = biome_start + 4 * 4 * 4;
        section_tag.insert(
            "biomes".to_string(),
            biome_container(&biomes[biome_start..biome_end]),
        );
        sections.push(Tag::Compound(Arc::new(section_tag)));
    }
    Ok(list_tag(tag_id::COMPOUND, sections))
}

pub(crate) fn heightmaps_tag(min_y: i32, heightmap: &[i32], ocean_floor_heightmap: &[i32]) -> Tag {
    let packed = pack_heightmap(heightmap, min_y);
    let ocean_floor = pack_heightmap(ocean_floor_heightmap, min_y);
    Tag::Compound(Arc::new(HashMap::from([
        (
            "WORLD_SURFACE".to_string(),
            Tag::LongArray(Arc::from(packed.clone())),
        ),
        (
            "OCEAN_FLOOR".to_string(),
            Tag::LongArray(Arc::from(ocean_floor)),
        ),
        (
            "MOTION_BLOCKING".to_string(),
            Tag::LongArray(Arc::from(packed.clone())),
        ),
        (
            "MOTION_BLOCKING_NO_LEAVES".to_string(),
            Tag::LongArray(Arc::from(packed)),
        ),
    ])))
}

pub(crate) fn ocean_floor_height(min_y: i32, column: &[i32], ids: &BlockIds) -> i32 {
    column
        .iter()
        .rposition(|block| ids.is_ocean_floor_block(*block))
        .map(|index| min_y + index as i32 + 1)
        .unwrap_or(min_y)
}

fn pack_heightmap(heightmap: &[i32], _min_y: i32) -> Vec<i64> {
    let mask = (1_u64 << HEIGHTMAP_BITS) - 1;
    let values = heightmap
        .iter()
        .map(|height| (*height).clamp(0, mask as i32))
        .collect::<Vec<_>>();
    pack_fixed_long_values(&values, HEIGHTMAP_BITS)
        .into_iter()
        .map(|value| value as i64)
        .collect()
}

fn paletted_block_container(blocks: &[i32]) -> Result<Tag> {
    let (palette, local_values) = local_palette(blocks);
    let palette_tags = palette
        .iter()
        .map(|id| block_state_tag(*id))
        .collect::<Result<Vec<_>>>()?;
    let data = if palette.len() <= 1 {
        None
    } else {
        let bits = ceil_log2(palette.len()).max(4);
        Some(
            pack_fixed_long_values(&local_values, bits)
                .into_iter()
                .map(|value| value as i64)
                .collect::<Vec<_>>(),
        )
    };
    Ok(paletted_container(palette_tags, data))
}

fn biome_container(biomes: &[&str]) -> Tag {
    let normalized = biomes
        .iter()
        .map(|biome| normalize_identifier(biome))
        .collect::<Vec<_>>();
    let mut palette = Vec::<String>::new();
    let mut index_by_value = HashMap::new();
    let mut local_values = Vec::with_capacity(normalized.len());
    for biome in normalized {
        let next = palette.len() as i32;
        let index = *index_by_value.entry(biome.clone()).or_insert_with(|| {
            palette.push(biome);
            next
        });
        local_values.push(index);
    }
    let palette_tags = palette
        .into_iter()
        .map(|biome| Tag::String(Arc::from(biome)))
        .collect::<Vec<_>>();
    let data = if palette_tags.len() <= 1 {
        None
    } else {
        let bits = ceil_log2(palette_tags.len()).max(1);
        Some(
            pack_fixed_long_values(&local_values, bits)
                .into_iter()
                .map(|value| value as i64)
                .collect(),
        )
    };
    paletted_container(palette_tags, data)
}

fn block_state_tag(id: i32) -> Result<Tag> {
    let report = read_blocks_report()?;
    let blocks = report
        .as_object()
        .context("blocks report root is not object")?;
    for (name, block) in blocks {
        let Some(states) = block.get("states").and_then(serde_json::Value::as_array) else {
            continue;
        };
        for state in states {
            if state.get("id").and_then(serde_json::Value::as_i64) == Some(i64::from(id)) {
                let mut fields = HashMap::new();
                fields.insert("Name".to_string(), Tag::String(Arc::from(name.clone())));
                if let Some(properties) = state
                    .get("properties")
                    .and_then(serde_json::Value::as_object)
                {
                    fields.insert(
                        "Properties".to_string(),
                        Tag::Compound(Arc::new(
                            properties
                                .iter()
                                .filter_map(|(key, value)| {
                                    value.as_str().map(|value| {
                                        (key.clone(), Tag::String(Arc::from(value.to_string())))
                                    })
                                })
                                .collect(),
                        )),
                    );
                }
                return Ok(Tag::Compound(Arc::new(fields)));
            }
        }
    }
    anyhow::bail!("unknown block state id in Mojang cache: {id}")
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

fn structures_tag() -> Tag {
    Tag::Compound(Arc::new(HashMap::from([
        (
            "starts".to_string(),
            Tag::Compound(Arc::new(HashMap::new())),
        ),
        (
            "References".to_string(),
            Tag::Compound(Arc::new(HashMap::new())),
        ),
    ])))
}

fn local_palette(values: &[i32]) -> (Vec<i32>, Vec<i32>) {
    let mut palette = Vec::new();
    let mut index_by_value = HashMap::new();
    let mut local_values = Vec::with_capacity(values.len());
    for value in values {
        let next = palette.len() as i32;
        let index = *index_by_value.entry(*value).or_insert_with(|| {
            palette.push(*value);
            next
        });
        local_values.push(index);
    }
    (palette, local_values)
}

fn pack_fixed_long_values(values: &[i32], bits: usize) -> Vec<u64> {
    let values_per_long = 64 / bits;
    let mut packed = vec![0_u64; values.len().div_ceil(values_per_long)];
    let mask = (1_u64 << bits) - 1;
    for (index, value) in values.iter().enumerate() {
        let value = (*value as u64) & mask;
        let cell = index / values_per_long;
        let offset = (index % values_per_long) * bits;
        packed[cell] |= value << offset;
    }
    packed
}

pub(crate) fn list_tag(item_tag_id: u8, items: Vec<Tag>) -> Tag {
    Tag::List(
        ListHeader {
            tag_id: item_tag_id,
            length: items.len() as i32,
        },
        Arc::from(items),
    )
}

fn empty_compound_list_tag() -> Tag {
    list_tag(tag_id::COMPOUND, Vec::new())
}

fn empty_list_tag() -> Tag {
    list_tag(tag_id::END, Vec::new())
}

fn empty_post_processing_tag(section_count: i32) -> Tag {
    let sections = (0..section_count.max(0))
        .map(|_| list_tag(tag_id::END, Vec::new()))
        .collect();
    list_tag(tag_id::LIST, sections)
}

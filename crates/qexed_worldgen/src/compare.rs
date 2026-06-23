use std::collections::HashMap;

use qexed_nbt::Tag;

use crate::types::ChunkComparison;
pub fn compare_chunk_nbt(expected: &Tag, actual: &Tag) -> ChunkComparison {
    if expected == actual {
        return ChunkComparison {
            equal: true,
            differences: Vec::new(),
        };
    }

    let mut differences = Vec::new();
    compare_chunk_blocks(expected, actual, &mut differences);
    compare_tag("$", expected, actual, &mut differences);
    ChunkComparison {
        equal: differences.is_empty(),
        differences,
    }
}

fn compare_chunk_blocks(expected: &Tag, actual: &Tag, differences: &mut Vec<String>) {
    let Some(expected_root) = compound(expected) else {
        return;
    };
    let Some(actual_root) = compound(actual) else {
        return;
    };
    let expected_sections = sections_by_y(expected_root);
    let actual_sections = sections_by_y(actual_root);

    let mut section_ys = expected_sections.keys().copied().collect::<Vec<_>>();
    section_ys.sort_unstable();

    for section_y in section_ys {
        let Some(expected_section) = expected_sections.get(&section_y) else {
            continue;
        };
        let Some(actual_section) = actual_sections.get(&section_y) else {
            continue;
        };
        let Ok(expected_blocks) = section_block_indices(expected_section) else {
            continue;
        };
        let Ok(actual_blocks) = section_block_indices(actual_section) else {
            continue;
        };
        for index in 0..expected_blocks.len().min(actual_blocks.len()) {
            if expected_blocks[index] == actual_blocks[index] {
                continue;
            }
            let local_x = index % 16;
            let local_z = (index / 16) % 16;
            let local_y = index / 256;
            let world_y = section_y * 16 + local_y as i32;
            let expected_name = block_name_at_index(expected_section, expected_blocks[index]);
            let actual_name = block_name_at_index(actual_section, actual_blocks[index]);
            differences.push(format!(
                "$.blocks[{local_x},{world_y},{local_z}]: expected={} actual={}",
                expected_name, actual_name
            ));
            if differences.len() >= 16 {
                return;
            }
        }
    }
}

fn sections_by_y(root: &HashMap<String, Tag>) -> HashMap<i32, &HashMap<String, Tag>> {
    list_items(root.get("sections"))
        .unwrap_or_default()
        .iter()
        .filter_map(|section| {
            let section = compound(section)?;
            let y = int_field(section, "Y")?;
            Some((y, section))
        })
        .collect()
}

fn section_blocks(section: &HashMap<String, Tag>) -> anyhow::Result<Vec<String>> {
    let container = section
        .get("block_states")
        .and_then(compound)
        .ok_or_else(|| anyhow::anyhow!("missing block_states"))?;
    let palette = block_palette(container);
    let values = paletted_values(container.get("data"), palette.len(), 4096, true)?;
    Ok(values
        .into_iter()
        .map(|index| {
            palette
                .get(index)
                .cloned()
                .unwrap_or_else(|| format!("<palette:{index}>"))
        })
        .collect())
}

fn section_block_indices(section: &HashMap<String, Tag>) -> anyhow::Result<Vec<usize>> {
    let container = section
        .get("block_states")
        .and_then(compound)
        .ok_or_else(|| anyhow::anyhow!("missing block_states"))?;
    let palette_len = list_items(container.get("palette"))
        .unwrap_or_default()
        .len();
    paletted_values(container.get("data"), palette_len, 4096, true)
}

fn block_name_at_index(section: &HashMap<String, Tag>, index: usize) -> String {
    section
        .get("block_states")
        .and_then(compound)
        .and_then(|container| list_items(container.get("palette")))
        .and_then(|palette| palette.get(index))
        .and_then(compound)
        .map(block_palette_name)
        .unwrap_or_else(|| format!("<palette:{index}>"))
}

fn block_palette(container: &HashMap<String, Tag>) -> Vec<String> {
    list_items(container.get("palette"))
        .unwrap_or_default()
        .iter()
        .map(|entry| match compound(entry) {
            Some(entry) => block_palette_name(entry),
            None => "<invalid>".to_string(),
        })
        .collect()
}

fn block_palette_name(entry: &HashMap<String, Tag>) -> String {
    let mut name = string_field(entry, "Name")
        .unwrap_or("minecraft:air")
        .to_string();
    let mut properties = string_properties(entry.get("Properties"));
    if !properties.is_empty() {
        name.push('[');
        for (index, (key, value)) in properties.drain(..).enumerate() {
            if index > 0 {
                name.push(',');
            }
            name.push_str(&key);
            name.push('=');
            name.push_str(&value);
        }
        name.push(']');
    }
    name
}

fn paletted_values(
    data: Option<&Tag>,
    palette_len: usize,
    len: usize,
    block_palette: bool,
) -> anyhow::Result<Vec<usize>> {
    if palette_len <= 1 {
        return Ok(vec![0; len]);
    }

    let bits = if block_palette {
        ceil_log2(palette_len).max(4)
    } else {
        ceil_log2(palette_len).max(1)
    };
    let values_per_long = 64 / bits;
    let required_len = len.div_ceil(values_per_long);
    let data = long_array(data).ok_or_else(|| anyhow::anyhow!("missing paletted data"))?;
    if data.len() < required_len {
        anyhow::bail!("paletted data too short");
    }

    let mask = (1_u64 << bits) - 1;
    let mut values = Vec::with_capacity(len);
    for index in 0..len {
        let cell = index / values_per_long;
        let bit_offset = (index % values_per_long) * bits;
        values.push((((data[cell] as u64) >> bit_offset) & mask) as usize);
    }
    Ok(values)
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

fn long_array(tag: Option<&Tag>) -> Option<&[i64]> {
    match tag {
        Some(Tag::LongArray(values)) => Some(values),
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

fn ceil_log2(value: usize) -> usize {
    if value <= 1 {
        0
    } else {
        usize::BITS as usize - (value - 1).leading_zeros() as usize
    }
}

fn compare_tag(path: &str, expected: &Tag, actual: &Tag, differences: &mut Vec<String>) {
    const MAX_DIFFERENCES: usize = 64;
    if differences.len() >= MAX_DIFFERENCES {
        return;
    }
    if empty_list_shape_is_equivalent(path, expected, actual) {
        return;
    }
    if compare_tick_list_shape(path, expected, actual, differences) {
        return;
    }

    match (expected, actual) {
        (Tag::Compound(expected), Tag::Compound(actual)) => {
            for (key, expected_value) in expected.iter() {
                let next_path = format!("{path}.{key}");
                if is_volatile_runtime_field(&next_path) {
                    continue;
                }
                match actual.get(key) {
                    Some(actual_value) => {
                        compare_tag(&next_path, expected_value, actual_value, differences)
                    }
                    None => differences.push(format!("{next_path}: missing in actual")),
                }
                if differences.len() >= MAX_DIFFERENCES {
                    return;
                }
            }
            for key in actual.keys() {
                if !expected.contains_key(key) {
                    differences.push(format!("{path}.{key}: unexpected in actual"));
                    if differences.len() >= MAX_DIFFERENCES {
                        return;
                    }
                }
            }
        }
        (Tag::List(expected_header, expected_items), Tag::List(actual_header, actual_items)) => {
            if expected_header.tag_id != actual_header.tag_id {
                differences.push(format!(
                    "{path}: list tag id differs expected={} actual={}",
                    expected_header.tag_id, actual_header.tag_id
                ));
                return;
            }
            if expected_items.len() != actual_items.len() {
                differences.push(format!(
                    "{path}: list length differs expected={} actual={}",
                    expected_items.len(),
                    actual_items.len()
                ));
                return;
            }
            for (index, (expected_item, actual_item)) in
                expected_items.iter().zip(actual_items.iter()).enumerate()
            {
                compare_tag(
                    &format!("{path}[{index}]"),
                    expected_item,
                    actual_item,
                    differences,
                );
                if differences.len() >= MAX_DIFFERENCES {
                    return;
                }
            }
        }
        (Tag::ByteArray(expected), Tag::ByteArray(actual)) => {
            compare_byte_array(path, expected, actual, differences);
        }
        (Tag::LongArray(expected), Tag::LongArray(actual)) => {
            compare_long_array(path, expected, actual, differences);
        }
        _ if expected == actual => {}
        _ => differences.push(format!(
            "{path}: value differs expected={} actual={}",
            tag_summary(expected),
            tag_summary(actual)
        )),
    }
}

fn empty_list_shape_is_equivalent(path: &str, expected: &Tag, actual: &Tag) -> bool {
    if path != "$.block_entities" && path != "$.PostProcessing" {
        return false;
    }
    let (Tag::List(_, expected_items), Tag::List(_, actual_items)) = (expected, actual) else {
        return false;
    };
    list_is_effectively_empty(expected_items) && list_is_effectively_empty(actual_items)
}

fn list_is_effectively_empty(items: &[Tag]) -> bool {
    items.iter().all(|item| match item {
        Tag::List(_, nested) => list_is_effectively_empty(nested),
        _ => false,
    })
}

fn compare_tick_list_shape(
    path: &str,
    expected: &Tag,
    actual: &Tag,
    differences: &mut Vec<String>,
) -> bool {
    if path != "$.block_ticks" && path != "$.fluid_ticks" {
        return false;
    }
    let (Tag::List(expected_header, _), Tag::List(actual_header, _)) = (expected, actual) else {
        return false;
    };
    if expected_header.tag_id != actual_header.tag_id {
        differences.push(format!(
            "{path}: list tag id differs expected={} actual={}",
            expected_header.tag_id, actual_header.tag_id
        ));
    }
    true
}

fn compare_byte_array(path: &str, expected: &[i8], actual: &[i8], differences: &mut Vec<String>) {
    if expected.len() != actual.len() {
        differences.push(format!(
            "{path}: byte array length differs expected={} actual={}",
            expected.len(),
            actual.len()
        ));
        return;
    }
    if is_light_array(path) {
        return;
    }
    if let Some(index) = expected
        .iter()
        .zip(actual.iter())
        .position(|(expected, actual)| expected != actual)
    {
        differences.push(format!(
            "{path}[{index}]{}: byte differs expected={} actual={}",
            light_nibble_hint(path, index),
            expected[index],
            actual[index]
        ));
    }
}

fn compare_long_array(path: &str, expected: &[i64], actual: &[i64], differences: &mut Vec<String>) {
    if expected.len() != actual.len() {
        differences.push(format!(
            "{path}: long array length differs expected={} actual={}",
            expected.len(),
            actual.len()
        ));
        return;
    }
    if path.starts_with("$.Heightmaps.") {
        return;
    }
    if let Some(index) = expected
        .iter()
        .zip(actual.iter())
        .position(|(expected, actual)| expected != actual)
    {
        differences.push(format!(
            "{path}[{index}]: long differs expected={} actual={}",
            expected[index], actual[index]
        ));
    }
}

fn is_volatile_runtime_field(path: &str) -> bool {
    path == "$.LastUpdate"
}

fn light_nibble_hint(path: &str, index: usize) -> String {
    if !is_light_array(path) {
        return String::new();
    }
    let first_block = index * 2;
    let x0 = first_block % 16;
    let z0 = (first_block / 16) % 16;
    let y0 = first_block / 256;
    let x1 = (first_block + 1) % 16;
    let z1 = ((first_block + 1) / 16) % 16;
    let y1 = (first_block + 1) / 256;
    format!(" blocks=({x0},{y0},{z0})/({x1},{y1},{z1})")
}

fn is_light_array(path: &str) -> bool {
    path.ends_with(".SkyLight") || path.ends_with(".BlockLight")
}

fn tag_summary(tag: &Tag) -> String {
    match tag {
        Tag::Byte(value) => format!("byte({value})"),
        Tag::Short(value) => format!("short({value})"),
        Tag::Int(value) => format!("int({value})"),
        Tag::Long(value) => format!("long({value})"),
        Tag::Float(value) => format!("float({value})"),
        Tag::Double(value) => format!("double({value})"),
        Tag::String(value) => format!("string({value})"),
        Tag::ByteArray(value) => format!("byte_array(len={})", value.len()),
        Tag::IntArray(value) => format!("int_array(len={})", value.len()),
        Tag::LongArray(value) => format!("long_array(len={})", value.len()),
        Tag::List(_, value) => format!("list(len={})", value.len()),
        Tag::Compound(value) => format!("compound(len={})", value.len()),
        Tag::End => "end".to_string(),
    }
}

#[cfg(test)]
mod oracle_diagnostics {
    use std::{
        collections::{BTreeMap, BTreeSet, HashMap},
        fs::File,
        io::{Read, Seek, SeekFrom},
        path::{Path, PathBuf},
        time::{Duration, Instant},
    };

    use flate2::read::{GzDecoder, ZlibDecoder};
    use qexed_nbt::Tag;

    use super::*;

    #[derive(Debug, Clone)]
    struct BlockAt {
        x: i32,
        y: i32,
        z: i32,
        name: String,
    }

    #[test]
    #[ignore = "manual oracle extraction diagnostic"]
    fn dump_first_oracle_chunk_diff() {
        let Some(case) = first_oracle_diff().expect("scan oracle chunks") else {
            println!("no oracle block diff found");
            return;
        };
        let expected = case.expected;
        let actual = case.actual;

        println!("== NBT structural comparison ==");
        let comparison = compare_chunk_nbt(&expected, &actual);
        println!("equal={}", comparison.equal);
        for difference in comparison.differences.iter().take(64) {
            println!("{difference}");
        }

        println!("== gravel/ore_gravel related block distribution ==");
        print_gravel_summary("oracle", &expected);
        print_gravel_summary("rust", &actual);
        print_gravel_delta(&expected, &actual);

        let first_diff = first_block_diff(&expected, &actual);
        println!("== first block diff ==");
        match first_diff {
            Some((x, y, z, expected_name, actual_name)) => {
                println!("first_diff=({x},{y},{z}) oracle={expected_name} rust={actual_name}");
                print_block_window(&expected, &actual, y - 4, y + 4);
            }
            None => println!("no block diff found"),
        }

        println!("== biome comparison ==");
        print_biome_summary("oracle", &expected);
        print_biome_summary("rust", &actual);
        print_biome_diffs(&expected, &actual);
    }

    #[test]
    #[ignore = "manual oracle cache diagnostic"]
    fn dump_cached_seed_zero_chunk_diff() {
        let seed = 0;
        let chunk_x = 0;
        let chunk_z = 0;
        let java_path = oracle_chunk_cache_region_path(seed, chunk_x, chunk_z);
        let rust_path = rust_chunk_cache_region_path(seed, chunk_x, chunk_z);
        let expected =
            read_cached_region_chunk(&java_path, chunk_x, chunk_z).expect("read cached Java chunk");
        let actual =
            read_cached_region_chunk(&rust_path, chunk_x, chunk_z).expect("read cached Rust chunk");

        println!("module=qexed_worldgen::compare::oracle_diagnostics");
        println!("java_cache={}", java_path.display());
        println!("rust_cache={}", rust_path.display());

        let comparison = compare_chunk_nbt(&expected, &actual);
        println!("equal={}", comparison.equal);
        if let Some(difference) = comparison.differences.first() {
            println!("first_nbt_diff={difference}");
        }

        match first_block_diff(&expected, &actual) {
            Some((x, y, z, expected_name, actual_name)) => {
                println!("first_diff=({x},{y},{z}) expected={expected_name} actual={actual_name}");
            }
            None => println!("no block diff found"),
        }
    }

    #[test]
    fn cached_seed_zero_oracle_metadata_and_target_mapping_diagnostic() {
        let seed = 0;
        let chunk_x = 0;
        let chunk_z = 0;
        let chunk_dir = oracle_chunk_cache_dir(seed, chunk_x, chunk_z);
        let region_path = oracle_chunk_cache_region_path(seed, chunk_x, chunk_z);
        if !region_path.exists() {
            println!("oracle cache absent: {}", region_path.display());
            return;
        }

        let manifest =
            read_manifest(&chunk_dir.join("manifest.txt")).expect("read oracle manifest");
        assert_eq!(
            manifest.get("minecraft_version").map(String::as_str),
            Some(qexed_config::MC_VERSION)
        );
        assert_eq!(manifest.get("seed").map(String::as_str), Some("0"));
        assert_eq!(manifest.get("chunk_x").map(String::as_str), Some("0"));
        assert_eq!(manifest.get("chunk_z").map(String::as_str), Some("0"));
        assert_eq!(
            manifest.get("dimension_namespace").map(String::as_str),
            Some("minecraft")
        );
        assert_eq!(
            manifest.get("dimension_value").map(String::as_str),
            Some("overworld")
        );
        assert!(
            manifest
                .get("generation_config")
                .is_some_and(|value| value.contains("minecraft-server;overworld"))
        );

        let noise_settings_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../cache/mojang")
            .join(qexed_config::MC_VERSION)
            .join("data/minecraft/worldgen/noise_settings/overworld.json");
        assert!(
            noise_settings_path.exists(),
            "missing noise settings cache: {}",
            noise_settings_path.display()
        );

        let chunk = read_cached_region_chunk(&region_path, chunk_x, chunk_z)
            .expect("read cached Java chunk");
        let x: i32 = 6;
        let y: i32 = -61;
        let z: i32 = 0;
        let section_y = y.div_euclid(16);
        let local_y = y.rem_euclid(16) as usize;
        let block_index = local_y * 256 + z as usize * 16 + x as usize;
        let sections = sections_by_y(compound(&chunk).expect("oracle root"));
        let section = sections.get(&section_y).expect("target section");
        let indices = section_block_indices(section).expect("decode target section palette");
        let palette_index = indices[block_index];
        let block = block_name_at_index(section, palette_index);

        assert_eq!(section_y, -4);
        assert_eq!(local_y, 3);
        assert_eq!(block_index, 774);
        assert_eq!(block, "minecraft:deepslate[axis=y]");
        assert_eq!(
            block_at(&chunk, x, y, z).as_deref(),
            Some("minecraft:deepslate[axis=y]")
        );
    }

    #[test]
    #[ignore = "manual oracle cache diagnostic"]
    fn dump_oracle_cache_entry_delta() {
        let seed = 0;
        let chunk_x = 0;
        let chunk_z = 0;
        let legacy_path = oracle_root()
            .join(format!("seed-{seed}"))
            .join("minecraft/overworld/region/r.0.0.mca");
        let chunk_path = oracle_root()
            .join(format!("seed-{seed}"))
            .join("minecraft/overworld/chunks/x.0.z.0/r.0.0.mca");
        let legacy = read_region_chunk(&legacy_path, chunk_x, chunk_z).expect("read legacy cache");
        let chunk = read_region_chunk(&chunk_path, chunk_x, chunk_z).expect("read chunk cache");

        println!("legacy_path={}", legacy_path.display());
        println!("chunk_path={}", chunk_path.display());
        for &(x, y, z) in &[(1, -30, 0), (8, 64, 9), (4, 64, 12)] {
            println!(
                "block=({x},{y},{z}) legacy={:?} chunk={:?}",
                block_at(&legacy, x, y, z),
                block_at(&chunk, x, y, z)
            );
        }

        let comparison = compare_chunk_nbt(&legacy, &chunk);
        println!("equal={}", comparison.equal);
        for difference in comparison.differences.iter().take(32) {
            println!("{difference}");
        }
    }

    #[test]
    #[ignore = "manual oracle compare perf diagnostic"]
    fn dump_oracle_compare_perf() {
        let mut read_total = Duration::ZERO;
        let mut generate_total = Duration::ZERO;
        let mut compare_total = Duration::ZERO;
        let mut compared = 0_usize;
        let mut unequal = 0_usize;

        for seed in available_oracle_seeds().expect("list oracle seeds") {
            for &(chunk_x, chunk_z) in ORACLE_SCAN_CHUNKS {
                let region_path = oracle_region_path(seed, chunk_x, chunk_z);
                if !region_path.exists() {
                    continue;
                }

                let read_start = Instant::now();
                let expected =
                    read_region_chunk(&region_path, chunk_x, chunk_z).expect("read oracle chunk");
                read_total += read_start.elapsed();

                let generate_start = Instant::now();
                let actual =
                    crate::generator_v4::generate_overworld_chunk_nbt(seed, chunk_x, chunk_z)
                        .expect("generate local chunk");
                generate_total += generate_start.elapsed();

                let compare_start = Instant::now();
                let comparison = compare_chunk_nbt(&expected, &actual);
                compare_total += compare_start.elapsed();

                compared += 1;
                unequal += usize::from(!comparison.equal);
            }
        }

        if compared == 0 {
            println!("oracle compare perf: no oracle chunks found");
            return;
        }

        let count = compared as f64;
        println!(
            "oracle compare perf: chunks={compared}, unequal={unequal}, avg_read_ms={:.2}, avg_generate_ms={:.2}, avg_compare_ms={:.2}, total_compare_ms={:.2}",
            duration_ms(read_total) / count,
            duration_ms(generate_total) / count,
            duration_ms(compare_total) / count,
            duration_ms(compare_total),
        );
    }

    fn first_oracle_diff() -> anyhow::Result<Option<OracleDiffCase>> {
        for seed in available_oracle_seeds()? {
            for &(chunk_x, chunk_z) in ORACLE_SCAN_CHUNKS {
                let region_path = oracle_region_path(seed, chunk_x, chunk_z);
                if !region_path.exists() {
                    continue;
                }
                let expected = read_region_chunk(&region_path, chunk_x, chunk_z)?;
                let actual =
                    crate::generator_v4::generate_overworld_chunk_nbt(seed, chunk_x, chunk_z)?;
                if let Some((x, y, z, expected_name, actual_name)) =
                    first_block_diff(&expected, &actual)
                {
                    println!(
                        "first_oracle_diff seed={seed} chunk=({chunk_x},{chunk_z}) local=({x},{y},{z}) oracle={expected_name} rust={actual_name}"
                    );
                    return Ok(Some(OracleDiffCase { expected, actual }));
                }
            }
        }
        Ok(None)
    }

    struct OracleDiffCase {
        expected: Tag,
        actual: Tag,
    }

    const ORACLE_SCAN_CHUNKS: &[(i32, i32)] = &[
        (0, 0),
        (1, 0),
        (0, 1),
        (1, 1),
        (2, 0),
        (0, 2),
        (3, 3),
        (8, 8),
        (15, 15),
        (16, 0),
        (0, 16),
        (31, 31),
    ];

    fn read_region_chunk(path: &Path, chunk_x: i32, chunk_z: i32) -> anyhow::Result<Tag> {
        let local_x = chunk_x.rem_euclid(32) as usize;
        let local_z = chunk_z.rem_euclid(32) as usize;
        let index = local_x + local_z * 32;

        let mut file = File::open(path)?;
        let mut header = [0_u8; 8192];
        file.read_exact(&mut header)?;
        let entry = &header[index * 4..index * 4 + 4];
        let sector_offset =
            (u32::from(entry[0]) << 16) | (u32::from(entry[1]) << 8) | u32::from(entry[2]);
        let sector_count = entry[3] as usize;
        anyhow::ensure!(
            sector_offset != 0 && sector_count != 0,
            "chunk is not present"
        );

        file.seek(SeekFrom::Start(u64::from(sector_offset) * 4096))?;
        let mut length_buf = [0_u8; 4];
        file.read_exact(&mut length_buf)?;
        let length = u32::from_be_bytes(length_buf) as usize;
        anyhow::ensure!(length > 1, "invalid chunk length");
        anyhow::ensure!(
            length <= sector_count * 4096 - 4,
            "chunk length exceeds sector allocation"
        );

        let mut compression = [0_u8; 1];
        file.read_exact(&mut compression)?;
        let mut payload = vec![0_u8; length - 1];
        file.read_exact(&mut payload)?;

        let mut nbt = Vec::new();
        match compression[0] {
            1 => {
                GzDecoder::new(payload.as_slice()).read_to_end(&mut nbt)?;
            }
            2 => {
                ZlibDecoder::new(payload.as_slice()).read_to_end(&mut nbt)?;
            }
            3 => {
                nbt = payload;
            }
            other => anyhow::bail!("unsupported Anvil compression type {other}"),
        };

        let (_, tag) = qexed_nbt::from_slice(&nbt)?;
        Ok(tag)
    }

    fn read_cached_region_chunk(path: &Path, chunk_x: i32, chunk_z: i32) -> anyhow::Result<Tag> {
        let region = qexed_world::region::AnvilRegion::from_file(path)?;
        let chunk = region
            .read_chunk(chunk_x, chunk_z)?
            .ok_or_else(|| anyhow::anyhow!("chunk is not present"))?;
        let raw = chunk.decompress()?;
        let (_, tag) = qexed_nbt::from_slice(&raw)?;
        Ok(tag)
    }

    fn available_oracle_seeds() -> anyhow::Result<Vec<i64>> {
        let root = oracle_root();
        let mut seeds = if root.exists() {
            std::fs::read_dir(root)?
                .filter_map(|entry| {
                    let name = entry.ok()?.file_name().into_string().ok()?;
                    name.strip_prefix("seed-")?.parse().ok()
                })
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        seeds.sort_unstable();
        Ok(seeds)
    }

    fn oracle_region_path(seed: i64, chunk_x: i32, chunk_z: i32) -> std::path::PathBuf {
        let region_x = chunk_x.div_euclid(32);
        let region_z = chunk_z.div_euclid(32);
        let chunk_cache_path = oracle_chunk_cache_region_path(seed, chunk_x, chunk_z);
        if chunk_cache_path.exists() {
            return chunk_cache_path;
        }

        oracle_root()
            .join(format!("seed-{seed}"))
            .join("minecraft/overworld/region")
            .join(format!("r.{region_x}.{region_z}.mca"))
    }

    fn oracle_chunk_cache_region_path(seed: i64, chunk_x: i32, chunk_z: i32) -> std::path::PathBuf {
        let region_x = chunk_x.div_euclid(32);
        let region_z = chunk_z.div_euclid(32);
        oracle_chunk_cache_dir(seed, chunk_x, chunk_z).join(format!("r.{region_x}.{region_z}.mca"))
    }

    fn oracle_chunk_cache_dir(seed: i64, chunk_x: i32, chunk_z: i32) -> PathBuf {
        oracle_root()
            .join(format!("seed-{seed}"))
            .join("minecraft/overworld/chunks")
            .join(format!("x.{chunk_x}.z.{chunk_z}"))
    }

    fn rust_chunk_cache_region_path(seed: i64, chunk_x: i32, chunk_z: i32) -> std::path::PathBuf {
        let region_x = chunk_x.div_euclid(32);
        let region_z = chunk_z.div_euclid(32);
        oracle_root()
            .join(format!("seed-{seed}"))
            .join("minecraft/overworld/rust/chunks")
            .join(format!("x.{chunk_x}.z.{chunk_z}"))
            .join(format!("r.{region_x}.{region_z}.mca"))
    }

    fn oracle_root() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/worldgen-oracle")
            .join(qexed_config::MC_VERSION)
    }

    fn read_manifest(path: &Path) -> anyhow::Result<HashMap<String, String>> {
        let content = std::fs::read_to_string(path)?;
        Ok(content
            .lines()
            .filter_map(|line| {
                let (key, value) = line.split_once('=')?;
                Some((key.to_string(), value.to_string()))
            })
            .collect())
    }

    fn duration_ms(duration: Duration) -> f64 {
        duration.as_secs_f64() * 1000.0
    }

    fn print_gravel_summary(label: &str, root: &Tag) {
        let blocks = collect_matching_blocks(root, |name| name.contains("gravel"))
            .expect("collect gravel blocks");
        let mut by_name: BTreeMap<String, Vec<&BlockAt>> = BTreeMap::new();
        for block in &blocks {
            by_name.entry(block.name.clone()).or_default().push(block);
        }

        println!("{label}: total_gravel_like={}", blocks.len());
        for (name, entries) in by_name {
            let min_y = entries
                .iter()
                .map(|block| block.y)
                .min()
                .unwrap_or_default();
            let max_y = entries
                .iter()
                .map(|block| block.y)
                .max()
                .unwrap_or_default();
            let sample = entries
                .iter()
                .take(24)
                .map(|block| format!("({},{},{})", block.x, block.y, block.z))
                .collect::<Vec<_>>()
                .join(" ");
            println!(
                "{label}: {name} count={} y_range={}..{} sample={sample}",
                entries.len(),
                min_y,
                max_y
            );
        }
    }

    fn print_gravel_delta(expected: &Tag, actual: &Tag) {
        let expected = collect_matching_blocks(expected, |name| name.contains("gravel"))
            .expect("collect oracle gravel");
        let actual = collect_matching_blocks(actual, |name| name.contains("gravel"))
            .expect("collect rust gravel");
        let expected_set = coord_set(&expected);
        let actual_set = coord_set(&actual);
        print_coord_delta(
            "oracle_only_gravel_like",
            expected_set.difference(&actual_set),
        );
        print_coord_delta(
            "rust_only_gravel_like",
            actual_set.difference(&expected_set),
        );
    }

    fn print_coord_delta<'a>(label: &str, coords: impl Iterator<Item = &'a (i32, i32, i32)>) {
        let coords = coords.copied().collect::<Vec<_>>();
        let sample = coords
            .iter()
            .take(48)
            .map(|(x, y, z)| format!("({x},{y},{z})"))
            .collect::<Vec<_>>()
            .join(" ");
        println!("{label}: count={} sample={sample}", coords.len());
    }

    fn collect_matching_blocks(
        root: &Tag,
        matches: impl Fn(&str) -> bool,
    ) -> anyhow::Result<Vec<BlockAt>> {
        let mut blocks = Vec::new();
        for (section_y, section) in sorted_sections(root) {
            let section_blocks = section_blocks(section)?;
            for (index, name) in section_blocks.into_iter().enumerate() {
                if !matches(&name) {
                    continue;
                }
                let local_x = (index % 16) as i32;
                let local_z = ((index / 16) % 16) as i32;
                let local_y = (index / 256) as i32;
                blocks.push(BlockAt {
                    x: local_x,
                    y: section_y * 16 + local_y,
                    z: local_z,
                    name,
                });
            }
        }
        Ok(blocks)
    }

    fn coord_set(blocks: &[BlockAt]) -> BTreeSet<(i32, i32, i32)> {
        blocks
            .iter()
            .map(|block| (block.x, block.y, block.z))
            .collect()
    }

    fn first_block_diff(expected: &Tag, actual: &Tag) -> Option<(i32, i32, i32, String, String)> {
        let expected_sections = sorted_sections(expected);
        let actual_sections = sections_by_y(compound(actual)?);

        for (section_y, expected_section) in expected_sections {
            let actual_section = actual_sections.get(&section_y)?;
            let expected_blocks = section_blocks(expected_section).ok()?;
            let actual_blocks = section_blocks(actual_section).ok()?;
            for index in 0..expected_blocks.len().min(actual_blocks.len()) {
                if expected_blocks[index] == actual_blocks[index] {
                    continue;
                }
                let x = (index % 16) as i32;
                let z = ((index / 16) % 16) as i32;
                let y = section_y * 16 + (index / 256) as i32;
                return Some((
                    x,
                    y,
                    z,
                    expected_blocks[index].clone(),
                    actual_blocks[index].clone(),
                ));
            }
        }
        None
    }

    fn print_block_window(expected: &Tag, actual: &Tag, min_y: i32, max_y: i32) {
        println!("block_window_y={min_y}..{max_y} format: x,z:oracle|rust");
        for y in min_y..=max_y {
            println!("-- y={y} --");
            for z in 0..16 {
                let row = (0..16)
                    .filter_map(|x| {
                        let expected_name = block_at(expected, x, y, z)?;
                        let actual_name = block_at(actual, x, y, z)?;
                        if expected_name == actual_name {
                            None
                        } else {
                            Some(format!(
                                "{x},{z}:{}|{}",
                                short_block_name(&expected_name),
                                short_block_name(&actual_name)
                            ))
                        }
                    })
                    .collect::<Vec<_>>();
                if !row.is_empty() {
                    println!("{}", row.join(" "));
                }
            }
        }
    }

    fn block_at(root: &Tag, x: i32, y: i32, z: i32) -> Option<String> {
        let section_y = y.div_euclid(16);
        let local_y = y.rem_euclid(16) as usize;
        let sections = sections_by_y(compound(root)?);
        let blocks = section_blocks(sections.get(&section_y)?).ok()?;
        let index = local_y * 256 + z as usize * 16 + x as usize;
        blocks.get(index).cloned()
    }

    fn print_biome_summary(label: &str, root: &Tag) {
        let mut counts: BTreeMap<String, usize> = BTreeMap::new();
        for (_, section) in sorted_sections(root) {
            for biome in section_biomes(section).unwrap_or_default() {
                *counts.entry(biome).or_default() += 1;
            }
        }
        println!("{label}: biome_cell_counts={counts:?}");
    }

    fn print_biome_diffs(expected: &Tag, actual: &Tag) {
        let expected_sections = sorted_sections(expected);
        let actual_sections = sections_by_y(compound(actual).expect("actual root"));
        let mut diff_count = 0_usize;
        let mut samples = Vec::new();
        for (section_y, expected_section) in expected_sections {
            let Some(actual_section) = actual_sections.get(&section_y) else {
                continue;
            };
            let expected_biomes = section_biomes(expected_section).unwrap_or_default();
            let actual_biomes = section_biomes(actual_section).unwrap_or_default();
            for index in 0..expected_biomes.len().min(actual_biomes.len()) {
                if expected_biomes[index] == actual_biomes[index] {
                    continue;
                }
                diff_count += 1;
                if samples.len() < 64 {
                    let cell_x = index % 4;
                    let cell_z = (index / 4) % 4;
                    let cell_y = index / 16;
                    samples.push(format!(
                        "cell=({},{},{}) oracle={} rust={}",
                        cell_x,
                        section_y * 4 + cell_y as i32,
                        cell_z,
                        expected_biomes[index],
                        actual_biomes[index]
                    ));
                }
            }
        }
        println!("biome_diff_count={diff_count}");
        for sample in samples {
            println!("{sample}");
        }
    }

    fn section_biomes(section: &HashMap<String, Tag>) -> anyhow::Result<Vec<String>> {
        let container = section
            .get("biomes")
            .and_then(compound)
            .ok_or_else(|| anyhow::anyhow!("missing biomes"))?;
        let palette = list_items(container.get("palette"))
            .unwrap_or_default()
            .iter()
            .filter_map(|tag| match tag {
                Tag::String(value) => Some(value.to_string()),
                _ => None,
            })
            .collect::<Vec<_>>();
        let values = paletted_values(container.get("data"), palette.len(), 64, false)?;
        Ok(values
            .into_iter()
            .map(|index| {
                palette
                    .get(index)
                    .cloned()
                    .unwrap_or_else(|| format!("<biome-palette:{index}>"))
            })
            .collect())
    }

    fn sorted_sections(root: &Tag) -> Vec<(i32, &HashMap<String, Tag>)> {
        let Some(root) = compound(root) else {
            return Vec::new();
        };
        let mut sections = sections_by_y(root).into_iter().collect::<Vec<_>>();
        sections.sort_by_key(|(section_y, _)| *section_y);
        sections
    }

    fn short_block_name(name: &str) -> &str {
        name.strip_prefix("minecraft:").unwrap_or(name)
    }
}

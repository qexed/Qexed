use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{Context as _, Result};
use qexed_nbt::{ListHeader, Tag, tag_id};
use serde::Deserialize;

const DATA_VERSION: i32 = 4790;
const WORLD_MIN_Y: i32 = -64;
const WORLD_HEIGHT: i32 = 384;
const SECTION_HEIGHT: i32 = 16;
const BLOCKS_PER_SECTION: usize = 16 * 16 * 16;
const HEIGHTMAP_BITS: usize = 9;
const HEIGHTMAP_LONGS: usize = 37;
const SEA_LEVEL: i32 = 63;

#[derive(Debug, Clone)]
pub struct WorldgenCache {
    data_root: PathBuf,
    reports_root: PathBuf,
}

#[derive(Debug, Clone)]
pub struct WorldGenerator {
    cache: WorldgenCache,
    seed: i64,
}

#[derive(Debug, Clone)]
pub struct ChunkRequest<'a> {
    pub dimension: &'a str,
    pub chunk_x: i32,
    pub chunk_z: i32,
}

#[derive(Debug, Clone, Deserialize)]
struct NoiseSettings {
    #[serde(default)]
    default_block: BlockStateJson,
    #[serde(default)]
    default_fluid: BlockStateJson,
    #[serde(default)]
    sea_level: Option<i32>,
    noise: NoiseShape,
}

#[derive(Debug, Clone, Deserialize)]
struct NoiseShape {
    min_y: i32,
    height: i32,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct BlockStateJson {
    #[serde(default = "default_air_name", rename = "Name")]
    name: String,
    #[serde(default, rename = "Properties")]
    properties: HashMap<String, String>,
}

#[derive(Debug, Clone)]
struct BlockIds {
    air: i32,
    bedrock: i32,
    stone: i32,
    deepslate: i32,
    grass_block: i32,
    dirt: i32,
    sand: i32,
    gravel: i32,
    snow_block: i32,
}

impl WorldgenCache {
    pub fn from_roots(data_root: impl Into<PathBuf>, reports_root: impl Into<PathBuf>) -> Self {
        Self {
            data_root: data_root.into(),
            reports_root: reports_root.into(),
        }
    }

    pub fn default_mojang_cache() -> Self {
        let cache_root = workspace_root()
            .join("cache/mojang")
            .join(qexed_config::MC_VERSION);
        Self::from_roots(
            cache_root.join("data/minecraft"),
            cache_root.join("generated/reports"),
        )
    }

    pub fn data_root(&self) -> &Path {
        &self.data_root
    }

    pub fn reports_root(&self) -> &Path {
        &self.reports_root
    }

    pub fn ensure_ready(&self) -> Result<()> {
        if !self.data_root.join("worldgen").is_dir() {
            anyhow::bail!(
                "Mojang worldgen cache is unavailable: {}",
                self.data_root.display()
            );
        }
        if !self.reports_root.join("blocks.json").is_file()
            || !self.reports_root.join("registries.json").is_file()
        {
            anyhow::bail!(
                "Mojang reports cache is unavailable: {}",
                self.reports_root.display()
            );
        }
        Ok(())
    }
}

impl Default for WorldgenCache {
    fn default() -> Self {
        Self::default_mojang_cache()
    }
}

impl WorldGenerator {
    pub fn new(cache: WorldgenCache, seed: i64) -> Result<Self> {
        cache.ensure_ready()?;
        Ok(Self { cache, seed })
    }

    pub fn default_cache(seed: i64) -> Result<Self> {
        Self::new(WorldgenCache::default(), seed)
    }

    pub fn generate_chunk_nbt(&self, request: ChunkRequest<'_>) -> Result<Tag> {
        match normalize_dimension(request.dimension).as_str() {
            "minecraft:overworld" => self.generate_overworld(request.chunk_x, request.chunk_z),
            "minecraft:the_nether" => self.generate_basic_dimension(
                request.chunk_x,
                request.chunk_z,
                "minecraft:netherrack",
                "minecraft:lava",
                "minecraft:nether_wastes",
                31,
            ),
            "minecraft:the_end" => self.generate_basic_dimension(
                request.chunk_x,
                request.chunk_z,
                "minecraft:end_stone",
                "minecraft:air",
                "minecraft:the_end",
                0,
            ),
            other => {
                log::warn!("unknown worldgen dimension, generating empty chunk: {other}");
                Ok(empty_chunk_root(request.chunk_x, request.chunk_z))
            }
        }
    }

    fn generate_overworld(&self, chunk_x: i32, chunk_z: i32) -> Result<Tag> {
        let settings = self.load_noise_settings("overworld")?;
        let min_y = settings.noise.min_y;
        let height = settings.noise.height;
        let sea_level = settings.sea_level.unwrap_or(SEA_LEVEL);
        let ids = BlockIds::from_cache()?;
        let default_block = block_state_id(&settings.default_block)?;
        let default_fluid = block_state_id(&settings.default_fluid)?;

        let mut columns = vec![vec![ids.air; height as usize]; 16 * 16];
        let mut heightmap = vec![min_y; 16 * 16];
        for local_z in 0..16 {
            for local_x in 0..16 {
                let world_x = chunk_x * 16 + local_x;
                let world_z = chunk_z * 16 + local_z;
                let terrain = overworld_height(self.seed, world_x, world_z);
                let index = column_index(local_x, local_z);
                heightmap[index] = terrain + 1;

                for y in min_y..min_y + height {
                    let block = if y == min_y {
                        ids.bedrock
                    } else if y <= terrain {
                        overworld_surface_block(&ids, y, terrain, sea_level, world_x, world_z)
                            .unwrap_or(default_block)
                    } else if y <= sea_level {
                        default_fluid
                    } else {
                        ids.air
                    };
                    columns[index][(y - min_y) as usize] = block;
                }
            }
        }

        Ok(chunk_root(
            chunk_x,
            chunk_z,
            min_y,
            height,
            "minecraft:plains",
            &columns,
            &heightmap,
        )?)
    }

    fn generate_basic_dimension(
        &self,
        chunk_x: i32,
        chunk_z: i32,
        solid_block: &str,
        fluid_block: &str,
        biome: &str,
        fluid_level: i32,
    ) -> Result<Tag> {
        let min_y = WORLD_MIN_Y;
        let height = WORLD_HEIGHT;
        let solid = qexed_registry_block_id(solid_block)?;
        let fluid = qexed_registry_block_id(fluid_block)?;
        let air = qexed_registry_block_id("minecraft:air")?;

        let mut columns = vec![vec![air; height as usize]; 16 * 16];
        let mut heightmap = vec![min_y; 16 * 16];
        for local_z in 0..16 {
            for local_x in 0..16 {
                let world_x = chunk_x * 16 + local_x;
                let world_z = chunk_z * 16 + local_z;
                let surface = basic_dimension_height(self.seed, world_x, world_z);
                let index = column_index(local_x, local_z);
                heightmap[index] = surface + 1;
                for y in min_y..min_y + height {
                    columns[index][(y - min_y) as usize] = if y <= surface {
                        solid
                    } else if fluid != air && y <= fluid_level {
                        fluid
                    } else {
                        air
                    };
                }
            }
        }
        chunk_root(chunk_x, chunk_z, min_y, height, biome, &columns, &heightmap)
    }

    fn load_noise_settings(&self, name: &str) -> Result<NoiseSettings> {
        let path = self
            .cache
            .data_root
            .join("worldgen/noise_settings")
            .join(format!("{name}.json"));
        read_json(&path)
    }
}

impl BlockIds {
    fn from_cache() -> Result<Self> {
        Ok(Self {
            air: qexed_registry_block_id("minecraft:air")?,
            bedrock: qexed_registry_block_id("minecraft:bedrock")?,
            stone: qexed_registry_block_id("minecraft:stone")?,
            deepslate: qexed_registry_block_id("minecraft:deepslate")?,
            grass_block: qexed_registry_block_id("minecraft:grass_block")?,
            dirt: qexed_registry_block_id("minecraft:dirt")?,
            sand: qexed_registry_block_id("minecraft:sand")?,
            gravel: qexed_registry_block_id("minecraft:gravel")?,
            snow_block: qexed_registry_block_id("minecraft:snow_block")?,
        })
    }
}

fn chunk_root(
    chunk_x: i32,
    chunk_z: i32,
    min_y: i32,
    height: i32,
    biome: &str,
    columns: &[Vec<i32>],
    heightmap: &[i32],
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
        sections_tag(min_y, height, biome, columns)?,
    );
    root.insert("Heightmaps".to_string(), heightmaps_tag(min_y, heightmap));
    root.insert(
        "block_entities".to_string(),
        list_tag(tag_id::COMPOUND, Vec::new()),
    );
    root.insert(
        "block_ticks".to_string(),
        list_tag(tag_id::COMPOUND, Vec::new()),
    );
    root.insert(
        "fluid_ticks".to_string(),
        list_tag(tag_id::COMPOUND, Vec::new()),
    );
    root.insert(
        "PostProcessing".to_string(),
        list_tag(tag_id::END, Vec::new()),
    );
    root.insert(
        "structures".to_string(),
        Tag::Compound(Arc::new(HashMap::new())),
    );
    Ok(Tag::Compound(Arc::new(root)))
}

fn sections_tag(min_y: i32, height: i32, biome: &str, columns: &[Vec<i32>]) -> Result<Tag> {
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
        section_tag.insert("biomes".to_string(), single_biome_container(biome));
        sections.push(Tag::Compound(Arc::new(section_tag)));
    }
    Ok(list_tag(tag_id::COMPOUND, sections))
}

fn heightmaps_tag(min_y: i32, heightmap: &[i32]) -> Tag {
    let packed = pack_heightmap(heightmap, min_y);
    Tag::Compound(Arc::new(HashMap::from([
        (
            "WORLD_SURFACE".to_string(),
            Tag::LongArray(Arc::from(packed.clone())),
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

fn pack_heightmap(heightmap: &[i32], min_y: i32) -> Vec<i64> {
    let mut packed = vec![0_u64; HEIGHTMAP_LONGS];
    let mask = (1_u64 << HEIGHTMAP_BITS) - 1;
    for (index, height) in heightmap.iter().enumerate() {
        let value = (*height - min_y).clamp(0, mask as i32) as u64;
        let bit_index = index * HEIGHTMAP_BITS;
        let cell = bit_index / 64;
        let offset = bit_index % 64;
        packed[cell] |= value << offset;
        if offset + HEIGHTMAP_BITS > 64 {
            packed[cell + 1] |= value >> (64 - offset);
        }
    }
    packed.into_iter().map(|value| value as i64).collect()
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
            pack_values(&local_values, bits)?
                .into_iter()
                .map(|value| value as i64)
                .collect::<Vec<_>>(),
        )
    };
    Ok(paletted_container(palette_tags, data))
}

fn single_biome_container(biome: &str) -> Tag {
    paletted_container(
        vec![Tag::String(Arc::from(normalize_identifier(biome)))],
        None,
    )
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

fn pack_values(values: &[i32], bits: usize) -> Result<Vec<u64>> {
    let values_per_long = 64 / bits;
    let mut packed = vec![0_u64; values.len().div_ceil(values_per_long)];
    let mask = (1_u64 << bits) - 1;
    for (index, value) in values.iter().enumerate() {
        let value = u64::try_from(*value).context("negative palette value")?;
        if value > mask {
            anyhow::bail!("palette value {value} exceeds {bits} bits");
        }
        let cell = index / values_per_long;
        let offset = (index % values_per_long) * bits;
        packed[cell] |= value << offset;
    }
    Ok(packed)
}

fn overworld_height(seed: i64, x: i32, z: i32) -> i32 {
    let continents = value_noise(seed, x, z, 256.0);
    let erosion = value_noise(seed ^ 0x5deece66d, x, z, 96.0);
    let detail = value_noise(seed ^ 0x9e3779b97f4a7c15_u64 as i64, x, z, 32.0);
    let ridge = 1.0 - value_noise(seed ^ 0x632be59bd9b4e019_u64 as i64, x, z, 160.0).abs();
    let base = 64.0 + continents * 42.0 - erosion * 18.0 + detail * 7.0;
    let mountains = if continents > 0.18 { ridge * 58.0 } else { 0.0 };
    (base + mountains)
        .round()
        .clamp(WORLD_MIN_Y as f64 + 1.0, 255.0) as i32
}

fn basic_dimension_height(seed: i64, x: i32, z: i32) -> i32 {
    (48.0 + value_noise(seed ^ 0x34d0, x, z, 64.0) * 18.0).round() as i32
}

fn value_noise(seed: i64, x: i32, z: i32, scale: f64) -> f64 {
    let fx = x as f64 / scale;
    let fz = z as f64 / scale;
    let x0 = fx.floor() as i32;
    let z0 = fz.floor() as i32;
    let tx = smoothstep(fx - f64::from(x0));
    let tz = smoothstep(fz - f64::from(z0));
    let a = hash_unit(seed, x0, z0);
    let b = hash_unit(seed, x0 + 1, z0);
    let c = hash_unit(seed, x0, z0 + 1);
    let d = hash_unit(seed, x0 + 1, z0 + 1);
    lerp(lerp(a, b, tx), lerp(c, d, tx), tz)
}

fn hash_unit(seed: i64, x: i32, z: i32) -> f64 {
    let mut value = seed as u64;
    value ^= (x as u64).wrapping_mul(0x9e3779b97f4a7c15);
    value ^= (z as u64).wrapping_mul(0xbf58476d1ce4e5b9);
    value ^= value >> 30;
    value = value.wrapping_mul(0xbf58476d1ce4e5b9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94d049bb133111eb);
    value ^= value >> 31;
    (value as f64 / u64::MAX as f64) * 2.0 - 1.0
}

fn overworld_surface_block(
    ids: &BlockIds,
    y: i32,
    terrain: i32,
    sea_level: i32,
    x: i32,
    z: i32,
) -> Option<i32> {
    if y < 0 {
        return Some(ids.deepslate);
    }
    if terrain > 118 && y >= terrain - 1 {
        return Some(ids.snow_block);
    }
    if terrain < sea_level - 2 && y >= terrain - 3 {
        return Some(if value_noise(17, x, z, 18.0) > 0.2 {
            ids.gravel
        } else {
            ids.sand
        });
    }
    if y == terrain {
        Some(ids.grass_block)
    } else if y >= terrain - 4 {
        Some(ids.dirt)
    } else {
        Some(ids.stone)
    }
}

fn block_state_id(value: &BlockStateJson) -> Result<i32> {
    if value.properties.is_empty() {
        return qexed_registry_block_id(&value.name);
    }

    let report = read_blocks_report()?;
    let block = report
        .get(&value.name)
        .with_context(|| format!("block not found in Mojang report: {}", value.name))?;
    let states = block
        .get("states")
        .and_then(serde_json::Value::as_array)
        .with_context(|| format!("block states missing in Mojang report: {}", value.name))?;
    let mut properties = value.properties.iter().collect::<Vec<_>>();
    properties.sort_by(|left, right| left.0.cmp(right.0));
    for state in states {
        let state_props = state
            .get("properties")
            .and_then(serde_json::Value::as_object)
            .into_iter()
            .flat_map(|props| props.iter())
            .filter_map(|(key, value)| value.as_str().map(|value| (key, value)))
            .collect::<Vec<_>>();
        if state_props.len() == properties.len()
            && properties
                .iter()
                .all(|(key, value)| state_props.iter().any(|(k, v)| k == key && v == value))
            && let Some(id) = state.get("id").and_then(serde_json::Value::as_i64)
        {
            return i32::try_from(id).context("block state id does not fit i32");
        }
    }
    qexed_registry_block_id(&value.name)
}

fn qexed_registry_block_id(name: &str) -> Result<i32> {
    let name = normalize_identifier(name);
    let report = read_blocks_report()?;
    let block = report
        .get(&name)
        .with_context(|| format!("block not found in Mojang report: {name}"))?;
    let states = block
        .get("states")
        .and_then(serde_json::Value::as_array)
        .with_context(|| format!("block states missing in Mojang report: {name}"))?;
    let default = states
        .iter()
        .find(|state| {
            state
                .get("default")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false)
        })
        .or_else(|| states.first())
        .and_then(|state| state.get("id"))
        .and_then(serde_json::Value::as_i64)
        .with_context(|| format!("block state id missing in Mojang report: {name}"))?;
    i32::try_from(default).context("block state id does not fit i32")
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read Mojang cache JSON: {}", path.display()))?;
    serde_json::from_str(&content)
        .with_context(|| format!("failed to parse Mojang cache JSON: {}", path.display()))
}

fn read_blocks_report() -> Result<serde_json::Value> {
    let cache = WorldgenCache::default();
    read_json(&cache.reports_root.join("blocks.json"))
        .or_else(|_| qexed_registry::load_blocks_report())
}

fn empty_chunk_root(chunk_x: i32, chunk_z: i32) -> Tag {
    Tag::Compound(Arc::new(HashMap::from([
        ("DataVersion".to_string(), Tag::Int(DATA_VERSION)),
        ("xPos".to_string(), Tag::Int(chunk_x)),
        ("yPos".to_string(), Tag::Int(WORLD_MIN_Y / SECTION_HEIGHT)),
        ("zPos".to_string(), Tag::Int(chunk_z)),
        (
            "Status".to_string(),
            Tag::String(Arc::from("minecraft:full")),
        ),
        (
            "sections".to_string(),
            list_tag(tag_id::COMPOUND, Vec::new()),
        ),
        (
            "Heightmaps".to_string(),
            heightmaps_tag(WORLD_MIN_Y, &[WORLD_MIN_Y; 16 * 16]),
        ),
        (
            "block_entities".to_string(),
            list_tag(tag_id::COMPOUND, Vec::new()),
        ),
    ])))
}

fn list_tag(item_tag_id: u8, items: Vec<Tag>) -> Tag {
    Tag::List(
        ListHeader {
            tag_id: item_tag_id,
            length: items.len() as i32,
        },
        Arc::from(items),
    )
}

fn column_index(local_x: i32, local_z: i32) -> usize {
    (local_z * 16 + local_x) as usize
}

fn normalize_identifier(value: &str) -> String {
    if value.contains(':') {
        value.to_string()
    } else {
        format!("minecraft:{value}")
    }
}

fn normalize_dimension(value: &str) -> String {
    match normalize_identifier(value).as_str() {
        "minecraft:overworld" | "minecraft:the_nether" | "minecraft:the_end" => {
            normalize_identifier(value)
        }
        "minecraft:nether" => "minecraft:the_nether".to_string(),
        "minecraft:end" => "minecraft:the_end".to_string(),
        other => other.to_string(),
    }
}

fn ceil_log2(value: usize) -> usize {
    if value <= 1 {
        0
    } else {
        usize::BITS as usize - (value - 1).leading_zeros() as usize
    }
}

fn smoothstep(value: f64) -> f64 {
    value * value * (3.0 - 2.0 * value)
}

fn lerp(left: f64, right: f64, t: f64) -> f64 {
    left + (right - left) * t
}

fn default_air_name() -> String {
    "minecraft:air".to_string()
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

#[cfg(test)]
mod tests {
    use super::{ChunkRequest, WorldGenerator};

    #[test]
    fn generates_overworld_chunk_from_mojang_cache() {
        let Ok(generator) = WorldGenerator::default_cache(0) else {
            return;
        };

        let root = generator
            .generate_chunk_nbt(ChunkRequest {
                dimension: "minecraft:overworld",
                chunk_x: 0,
                chunk_z: 0,
            })
            .unwrap();

        assert!(matches!(root, qexed_nbt::Tag::Compound(_)));
    }
}

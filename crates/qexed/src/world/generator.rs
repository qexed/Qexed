use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{Context, Result};
use qexed_config::app::qexed::server::{
    World as WorldConfig, WorldGenerator as WorldGeneratorConfig,
};
use qexed_nbt::{ListHeader, Tag, tag_id};
use qexed_protocol::to_client::play::map_chunk::MapChunk;
use serde::Deserialize;

use super::{
    CHUNK_DAMPENING_LEN, SECTION_HEIGHT, WORLD_MAX_Y, WORLD_MIN_SECTION_Y, WORLD_MIN_Y,
    WorldLightAlgorithm, chunk_nbt, empty_chunk_packet, section_count, vanilla_noise,
};

const DEFAULT_FLAT_PRESET: &str = "minecraft:classic_flat";
const FLAT_PRESET_ROOT: &str =
    "assets/decompiled_source/src/data/minecraft/worldgen/flat_level_generator_preset";
const DEFAULT_NOISE_PRESET: &str = "minecraft:overworld";
const NOISE_SETTINGS_ROOT: &str =
    "assets/decompiled_source/src/data/minecraft/worldgen/noise_settings";
const HEIGHTMAP_BITS: usize = 9;
const HEIGHTMAP_ENTRY_COUNT: usize = 16 * 16;
const BLOCK_ENTRY_COUNT: usize = 16 * 16 * 16;

pub(crate) struct GeneratedChunk {
    pub packet: MapChunk,
    pub light_dampening: Vec<u8>,
}

pub(crate) trait WorldChunkGenerator: Send + Sync + std::fmt::Debug {
    fn generate(
        &self,
        dimension: &str,
        chunk_x: i32,
        chunk_z: i32,
        light_algorithm: WorldLightAlgorithm,
    ) -> Result<GeneratedChunk>;

    fn light_dampening(
        &self,
        dimension: &str,
        chunk_x: i32,
        chunk_z: i32,
        light_algorithm: WorldLightAlgorithm,
    ) -> Result<Vec<u8>> {
        Ok(self
            .generate(dimension, chunk_x, chunk_z, light_algorithm)?
            .light_dampening)
    }

    fn block_state_at(
        &self,
        dimension: &str,
        position: &qexed_packet::net_types::Position,
    ) -> Option<i32>;
}

pub(crate) fn from_config(config: &WorldConfig) -> Arc<dyn WorldChunkGenerator> {
    match config.generator {
        WorldGeneratorConfig::Empty => Arc::new(EmptyWorldGenerator),
        WorldGeneratorConfig::VanillaFlat => Arc::new(VanillaFlatGenerator::from_preset(
            config.generator_preset.trim(),
        )),
        WorldGeneratorConfig::VanillaNoise => Arc::new(VanillaNoiseGenerator::from_config(config)),
    }
}

#[derive(Debug)]
pub(crate) struct EmptyWorldGenerator;

impl WorldChunkGenerator for EmptyWorldGenerator {
    fn generate(
        &self,
        _dimension: &str,
        chunk_x: i32,
        chunk_z: i32,
        _light_algorithm: WorldLightAlgorithm,
    ) -> Result<GeneratedChunk> {
        Ok(GeneratedChunk {
            packet: empty_chunk_packet(chunk_x, chunk_z, super::WorldLightMode::Static),
            light_dampening: vec![0; CHUNK_DAMPENING_LEN],
        })
    }

    fn light_dampening(
        &self,
        _dimension: &str,
        _chunk_x: i32,
        _chunk_z: i32,
        _light_algorithm: WorldLightAlgorithm,
    ) -> Result<Vec<u8>> {
        Ok(vec![0; CHUNK_DAMPENING_LEN])
    }

    fn block_state_at(
        &self,
        _dimension: &str,
        _position: &qexed_packet::net_types::Position,
    ) -> Option<i32> {
        None
    }
}

#[derive(Debug)]
pub(crate) struct VanillaFlatGenerator {
    layers: Vec<FlatLayer>,
    root: Tag,
}

impl VanillaFlatGenerator {
    pub(crate) fn from_preset(preset: &str) -> Self {
        let preset = if preset.is_empty() {
            DEFAULT_FLAT_PRESET
        } else {
            preset
        };

        match load_flat_preset(preset) {
            Ok(settings) => Self::from_settings(settings),
            Err(err) => {
                log::warn!(
                    "failed to load vanilla flat preset {preset}, using classic_flat: {err:#}"
                );
                Self::from_settings(FlatSettings::classic())
            }
        }
    }

    fn from_settings(settings: FlatSettings) -> Self {
        let layers = expand_layers(settings.layers);
        let biome = normalize_identifier(&settings.biome);
        let root = flat_chunk_root(&layers, &biome);
        Self { layers, root }
    }
}

impl WorldChunkGenerator for VanillaFlatGenerator {
    fn generate(
        &self,
        _dimension: &str,
        chunk_x: i32,
        chunk_z: i32,
        light_algorithm: WorldLightAlgorithm,
    ) -> Result<GeneratedChunk> {
        let (packet, light_dampening) = chunk_nbt::network_chunk_and_light_dampening_from_nbt(
            chunk_x,
            chunk_z,
            &self.root,
            light_algorithm,
        )?;
        Ok(GeneratedChunk {
            packet,
            light_dampening,
        })
    }

    fn block_state_at(
        &self,
        _dimension: &str,
        position: &qexed_packet::net_types::Position,
    ) -> Option<i32> {
        let layer_index = position.y - WORLD_MIN_Y;
        let layer = usize::try_from(layer_index)
            .ok()
            .and_then(|index| self.layers.get(index))?;
        (!layer.is_air).then_some(layer.block_state_id)
    }
}

#[derive(Debug)]
pub(crate) struct VanillaNoiseGenerator {
    settings: NoiseSettings,
}

impl VanillaNoiseGenerator {
    fn from_config(config: &WorldConfig) -> Self {
        let preset = config.generator_preset.trim();
        let preset = if preset.is_empty() {
            DEFAULT_NOISE_PRESET
        } else {
            preset
        };

        match load_noise_settings(preset, config.seed) {
            Ok(settings) => Self { settings },
            Err(err) => {
                log::warn!(
                    "failed to load vanilla noise settings {preset}, using overworld: {err:#}"
                );
                Self {
                    settings: NoiseSettings::overworld(config.seed),
                }
            }
        }
    }
}

impl WorldChunkGenerator for VanillaNoiseGenerator {
    fn generate(
        &self,
        _dimension: &str,
        chunk_x: i32,
        chunk_z: i32,
        light_algorithm: WorldLightAlgorithm,
    ) -> Result<GeneratedChunk> {
        let chunk = self.settings.generate_chunk(chunk_x, chunk_z);
        let root = noise_chunk_root(&chunk, self.settings.biome.as_str());
        let (packet, light_dampening) = chunk_nbt::network_chunk_and_light_dampening_from_nbt(
            chunk_x,
            chunk_z,
            &root,
            light_algorithm,
        )?;
        Ok(GeneratedChunk {
            packet,
            light_dampening,
        })
    }

    fn block_state_at(
        &self,
        _dimension: &str,
        position: &qexed_packet::net_types::Position,
    ) -> Option<i32> {
        self.settings
            .block_state_at(position.x, position.y, position.z)
    }
}

#[derive(Debug, Clone)]
struct FlatSettings {
    biome: String,
    layers: Vec<FlatLayerSetting>,
}

impl FlatSettings {
    fn classic() -> Self {
        Self {
            biome: "minecraft:plains".to_string(),
            layers: vec![
                FlatLayerSetting {
                    block: "minecraft:bedrock".to_string(),
                    height: 1,
                },
                FlatLayerSetting {
                    block: "minecraft:dirt".to_string(),
                    height: 2,
                },
                FlatLayerSetting {
                    block: "minecraft:grass_block".to_string(),
                    height: 1,
                },
            ],
        }
    }
}

#[derive(Debug, Clone)]
struct FlatLayerSetting {
    block: String,
    height: usize,
}

#[derive(Debug, Clone)]
struct NoiseSettings {
    min_y: i32,
    height: i32,
    sea_level: i32,
    default_block: BlockLayer,
    default_fluid: BlockLayer,
    surface_block: BlockLayer,
    subsurface_block: BlockLayer,
    deepslate_block: BlockLayer,
    biome: String,
    density: TerrainDensity,
    surface_rules: vanilla_noise::OverworldSurfaceRules,
}

impl NoiseSettings {
    fn overworld(seed: i64) -> Self {
        let surface_rules = vanilla_noise::OverworldSurfaceRules::new(seed);
        Self {
            min_y: WORLD_MIN_Y,
            height: super::WORLD_SECTION_COUNT as i32 * SECTION_HEIGHT,
            sea_level: 63,
            default_block: BlockLayer::new("minecraft:stone"),
            default_fluid: BlockLayer::new("minecraft:water"),
            surface_block: BlockLayer::new("minecraft:grass_block"),
            subsurface_block: BlockLayer::new("minecraft:dirt"),
            deepslate_block: BlockLayer::new("minecraft:deepslate"),
            biome: "minecraft:plains".to_string(),
            density: TerrainDensity::overworld(seed),
            surface_rules,
        }
    }

    fn generate_chunk(&self, chunk_x: i32, chunk_z: i32) -> NoiseChunkBlocks {
        let mut columns = Vec::with_capacity(HEIGHTMAP_ENTRY_COUNT);
        for z in 0..16 {
            for x in 0..16 {
                let world_x = chunk_x * 16 + x;
                let world_z = chunk_z * 16 + z;
                columns.push(self.generate_column(world_x, world_z));
            }
        }
        NoiseChunkBlocks { columns }
    }

    fn generate_column(&self, world_x: i32, world_z: i32) -> NoiseColumnBlocks {
        let mut blocks = vec![self.air_layer(); self.height as usize];
        let profile = self.density.profile(world_x, world_z);
        let surface_height = self.surface_height_with_profile(world_x, world_z, &profile);
        for y in self.min_y..self.min_y + self.height {
            let index = (y - self.min_y) as usize;
            blocks[index] = self.layer_at(world_x, y, world_z, surface_height, &profile);
        }
        NoiseColumnBlocks {
            blocks,
            first_available_height: self.first_available_height(surface_height),
        }
    }

    fn block_state_at(&self, x: i32, y: i32, z: i32) -> Option<i32> {
        if !(self.min_y..self.min_y + self.height).contains(&y) {
            return None;
        }
        let profile = self.density.profile(x, z);
        let surface_height = self.surface_height_with_profile(x, z, &profile);
        let layer = self.layer_at(x, y, z, surface_height, &profile);
        (!layer.is_air).then_some(layer.block_state_id)
    }

    fn surface_height_with_profile(
        &self,
        x: i32,
        z: i32,
        profile: &vanilla_noise::OverworldTerrainProfile,
    ) -> i32 {
        self.density
            .surface_height(x, z, profile)
            .clamp(self.min_y + 1, self.min_y + self.height - 1)
    }

    fn layer_at(
        &self,
        x: i32,
        y: i32,
        z: i32,
        surface_height: i32,
        profile: &vanilla_noise::OverworldTerrainProfile,
    ) -> BlockLayer {
        if y <= self.min_y {
            return BlockLayer::new("minecraft:bedrock");
        }

        let density = self.density.sample(x, y, z, surface_height, profile);
        if density > 0.0 {
            if self.surface_rules.is_bedrock_floor(x, y, z, self.min_y) {
                return BlockLayer::new("minecraft:bedrock");
            }

            if y == surface_height {
                self.surface_block.clone()
            } else if y >= surface_height - 3 {
                self.subsurface_block.clone()
            } else if self.surface_rules.is_deepslate(x, y, z) {
                self.deepslate_block.clone()
            } else {
                self.default_block.clone()
            }
        } else if y <= self.sea_level {
            self.default_fluid.clone()
        } else {
            self.air_layer()
        }
    }

    fn first_available_height(&self, surface_height: i32) -> i32 {
        let height = surface_height.max(self.sea_level) + 1;
        (height - self.min_y).clamp(0, self.height)
    }

    fn air_layer(&self) -> BlockLayer {
        BlockLayer::new("minecraft:air")
    }
}

#[derive(Debug, Clone)]
struct TerrainDensity {
    blended_noise: vanilla_noise::BlendedNoise,
    terrain_noise: vanilla_noise::OverworldTerrainNoise,
}

impl TerrainDensity {
    fn overworld(seed: i64) -> Self {
        Self {
            blended_noise: vanilla_noise::BlendedNoise::overworld(seed),
            terrain_noise: vanilla_noise::OverworldTerrainNoise::new(seed),
        }
    }

    fn profile(&self, x: i32, z: i32) -> vanilla_noise::OverworldTerrainProfile {
        self.terrain_noise.profile(x, z)
    }

    fn surface_height(
        &self,
        x: i32,
        z: i32,
        profile: &vanilla_noise::OverworldTerrainProfile,
    ) -> i32 {
        (-64..=320)
            .rev()
            .find(|y| self.sample_with_profile(x, *y, z, &profile) > 0.0)
            .unwrap_or(-64)
    }

    fn sample(
        &self,
        x: i32,
        y: i32,
        z: i32,
        surface_height: i32,
        profile: &vanilla_noise::OverworldTerrainProfile,
    ) -> f64 {
        let density = self.sample_with_profile(x, y, z, &profile);
        if y <= surface_height {
            density
        } else {
            density.min(-1.0)
        }
    }

    fn sample_with_profile(
        &self,
        x: i32,
        y: i32,
        z: i32,
        profile: &vanilla_noise::OverworldTerrainProfile,
    ) -> f64 {
        let base_3d = self.blended_noise.compute(x, y, z);
        self.terrain_noise.final_density(profile, x, y, z, base_3d)
    }
}

#[derive(Debug, Clone)]
struct NoiseChunkBlocks {
    columns: Vec<NoiseColumnBlocks>,
}

impl NoiseChunkBlocks {
    fn column(&self, x: usize, z: usize) -> &NoiseColumnBlocks {
        &self.columns[z * 16 + x]
    }
}

#[derive(Debug, Clone)]
struct NoiseColumnBlocks {
    blocks: Vec<BlockLayer>,
    first_available_height: i32,
}

#[derive(Debug, Clone)]
struct BlockLayer {
    block: String,
    block_state_id: i32,
    properties: Vec<(String, String)>,
    is_air: bool,
}

impl BlockLayer {
    fn new(block: &str) -> Self {
        let block = normalize_identifier(block);
        let state = chunk_nbt::default_block_state(&block);
        Self {
            is_air: is_air_block(&block),
            block,
            block_state_id: state.id,
            properties: state.properties,
        }
    }
}

#[derive(Debug, Clone)]
struct FlatLayer {
    block: String,
    block_state_id: i32,
    properties: Vec<(String, String)>,
    is_air: bool,
}

#[derive(Deserialize)]
struct FlatPresetFile {
    settings: FlatPresetSettings,
}

#[derive(Deserialize)]
struct FlatPresetSettings {
    biome: String,
    layers: Vec<FlatPresetLayer>,
}

#[derive(Deserialize)]
struct FlatPresetLayer {
    block: String,
    height: usize,
}

fn load_flat_preset(preset: &str) -> Result<FlatSettings> {
    let path = flat_preset_path(preset)?;
    let content =
        std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    let preset: FlatPresetFile =
        serde_json::from_str(&content).with_context(|| format!("parse {}", path.display()))?;
    Ok(FlatSettings {
        biome: preset.settings.biome,
        layers: preset
            .settings
            .layers
            .into_iter()
            .map(|layer| FlatLayerSetting {
                block: layer.block,
                height: layer.height,
            })
            .collect(),
    })
}

fn load_noise_settings(preset: &str, seed: i64) -> Result<NoiseSettings> {
    let path = noise_settings_path(preset)?;
    let content =
        std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    let value: serde_json::Value =
        serde_json::from_str(&content).with_context(|| format!("parse {}", path.display()))?;
    let noise = value
        .get("noise")
        .and_then(serde_json::Value::as_object)
        .context("noise settings missing noise object")?;
    let min_y = json_i32(noise.get("min_y")).unwrap_or(WORLD_MIN_Y);
    let height =
        json_i32(noise.get("height")).unwrap_or(super::WORLD_SECTION_COUNT as i32 * SECTION_HEIGHT);
    let sea_level = json_i32(value.get("sea_level")).unwrap_or(63);
    let default_block = json_block_name(value.get("default_block")).unwrap_or("minecraft:stone");
    let default_fluid = json_block_name(value.get("default_fluid")).unwrap_or("minecraft:water");
    let surface_rules = vanilla_noise::OverworldSurfaceRules::new(seed);

    Ok(NoiseSettings {
        min_y,
        height,
        sea_level,
        default_block: BlockLayer::new(default_block),
        default_fluid: BlockLayer::new(default_fluid),
        surface_block: BlockLayer::new("minecraft:grass_block"),
        subsurface_block: BlockLayer::new("minecraft:dirt"),
        deepslate_block: BlockLayer::new("minecraft:deepslate"),
        biome: "minecraft:plains".to_string(),
        density: TerrainDensity::overworld(seed),
        surface_rules,
    })
}

fn flat_preset_path(preset: &str) -> Result<PathBuf> {
    let preset = preset.trim();
    let name = preset
        .strip_prefix("minecraft:")
        .or_else(|| preset.strip_prefix(':'))
        .unwrap_or(preset)
        .trim_matches('/');
    if name.is_empty()
        || Path::new(name)
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        anyhow::bail!("invalid flat preset name: {preset}");
    }
    Ok(workspace_root()
        .join(FLAT_PRESET_ROOT)
        .join(name)
        .with_extension("json"))
}

fn noise_settings_path(preset: &str) -> Result<PathBuf> {
    let name = resource_path_name(preset)?;
    Ok(workspace_root()
        .join(NOISE_SETTINGS_ROOT)
        .join(name)
        .with_extension("json"))
}

fn resource_path_name(value: &str) -> Result<String> {
    let value = value.trim();
    let name = value
        .strip_prefix("minecraft:")
        .or_else(|| value.strip_prefix(':'))
        .unwrap_or(value)
        .trim_matches('/');
    if name.is_empty()
        || Path::new(name)
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        anyhow::bail!("invalid resource name: {value}");
    }
    Ok(name.to_string())
}

fn expand_layers(settings: Vec<FlatLayerSetting>) -> Vec<FlatLayer> {
    let mut layers = Vec::new();
    for layer in settings {
        let block = normalize_identifier(&layer.block);
        let block_state = chunk_nbt::default_block_state(&block);
        let is_air = is_air_block(&block);
        for _ in 0..layer.height {
            layers.push(FlatLayer {
                block: block.clone(),
                block_state_id: block_state.id,
                properties: block_state.properties.clone(),
                is_air,
            });
        }
        if layers.len() >= super::WORLD_SECTION_COUNT * SECTION_HEIGHT as usize {
            layers.truncate(super::WORLD_SECTION_COUNT * SECTION_HEIGHT as usize);
            break;
        }
    }
    layers
}

fn flat_chunk_root(layers: &[FlatLayer], biome: &str) -> Tag {
    compound_tag([
        ("sections", sections_tag(layers, biome)),
        ("Heightmaps", heightmaps_tag(layers)),
    ])
}

fn noise_chunk_root(chunk: &NoiseChunkBlocks, biome: &str) -> Tag {
    compound_tag([
        ("sections", noise_sections_tag(chunk, biome)),
        ("Heightmaps", noise_heightmaps_tag(chunk)),
    ])
}

fn sections_tag(layers: &[FlatLayer], biome: &str) -> Tag {
    let mut sections = Vec::new();
    for section_y in WORLD_MIN_SECTION_Y..WORLD_MIN_SECTION_Y + section_count() {
        let start_layer = ((section_y * SECTION_HEIGHT) - WORLD_MIN_Y) as usize;
        let end_layer = start_layer + SECTION_HEIGHT as usize;
        let section_layers = &layers[start_layer.min(layers.len())..end_layer.min(layers.len())];
        if section_layers.iter().all(|layer| layer.is_air) {
            continue;
        }
        sections.push(section_tag(section_y, section_layers, biome));
    }

    Tag::List(
        ListHeader {
            tag_id: tag_id::COMPOUND,
            length: sections.len() as i32,
        },
        Arc::from(sections),
    )
}

fn section_tag(section_y: i32, layers: &[FlatLayer], biome: &str) -> Tag {
    compound_tag([
        ("Y", Tag::Byte(section_y as i8)),
        ("block_states", block_states_tag(layers)),
        ("biomes", biomes_tag(biome)),
    ])
}

fn noise_sections_tag(chunk: &NoiseChunkBlocks, biome: &str) -> Tag {
    let mut sections = Vec::new();
    for section_y in WORLD_MIN_SECTION_Y..WORLD_MIN_SECTION_Y + section_count() {
        let block_states = noise_block_states_tag(chunk, section_y);
        if !noise_section_is_air(chunk, section_y) {
            sections.push(compound_tag([
                ("Y", Tag::Byte(section_y as i8)),
                ("block_states", block_states),
                ("biomes", biomes_tag(biome)),
            ]));
        }
    }

    Tag::List(
        ListHeader {
            tag_id: tag_id::COMPOUND,
            length: sections.len() as i32,
        },
        Arc::from(sections),
    )
}

fn noise_section_is_air(chunk: &NoiseChunkBlocks, section_y: i32) -> bool {
    let start_y = section_y * SECTION_HEIGHT;
    for y in 0..SECTION_HEIGHT as usize {
        let world_y = start_y + y as i32;
        if !(WORLD_MIN_Y..=WORLD_MAX_Y).contains(&world_y) {
            continue;
        }
        let index = (world_y - WORLD_MIN_Y) as usize;
        for z in 0..16 {
            for x in 0..16 {
                if !chunk.column(x, z).blocks[index].is_air {
                    return false;
                }
            }
        }
    }
    true
}

fn noise_block_states_tag(chunk: &NoiseChunkBlocks, section_y: i32) -> Tag {
    let mut palette = Vec::new();
    let mut index_by_block = HashMap::<String, usize>::new();
    let mut values = vec![0_i32; BLOCK_ENTRY_COUNT];
    let start_y = section_y * SECTION_HEIGHT;

    for local_y in 0..SECTION_HEIGHT as usize {
        let world_y = start_y + local_y as i32;
        for z in 0..16 {
            for x in 0..16 {
                let layer = if (WORLD_MIN_Y..=WORLD_MAX_Y).contains(&world_y) {
                    let y_index = (world_y - WORLD_MIN_Y) as usize;
                    &chunk.column(x, z).blocks[y_index]
                } else {
                    continue;
                };
                let key = block_state_palette_key(&layer.block, &layer.properties);
                let next_index = palette.len();
                let palette_index = *index_by_block.entry(key).or_insert_with(|| {
                    palette.push(block_state_tag(&layer.block, &layer.properties));
                    next_index
                });
                values[(local_y * 16 + z) * 16 + x] = palette_index as i32;
            }
        }
    }

    let data = (palette.len() > 1).then(|| {
        let bits = palette_storage_bits(palette.len());
        pack_fixed_long_values(&values, bits)
            .into_iter()
            .map(|value| value as i64)
            .collect::<Vec<_>>()
    });
    paletted_container(palette, data)
}

fn block_states_tag(layers: &[FlatLayer]) -> Tag {
    let mut palette = Vec::new();
    let mut index_by_block = HashMap::<String, usize>::new();
    let mut values = vec![0_i32; BLOCK_ENTRY_COUNT];
    let air = chunk_nbt::default_block_state("minecraft:air");

    for local_y in 0..SECTION_HEIGHT as usize {
        let (block, properties) = layers
            .get(local_y)
            .map(|layer| (layer.block.as_str(), layer.properties.as_slice()))
            .unwrap_or(("minecraft:air", air.properties.as_slice()));
        let key = block_state_palette_key(block, properties);
        let next_index = palette.len();
        let palette_index = *index_by_block.entry(key).or_insert_with(|| {
            palette.push(block_state_tag(block, properties));
            next_index
        });

        for z in 0..16 {
            for x in 0..16 {
                values[(local_y * 16 + z) * 16 + x] = palette_index as i32;
            }
        }
    }

    let data = (palette.len() > 1).then(|| {
        let bits = palette_storage_bits(palette.len());
        pack_fixed_long_values(&values, bits)
            .into_iter()
            .map(|value| value as i64)
            .collect::<Vec<_>>()
    });
    paletted_container(palette, data)
}

fn biomes_tag(biome: &str) -> Tag {
    paletted_container(vec![Tag::String(Arc::from(biome.to_string()))], None)
}

fn heightmaps_tag(layers: &[FlatLayer]) -> Tag {
    let height = flat_first_available_height(layers);
    let packed = pack_fixed_long_values(&vec![height; HEIGHTMAP_ENTRY_COUNT], HEIGHTMAP_BITS)
        .into_iter()
        .map(|value| value as i64)
        .collect::<Vec<_>>();
    compound_tag([
        ("WORLD_SURFACE", Tag::LongArray(Arc::from(packed.clone()))),
        ("MOTION_BLOCKING", Tag::LongArray(Arc::from(packed.clone()))),
        (
            "MOTION_BLOCKING_NO_LEAVES",
            Tag::LongArray(Arc::from(packed)),
        ),
    ])
}

fn noise_heightmaps_tag(chunk: &NoiseChunkBlocks) -> Tag {
    let mut values = vec![0_i32; HEIGHTMAP_ENTRY_COUNT];
    for z in 0..16 {
        for x in 0..16 {
            values[z * 16 + x] = chunk.column(x, z).first_available_height;
        }
    }
    let packed = pack_fixed_long_values(&values, HEIGHTMAP_BITS)
        .into_iter()
        .map(|value| value as i64)
        .collect::<Vec<_>>();
    compound_tag([
        ("WORLD_SURFACE", Tag::LongArray(Arc::from(packed.clone()))),
        ("MOTION_BLOCKING", Tag::LongArray(Arc::from(packed.clone()))),
        (
            "MOTION_BLOCKING_NO_LEAVES",
            Tag::LongArray(Arc::from(packed)),
        ),
    ])
}

fn flat_first_available_height(layers: &[FlatLayer]) -> i32 {
    let Some(highest) = layers.iter().rposition(|layer| !layer.is_air) else {
        return 0;
    };
    let height = WORLD_MIN_Y + highest as i32 + 1;
    (height - WORLD_MIN_Y).clamp(0, WORLD_MAX_Y - WORLD_MIN_Y + 1)
}

fn block_state_tag(name: &str, properties: &[(String, String)]) -> Tag {
    let mut fields = HashMap::new();
    fields.insert("Name".to_string(), Tag::String(Arc::from(name.to_string())));
    if !properties.is_empty() {
        fields.insert(
            "Properties".to_string(),
            Tag::Compound(Arc::new(
                properties
                    .iter()
                    .map(|(key, value)| (key.clone(), Tag::String(Arc::from(value.clone()))))
                    .collect(),
            )),
        );
    }
    Tag::Compound(Arc::new(fields))
}

fn block_state_palette_key(name: &str, properties: &[(String, String)]) -> String {
    let mut key = name.to_string();
    key.push('|');
    for (name, value) in properties {
        key.push_str(name);
        key.push('=');
        key.push_str(value);
        key.push(';');
    }
    key
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

fn palette_storage_bits(palette_len: usize) -> usize {
    ceil_log2(palette_len).max(4)
}

fn pack_fixed_long_values(values: &[i32], bits: usize) -> Vec<u64> {
    let values_per_long = 64 / bits;
    let mask = (1_u64 << bits) - 1;
    let mut packed = vec![0_u64; values.len().div_ceil(values_per_long)];

    for (index, value) in values.iter().enumerate() {
        let cell = index / values_per_long;
        let bit_offset = (index % values_per_long) * bits;
        packed[cell] |= ((*value as u64) & mask) << bit_offset;
    }

    packed
}

fn normalize_identifier(value: &str) -> String {
    let value = value.trim();
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

fn json_i32(value: Option<&serde_json::Value>) -> Option<i32> {
    value
        .and_then(serde_json::Value::as_i64)
        .and_then(|value| i32::try_from(value).ok())
}

fn json_block_name(value: Option<&serde_json::Value>) -> Option<&str> {
    value?
        .get("Name")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn ceil_log2(count: usize) -> usize {
    if count <= 1 {
        0
    } else {
        usize::BITS as usize - (count - 1).leading_zeros() as usize
    }
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
    use qexed_packet::Packet;

    use super::*;

    #[test]
    fn vanilla_flat_uses_classic_flat_layers_from_min_y() {
        let generator = VanillaFlatGenerator::from_preset(DEFAULT_FLAT_PRESET);
        let bedrock = chunk_nbt::default_block_state_id("minecraft:bedrock");
        let dirt = chunk_nbt::default_block_state_id("minecraft:dirt");
        let grass = chunk_nbt::default_block_state_id("minecraft:grass_block");

        assert_eq!(
            generator.block_state_at(
                "minecraft:overworld",
                &qexed_packet::net_types::Position {
                    x: 0,
                    y: WORLD_MIN_Y,
                    z: 0
                }
            ),
            Some(bedrock)
        );
        assert_eq!(
            generator.block_state_at(
                "minecraft:overworld",
                &qexed_packet::net_types::Position {
                    x: 0,
                    y: WORLD_MIN_Y + 2,
                    z: 0
                }
            ),
            Some(dirt)
        );
        assert_eq!(
            generator.block_state_at(
                "minecraft:overworld",
                &qexed_packet::net_types::Position {
                    x: 0,
                    y: WORLD_MIN_Y + 3,
                    z: 0
                }
            ),
            Some(grass)
        );
        assert_eq!(
            generator.block_state_at(
                "minecraft:overworld",
                &qexed_packet::net_types::Position {
                    x: 0,
                    y: WORLD_MIN_Y + 4,
                    z: 0
                }
            ),
            None
        );
    }

    #[test]
    fn vanilla_flat_generated_chunk_serializes() {
        let generator = VanillaFlatGenerator::from_preset(DEFAULT_FLAT_PRESET);
        let generated = generator
            .generate("minecraft:overworld", 0, 0, WorldLightAlgorithm::Fast)
            .unwrap();
        let mut payload = bytes::BytesMut::new();
        let mut writer = qexed_packet::PacketWriter::new(&mut payload);

        generated.packet.serialize(&mut writer).unwrap();

        assert!(!payload.is_empty());
        assert_eq!(generated.light_dampening.len(), CHUNK_DAMPENING_LEN);
        assert_eq!(generated.packet.data.heightmaps.len(), 3);
    }

    #[test]
    fn flat_heightmap_uses_vanilla_first_available_offset() {
        let layers = expand_layers(FlatSettings::classic().layers);
        let packed = match heightmaps_tag(&layers) {
            Tag::Compound(fields) => fields
                .get("WORLD_SURFACE")
                .and_then(|tag| match tag {
                    Tag::LongArray(values) => Some(values.clone()),
                    _ => None,
                })
                .unwrap(),
            _ => panic!("heightmaps must be compound"),
        };

        assert_eq!(packed.len(), 37);
        assert_eq!(packed[0] & 0x1ff, 4);
    }

    #[test]
    fn vanilla_noise_generated_chunk_serializes() {
        let config = WorldConfig {
            generator: WorldGeneratorConfig::VanillaNoise,
            generator_preset: "minecraft:overworld".to_string(),
            seed: 0,
            ..WorldConfig::default()
        };
        let generator = VanillaNoiseGenerator::from_config(&config);
        let generated = generator
            .generate("minecraft:overworld", 0, 0, WorldLightAlgorithm::Fast)
            .unwrap();
        let mut payload = bytes::BytesMut::new();
        let mut writer = qexed_packet::PacketWriter::new(&mut payload);

        generated.packet.serialize(&mut writer).unwrap();

        assert!(!payload.is_empty());
        assert_eq!(generated.light_dampening.len(), CHUNK_DAMPENING_LEN);
        assert_eq!(generated.packet.data.heightmaps.len(), 3);
    }

    #[test]
    fn vanilla_noise_block_state_at_uses_seeded_terrain() {
        let config = WorldConfig {
            generator: WorldGeneratorConfig::VanillaNoise,
            generator_preset: "minecraft:overworld".to_string(),
            seed: 12345,
            ..WorldConfig::default()
        };
        let generator = VanillaNoiseGenerator::from_config(&config);

        assert!(
            generator
                .block_state_at(
                    "minecraft:overworld",
                    &qexed_packet::net_types::Position { x: 0, y: -64, z: 0 }
                )
                .is_some()
        );
        assert_eq!(
            generator.block_state_at(
                "minecraft:overworld",
                &qexed_packet::net_types::Position { x: 0, y: 320, z: 0 }
            ),
            None
        );
    }

    #[test]
    fn vanilla_noise_applies_deepslate_surface_rule() {
        let config = WorldConfig {
            generator: WorldGeneratorConfig::VanillaNoise,
            generator_preset: "minecraft:overworld".to_string(),
            seed: 12345,
            ..WorldConfig::default()
        };
        let generator = VanillaNoiseGenerator::from_config(&config);
        let deepslate = chunk_nbt::default_block_state_id("minecraft:deepslate");

        let has_deepslate = (-16..=16).any(|x| {
            (-16..=16).any(|z| {
                (-32..=0).any(|y| {
                    generator.block_state_at(
                        "minecraft:overworld",
                        &qexed_packet::net_types::Position { x, y, z },
                    ) == Some(deepslate)
                })
            })
        });

        assert!(has_deepslate);
    }
}

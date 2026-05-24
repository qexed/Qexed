use std::{
    collections::{HashMap, HashSet},
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
const CARVER_RANGE: i32 = 4;
const CARVER_SOURCE_RANGE: i32 = 8;
const EMERALD_ORE_BIOMES: &[&str] = &[
    "minecraft:cherry_grove",
    "minecraft:frozen_peaks",
    "minecraft:grove",
    "minecraft:jagged_peaks",
    "minecraft:meadow",
    "minecraft:snowy_slopes",
    "minecraft:stony_peaks",
    "minecraft:windswept_gravelly_hills",
    "minecraft:windswept_hills",
    "minecraft:windswept_forest",
];
const BADLANDS_ORE_BIOMES: &[&str] = &[
    "minecraft:badlands",
    "minecraft:eroded_badlands",
    "minecraft:wooded_badlands",
];
const DRIPSTONE_CAVES_ORE_BIOMES: &[&str] = &["minecraft:dripstone_caves"];
const DISK_DIRT_GRASS_TARGETS: &[&str] = &["minecraft:dirt", "minecraft:grass_block"];
const DISK_DIRT_CLAY_TARGETS: &[&str] = &["minecraft:dirt", "minecraft:clay"];
const SPRING_WATER_VALID_BLOCKS: &[&str] = &[
    "minecraft:stone",
    "minecraft:granite",
    "minecraft:diorite",
    "minecraft:andesite",
    "minecraft:deepslate",
    "minecraft:tuff",
    "minecraft:calcite",
    "minecraft:dirt",
    "minecraft:snow_block",
    "minecraft:powder_snow",
    "minecraft:packed_ice",
];
const SPRING_LAVA_VALID_BLOCKS: &[&str] = &[
    "minecraft:stone",
    "minecraft:granite",
    "minecraft:diorite",
    "minecraft:andesite",
    "minecraft:deepslate",
    "minecraft:tuff",
    "minecraft:calcite",
    "minecraft:dirt",
];

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
                    settings: NoiseSettings::overworld(
                        config.seed,
                        vanilla_noise::OverworldNoiseKind::Default,
                    ),
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
    lava_block: BlockLayer,
    surface_block: BlockLayer,
    subsurface_block: BlockLayer,
    deepslate_block: BlockLayer,
    podzol_block: BlockLayer,
    coarse_dirt_block: BlockLayer,
    mycelium_block: BlockLayer,
    calcite_block: BlockLayer,
    gravel_block: BlockLayer,
    sand_block: BlockLayer,
    sandstone_block: BlockLayer,
    packed_ice_block: BlockLayer,
    ice_block: BlockLayer,
    snow_block: BlockLayer,
    powder_snow_block: BlockLayer,
    mud_block: BlockLayer,
    water_block: BlockLayer,
    terracotta_block: BlockLayer,
    orange_terracotta_block: BlockLayer,
    white_terracotta_block: BlockLayer,
    yellow_terracotta_block: BlockLayer,
    brown_terracotta_block: BlockLayer,
    red_terracotta_block: BlockLayer,
    light_gray_terracotta_block: BlockLayer,
    red_sand_block: BlockLayer,
    copper_ore_block: BlockLayer,
    raw_copper_block: BlockLayer,
    granite_block: BlockLayer,
    deepslate_iron_ore_block: BlockLayer,
    raw_iron_block: BlockLayer,
    tuff_block: BlockLayer,
    biome: String,
    density: TerrainDensity,
    surface_rules: vanilla_noise::OverworldSurfaceRules,
    aquifer: vanilla_noise::OverworldAquifer,
    ore_veins: vanilla_noise::OreVeinNoise,
    carvers: VanillaCarvers,
    ore_features: OverworldOreFeatures,
}

impl NoiseSettings {
    fn overworld(seed: i64, noise_kind: vanilla_noise::OverworldNoiseKind) -> Self {
        let surface_rules = vanilla_noise::OverworldSurfaceRules::new(seed);
        let sea_level = 63;
        Self {
            min_y: WORLD_MIN_Y,
            height: super::WORLD_SECTION_COUNT as i32 * SECTION_HEIGHT,
            sea_level,
            default_block: BlockLayer::new("minecraft:stone"),
            default_fluid: BlockLayer::new("minecraft:water"),
            lava_block: BlockLayer::new("minecraft:lava"),
            surface_block: BlockLayer::new("minecraft:grass_block"),
            subsurface_block: BlockLayer::new("minecraft:dirt"),
            deepslate_block: BlockLayer::new("minecraft:deepslate"),
            podzol_block: BlockLayer::new("minecraft:podzol"),
            coarse_dirt_block: BlockLayer::new("minecraft:coarse_dirt"),
            mycelium_block: BlockLayer::new("minecraft:mycelium"),
            calcite_block: BlockLayer::new("minecraft:calcite"),
            gravel_block: BlockLayer::new("minecraft:gravel"),
            sand_block: BlockLayer::new("minecraft:sand"),
            sandstone_block: BlockLayer::new("minecraft:sandstone"),
            packed_ice_block: BlockLayer::new("minecraft:packed_ice"),
            ice_block: BlockLayer::new("minecraft:ice"),
            snow_block: BlockLayer::new("minecraft:snow_block"),
            powder_snow_block: BlockLayer::new("minecraft:powder_snow"),
            mud_block: BlockLayer::new("minecraft:mud"),
            water_block: BlockLayer::new("minecraft:water"),
            terracotta_block: BlockLayer::new("minecraft:terracotta"),
            orange_terracotta_block: BlockLayer::new("minecraft:orange_terracotta"),
            white_terracotta_block: BlockLayer::new("minecraft:white_terracotta"),
            yellow_terracotta_block: BlockLayer::new("minecraft:yellow_terracotta"),
            brown_terracotta_block: BlockLayer::new("minecraft:brown_terracotta"),
            red_terracotta_block: BlockLayer::new("minecraft:red_terracotta"),
            light_gray_terracotta_block: BlockLayer::new("minecraft:light_gray_terracotta"),
            red_sand_block: BlockLayer::new("minecraft:red_sand"),
            copper_ore_block: BlockLayer::new("minecraft:copper_ore"),
            raw_copper_block: BlockLayer::new("minecraft:raw_copper_block"),
            granite_block: BlockLayer::new("minecraft:granite"),
            deepslate_iron_ore_block: BlockLayer::new("minecraft:deepslate_iron_ore"),
            raw_iron_block: BlockLayer::new("minecraft:raw_iron_block"),
            tuff_block: BlockLayer::new("minecraft:tuff"),
            biome: "minecraft:plains".to_string(),
            density: TerrainDensity::overworld(seed, noise_kind),
            surface_rules,
            aquifer: vanilla_noise::OverworldAquifer::new(seed, sea_level),
            ore_veins: vanilla_noise::OreVeinNoise::new(seed),
            carvers: VanillaCarvers::new(seed),
            ore_features: OverworldOreFeatures::new(seed),
        }
    }

    fn generate_chunk(&self, chunk_x: i32, chunk_z: i32) -> NoiseChunkBlocks {
        let (mut chunk, preliminary_surfaces) = self.generate_base_chunk(chunk_x, chunk_z);
        self.carvers
            .carve_chunk(self, chunk_x, chunk_z, &preliminary_surfaces, &mut chunk);
        self.ore_features
            .place_chunk(self, chunk_x, chunk_z, &mut chunk);
        chunk.recompute_first_available_heights(self.min_y, self.height);
        chunk
    }

    fn generate_base_chunk(&self, chunk_x: i32, chunk_z: i32) -> (NoiseChunkBlocks, Vec<i32>) {
        let mut columns = Vec::with_capacity(HEIGHTMAP_ENTRY_COUNT);
        let mut profiles = Vec::with_capacity((17 * 17) as usize);
        let mut surface_heights = Vec::with_capacity((17 * 17) as usize);
        let mut preliminary_surfaces = Vec::with_capacity(HEIGHTMAP_ENTRY_COUNT);

        for z in 0..=16 {
            for x in 0..=16 {
                let world_x = chunk_x * 16 + x;
                let world_z = chunk_z * 16 + z;
                let profile = self.density.profile(world_x, world_z);
                let surface_height = self.surface_height_with_profile(world_x, world_z, &profile);
                profiles.push(profile);
                surface_heights.push(surface_height);
                if x < 16 && z < 16 {
                    preliminary_surfaces.push(self.preliminary_surface_with_profile(
                        world_x,
                        world_z,
                        profiles.last().expect("profile was just pushed"),
                    ));
                }
            }
        }

        for z in 0..16 {
            for x in 0..16 {
                let world_x = chunk_x * 16 + x;
                let world_z = chunk_z * 16 + z;
                let index = (z * 17 + x) as usize;
                let surface_height = surface_heights[index];
                let east_height = surface_heights[index + 1];
                let south_height = surface_heights[index + 17];
                let slope = (east_height - surface_height)
                    .abs()
                    .max((south_height - surface_height).abs());
                columns.push(self.generate_column_with_profile(
                    world_x,
                    world_z,
                    &profiles[index],
                    surface_height,
                    preliminary_surfaces[(z * 16 + x) as usize],
                    slope,
                ));
            }
        }
        (
            NoiseChunkBlocks {
                columns,
                biomes: self.generate_biomes(chunk_x, chunk_z),
            },
            preliminary_surfaces,
        )
    }

    fn generate_biomes(&self, chunk_x: i32, chunk_z: i32) -> Vec<String> {
        let mut biomes = Vec::with_capacity(section_count() as usize * 64);
        for section_y in WORLD_MIN_SECTION_Y..WORLD_MIN_SECTION_Y + section_count() {
            for local_x in 0..4 {
                let world_x = chunk_x * 16 + local_x * 4;
                for local_y in 0..4 {
                    let world_y = section_y * SECTION_HEIGHT + local_y * 4;
                    for local_z in 0..4 {
                        let world_z = chunk_z * 16 + local_z * 4;
                        biomes.push(self.density.biome(world_x, world_y, world_z).to_string());
                    }
                }
            }
        }
        biomes
    }

    fn generate_column_with_profile(
        &self,
        world_x: i32,
        world_z: i32,
        profile: &vanilla_noise::OverworldTerrainProfile,
        surface_height: i32,
        preliminary_surface: i32,
        surface_slope: i32,
    ) -> NoiseColumnBlocks {
        let mut blocks = vec![self.air_layer(); self.height as usize];
        for y in self.min_y..self.min_y + self.height {
            let index = (y - self.min_y) as usize;
            blocks[index] = self.layer_at(
                world_x,
                y,
                world_z,
                surface_height,
                preliminary_surface,
                surface_slope,
                profile,
            );
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
        let preliminary_surface = self.preliminary_surface_with_profile(x, z, &profile);
        let surface_slope = self.surface_slope(x, z, surface_height);
        let layer = self.layer_at(
            x,
            y,
            z,
            surface_height,
            preliminary_surface,
            surface_slope,
            &profile,
        );
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

    fn preliminary_surface_with_profile(
        &self,
        x: i32,
        z: i32,
        profile: &vanilla_noise::OverworldTerrainProfile,
    ) -> i32 {
        self.density
            .preliminary_surface_height(x, z, profile)
            .clamp(self.min_y, self.min_y + self.height - 1)
    }

    fn layer_at(
        &self,
        x: i32,
        y: i32,
        z: i32,
        surface_height: i32,
        preliminary_surface: i32,
        surface_slope: i32,
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

            if y < surface_height - 8 {
                return if self.surface_rules.is_deepslate(x, y, z) {
                    self.ore_vein_at(x, y, z)
                        .unwrap_or_else(|| self.deepslate_block.clone())
                } else {
                    self.ore_vein_at(x, y, z)
                        .unwrap_or_else(|| self.default_block.clone())
                };
            }

            let biome = self.density.biome(x, y, z);
            let surface_context = vanilla_noise::SurfaceRuleContext {
                x,
                y,
                z,
                surface_height,
                sea_level: self.sea_level,
                min_y: self.min_y,
                biome,
                slope: surface_slope,
            };

            if let Some(block) = self.surface_rules.block_at(surface_context) {
                self.surface_block_layer(block)
            } else if self.surface_rules.is_deepslate(x, y, z) {
                self.ore_vein_at(x, y, z)
                    .unwrap_or_else(|| self.deepslate_block.clone())
            } else {
                self.ore_vein_at(x, y, z)
                    .unwrap_or_else(|| self.default_block.clone())
            }
        } else {
            match self
                .aquifer
                .substance_at(x, y, z, density, preliminary_surface)
            {
                vanilla_noise::AquiferSubstance::DefaultBlock => self
                    .ore_vein_at(x, y, z)
                    .unwrap_or_else(|| self.default_block.clone()),
                vanilla_noise::AquiferSubstance::Fluid(vanilla_noise::AquiferFluid::Air) => {
                    self.air_layer()
                }
                vanilla_noise::AquiferSubstance::Fluid(vanilla_noise::AquiferFluid::Water) => {
                    self.default_fluid.clone()
                }
                vanilla_noise::AquiferSubstance::Fluid(vanilla_noise::AquiferFluid::Lava) => {
                    self.lava_block.clone()
                }
            }
        }
    }

    fn surface_block_layer(&self, block: vanilla_noise::SurfaceBlock) -> BlockLayer {
        match block {
            vanilla_noise::SurfaceBlock::Bedrock => BlockLayer::new("minecraft:bedrock"),
            vanilla_noise::SurfaceBlock::Stone => self.default_block.clone(),
            vanilla_noise::SurfaceBlock::Deepslate => self.deepslate_block.clone(),
            vanilla_noise::SurfaceBlock::Dirt => self.subsurface_block.clone(),
            vanilla_noise::SurfaceBlock::GrassBlock => self.surface_block.clone(),
            vanilla_noise::SurfaceBlock::Podzol => self.podzol_block.clone(),
            vanilla_noise::SurfaceBlock::CoarseDirt => self.coarse_dirt_block.clone(),
            vanilla_noise::SurfaceBlock::Mycelium => self.mycelium_block.clone(),
            vanilla_noise::SurfaceBlock::Calcite => self.calcite_block.clone(),
            vanilla_noise::SurfaceBlock::Gravel => self.gravel_block.clone(),
            vanilla_noise::SurfaceBlock::Sand => self.sand_block.clone(),
            vanilla_noise::SurfaceBlock::Sandstone => self.sandstone_block.clone(),
            vanilla_noise::SurfaceBlock::PackedIce => self.packed_ice_block.clone(),
            vanilla_noise::SurfaceBlock::Ice => self.ice_block.clone(),
            vanilla_noise::SurfaceBlock::SnowBlock => self.snow_block.clone(),
            vanilla_noise::SurfaceBlock::PowderSnow => self.powder_snow_block.clone(),
            vanilla_noise::SurfaceBlock::Mud => self.mud_block.clone(),
            vanilla_noise::SurfaceBlock::Water => self.water_block.clone(),
            vanilla_noise::SurfaceBlock::Terracotta => self.terracotta_block.clone(),
            vanilla_noise::SurfaceBlock::OrangeTerracotta => self.orange_terracotta_block.clone(),
            vanilla_noise::SurfaceBlock::WhiteTerracotta => self.white_terracotta_block.clone(),
            vanilla_noise::SurfaceBlock::YellowTerracotta => self.yellow_terracotta_block.clone(),
            vanilla_noise::SurfaceBlock::BrownTerracotta => self.brown_terracotta_block.clone(),
            vanilla_noise::SurfaceBlock::RedTerracotta => self.red_terracotta_block.clone(),
            vanilla_noise::SurfaceBlock::LightGrayTerracotta => {
                self.light_gray_terracotta_block.clone()
            }
            vanilla_noise::SurfaceBlock::RedSand => self.red_sand_block.clone(),
        }
    }

    fn surface_slope(&self, x: i32, z: i32, center: i32) -> i32 {
        let east = self.surface_height_with_profile(x + 1, z, &self.density.profile(x + 1, z));
        let south = self.surface_height_with_profile(x, z + 1, &self.density.profile(x, z + 1));
        (east - center).abs().max((south - center).abs())
    }

    fn ore_vein_at(&self, x: i32, y: i32, z: i32) -> Option<BlockLayer> {
        match self.ore_veins.block_at(x, y, z)? {
            vanilla_noise::OreVeinBlock::CopperOre => Some(self.copper_ore_block.clone()),
            vanilla_noise::OreVeinBlock::RawCopperBlock => Some(self.raw_copper_block.clone()),
            vanilla_noise::OreVeinBlock::Granite => Some(self.granite_block.clone()),
            vanilla_noise::OreVeinBlock::DeepslateIronOre => {
                Some(self.deepslate_iron_ore_block.clone())
            }
            vanilla_noise::OreVeinBlock::RawIronBlock => Some(self.raw_iron_block.clone()),
            vanilla_noise::OreVeinBlock::Tuff => Some(self.tuff_block.clone()),
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
    fn overworld(seed: i64, noise_kind: vanilla_noise::OverworldNoiseKind) -> Self {
        Self {
            blended_noise: vanilla_noise::BlendedNoise::overworld(seed),
            terrain_noise: vanilla_noise::OverworldTerrainNoise::with_kind(seed, noise_kind),
        }
    }

    fn profile(&self, x: i32, z: i32) -> vanilla_noise::OverworldTerrainProfile {
        self.terrain_noise.profile(x, z)
    }

    fn preliminary_surface_height(
        &self,
        x: i32,
        z: i32,
        profile: &vanilla_noise::OverworldTerrainProfile,
    ) -> i32 {
        self.terrain_noise.preliminary_surface_height(profile, x, z)
    }

    fn biome(&self, x: i32, y: i32, z: i32) -> &'static str {
        self.terrain_noise.biome(x, y, z)
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
    biomes: Vec<String>,
}

impl NoiseChunkBlocks {
    fn column(&self, x: usize, z: usize) -> &NoiseColumnBlocks {
        &self.columns[z * 16 + x]
    }

    fn column_mut(&mut self, x: usize, z: usize) -> &mut NoiseColumnBlocks {
        &mut self.columns[z * 16 + x]
    }

    fn layer(&self, x: usize, y: i32, z: usize, min_y: i32) -> Option<&BlockLayer> {
        let index = usize::try_from(y - min_y).ok()?;
        self.column(x, z).blocks.get(index)
    }

    fn set_layer(&mut self, x: usize, y: i32, z: usize, min_y: i32, layer: BlockLayer) {
        if let Ok(index) = usize::try_from(y - min_y)
            && let Some(block) = self.column_mut(x, z).blocks.get_mut(index)
        {
            *block = layer;
        }
    }

    fn biome(&self, section_y: i32, x: usize, y: usize, z: usize) -> &str {
        let section_index = (section_y - WORLD_MIN_SECTION_Y) as usize;
        &self.biomes[((section_index * 4 + x) * 4 + y) * 4 + z]
    }

    fn ocean_floor_wg_height(&self, x: usize, z: usize, min_y: i32) -> i32 {
        self.column(x, z)
            .blocks
            .iter()
            .rposition(is_full_solid_layer)
            .map(|index| min_y + index as i32 + 1)
            .unwrap_or(min_y)
    }

    fn recompute_first_available_heights(&mut self, min_y: i32, height: i32) {
        for column in &mut self.columns {
            let highest = column
                .blocks
                .iter()
                .rposition(|layer| !layer.is_air)
                .map(|index| min_y + index as i32 + 1)
                .unwrap_or(min_y);
            column.first_available_height = (highest - min_y).clamp(0, height);
        }
    }
}

#[derive(Debug, Clone)]
struct NoiseColumnBlocks {
    blocks: Vec<BlockLayer>,
    first_available_height: i32,
}

#[derive(Debug, Clone)]
struct VanillaCarvers {
    seed: i64,
    cave: CaveCarver,
    cave_extra_underground: CaveCarver,
    canyon: CanyonCarver,
}

impl VanillaCarvers {
    fn new(seed: i64) -> Self {
        Self {
            seed,
            cave: CaveCarver::default_cave(),
            cave_extra_underground: CaveCarver::extra_underground(),
            canyon: CanyonCarver::default_overworld(),
        }
    }

    fn carve_chunk(
        &self,
        settings: &NoiseSettings,
        chunk_x: i32,
        chunk_z: i32,
        preliminary_surfaces: &[i32],
        chunk: &mut NoiseChunkBlocks,
    ) {
        let mut mask = CarvingMask::new(settings.height as usize);
        for source_dx in -CARVER_SOURCE_RANGE..=CARVER_SOURCE_RANGE {
            for source_dz in -CARVER_SOURCE_RANGE..=CARVER_SOURCE_RANGE {
                let source_x = chunk_x + source_dx;
                let source_z = chunk_z + source_dz;
                let mut random = JavaRandom::new(0);
                random.set_large_feature_seed(self.seed, source_x, source_z);

                if self.cave.is_start_chunk(&mut random) {
                    self.cave.carve(
                        settings,
                        chunk_x,
                        chunk_z,
                        source_x,
                        source_z,
                        preliminary_surfaces,
                        chunk,
                        &mut mask,
                        &mut random,
                    );
                }

                random.set_large_feature_seed(self.seed + 1, source_x, source_z);
                if self.cave_extra_underground.is_start_chunk(&mut random) {
                    self.cave_extra_underground.carve(
                        settings,
                        chunk_x,
                        chunk_z,
                        source_x,
                        source_z,
                        preliminary_surfaces,
                        chunk,
                        &mut mask,
                        &mut random,
                    );
                }

                random.set_large_feature_seed(self.seed + 2, source_x, source_z);
                if self.canyon.is_start_chunk(&mut random) {
                    self.canyon.carve(
                        settings,
                        chunk_x,
                        chunk_z,
                        source_x,
                        source_z,
                        preliminary_surfaces,
                        chunk,
                        &mut mask,
                        &mut random,
                    );
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct CaveCarver {
    probability: f32,
    min_y: HeightAnchor,
    max_y: HeightAnchor,
    y_scale: UniformFloat,
    lava_level: HeightAnchor,
    horizontal_radius_multiplier: UniformFloat,
    vertical_radius_multiplier: UniformFloat,
    floor_level: UniformFloat,
}

impl CaveCarver {
    fn default_cave() -> Self {
        Self {
            probability: 0.15,
            min_y: HeightAnchor::AboveBottom(8),
            max_y: HeightAnchor::Absolute(180),
            y_scale: UniformFloat::new(0.1, 0.9),
            lava_level: HeightAnchor::AboveBottom(8),
            horizontal_radius_multiplier: UniformFloat::new(0.7, 1.4),
            vertical_radius_multiplier: UniformFloat::new(0.8, 1.3),
            floor_level: UniformFloat::new(-1.0, -0.4),
        }
    }

    fn extra_underground() -> Self {
        Self {
            probability: 0.07,
            min_y: HeightAnchor::AboveBottom(8),
            max_y: HeightAnchor::Absolute(47),
            y_scale: UniformFloat::new(0.1, 0.9),
            lava_level: HeightAnchor::AboveBottom(8),
            horizontal_radius_multiplier: UniformFloat::new(0.7, 1.4),
            vertical_radius_multiplier: UniformFloat::new(0.8, 1.3),
            floor_level: UniformFloat::new(-1.0, -0.4),
        }
    }

    fn is_start_chunk(&self, random: &mut JavaRandom) -> bool {
        random.next_float() <= self.probability
    }

    #[allow(clippy::too_many_arguments)]
    fn carve(
        &self,
        settings: &NoiseSettings,
        chunk_x: i32,
        chunk_z: i32,
        source_x: i32,
        source_z: i32,
        preliminary_surfaces: &[i32],
        chunk: &mut NoiseChunkBlocks,
        mask: &mut CarvingMask,
        random: &mut JavaRandom,
    ) {
        let max_distance = (CARVER_RANGE * 2 - 1) * 16;
        let cave_bound = random.next_int(self.cave_bound()) + 1;
        let cave_bound = random.next_int(cave_bound) + 1;
        let cave_count = random.next_int(cave_bound);

        for _ in 0..cave_count {
            let x = source_x * 16 + random.next_int(16);
            let y = self.sample_y(random, settings) as f64;
            let z = source_z * 16 + random.next_int(16);
            let horizontal_radius_multiplier =
                self.horizontal_radius_multiplier.sample(random) as f64;
            let vertical_radius_multiplier = self.vertical_radius_multiplier.sample(random) as f64;
            let floor_level = self.floor_level.sample(random) as f64;
            let mut tunnels = 1;

            if random.next_int(4) == 0 {
                let y_scale = self.y_scale.sample(random) as f64;
                let thickness = 1.0 + random.next_float() * 6.0;
                self.create_room(
                    settings,
                    chunk_x,
                    chunk_z,
                    x as f64,
                    y,
                    z as f64,
                    thickness,
                    y_scale,
                    floor_level,
                    preliminary_surfaces,
                    chunk,
                    mask,
                );
                tunnels += random.next_int(4);
            }

            for _ in 0..tunnels {
                let horizontal_rotation = random.next_float() * std::f32::consts::TAU;
                let vertical_rotation = (random.next_float() - 0.5) / 4.0;
                let thickness = self.thickness(random);
                let distance = max_distance - random.next_int(max_distance / 4);
                self.create_tunnel(
                    settings,
                    chunk_x,
                    chunk_z,
                    random.next_long(),
                    x as f64,
                    y,
                    z as f64,
                    horizontal_radius_multiplier,
                    vertical_radius_multiplier,
                    thickness,
                    horizontal_rotation,
                    vertical_rotation,
                    0,
                    distance,
                    1.0,
                    floor_level,
                    preliminary_surfaces,
                    chunk,
                    mask,
                );
            }
        }
    }

    fn cave_bound(&self) -> i32 {
        15
    }

    fn thickness(&self, random: &mut JavaRandom) -> f32 {
        let mut thickness = random.next_float() * 2.0 + random.next_float();
        if random.next_int(10) == 0 {
            thickness *= random.next_float() * random.next_float() * 3.0 + 1.0;
        }
        thickness
    }

    fn sample_y(&self, random: &mut JavaRandom, settings: &NoiseSettings) -> i32 {
        let min = self.min_y.resolve(settings);
        let max = self.max_y.resolve(settings);
        if min > max {
            min
        } else {
            min + random.next_int(max - min + 1)
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn create_room(
        &self,
        settings: &NoiseSettings,
        chunk_x: i32,
        chunk_z: i32,
        x: f64,
        y: f64,
        z: f64,
        thickness: f32,
        y_scale: f64,
        floor_level: f64,
        preliminary_surfaces: &[i32],
        chunk: &mut NoiseChunkBlocks,
        mask: &mut CarvingMask,
    ) {
        let horizontal_radius = 1.5 + (std::f32::consts::FRAC_PI_2).sin() as f64 * thickness as f64;
        let vertical_radius = horizontal_radius * y_scale;
        carve_ellipsoid(
            settings,
            chunk_x,
            chunk_z,
            x + 1.0,
            y,
            z,
            horizontal_radius,
            vertical_radius,
            self.lava_level.resolve(settings),
            preliminary_surfaces,
            chunk,
            mask,
            |_, xd, yd, zd, _| cave_should_skip(xd, yd, zd, floor_level),
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn create_tunnel(
        &self,
        settings: &NoiseSettings,
        chunk_x: i32,
        chunk_z: i32,
        tunnel_seed: i64,
        mut x: f64,
        mut y: f64,
        mut z: f64,
        horizontal_radius_multiplier: f64,
        vertical_radius_multiplier: f64,
        thickness: f32,
        mut horizontal_rotation: f32,
        mut vertical_rotation: f32,
        step: i32,
        distance: i32,
        y_scale: f64,
        floor_level: f64,
        preliminary_surfaces: &[i32],
        chunk: &mut NoiseChunkBlocks,
        mask: &mut CarvingMask,
    ) {
        if distance <= 0 {
            return;
        }

        let mut random = JavaRandom::new(tunnel_seed);
        let split_point = random.next_int(distance / 2) + distance / 4;
        let steep = random.next_int(6) == 0;
        let mut y_rota = 0.0_f32;
        let mut x_rota = 0.0_f32;

        for current_step in step..distance {
            let horizontal_radius = 1.5
                + (std::f32::consts::PI * current_step as f32 / distance as f32).sin() as f64
                    * thickness as f64;
            let vertical_radius = horizontal_radius * y_scale;
            let cos_x = vertical_rotation.cos();
            x += horizontal_rotation.cos() as f64 * cos_x as f64;
            y += vertical_rotation.sin() as f64;
            z += horizontal_rotation.sin() as f64 * cos_x as f64;
            vertical_rotation *= if steep { 0.92 } else { 0.7 };
            vertical_rotation += x_rota * 0.1;
            horizontal_rotation += y_rota * 0.1;
            x_rota *= 0.9;
            y_rota *= 0.75;
            x_rota += (random.next_float() - random.next_float()) * random.next_float() * 2.0;
            y_rota += (random.next_float() - random.next_float()) * random.next_float() * 4.0;

            if current_step == split_point && thickness > 1.0 {
                self.create_tunnel(
                    settings,
                    chunk_x,
                    chunk_z,
                    random.next_long(),
                    x,
                    y,
                    z,
                    horizontal_radius_multiplier,
                    vertical_radius_multiplier,
                    random.next_float() * 0.5 + 0.5,
                    horizontal_rotation - std::f32::consts::FRAC_PI_2,
                    vertical_rotation / 3.0,
                    current_step,
                    distance,
                    1.0,
                    floor_level,
                    preliminary_surfaces,
                    chunk,
                    mask,
                );
                self.create_tunnel(
                    settings,
                    chunk_x,
                    chunk_z,
                    random.next_long(),
                    x,
                    y,
                    z,
                    horizontal_radius_multiplier,
                    vertical_radius_multiplier,
                    random.next_float() * 0.5 + 0.5,
                    horizontal_rotation + std::f32::consts::FRAC_PI_2,
                    vertical_rotation / 3.0,
                    current_step,
                    distance,
                    1.0,
                    floor_level,
                    preliminary_surfaces,
                    chunk,
                    mask,
                );
                return;
            }

            if random.next_int(4) != 0 {
                if !carver_can_reach(chunk_x, chunk_z, x, z, current_step, distance, thickness) {
                    return;
                }

                carve_ellipsoid(
                    settings,
                    chunk_x,
                    chunk_z,
                    x,
                    y,
                    z,
                    horizontal_radius * horizontal_radius_multiplier,
                    vertical_radius * vertical_radius_multiplier,
                    self.lava_level.resolve(settings),
                    preliminary_surfaces,
                    chunk,
                    mask,
                    |_, xd, yd, zd, _| cave_should_skip(xd, yd, zd, floor_level),
                );
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct CanyonCarver {
    probability: f32,
    min_y: HeightAnchor,
    max_y: HeightAnchor,
    y_scale: ConstantFloat,
    lava_level: HeightAnchor,
    vertical_rotation: UniformFloat,
    distance_factor: UniformFloat,
    thickness: TrapezoidFloat,
    width_smoothness: i32,
    horizontal_radius_factor: UniformFloat,
    vertical_radius_default_factor: f32,
    vertical_radius_center_factor: f32,
}

impl CanyonCarver {
    fn default_overworld() -> Self {
        Self {
            probability: 0.01,
            min_y: HeightAnchor::Absolute(10),
            max_y: HeightAnchor::Absolute(67),
            y_scale: ConstantFloat(3.0),
            lava_level: HeightAnchor::AboveBottom(8),
            vertical_rotation: UniformFloat::new(-0.125, 0.125),
            distance_factor: UniformFloat::new(0.75, 1.0),
            thickness: TrapezoidFloat {
                min: 0.0,
                max: 6.0,
                plateau: 2.0,
            },
            width_smoothness: 3,
            horizontal_radius_factor: UniformFloat::new(0.75, 1.0),
            vertical_radius_default_factor: 1.0,
            vertical_radius_center_factor: 0.0,
        }
    }

    fn is_start_chunk(&self, random: &mut JavaRandom) -> bool {
        random.next_float() <= self.probability
    }

    #[allow(clippy::too_many_arguments)]
    fn carve(
        &self,
        settings: &NoiseSettings,
        chunk_x: i32,
        chunk_z: i32,
        source_x: i32,
        source_z: i32,
        preliminary_surfaces: &[i32],
        chunk: &mut NoiseChunkBlocks,
        mask: &mut CarvingMask,
        random: &mut JavaRandom,
    ) {
        let max_distance = (CARVER_RANGE * 2 - 1) * 16;
        let x = source_x * 16 + random.next_int(16);
        let y = self.sample_y(random, settings);
        let z = source_z * 16 + random.next_int(16);
        let horizontal_rotation = random.next_float() * std::f32::consts::TAU;
        let vertical_rotation = self.vertical_rotation.sample(random);
        let y_scale = self.y_scale.sample(random) as f64;
        let thickness = self.thickness.sample(random);
        let distance = (max_distance as f32 * self.distance_factor.sample(random)) as i32;

        self.do_carve(
            settings,
            chunk_x,
            chunk_z,
            random.next_long(),
            x as f64,
            y as f64,
            z as f64,
            thickness,
            horizontal_rotation,
            vertical_rotation,
            0,
            distance,
            y_scale,
            preliminary_surfaces,
            chunk,
            mask,
        );
    }

    fn sample_y(&self, random: &mut JavaRandom, settings: &NoiseSettings) -> i32 {
        let min = self.min_y.resolve(settings);
        let max = self.max_y.resolve(settings);
        if min > max {
            min
        } else {
            min + random.next_int(max - min + 1)
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn do_carve(
        &self,
        settings: &NoiseSettings,
        chunk_x: i32,
        chunk_z: i32,
        tunnel_seed: i64,
        mut x: f64,
        mut y: f64,
        mut z: f64,
        thickness: f32,
        mut horizontal_rotation: f32,
        mut vertical_rotation: f32,
        step: i32,
        distance: i32,
        y_scale: f64,
        preliminary_surfaces: &[i32],
        chunk: &mut NoiseChunkBlocks,
        mask: &mut CarvingMask,
    ) {
        if distance <= 0 {
            return;
        }

        let mut random = JavaRandom::new(tunnel_seed);
        let width_factors = self.init_width_factors(settings, &mut random);
        let mut y_rota = 0.0_f32;
        let mut x_rota = 0.0_f32;

        for current_step in step..distance {
            let mut horizontal_radius = 1.5
                + (current_step as f32 * std::f32::consts::PI / distance as f32).sin() as f64
                    * thickness as f64;
            let mut vertical_radius = horizontal_radius * y_scale;
            horizontal_radius *= self.horizontal_radius_factor.sample(&mut random) as f64;
            vertical_radius =
                self.update_vertical_radius(&mut random, vertical_radius, distance, current_step);

            let x_cos = vertical_rotation.cos();
            x += horizontal_rotation.cos() as f64 * x_cos as f64;
            y += vertical_rotation.sin() as f64;
            z += horizontal_rotation.sin() as f64 * x_cos as f64;
            vertical_rotation *= 0.7;
            vertical_rotation += x_rota * 0.05;
            horizontal_rotation += y_rota * 0.05;
            x_rota *= 0.8;
            y_rota *= 0.5;
            x_rota += (random.next_float() - random.next_float()) * random.next_float() * 2.0;
            y_rota += (random.next_float() - random.next_float()) * random.next_float() * 4.0;

            if random.next_int(4) != 0 {
                if !carver_can_reach(chunk_x, chunk_z, x, z, current_step, distance, thickness) {
                    return;
                }

                carve_ellipsoid(
                    settings,
                    chunk_x,
                    chunk_z,
                    x,
                    y,
                    z,
                    horizontal_radius,
                    vertical_radius,
                    self.lava_level.resolve(settings),
                    preliminary_surfaces,
                    chunk,
                    mask,
                    |settings, xd, yd, zd, world_y| {
                        canyon_should_skip(settings, &width_factors, xd, yd, zd, world_y)
                    },
                );
            }
        }
    }

    fn init_width_factors(&self, settings: &NoiseSettings, random: &mut JavaRandom) -> Vec<f32> {
        let mut width_factors = vec![1.0; settings.height as usize];
        let mut width_factor = 1.0_f32;
        for (y_index, factor) in width_factors.iter_mut().enumerate() {
            if y_index == 0 || random.next_int(self.width_smoothness) == 0 {
                width_factor = 1.0 + random.next_float() * random.next_float();
            }
            *factor = width_factor * width_factor;
        }
        width_factors
    }

    fn update_vertical_radius(
        &self,
        random: &mut JavaRandom,
        vertical_radius: f64,
        distance: i32,
        current_step: i32,
    ) -> f64 {
        let vertical_multiplier = 1.0 - (0.5 - current_step as f32 / distance as f32).abs() * 2.0;
        let factor = self.vertical_radius_default_factor
            + self.vertical_radius_center_factor * vertical_multiplier;
        factor as f64 * vertical_radius * random_between(random, 0.75, 1.0) as f64
    }
}

#[allow(clippy::too_many_arguments)]
fn carve_ellipsoid<F>(
    settings: &NoiseSettings,
    chunk_x: i32,
    chunk_z: i32,
    x: f64,
    y: f64,
    z: f64,
    horizontal_radius: f64,
    vertical_radius: f64,
    lava_level: i32,
    preliminary_surfaces: &[i32],
    chunk: &mut NoiseChunkBlocks,
    mask: &mut CarvingMask,
    should_skip: F,
) -> bool
where
    F: Fn(&NoiseSettings, f64, f64, f64, i32) -> bool,
{
    if horizontal_radius <= 0.0 || vertical_radius <= 0.0 {
        return false;
    }

    let center_x = chunk_x * 16 + 8;
    let center_z = chunk_z * 16 + 8;
    let max_delta = 16.0 + horizontal_radius * 2.0;
    if (x - center_x as f64).abs() > max_delta || (z - center_z as f64).abs() > max_delta {
        return false;
    }

    let chunk_min_x = chunk_x * 16;
    let chunk_min_z = chunk_z * 16;
    let min_x = ((x - horizontal_radius).floor() as i32 - chunk_min_x - 1).max(0);
    let max_x = ((x + horizontal_radius).floor() as i32 - chunk_min_x).min(15);
    let min_y = ((y - vertical_radius).floor() as i32 - 1).max(settings.min_y + 1);
    let max_y =
        ((y + vertical_radius).floor() as i32 + 1).min(settings.min_y + settings.height - 1 - 7);
    let min_z = ((z - horizontal_radius).floor() as i32 - chunk_min_z - 1).max(0);
    let max_z = ((z + horizontal_radius).floor() as i32 - chunk_min_z).min(15);
    let mut carved = false;

    for local_x in min_x..=max_x {
        let world_x = chunk_min_x + local_x;
        let xd = (world_x as f64 + 0.5 - x) / horizontal_radius;

        for local_z in min_z..=max_z {
            let world_z = chunk_min_z + local_z;
            let zd = (world_z as f64 + 0.5 - z) / horizontal_radius;
            if xd * xd + zd * zd >= 1.0 {
                continue;
            }

            let mut has_grass = false;
            for world_y in (min_y + 1..=max_y).rev() {
                let yd = (world_y as f64 - 0.5 - y) / vertical_radius;
                if should_skip(settings, xd, yd, zd, world_y)
                    || mask.get(local_x as usize, world_y, local_z as usize, settings.min_y)
                {
                    continue;
                }

                mask.set(local_x as usize, world_y, local_z as usize, settings.min_y);
                if carve_block(
                    settings,
                    chunk,
                    preliminary_surfaces,
                    world_x,
                    local_x as usize,
                    world_y,
                    world_z,
                    local_z as usize,
                    lava_level,
                    &mut has_grass,
                ) {
                    carved = true;
                }
            }
        }
    }

    carved
}

fn carve_block(
    settings: &NoiseSettings,
    chunk: &mut NoiseChunkBlocks,
    preliminary_surfaces: &[i32],
    world_x: i32,
    local_x: usize,
    world_y: i32,
    world_z: i32,
    local_z: usize,
    lava_level: i32,
    has_grass: &mut bool,
) -> bool {
    let Some(current) = chunk.layer(local_x, world_y, local_z, settings.min_y) else {
        return false;
    };
    if current.is("minecraft:grass_block") || current.is("minecraft:mycelium") {
        *has_grass = true;
    }
    if !is_overworld_carver_replaceable(current) {
        return false;
    }

    let Some(layer) = carve_layer(
        settings,
        world_x,
        world_y,
        world_z,
        preliminary_surfaces[local_z * 16 + local_x],
        lava_level,
    ) else {
        return false;
    };
    chunk.set_layer(local_x, world_y, local_z, settings.min_y, layer.clone());

    if *has_grass
        && world_y > settings.min_y
        && chunk
            .layer(local_x, world_y - 1, local_z, settings.min_y)
            .is_some_and(|below| below.is("minecraft:dirt"))
    {
        chunk.set_layer(
            local_x,
            world_y - 1,
            local_z,
            settings.min_y,
            settings.surface_block.clone(),
        );
    }

    true
}

fn carve_layer(
    settings: &NoiseSettings,
    world_x: i32,
    world_y: i32,
    world_z: i32,
    preliminary_surface: i32,
    lava_level: i32,
) -> Option<BlockLayer> {
    if world_y <= lava_level {
        Some(settings.lava_block.clone())
    } else {
        match settings
            .aquifer
            .substance_at(world_x, world_y, world_z, 0.0, preliminary_surface)
        {
            vanilla_noise::AquiferSubstance::Fluid(vanilla_noise::AquiferFluid::Water) => {
                Some(settings.default_fluid.clone())
            }
            vanilla_noise::AquiferSubstance::Fluid(vanilla_noise::AquiferFluid::Lava) => {
                Some(settings.lava_block.clone())
            }
            vanilla_noise::AquiferSubstance::Fluid(vanilla_noise::AquiferFluid::Air) => {
                Some(settings.air_layer())
            }
            vanilla_noise::AquiferSubstance::DefaultBlock => None,
        }
    }
}

fn is_overworld_carver_replaceable(layer: &BlockLayer) -> bool {
    matches!(
        layer.block.as_str(),
        "minecraft:stone"
            | "minecraft:granite"
            | "minecraft:diorite"
            | "minecraft:andesite"
            | "minecraft:tuff"
            | "minecraft:deepslate"
            | "minecraft:dirt"
            | "minecraft:coarse_dirt"
            | "minecraft:rooted_dirt"
            | "minecraft:mud"
            | "minecraft:muddy_mangrove_roots"
            | "minecraft:moss_block"
            | "minecraft:pale_moss_block"
            | "minecraft:grass_block"
            | "minecraft:podzol"
            | "minecraft:mycelium"
            | "minecraft:sand"
            | "minecraft:red_sand"
            | "minecraft:suspicious_sand"
            | "minecraft:terracotta"
            | "minecraft:white_terracotta"
            | "minecraft:orange_terracotta"
            | "minecraft:magenta_terracotta"
            | "minecraft:light_blue_terracotta"
            | "minecraft:yellow_terracotta"
            | "minecraft:lime_terracotta"
            | "minecraft:pink_terracotta"
            | "minecraft:gray_terracotta"
            | "minecraft:light_gray_terracotta"
            | "minecraft:cyan_terracotta"
            | "minecraft:purple_terracotta"
            | "minecraft:blue_terracotta"
            | "minecraft:brown_terracotta"
            | "minecraft:green_terracotta"
            | "minecraft:red_terracotta"
            | "minecraft:black_terracotta"
            | "minecraft:iron_ore"
            | "minecraft:deepslate_iron_ore"
            | "minecraft:copper_ore"
            | "minecraft:deepslate_copper_ore"
            | "minecraft:snow"
            | "minecraft:snow_block"
            | "minecraft:powder_snow"
            | "minecraft:water"
            | "minecraft:gravel"
            | "minecraft:suspicious_gravel"
            | "minecraft:sandstone"
            | "minecraft:red_sandstone"
            | "minecraft:calcite"
            | "minecraft:packed_ice"
            | "minecraft:raw_iron_block"
            | "minecraft:raw_copper_block"
    )
}

fn cave_should_skip(xd: f64, yd: f64, zd: f64, floor_level: f64) -> bool {
    yd <= floor_level || xd * xd + yd * yd + zd * zd >= 1.0
}

fn canyon_should_skip(
    settings: &NoiseSettings,
    width_factors: &[f32],
    xd: f64,
    yd: f64,
    zd: f64,
    world_y: i32,
) -> bool {
    let y_index = usize::try_from(world_y - settings.min_y - 1).unwrap_or(0);
    let width_factor = width_factors
        .get(y_index.min(width_factors.len().saturating_sub(1)))
        .copied()
        .unwrap_or(1.0) as f64;
    (xd * xd + zd * zd) * width_factor + yd * yd / 6.0 >= 1.0
}

fn carver_can_reach(
    chunk_x: i32,
    chunk_z: i32,
    x: f64,
    z: f64,
    current_step: i32,
    total_steps: i32,
    thickness: f32,
) -> bool {
    let x_mid = chunk_x * 16 + 8;
    let z_mid = chunk_z * 16 + 8;
    let xd = x - x_mid as f64;
    let zd = z - z_mid as f64;
    let remaining = (total_steps - current_step) as f64;
    let radius = thickness as f64 + 2.0 + 16.0;
    xd * xd + zd * zd - remaining * remaining <= radius * radius
}

#[derive(Clone, Debug)]
struct CarvingMask {
    height: usize,
    values: Vec<bool>,
}

impl CarvingMask {
    fn new(height: usize) -> Self {
        Self {
            height,
            values: vec![false; 16 * height * 16],
        }
    }

    fn get(&self, x: usize, y: i32, z: usize, min_y: i32) -> bool {
        self.index(x, y, z, min_y)
            .and_then(|index| self.values.get(index))
            .copied()
            .unwrap_or(false)
    }

    fn set(&mut self, x: usize, y: i32, z: usize, min_y: i32) {
        if let Some(index) = self.index(x, y, z, min_y)
            && let Some(value) = self.values.get_mut(index)
        {
            *value = true;
        }
    }

    fn index(&self, x: usize, y: i32, z: usize, min_y: i32) -> Option<usize> {
        if x >= 16 || z >= 16 {
            return None;
        }
        let y = usize::try_from(y - min_y).ok()?;
        (y < self.height).then_some((y * 16 + z) * 16 + x)
    }
}

#[derive(Clone, Copy, Debug)]
enum HeightAnchor {
    Absolute(i32),
    AboveBottom(i32),
    BelowTop(i32),
}

impl HeightAnchor {
    fn resolve(self, settings: &NoiseSettings) -> i32 {
        match self {
            Self::Absolute(y) => y,
            Self::AboveBottom(offset) => settings.min_y + offset,
            Self::BelowTop(offset) => settings.min_y + settings.height - 1 - offset,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct UniformFloat {
    min: f32,
    max: f32,
}

impl UniformFloat {
    fn new(min: f32, max: f32) -> Self {
        Self { min, max }
    }

    fn sample(self, random: &mut JavaRandom) -> f32 {
        random_between(random, self.min, self.max)
    }
}

#[derive(Clone, Copy, Debug)]
struct ConstantFloat(f32);

impl ConstantFloat {
    fn sample(self, _random: &mut JavaRandom) -> f32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug)]
struct TrapezoidFloat {
    min: f32,
    max: f32,
    plateau: f32,
}

impl TrapezoidFloat {
    fn sample(self, random: &mut JavaRandom) -> f32 {
        let range = self.max - self.min;
        let plateau_start = (range - self.plateau) / 2.0;
        let plateau_end = range - plateau_start;
        self.min + random.next_float() * plateau_end + random.next_float() * plateau_start
    }
}

fn random_between(random: &mut JavaRandom, min: f32, max_exclusive: f32) -> f32 {
    random.next_float() * (max_exclusive - min) + min
}

#[derive(Clone, Debug)]
struct JavaRandom {
    seed: u64,
}

impl JavaRandom {
    const MULTIPLIER: u64 = 25_214_903_917;
    const ADDEND: u64 = 11;
    const MASK: u64 = (1_u64 << 48) - 1;

    fn new(seed: i64) -> Self {
        let mut random = Self { seed: 0 };
        random.set_seed(seed);
        random
    }

    fn set_seed(&mut self, seed: i64) {
        self.seed = ((seed as u64) ^ Self::MULTIPLIER) & Self::MASK;
    }

    fn next(&mut self, bits: u32) -> i32 {
        self.seed = self
            .seed
            .wrapping_mul(Self::MULTIPLIER)
            .wrapping_add(Self::ADDEND)
            & Self::MASK;
        (self.seed >> (48 - bits)) as i32
    }

    fn next_int(&mut self, bound: i32) -> i32 {
        assert!(bound > 0);
        if (bound & -bound) == bound {
            return (((bound as i64) * (self.next(31) as i64)) >> 31) as i32;
        }

        loop {
            let sample = self.next(31);
            let modulo = sample % bound;
            if sample.wrapping_sub(modulo).wrapping_add(bound - 1) >= 0 {
                return modulo;
            }
        }
    }

    fn next_long(&mut self) -> i64 {
        let upper = self.next(32) as i64;
        let lower = self.next(32) as i64;
        (upper << 32).wrapping_add(lower)
    }

    fn next_float(&mut self) -> f32 {
        self.next(24) as f32 * 5.960_464_5e-8_f32
    }

    fn set_large_feature_seed(&mut self, world_seed: i64, chunk_x: i32, chunk_z: i32) {
        self.set_seed(world_seed);
        let x_scale = self.next_long();
        let z_scale = self.next_long();
        let seed = ((chunk_x as i64).wrapping_mul(x_scale))
            ^ ((chunk_z as i64).wrapping_mul(z_scale))
            ^ world_seed;
        self.set_seed(seed);
    }
}

#[derive(Debug, Clone)]
struct OverworldOreFeatures {
    seed: i64,
    features: Vec<PlacedOreFeature>,
    underwater_magma: PlacedUnderwaterMagmaFeature,
    disks: Vec<PlacedDiskFeature>,
    springs: Vec<PlacedSpringFeature>,
}

impl OverworldOreFeatures {
    fn new(seed: i64) -> Self {
        let dirt = OreFeatureConfig::base_stone(33, "minecraft:dirt");
        let gravel = OreFeatureConfig::base_stone(33, "minecraft:gravel");
        let granite = OreFeatureConfig::base_stone(64, "minecraft:granite");
        let diorite = OreFeatureConfig::base_stone(64, "minecraft:diorite");
        let andesite = OreFeatureConfig::base_stone(64, "minecraft:andesite");
        let tuff = OreFeatureConfig::base_stone(64, "minecraft:tuff");
        let coal = OreFeatureConfig::new(
            17,
            0.0,
            "minecraft:coal_ore",
            "minecraft:deepslate_coal_ore",
        );
        let coal_buried = OreFeatureConfig::new(
            17,
            0.5,
            "minecraft:coal_ore",
            "minecraft:deepslate_coal_ore",
        );
        let iron =
            OreFeatureConfig::new(9, 0.0, "minecraft:iron_ore", "minecraft:deepslate_iron_ore");
        let iron_small =
            OreFeatureConfig::new(4, 0.0, "minecraft:iron_ore", "minecraft:deepslate_iron_ore");
        let gold =
            OreFeatureConfig::new(9, 0.5, "minecraft:gold_ore", "minecraft:deepslate_gold_ore");
        let redstone = OreFeatureConfig::new(
            8,
            0.0,
            "minecraft:redstone_ore",
            "minecraft:deepslate_redstone_ore",
        );
        let diamond_small = OreFeatureConfig::new(
            4,
            0.5,
            "minecraft:diamond_ore",
            "minecraft:deepslate_diamond_ore",
        );
        let diamond_medium = OreFeatureConfig::new(
            8,
            0.5,
            "minecraft:diamond_ore",
            "minecraft:deepslate_diamond_ore",
        );
        let diamond_large = OreFeatureConfig::new(
            12,
            0.7,
            "minecraft:diamond_ore",
            "minecraft:deepslate_diamond_ore",
        );
        let diamond_buried = OreFeatureConfig::new(
            8,
            1.0,
            "minecraft:diamond_ore",
            "minecraft:deepslate_diamond_ore",
        );
        let lapis = OreFeatureConfig::new(
            7,
            0.0,
            "minecraft:lapis_ore",
            "minecraft:deepslate_lapis_ore",
        );
        let lapis_buried = OreFeatureConfig::new(
            7,
            1.0,
            "minecraft:lapis_ore",
            "minecraft:deepslate_lapis_ore",
        );
        let copper = OreFeatureConfig::new(
            10,
            0.0,
            "minecraft:copper_ore",
            "minecraft:deepslate_copper_ore",
        );
        let copper_large = OreFeatureConfig::new(
            20,
            0.0,
            "minecraft:copper_ore",
            "minecraft:deepslate_copper_ore",
        );
        let emerald = OreFeatureConfig::new(
            3,
            0.0,
            "minecraft:emerald_ore",
            "minecraft:deepslate_emerald_ore",
        );
        let infested = OreFeatureConfig::new(
            9,
            0.0,
            "minecraft:infested_stone",
            "minecraft:infested_deepslate",
        );

        Self {
            seed,
            features: vec![
                PlacedOreFeature::new(
                    0,
                    OrePlacementCount::Constant(7),
                    OreHeight::Uniform(HeightAnchor::Absolute(0), HeightAnchor::Absolute(160)),
                    dirt,
                ),
                PlacedOreFeature::new(
                    1,
                    OrePlacementCount::Constant(14),
                    OreHeight::Uniform(HeightAnchor::AboveBottom(0), HeightAnchor::BelowTop(0)),
                    gravel,
                ),
                PlacedOreFeature::new(
                    2,
                    OrePlacementCount::Rarity(6),
                    OreHeight::Uniform(HeightAnchor::Absolute(64), HeightAnchor::Absolute(128)),
                    granite.clone(),
                ),
                PlacedOreFeature::new(
                    3,
                    OrePlacementCount::Constant(2),
                    OreHeight::Uniform(HeightAnchor::Absolute(0), HeightAnchor::Absolute(60)),
                    granite,
                ),
                PlacedOreFeature::new(
                    4,
                    OrePlacementCount::Rarity(6),
                    OreHeight::Uniform(HeightAnchor::Absolute(64), HeightAnchor::Absolute(128)),
                    diorite.clone(),
                ),
                PlacedOreFeature::new(
                    5,
                    OrePlacementCount::Constant(2),
                    OreHeight::Uniform(HeightAnchor::Absolute(0), HeightAnchor::Absolute(60)),
                    diorite,
                ),
                PlacedOreFeature::new(
                    6,
                    OrePlacementCount::Rarity(6),
                    OreHeight::Uniform(HeightAnchor::Absolute(64), HeightAnchor::Absolute(128)),
                    andesite.clone(),
                ),
                PlacedOreFeature::new(
                    7,
                    OrePlacementCount::Constant(2),
                    OreHeight::Uniform(HeightAnchor::Absolute(0), HeightAnchor::Absolute(60)),
                    andesite,
                ),
                PlacedOreFeature::new(
                    8,
                    OrePlacementCount::Constant(2),
                    OreHeight::Uniform(HeightAnchor::AboveBottom(0), HeightAnchor::Absolute(0)),
                    tuff,
                ),
                PlacedOreFeature::new(
                    9,
                    OrePlacementCount::Constant(30),
                    OreHeight::Uniform(HeightAnchor::Absolute(136), HeightAnchor::BelowTop(0)),
                    coal,
                ),
                PlacedOreFeature::new(
                    10,
                    OrePlacementCount::Constant(20),
                    OreHeight::Trapezoid(HeightAnchor::Absolute(0), HeightAnchor::Absolute(192)),
                    coal_buried,
                ),
                PlacedOreFeature::new(
                    11,
                    OrePlacementCount::Constant(90),
                    OreHeight::Trapezoid(HeightAnchor::Absolute(80), HeightAnchor::Absolute(384)),
                    iron.clone(),
                ),
                PlacedOreFeature::new(
                    12,
                    OrePlacementCount::Constant(10),
                    OreHeight::Trapezoid(HeightAnchor::Absolute(-24), HeightAnchor::Absolute(56)),
                    iron,
                ),
                PlacedOreFeature::new(
                    13,
                    OrePlacementCount::Constant(10),
                    OreHeight::Uniform(HeightAnchor::AboveBottom(0), HeightAnchor::Absolute(72)),
                    iron_small,
                ),
                PlacedOreFeature::new(
                    14,
                    OrePlacementCount::Constant(4),
                    OreHeight::Trapezoid(HeightAnchor::Absolute(-64), HeightAnchor::Absolute(32)),
                    gold.clone(),
                ),
                PlacedOreFeature::new(
                    15,
                    OrePlacementCount::Uniform { min: 0, max: 1 },
                    OreHeight::Uniform(HeightAnchor::Absolute(-64), HeightAnchor::Absolute(-48)),
                    gold.clone(),
                ),
                PlacedOreFeature::new(
                    16,
                    OrePlacementCount::Constant(4),
                    OreHeight::Uniform(HeightAnchor::AboveBottom(0), HeightAnchor::Absolute(15)),
                    redstone.clone(),
                ),
                PlacedOreFeature::new(
                    17,
                    OrePlacementCount::Constant(8),
                    OreHeight::Trapezoid(
                        HeightAnchor::AboveBottom(-32),
                        HeightAnchor::AboveBottom(32),
                    ),
                    redstone,
                ),
                PlacedOreFeature::new(
                    18,
                    OrePlacementCount::Constant(7),
                    OreHeight::Trapezoid(
                        HeightAnchor::AboveBottom(-80),
                        HeightAnchor::AboveBottom(80),
                    ),
                    diamond_small,
                ),
                PlacedOreFeature::new(
                    19,
                    OrePlacementCount::Constant(2),
                    OreHeight::Uniform(HeightAnchor::Absolute(-64), HeightAnchor::Absolute(-4)),
                    diamond_medium,
                ),
                PlacedOreFeature::new(
                    20,
                    OrePlacementCount::Rarity(9),
                    OreHeight::Trapezoid(
                        HeightAnchor::AboveBottom(-80),
                        HeightAnchor::AboveBottom(80),
                    ),
                    diamond_large,
                ),
                PlacedOreFeature::new(
                    21,
                    OrePlacementCount::Constant(4),
                    OreHeight::Trapezoid(
                        HeightAnchor::AboveBottom(-80),
                        HeightAnchor::AboveBottom(80),
                    ),
                    diamond_buried,
                ),
                PlacedOreFeature::new(
                    22,
                    OrePlacementCount::Constant(2),
                    OreHeight::Trapezoid(HeightAnchor::Absolute(-32), HeightAnchor::Absolute(32)),
                    lapis,
                ),
                PlacedOreFeature::new(
                    23,
                    OrePlacementCount::Constant(4),
                    OreHeight::Uniform(HeightAnchor::AboveBottom(0), HeightAnchor::Absolute(64)),
                    lapis_buried,
                ),
                PlacedOreFeature::new(
                    24,
                    OrePlacementCount::Constant(16),
                    OreHeight::Trapezoid(HeightAnchor::Absolute(-16), HeightAnchor::Absolute(112)),
                    copper,
                )
                .with_biome_filter(FeatureBiomeFilter::Exclude(DRIPSTONE_CAVES_ORE_BIOMES)),
                PlacedOreFeature::new(
                    24,
                    OrePlacementCount::Constant(16),
                    OreHeight::Trapezoid(HeightAnchor::Absolute(-16), HeightAnchor::Absolute(112)),
                    copper_large,
                )
                .with_biome_filter(FeatureBiomeFilter::Include(DRIPSTONE_CAVES_ORE_BIOMES)),
                PlacedOreFeature::new(
                    26,
                    OrePlacementCount::Constant(50),
                    OreHeight::Uniform(HeightAnchor::Absolute(32), HeightAnchor::Absolute(256)),
                    gold,
                )
                .with_biome_filter(FeatureBiomeFilter::Include(BADLANDS_ORE_BIOMES)),
                PlacedOreFeature::new(
                    29,
                    OrePlacementCount::Constant(100),
                    OreHeight::Trapezoid(HeightAnchor::Absolute(-16), HeightAnchor::Absolute(480)),
                    emerald,
                )
                .with_biome_filter(FeatureBiomeFilter::Include(EMERALD_ORE_BIOMES)),
                PlacedOreFeature::new(
                    0,
                    OrePlacementCount::Constant(14),
                    OreHeight::Uniform(HeightAnchor::AboveBottom(0), HeightAnchor::Absolute(63)),
                    infested,
                )
                .with_step_index(7)
                .with_biome_filter(FeatureBiomeFilter::Include(EMERALD_ORE_BIOMES)),
            ],
            underwater_magma: PlacedUnderwaterMagmaFeature::new(25),
            disks: vec![
                PlacedDiskFeature::sand(26)
                    .with_biome_filter(FeatureBiomeFilter::Exclude(BADLANDS_ORE_BIOMES)),
                PlacedDiskFeature::sand(27)
                    .with_biome_filter(FeatureBiomeFilter::Include(BADLANDS_ORE_BIOMES)),
                PlacedDiskFeature::clay(27)
                    .with_biome_filter(FeatureBiomeFilter::Exclude(BADLANDS_ORE_BIOMES)),
                PlacedDiskFeature::clay(28)
                    .with_biome_filter(FeatureBiomeFilter::Include(BADLANDS_ORE_BIOMES)),
                PlacedDiskFeature::gravel(28)
                    .with_biome_filter(FeatureBiomeFilter::Exclude(BADLANDS_ORE_BIOMES)),
                PlacedDiskFeature::gravel(29)
                    .with_biome_filter(FeatureBiomeFilter::Include(BADLANDS_ORE_BIOMES)),
            ],
            springs: vec![
                PlacedSpringFeature::water(0),
                PlacedSpringFeature::lava_overworld(1),
            ],
        }
    }

    fn place_chunk(
        &self,
        settings: &NoiseSettings,
        chunk_x: i32,
        chunk_z: i32,
        chunk: &mut NoiseChunkBlocks,
    ) {
        let origin_x = chunk_x * 16;
        let origin_z = chunk_z * 16;
        let decoration_seed = FeatureRandom::decoration_seed(self.seed, origin_x, origin_z);

        let mut features =
            Vec::with_capacity(self.features.len() + self.disks.len() + self.springs.len() + 1);
        features.extend(self.features.iter().map(PlacedUndergroundFeature::Ore));
        features.push(PlacedUndergroundFeature::UnderwaterMagma(
            &self.underwater_magma,
        ));
        features.extend(self.disks.iter().map(PlacedUndergroundFeature::Disk));
        features.extend(self.springs.iter().map(PlacedUndergroundFeature::Spring));
        features.sort_by_key(|feature| (feature.step_index(), feature.feature_index()));

        for feature in features {
            let mut random = FeatureRandom::for_feature(
                decoration_seed,
                feature.feature_index(),
                feature.step_index(),
            );
            feature.place(settings, origin_x, origin_z, chunk, &mut random);
        }
    }
}

#[derive(Clone, Copy)]
enum PlacedUndergroundFeature<'a> {
    Ore(&'a PlacedOreFeature),
    UnderwaterMagma(&'a PlacedUnderwaterMagmaFeature),
    Disk(&'a PlacedDiskFeature),
    Spring(&'a PlacedSpringFeature),
}

impl PlacedUndergroundFeature<'_> {
    fn step_index(self) -> i32 {
        match self {
            Self::Ore(feature) => feature.step_index,
            Self::UnderwaterMagma(feature) => feature.step_index,
            Self::Disk(feature) => feature.step_index,
            Self::Spring(feature) => feature.step_index,
        }
    }

    fn feature_index(self) -> i32 {
        match self {
            Self::Ore(feature) => feature.feature_index,
            Self::UnderwaterMagma(feature) => feature.feature_index,
            Self::Disk(feature) => feature.feature_index,
            Self::Spring(feature) => feature.feature_index,
        }
    }

    fn place(
        self,
        settings: &NoiseSettings,
        origin_x: i32,
        origin_z: i32,
        chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
    ) {
        match self {
            Self::Ore(feature) => feature.place(settings, origin_x, origin_z, chunk, random),
            Self::UnderwaterMagma(feature) => {
                feature.place(settings, origin_x, origin_z, chunk, random);
            }
            Self::Disk(feature) => feature.place(settings, origin_x, origin_z, chunk, random),
            Self::Spring(feature) => feature.place(settings, origin_x, origin_z, chunk, random),
        }
    }
}

#[derive(Debug, Clone)]
struct PlacedOreFeature {
    step_index: i32,
    feature_index: i32,
    count: OrePlacementCount,
    height: OreHeight,
    ore: OreFeatureConfig,
    biome_filter: FeatureBiomeFilter,
}

impl PlacedOreFeature {
    fn new(
        feature_index: i32,
        count: OrePlacementCount,
        height: OreHeight,
        ore: OreFeatureConfig,
    ) -> Self {
        Self {
            step_index: 6,
            feature_index,
            count,
            height,
            ore,
            biome_filter: FeatureBiomeFilter::All,
        }
    }

    fn with_step_index(mut self, step_index: i32) -> Self {
        self.step_index = step_index;
        self
    }

    fn with_biome_filter(mut self, biome_filter: FeatureBiomeFilter) -> Self {
        self.biome_filter = biome_filter;
        self
    }

    fn place(
        &self,
        settings: &NoiseSettings,
        origin_x: i32,
        origin_z: i32,
        chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
    ) {
        for _ in 0..self.count.sample(random) {
            let x = origin_x + random.next_int(16);
            let z = origin_z + random.next_int(16);
            let y = self.height.sample(settings, random);
            if !self.biome_filter.allows(settings.density.biome(x, y, z)) {
                continue;
            }
            self.ore
                .place(settings, origin_x, origin_z, chunk, random, x, y, z);
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum FeatureBiomeFilter {
    All,
    Include(&'static [&'static str]),
    Exclude(&'static [&'static str]),
}

impl FeatureBiomeFilter {
    fn allows(self, biome: &str) -> bool {
        match self {
            Self::All => true,
            Self::Include(biomes) => biomes.contains(&biome),
            Self::Exclude(biomes) => !biomes.contains(&biome),
        }
    }
}

#[derive(Debug, Clone)]
struct PlacedUnderwaterMagmaFeature {
    step_index: i32,
    feature_index: i32,
    count: OrePlacementCount,
    height: OreHeight,
    floor_search_range: i32,
    placement_radius_around_floor: i32,
    placement_probability_per_valid_position: f32,
    magma_block: BlockLayer,
    biome_filter: FeatureBiomeFilter,
}

impl PlacedUnderwaterMagmaFeature {
    fn new(feature_index: i32) -> Self {
        Self {
            step_index: 6,
            feature_index,
            count: OrePlacementCount::Uniform { min: 44, max: 52 },
            height: OreHeight::Uniform(HeightAnchor::AboveBottom(0), HeightAnchor::Absolute(256)),
            floor_search_range: 5,
            placement_radius_around_floor: 1,
            placement_probability_per_valid_position: 0.5,
            magma_block: BlockLayer::new("minecraft:magma_block"),
            biome_filter: FeatureBiomeFilter::All,
        }
    }

    fn place(
        &self,
        settings: &NoiseSettings,
        origin_x: i32,
        origin_z: i32,
        chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
    ) {
        for _ in 0..self.count.sample(random) {
            let x = origin_x + random.next_int(16);
            let z = origin_z + random.next_int(16);
            let y = self.height.sample(settings, random);
            let local_x = (x - origin_x) as usize;
            let local_z = (z - origin_z) as usize;
            let ocean_floor = chunk.ocean_floor_wg_height(local_x, local_z, settings.min_y);
            if y > ocean_floor - 2 || !self.biome_filter.allows(settings.density.biome(x, y, z)) {
                continue;
            }

            if let Some(floor_y) = self.find_floor_y(settings, chunk, local_x, y, local_z) {
                self.place_around_floor(settings, origin_x, origin_z, chunk, random, x, floor_y, z);
            }
        }
    }

    fn find_floor_y(
        &self,
        settings: &NoiseSettings,
        chunk: &NoiseChunkBlocks,
        local_x: usize,
        origin_y: i32,
        local_z: usize,
    ) -> Option<i32> {
        if !is_water_at(chunk, local_x, origin_y, local_z, settings.min_y) {
            return None;
        }

        let mut y = origin_y;
        for _ in 1..self.floor_search_range {
            if !is_water_at(chunk, local_x, y, local_z, settings.min_y) {
                break;
            }
            y -= 1;
        }

        let layer = chunk.layer(local_x, y, local_z, settings.min_y)?;
        (!is_water_layer(layer)).then_some(y)
    }

    #[allow(clippy::too_many_arguments)]
    fn place_around_floor(
        &self,
        settings: &NoiseSettings,
        origin_x: i32,
        origin_z: i32,
        chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
        floor_x: i32,
        floor_y: i32,
        floor_z: i32,
    ) {
        let radius = self.placement_radius_around_floor;
        for world_x in floor_x - radius..=floor_x + radius {
            for world_y in floor_y - radius..=floor_y + radius {
                for world_z in floor_z - radius..=floor_z + radius {
                    if random.next_float() >= self.placement_probability_per_valid_position {
                        continue;
                    }
                    let Some(local_x) = local_coord(world_x, origin_x) else {
                        continue;
                    };
                    let Some(local_z) = local_coord(world_z, origin_z) else {
                        continue;
                    };
                    if self.is_valid_placement(settings, chunk, local_x, world_y, local_z) {
                        chunk.set_layer(
                            local_x,
                            world_y,
                            local_z,
                            settings.min_y,
                            self.magma_block.clone(),
                        );
                    }
                }
            }
        }
    }

    fn is_valid_placement(
        &self,
        settings: &NoiseSettings,
        chunk: &NoiseChunkBlocks,
        local_x: usize,
        world_y: i32,
        local_z: usize,
    ) -> bool {
        let Some(current) = chunk.layer(local_x, world_y, local_z, settings.min_y) else {
            return false;
        };
        if is_water_or_air_layer(current) {
            return false;
        }
        let Some(below) = chunk.layer(local_x, world_y - 1, local_z, settings.min_y) else {
            return false;
        };
        if !is_full_solid_layer(below) {
            return false;
        }

        for (dx, dz) in [(-1_i32, 0_i32), (1, 0), (0, -1), (0, 1)] {
            let local_x = local_x as i32 + dx;
            let local_z = local_z as i32 + dz;
            if !(0..16).contains(&local_x) || !(0..16).contains(&local_z) {
                return false;
            }
            let Some(neighbor) =
                chunk.layer(local_x as usize, world_y, local_z as usize, settings.min_y)
            else {
                return false;
            };
            if !is_full_solid_layer(neighbor) {
                return false;
            }
        }

        true
    }
}

#[derive(Debug, Clone)]
struct PlacedDiskFeature {
    step_index: i32,
    feature_index: i32,
    count: OrePlacementCount,
    half_height: i32,
    radius: UniformInt,
    target_blocks: &'static [&'static str],
    state_provider: DiskStateProvider,
    biome_filter: FeatureBiomeFilter,
}

impl PlacedDiskFeature {
    fn sand(feature_index: i32) -> Self {
        Self {
            step_index: 6,
            feature_index,
            count: OrePlacementCount::Constant(3),
            half_height: 2,
            radius: UniformInt { min: 2, max: 6 },
            target_blocks: DISK_DIRT_GRASS_TARGETS,
            state_provider: DiskStateProvider::Sand {
                sand: BlockLayer::new("minecraft:sand"),
                sandstone: BlockLayer::new("minecraft:sandstone"),
            },
            biome_filter: FeatureBiomeFilter::All,
        }
    }

    fn clay(feature_index: i32) -> Self {
        Self::simple(
            feature_index,
            1,
            UniformInt { min: 2, max: 3 },
            DISK_DIRT_CLAY_TARGETS,
            "minecraft:clay",
        )
    }

    fn gravel(feature_index: i32) -> Self {
        Self::simple(
            feature_index,
            2,
            UniformInt { min: 2, max: 5 },
            DISK_DIRT_GRASS_TARGETS,
            "minecraft:gravel",
        )
    }

    fn simple(
        feature_index: i32,
        half_height: i32,
        radius: UniformInt,
        target_blocks: &'static [&'static str],
        block: &str,
    ) -> Self {
        Self {
            step_index: 6,
            feature_index,
            count: OrePlacementCount::Constant(1),
            half_height,
            radius,
            target_blocks,
            state_provider: DiskStateProvider::Simple(BlockLayer::new(block)),
            biome_filter: FeatureBiomeFilter::All,
        }
    }

    fn with_biome_filter(mut self, biome_filter: FeatureBiomeFilter) -> Self {
        self.biome_filter = biome_filter;
        self
    }

    fn place(
        &self,
        settings: &NoiseSettings,
        origin_x: i32,
        origin_z: i32,
        chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
    ) {
        for _ in 0..self.count.sample(random) {
            let x = origin_x + random.next_int(16);
            let z = origin_z + random.next_int(16);
            let local_x = (x - origin_x) as usize;
            let local_z = (z - origin_z) as usize;
            let y = chunk.ocean_floor_wg_height(local_x, local_z, settings.min_y);
            if y <= settings.min_y
                || !self.biome_filter.allows(settings.density.biome(x, y, z))
                || !is_water_at(chunk, local_x, y, local_z, settings.min_y)
            {
                continue;
            }

            self.place_disk(settings, origin_x, origin_z, chunk, random, x, y, z);
        }
    }

    fn place_disk(
        &self,
        settings: &NoiseSettings,
        origin_x: i32,
        origin_z: i32,
        chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
        center_x: i32,
        center_y: i32,
        center_z: i32,
    ) {
        let radius = self.radius.sample(random);
        let min_y = (center_y - self.half_height).max(settings.min_y);
        let max_y = (center_y + self.half_height).min(settings.min_y + settings.height - 1);
        if min_y > max_y {
            return;
        }

        for world_x in center_x - radius..=center_x + radius {
            let dx = world_x - center_x;
            for world_z in center_z - radius..=center_z + radius {
                let dz = world_z - center_z;
                if dx * dx + dz * dz > radius * radius {
                    continue;
                }
                let Some(local_x) = local_coord(world_x, origin_x) else {
                    continue;
                };
                let Some(local_z) = local_coord(world_z, origin_z) else {
                    continue;
                };
                for world_y in (min_y..=max_y).rev() {
                    let Some(current) = chunk.layer(local_x, world_y, local_z, settings.min_y)
                    else {
                        continue;
                    };
                    if !self.target_blocks.contains(&current.block.as_str()) {
                        continue;
                    }

                    let replacement = self.state_provider.block_at(
                        chunk,
                        local_x,
                        world_y,
                        local_z,
                        settings.min_y,
                    );
                    chunk.set_layer(local_x, world_y, local_z, settings.min_y, replacement);
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
enum DiskStateProvider {
    Simple(BlockLayer),
    Sand {
        sand: BlockLayer,
        sandstone: BlockLayer,
    },
}

impl DiskStateProvider {
    fn block_at(
        &self,
        chunk: &NoiseChunkBlocks,
        local_x: usize,
        world_y: i32,
        local_z: usize,
        min_y: i32,
    ) -> BlockLayer {
        match self {
            Self::Simple(block) => block.clone(),
            Self::Sand { sand, sandstone } => {
                if chunk
                    .layer(local_x, world_y - 1, local_z, min_y)
                    .is_some_and(|layer| layer.is_air)
                {
                    sandstone.clone()
                } else {
                    sand.clone()
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
struct PlacedSpringFeature {
    step_index: i32,
    feature_index: i32,
    count: OrePlacementCount,
    height: OreHeight,
    config: SpringFeatureConfig,
    biome_filter: FeatureBiomeFilter,
}

impl PlacedSpringFeature {
    fn water(feature_index: i32) -> Self {
        Self {
            step_index: 8,
            feature_index,
            count: OrePlacementCount::Constant(25),
            height: OreHeight::Uniform(HeightAnchor::AboveBottom(0), HeightAnchor::Absolute(192)),
            config: SpringFeatureConfig {
                state: BlockLayer::new("minecraft:water"),
                rock_count: 4,
                hole_count: 1,
                requires_block_below: true,
                valid_blocks: SPRING_WATER_VALID_BLOCKS,
            },
            biome_filter: FeatureBiomeFilter::All,
        }
    }

    fn lava_overworld(feature_index: i32) -> Self {
        Self {
            step_index: 8,
            feature_index,
            count: OrePlacementCount::Constant(20),
            height: OreHeight::VeryBiasedToBottom {
                min: HeightAnchor::AboveBottom(0),
                max: HeightAnchor::BelowTop(8),
                inner: 8,
            },
            config: SpringFeatureConfig {
                state: BlockLayer::new("minecraft:lava"),
                rock_count: 4,
                hole_count: 1,
                requires_block_below: true,
                valid_blocks: SPRING_LAVA_VALID_BLOCKS,
            },
            biome_filter: FeatureBiomeFilter::All,
        }
    }

    fn place(
        &self,
        settings: &NoiseSettings,
        origin_x: i32,
        origin_z: i32,
        chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
    ) {
        for _ in 0..self.count.sample(random) {
            let world_x = origin_x + random.next_int(16);
            let world_z = origin_z + random.next_int(16);
            let world_y = self.height.sample(settings, random);
            if !self
                .biome_filter
                .allows(settings.density.biome(world_x, world_y, world_z))
            {
                continue;
            }
            let Some(local_x) = local_coord(world_x, origin_x) else {
                continue;
            };
            let Some(local_z) = local_coord(world_z, origin_z) else {
                continue;
            };
            self.config
                .try_place(settings, chunk, local_x, world_y, local_z);
        }
    }
}

#[derive(Debug, Clone)]
struct SpringFeatureConfig {
    state: BlockLayer,
    rock_count: i32,
    hole_count: i32,
    requires_block_below: bool,
    valid_blocks: &'static [&'static str],
}

impl SpringFeatureConfig {
    fn try_place(
        &self,
        settings: &NoiseSettings,
        chunk: &mut NoiseChunkBlocks,
        local_x: usize,
        world_y: i32,
        local_z: usize,
    ) -> bool {
        if !self.is_valid_block(chunk, local_x, world_y + 1, local_z, settings.min_y) {
            return false;
        }
        if self.requires_block_below
            && !self.is_valid_block(chunk, local_x, world_y - 1, local_z, settings.min_y)
        {
            return false;
        }

        let Some(current) = chunk.layer(local_x, world_y, local_z, settings.min_y) else {
            return false;
        };
        if !current.is_air && !self.valid_blocks.contains(&current.block.as_str()) {
            return false;
        }

        let mut rock_count = 0;
        let mut hole_count = 0;
        for (dx, dy, dz) in [
            (-1_i32, 0_i32, 0_i32),
            (1, 0, 0),
            (0, 0, -1),
            (0, 0, 1),
            (0, -1, 0),
        ] {
            let local_x = local_x as i32 + dx;
            let local_z = local_z as i32 + dz;
            if !(0..16).contains(&local_x) || !(0..16).contains(&local_z) {
                continue;
            }
            let y = world_y + dy;
            if self.is_valid_block(chunk, local_x as usize, y, local_z as usize, settings.min_y) {
                rock_count += 1;
            }
            if chunk
                .layer(local_x as usize, y, local_z as usize, settings.min_y)
                .is_some_and(|layer| layer.is_air)
            {
                hole_count += 1;
            }
        }

        if rock_count == self.rock_count && hole_count == self.hole_count {
            chunk.set_layer(
                local_x,
                world_y,
                local_z,
                settings.min_y,
                self.state.clone(),
            );
            true
        } else {
            false
        }
    }

    fn is_valid_block(
        &self,
        chunk: &NoiseChunkBlocks,
        local_x: usize,
        world_y: i32,
        local_z: usize,
        min_y: i32,
    ) -> bool {
        chunk
            .layer(local_x, world_y, local_z, min_y)
            .is_some_and(|layer| self.valid_blocks.contains(&layer.block.as_str()))
    }
}

#[derive(Debug, Clone)]
struct OreFeatureConfig {
    size: i32,
    discard_chance_on_air_exposure: f32,
    targets: Vec<OreFeatureTarget>,
}

#[derive(Debug, Clone)]
struct OreFeatureTarget {
    predicate: OreTargetPredicate,
    block: BlockLayer,
}

#[derive(Debug, Clone, Copy)]
enum OreTargetPredicate {
    StoneOreReplaceables,
    DeepslateOreReplaceables,
    BaseStoneOverworld,
}

impl OreFeatureConfig {
    fn new(
        size: i32,
        discard_chance_on_air_exposure: f32,
        stone_ore: &str,
        deepslate_ore: &str,
    ) -> Self {
        Self {
            size,
            discard_chance_on_air_exposure,
            targets: vec![
                OreFeatureTarget {
                    predicate: OreTargetPredicate::StoneOreReplaceables,
                    block: BlockLayer::new(stone_ore),
                },
                OreFeatureTarget {
                    predicate: OreTargetPredicate::DeepslateOreReplaceables,
                    block: BlockLayer::new(deepslate_ore),
                },
            ],
        }
    }

    fn base_stone(size: i32, block: &str) -> Self {
        Self {
            size,
            discard_chance_on_air_exposure: 0.0,
            targets: vec![OreFeatureTarget {
                predicate: OreTargetPredicate::BaseStoneOverworld,
                block: BlockLayer::new(block),
            }],
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn place(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
        origin_x: i32,
        origin_y: i32,
        origin_z: i32,
    ) -> bool {
        let direction = random.next_float() * std::f32::consts::PI;
        let spread_xy = self.size as f64 / 8.0;
        let x0 = origin_x as f64 + direction.sin() as f64 * spread_xy;
        let x1 = origin_x as f64 - direction.sin() as f64 * spread_xy;
        let z0 = origin_z as f64 + direction.cos() as f64 * spread_xy;
        let z1 = origin_z as f64 - direction.cos() as f64 * spread_xy;
        let y0 = origin_y as f64 + random.next_int(3) as f64 - 2.0;
        let y1 = origin_y as f64 + random.next_int(3) as f64 - 2.0;
        let mut spheres = vec![[0.0; 4]; self.size as usize];

        for i in 0..self.size {
            let step = i as f64 / self.size as f64;
            let radius_noise = random.next_double() * self.size as f64 / 16.0;
            let radius = (((std::f32::consts::PI * i as f32 / self.size as f32).sin() + 1.0)
                as f64
                * radius_noise
                + 1.0)
                / 2.0;
            spheres[i as usize] = [
                lerp_f64(step, x0, x1),
                lerp_f64(step, y0, y1),
                lerp_f64(step, z0, z1),
                radius,
            ];
        }

        for i1 in 0..self.size as usize {
            if spheres[i1][3] <= 0.0 {
                continue;
            }
            for i2 in i1 + 1..self.size as usize {
                if spheres[i2][3] <= 0.0 {
                    continue;
                }
                let dx = spheres[i1][0] - spheres[i2][0];
                let dy = spheres[i1][1] - spheres[i2][1];
                let dz = spheres[i1][2] - spheres[i2][2];
                let dr = spheres[i1][3] - spheres[i2][3];
                if dr * dr > dx * dx + dy * dy + dz * dz {
                    if dr > 0.0 {
                        spheres[i2][3] = -1.0;
                    } else {
                        spheres[i1][3] = -1.0;
                    }
                }
            }
        }

        let mut tested = HashSet::<(i32, i32, i32)>::new();
        let mut placed = false;
        for sphere in spheres {
            let [x, y, z, radius] = sphere;
            if radius < 0.0 {
                continue;
            }

            let min_x = (x - radius).floor() as i32;
            let max_x = ((x + radius).floor() as i32).max(min_x);
            let min_y = (y - radius).floor() as i32;
            let max_y = ((y + radius).floor() as i32).max(min_y);
            let min_z = (z - radius).floor() as i32;
            let max_z = ((z + radius).floor() as i32).max(min_z);

            for world_x in min_x.max(chunk_min_x)..=max_x.min(chunk_min_x + 15) {
                let xd = (world_x as f64 + 0.5 - x) / radius;
                if xd * xd >= 1.0 {
                    continue;
                }

                for world_y in
                    min_y.max(settings.min_y)..=max_y.min(settings.min_y + settings.height - 1)
                {
                    let yd = (world_y as f64 + 0.5 - y) / radius;
                    if xd * xd + yd * yd >= 1.0 {
                        continue;
                    }

                    for world_z in min_z.max(chunk_min_z)..=max_z.min(chunk_min_z + 15) {
                        let zd = (world_z as f64 + 0.5 - z) / radius;
                        if xd * xd + yd * yd + zd * zd >= 1.0 {
                            continue;
                        }
                        if tested.insert((world_x, world_y, world_z))
                            && self.try_place_block(
                                settings,
                                chunk_min_x,
                                chunk_min_z,
                                chunk,
                                random,
                                world_x,
                                world_y,
                                world_z,
                            )
                        {
                            placed = true;
                        }
                    }
                }
            }
        }

        placed
    }

    #[allow(clippy::too_many_arguments)]
    fn try_place_block(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
        world_x: i32,
        world_y: i32,
        world_z: i32,
    ) -> bool {
        let local_x = (world_x - chunk_min_x) as usize;
        let local_z = (world_z - chunk_min_z) as usize;
        let Some(current) = chunk.layer(local_x, world_y, local_z, settings.min_y) else {
            return false;
        };
        let Some(ore) = self.target_ore(current).cloned() else {
            return false;
        };
        if !self.should_skip_air_check(random)
            && is_adjacent_to_air(settings, chunk, local_x, world_y, local_z)
        {
            return false;
        }

        chunk.set_layer(local_x, world_y, local_z, settings.min_y, ore);
        true
    }

    fn target_ore(&self, current: &BlockLayer) -> Option<&BlockLayer> {
        self.targets
            .iter()
            .find(|target| target.predicate.matches(current))
            .map(|target| &target.block)
    }

    fn should_skip_air_check(&self, random: &mut FeatureRandom) -> bool {
        if self.discard_chance_on_air_exposure <= 0.0 {
            true
        } else if self.discard_chance_on_air_exposure >= 1.0 {
            false
        } else {
            random.next_float() >= self.discard_chance_on_air_exposure
        }
    }
}

impl OreTargetPredicate {
    fn matches(self, layer: &BlockLayer) -> bool {
        match self {
            Self::StoneOreReplaceables => is_stone_ore_replaceable(layer),
            Self::DeepslateOreReplaceables => is_deepslate_ore_replaceable(layer),
            Self::BaseStoneOverworld => is_base_stone_overworld(layer),
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum OrePlacementCount {
    Constant(i32),
    Uniform { min: i32, max: i32 },
    Rarity(i32),
}

impl OrePlacementCount {
    fn sample(self, random: &mut FeatureRandom) -> i32 {
        match self {
            Self::Constant(value) => value,
            Self::Uniform { min, max } => min + random.next_int(max - min + 1),
            Self::Rarity(chance) => (random.next_float() < 1.0 / chance as f32) as i32,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct UniformInt {
    min: i32,
    max: i32,
}

impl UniformInt {
    fn sample(self, random: &mut FeatureRandom) -> i32 {
        self.min + random.next_int(self.max - self.min + 1)
    }
}

#[derive(Clone, Copy, Debug)]
enum OreHeight {
    Uniform(HeightAnchor, HeightAnchor),
    Trapezoid(HeightAnchor, HeightAnchor),
    VeryBiasedToBottom {
        min: HeightAnchor,
        max: HeightAnchor,
        inner: i32,
    },
}

impl OreHeight {
    fn sample(self, settings: &NoiseSettings, random: &mut FeatureRandom) -> i32 {
        match self {
            Self::Uniform(min, max) => {
                let min = min.resolve(settings);
                let max = max.resolve(settings);
                if min > max {
                    min
                } else {
                    min + random.next_int(max - min + 1)
                }
            }
            Self::Trapezoid(min, max) => {
                let min = min.resolve(settings);
                let max = max.resolve(settings);
                if min > max {
                    return min;
                }
                let range = max - min;
                let plateau_start = range / 2;
                let plateau_end = range - plateau_start;
                min + random.next_int(plateau_end + 1) + random.next_int(plateau_start + 1)
            }
            Self::VeryBiasedToBottom { min, max, inner } => {
                let min = min.resolve(settings);
                let max = max.resolve(settings);
                if max - min - inner + 1 <= 0 {
                    return min;
                }
                let upper_inclusive = min + inner + random.next_int(max - min - inner + 1);
                let biased_upper_inclusive = min + random.next_int(upper_inclusive - min);
                min + random.next_int(biased_upper_inclusive - min + inner)
            }
        }
    }
}

#[derive(Clone, Debug)]
struct FeatureRandom {
    source: vanilla_noise::XoroshiroRandomSource,
}

impl FeatureRandom {
    fn new(seed: i64) -> Self {
        Self {
            source: vanilla_noise::XoroshiroRandomSource::new(seed),
        }
    }

    fn decoration_seed(world_seed: i64, origin_x: i32, origin_z: i32) -> i64 {
        let mut random = Self::new(world_seed);
        let x_scale = random.next_long() | 1;
        let z_scale = random.next_long() | 1;
        (origin_x as i64)
            .wrapping_mul(x_scale)
            .wrapping_add((origin_z as i64).wrapping_mul(z_scale))
            ^ world_seed
    }

    fn for_feature(decoration_seed: i64, feature_index: i32, step_index: i32) -> Self {
        Self::new(
            decoration_seed
                .wrapping_add(feature_index as i64)
                .wrapping_add((10_000 * step_index) as i64),
        )
    }

    fn next_bits(&mut self, bits: u32) -> i32 {
        (self.source.next_long() >> (64 - bits)) as i32
    }

    fn next_int(&mut self, bound: i32) -> i32 {
        assert!(bound > 0);
        if (bound & -bound) == bound {
            return (((bound as i64) * (self.next_bits(31) as i64)) >> 31) as i32;
        }

        loop {
            let sample = self.next_bits(31);
            let modulo = sample % bound;
            if sample.wrapping_sub(modulo).wrapping_add(bound - 1) >= 0 {
                return modulo;
            }
        }
    }

    fn next_long(&mut self) -> i64 {
        let upper = self.next_bits(32) as i64;
        let lower = self.next_bits(32) as i64;
        (upper << 32).wrapping_add(lower)
    }

    fn next_float(&mut self) -> f32 {
        self.next_bits(24) as f32 * 5.960_464_5e-8_f32
    }

    fn next_double(&mut self) -> f64 {
        let upper = self.next_bits(26) as i64;
        let lower = self.next_bits(27) as i64;
        ((upper << 27) + lower) as f64 * (1.110_223e-16_f32 as f64)
    }
}

fn is_stone_ore_replaceable(layer: &BlockLayer) -> bool {
    matches!(
        layer.block.as_str(),
        "minecraft:stone" | "minecraft:granite" | "minecraft:diorite" | "minecraft:andesite"
    )
}

fn is_deepslate_ore_replaceable(layer: &BlockLayer) -> bool {
    matches!(
        layer.block.as_str(),
        "minecraft:deepslate" | "minecraft:tuff"
    )
}

fn is_base_stone_overworld(layer: &BlockLayer) -> bool {
    matches!(
        layer.block.as_str(),
        "minecraft:stone"
            | "minecraft:granite"
            | "minecraft:diorite"
            | "minecraft:andesite"
            | "minecraft:tuff"
            | "minecraft:deepslate"
    )
}

fn is_water_at(
    chunk: &NoiseChunkBlocks,
    local_x: usize,
    world_y: i32,
    local_z: usize,
    min_y: i32,
) -> bool {
    chunk
        .layer(local_x, world_y, local_z, min_y)
        .is_some_and(is_water_layer)
}

fn is_water_layer(layer: &BlockLayer) -> bool {
    layer.is("minecraft:water")
}

fn is_fluid_layer(layer: &BlockLayer) -> bool {
    matches!(layer.block.as_str(), "minecraft:water" | "minecraft:lava")
}

fn is_water_or_air_layer(layer: &BlockLayer) -> bool {
    layer.is_air || is_water_layer(layer)
}

fn is_full_solid_layer(layer: &BlockLayer) -> bool {
    !layer.is_air && !is_fluid_layer(layer)
}

fn local_coord(world: i32, origin: i32) -> Option<usize> {
    let local = world - origin;
    (0..16).contains(&local).then_some(local as usize)
}

fn is_adjacent_to_air(
    settings: &NoiseSettings,
    chunk: &NoiseChunkBlocks,
    local_x: usize,
    world_y: i32,
    local_z: usize,
) -> bool {
    const DIRECTIONS: [(i32, i32, i32); 6] = [
        (1, 0, 0),
        (-1, 0, 0),
        (0, 1, 0),
        (0, -1, 0),
        (0, 0, 1),
        (0, 0, -1),
    ];

    DIRECTIONS.into_iter().any(|(dx, dy, dz)| {
        let x = local_x as i32 + dx;
        let y = world_y + dy;
        let z = local_z as i32 + dz;
        if !(0..16).contains(&x)
            || !(0..16).contains(&z)
            || !(settings.min_y..settings.min_y + settings.height).contains(&y)
        {
            return false;
        }
        chunk
            .layer(x as usize, y, z as usize, settings.min_y)
            .is_some_and(|layer| layer.is_air)
    })
}

fn lerp_f64(delta: f64, start: f64, end: f64) -> f64 {
    start + delta * (end - start)
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

    fn is(&self, block: &str) -> bool {
        self.block == block
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
    let noise_kind = vanilla_noise::OverworldNoiseKind::from_preset(preset);

    Ok(NoiseSettings {
        min_y,
        height,
        sea_level,
        default_block: BlockLayer::new(default_block),
        default_fluid: BlockLayer::new(default_fluid),
        lava_block: BlockLayer::new("minecraft:lava"),
        surface_block: BlockLayer::new("minecraft:grass_block"),
        subsurface_block: BlockLayer::new("minecraft:dirt"),
        deepslate_block: BlockLayer::new("minecraft:deepslate"),
        podzol_block: BlockLayer::new("minecraft:podzol"),
        coarse_dirt_block: BlockLayer::new("minecraft:coarse_dirt"),
        mycelium_block: BlockLayer::new("minecraft:mycelium"),
        calcite_block: BlockLayer::new("minecraft:calcite"),
        gravel_block: BlockLayer::new("minecraft:gravel"),
        sand_block: BlockLayer::new("minecraft:sand"),
        sandstone_block: BlockLayer::new("minecraft:sandstone"),
        packed_ice_block: BlockLayer::new("minecraft:packed_ice"),
        ice_block: BlockLayer::new("minecraft:ice"),
        snow_block: BlockLayer::new("minecraft:snow_block"),
        powder_snow_block: BlockLayer::new("minecraft:powder_snow"),
        mud_block: BlockLayer::new("minecraft:mud"),
        water_block: BlockLayer::new("minecraft:water"),
        terracotta_block: BlockLayer::new("minecraft:terracotta"),
        orange_terracotta_block: BlockLayer::new("minecraft:orange_terracotta"),
        white_terracotta_block: BlockLayer::new("minecraft:white_terracotta"),
        yellow_terracotta_block: BlockLayer::new("minecraft:yellow_terracotta"),
        brown_terracotta_block: BlockLayer::new("minecraft:brown_terracotta"),
        red_terracotta_block: BlockLayer::new("minecraft:red_terracotta"),
        light_gray_terracotta_block: BlockLayer::new("minecraft:light_gray_terracotta"),
        red_sand_block: BlockLayer::new("minecraft:red_sand"),
        copper_ore_block: BlockLayer::new("minecraft:copper_ore"),
        raw_copper_block: BlockLayer::new("minecraft:raw_copper_block"),
        granite_block: BlockLayer::new("minecraft:granite"),
        deepslate_iron_ore_block: BlockLayer::new("minecraft:deepslate_iron_ore"),
        raw_iron_block: BlockLayer::new("minecraft:raw_iron_block"),
        tuff_block: BlockLayer::new("minecraft:tuff"),
        biome: "minecraft:plains".to_string(),
        density: TerrainDensity::overworld(seed, noise_kind),
        surface_rules,
        aquifer: vanilla_noise::OverworldAquifer::new(seed, sea_level),
        ore_veins: vanilla_noise::OreVeinNoise::new(seed),
        carvers: VanillaCarvers::new(seed),
        ore_features: OverworldOreFeatures::new(seed),
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

fn noise_chunk_root(chunk: &NoiseChunkBlocks, _biome: &str) -> Tag {
    compound_tag([
        ("sections", noise_sections_tag(chunk)),
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

fn noise_sections_tag(chunk: &NoiseChunkBlocks) -> Tag {
    let mut sections = Vec::new();
    for section_y in WORLD_MIN_SECTION_Y..WORLD_MIN_SECTION_Y + section_count() {
        let block_states = noise_block_states_tag(chunk, section_y);
        if !noise_section_is_air(chunk, section_y) {
            sections.push(compound_tag([
                ("Y", Tag::Byte(section_y as i8)),
                ("block_states", block_states),
                ("biomes", noise_biomes_tag(chunk, section_y)),
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

fn noise_biomes_tag(chunk: &NoiseChunkBlocks, section_y: i32) -> Tag {
    let mut palette = Vec::new();
    let mut index_by_biome = HashMap::<String, usize>::new();
    let mut values = vec![0_i32; 4 * 4 * 4];

    for x in 0..4 {
        for y in 0..4 {
            for z in 0..4 {
                let biome = chunk.biome(section_y, x, y, z);
                let next_index = palette.len();
                let palette_index = *index_by_biome.entry(biome.to_string()).or_insert_with(|| {
                    palette.push(Tag::String(Arc::from(biome.to_string())));
                    next_index
                });
                values[(x * 4 + y) * 4 + z] = palette_index as i32;
            }
        }
    }

    let data = (palette.len() > 1).then(|| {
        let bits = ceil_log2(palette.len());
        pack_fixed_long_values(&values, bits)
            .into_iter()
            .map(|value| value as i64)
            .collect::<Vec<_>>()
    });
    paletted_container(palette, data)
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
    fn java_random_matches_legacy_lcg_outputs() {
        let mut random = JavaRandom::new(12345);

        assert_eq!(random.next_int(15), 1);
        assert_eq!(random.next_int(1000), 80);
        assert_eq!(random.next_float().to_bits(), 0.932_993_5_f32.to_bits());
        assert_eq!(random.next_long(), -1_528_963_862_231_680_626);

        random.set_large_feature_seed(12345, 3, -7);
        assert_eq!(random.next_int(16), 7);
        assert_eq!(random.next_float().to_bits(), 0.294_135_75_f32.to_bits());
    }

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

    #[test]
    fn vanilla_noise_uses_lava_below_global_fluid_cutoff() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);

        assert_eq!(
            settings.aquifer.substance_at(0, -55, 0, -1.0, 64),
            vanilla_noise::AquiferSubstance::Fluid(vanilla_noise::AquiferFluid::Lava)
        );
    }

    #[test]
    fn vanilla_noise_maps_ore_vein_blocks() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let has_ore_vein_block = (-32..=32).any(|x| {
            (-32..=32).any(|z| (-60..=50).any(|y| settings.ore_vein_at(x, y, z).is_some()))
        });

        assert!(has_ore_vein_block);
    }

    #[test]
    fn vanilla_noise_carvers_modify_seeded_chunks() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let (base, _) = settings.generate_base_chunk(0, 0);
        let carved = settings.generate_chunk(0, 0);
        let carved_air = (0..16).any(|z| {
            (0..16).any(|x| {
                let base_column = base.column(x, z);
                let carved_column = carved.column(x, z);
                base_column
                    .blocks
                    .iter()
                    .zip(&carved_column.blocks)
                    .any(|(before, after)| {
                        !before.is_air && before.block != "minecraft:lava" && after.is_air
                    })
            })
        });

        assert!(carved_air);
    }

    #[test]
    fn vanilla_noise_places_common_overworld_ores() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let stone = BlockLayer::new("minecraft:stone");
        let deepslate = BlockLayer::new("minecraft:deepslate");
        let columns = (0..HEIGHTMAP_ENTRY_COUNT)
            .map(|_| NoiseColumnBlocks {
                blocks: (settings.min_y..settings.min_y + settings.height)
                    .map(|y| {
                        if y < 0 {
                            deepslate.clone()
                        } else {
                            stone.clone()
                        }
                    })
                    .collect(),
                first_available_height: settings.height,
            })
            .collect();
        let mut chunk = NoiseChunkBlocks {
            columns,
            biomes: Vec::new(),
        };
        let mut found = HashSet::new();

        settings
            .ore_features
            .place_chunk(&settings, 0, 0, &mut chunk);
        for column in &chunk.columns {
            for layer in &column.blocks {
                match layer.block.as_str() {
                    "minecraft:dirt"
                    | "minecraft:gravel"
                    | "minecraft:granite"
                    | "minecraft:diorite"
                    | "minecraft:andesite"
                    | "minecraft:tuff"
                    | "minecraft:coal_ore"
                    | "minecraft:deepslate_coal_ore"
                    | "minecraft:iron_ore"
                    | "minecraft:deepslate_iron_ore"
                    | "minecraft:copper_ore"
                    | "minecraft:deepslate_copper_ore"
                    | "minecraft:redstone_ore"
                    | "minecraft:deepslate_redstone_ore" => {
                        found.insert(layer.block.clone());
                    }
                    _ => {}
                }
            }
        }

        assert!(found.contains("minecraft:dirt"));
        assert!(found.contains("minecraft:gravel"));
        assert!(found.contains("minecraft:coal_ore"));
        assert!(
            found.contains("minecraft:iron_ore") || found.contains("minecraft:deepslate_iron_ore")
        );
        assert!(
            found.contains("minecraft:copper_ore")
                || found.contains("minecraft:deepslate_copper_ore")
        );
        assert!(
            found.contains("minecraft:redstone_ore")
                || found.contains("minecraft:deepslate_redstone_ore")
        );
    }

    #[test]
    fn vanilla_noise_configures_biome_filtered_overworld_ores() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let features = &settings.ore_features.features;

        assert!(features.iter().any(|feature| {
            feature.feature_index == 24
                && feature.ore.size == 10
                && matches!(feature.biome_filter, FeatureBiomeFilter::Exclude(biomes) if biomes.contains(&"minecraft:dripstone_caves"))
        }));
        assert!(features.iter().any(|feature| {
            feature.feature_index == 24
                && feature.ore.size == 20
                && matches!(feature.biome_filter, FeatureBiomeFilter::Include(biomes) if biomes.contains(&"minecraft:dripstone_caves"))
        }));
        assert!(features.iter().any(|feature| {
            feature.feature_index == 26
                && feature
                    .ore
                    .targets
                    .iter()
                    .any(|target| target.block.block == "minecraft:gold_ore")
                && matches!(feature.biome_filter, FeatureBiomeFilter::Include(biomes) if biomes.contains(&"minecraft:badlands"))
        }));
        assert!(features.iter().any(|feature| {
            feature.feature_index == 29
                && feature.step_index == 6
                && feature
                    .ore
                    .targets
                    .iter()
                    .any(|target| target.block.block == "minecraft:emerald_ore")
                && matches!(feature.biome_filter, FeatureBiomeFilter::Include(biomes) if biomes.contains(&"minecraft:meadow"))
        }));
        assert!(features.iter().any(|feature| {
            feature.feature_index == 0
                && feature.step_index == 7
                && feature
                    .ore
                    .targets
                    .iter()
                    .any(|target| target.block.block == "minecraft:infested_stone")
                && matches!(feature.biome_filter, FeatureBiomeFilter::Include(biomes) if biomes.contains(&"minecraft:meadow"))
        }));
    }

    #[test]
    fn vanilla_noise_configures_underwater_magma_and_soft_disks() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let magma = &settings.ore_features.underwater_magma;
        let disks = &settings.ore_features.disks;

        assert_eq!(magma.step_index, 6);
        assert_eq!(magma.feature_index, 25);
        assert!(matches!(
            magma.count,
            OrePlacementCount::Uniform { min: 44, max: 52 }
        ));
        assert!(disks.iter().any(|feature| {
            feature.feature_index == 26
                && feature.target_blocks == DISK_DIRT_GRASS_TARGETS
                && matches!(feature.biome_filter, FeatureBiomeFilter::Exclude(biomes) if biomes.contains(&"minecraft:badlands"))
        }));
        assert!(disks.iter().any(|feature| {
            feature.feature_index == 27
                && feature.target_blocks == DISK_DIRT_GRASS_TARGETS
                && matches!(feature.biome_filter, FeatureBiomeFilter::Include(biomes) if biomes.contains(&"minecraft:badlands"))
        }));
        assert!(disks.iter().any(|feature| {
            feature.feature_index == 27
                && feature.target_blocks == DISK_DIRT_CLAY_TARGETS
                && matches!(feature.biome_filter, FeatureBiomeFilter::Exclude(biomes) if biomes.contains(&"minecraft:badlands"))
        }));
        assert!(disks.iter().any(|feature| {
            feature.feature_index == 29
                && feature.target_blocks == DISK_DIRT_GRASS_TARGETS
                && matches!(feature.biome_filter, FeatureBiomeFilter::Include(biomes) if biomes.contains(&"minecraft:badlands"))
        }));
    }

    #[test]
    fn vanilla_noise_configures_overworld_springs() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let springs = &settings.ore_features.springs;

        assert!(springs.iter().any(|feature| {
            feature.step_index == 8
                && feature.feature_index == 0
                && matches!(feature.count, OrePlacementCount::Constant(25))
                && feature.config.state.block == "minecraft:water"
        }));
        assert!(springs.iter().any(|feature| {
            feature.step_index == 8
                && feature.feature_index == 1
                && matches!(feature.count, OrePlacementCount::Constant(20))
                && matches!(
                    feature.height,
                    OreHeight::VeryBiasedToBottom {
                        min: HeightAnchor::AboveBottom(0),
                        max: HeightAnchor::BelowTop(8),
                        inner: 8
                    }
                )
                && feature.config.state.block == "minecraft:lava"
        }));
    }

    #[test]
    fn vanilla_noise_ore_biome_filter_skips_disallowed_positions() {
        const MISSING_BIOMES: &[&str] = &["minecraft:missing_biome"];

        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let stone = BlockLayer::new("minecraft:stone");
        let columns = (0..HEIGHTMAP_ENTRY_COUNT)
            .map(|_| NoiseColumnBlocks {
                blocks: (settings.min_y..settings.min_y + settings.height)
                    .map(|_| stone.clone())
                    .collect(),
                first_available_height: settings.height,
            })
            .collect();
        let mut chunk = NoiseChunkBlocks {
            columns,
            biomes: Vec::new(),
        };
        let ore = OreFeatureConfig::new(
            32,
            0.0,
            "minecraft:emerald_ore",
            "minecraft:deepslate_emerald_ore",
        );
        let feature = PlacedOreFeature::new(
            0,
            OrePlacementCount::Constant(64),
            OreHeight::Uniform(HeightAnchor::Absolute(48), HeightAnchor::Absolute(48)),
            ore,
        )
        .with_biome_filter(FeatureBiomeFilter::Include(MISSING_BIOMES));
        let mut random = FeatureRandom::new(12345);

        feature.place(&settings, 0, 0, &mut chunk, &mut random);

        assert!(chunk.columns.iter().all(|column| {
            column
                .blocks
                .iter()
                .all(|layer| layer.block != "minecraft:emerald_ore")
        }));
    }

    #[test]
    fn vanilla_noise_soft_disk_replaces_submerged_floor() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let stone = BlockLayer::new("minecraft:stone");
        let dirt = BlockLayer::new("minecraft:dirt");
        let water = BlockLayer::new("minecraft:water");
        let air = BlockLayer::new("minecraft:air");
        let columns = (0..HEIGHTMAP_ENTRY_COUNT)
            .map(|_| NoiseColumnBlocks {
                blocks: (settings.min_y..settings.min_y + settings.height)
                    .map(|y| match y {
                        i32::MIN..=61 => stone.clone(),
                        62 => dirt.clone(),
                        63 => water.clone(),
                        _ => air.clone(),
                    })
                    .collect(),
                first_available_height: 128,
            })
            .collect();
        let mut chunk = NoiseChunkBlocks {
            columns,
            biomes: Vec::new(),
        };
        let mut random = FeatureRandom::new(12345);

        PlacedDiskFeature::sand(0).place(&settings, 0, 0, &mut chunk, &mut random);

        assert_eq!(chunk.ocean_floor_wg_height(0, 0, settings.min_y), 63);
        assert!(chunk.columns.iter().any(|column| {
            column
                .blocks
                .iter()
                .any(|layer| layer.block == "minecraft:sand")
        }));
    }

    #[test]
    fn vanilla_noise_spring_places_when_rock_and_hole_counts_match() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let stone = BlockLayer::new("minecraft:stone");
        let air = BlockLayer::new("minecraft:air");
        let columns = (0..HEIGHTMAP_ENTRY_COUNT)
            .map(|_| NoiseColumnBlocks {
                blocks: (settings.min_y..settings.min_y + settings.height)
                    .map(|_| stone.clone())
                    .collect(),
                first_available_height: settings.height,
            })
            .collect();
        let mut chunk = NoiseChunkBlocks {
            columns,
            biomes: Vec::new(),
        };
        chunk.set_layer(8, 64, 8, settings.min_y, air.clone());
        chunk.set_layer(9, 64, 8, settings.min_y, air);
        let spring = PlacedSpringFeature::water(0);

        assert!(spring.config.try_place(&settings, &mut chunk, 8, 64, 8));
        assert_eq!(
            chunk
                .layer(8, 64, 8, settings.min_y)
                .map(|layer| layer.block.as_str()),
            Some("minecraft:water")
        );
    }

    #[test]
    fn vanilla_noise_generates_biome_cells() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let biomes = settings.generate_biomes(0, 0);

        assert_eq!(biomes.len(), section_count() as usize * 64);
        assert!(biomes.iter().all(|biome| biome.starts_with("minecraft:")));
    }

    #[test]
    fn vanilla_noise_loads_large_and_amplified_presets() {
        let large = load_noise_settings("minecraft:large_biomes", 12345).unwrap();
        let amplified = load_noise_settings("minecraft:amplified", 12345).unwrap();
        let default = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let points = [(128, 64, 128), (320, 96, -144), (-512, 140, 384)];
        let differs_from_default = |settings: &NoiseSettings| {
            points.into_iter().any(|(x, y, z)| {
                let profile = settings.density.profile(x, z);
                let default_profile = default.density.profile(x, z);
                settings
                    .density
                    .sample_with_profile(x, y, z, &profile)
                    .to_bits()
                    != default
                        .density
                        .sample_with_profile(x, y, z, &default_profile)
                        .to_bits()
            })
        };

        assert!(differs_from_default(&large));
        assert!(differs_from_default(&amplified));
    }
}

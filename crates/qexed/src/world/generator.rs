use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use qexed_config::app::qexed::server::{
    World as WorldConfig, WorldGenerator as WorldGeneratorConfig, WorldGpu,
};
use qexed_nbt::{ListHeader, Tag, tag_id};
use qexed_packet::net_types::{OptionalNbt, VarInt};
use qexed_protocol::to_client::play::map_chunk::{BlockEntities, MapChunk};
use rayon::prelude::*;
use serde::Deserialize;

use super::{
    CHUNK_DAMPENING_LEN, SECTION_HEIGHT, WORLD_MAX_Y, WORLD_MIN_SECTION_Y, WORLD_MIN_Y,
    WorldLightAlgorithm, chunk_nbt, empty_chunk_packet, gpu_worldgen, section_count, vanilla_noise,
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
const SLOW_NOISE_CHUNK_LOG_THRESHOLD: Duration = Duration::from_millis(200);
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
const FLOWER_PLAINS_BIOMES: &[&str] = &[
    "minecraft:plains",
    "minecraft:sunflower_plains",
    "minecraft:deep_dark",
    "minecraft:dripstone_caves",
];
const PATCH_GRASS_PLAIN_BIOMES: &[&str] = &[
    "minecraft:plains",
    "minecraft:sunflower_plains",
    "minecraft:deep_dark",
    "minecraft:dripstone_caves",
    "minecraft:cherry_grove",
];
const PATCH_TALL_GRASS_2_BIOMES: &[&str] = &[
    "minecraft:cherry_grove",
    "minecraft:deep_dark",
    "minecraft:dripstone_caves",
    "minecraft:lush_caves",
    "minecraft:meadow",
    "minecraft:plains",
    "minecraft:sunflower_plains",
];
const PATCH_BUSH_BIOMES: &[&str] = &[
    "minecraft:birch_forest",
    "minecraft:forest",
    "minecraft:frozen_river",
    "minecraft:old_growth_birch_forest",
    "minecraft:plains",
    "minecraft:river",
    "minecraft:windswept_forest",
    "minecraft:windswept_gravelly_hills",
    "minecraft:windswept_hills",
];
const PATCH_GRASS_NORMAL_BIOMES: &[&str] = &[
    "minecraft:mangrove_swamp",
    "minecraft:swamp",
    "minecraft:windswept_savanna",
];
const PATCH_GRASS_FOREST_BIOMES: &[&str] = &[
    "minecraft:birch_forest",
    "minecraft:dark_forest",
    "minecraft:forest",
    "minecraft:old_growth_birch_forest",
    "minecraft:pale_garden",
];
const PATCH_GRASS_BADLANDS_BIOMES: &[&str] = &[
    "minecraft:badlands",
    "minecraft:beach",
    "minecraft:cold_ocean",
    "minecraft:deep_cold_ocean",
    "minecraft:deep_frozen_ocean",
    "minecraft:deep_lukewarm_ocean",
    "minecraft:deep_ocean",
    "minecraft:desert",
    "minecraft:eroded_badlands",
    "minecraft:flower_forest",
    "minecraft:frozen_ocean",
    "minecraft:frozen_river",
    "minecraft:ice_spikes",
    "minecraft:lukewarm_ocean",
    "minecraft:ocean",
    "minecraft:river",
    "minecraft:snowy_beach",
    "minecraft:snowy_plains",
    "minecraft:stony_shore",
    "minecraft:warm_ocean",
    "minecraft:windswept_forest",
    "minecraft:windswept_gravelly_hills",
    "minecraft:windswept_hills",
    "minecraft:wooded_badlands",
];
const PATCH_GRASS_SAVANNA_BIOMES: &[&str] = &["minecraft:savanna", "minecraft:savanna_plateau"];
const PATCH_GRASS_TAIGA_BIOMES: &[&str] = &[
    "minecraft:old_growth_pine_taiga",
    "minecraft:old_growth_spruce_taiga",
];
const PATCH_GRASS_TAIGA_2_BIOMES: &[&str] = &["minecraft:snowy_taiga", "minecraft:taiga"];
const PATCH_GRASS_JUNGLE_BIOMES: &[&str] = &[
    "minecraft:bamboo_jungle",
    "minecraft:jungle",
    "minecraft:sparse_jungle",
];
const PATCH_GRASS_MEADOW_BIOMES: &[&str] = &["minecraft:meadow"];
const PATCH_LARGE_FERN_BIOMES: &[&str] = &[
    "minecraft:old_growth_pine_taiga",
    "minecraft:old_growth_spruce_taiga",
    "minecraft:snowy_taiga",
    "minecraft:taiga",
];
const NORMAL_MUSHROOM_BIOMES: &[&str] = &[
    "minecraft:badlands",
    "minecraft:bamboo_jungle",
    "minecraft:beach",
    "minecraft:birch_forest",
    "minecraft:cold_ocean",
    "minecraft:dark_forest",
    "minecraft:deep_cold_ocean",
    "minecraft:deep_dark",
    "minecraft:deep_frozen_ocean",
    "minecraft:deep_lukewarm_ocean",
    "minecraft:deep_ocean",
    "minecraft:desert",
    "minecraft:dripstone_caves",
    "minecraft:eroded_badlands",
    "minecraft:flower_forest",
    "minecraft:forest",
    "minecraft:frozen_ocean",
    "minecraft:frozen_river",
    "minecraft:ice_spikes",
    "minecraft:jungle",
    "minecraft:lukewarm_ocean",
    "minecraft:ocean",
    "minecraft:old_growth_birch_forest",
    "minecraft:old_growth_pine_taiga",
    "minecraft:old_growth_spruce_taiga",
    "minecraft:plains",
    "minecraft:river",
    "minecraft:savanna",
    "minecraft:savanna_plateau",
    "minecraft:snowy_beach",
    "minecraft:snowy_plains",
    "minecraft:sparse_jungle",
    "minecraft:stony_shore",
    "minecraft:sunflower_plains",
    "minecraft:swamp",
    "minecraft:warm_ocean",
    "minecraft:windswept_forest",
    "minecraft:windswept_gravelly_hills",
    "minecraft:windswept_hills",
    "minecraft:windswept_savanna",
    "minecraft:wooded_badlands",
];
const PUMPKIN_PATCH_BIOMES: &[&str] = &[
    "minecraft:badlands",
    "minecraft:bamboo_jungle",
    "minecraft:beach",
    "minecraft:birch_forest",
    "minecraft:cold_ocean",
    "minecraft:dark_forest",
    "minecraft:deep_cold_ocean",
    "minecraft:deep_dark",
    "minecraft:deep_frozen_ocean",
    "minecraft:deep_lukewarm_ocean",
    "minecraft:deep_ocean",
    "minecraft:desert",
    "minecraft:dripstone_caves",
    "minecraft:eroded_badlands",
    "minecraft:flower_forest",
    "minecraft:forest",
    "minecraft:frozen_ocean",
    "minecraft:frozen_river",
    "minecraft:grove",
    "minecraft:ice_spikes",
    "minecraft:jungle",
    "minecraft:lukewarm_ocean",
    "minecraft:ocean",
    "minecraft:old_growth_birch_forest",
    "minecraft:old_growth_pine_taiga",
    "minecraft:old_growth_spruce_taiga",
    "minecraft:pale_garden",
    "minecraft:plains",
    "minecraft:river",
    "minecraft:savanna",
    "minecraft:savanna_plateau",
    "minecraft:snowy_beach",
    "minecraft:snowy_plains",
    "minecraft:snowy_slopes",
    "minecraft:snowy_taiga",
    "minecraft:sparse_jungle",
    "minecraft:stony_shore",
    "minecraft:sunflower_plains",
    "minecraft:swamp",
    "minecraft:taiga",
    "minecraft:warm_ocean",
    "minecraft:windswept_forest",
    "minecraft:windswept_gravelly_hills",
    "minecraft:windswept_hills",
    "minecraft:windswept_savanna",
    "minecraft:wooded_badlands",
];
const SUNFLOWER_PATCH_BIOMES: &[&str] = &["minecraft:sunflower_plains"];
const DEAD_BUSH_NORMAL_BIOMES: &[&str] = &[
    "minecraft:mangrove_swamp",
    "minecraft:old_growth_pine_taiga",
    "minecraft:old_growth_spruce_taiga",
    "minecraft:swamp",
];
const DEAD_BUSH_DESERT_BIOMES: &[&str] = &["minecraft:desert"];
const DEAD_BUSH_BADLANDS_BIOMES: &[&str] = &[
    "minecraft:badlands",
    "minecraft:eroded_badlands",
    "minecraft:wooded_badlands",
];
const MELON_PATCH_BIOMES: &[&str] = &["minecraft:bamboo_jungle", "minecraft:jungle"];
const MELON_SPARSE_PATCH_BIOMES: &[&str] = &["minecraft:sparse_jungle"];
const SUGAR_CANE_NORMAL_BIOMES: &[&str] = &[
    "minecraft:bamboo_jungle",
    "minecraft:beach",
    "minecraft:birch_forest",
    "minecraft:cold_ocean",
    "minecraft:dark_forest",
    "minecraft:deep_cold_ocean",
    "minecraft:deep_frozen_ocean",
    "minecraft:deep_lukewarm_ocean",
    "minecraft:deep_ocean",
    "minecraft:flower_forest",
    "minecraft:forest",
    "minecraft:frozen_ocean",
    "minecraft:frozen_river",
    "minecraft:ice_spikes",
    "minecraft:jungle",
    "minecraft:lukewarm_ocean",
    "minecraft:mushroom_fields",
    "minecraft:ocean",
    "minecraft:old_growth_birch_forest",
    "minecraft:old_growth_pine_taiga",
    "minecraft:old_growth_spruce_taiga",
    "minecraft:pale_garden",
    "minecraft:plains",
    "minecraft:river",
    "minecraft:savanna",
    "minecraft:savanna_plateau",
    "minecraft:snowy_beach",
    "minecraft:snowy_plains",
    "minecraft:snowy_taiga",
    "minecraft:sparse_jungle",
    "minecraft:stony_shore",
    "minecraft:sunflower_plains",
    "minecraft:taiga",
    "minecraft:warm_ocean",
    "minecraft:windswept_forest",
    "minecraft:windswept_gravelly_hills",
    "minecraft:windswept_hills",
    "minecraft:windswept_savanna",
];
const SUGAR_CANE_BADLANDS_BIOMES: &[&str] = &[
    "minecraft:badlands",
    "minecraft:eroded_badlands",
    "minecraft:wooded_badlands",
];
const SUGAR_CANE_DESERT_BIOMES: &[&str] = &["minecraft:desert"];
const SUGAR_CANE_SWAMP_BIOMES: &[&str] = &["minecraft:swamp"];
const CACTUS_DESERT_BIOMES: &[&str] = &["minecraft:desert"];
const CACTUS_DECORATED_BIOMES: &[&str] = &[
    "minecraft:badlands",
    "minecraft:eroded_badlands",
    "minecraft:wooded_badlands",
];
const DRY_GRASS_DESERT_BIOMES: &[&str] = &["minecraft:desert"];
const DRY_GRASS_BADLANDS_BIOMES: &[&str] = &[
    "minecraft:badlands",
    "minecraft:eroded_badlands",
    "minecraft:wooded_badlands",
];
const TAIGA_MUSHROOM_BIOMES: &[&str] = &[
    "minecraft:mushroom_fields",
    "minecraft:snowy_taiga",
    "minecraft:taiga",
];
const OLD_GROWTH_MUSHROOM_BIOMES: &[&str] = &[
    "minecraft:old_growth_pine_taiga",
    "minecraft:old_growth_spruce_taiga",
];
const SWAMP_MUSHROOM_BIOMES: &[&str] = &["minecraft:swamp"];
const PLAINS_FLOWER_LOW_BLOCKS: &[&str] = &[
    "minecraft:orange_tulip",
    "minecraft:red_tulip",
    "minecraft:pink_tulip",
    "minecraft:white_tulip",
];
const PLAINS_FLOWER_HIGH_BLOCKS: &[&str] = &[
    "minecraft:poppy",
    "minecraft:azure_bluet",
    "minecraft:oxeye_daisy",
    "minecraft:cornflower",
];
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
const GLOW_LICHEN_CAN_BE_PLACED_ON: &[&str] = &[
    "minecraft:stone",
    "minecraft:andesite",
    "minecraft:diorite",
    "minecraft:granite",
    "minecraft:dripstone_block",
    "minecraft:calcite",
    "minecraft:tuff",
    "minecraft:deepslate",
];
const SUPPORTS_VEGETATION_BLOCKS: &[&str] = &[
    "minecraft:dirt",
    "minecraft:coarse_dirt",
    "minecraft:rooted_dirt",
    "minecraft:mud",
    "minecraft:muddy_mangrove_roots",
    "minecraft:moss_block",
    "minecraft:pale_moss_block",
    "minecraft:grass_block",
    "minecraft:podzol",
    "minecraft:mycelium",
    "minecraft:farmland",
];
const CHEST_BLOCK_ENTITY_TYPE_ID: i32 = 1;
const MOB_SPAWNER_BLOCK_ENTITY_TYPE_ID: i32 = 9;
const BEEHIVE_BLOCK_ENTITY_TYPE_ID: i32 = 34;

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

fn worldgen_gpu_from_config(
    config: &WorldGpu,
    height: i32,
) -> Option<Arc<gpu_worldgen::GpuWorldgenEngine>> {
    if !config.enable {
        return None;
    }

    match gpu_worldgen::GpuWorldgenEngine::new(&config.device, height) {
        Ok(engine) => Some(Arc::new(engine)),
        Err(err) => {
            log::warn!("GPU 世界生成后处理初始化失败，已回退 CPU: {err:#}");
            None
        }
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
    gpu_worldgen: Option<Arc<gpu_worldgen::GpuWorldgenEngine>>,
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
            Ok(settings) => Self::from_settings(settings, config),
            Err(err) => {
                log::warn!(
                    "failed to load vanilla noise settings {preset}, using overworld: {err:#}"
                );
                Self::from_settings(
                    NoiseSettings::overworld(
                        config.seed,
                        vanilla_noise::OverworldNoiseKind::Default,
                    ),
                    config,
                )
            }
        }
    }

    fn from_settings(settings: NoiseSettings, config: &WorldConfig) -> Self {
        let gpu_worldgen = worldgen_gpu_from_config(&config.gpu, settings.height);
        Self {
            settings,
            gpu_worldgen,
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
        let total_start = Instant::now();
        let (chunk, timings) =
            self.settings
                .generate_chunk_profiled(chunk_x, chunk_z, self.gpu_worldgen.as_deref());

        let root_start = Instant::now();
        let root = noise_chunk_root(&chunk, self.settings.biome.as_str());
        let root_elapsed = root_start.elapsed();

        let packet_start = Instant::now();
        let (packet, light_dampening) = chunk_nbt::network_chunk_and_light_dampening_from_nbt(
            chunk_x,
            chunk_z,
            &root,
            light_algorithm,
        )?;
        let packet_elapsed = packet_start.elapsed();

        let block_entities_start = Instant::now();
        let mut packet = packet;
        packet.data.block_entities = chunk.block_entities_as_packet(chunk_x, chunk_z);
        let block_entities_elapsed = block_entities_start.elapsed();

        log_noise_chunk_timings(
            chunk_x,
            chunk_z,
            total_start.elapsed(),
            &timings,
            root_elapsed,
            packet_elapsed,
            block_entities_elapsed,
        );

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

#[derive(Debug, Clone, Copy)]
struct NoiseChunkTimings {
    base: Duration,
    carvers: Duration,
    features: Duration,
    heightmap: Duration,
}

fn log_noise_chunk_timings(
    chunk_x: i32,
    chunk_z: i32,
    total: Duration,
    timings: &NoiseChunkTimings,
    root: Duration,
    packet: Duration,
    block_entities: Duration,
) {
    if total < SLOW_NOISE_CHUNK_LOG_THRESHOLD || !log::log_enabled!(log::Level::Debug) {
        return;
    }

    log::debug!(
        "vanilla_noise 区块生成耗时: chunk=({chunk_x}, {chunk_z}), total_ms={:.2}, base_ms={:.2}, carvers_ms={:.2}, features_ms={:.2}, heightmap_ms={:.2}, nbt_root_ms={:.2}, packet_ms={:.2}, block_entities_ms={:.2}",
        duration_ms(total),
        duration_ms(timings.base),
        duration_ms(timings.carvers),
        duration_ms(timings.features),
        duration_ms(timings.heightmap),
        duration_ms(root),
        duration_ms(packet),
        duration_ms(block_entities)
    );
}

fn duration_ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
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
    air_block: BlockLayer,
    bedrock_block: BlockLayer,
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
    lava_lake_fluid_block: BlockLayer,
    lava_lake_barrier_block: BlockLayer,
    cave_air_block: BlockLayer,
}

impl NoiseSettings {
    fn overworld(seed: i64, noise_kind: vanilla_noise::OverworldNoiseKind) -> Self {
        let surface_rules = vanilla_noise::OverworldSurfaceRules::new(seed);
        let sea_level = 63;
        Self {
            min_y: WORLD_MIN_Y,
            height: super::WORLD_SECTION_COUNT as i32 * SECTION_HEIGHT,
            sea_level,
            air_block: BlockLayer::new("minecraft:air"),
            bedrock_block: BlockLayer::new("minecraft:bedrock"),
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
            lava_lake_fluid_block: BlockLayer::new("minecraft:lava"),
            lava_lake_barrier_block: BlockLayer::new("minecraft:stone"),
            cave_air_block: BlockLayer::new("minecraft:cave_air"),
        }
    }

    #[cfg(test)]
    fn generate_chunk(&self, chunk_x: i32, chunk_z: i32) -> NoiseChunkBlocks {
        self.generate_chunk_with_gpu(chunk_x, chunk_z, None)
    }

    #[cfg(test)]
    fn generate_chunk_with_gpu(
        &self,
        chunk_x: i32,
        chunk_z: i32,
        gpu: Option<&gpu_worldgen::GpuWorldgenEngine>,
    ) -> NoiseChunkBlocks {
        self.generate_chunk_profiled(chunk_x, chunk_z, gpu).0
    }

    fn generate_chunk_profiled(
        &self,
        chunk_x: i32,
        chunk_z: i32,
        gpu: Option<&gpu_worldgen::GpuWorldgenEngine>,
    ) -> (NoiseChunkBlocks, NoiseChunkTimings) {
        let base_start = Instant::now();
        let (mut chunk, preliminary_surfaces) = self.generate_base_chunk(chunk_x, chunk_z);
        let base = base_start.elapsed();

        let carvers_start = Instant::now();
        self.carvers
            .carve_chunk(self, chunk_x, chunk_z, &preliminary_surfaces, &mut chunk);
        let carvers = carvers_start.elapsed();

        let features_start = Instant::now();
        self.ore_features
            .place_chunk(self, chunk_x, chunk_z, &mut chunk);
        let features = features_start.elapsed();

        let heightmap_start = Instant::now();
        chunk.recompute_first_available_heights_accelerated(self.min_y, self.height, gpu);
        let heightmap = heightmap_start.elapsed();

        (
            chunk,
            NoiseChunkTimings {
                base,
                carvers,
                features,
                heightmap,
            },
        )
    }

    fn generate_base_chunk(&self, chunk_x: i32, chunk_z: i32) -> (NoiseChunkBlocks, Vec<i32>) {
        let mut profiles = Vec::with_capacity((17 * 17) as usize);
        let mut preliminary_surfaces = Vec::with_capacity(HEIGHTMAP_ENTRY_COUNT);

        for z in 0..=16 {
            for x in 0..=16 {
                let world_x = chunk_x * 16 + x;
                let world_z = chunk_z * 16 + z;
                let profile = self.density.profile(world_x, world_z);
                profiles.push(profile);
                if x < 16 && z < 16 {
                    preliminary_surfaces.push(self.preliminary_surface_with_profile(
                        world_x,
                        world_z,
                        profiles.last().expect("profile was just pushed"),
                    ));
                }
            }
        }

        let column_density = (0..HEIGHTMAP_ENTRY_COUNT)
            .into_par_iter()
            .map(|column| {
                let x = (column % 16) as i32;
                let z = (column / 16) as i32;
                let world_x = chunk_x * 16 + x;
                let world_z = chunk_z * 16 + z;
                let index = (z * 17 + x) as usize;
                self.column_density_cache(world_x, world_z, &profiles[index])
            })
            .collect::<Vec<_>>();

        let mut surface_heights = vec![self.min_y; (17 * 17) as usize];
        for z in 0..=16 {
            for x in 0..=16 {
                let index = (z * 17 + x) as usize;
                surface_heights[index] = if x < 16 && z < 16 {
                    column_density[(z * 16 + x) as usize].surface_height
                } else {
                    let world_x = chunk_x * 16 + x;
                    let world_z = chunk_z * 16 + z;
                    self.surface_height_with_profile(world_x, world_z, &profiles[index])
                };
            }
        }

        let columns = (0..HEIGHTMAP_ENTRY_COUNT)
            .into_par_iter()
            .map(|column| {
                let x = (column % 16) as i32;
                let z = (column / 16) as i32;
                let world_x = chunk_x * 16 + x;
                let world_z = chunk_z * 16 + z;
                let index = (z * 17 + x) as usize;
                let surface_height = surface_heights[index];
                let east_height = surface_heights[index + 1];
                let south_height = surface_heights[index + 17];
                let slope = (east_height - surface_height)
                    .abs()
                    .max((south_height - surface_height).abs());
                self.generate_column_with_profile(
                    world_x,
                    world_z,
                    &column_density[column],
                    surface_height,
                    preliminary_surfaces[column],
                    slope,
                )
            })
            .collect();
        (
            NoiseChunkBlocks {
                columns,
                biomes: self.generate_biomes(chunk_x, chunk_z),
                block_entities: Vec::new(),
            },
            preliminary_surfaces,
        )
    }

    fn generate_biomes(&self, chunk_x: i32, chunk_z: i32) -> Vec<&'static str> {
        (0..section_count() as usize * 64)
            .into_par_iter()
            .map(|index| {
                let section_offset = index / 64;
                let cell = index % 64;
                let local_x = (cell / 16) as i32;
                let local_y = ((cell / 4) % 4) as i32;
                let local_z = (cell % 4) as i32;
                let section_y = WORLD_MIN_SECTION_Y + section_offset as i32;
                let world_x = chunk_x * 16 + local_x * 4;
                let world_y = section_y * SECTION_HEIGHT + local_y * 4;
                let world_z = chunk_z * 16 + local_z * 4;
                self.density.biome(world_x, world_y, world_z)
            })
            .collect()
    }

    fn generate_column_with_profile(
        &self,
        world_x: i32,
        world_z: i32,
        density_cache: &ColumnDensityCache,
        surface_height: i32,
        preliminary_surface: i32,
        surface_slope: i32,
    ) -> NoiseColumnBlocks {
        let mut blocks = Vec::with_capacity(self.height as usize);
        for y in self.min_y..self.min_y + self.height {
            blocks.push(self.layer_at_with_density(
                world_x,
                y,
                world_z,
                density_cache.density_at(y, self.min_y),
                surface_height,
                preliminary_surface,
                surface_slope,
            ));
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

    fn column_density_cache(
        &self,
        x: i32,
        z: i32,
        profile: &vanilla_noise::OverworldTerrainProfile,
    ) -> ColumnDensityCache {
        let mut densities = Vec::with_capacity(self.height as usize + 1);
        let density_column = self.density.column_sampler(x, z, profile);
        for y in self.min_y..=self.min_y + self.height {
            densities.push(density_column.sample(y));
        }

        let surface_height = densities
            .iter()
            .rposition(|density| *density > 0.0)
            .map(|index| self.min_y + index as i32)
            .unwrap_or(self.min_y)
            .clamp(self.min_y + 1, self.min_y + self.height - 1);

        ColumnDensityCache {
            surface_height,
            densities,
        }
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
        let density = self.density.sample_with_profile(x, y, z, profile);
        self.layer_at_with_density(
            x,
            y,
            z,
            density,
            surface_height,
            preliminary_surface,
            surface_slope,
        )
    }

    fn layer_at_with_density(
        &self,
        x: i32,
        y: i32,
        z: i32,
        density: f64,
        surface_height: i32,
        preliminary_surface: i32,
        surface_slope: i32,
    ) -> BlockLayer {
        if y <= self.min_y {
            return self.bedrock_block.clone();
        }

        let density = if y <= surface_height {
            density
        } else {
            density.min(-1.0)
        };
        if density > 0.0 {
            if self.surface_rules.is_bedrock_floor(x, y, z, self.min_y) {
                return self.bedrock_block.clone();
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
                    self.air_block.clone()
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
            vanilla_noise::SurfaceBlock::Bedrock => self.bedrock_block.clone(),
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
        self.air_block.clone()
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
        let density_column = self.column_sampler(x, z, profile);
        (-64..=320)
            .rev()
            .find(|y| density_column.sample(*y) > 0.0)
            .unwrap_or(-64)
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

    fn column_sampler<'a>(
        &'a self,
        x: i32,
        z: i32,
        profile: &'a vanilla_noise::OverworldTerrainProfile,
    ) -> TerrainDensityColumn<'a> {
        TerrainDensityColumn {
            blended_noise: self.blended_noise.column_sampler(x, z),
            terrain_noise: self.terrain_noise.column_sampler(profile, x, z),
        }
    }
}

struct TerrainDensityColumn<'a> {
    blended_noise: vanilla_noise::BlendedNoiseColumn<'a>,
    terrain_noise: vanilla_noise::OverworldTerrainColumn<'a>,
}

impl TerrainDensityColumn<'_> {
    fn sample(&self, y: i32) -> f64 {
        let base_3d = self.blended_noise.compute(y);
        self.terrain_noise.final_density(y, base_3d)
    }
}

#[derive(Debug, Clone)]
struct ColumnDensityCache {
    surface_height: i32,
    densities: Vec<f64>,
}

impl ColumnDensityCache {
    fn density_at(&self, y: i32, min_y: i32) -> f64 {
        let index = (y - min_y) as usize;
        self.densities[index]
    }
}

#[derive(Debug, Clone)]
struct NoiseChunkBlocks {
    columns: Vec<NoiseColumnBlocks>,
    biomes: Vec<&'static str>,
    block_entities: Vec<GeneratedBlockEntity>,
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
        if let Ok(index) = usize::try_from(y - min_y) {
            let is_air = layer.is_air;
            if !layer.is("minecraft:chest") && !layer.is("minecraft:spawner") {
                self.remove_block_entity_by_local(x, y, z);
            }
            let column = self.column_mut(x, z);
            if let Some(block) = column.blocks.get_mut(index) {
                *block = layer;
                update_first_available_height(column, index, is_air);
            }
        }
    }

    fn push_block_entity(
        &mut self,
        world_x: i32,
        world_y: i32,
        world_z: i32,
        entity_type: i32,
        nbt: Tag,
    ) {
        self.block_entities
            .retain(|entity| entity.position != (world_x, world_y, world_z));
        self.block_entities.push(GeneratedBlockEntity {
            position: (world_x, world_y, world_z),
            entity_type,
            nbt,
        });
    }

    fn remove_block_entity_by_local(&mut self, local_x: usize, world_y: i32, local_z: usize) {
        self.block_entities.retain(|entity| {
            entity.position.1 != world_y
                || entity.position.0.rem_euclid(16) as usize != local_x
                || entity.position.2.rem_euclid(16) as usize != local_z
        });
    }

    fn block_entities_as_packet(&self, chunk_x: i32, chunk_z: i32) -> Vec<BlockEntities> {
        let chunk_min_x = chunk_x * 16;
        let chunk_min_z = chunk_z * 16;
        let mut entities = self
            .block_entities
            .iter()
            .filter_map(|entity| {
                let local_x = local_coord(entity.position.0, chunk_min_x)?;
                let local_z = local_coord(entity.position.2, chunk_min_z)?;
                Some(BlockEntities {
                    xz: ((local_x as u8) << 4) | local_z as u8,
                    y: entity.position.1 as u16,
                    entity_type: VarInt(entity.entity_type),
                    nbt: OptionalNbt(Some(entity.nbt.clone())),
                })
            })
            .collect::<Vec<_>>();
        entities.sort_by_key(|entity| (entity.y, entity.xz));
        entities
    }

    fn biome(&self, section_y: i32, x: usize, y: usize, z: usize) -> &'static str {
        let section_index = (section_y - WORLD_MIN_SECTION_Y) as usize;
        self.biomes[((section_index * 4 + x) * 4 + y) * 4 + z]
    }

    fn ocean_floor_wg_height(&self, x: usize, z: usize, min_y: i32) -> i32 {
        let column = self.column(x, z);
        let top = column
            .first_available_height
            .clamp(0, column.blocks.len() as i32) as usize;
        column
            .blocks
            .get(..top)
            .unwrap_or(&column.blocks)
            .iter()
            .rposition(is_full_solid_layer)
            .map(|index| min_y + index as i32 + 1)
            .unwrap_or(min_y)
    }

    fn world_surface_wg_height(&self, x: usize, z: usize, min_y: i32) -> i32 {
        min_y + self.column(x, z).first_available_height
    }

    fn recompute_first_available_heights_accelerated(
        &mut self,
        min_y: i32,
        height: i32,
        gpu: Option<&gpu_worldgen::GpuWorldgenEngine>,
    ) {
        if let Some(gpu) = gpu
            && height as usize * HEIGHTMAP_ENTRY_COUNT == CHUNK_DAMPENING_LEN
        {
            match gpu.first_available_heights(&self.occupied_mask(height)) {
                Ok(heights) if heights.len() == HEIGHTMAP_ENTRY_COUNT => {
                    for (column, height) in self.columns.iter_mut().zip(heights) {
                        column.first_available_height = height;
                    }
                    return;
                }
                Ok(heights) => {
                    log::warn!(
                        "GPU 世界生成后处理返回高度图长度异常，已回退 CPU: expected={}, got={}",
                        HEIGHTMAP_ENTRY_COUNT,
                        heights.len()
                    );
                }
                Err(err) => {
                    log::warn!("GPU 世界生成后处理失败，已回退 CPU: {err:#}");
                }
            }
        }

        self.recompute_first_available_heights(min_y, height);
    }

    fn occupied_mask(&self, height: i32) -> Vec<u32> {
        let mut mask = vec![0_u32; height as usize * HEIGHTMAP_ENTRY_COUNT];
        for z in 0..16 {
            for x in 0..16 {
                for (y, layer) in self.column(x, z).blocks.iter().enumerate() {
                    if !layer.is_air {
                        mask[y * HEIGHTMAP_ENTRY_COUNT + z * 16 + x] = 1;
                    }
                }
            }
        }
        mask
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

fn update_first_available_height(column: &mut NoiseColumnBlocks, index: usize, is_air: bool) {
    let height = index as i32 + 1;
    if is_air {
        if column.first_available_height == height {
            column.first_available_height = column
                .blocks
                .iter()
                .rposition(|layer| !layer.is_air)
                .map(|index| index as i32 + 1)
                .unwrap_or(0);
        }
    } else if height > column.first_available_height {
        column.first_available_height = height;
    }
}

#[derive(Debug, Clone)]
struct GeneratedBlockEntity {
    position: (i32, i32, i32),
    entity_type: i32,
    nbt: Tag,
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
        layer.block.as_ref(),
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

    fn next_double(&mut self) -> f64 {
        let upper = self.next(26) as i64;
        let lower = self.next(27) as i64;
        ((upper << 27) + lower) as f64 * (1.110_223e-16_f32 as f64)
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
    lakes: Vec<PlacedLakeFeature>,
    geodes: Vec<PlacedGeodeFeature>,
    monster_rooms: Vec<PlacedMonsterRoomFeature>,
    glow_lichen: PlacedMultifaceGrowthFeature,
    vegetation_patches: Vec<PlacedSimpleVegetationFeature>,
    block_columns: Vec<PlacedBlockColumnFeature>,
    trees_plains: PlacedTreeFeature,
    freeze_top_layer: PlacedFreezeTopLayerFeature,
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
            lakes: vec![
                PlacedLakeFeature::lava_underground(0),
                PlacedLakeFeature::lava_surface(1),
            ],
            geodes: vec![PlacedGeodeFeature::amethyst(0)],
            monster_rooms: vec![
                PlacedMonsterRoomFeature::regular(0),
                PlacedMonsterRoomFeature::deep(1),
            ],
            glow_lichen: PlacedMultifaceGrowthFeature::glow_lichen(0),
            vegetation_patches: vec![
                PlacedSimpleVegetationFeature::patch_tall_grass_2(1)
                    .with_biome_filter(FeatureBiomeFilter::Include(PATCH_TALL_GRASS_2_BIOMES)),
                PlacedSimpleVegetationFeature::patch_bush(2)
                    .with_biome_filter(FeatureBiomeFilter::Include(PATCH_BUSH_BIOMES)),
                PlacedSimpleVegetationFeature::patch_sunflower(3)
                    .with_biome_filter(FeatureBiomeFilter::Include(SUNFLOWER_PATCH_BIOMES)),
                PlacedSimpleVegetationFeature::flower_plains(4)
                    .with_biome_filter(FeatureBiomeFilter::Include(FLOWER_PLAINS_BIOMES)),
                PlacedSimpleVegetationFeature::patch_grass_plain(5)
                    .with_biome_filter(FeatureBiomeFilter::Include(PATCH_GRASS_PLAIN_BIOMES)),
                PlacedSimpleVegetationFeature::brown_mushroom_normal(6)
                    .with_biome_filter(FeatureBiomeFilter::Include(NORMAL_MUSHROOM_BIOMES)),
                PlacedSimpleVegetationFeature::red_mushroom_normal(7)
                    .with_biome_filter(FeatureBiomeFilter::Include(NORMAL_MUSHROOM_BIOMES)),
                PlacedSimpleVegetationFeature::patch_pumpkin(8)
                    .with_biome_filter(FeatureBiomeFilter::Include(PUMPKIN_PATCH_BIOMES)),
                PlacedSimpleVegetationFeature::patch_dead_bush(9, 1)
                    .with_biome_filter(FeatureBiomeFilter::Include(DEAD_BUSH_NORMAL_BIOMES)),
                PlacedSimpleVegetationFeature::patch_dead_bush(10, 2)
                    .with_biome_filter(FeatureBiomeFilter::Include(DEAD_BUSH_DESERT_BIOMES)),
                PlacedSimpleVegetationFeature::patch_dead_bush(11, 20)
                    .with_biome_filter(FeatureBiomeFilter::Include(DEAD_BUSH_BADLANDS_BIOMES)),
                PlacedSimpleVegetationFeature::patch_melon(12, 6)
                    .with_biome_filter(FeatureBiomeFilter::Include(MELON_PATCH_BIOMES)),
                PlacedSimpleVegetationFeature::patch_melon(13, 64)
                    .with_biome_filter(FeatureBiomeFilter::Include(MELON_SPARSE_PATCH_BIOMES)),
                PlacedSimpleVegetationFeature::patch_grass_normal(20)
                    .with_biome_filter(FeatureBiomeFilter::Include(PATCH_GRASS_NORMAL_BIOMES)),
                PlacedSimpleVegetationFeature::patch_grass_forest(21)
                    .with_biome_filter(FeatureBiomeFilter::Include(PATCH_GRASS_FOREST_BIOMES)),
                PlacedSimpleVegetationFeature::patch_grass_badlands(22)
                    .with_biome_filter(FeatureBiomeFilter::Include(PATCH_GRASS_BADLANDS_BIOMES)),
                PlacedSimpleVegetationFeature::patch_grass_savanna(23)
                    .with_biome_filter(FeatureBiomeFilter::Include(PATCH_GRASS_SAVANNA_BIOMES)),
                PlacedSimpleVegetationFeature::patch_grass_taiga(24)
                    .with_biome_filter(FeatureBiomeFilter::Include(PATCH_GRASS_TAIGA_BIOMES)),
                PlacedSimpleVegetationFeature::patch_grass_taiga_2(25)
                    .with_biome_filter(FeatureBiomeFilter::Include(PATCH_GRASS_TAIGA_2_BIOMES)),
                PlacedSimpleVegetationFeature::patch_grass_jungle(26)
                    .with_biome_filter(FeatureBiomeFilter::Include(PATCH_GRASS_JUNGLE_BIOMES)),
                PlacedSimpleVegetationFeature::patch_grass_meadow(27)
                    .with_biome_filter(FeatureBiomeFilter::Include(PATCH_GRASS_MEADOW_BIOMES)),
                PlacedSimpleVegetationFeature::patch_large_fern(28)
                    .with_biome_filter(FeatureBiomeFilter::Include(PATCH_LARGE_FERN_BIOMES)),
                PlacedSimpleVegetationFeature::patch_dry_grass(29, 3)
                    .with_biome_filter(FeatureBiomeFilter::Include(DRY_GRASS_DESERT_BIOMES)),
                PlacedSimpleVegetationFeature::patch_dry_grass(30, 6)
                    .with_biome_filter(FeatureBiomeFilter::Include(DRY_GRASS_BADLANDS_BIOMES)),
                PlacedSimpleVegetationFeature::brown_mushroom_taiga(31)
                    .with_biome_filter(FeatureBiomeFilter::Include(TAIGA_MUSHROOM_BIOMES)),
                PlacedSimpleVegetationFeature::red_mushroom_taiga(32)
                    .with_biome_filter(FeatureBiomeFilter::Include(TAIGA_MUSHROOM_BIOMES)),
                PlacedSimpleVegetationFeature::brown_mushroom_old_growth(33)
                    .with_biome_filter(FeatureBiomeFilter::Include(OLD_GROWTH_MUSHROOM_BIOMES)),
                PlacedSimpleVegetationFeature::red_mushroom_old_growth(34)
                    .with_biome_filter(FeatureBiomeFilter::Include(OLD_GROWTH_MUSHROOM_BIOMES)),
                PlacedSimpleVegetationFeature::brown_mushroom_swamp(35)
                    .with_biome_filter(FeatureBiomeFilter::Include(SWAMP_MUSHROOM_BIOMES)),
                PlacedSimpleVegetationFeature::red_mushroom_swamp(36)
                    .with_biome_filter(FeatureBiomeFilter::Include(SWAMP_MUSHROOM_BIOMES)),
            ],
            block_columns: vec![
                PlacedBlockColumnFeature::sugar_cane(14, 6)
                    .with_biome_filter(FeatureBiomeFilter::Include(SUGAR_CANE_NORMAL_BIOMES)),
                PlacedBlockColumnFeature::sugar_cane(15, 5)
                    .with_biome_filter(FeatureBiomeFilter::Include(SUGAR_CANE_BADLANDS_BIOMES)),
                PlacedBlockColumnFeature::sugar_cane(16, 1)
                    .with_biome_filter(FeatureBiomeFilter::Include(SUGAR_CANE_DESERT_BIOMES)),
                PlacedBlockColumnFeature::sugar_cane(17, 3)
                    .with_biome_filter(FeatureBiomeFilter::Include(SUGAR_CANE_SWAMP_BIOMES)),
                PlacedBlockColumnFeature::cactus(18, 6)
                    .with_biome_filter(FeatureBiomeFilter::Include(CACTUS_DESERT_BIOMES)),
                PlacedBlockColumnFeature::cactus(19, 13)
                    .with_biome_filter(FeatureBiomeFilter::Include(CACTUS_DECORATED_BIOMES)),
            ],
            trees_plains: PlacedTreeFeature::trees_plains(3),
            freeze_top_layer: PlacedFreezeTopLayerFeature::new(0),
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

        let mut features = Vec::with_capacity(
            self.features.len()
                + self.disks.len()
                + self.springs.len()
                + self.lakes.len()
                + self.geodes.len()
                + self.monster_rooms.len()
                + self.vegetation_patches.len()
                + self.block_columns.len()
                + 4,
        );
        features.extend(self.lakes.iter().map(PlacedUndergroundFeature::Lake));
        features.extend(self.geodes.iter().map(PlacedUndergroundFeature::Geode));
        features.extend(
            self.monster_rooms
                .iter()
                .map(PlacedUndergroundFeature::MonsterRoom),
        );
        features.extend(self.features.iter().map(PlacedUndergroundFeature::Ore));
        features.push(PlacedUndergroundFeature::UnderwaterMagma(
            &self.underwater_magma,
        ));
        features.extend(self.disks.iter().map(PlacedUndergroundFeature::Disk));
        features.extend(self.springs.iter().map(PlacedUndergroundFeature::Spring));
        features.push(PlacedUndergroundFeature::MultifaceGrowth(&self.glow_lichen));
        features.extend(
            self.vegetation_patches
                .iter()
                .map(PlacedUndergroundFeature::SimpleVegetation),
        );
        features.extend(
            self.block_columns
                .iter()
                .map(PlacedUndergroundFeature::BlockColumn),
        );
        features.push(PlacedUndergroundFeature::Tree(&self.trees_plains));
        features.push(PlacedUndergroundFeature::FreezeTopLayer(
            &self.freeze_top_layer,
        ));
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
    Lake(&'a PlacedLakeFeature),
    Geode(&'a PlacedGeodeFeature),
    MonsterRoom(&'a PlacedMonsterRoomFeature),
    Ore(&'a PlacedOreFeature),
    UnderwaterMagma(&'a PlacedUnderwaterMagmaFeature),
    Disk(&'a PlacedDiskFeature),
    Spring(&'a PlacedSpringFeature),
    MultifaceGrowth(&'a PlacedMultifaceGrowthFeature),
    SimpleVegetation(&'a PlacedSimpleVegetationFeature),
    BlockColumn(&'a PlacedBlockColumnFeature),
    Tree(&'a PlacedTreeFeature),
    FreezeTopLayer(&'a PlacedFreezeTopLayerFeature),
}

impl PlacedUndergroundFeature<'_> {
    fn step_index(self) -> i32 {
        match self {
            Self::Lake(feature) => feature.step_index,
            Self::Geode(feature) => feature.step_index,
            Self::MonsterRoom(feature) => feature.step_index,
            Self::Ore(feature) => feature.step_index,
            Self::UnderwaterMagma(feature) => feature.step_index,
            Self::Disk(feature) => feature.step_index,
            Self::Spring(feature) => feature.step_index,
            Self::MultifaceGrowth(feature) => feature.step_index,
            Self::SimpleVegetation(feature) => feature.step_index,
            Self::BlockColumn(feature) => feature.step_index,
            Self::Tree(feature) => feature.step_index,
            Self::FreezeTopLayer(feature) => feature.step_index,
        }
    }

    fn feature_index(self) -> i32 {
        match self {
            Self::Lake(feature) => feature.feature_index,
            Self::Geode(feature) => feature.feature_index,
            Self::MonsterRoom(feature) => feature.feature_index,
            Self::Ore(feature) => feature.feature_index,
            Self::UnderwaterMagma(feature) => feature.feature_index,
            Self::Disk(feature) => feature.feature_index,
            Self::Spring(feature) => feature.feature_index,
            Self::MultifaceGrowth(feature) => feature.feature_index,
            Self::SimpleVegetation(feature) => feature.feature_index,
            Self::BlockColumn(feature) => feature.feature_index,
            Self::Tree(feature) => feature.feature_index,
            Self::FreezeTopLayer(feature) => feature.feature_index,
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
            Self::Lake(feature) => feature.place(settings, origin_x, origin_z, chunk, random),
            Self::Geode(feature) => feature.place(settings, origin_x, origin_z, chunk, random),
            Self::MonsterRoom(feature) => {
                feature.place(settings, origin_x, origin_z, chunk, random);
            }
            Self::Ore(feature) => feature.place(settings, origin_x, origin_z, chunk, random),
            Self::UnderwaterMagma(feature) => {
                feature.place(settings, origin_x, origin_z, chunk, random);
            }
            Self::Disk(feature) => feature.place(settings, origin_x, origin_z, chunk, random),
            Self::Spring(feature) => feature.place(settings, origin_x, origin_z, chunk, random),
            Self::MultifaceGrowth(feature) => {
                feature.place(settings, origin_x, origin_z, chunk, random);
            }
            Self::SimpleVegetation(feature) => {
                feature.place(settings, origin_x, origin_z, chunk, random);
            }
            Self::BlockColumn(feature) => {
                feature.place(settings, origin_x, origin_z, chunk, random);
            }
            Self::Tree(feature) => {
                feature.place(settings, origin_x, origin_z, chunk, random);
            }
            Self::FreezeTopLayer(feature) => feature.place(settings, origin_x, origin_z, chunk),
        }
    }
}

#[derive(Debug, Clone)]
struct PlacedLakeFeature {
    step_index: i32,
    feature_index: i32,
    placement: LakePlacement,
    biome_filter: FeatureBiomeFilter,
}

impl PlacedLakeFeature {
    fn lava_underground(feature_index: i32) -> Self {
        Self {
            step_index: 1,
            feature_index,
            placement: LakePlacement::Underground {
                rarity: 9,
                height: OreHeight::Uniform(HeightAnchor::Absolute(0), HeightAnchor::BelowTop(0)),
                max_scan_steps: 32,
            },
            biome_filter: FeatureBiomeFilter::All,
        }
    }

    fn lava_surface(feature_index: i32) -> Self {
        Self {
            step_index: 1,
            feature_index,
            placement: LakePlacement::Surface { rarity: 200 },
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
        let Some((world_x, world_y, world_z)) =
            self.sample_origin(settings, origin_x, origin_z, chunk, random)
        else {
            return;
        };

        if self
            .biome_filter
            .allows_at(&settings.density, world_x, world_y, world_z)
        {
            LakeFeatureConfig::lava().place(
                settings, origin_x, origin_z, chunk, random, world_x, world_y, world_z,
            );
        }
    }

    fn sample_origin(
        &self,
        settings: &NoiseSettings,
        origin_x: i32,
        origin_z: i32,
        chunk: &NoiseChunkBlocks,
        random: &mut FeatureRandom,
    ) -> Option<(i32, i32, i32)> {
        match self.placement {
            LakePlacement::Underground {
                rarity,
                height,
                max_scan_steps,
            } => {
                if random.next_float() >= 1.0 / rarity as f32 {
                    return None;
                }
                let world_x = origin_x + random.next_int(16);
                let world_z = origin_z + random.next_int(16);
                let sampled_y = height.sample(settings, random);
                let local_x = (world_x - origin_x) as usize;
                let local_z = (world_z - origin_z) as usize;
                let world_y = scan_down_to_solid(
                    settings,
                    chunk,
                    local_x,
                    sampled_y,
                    local_z,
                    max_scan_steps,
                )?;
                let ocean_floor = chunk.ocean_floor_wg_height(local_x, local_z, settings.min_y);
                (world_y <= ocean_floor - 5).then_some((world_x, world_y, world_z))
            }
            LakePlacement::Surface { rarity } => {
                if random.next_float() >= 1.0 / rarity as f32 {
                    return None;
                }
                let world_x = origin_x + random.next_int(16);
                let world_z = origin_z + random.next_int(16);
                let local_x = (world_x - origin_x) as usize;
                let local_z = (world_z - origin_z) as usize;
                let world_y = chunk.world_surface_wg_height(local_x, local_z, settings.min_y);
                (world_y > settings.min_y).then_some((world_x, world_y, world_z))
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum LakePlacement {
    Underground {
        rarity: i32,
        height: OreHeight,
        max_scan_steps: i32,
    },
    Surface {
        rarity: i32,
    },
}

#[derive(Debug, Clone)]
struct LakeFeatureConfig;

impl LakeFeatureConfig {
    fn lava() -> Self {
        Self
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
        if origin_y <= settings.min_y + 4 {
            return false;
        }

        let base_x = origin_x - 8;
        let base_y = origin_y - 4;
        let base_z = origin_z - 8;
        let mut grid = vec![false; 16 * 16 * 8];
        let spots = random.next_int(4) + 4;

        for _ in 0..spots {
            let xr = random.next_double() * 6.0 + 3.0;
            let yr = random.next_double() * 4.0 + 2.0;
            let zr = random.next_double() * 6.0 + 3.0;
            let xp = random.next_double() * (16.0 - xr - 2.0) + 1.0 + xr / 2.0;
            let yp = random.next_double() * (8.0 - yr - 4.0) + 2.0 + yr / 2.0;
            let zp = random.next_double() * (16.0 - zr - 2.0) + 1.0 + zr / 2.0;

            for xx in 1..15 {
                for zz in 1..15 {
                    for yy in 1..7 {
                        let xd = (xx as f64 - xp) / (xr / 2.0);
                        let yd = (yy as f64 - yp) / (yr / 2.0);
                        let zd = (zz as f64 - zp) / (zr / 2.0);
                        if xd * xd + yd * yd + zd * zd < 1.0 {
                            grid[lake_index(xx, yy, zz)] = true;
                        }
                    }
                }
            }
        }

        if !self.can_place(
            settings,
            chunk_min_x,
            chunk_min_z,
            chunk,
            base_x,
            base_y,
            base_z,
            &grid,
        ) {
            return false;
        }

        for xx in 0..16 {
            for zz in 0..16 {
                for yy in 0..8 {
                    if !grid[lake_index(xx, yy, zz)] {
                        continue;
                    }
                    let world_x = base_x + xx as i32;
                    let world_y = base_y + yy as i32;
                    let world_z = base_z + zz as i32;
                    let Some(local_x) = local_coord(world_x, chunk_min_x) else {
                        continue;
                    };
                    let Some(local_z) = local_coord(world_z, chunk_min_z) else {
                        continue;
                    };
                    if chunk
                        .layer(local_x, world_y, local_z, settings.min_y)
                        .is_some_and(can_lake_replace_block)
                    {
                        let layer = if yy >= 4 {
                            settings.cave_air_block.clone()
                        } else {
                            settings.lava_lake_fluid_block.clone()
                        };
                        chunk.set_layer(local_x, world_y, local_z, settings.min_y, layer);
                    }
                }
            }
        }

        for xx in 0..16 {
            for zz in 0..16 {
                for yy in 0..8 {
                    if grid[lake_index(xx, yy, zz)]
                        || !is_lake_boundary(&grid, xx, yy, zz)
                        || (yy >= 4 && random.next_int(2) == 0)
                    {
                        continue;
                    }
                    let world_x = base_x + xx as i32;
                    let world_y = base_y + yy as i32;
                    let world_z = base_z + zz as i32;
                    let Some(local_x) = local_coord(world_x, chunk_min_x) else {
                        continue;
                    };
                    let Some(local_z) = local_coord(world_z, chunk_min_z) else {
                        continue;
                    };
                    if chunk
                        .layer(local_x, world_y, local_z, settings.min_y)
                        .is_some_and(is_full_solid_layer)
                    {
                        chunk.set_layer(
                            local_x,
                            world_y,
                            local_z,
                            settings.min_y,
                            settings.lava_lake_barrier_block.clone(),
                        );
                    }
                }
            }
        }

        true
    }

    #[allow(clippy::too_many_arguments)]
    fn can_place(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &NoiseChunkBlocks,
        base_x: i32,
        base_y: i32,
        base_z: i32,
        grid: &[bool],
    ) -> bool {
        for xx in 0..16 {
            for zz in 0..16 {
                for yy in 0..8 {
                    if grid[lake_index(xx, yy, zz)] || !is_lake_boundary(grid, xx, yy, zz) {
                        continue;
                    }
                    let world_x = base_x + xx as i32;
                    let world_y = base_y + yy as i32;
                    let world_z = base_z + zz as i32;
                    let Some(local_x) = local_coord(world_x, chunk_min_x) else {
                        continue;
                    };
                    let Some(local_z) = local_coord(world_z, chunk_min_z) else {
                        continue;
                    };
                    let Some(layer) = chunk.layer(local_x, world_y, local_z, settings.min_y) else {
                        return false;
                    };
                    if yy >= 4 && is_fluid_layer(layer) {
                        return false;
                    }
                    if yy < 4 && !is_full_solid_layer(layer) && !layer.is("minecraft:lava") {
                        return false;
                    }
                }
            }
        }

        true
    }
}

#[derive(Debug, Clone)]
struct PlacedGeodeFeature {
    step_index: i32,
    feature_index: i32,
    rarity: i32,
    height: OreHeight,
    config: GeodeFeatureConfig,
    biome_filter: FeatureBiomeFilter,
}

impl PlacedGeodeFeature {
    fn amethyst(feature_index: i32) -> Self {
        Self {
            step_index: 2,
            feature_index,
            rarity: 24,
            height: OreHeight::Uniform(HeightAnchor::AboveBottom(6), HeightAnchor::Absolute(30)),
            config: GeodeFeatureConfig::amethyst(),
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
        if random.next_float() >= 1.0 / self.rarity as f32 {
            return;
        }
        let world_x = origin_x + random.next_int(16);
        let world_z = origin_z + random.next_int(16);
        let world_y = self.height.sample(settings, random);
        if self
            .biome_filter
            .allows_at(&settings.density, world_x, world_y, world_z)
        {
            self.config.place(
                settings, origin_x, origin_z, chunk, random, world_x, world_y, world_z,
            );
        }
    }
}

#[derive(Debug, Clone)]
struct GeodeFeatureConfig {
    filling_block: BlockLayer,
    inner_block: BlockLayer,
    alternate_inner_block: BlockLayer,
    middle_block: BlockLayer,
    outer_block: BlockLayer,
    inner_placements: Vec<BlockLayer>,
    outer_wall_distance: UniformInt,
    distribution_points: UniformInt,
    point_offset: UniformInt,
    min_gen_offset: i32,
    max_gen_offset: i32,
    noise_multiplier: f64,
    invalid_blocks_threshold: i32,
    filling: f64,
    inner_layer: f64,
    middle_layer: f64,
    outer_layer: f64,
    use_potential_placements_chance: f32,
    use_alternate_layer0_chance: f32,
    placements_require_layer0_alternate: bool,
    generate_crack_chance: f32,
    base_crack_size: f64,
    crack_point_offset: i32,
}

impl GeodeFeatureConfig {
    fn amethyst() -> Self {
        Self {
            filling_block: BlockLayer::new("minecraft:air"),
            inner_block: BlockLayer::new("minecraft:amethyst_block"),
            alternate_inner_block: BlockLayer::new("minecraft:budding_amethyst"),
            middle_block: BlockLayer::new("minecraft:calcite"),
            outer_block: BlockLayer::new("minecraft:smooth_basalt"),
            inner_placements: vec![
                amethyst_cluster_block("minecraft:small_amethyst_bud"),
                amethyst_cluster_block("minecraft:medium_amethyst_bud"),
                amethyst_cluster_block("minecraft:large_amethyst_bud"),
                amethyst_cluster_block("minecraft:amethyst_cluster"),
            ],
            outer_wall_distance: UniformInt { min: 4, max: 6 },
            distribution_points: UniformInt { min: 3, max: 4 },
            point_offset: UniformInt { min: 1, max: 2 },
            min_gen_offset: -16,
            max_gen_offset: 16,
            noise_multiplier: 0.05,
            invalid_blocks_threshold: 1,
            filling: 1.7,
            inner_layer: 2.2,
            middle_layer: 3.2,
            outer_layer: 4.2,
            use_potential_placements_chance: 0.35,
            use_alternate_layer0_chance: 0.083,
            placements_require_layer0_alternate: true,
            generate_crack_chance: 0.95,
            base_crack_size: 2.0,
            crack_point_offset: 2,
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
        let num_points = self.distribution_points.sample(random);
        let mut points = Vec::with_capacity(num_points as usize);
        let mut invalid_points = 0;
        for _ in 0..num_points {
            let point = (
                origin_x + self.outer_wall_distance.sample(random),
                origin_y + self.outer_wall_distance.sample(random),
                origin_z + self.outer_wall_distance.sample(random),
            );
            if let Some((local_x, local_z)) =
                local_coords(point.0, point.2, chunk_min_x, chunk_min_z)
                && chunk
                    .layer(local_x, point.1, local_z, settings.min_y)
                    .is_some_and(is_geode_invalid_block)
            {
                invalid_points += 1;
                if invalid_points > self.invalid_blocks_threshold {
                    return false;
                }
            }
            points.push((point, self.point_offset.sample(random)));
        }

        let crack_size_adjustment = num_points as f64 / self.outer_wall_distance.max as f64;
        let inner_air = 1.0 / self.filling.sqrt();
        let innermost_block_layer = 1.0 / (self.inner_layer + crack_size_adjustment).sqrt();
        let inner_crust = 1.0 / (self.middle_layer + crack_size_adjustment).sqrt();
        let outer_crust = 1.0 / (self.outer_layer + crack_size_adjustment).sqrt();
        let crack_size = 1.0
            / (self.base_crack_size
                + random.next_double() / 2.0
                + if num_points > 3 {
                    crack_size_adjustment
                } else {
                    0.0
                })
            .sqrt();
        let should_generate_crack = random.next_float() < self.generate_crack_chance;
        let crack_points = if should_generate_crack {
            self.crack_points(random, origin_x, origin_y, origin_z, num_points)
        } else {
            Vec::new()
        };
        let mut potential_crystal_placements = Vec::new();

        for world_x in origin_x + self.min_gen_offset..=origin_x + self.max_gen_offset {
            let Some(local_x) = local_coord(world_x, chunk_min_x) else {
                continue;
            };
            for world_z in origin_z + self.min_gen_offset..=origin_z + self.max_gen_offset {
                let Some(local_z) = local_coord(world_z, chunk_min_z) else {
                    continue;
                };
                for world_y in origin_y + self.min_gen_offset..=origin_y + self.max_gen_offset {
                    if !(settings.min_y..settings.min_y + settings.height).contains(&world_y) {
                        continue;
                    }
                    let noise_offset =
                        geode_noise(world_x, world_y, world_z) * self.noise_multiplier;
                    let mut dist_sum_shell = 0.0;
                    let mut dist_sum_crack = 0.0;
                    for (point, offset) in &points {
                        dist_sum_shell += inv_sqrt_distance(
                            world_x, world_y, world_z, point.0, point.1, point.2, *offset,
                        ) + noise_offset;
                    }
                    for point in &crack_points {
                        dist_sum_crack += inv_sqrt_distance(
                            world_x,
                            world_y,
                            world_z,
                            point.0,
                            point.1,
                            point.2,
                            self.crack_point_offset,
                        ) + noise_offset;
                    }

                    if dist_sum_shell < outer_crust {
                        continue;
                    }
                    let replacement = if should_generate_crack
                        && dist_sum_crack >= crack_size
                        && dist_sum_shell < inner_air
                    {
                        Some(self.filling_block.clone())
                    } else if dist_sum_shell >= inner_air {
                        Some(self.filling_block.clone())
                    } else if dist_sum_shell >= innermost_block_layer {
                        let use_alternate = random.next_float() < self.use_alternate_layer0_chance;
                        let block = if use_alternate {
                            self.alternate_inner_block.clone()
                        } else {
                            self.inner_block.clone()
                        };
                        if (!self.placements_require_layer0_alternate || use_alternate)
                            && random.next_float() < self.use_potential_placements_chance
                        {
                            potential_crystal_placements.push((world_x, world_y, world_z));
                        }
                        Some(block)
                    } else if dist_sum_shell >= inner_crust {
                        Some(self.middle_block.clone())
                    } else if dist_sum_shell >= outer_crust {
                        Some(self.outer_block.clone())
                    } else {
                        None
                    };

                    if let Some(block) = replacement
                        && chunk
                            .layer(local_x, world_y, local_z, settings.min_y)
                            .is_some_and(can_geode_replace_block)
                    {
                        chunk.set_layer(local_x, world_y, local_z, settings.min_y, block);
                    }
                }
            }
        }

        for (world_x, world_y, world_z) in potential_crystal_placements {
            let block = self.inner_placements
                [random.next_int(self.inner_placements.len() as i32) as usize]
                .clone();
            for (dx, dy, dz, facing) in [
                (0, -1, 0, "down"),
                (0, 1, 0, "up"),
                (0, 0, -1, "north"),
                (0, 0, 1, "south"),
                (-1, 0, 0, "west"),
                (1, 0, 0, "east"),
            ] {
                let place_x = world_x + dx;
                let place_y = world_y + dy;
                let place_z = world_z + dz;
                let Some((local_x, local_z)) =
                    local_coords(place_x, place_z, chunk_min_x, chunk_min_z)
                else {
                    continue;
                };
                let Some(place_state) = chunk.layer(local_x, place_y, local_z, settings.min_y)
                else {
                    continue;
                };
                if can_amethyst_cluster_grow_at(place_state) {
                    let waterlogged = if place_state.is("minecraft:water") {
                        "true"
                    } else {
                        "false"
                    };
                    chunk.set_layer(
                        local_x,
                        place_y,
                        local_z,
                        settings.min_y,
                        block
                            .with_property("facing", facing)
                            .with_property("waterlogged", waterlogged),
                    );
                    break;
                }
            }
        }

        true
    }

    fn crack_points(
        &self,
        random: &mut FeatureRandom,
        origin_x: i32,
        origin_y: i32,
        origin_z: i32,
        num_points: i32,
    ) -> Vec<(i32, i32, i32)> {
        let crack_offset = num_points * 2 + 1;
        match random.next_int(4) {
            0 => vec![
                (origin_x + crack_offset, origin_y + 7, origin_z),
                (origin_x + crack_offset, origin_y + 5, origin_z),
                (origin_x + crack_offset, origin_y + 1, origin_z),
            ],
            1 => vec![
                (origin_x, origin_y + 7, origin_z + crack_offset),
                (origin_x, origin_y + 5, origin_z + crack_offset),
                (origin_x, origin_y + 1, origin_z + crack_offset),
            ],
            2 => vec![
                (
                    origin_x + crack_offset,
                    origin_y + 7,
                    origin_z + crack_offset,
                ),
                (
                    origin_x + crack_offset,
                    origin_y + 5,
                    origin_z + crack_offset,
                ),
                (
                    origin_x + crack_offset,
                    origin_y + 1,
                    origin_z + crack_offset,
                ),
            ],
            _ => vec![
                (origin_x, origin_y + 7, origin_z),
                (origin_x, origin_y + 5, origin_z),
                (origin_x, origin_y + 1, origin_z),
            ],
        }
    }
}

#[derive(Debug, Clone)]
struct PlacedFreezeTopLayerFeature {
    step_index: i32,
    feature_index: i32,
    snow_layer: BlockLayer,
}

impl PlacedFreezeTopLayerFeature {
    fn new(feature_index: i32) -> Self {
        Self {
            step_index: 10,
            feature_index,
            snow_layer: BlockLayer::new("minecraft:snow"),
        }
    }

    fn place(
        &self,
        settings: &NoiseSettings,
        origin_x: i32,
        origin_z: i32,
        chunk: &mut NoiseChunkBlocks,
    ) {
        for local_z in 0..16 {
            for local_x in 0..16 {
                let world_x = origin_x + local_x as i32;
                let world_z = origin_z + local_z as i32;
                let top_y = chunk.world_surface_wg_height(local_x, local_z, settings.min_y);
                if top_y <= settings.min_y || top_y >= settings.min_y + settings.height {
                    continue;
                }
                let biome = settings.density.biome(world_x, top_y, world_z);
                if !is_freezing_biome(biome) {
                    continue;
                }

                let below_y = top_y - 1;
                if chunk
                    .layer(local_x, below_y, local_z, settings.min_y)
                    .is_some_and(is_water_layer)
                {
                    chunk.set_layer(
                        local_x,
                        below_y,
                        local_z,
                        settings.min_y,
                        settings.ice_block.clone(),
                    );
                }

                if chunk
                    .layer(local_x, top_y, local_z, settings.min_y)
                    .is_some_and(|layer| layer.is_air || layer.is("minecraft:snow"))
                    && chunk
                        .layer(local_x, below_y, local_z, settings.min_y)
                        .is_some_and(is_full_solid_layer)
                {
                    chunk.set_layer(
                        local_x,
                        top_y,
                        local_z,
                        settings.min_y,
                        self.snow_layer.clone(),
                    );
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
struct PlacedMonsterRoomFeature {
    step_index: i32,
    feature_index: i32,
    count: OrePlacementCount,
    height: OreHeight,
    config: MonsterRoomFeatureConfig,
    biome_filter: FeatureBiomeFilter,
}

impl PlacedMonsterRoomFeature {
    fn regular(feature_index: i32) -> Self {
        Self {
            step_index: 3,
            feature_index,
            count: OrePlacementCount::Constant(10),
            height: OreHeight::Uniform(HeightAnchor::Absolute(0), HeightAnchor::BelowTop(0)),
            config: MonsterRoomFeatureConfig::new(),
            biome_filter: FeatureBiomeFilter::All,
        }
    }

    fn deep(feature_index: i32) -> Self {
        Self {
            step_index: 3,
            feature_index,
            count: OrePlacementCount::Constant(4),
            height: OreHeight::Uniform(HeightAnchor::AboveBottom(6), HeightAnchor::Absolute(-1)),
            config: MonsterRoomFeatureConfig::new(),
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
                .allows_at(&settings.density, world_x, world_y, world_z)
            {
                continue;
            }
            self.config.place(
                settings, origin_x, origin_z, chunk, random, world_x, world_y, world_z,
            );
        }
    }
}

#[derive(Debug, Clone)]
struct PlacedMultifaceGrowthFeature {
    step_index: i32,
    feature_index: i32,
    count: OrePlacementCount,
    height: OreHeight,
    max_below_ocean_floor: i32,
    config: MultifaceGrowthFeatureConfig,
    biome_filter: FeatureBiomeFilter,
}

impl PlacedMultifaceGrowthFeature {
    fn glow_lichen(feature_index: i32) -> Self {
        Self {
            step_index: 9,
            feature_index,
            count: OrePlacementCount::Uniform { min: 104, max: 157 },
            height: OreHeight::Uniform(HeightAnchor::AboveBottom(0), HeightAnchor::Absolute(256)),
            max_below_ocean_floor: -13,
            config: MultifaceGrowthFeatureConfig::glow_lichen(),
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
            let local_x = (world_x - origin_x) as usize;
            let local_z = (world_z - origin_z) as usize;
            let ocean_floor = chunk.ocean_floor_wg_height(local_x, local_z, settings.min_y);
            if world_y > ocean_floor + self.max_below_ocean_floor
                || !self
                    .biome_filter
                    .allows_at(&settings.density, world_x, world_y, world_z)
            {
                continue;
            }
            self.config.place(
                settings, origin_x, origin_z, chunk, random, world_x, world_y, world_z,
            );
        }
    }
}

#[derive(Debug, Clone)]
struct PlacedSimpleVegetationFeature {
    step_index: i32,
    feature_index: i32,
    outer_count: i32,
    noise_threshold: Option<NoiseThresholdCount>,
    rarity: i32,
    inner_count: i32,
    xz_offset: TrapezoidInt,
    y_offset: TrapezoidInt,
    block: SimpleVegetationBlock,
    required_support: Option<&'static str>,
    biome_filter: FeatureBiomeFilter,
}

impl PlacedSimpleVegetationFeature {
    fn patch_tall_grass_2(feature_index: i32) -> Self {
        Self {
            step_index: 9,
            feature_index,
            outer_count: 1,
            noise_threshold: Some(NoiseThresholdCount {
                noise_level: -0.8,
                below_noise: 0,
                above_noise: 7,
            }),
            rarity: 32,
            inner_count: 96,
            xz_offset: TrapezoidInt::new(-7, 7, 0),
            y_offset: TrapezoidInt::new(-3, 3, 0),
            block: SimpleVegetationBlock::tall_grass(),
            required_support: None,
            biome_filter: FeatureBiomeFilter::All,
        }
    }

    fn patch_bush(feature_index: i32) -> Self {
        Self {
            step_index: 9,
            feature_index,
            outer_count: 1,
            noise_threshold: None,
            rarity: 4,
            inner_count: 24,
            xz_offset: TrapezoidInt::new(-5, 5, 0),
            y_offset: TrapezoidInt::new(-3, 3, 0),
            block: SimpleVegetationBlock::single("minecraft:bush"),
            required_support: None,
            biome_filter: FeatureBiomeFilter::All,
        }
    }

    fn patch_sunflower(feature_index: i32) -> Self {
        Self::simple_patch(
            feature_index,
            1,
            3,
            96,
            SimpleVegetationBlock::sunflower(),
            None,
        )
    }

    fn flower_plains(feature_index: i32) -> Self {
        Self {
            step_index: 9,
            feature_index,
            outer_count: 1,
            noise_threshold: Some(NoiseThresholdCount {
                noise_level: -0.8,
                below_noise: 15,
                above_noise: 4,
            }),
            rarity: 32,
            inner_count: 64,
            xz_offset: TrapezoidInt::new(-6, 6, 0),
            y_offset: TrapezoidInt::new(-2, 2, 0),
            block: SimpleVegetationBlock::plains_flower(),
            required_support: None,
            biome_filter: FeatureBiomeFilter::All,
        }
    }

    fn patch_grass_plain(feature_index: i32) -> Self {
        Self {
            step_index: 9,
            feature_index,
            outer_count: 1,
            noise_threshold: Some(NoiseThresholdCount {
                noise_level: -0.8,
                below_noise: 5,
                above_noise: 10,
            }),
            rarity: 1,
            inner_count: 32,
            xz_offset: TrapezoidInt::new(-7, 7, 0),
            y_offset: TrapezoidInt::new(-3, 3, 0),
            block: SimpleVegetationBlock::single("minecraft:short_grass"),
            required_support: None,
            biome_filter: FeatureBiomeFilter::All,
        }
    }

    fn patch_grass_normal(feature_index: i32) -> Self {
        Self::grass_patch(feature_index, 5, 32, SimpleVegetationBlock::short_grass())
    }

    fn patch_grass_forest(feature_index: i32) -> Self {
        Self::grass_patch(feature_index, 2, 32, SimpleVegetationBlock::short_grass())
    }

    fn patch_grass_badlands(feature_index: i32) -> Self {
        Self::grass_patch(feature_index, 1, 32, SimpleVegetationBlock::short_grass())
    }

    fn patch_grass_savanna(feature_index: i32) -> Self {
        Self::grass_patch(feature_index, 20, 32, SimpleVegetationBlock::short_grass())
    }

    fn patch_grass_taiga(feature_index: i32) -> Self {
        Self::grass_patch(feature_index, 7, 32, SimpleVegetationBlock::taiga_grass())
    }

    fn patch_grass_taiga_2(feature_index: i32) -> Self {
        Self::grass_patch(feature_index, 1, 32, SimpleVegetationBlock::taiga_grass())
    }

    fn patch_grass_jungle(feature_index: i32) -> Self {
        Self::grass_patch(feature_index, 25, 32, SimpleVegetationBlock::jungle_grass())
    }

    fn patch_grass_meadow(feature_index: i32) -> Self {
        Self {
            step_index: 9,
            feature_index,
            outer_count: 1,
            noise_threshold: Some(NoiseThresholdCount {
                noise_level: -0.8,
                below_noise: 5,
                above_noise: 10,
            }),
            rarity: 1,
            inner_count: 16,
            xz_offset: TrapezoidInt::new(-7, 7, 0),
            y_offset: TrapezoidInt::new(-3, 3, 0),
            block: SimpleVegetationBlock::short_grass(),
            required_support: None,
            biome_filter: FeatureBiomeFilter::All,
        }
    }

    fn patch_large_fern(feature_index: i32) -> Self {
        Self::simple_patch(
            feature_index,
            1,
            5,
            96,
            SimpleVegetationBlock::large_fern(),
            None,
        )
    }

    fn patch_dry_grass(feature_index: i32, rarity: i32) -> Self {
        Self::simple_patch(
            feature_index,
            1,
            rarity,
            64,
            SimpleVegetationBlock::dry_grass(),
            None,
        )
    }

    fn brown_mushroom_normal(feature_index: i32) -> Self {
        Self::simple_patch(
            feature_index,
            1,
            256,
            96,
            SimpleVegetationBlock::single("minecraft:brown_mushroom"),
            None,
        )
    }

    fn red_mushroom_normal(feature_index: i32) -> Self {
        Self::simple_patch(
            feature_index,
            1,
            512,
            96,
            SimpleVegetationBlock::single("minecraft:red_mushroom"),
            None,
        )
    }

    fn brown_mushroom_taiga(feature_index: i32) -> Self {
        Self::simple_patch(
            feature_index,
            1,
            4,
            96,
            SimpleVegetationBlock::single("minecraft:brown_mushroom"),
            None,
        )
    }

    fn red_mushroom_taiga(feature_index: i32) -> Self {
        Self::simple_patch(
            feature_index,
            1,
            256,
            96,
            SimpleVegetationBlock::single("minecraft:red_mushroom"),
            None,
        )
    }

    fn brown_mushroom_old_growth(feature_index: i32) -> Self {
        Self::simple_patch(
            feature_index,
            3,
            4,
            96,
            SimpleVegetationBlock::single("minecraft:brown_mushroom"),
            None,
        )
    }

    fn red_mushroom_old_growth(feature_index: i32) -> Self {
        Self::simple_patch(
            feature_index,
            1,
            171,
            96,
            SimpleVegetationBlock::single("minecraft:red_mushroom"),
            None,
        )
    }

    fn brown_mushroom_swamp(feature_index: i32) -> Self {
        Self::simple_patch(
            feature_index,
            2,
            1,
            96,
            SimpleVegetationBlock::single("minecraft:brown_mushroom"),
            None,
        )
    }

    fn red_mushroom_swamp(feature_index: i32) -> Self {
        Self::simple_patch(
            feature_index,
            1,
            64,
            96,
            SimpleVegetationBlock::single("minecraft:red_mushroom"),
            None,
        )
    }

    fn patch_pumpkin(feature_index: i32) -> Self {
        Self::simple_patch(
            feature_index,
            1,
            300,
            96,
            SimpleVegetationBlock::single("minecraft:pumpkin"),
            Some("minecraft:grass_block"),
        )
    }

    fn patch_dead_bush(feature_index: i32, outer_count: i32) -> Self {
        Self::simple_patch(
            feature_index,
            outer_count,
            1,
            4,
            SimpleVegetationBlock::dead_bush(),
            None,
        )
    }

    fn patch_melon(feature_index: i32, rarity: i32) -> Self {
        Self::simple_patch(
            feature_index,
            1,
            rarity,
            64,
            SimpleVegetationBlock::single("minecraft:melon"),
            Some("minecraft:grass_block"),
        )
    }

    fn grass_patch(
        feature_index: i32,
        outer_count: i32,
        inner_count: i32,
        block: SimpleVegetationBlock,
    ) -> Self {
        Self::simple_patch(feature_index, outer_count, 1, inner_count, block, None)
    }

    fn simple_patch(
        feature_index: i32,
        outer_count: i32,
        rarity: i32,
        inner_count: i32,
        block: SimpleVegetationBlock,
        required_support: Option<&'static str>,
    ) -> Self {
        Self {
            step_index: 9,
            feature_index,
            outer_count,
            noise_threshold: None,
            rarity,
            inner_count,
            xz_offset: TrapezoidInt::new(-7, 7, 0),
            y_offset: TrapezoidInt::new(-3, 3, 0),
            block,
            required_support,
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
        let outer_count = self
            .noise_threshold
            .as_ref()
            .map(|threshold| threshold.sample(origin_x, origin_z))
            .unwrap_or(self.outer_count);
        for _ in 0..outer_count {
            if random.next_float() >= 1.0 / self.rarity as f32 {
                continue;
            }

            let base_x = origin_x + random.next_int(16);
            let base_z = origin_z + random.next_int(16);
            let Some((base_local_x, base_local_z)) =
                local_coords(base_x, base_z, origin_x, origin_z)
            else {
                continue;
            };
            let base_y = chunk.world_surface_wg_height(base_local_x, base_local_z, settings.min_y);
            if base_y <= settings.min_y
                || !self
                    .biome_filter
                    .allows_at(&settings.density, base_x, base_y, base_z)
            {
                continue;
            }

            for _ in 0..self.inner_count {
                let world_x = base_x + self.xz_offset.sample(random);
                let world_y = base_y + self.y_offset.sample(random);
                let world_z = base_z + self.xz_offset.sample(random);
                if !matches!(
                    layer_at_world(
                        chunk,
                        origin_x,
                        origin_z,
                        world_x,
                        world_y,
                        world_z,
                        settings.min_y,
                    ),
                    Some(layer) if layer.is_air
                ) {
                    continue;
                }
                if !self.has_required_support(
                    chunk,
                    origin_x,
                    origin_z,
                    world_x,
                    world_y - 1,
                    world_z,
                    settings.min_y,
                ) {
                    continue;
                }
                self.block.place_at(
                    settings, origin_x, origin_z, chunk, random, world_x, world_y, world_z,
                );
            }
        }
    }

    fn has_required_support(
        &self,
        chunk: &NoiseChunkBlocks,
        chunk_min_x: i32,
        chunk_min_z: i32,
        world_x: i32,
        world_y: i32,
        world_z: i32,
        min_y: i32,
    ) -> bool {
        let Some(required_support) = self.required_support else {
            return true;
        };
        layer_at_world(
            chunk,
            chunk_min_x,
            chunk_min_z,
            world_x,
            world_y,
            world_z,
            min_y,
        )
        .is_some_and(|layer| layer.is(required_support))
    }
}

#[derive(Debug, Clone)]
struct NoiseThresholdCount {
    noise_level: f64,
    below_noise: i32,
    above_noise: i32,
}

impl NoiseThresholdCount {
    fn sample(&self, origin_x: i32, origin_z: i32) -> i32 {
        if biome_info_noise(origin_x as f64 / 200.0, origin_z as f64 / 200.0) < self.noise_level {
            self.below_noise
        } else {
            self.above_noise
        }
    }
}

fn biome_info_noise(x: f64, z: f64) -> f64 {
    SimplexNoise2d::biome_info().value(x, z)
}

#[derive(Clone, Debug)]
struct SimplexNoise2d {
    p: [u8; 512],
}

impl SimplexNoise2d {
    fn biome_info() -> Self {
        Self::new(JavaRandom::new(2345))
    }

    fn new(mut random: JavaRandom) -> Self {
        random.next_double();
        random.next_double();
        random.next_double();

        let mut p = [0_u8; 512];
        for (index, value) in p.iter_mut().take(256).enumerate() {
            *value = index as u8;
        }
        for index in 0..256 {
            let offset = random.next_int(256 - index as i32) as usize;
            p.swap(index, index + offset);
        }
        for index in 0..256 {
            p[index + 256] = p[index];
        }
        Self { p }
    }

    fn value(&self, x: f64, z: f64) -> f64 {
        const SQRT_3: f64 = 1.732_050_807_568_877_2;
        const F2: f64 = 0.5 * (SQRT_3 - 1.0);
        const G2: f64 = (3.0 - SQRT_3) / 6.0;

        let skew = (x + z) * F2;
        let cell_x = (x + skew).floor() as i32;
        let cell_z = (z + skew).floor() as i32;
        let unskew = (cell_x + cell_z) as f64 * G2;
        let origin_x = cell_x as f64 - unskew;
        let origin_z = cell_z as f64 - unskew;
        let x0 = x - origin_x;
        let z0 = z - origin_z;
        let (x_step, z_step) = if x0 > z0 { (1, 0) } else { (0, 1) };
        let x1 = x0 - x_step as f64 + G2;
        let z1 = z0 - z_step as f64 + G2;
        let x2 = x0 - 1.0 + 2.0 * G2;
        let z2 = z0 - 1.0 + 2.0 * G2;
        let ii = (cell_x & 255) as usize;
        let jj = (cell_z & 255) as usize;
        let gi0 = self.p[ii + self.p[jj] as usize] as usize % 12;
        let gi1 = self.p[ii + x_step + self.p[jj + z_step] as usize] as usize % 12;
        let gi2 = self.p[ii + 1 + self.p[jj + 1] as usize] as usize % 12;

        70.0 * (simplex_corner(gi0, x0, z0)
            + simplex_corner(gi1, x1, z1)
            + simplex_corner(gi2, x2, z2))
    }
}

fn simplex_corner(gradient_index: usize, x: f64, z: f64) -> f64 {
    const GRADIENTS: [[f64; 2]; 12] = [
        [1.0, 1.0],
        [-1.0, 1.0],
        [1.0, -1.0],
        [-1.0, -1.0],
        [1.0, 0.0],
        [-1.0, 0.0],
        [1.0, 0.0],
        [-1.0, 0.0],
        [0.0, 1.0],
        [0.0, -1.0],
        [0.0, 1.0],
        [0.0, -1.0],
    ];
    let mut weight = 0.5 - x * x - z * z;
    if weight < 0.0 {
        return 0.0;
    }
    weight *= weight;
    weight * weight * (GRADIENTS[gradient_index][0] * x + GRADIENTS[gradient_index][1] * z)
}

#[derive(Debug, Clone)]
struct SimpleVegetationBlock {
    lower: BlockLayer,
    upper: Option<BlockLayer>,
    provider: SimpleVegetationProvider,
    support: SimpleVegetationSupport,
}

impl SimpleVegetationBlock {
    fn single(block: &str) -> Self {
        Self {
            lower: BlockLayer::new(block),
            upper: None,
            provider: SimpleVegetationProvider::Fixed,
            support: SimpleVegetationSupport::Vegetation,
        }
    }

    fn short_grass() -> Self {
        Self::single("minecraft:short_grass")
    }

    fn weighted_single(blocks: Vec<(BlockLayer, i32)>, support: SimpleVegetationSupport) -> Self {
        let lower = blocks
            .first()
            .map(|(block, _)| block.clone())
            .unwrap_or_else(|| BlockLayer::new("minecraft:air"));
        Self {
            lower,
            upper: None,
            provider: SimpleVegetationProvider::Weighted { entries: blocks },
            support,
        }
    }

    fn taiga_grass() -> Self {
        Self::weighted_single(
            vec![
                (BlockLayer::new("minecraft:short_grass"), 1),
                (BlockLayer::new("minecraft:fern"), 4),
            ],
            SimpleVegetationSupport::Vegetation,
        )
    }

    fn jungle_grass() -> Self {
        Self::weighted_single(
            vec![
                (BlockLayer::new("minecraft:short_grass"), 3),
                (BlockLayer::new("minecraft:fern"), 1),
            ],
            SimpleVegetationSupport::Vegetation,
        )
    }

    fn dry_grass() -> Self {
        Self::weighted_single(
            vec![
                (BlockLayer::new("minecraft:short_dry_grass"), 1),
                (BlockLayer::new("minecraft:tall_dry_grass"), 1),
            ],
            SimpleVegetationSupport::DryVegetation,
        )
    }

    fn tall_grass() -> Self {
        Self {
            lower: BlockLayer::with_properties("minecraft:tall_grass", &[("half", "lower")]),
            upper: Some(BlockLayer::with_properties(
                "minecraft:tall_grass",
                &[("half", "upper")],
            )),
            provider: SimpleVegetationProvider::Fixed,
            support: SimpleVegetationSupport::Vegetation,
        }
    }

    fn large_fern() -> Self {
        Self {
            lower: BlockLayer::with_properties("minecraft:large_fern", &[("half", "lower")]),
            upper: Some(BlockLayer::with_properties(
                "minecraft:large_fern",
                &[("half", "upper")],
            )),
            provider: SimpleVegetationProvider::Fixed,
            support: SimpleVegetationSupport::Vegetation,
        }
    }

    fn sunflower() -> Self {
        Self {
            lower: BlockLayer::with_properties("minecraft:sunflower", &[("half", "lower")]),
            upper: Some(BlockLayer::with_properties(
                "minecraft:sunflower",
                &[("half", "upper")],
            )),
            provider: SimpleVegetationProvider::Fixed,
            support: SimpleVegetationSupport::Vegetation,
        }
    }

    fn dead_bush() -> Self {
        Self {
            lower: BlockLayer::new("minecraft:dead_bush"),
            upper: None,
            provider: SimpleVegetationProvider::Fixed,
            support: SimpleVegetationSupport::DeadBush,
        }
    }

    fn plains_flower() -> Self {
        Self {
            lower: BlockLayer::new("minecraft:dandelion"),
            upper: None,
            provider: SimpleVegetationProvider::plains_flower(),
            support: SimpleVegetationSupport::Vegetation,
        }
    }

    fn place_at(
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
        let lower = self
            .provider
            .block_at(&self.lower, random, world_x, world_z);
        self.place_selected(
            settings,
            chunk_min_x,
            chunk_min_z,
            chunk,
            lower,
            world_x,
            world_y,
            world_z,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn place_selected(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        lower: BlockLayer,
        world_x: i32,
        world_y: i32,
        world_z: i32,
    ) -> bool {
        let Some((local_x, local_z)) = local_coords(world_x, world_z, chunk_min_x, chunk_min_z)
        else {
            return false;
        };
        if !chunk
            .layer(local_x, world_y, local_z, settings.min_y)
            .is_some_and(|layer| layer.is_air)
            || !self.support.allows_at_world(
                chunk,
                chunk_min_x,
                chunk_min_z,
                world_x,
                world_y - 1,
                world_z,
                settings.min_y,
            )
        {
            return false;
        }

        if let Some(upper) = &self.upper {
            if !matches!(
                layer_at_world(
                    chunk,
                    chunk_min_x,
                    chunk_min_z,
                    world_x,
                    world_y + 1,
                    world_z,
                    settings.min_y,
                ),
                Some(layer) if layer.is_air
            ) {
                return false;
            }
            chunk.set_layer(local_x, world_y, local_z, settings.min_y, lower);
            chunk.set_layer(local_x, world_y + 1, local_z, settings.min_y, upper.clone());
        } else {
            chunk.set_layer(local_x, world_y, local_z, settings.min_y, lower);
        }
        true
    }
}

#[derive(Debug, Clone, Copy)]
enum SimpleVegetationSupport {
    Vegetation,
    DeadBush,
    DryVegetation,
}

impl SimpleVegetationSupport {
    fn allows_at_world(
        self,
        chunk: &NoiseChunkBlocks,
        chunk_min_x: i32,
        chunk_min_z: i32,
        world_x: i32,
        world_y: i32,
        world_z: i32,
        min_y: i32,
    ) -> bool {
        let Some(layer) = layer_at_world(
            chunk,
            chunk_min_x,
            chunk_min_z,
            world_x,
            world_y,
            world_z,
            min_y,
        ) else {
            return false;
        };
        match self {
            Self::Vegetation => supports_vegetation_layer(layer),
            Self::DeadBush => supports_dead_bush_layer(layer),
            Self::DryVegetation => supports_dry_vegetation_layer(layer),
        }
    }
}

#[derive(Debug, Clone)]
enum SimpleVegetationProvider {
    Fixed,
    Weighted {
        entries: Vec<(BlockLayer, i32)>,
    },
    PlainsFlower {
        low_states: Vec<BlockLayer>,
        high_states: Vec<BlockLayer>,
        high_chance: f32,
        threshold: f64,
        scale: f64,
    },
}

impl SimpleVegetationProvider {
    fn plains_flower() -> Self {
        Self::PlainsFlower {
            low_states: PLAINS_FLOWER_LOW_BLOCKS
                .iter()
                .map(|block| BlockLayer::new(block))
                .collect(),
            high_states: PLAINS_FLOWER_HIGH_BLOCKS
                .iter()
                .map(|block| BlockLayer::new(block))
                .collect(),
            high_chance: 0.333_333_34,
            threshold: -0.8,
            scale: 0.005,
        }
    }

    fn block_at(
        &self,
        default_state: &BlockLayer,
        random: &mut FeatureRandom,
        world_x: i32,
        world_z: i32,
    ) -> BlockLayer {
        match self {
            Self::Fixed => default_state.clone(),
            Self::Weighted { entries } => {
                let total_weight = entries.iter().map(|(_, weight)| *weight).sum();
                let mut selection = random.next_int(total_weight);
                for (block, weight) in entries {
                    selection -= *weight;
                    if selection < 0 {
                        return block.clone();
                    }
                }
                default_state.clone()
            }
            Self::PlainsFlower {
                low_states,
                high_states,
                high_chance,
                threshold,
                scale,
            } => {
                let noise = biome_info_noise(world_x as f64 * *scale, world_z as f64 * *scale);
                if noise < *threshold {
                    low_states[random.next_int(low_states.len() as i32) as usize].clone()
                } else if random.next_float() < *high_chance {
                    high_states[random.next_int(high_states.len() as i32) as usize].clone()
                } else {
                    default_state.clone()
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
struct PlacedBlockColumnFeature {
    step_index: i32,
    feature_index: i32,
    rarity: i32,
    inner_count: i32,
    xz_offset: TrapezoidInt,
    y_offset: TrapezoidInt,
    column: BlockColumnFeatureConfig,
    biome_filter: FeatureBiomeFilter,
}

impl PlacedBlockColumnFeature {
    fn sugar_cane(feature_index: i32, rarity: i32) -> Self {
        Self {
            step_index: 9,
            feature_index,
            rarity,
            inner_count: 20,
            xz_offset: TrapezoidInt::new(-4, 4, 0),
            y_offset: TrapezoidInt::new(0, 0, 0),
            column: BlockColumnFeatureConfig::sugar_cane(),
            biome_filter: FeatureBiomeFilter::All,
        }
    }

    fn cactus(feature_index: i32, rarity: i32) -> Self {
        Self {
            step_index: 9,
            feature_index,
            rarity,
            inner_count: 10,
            xz_offset: TrapezoidInt::new(-7, 7, 0),
            y_offset: TrapezoidInt::new(-3, 3, 0),
            column: BlockColumnFeatureConfig::cactus(),
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
        if random.next_float() >= 1.0 / self.rarity as f32 {
            return;
        }

        let base_x = origin_x + random.next_int(16);
        let base_z = origin_z + random.next_int(16);
        let Some((base_local_x, base_local_z)) = local_coords(base_x, base_z, origin_x, origin_z)
        else {
            return;
        };
        let base_y = chunk.world_surface_wg_height(base_local_x, base_local_z, settings.min_y);
        if base_y <= settings.min_y
            || !self
                .biome_filter
                .allows_at(&settings.density, base_x, base_y, base_z)
        {
            return;
        }

        for _ in 0..self.inner_count {
            let world_x = base_x + self.xz_offset.sample(random);
            let world_y = base_y + self.y_offset.sample(random);
            let world_z = base_z + self.xz_offset.sample(random);
            self.column.place_at(
                settings, origin_x, origin_z, chunk, random, world_x, world_y, world_z,
            );
        }
    }
}

#[derive(Debug, Clone)]
struct BlockColumnFeatureConfig {
    block: BlockLayer,
    height: BiasedToBottomInt,
    tip: Option<BlockColumnTip>,
    support: BlockColumnSupport,
}

impl BlockColumnFeatureConfig {
    fn sugar_cane() -> Self {
        Self {
            block: BlockLayer::with_properties("minecraft:sugar_cane", &[("age", "0")]),
            height: BiasedToBottomInt { min: 2, max: 4 },
            tip: None,
            support: BlockColumnSupport::SugarCane,
        }
    }

    fn cactus() -> Self {
        Self {
            block: BlockLayer::with_properties("minecraft:cactus", &[("age", "0")]),
            height: BiasedToBottomInt { min: 1, max: 3 },
            tip: Some(BlockColumnTip {
                block: BlockLayer::new("minecraft:cactus_flower"),
                count: WeightedInt::new(&[(0, 3), (1, 1)]),
            }),
            support: BlockColumnSupport::Cactus,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn place_at(
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
        if !self.support.allows_at_world(
            chunk,
            chunk_min_x,
            chunk_min_z,
            world_x,
            world_y,
            world_z,
            settings.min_y,
        ) {
            return false;
        }
        let Some((local_x, local_z)) = local_coords(world_x, world_z, chunk_min_x, chunk_min_z)
        else {
            return false;
        };

        let mut height = self.height.sample(random);
        let tip_count = self.tip.as_ref().map_or(0, |tip| tip.count.sample(random));
        height += tip_count;
        if height <= 0 {
            return false;
        }

        for dy in 0..height {
            let y = world_y + dy;
            let Some(layer) = chunk.layer(local_x, y, local_z, settings.min_y) else {
                return false;
            };
            if !layer.is_air {
                return false;
            }
        }

        let main_height = height - tip_count;
        for dy in 0..main_height {
            chunk.set_layer(
                local_x,
                world_y + dy,
                local_z,
                settings.min_y,
                self.block.clone(),
            );
        }
        if let Some(tip) = &self.tip {
            for dy in main_height..height {
                chunk.set_layer(
                    local_x,
                    world_y + dy,
                    local_z,
                    settings.min_y,
                    tip.block.clone(),
                );
            }
        }
        true
    }
}

#[derive(Debug, Clone)]
struct BlockColumnTip {
    block: BlockLayer,
    count: WeightedInt,
}

#[derive(Debug, Clone, Copy)]
struct BiasedToBottomInt {
    min: i32,
    max: i32,
}

impl BiasedToBottomInt {
    fn sample(self, random: &mut FeatureRandom) -> i32 {
        let first = random.next_int(self.max - self.min + 1);
        let second = random.next_int(self.max - self.min + 1);
        self.min + first.min(second)
    }
}

#[derive(Debug, Clone, Copy)]
enum BlockColumnSupport {
    SugarCane,
    Cactus,
}

impl BlockColumnSupport {
    #[allow(clippy::too_many_arguments)]
    fn allows_at_world(
        self,
        chunk: &NoiseChunkBlocks,
        chunk_min_x: i32,
        chunk_min_z: i32,
        world_x: i32,
        world_y: i32,
        world_z: i32,
        min_y: i32,
    ) -> bool {
        if !is_air_at_world(
            chunk,
            chunk_min_x,
            chunk_min_z,
            world_x,
            world_y,
            world_z,
            min_y,
        ) {
            return false;
        }
        match self {
            Self::SugarCane => {
                let below_y = world_y - 1;
                let Some(below) = layer_at_world(
                    chunk,
                    chunk_min_x,
                    chunk_min_z,
                    world_x,
                    below_y,
                    world_z,
                    min_y,
                ) else {
                    return false;
                };
                if !supports_sugar_cane_layer(below) {
                    return false;
                }
                horizontal_directions().iter().any(|(dx, dz)| {
                    is_water_at_world(
                        chunk,
                        chunk_min_x,
                        chunk_min_z,
                        world_x + dx,
                        below_y,
                        world_z + dz,
                        min_y,
                    )
                })
            }
            Self::Cactus => {
                let Some(below) = layer_at_world(
                    chunk,
                    chunk_min_x,
                    chunk_min_z,
                    world_x,
                    world_y - 1,
                    world_z,
                    min_y,
                ) else {
                    return false;
                };
                if !supports_cactus_layer(below) {
                    return false;
                }
                horizontal_directions().iter().all(|(dx, dz)| {
                    layer_at_world(
                        chunk,
                        chunk_min_x,
                        chunk_min_z,
                        world_x + dx,
                        world_y,
                        world_z + dz,
                        min_y,
                    )
                    .is_none_or(|layer| layer.is_air)
                })
            }
        }
    }
}

#[derive(Debug, Clone)]
struct PlacedTreeFeature {
    step_index: i32,
    feature_index: i32,
    count: WeightedInt,
    surface_water_depth: i32,
    config: TreeFeatureConfig,
    biome_filter: FeatureBiomeFilter,
}

impl PlacedTreeFeature {
    fn trees_plains(feature_index: i32) -> Self {
        Self {
            step_index: 9,
            feature_index,
            count: WeightedInt::new(&[(0, 19), (1, 1)]),
            surface_water_depth: 0,
            config: TreeFeatureConfig::oak_bees_005(),
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
            let Some((local_x, local_z)) = local_coords(world_x, world_z, origin_x, origin_z)
            else {
                continue;
            };
            if chunk.world_surface_wg_height(local_x, local_z, settings.min_y)
                - chunk.ocean_floor_wg_height(local_x, local_z, settings.min_y)
                > self.surface_water_depth
            {
                continue;
            }
            let world_y = chunk.ocean_floor_wg_height(local_x, local_z, settings.min_y);
            if world_y <= settings.min_y
                || !oak_sapling_would_survive_at(
                    chunk,
                    origin_x,
                    origin_z,
                    world_x,
                    world_y,
                    world_z,
                    settings.min_y,
                )
                || !self
                    .biome_filter
                    .allows_at(&settings.density, world_x, world_y, world_z)
            {
                continue;
            }
            self.config.place(
                settings, origin_x, origin_z, chunk, random, world_x, world_y, world_z,
            );
        }
    }
}

#[derive(Debug, Clone)]
struct TreeFeatureConfig {
    default_tree: OakTreeConfig,
    fancy_chance: f32,
    fallen_chance: f32,
}

impl TreeFeatureConfig {
    fn oak_bees_005() -> Self {
        Self {
            default_tree: OakTreeConfig::oak_bees_005(),
            fancy_chance: 0.333_333_34,
            fallen_chance: 0.0125,
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
        world_x: i32,
        world_y: i32,
        world_z: i32,
    ) -> bool {
        if random.next_float() < self.fancy_chance {
            return self.default_tree.place(
                settings,
                chunk_min_x,
                chunk_min_z,
                chunk,
                random,
                world_x,
                world_y,
                world_z,
            );
        }
        if random.next_float() < self.fallen_chance {
            return self.default_tree.place_fallen(
                settings,
                chunk_min_x,
                chunk_min_z,
                chunk,
                random,
                world_x,
                world_y,
                world_z,
            );
        }
        self.default_tree.place(
            settings,
            chunk_min_x,
            chunk_min_z,
            chunk,
            random,
            world_x,
            world_y,
            world_z,
        )
    }
}

#[derive(Debug, Clone)]
struct OakTreeConfig {
    trunk: BlockLayer,
    leaves: BlockLayer,
    dirt: BlockLayer,
    bee_nest: BlockLayer,
    base_height: i32,
    height_rand_a: i32,
    foliage_height: i32,
    foliage_radius: i32,
    beehive_probability: f32,
}

impl OakTreeConfig {
    fn oak_bees_005() -> Self {
        Self {
            trunk: BlockLayer::with_properties("minecraft:oak_log", &[("axis", "y")]),
            leaves: BlockLayer::with_properties(
                "minecraft:oak_leaves",
                &[
                    ("distance", "7"),
                    ("persistent", "false"),
                    ("waterlogged", "false"),
                ],
            ),
            dirt: BlockLayer::new("minecraft:dirt"),
            bee_nest: BlockLayer::with_properties(
                "minecraft:bee_nest",
                &[("facing", "south"), ("honey_level", "0")],
            ),
            base_height: 4,
            height_rand_a: 2,
            foliage_height: 3,
            foliage_radius: 2,
            beehive_probability: 0.05,
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
        world_x: i32,
        world_y: i32,
        world_z: i32,
    ) -> bool {
        let tree_height = self.base_height + random.next_int(self.height_rand_a + 1);
        if !self.has_space(
            settings,
            chunk_min_x,
            chunk_min_z,
            chunk,
            world_x,
            world_y,
            world_z,
            tree_height,
        ) {
            return false;
        }

        if let Some((local_x, local_z)) = local_coords(world_x, world_z, chunk_min_x, chunk_min_z)
            && chunk
                .layer(local_x, world_y - 1, local_z, settings.min_y)
                .is_some_and(|layer| !cannot_replace_below_tree_trunk(layer))
        {
            chunk.set_layer(
                local_x,
                world_y - 1,
                local_z,
                settings.min_y,
                self.dirt.clone(),
            );
        }

        let mut logs = Vec::new();
        for dy in 0..tree_height {
            if let Some((local_x, local_z)) =
                local_coords(world_x, world_z, chunk_min_x, chunk_min_z)
            {
                chunk.set_layer(
                    local_x,
                    world_y + dy,
                    local_z,
                    settings.min_y,
                    self.trunk.clone(),
                );
                logs.push((world_x, world_y + dy, world_z));
            }
        }

        let leaf_origin_y = world_y + tree_height;
        let mut leaves = Vec::new();
        for y_offset in (-self.foliage_height..=0).rev() {
            let radius = (self.foliage_radius - 1 - y_offset / 2).max(0);
            for dx in -radius..=radius {
                for dz in -radius..=radius {
                    if dx.abs() == radius
                        && dz.abs() == radius
                        && (random.next_int(2) == 0 || y_offset == 0)
                    {
                        continue;
                    }
                    let leaf_x = world_x + dx;
                    let leaf_y = leaf_origin_y + y_offset;
                    let leaf_z = world_z + dz;
                    if self.try_place_leaf(
                        settings,
                        chunk_min_x,
                        chunk_min_z,
                        chunk,
                        leaf_x,
                        leaf_y,
                        leaf_z,
                    ) {
                        leaves.push((leaf_x, leaf_y, leaf_z));
                    }
                }
            }
        }

        self.try_place_beehive(
            settings,
            chunk_min_x,
            chunk_min_z,
            chunk,
            random,
            &logs,
            &leaves,
        );

        true
    }

    #[allow(clippy::too_many_arguments)]
    fn place_fallen(
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
        let direction = horizontal_directions()[random.next_int(4) as usize];
        let length = 2 + random.next_int(4);
        let start_x = world_x + direction.0 * (2 + random.next_int(2));
        let start_z = world_z + direction.1 * (2 + random.next_int(2));
        let axis = if direction.0 != 0 { "x" } else { "z" };
        let sideways_trunk = self.trunk.with_property("axis", axis);

        if let Some((local_x, local_z)) = local_coords(world_x, world_z, chunk_min_x, chunk_min_z) {
            chunk.set_layer(
                local_x,
                world_y,
                local_z,
                settings.min_y,
                self.trunk.clone(),
            );
        }

        let mut current_x = start_x;
        let mut current_z = start_z;
        for _ in 0..length {
            let Some((local_x, local_z)) =
                local_coords(current_x, current_z, chunk_min_x, chunk_min_z)
            else {
                current_x += direction.0;
                current_z += direction.1;
                continue;
            };
            let y = chunk.ocean_floor_wg_height(local_x, local_z, settings.min_y);
            if y > settings.min_y
                && chunk
                    .layer(local_x, y, local_z, settings.min_y)
                    .is_some_and(valid_tree_position_layer)
                && is_full_solid_at_world(
                    chunk,
                    chunk_min_x,
                    chunk_min_z,
                    current_x,
                    y - 1,
                    current_z,
                    settings.min_y,
                )
            {
                chunk.set_layer(local_x, y, local_z, settings.min_y, sideways_trunk.clone());
            }
            current_x += direction.0;
            current_z += direction.1;
        }
        true
    }

    #[allow(clippy::too_many_arguments)]
    fn has_space(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &NoiseChunkBlocks,
        world_x: i32,
        world_y: i32,
        world_z: i32,
        tree_height: i32,
    ) -> bool {
        if world_y < settings.min_y + 1
            || world_y + tree_height + 1 > settings.min_y + settings.height
        {
            return false;
        }

        for dy in 0..=tree_height + 1 {
            let radius = if dy < 1 { 0 } else { 1 };
            for dx in -radius..=radius {
                for dz in -radius..=radius {
                    let Some(layer) = layer_at_world(
                        chunk,
                        chunk_min_x,
                        chunk_min_z,
                        world_x + dx,
                        world_y + dy,
                        world_z + dz,
                        settings.min_y,
                    ) else {
                        continue;
                    };
                    if !valid_tree_position_layer(layer) && !is_log_layer(layer) {
                        return false;
                    }
                }
            }
        }
        true
    }

    #[allow(clippy::too_many_arguments)]
    fn try_place_leaf(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        world_x: i32,
        world_y: i32,
        world_z: i32,
    ) -> bool {
        let Some((local_x, local_z)) = local_coords(world_x, world_z, chunk_min_x, chunk_min_z)
        else {
            return false;
        };
        if !chunk
            .layer(local_x, world_y, local_z, settings.min_y)
            .is_some_and(valid_tree_position_layer)
        {
            return false;
        }
        chunk.set_layer(
            local_x,
            world_y,
            local_z,
            settings.min_y,
            self.leaves.clone(),
        );
        true
    }

    #[allow(clippy::too_many_arguments)]
    fn try_place_beehive(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
        logs: &[(i32, i32, i32)],
        leaves: &[(i32, i32, i32)],
    ) {
        if logs.is_empty() || random.next_float() >= self.beehive_probability {
            return;
        }
        let lowest_log_y = logs.iter().map(|log| log.1).min().unwrap_or(logs[0].1);
        let highest_log_y = logs.iter().map(|log| log.1).max().unwrap_or(logs[0].1);
        let hive_y = leaves
            .iter()
            .map(|leaf| leaf.1)
            .min()
            .map(|leaf_y| (leaf_y - 1).max(lowest_log_y + 1))
            .unwrap_or_else(|| (lowest_log_y + 1 + random.next_int(3)).min(highest_log_y));
        let mut candidates = Vec::new();
        for &(log_x, log_y, log_z) in logs {
            if log_y != hive_y {
                continue;
            }
            candidates.push((log_x, log_y, log_z + 1));
            candidates.push((log_x + 1, log_y, log_z));
            candidates.push((log_x - 1, log_y, log_z));
        }
        shuffle_positions(&mut candidates, random);
        for (hive_x, hive_y, hive_z) in candidates {
            if !is_air_at_world(
                chunk,
                chunk_min_x,
                chunk_min_z,
                hive_x,
                hive_y,
                hive_z,
                settings.min_y,
            ) || !is_air_at_world(
                chunk,
                chunk_min_x,
                chunk_min_z,
                hive_x,
                hive_y,
                hive_z + 1,
                settings.min_y,
            ) {
                continue;
            }
            let Some((local_x, local_z)) = local_coords(hive_x, hive_z, chunk_min_x, chunk_min_z)
            else {
                continue;
            };
            chunk.set_layer(
                local_x,
                hive_y,
                local_z,
                settings.min_y,
                self.bee_nest.clone(),
            );
            chunk.push_block_entity(
                hive_x,
                hive_y,
                hive_z,
                BEEHIVE_BLOCK_ENTITY_TYPE_ID,
                beehive_block_entity_nbt(random),
            );
            return;
        }
    }
}

#[derive(Debug, Clone)]
struct WeightedInt {
    entries: Vec<(i32, i32)>,
    total_weight: i32,
}

impl WeightedInt {
    fn new(entries: &[(i32, i32)]) -> Self {
        Self {
            entries: entries.to_vec(),
            total_weight: entries.iter().map(|(_, weight)| *weight).sum(),
        }
    }

    fn sample(&self, random: &mut FeatureRandom) -> i32 {
        let mut selection = random.next_int(self.total_weight);
        for (value, weight) in &self.entries {
            selection -= *weight;
            if selection < 0 {
                return *value;
            }
        }
        0
    }
}

#[derive(Debug, Clone)]
struct MultifaceGrowthFeatureConfig {
    block: BlockLayer,
    search_range: i32,
    can_place_on_floor: bool,
    can_place_on_ceiling: bool,
    can_place_on_wall: bool,
    chance_of_spreading: f32,
    can_be_placed_on: &'static [&'static str],
}

impl MultifaceGrowthFeatureConfig {
    fn glow_lichen() -> Self {
        Self {
            block: BlockLayer::with_properties(
                "minecraft:glow_lichen",
                &[
                    ("down", "false"),
                    ("east", "false"),
                    ("north", "false"),
                    ("south", "false"),
                    ("up", "false"),
                    ("waterlogged", "false"),
                    ("west", "false"),
                ],
            ),
            search_range: 20,
            can_place_on_floor: false,
            can_place_on_ceiling: true,
            can_place_on_wall: true,
            chance_of_spreading: 0.5,
            can_be_placed_on: GLOW_LICHEN_CAN_BE_PLACED_ON,
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
        let Some(origin_state) = layer_at_world(
            chunk,
            chunk_min_x,
            chunk_min_z,
            origin_x,
            origin_y,
            origin_z,
            settings.min_y,
        ) else {
            return false;
        };
        if !is_air_or_water_layer(origin_state) {
            return false;
        }

        let search_directions = self.shuffled_directions(random);
        if self.place_growth_if_possible(
            settings,
            chunk_min_x,
            chunk_min_z,
            chunk,
            random,
            origin_x,
            origin_y,
            origin_z,
            &search_directions,
        ) {
            return true;
        }

        for search_direction in &search_directions {
            let placement_directions =
                self.shuffled_directions_except(random, direction_opposite(*search_direction));
            for step in 1..=self.search_range {
                let world_x = origin_x + search_direction.0 * step;
                let world_y = origin_y + search_direction.1 * step;
                let world_z = origin_z + search_direction.2 * step;
                let Some(state) = layer_at_world(
                    chunk,
                    chunk_min_x,
                    chunk_min_z,
                    world_x,
                    world_y,
                    world_z,
                    settings.min_y,
                ) else {
                    break;
                };
                if !is_air_or_water_layer(state) && !state.is(self.block.block.as_ref()) {
                    break;
                }
                if self.place_growth_if_possible(
                    settings,
                    chunk_min_x,
                    chunk_min_z,
                    chunk,
                    random,
                    world_x,
                    world_y,
                    world_z,
                    &placement_directions,
                ) {
                    return true;
                }
            }
        }

        false
    }

    #[allow(clippy::too_many_arguments)]
    fn place_growth_if_possible(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
        world_x: i32,
        world_y: i32,
        world_z: i32,
        directions: &[(i32, i32, i32, &'static str)],
    ) -> bool {
        for direction in directions {
            let neighbor_x = world_x + direction.0;
            let neighbor_y = world_y + direction.1;
            let neighbor_z = world_z + direction.2;
            let Some(neighbor) = layer_at_world(
                chunk,
                chunk_min_x,
                chunk_min_z,
                neighbor_x,
                neighbor_y,
                neighbor_z,
                settings.min_y,
            ) else {
                continue;
            };
            if !self.can_be_placed_on.contains(&neighbor.block.as_ref()) {
                continue;
            }
            if !self.try_place_face(
                settings,
                chunk_min_x,
                chunk_min_z,
                chunk,
                world_x,
                world_y,
                world_z,
                direction.3,
            ) {
                continue;
            }
            if random.next_float() < self.chance_of_spreading {
                self.spread_once(
                    settings,
                    chunk_min_x,
                    chunk_min_z,
                    chunk,
                    random,
                    world_x,
                    world_y,
                    world_z,
                    *direction,
                );
            }
            return true;
        }
        false
    }

    #[allow(clippy::too_many_arguments)]
    fn try_place_face(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        world_x: i32,
        world_y: i32,
        world_z: i32,
        face: &str,
    ) -> bool {
        let Some((local_x, local_z)) = local_coords(world_x, world_z, chunk_min_x, chunk_min_z)
        else {
            return false;
        };
        let Some(current) = chunk.layer(local_x, world_y, local_z, settings.min_y) else {
            return false;
        };
        if !is_air_or_water_layer(current) && !current.is(self.block.block.as_ref()) {
            return false;
        }
        if current.is(self.block.block.as_ref())
            && current
                .properties
                .iter()
                .any(|(name, value)| name == face && value == "true")
        {
            return false;
        }

        let waterlogged = if current.is("minecraft:water") {
            "true"
        } else {
            "false"
        };
        let new_state = if current.is(self.block.block.as_ref()) {
            current.with_property(face, "true")
        } else {
            self.block
                .with_property("waterlogged", waterlogged)
                .with_property(face, "true")
        };
        chunk.set_layer(local_x, world_y, local_z, settings.min_y, new_state);
        true
    }

    #[allow(clippy::too_many_arguments)]
    fn spread_once(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
        world_x: i32,
        world_y: i32,
        world_z: i32,
        from_face: (i32, i32, i32, &'static str),
    ) -> bool {
        for spread_direction in shuffled_all_directions(random) {
            if spread_direction_axis(spread_direction) == spread_direction_axis(from_face) {
                continue;
            }
            for (target_x, target_y, target_z, target_face) in [
                (world_x, world_y, world_z, spread_direction.3),
                (
                    world_x + spread_direction.0,
                    world_y + spread_direction.1,
                    world_z + spread_direction.2,
                    from_face.3,
                ),
                (
                    world_x + spread_direction.0 + from_face.0,
                    world_y + spread_direction.1 + from_face.1,
                    world_z + spread_direction.2 + from_face.2,
                    direction_opposite_name(spread_direction.3),
                ),
            ] {
                let neighbor = direction_by_name(target_face);
                let Some(neighbor_state) = layer_at_world(
                    chunk,
                    chunk_min_x,
                    chunk_min_z,
                    target_x + neighbor.0,
                    target_y + neighbor.1,
                    target_z + neighbor.2,
                    settings.min_y,
                ) else {
                    continue;
                };
                if self
                    .can_be_placed_on
                    .contains(&neighbor_state.block.as_ref())
                    && self.try_place_face(
                        settings,
                        chunk_min_x,
                        chunk_min_z,
                        chunk,
                        target_x,
                        target_y,
                        target_z,
                        target_face,
                    )
                {
                    return true;
                }
            }
        }
        false
    }

    fn shuffled_directions(
        &self,
        random: &mut FeatureRandom,
    ) -> Vec<(i32, i32, i32, &'static str)> {
        let mut directions = Vec::with_capacity(5);
        if self.can_place_on_ceiling {
            directions.push((0, 1, 0, "up"));
        }
        if self.can_place_on_floor {
            directions.push((0, -1, 0, "down"));
        }
        if self.can_place_on_wall {
            directions.extend([
                (0, 0, -1, "north"),
                (1, 0, 0, "east"),
                (0, 0, 1, "south"),
                (-1, 0, 0, "west"),
            ]);
        }
        shuffle_directions(&mut directions, random);
        directions
    }

    fn shuffled_directions_except(
        &self,
        random: &mut FeatureRandom,
        excluded: (i32, i32, i32, &'static str),
    ) -> Vec<(i32, i32, i32, &'static str)> {
        let mut directions = self.shuffled_directions(random);
        directions.retain(|direction| *direction != excluded);
        directions
    }
}

#[derive(Debug, Clone)]
struct MonsterRoomFeatureConfig {
    cave_air: BlockLayer,
    cobblestone: BlockLayer,
    mossy_cobblestone: BlockLayer,
    chest: BlockLayer,
    spawner: BlockLayer,
}

impl MonsterRoomFeatureConfig {
    fn new() -> Self {
        Self {
            cave_air: BlockLayer::new("minecraft:cave_air"),
            cobblestone: BlockLayer::new("minecraft:cobblestone"),
            mossy_cobblestone: BlockLayer::new("minecraft:mossy_cobblestone"),
            chest: BlockLayer::new("minecraft:chest"),
            spawner: BlockLayer::new("minecraft:spawner"),
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
        let x_radius = random.next_int(2) + 2;
        let z_radius = random.next_int(2) + 2;
        let min_x = -x_radius - 1;
        let max_x = x_radius + 1;
        let min_z = -z_radius - 1;
        let max_z = z_radius + 1;

        if !self.can_place(
            settings,
            chunk_min_x,
            chunk_min_z,
            chunk,
            origin_x,
            origin_y,
            origin_z,
            min_x,
            max_x,
            min_z,
            max_z,
        ) {
            return false;
        }

        self.place_shell_and_room(
            settings,
            chunk_min_x,
            chunk_min_z,
            chunk,
            random,
            origin_x,
            origin_y,
            origin_z,
            min_x,
            max_x,
            min_z,
            max_z,
        );
        self.place_chests(
            settings,
            chunk_min_x,
            chunk_min_z,
            chunk,
            random,
            origin_x,
            origin_y,
            origin_z,
            x_radius,
            z_radius,
        );
        self.place_spawner(
            settings,
            chunk_min_x,
            chunk_min_z,
            chunk,
            random,
            origin_x,
            origin_y,
            origin_z,
        );
        true
    }

    #[allow(clippy::too_many_arguments)]
    fn can_place(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &NoiseChunkBlocks,
        origin_x: i32,
        origin_y: i32,
        origin_z: i32,
        min_x: i32,
        max_x: i32,
        min_z: i32,
        max_z: i32,
    ) -> bool {
        let mut hole_count = 0;
        for dx in min_x..=max_x {
            for dy in -1..=4 {
                for dz in min_z..=max_z {
                    let world_x = origin_x + dx;
                    let world_y = origin_y + dy;
                    let world_z = origin_z + dz;
                    let Some(layer) = layer_at_world(
                        chunk,
                        chunk_min_x,
                        chunk_min_z,
                        world_x,
                        world_y,
                        world_z,
                        settings.min_y,
                    ) else {
                        return false;
                    };
                    let solid = is_full_solid_layer(layer);
                    if (dy == -1 || dy == 4) && !solid {
                        return false;
                    }
                    if (dx == min_x || dx == max_x || dz == min_z || dz == max_z)
                        && dy == 0
                        && layer.is_air
                        && is_air_at_world(
                            chunk,
                            chunk_min_x,
                            chunk_min_z,
                            world_x,
                            world_y + 1,
                            world_z,
                            settings.min_y,
                        )
                    {
                        hole_count += 1;
                    }
                }
            }
        }

        (1..=5).contains(&hole_count)
    }

    #[allow(clippy::too_many_arguments)]
    fn place_shell_and_room(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
        origin_x: i32,
        origin_y: i32,
        origin_z: i32,
        min_x: i32,
        max_x: i32,
        min_z: i32,
        max_z: i32,
    ) {
        for dx in min_x..=max_x {
            for dy in (-1..=4).rev() {
                for dz in min_z..=max_z {
                    let world_x = origin_x + dx;
                    let world_y = origin_y + dy;
                    let world_z = origin_z + dz;
                    let Some((local_x, local_z)) =
                        local_coords(world_x, world_z, chunk_min_x, chunk_min_z)
                    else {
                        continue;
                    };
                    let Some(current) = chunk.layer(local_x, world_y, local_z, settings.min_y)
                    else {
                        continue;
                    };
                    let boundary = dx == min_x
                        || dy == -1
                        || dz == min_z
                        || dx == max_x
                        || dy == 4
                        || dz == max_z;
                    if boundary {
                        if world_y >= settings.min_y
                            && !is_solid_at_world(
                                chunk,
                                chunk_min_x,
                                chunk_min_z,
                                world_x,
                                world_y - 1,
                                world_z,
                                settings.min_y,
                            )
                        {
                            self.set_room_block(
                                chunk,
                                local_x,
                                world_y,
                                local_z,
                                settings.min_y,
                                self.cave_air.clone(),
                            );
                        } else if is_full_solid_layer(current) && !current.is("minecraft:chest") {
                            let block = if dy == -1 && random.next_int(4) != 0 {
                                self.mossy_cobblestone.clone()
                            } else {
                                self.cobblestone.clone()
                            };
                            self.set_room_block(
                                chunk,
                                local_x,
                                world_y,
                                local_z,
                                settings.min_y,
                                block,
                            );
                        }
                    } else if !current.is("minecraft:chest") && !current.is("minecraft:spawner") {
                        self.set_room_block(
                            chunk,
                            local_x,
                            world_y,
                            local_z,
                            settings.min_y,
                            self.cave_air.clone(),
                        );
                    }
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn place_chests(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
        origin_x: i32,
        origin_y: i32,
        origin_z: i32,
        x_radius: i32,
        z_radius: i32,
    ) {
        for _ in 0..2 {
            for _ in 0..3 {
                let world_x = origin_x + random.next_int(x_radius * 2 + 1) - x_radius;
                let world_z = origin_z + random.next_int(z_radius * 2 + 1) - z_radius;
                let Some((local_x, local_z)) =
                    local_coords(world_x, world_z, chunk_min_x, chunk_min_z)
                else {
                    continue;
                };
                if !chunk
                    .layer(local_x, origin_y, local_z, settings.min_y)
                    .is_some_and(|layer| layer.is_air)
                {
                    continue;
                }

                let wall_count = horizontal_directions()
                    .iter()
                    .filter(|(dx, dz)| {
                        is_solid_at_world(
                            chunk,
                            chunk_min_x,
                            chunk_min_z,
                            world_x + dx,
                            origin_y,
                            world_z + dz,
                            settings.min_y,
                        )
                    })
                    .count();
                if wall_count != 1 {
                    continue;
                }

                let chest = self
                    .chest
                    .with_property(
                        "facing",
                        chest_facing(
                            chunk,
                            chunk_min_x,
                            chunk_min_z,
                            world_x,
                            origin_y,
                            world_z,
                            settings.min_y,
                        ),
                    )
                    .with_property("waterlogged", "false")
                    .with_property("type", "single");
                chunk.set_layer(local_x, origin_y, local_z, settings.min_y, chest);
                chunk.push_block_entity(
                    world_x,
                    origin_y,
                    world_z,
                    CHEST_BLOCK_ENTITY_TYPE_ID,
                    chest_block_entity_nbt(random.next_long()),
                );
                break;
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn place_spawner(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
        origin_x: i32,
        origin_y: i32,
        origin_z: i32,
    ) {
        let Some((local_x, local_z)) = local_coords(origin_x, origin_z, chunk_min_x, chunk_min_z)
        else {
            return;
        };
        chunk.set_layer(
            local_x,
            origin_y,
            local_z,
            settings.min_y,
            self.spawner.clone(),
        );
        chunk.push_block_entity(
            origin_x,
            origin_y,
            origin_z,
            MOB_SPAWNER_BLOCK_ENTITY_TYPE_ID,
            spawner_block_entity_nbt(random_monster_room_entity(random)),
        );
    }

    fn set_room_block(
        &self,
        chunk: &mut NoiseChunkBlocks,
        local_x: usize,
        world_y: i32,
        local_z: usize,
        min_y: i32,
        block: BlockLayer,
    ) {
        chunk.set_layer(local_x, world_y, local_z, min_y, block);
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
            if !self.biome_filter.allows_at(&settings.density, x, y, z) {
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

    fn allows_at(self, density: &TerrainDensity, x: i32, y: i32, z: i32) -> bool {
        match self {
            Self::All => true,
            Self::Include(_) | Self::Exclude(_) => self.allows(density.biome(x, y, z)),
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
            if y > ocean_floor - 2 || !self.biome_filter.allows_at(&settings.density, x, y, z) {
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
                || !self.biome_filter.allows_at(&settings.density, x, y, z)
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
                    if !self.target_blocks.contains(&current.block.as_ref()) {
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
                .allows_at(&settings.density, world_x, world_y, world_z)
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
        if !current.is_air && !self.valid_blocks.contains(&current.block.as_ref()) {
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
            .is_some_and(|layer| self.valid_blocks.contains(&layer.block.as_ref()))
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
struct TrapezoidInt {
    min: i32,
    max: i32,
    plateau: i32,
}

impl TrapezoidInt {
    fn new(min: i32, max: i32, plateau: i32) -> Self {
        Self { min, max, plateau }
    }

    fn sample(self, random: &mut FeatureRandom) -> i32 {
        if self.plateau == 0 && self.max == -self.min {
            return random.next_int(self.max + 1) - random.next_int(self.max + 1);
        }

        let range = self.max - self.min;
        if self.plateau == range {
            return self.min + random.next_int(range + 1);
        }

        let plateau_start = (range - self.plateau) / 2;
        let plateau_end = range - plateau_start;
        self.min + random.next_int(plateau_end + 1) + random.next_int(plateau_start + 1)
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
        layer.block.as_ref(),
        "minecraft:stone" | "minecraft:granite" | "minecraft:diorite" | "minecraft:andesite"
    )
}

fn is_deepslate_ore_replaceable(layer: &BlockLayer) -> bool {
    matches!(
        layer.block.as_ref(),
        "minecraft:deepslate" | "minecraft:tuff"
    )
}

fn is_base_stone_overworld(layer: &BlockLayer) -> bool {
    matches!(
        layer.block.as_ref(),
        "minecraft:stone"
            | "minecraft:granite"
            | "minecraft:diorite"
            | "minecraft:andesite"
            | "minecraft:tuff"
            | "minecraft:deepslate"
    )
}

fn is_leaf_layer(layer: &BlockLayer) -> bool {
    matches!(
        layer.block.as_ref(),
        "minecraft:jungle_leaves"
            | "minecraft:oak_leaves"
            | "minecraft:spruce_leaves"
            | "minecraft:pale_oak_leaves"
            | "minecraft:dark_oak_leaves"
            | "minecraft:acacia_leaves"
            | "minecraft:birch_leaves"
            | "minecraft:azalea_leaves"
            | "minecraft:flowering_azalea_leaves"
            | "minecraft:mangrove_leaves"
            | "minecraft:cherry_leaves"
    )
}

fn is_log_layer(layer: &BlockLayer) -> bool {
    matches!(
        layer.block.as_ref(),
        "minecraft:oak_log"
            | "minecraft:oak_wood"
            | "minecraft:stripped_oak_log"
            | "minecraft:stripped_oak_wood"
    )
}

fn is_small_flower_layer(layer: &BlockLayer) -> bool {
    matches!(
        layer.block.as_ref(),
        "minecraft:dandelion"
            | "minecraft:open_eyeblossom"
            | "minecraft:poppy"
            | "minecraft:blue_orchid"
            | "minecraft:allium"
            | "minecraft:azure_bluet"
            | "minecraft:red_tulip"
            | "minecraft:orange_tulip"
            | "minecraft:white_tulip"
            | "minecraft:pink_tulip"
            | "minecraft:oxeye_daisy"
            | "minecraft:cornflower"
            | "minecraft:lily_of_the_valley"
            | "minecraft:wither_rose"
            | "minecraft:torchflower"
            | "minecraft:closed_eyeblossom"
            | "minecraft:golden_dandelion"
    )
}

fn valid_tree_position_layer(layer: &BlockLayer) -> bool {
    layer.is_air
        || is_leaf_layer(layer)
        || is_small_flower_layer(layer)
        || matches!(
            layer.block.as_ref(),
            "minecraft:pale_moss_carpet"
                | "minecraft:short_grass"
                | "minecraft:fern"
                | "minecraft:dead_bush"
                | "minecraft:vine"
                | "minecraft:glow_lichen"
                | "minecraft:sunflower"
                | "minecraft:lilac"
                | "minecraft:rose_bush"
                | "minecraft:peony"
                | "minecraft:tall_grass"
                | "minecraft:large_fern"
                | "minecraft:hanging_roots"
                | "minecraft:pitcher_plant"
                | "minecraft:water"
                | "minecraft:seagrass"
                | "minecraft:tall_seagrass"
                | "minecraft:bush"
                | "minecraft:firefly_bush"
                | "minecraft:warped_roots"
                | "minecraft:nether_sprouts"
                | "minecraft:crimson_roots"
                | "minecraft:leaf_litter"
                | "minecraft:short_dry_grass"
                | "minecraft:tall_dry_grass"
        )
}

fn cannot_replace_below_tree_trunk(layer: &BlockLayer) -> bool {
    matches!(
        layer.block.as_ref(),
        "minecraft:dirt"
            | "minecraft:coarse_dirt"
            | "minecraft:rooted_dirt"
            | "minecraft:mud"
            | "minecraft:muddy_mangrove_roots"
            | "minecraft:moss_block"
            | "minecraft:pale_moss_block"
            | "minecraft:podzol"
    )
}

fn oak_sapling_would_survive_at(
    chunk: &NoiseChunkBlocks,
    chunk_min_x: i32,
    chunk_min_z: i32,
    world_x: i32,
    world_y: i32,
    world_z: i32,
    min_y: i32,
) -> bool {
    layer_at_world(
        chunk,
        chunk_min_x,
        chunk_min_z,
        world_x,
        world_y,
        world_z,
        min_y,
    )
    .is_some_and(valid_tree_position_layer)
        && supports_vegetation_at_world(
            chunk,
            chunk_min_x,
            chunk_min_z,
            world_x,
            world_y - 1,
            world_z,
            min_y,
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
    matches!(layer.block.as_ref(), "minecraft:water" | "minecraft:lava")
}

fn is_water_or_air_layer(layer: &BlockLayer) -> bool {
    layer.is_air || is_water_layer(layer)
}

fn is_air_or_water_layer(layer: &BlockLayer) -> bool {
    layer.is_air || is_water_layer(layer)
}

fn is_full_solid_layer(layer: &BlockLayer) -> bool {
    !layer.is_air && !is_fluid_layer(layer)
}

fn supports_vegetation_layer(layer: &BlockLayer) -> bool {
    SUPPORTS_VEGETATION_BLOCKS.contains(&layer.block.as_ref())
}

fn supports_dead_bush_layer(layer: &BlockLayer) -> bool {
    supports_vegetation_layer(layer)
        || matches!(
            layer.block.as_ref(),
            "minecraft:sand"
                | "minecraft:red_sand"
                | "minecraft:terracotta"
                | "minecraft:white_terracotta"
                | "minecraft:orange_terracotta"
                | "minecraft:yellow_terracotta"
                | "minecraft:brown_terracotta"
                | "minecraft:red_terracotta"
                | "minecraft:light_gray_terracotta"
        )
}

fn supports_dry_vegetation_layer(layer: &BlockLayer) -> bool {
    matches!(
        layer.block.as_ref(),
        "minecraft:sand"
            | "minecraft:red_sand"
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
    )
}

fn supports_sugar_cane_layer(layer: &BlockLayer) -> bool {
    matches!(
        layer.block.as_ref(),
        "minecraft:grass_block"
            | "minecraft:dirt"
            | "minecraft:coarse_dirt"
            | "minecraft:podzol"
            | "minecraft:sand"
            | "minecraft:red_sand"
            | "minecraft:mud"
    )
}

fn supports_cactus_layer(layer: &BlockLayer) -> bool {
    matches!(
        layer.block.as_ref(),
        "minecraft:sand" | "minecraft:red_sand" | "minecraft:cactus"
    )
}

fn can_lake_replace_block(layer: &BlockLayer) -> bool {
    can_feature_replace_block(layer)
}

fn local_coord(world: i32, origin: i32) -> Option<usize> {
    let local = world - origin;
    (0..16).contains(&local).then_some(local as usize)
}

fn local_coords(
    world_x: i32,
    world_z: i32,
    origin_x: i32,
    origin_z: i32,
) -> Option<(usize, usize)> {
    Some((
        local_coord(world_x, origin_x)?,
        local_coord(world_z, origin_z)?,
    ))
}

fn can_feature_replace_block(layer: &BlockLayer) -> bool {
    !matches!(
        layer.block.as_ref(),
        "minecraft:bedrock"
            | "minecraft:spawner"
            | "minecraft:chest"
            | "minecraft:end_portal_frame"
            | "minecraft:reinforced_deepslate"
            | "minecraft:trial_spawner"
            | "minecraft:vault"
    )
}

fn layer_at_world<'a>(
    chunk: &'a NoiseChunkBlocks,
    chunk_min_x: i32,
    chunk_min_z: i32,
    world_x: i32,
    world_y: i32,
    world_z: i32,
    min_y: i32,
) -> Option<&'a BlockLayer> {
    let (local_x, local_z) = local_coords(world_x, world_z, chunk_min_x, chunk_min_z)?;
    chunk.layer(local_x, world_y, local_z, min_y)
}

fn is_air_at_world(
    chunk: &NoiseChunkBlocks,
    chunk_min_x: i32,
    chunk_min_z: i32,
    world_x: i32,
    world_y: i32,
    world_z: i32,
    min_y: i32,
) -> bool {
    layer_at_world(
        chunk,
        chunk_min_x,
        chunk_min_z,
        world_x,
        world_y,
        world_z,
        min_y,
    )
    .is_some_and(|layer| layer.is_air)
}

fn is_water_at_world(
    chunk: &NoiseChunkBlocks,
    chunk_min_x: i32,
    chunk_min_z: i32,
    world_x: i32,
    world_y: i32,
    world_z: i32,
    min_y: i32,
) -> bool {
    layer_at_world(
        chunk,
        chunk_min_x,
        chunk_min_z,
        world_x,
        world_y,
        world_z,
        min_y,
    )
    .is_some_and(is_water_layer)
}

fn is_solid_at_world(
    chunk: &NoiseChunkBlocks,
    chunk_min_x: i32,
    chunk_min_z: i32,
    world_x: i32,
    world_y: i32,
    world_z: i32,
    min_y: i32,
) -> bool {
    layer_at_world(
        chunk,
        chunk_min_x,
        chunk_min_z,
        world_x,
        world_y,
        world_z,
        min_y,
    )
    .is_some_and(is_full_solid_layer)
}

fn supports_vegetation_at_world(
    chunk: &NoiseChunkBlocks,
    chunk_min_x: i32,
    chunk_min_z: i32,
    world_x: i32,
    world_y: i32,
    world_z: i32,
    min_y: i32,
) -> bool {
    layer_at_world(
        chunk,
        chunk_min_x,
        chunk_min_z,
        world_x,
        world_y,
        world_z,
        min_y,
    )
    .is_some_and(supports_vegetation_layer)
}

fn is_full_solid_at_world(
    chunk: &NoiseChunkBlocks,
    chunk_min_x: i32,
    chunk_min_z: i32,
    world_x: i32,
    world_y: i32,
    world_z: i32,
    min_y: i32,
) -> bool {
    layer_at_world(
        chunk,
        chunk_min_x,
        chunk_min_z,
        world_x,
        world_y,
        world_z,
        min_y,
    )
    .is_some_and(is_full_solid_layer)
}

fn horizontal_directions() -> &'static [(i32, i32)] {
    &[(0, -1), (1, 0), (0, 1), (-1, 0)]
}

fn all_directions() -> [(i32, i32, i32, &'static str); 6] {
    [
        (0, -1, 0, "down"),
        (0, 1, 0, "up"),
        (0, 0, -1, "north"),
        (0, 0, 1, "south"),
        (-1, 0, 0, "west"),
        (1, 0, 0, "east"),
    ]
}

fn shuffled_all_directions(random: &mut FeatureRandom) -> Vec<(i32, i32, i32, &'static str)> {
    let mut directions = all_directions().to_vec();
    shuffle_directions(&mut directions, random);
    directions
}

fn shuffle_directions(
    directions: &mut [(i32, i32, i32, &'static str)],
    random: &mut FeatureRandom,
) {
    for index in (1..directions.len()).rev() {
        let swap = random.next_int(index as i32 + 1) as usize;
        directions.swap(index, swap);
    }
}

fn shuffle_positions(positions: &mut [(i32, i32, i32)], random: &mut FeatureRandom) {
    for index in (1..positions.len()).rev() {
        let swap = random.next_int(index as i32 + 1) as usize;
        positions.swap(index, swap);
    }
}

fn direction_opposite(direction: (i32, i32, i32, &'static str)) -> (i32, i32, i32, &'static str) {
    let name = direction_opposite_name(direction.3);
    direction_by_name(name)
}

fn direction_opposite_name(name: &str) -> &'static str {
    match name {
        "down" => "up",
        "up" => "down",
        "north" => "south",
        "south" => "north",
        "west" => "east",
        "east" => "west",
        _ => "north",
    }
}

fn direction_by_name(name: &str) -> (i32, i32, i32, &'static str) {
    match name {
        "down" => (0, -1, 0, "down"),
        "up" => (0, 1, 0, "up"),
        "north" => (0, 0, -1, "north"),
        "south" => (0, 0, 1, "south"),
        "west" => (-1, 0, 0, "west"),
        "east" => (1, 0, 0, "east"),
        _ => (0, 0, -1, "north"),
    }
}

fn spread_direction_axis(direction: (i32, i32, i32, &'static str)) -> u8 {
    if direction.0 != 0 {
        0
    } else if direction.1 != 0 {
        1
    } else {
        2
    }
}

fn chest_facing(
    chunk: &NoiseChunkBlocks,
    chunk_min_x: i32,
    chunk_min_z: i32,
    world_x: i32,
    world_y: i32,
    world_z: i32,
    min_y: i32,
) -> &'static str {
    for (dx, dz, facing) in [
        (0, -1, "south"),
        (1, 0, "west"),
        (0, 1, "north"),
        (-1, 0, "east"),
    ] {
        if is_solid_at_world(
            chunk,
            chunk_min_x,
            chunk_min_z,
            world_x + dx,
            world_y,
            world_z + dz,
            min_y,
        ) {
            return facing;
        }
    }
    "north"
}

fn random_monster_room_entity(random: &mut FeatureRandom) -> &'static str {
    match random.next_int(4) {
        0 => "minecraft:skeleton",
        1 | 2 => "minecraft:zombie",
        _ => "minecraft:spider",
    }
}

fn chest_block_entity_nbt(loot_table_seed: i64) -> Tag {
    let mut fields = HashMap::new();
    fields.insert(
        "LootTable".to_string(),
        Tag::String(Arc::from("minecraft:chests/simple_dungeon")),
    );
    if loot_table_seed != 0 {
        fields.insert("LootTableSeed".to_string(), Tag::Long(loot_table_seed));
    }
    Tag::Compound(Arc::new(fields))
}

fn spawner_block_entity_nbt(entity_id: &str) -> Tag {
    compound_tag([
        ("Delay", Tag::Short(20)),
        ("MinSpawnDelay", Tag::Short(200)),
        ("MaxSpawnDelay", Tag::Short(800)),
        ("SpawnCount", Tag::Short(4)),
        ("MaxNearbyEntities", Tag::Short(6)),
        ("RequiredPlayerRange", Tag::Short(16)),
        ("SpawnRange", Tag::Short(4)),
        (
            "SpawnData",
            compound_tag([(
                "entity",
                compound_tag([("id", Tag::String(Arc::from(entity_id.to_string())))]),
            )]),
        ),
    ])
}

fn beehive_block_entity_nbt(random: &mut FeatureRandom) -> Tag {
    let bee_count = 2 + random.next_int(2);
    let bees = (0..bee_count)
        .map(|_| {
            compound_tag([
                (
                    "entity_data",
                    compound_tag([("id", Tag::String(Arc::from("minecraft:bee")))]),
                ),
                ("ticks_in_hive", Tag::Int(random.next_int(599))),
                ("min_ticks_in_hive", Tag::Int(600)),
            ])
        })
        .collect::<Vec<_>>();

    let mut fields = HashMap::new();
    fields.insert(
        "bees".to_string(),
        Tag::List(
            ListHeader {
                tag_id: tag_id::COMPOUND,
                length: bees.len() as i32,
            },
            Arc::from(bees),
        ),
    );
    Tag::Compound(Arc::new(fields))
}

fn can_geode_replace_block(layer: &BlockLayer) -> bool {
    can_feature_replace_block(layer)
}

fn is_geode_invalid_block(layer: &BlockLayer) -> bool {
    matches!(
        layer.block.as_ref(),
        "minecraft:bedrock"
            | "minecraft:water"
            | "minecraft:lava"
            | "minecraft:ice"
            | "minecraft:packed_ice"
            | "minecraft:blue_ice"
    )
}

fn can_amethyst_cluster_grow_at(layer: &BlockLayer) -> bool {
    layer.is_air || layer.is("minecraft:water")
}

fn is_freezing_biome(biome: &str) -> bool {
    matches!(
        biome,
        "minecraft:frozen_ocean"
            | "minecraft:deep_frozen_ocean"
            | "minecraft:frozen_river"
            | "minecraft:snowy_beach"
            | "minecraft:snowy_plains"
            | "minecraft:ice_spikes"
            | "minecraft:snowy_taiga"
            | "minecraft:grove"
            | "minecraft:snowy_slopes"
            | "minecraft:jagged_peaks"
            | "minecraft:frozen_peaks"
    )
}

fn amethyst_cluster_block(block: &str) -> BlockLayer {
    BlockLayer::with_properties(block, &[("facing", "up"), ("waterlogged", "false")])
}

fn inv_sqrt_distance(x0: i32, y0: i32, z0: i32, x1: i32, y1: i32, z1: i32, offset: i32) -> f64 {
    let dx = x0 - x1;
    let dy = y0 - y1;
    let dz = z0 - z1;
    1.0 / ((dx * dx + dy * dy + dz * dz + offset) as f64).sqrt()
}

fn geode_noise(x: i32, y: i32, z: i32) -> f64 {
    let seed = (x as i64).wrapping_mul(3_129_871)
        ^ (z as i64).wrapping_mul(116_129_781)
        ^ (y as i64).wrapping_mul(42_317_861);
    let mut random = FeatureRandom::new(seed);
    random.next_double() * 2.0 - 1.0
}

fn scan_down_to_solid(
    settings: &NoiseSettings,
    chunk: &NoiseChunkBlocks,
    local_x: usize,
    origin_y: i32,
    local_z: usize,
    max_steps: i32,
) -> Option<i32> {
    for step in 0..=max_steps {
        let y = origin_y - step;
        if !(settings.min_y..settings.min_y + settings.height).contains(&y)
            || y - 5 < settings.min_y
        {
            return None;
        }
        if chunk
            .layer(local_x, y, local_z, settings.min_y)
            .is_some_and(|layer| !layer.is_air)
        {
            return Some(y);
        }
    }

    None
}

fn lake_index(x: usize, y: usize, z: usize) -> usize {
    (x * 16 + z) * 8 + y
}

fn is_lake_boundary(grid: &[bool], x: usize, y: usize, z: usize) -> bool {
    (x < 15 && grid[lake_index(x + 1, y, z)])
        || (x > 0 && grid[lake_index(x - 1, y, z)])
        || (z < 15 && grid[lake_index(x, y, z + 1)])
        || (z > 0 && grid[lake_index(x, y, z - 1)])
        || (y < 7 && grid[lake_index(x, y + 1, z)])
        || (y > 0 && grid[lake_index(x, y - 1, z)])
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
    block: Arc<str>,
    block_state_id: i32,
    properties: Arc<[(String, String)]>,
    is_air: bool,
}

impl BlockLayer {
    fn new(block: &str) -> Self {
        let block = normalize_identifier(block);
        let state = chunk_nbt::default_block_state(&block);
        Self {
            is_air: is_air_block(&block),
            block: Arc::from(block),
            block_state_id: state.id,
            properties: Arc::from(state.properties),
        }
    }

    fn with_properties(block: &str, properties: &[(&str, &str)]) -> Self {
        let block = normalize_identifier(block);
        let mut properties = properties
            .iter()
            .map(|(name, value)| ((*name).to_string(), (*value).to_string()))
            .collect::<Vec<_>>();
        properties.sort_by(|left, right| left.0.cmp(&right.0));
        let state = chunk_nbt::block_state(&block, &properties);
        Self {
            is_air: is_air_block(&block),
            block: Arc::from(block),
            block_state_id: state.id,
            properties: Arc::from(state.properties),
        }
    }

    fn is(&self, block: &str) -> bool {
        self.block.as_ref() == block
    }

    fn with_property(&self, name: &str, value: &str) -> Self {
        let mut properties = self.properties.iter().cloned().collect::<Vec<_>>();
        if let Some((_, existing)) = properties
            .iter_mut()
            .find(|(property_name, _)| property_name == name)
        {
            *existing = value.to_string();
        } else {
            properties.push((name.to_string(), value.to_string()));
        }
        properties.sort_by(|left, right| left.0.cmp(&right.0));
        let state = chunk_nbt::block_state(self.block.as_ref(), &properties);
        Self {
            is_air: self.is_air,
            block: self.block.clone(),
            block_state_id: state.id,
            properties: Arc::from(state.properties),
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
    let noise_kind = vanilla_noise::OverworldNoiseKind::from_preset(preset);

    Ok(NoiseSettings {
        min_y,
        height,
        sea_level,
        air_block: BlockLayer::new("minecraft:air"),
        bedrock_block: BlockLayer::new("minecraft:bedrock"),
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
        lava_lake_fluid_block: BlockLayer::new("minecraft:lava"),
        lava_lake_barrier_block: BlockLayer::new("minecraft:stone"),
        cave_air_block: BlockLayer::new("minecraft:cave_air"),
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
        if noise_section_is_air(chunk, section_y) {
            continue;
        }
        sections.push(compound_tag([
            ("Y", Tag::Byte(section_y as i8)),
            ("block_states", noise_block_states_tag(chunk, section_y)),
            ("biomes", noise_biomes_tag(chunk, section_y)),
        ]));
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
    let mut index_by_biome = HashMap::<&'static str, usize>::new();
    let mut values = vec![0_i32; 4 * 4 * 4];

    for x in 0..4 {
        for y in 0..4 {
            for z in 0..4 {
                let biome = chunk.biome(section_y, x, y, z);
                let next_index = palette.len();
                let palette_index = *index_by_biome.entry(biome).or_insert_with(|| {
                    palette.push(Tag::String(Arc::from(biome)));
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
                        !before.is_air && before.block.as_ref() != "minecraft:lava" && after.is_air
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
            block_entities: Vec::new(),
        };
        let mut found = HashSet::new();

        settings
            .ore_features
            .place_chunk(&settings, 0, 0, &mut chunk);
        for column in &chunk.columns {
            for layer in &column.blocks {
                match layer.block.as_ref() {
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
                    .any(|target| target.block.block.as_ref() == "minecraft:gold_ore")
                && matches!(feature.biome_filter, FeatureBiomeFilter::Include(biomes) if biomes.contains(&"minecraft:badlands"))
        }));
        assert!(features.iter().any(|feature| {
            feature.feature_index == 29
                && feature.step_index == 6
                && feature
                    .ore
                    .targets
                    .iter()
                    .any(|target| target.block.block.as_ref() == "minecraft:emerald_ore")
                && matches!(feature.biome_filter, FeatureBiomeFilter::Include(biomes) if biomes.contains(&"minecraft:meadow"))
        }));
        assert!(features.iter().any(|feature| {
            feature.feature_index == 0
                && feature.step_index == 7
                && feature
                    .ore
                    .targets
                    .iter()
                    .any(|target| target.block.block.as_ref() == "minecraft:infested_stone")
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
                && feature.config.state.block.as_ref() == "minecraft:water"
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
                && feature.config.state.block.as_ref() == "minecraft:lava"
        }));
    }

    #[test]
    fn vanilla_noise_configures_lava_lakes() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let lakes = &settings.ore_features.lakes;

        assert!(lakes.iter().any(|feature| {
            feature.step_index == 1
                && feature.feature_index == 0
                && matches!(
                    feature.placement,
                    LakePlacement::Underground {
                        rarity: 9,
                        max_scan_steps: 32,
                        ..
                    }
                )
        }));
        assert!(lakes.iter().any(|feature| {
            feature.step_index == 1
                && feature.feature_index == 1
                && matches!(feature.placement, LakePlacement::Surface { rarity: 200 })
        }));
    }

    #[test]
    fn vanilla_noise_configures_amethyst_geodes() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let geodes = &settings.ore_features.geodes;

        assert!(geodes.iter().any(|feature| {
            feature.step_index == 2
                && feature.feature_index == 0
                && feature.rarity == 24
                && matches!(
                    feature.height,
                    OreHeight::Uniform(HeightAnchor::AboveBottom(6), HeightAnchor::Absolute(30))
                )
        }));
    }

    #[test]
    fn vanilla_noise_configures_monster_rooms() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let rooms = &settings.ore_features.monster_rooms;

        assert!(rooms.iter().any(|feature| {
            feature.step_index == 3
                && feature.feature_index == 0
                && matches!(feature.count, OrePlacementCount::Constant(10))
                && matches!(
                    feature.height,
                    OreHeight::Uniform(HeightAnchor::Absolute(0), HeightAnchor::BelowTop(0))
                )
        }));
        assert!(rooms.iter().any(|feature| {
            feature.step_index == 3
                && feature.feature_index == 1
                && matches!(feature.count, OrePlacementCount::Constant(4))
                && matches!(
                    feature.height,
                    OreHeight::Uniform(HeightAnchor::AboveBottom(6), HeightAnchor::Absolute(-1))
                )
        }));
    }

    #[test]
    fn vanilla_noise_configures_glow_lichen() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let feature = &settings.ore_features.glow_lichen;

        assert_eq!(feature.step_index, 9);
        assert_eq!(feature.feature_index, 0);
        assert_eq!(feature.max_below_ocean_floor, -13);
        assert!(matches!(
            feature.count,
            OrePlacementCount::Uniform { min: 104, max: 157 }
        ));
        assert!(matches!(
            feature.height,
            OreHeight::Uniform(HeightAnchor::AboveBottom(0), HeightAnchor::Absolute(256))
        ));
        assert_eq!(feature.config.search_range, 20);
        assert!(feature.config.can_place_on_ceiling);
        assert!(!feature.config.can_place_on_floor);
        assert!(feature.config.can_place_on_wall);
        assert_eq!(feature.config.chance_of_spreading, 0.5);
        assert_eq!(
            feature.config.can_be_placed_on,
            GLOW_LICHEN_CAN_BE_PLACED_ON
        );
    }

    #[test]
    fn vanilla_noise_configures_patch_tall_grass_2() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let feature = settings
            .ore_features
            .vegetation_patches
            .iter()
            .find(|feature| feature.feature_index == 1)
            .unwrap();

        assert_eq!(feature.step_index, 9);
        assert_eq!(feature.feature_index, 1);
        let noise_threshold = feature.noise_threshold.as_ref().unwrap();
        assert_eq!(noise_threshold.noise_level, -0.8);
        assert_eq!(noise_threshold.below_noise, 0);
        assert_eq!(noise_threshold.above_noise, 7);
        assert_eq!(feature.rarity, 32);
        assert_eq!(feature.inner_count, 96);
        assert_eq!(feature.xz_offset.min, -7);
        assert_eq!(feature.xz_offset.max, 7);
        assert_eq!(feature.xz_offset.plateau, 0);
        assert_eq!(feature.y_offset.min, -3);
        assert_eq!(feature.y_offset.max, 3);
        assert_eq!(feature.y_offset.plateau, 0);
        assert!(feature.block.lower.is("minecraft:tall_grass"));
        assert!(
            feature
                .block
                .lower
                .properties
                .iter()
                .any(|(name, value)| { name == "half" && value == "lower" })
        );
        assert!(feature.block.upper.as_ref().is_some_and(|upper| {
            upper.is("minecraft:tall_grass")
                && upper
                    .properties
                    .iter()
                    .any(|(name, value)| name == "half" && value == "upper")
        }));
    }

    #[test]
    fn vanilla_noise_configures_patch_bush() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let feature = settings
            .ore_features
            .vegetation_patches
            .iter()
            .find(|feature| feature.feature_index == 2)
            .unwrap();

        assert_eq!(feature.step_index, 9);
        assert_eq!(feature.feature_index, 2);
        assert!(feature.noise_threshold.is_none());
        assert_eq!(feature.rarity, 4);
        assert_eq!(feature.inner_count, 24);
        assert_eq!(feature.xz_offset.min, -5);
        assert_eq!(feature.xz_offset.max, 5);
        assert_eq!(feature.xz_offset.plateau, 0);
        assert_eq!(feature.y_offset.min, -3);
        assert_eq!(feature.y_offset.max, 3);
        assert_eq!(feature.y_offset.plateau, 0);
        assert!(feature.block.lower.is("minecraft:bush"));
        assert!(feature.block.upper.is_none());
    }

    #[test]
    fn vanilla_noise_configures_patch_sunflower() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let feature = settings
            .ore_features
            .vegetation_patches
            .iter()
            .find(|feature| feature.feature_index == 3)
            .unwrap();

        assert_eq!(feature.step_index, 9);
        assert_eq!(feature.feature_index, 3);
        assert!(feature.noise_threshold.is_none());
        assert_eq!(feature.rarity, 3);
        assert_eq!(feature.inner_count, 96);
        assert_eq!(feature.xz_offset.min, -7);
        assert_eq!(feature.xz_offset.max, 7);
        assert_eq!(feature.y_offset.min, -3);
        assert_eq!(feature.y_offset.max, 3);
        assert!(feature.block.lower.is("minecraft:sunflower"));
        assert!(feature.block.upper.as_ref().is_some_and(|upper| {
            upper.is("minecraft:sunflower")
                && upper
                    .properties
                    .iter()
                    .any(|(name, value)| name == "half" && value == "upper")
        }));
        assert!(matches!(
            feature.biome_filter,
            FeatureBiomeFilter::Include(biomes)
                if biomes == SUNFLOWER_PATCH_BIOMES
        ));
    }

    #[test]
    fn vanilla_noise_configures_flower_plains() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let feature = settings
            .ore_features
            .vegetation_patches
            .iter()
            .find(|feature| feature.feature_index == 4)
            .unwrap();

        assert_eq!(feature.step_index, 9);
        assert_eq!(feature.feature_index, 4);
        let noise_threshold = feature.noise_threshold.as_ref().unwrap();
        assert_eq!(noise_threshold.noise_level, -0.8);
        assert_eq!(noise_threshold.below_noise, 15);
        assert_eq!(noise_threshold.above_noise, 4);
        assert_eq!(feature.rarity, 32);
        assert_eq!(feature.inner_count, 64);
        assert_eq!(feature.xz_offset.min, -6);
        assert_eq!(feature.xz_offset.max, 6);
        assert_eq!(feature.y_offset.min, -2);
        assert_eq!(feature.y_offset.max, 2);
        assert!(feature.block.lower.is("minecraft:dandelion"));
        assert!(matches!(
            &feature.block.provider,
            SimpleVegetationProvider::PlainsFlower {
                high_chance: 0.333_333_34,
                threshold: -0.8,
                scale: 0.005,
                ..
            }
        ));
        assert!(matches!(
            feature.biome_filter,
            FeatureBiomeFilter::Include(biomes)
                if biomes.contains(&"minecraft:plains")
                    && biomes.contains(&"minecraft:sunflower_plains")
        ));
    }

    #[test]
    fn vanilla_noise_configures_patch_grass_plain() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let feature = settings
            .ore_features
            .vegetation_patches
            .iter()
            .find(|feature| feature.feature_index == 5)
            .unwrap();

        assert_eq!(feature.step_index, 9);
        assert_eq!(feature.feature_index, 5);
        let noise_threshold = feature.noise_threshold.as_ref().unwrap();
        assert_eq!(noise_threshold.noise_level, -0.8);
        assert_eq!(noise_threshold.below_noise, 5);
        assert_eq!(noise_threshold.above_noise, 10);
        assert_eq!(feature.rarity, 1);
        assert_eq!(feature.inner_count, 32);
        assert_eq!(feature.xz_offset.min, -7);
        assert_eq!(feature.xz_offset.max, 7);
        assert_eq!(feature.y_offset.min, -3);
        assert_eq!(feature.y_offset.max, 3);
        assert!(feature.block.lower.is("minecraft:short_grass"));
        assert!(matches!(
            feature.biome_filter,
            FeatureBiomeFilter::Include(biomes)
                if biomes.contains(&"minecraft:plains")
                    && biomes.contains(&"minecraft:cherry_grove")
        ));
    }

    #[test]
    fn vanilla_noise_configures_normal_mushrooms_and_pumpkins() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let brown_mushroom = settings
            .ore_features
            .vegetation_patches
            .iter()
            .find(|feature| feature.feature_index == 6)
            .unwrap();
        let red_mushroom = settings
            .ore_features
            .vegetation_patches
            .iter()
            .find(|feature| feature.feature_index == 7)
            .unwrap();
        let pumpkin = settings
            .ore_features
            .vegetation_patches
            .iter()
            .find(|feature| feature.feature_index == 8)
            .unwrap();

        assert_eq!(brown_mushroom.step_index, 9);
        assert_eq!(brown_mushroom.rarity, 256);
        assert_eq!(brown_mushroom.inner_count, 96);
        assert!(brown_mushroom.block.lower.is("minecraft:brown_mushroom"));
        assert!(brown_mushroom.required_support.is_none());
        assert!(matches!(
            brown_mushroom.biome_filter,
            FeatureBiomeFilter::Include(biomes)
                if biomes.contains(&"minecraft:forest")
                    && biomes.contains(&"minecraft:plains")
        ));

        assert_eq!(red_mushroom.step_index, 9);
        assert_eq!(red_mushroom.rarity, 512);
        assert_eq!(red_mushroom.inner_count, 96);
        assert!(red_mushroom.block.lower.is("minecraft:red_mushroom"));
        assert!(matches!(
            red_mushroom.biome_filter,
            FeatureBiomeFilter::Include(biomes)
                if biomes.contains(&"minecraft:forest")
                    && biomes.contains(&"minecraft:plains")
        ));

        assert_eq!(pumpkin.step_index, 9);
        assert_eq!(pumpkin.rarity, 300);
        assert_eq!(pumpkin.inner_count, 96);
        assert!(pumpkin.block.lower.is("minecraft:pumpkin"));
        assert_eq!(pumpkin.required_support, Some("minecraft:grass_block"));
        assert!(matches!(
            pumpkin.biome_filter,
            FeatureBiomeFilter::Include(biomes)
                if biomes.contains(&"minecraft:plains")
                    && biomes.contains(&"minecraft:snowy_taiga")
        ));
    }

    #[test]
    fn vanilla_noise_configures_dead_bush_patches() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let normal = settings
            .ore_features
            .vegetation_patches
            .iter()
            .find(|feature| feature.feature_index == 9)
            .unwrap();
        let desert = settings
            .ore_features
            .vegetation_patches
            .iter()
            .find(|feature| feature.feature_index == 10)
            .unwrap();
        let badlands = settings
            .ore_features
            .vegetation_patches
            .iter()
            .find(|feature| feature.feature_index == 11)
            .unwrap();

        assert_eq!(normal.step_index, 9);
        assert_eq!(normal.outer_count, 1);
        assert_eq!(normal.rarity, 1);
        assert_eq!(normal.inner_count, 4);
        assert!(normal.block.lower.is("minecraft:dead_bush"));
        assert!(matches!(
            normal.biome_filter,
            FeatureBiomeFilter::Include(biomes)
                if biomes.contains(&"minecraft:swamp")
                    && biomes.contains(&"minecraft:old_growth_pine_taiga")
        ));

        assert_eq!(desert.outer_count, 2);
        assert_eq!(desert.rarity, 1);
        assert!(matches!(
            desert.biome_filter,
            FeatureBiomeFilter::Include(biomes) if biomes == DEAD_BUSH_DESERT_BIOMES
        ));

        assert_eq!(badlands.outer_count, 20);
        assert_eq!(badlands.rarity, 1);
        assert!(matches!(
            badlands.biome_filter,
            FeatureBiomeFilter::Include(biomes)
                if biomes.contains(&"minecraft:badlands")
                    && biomes.contains(&"minecraft:wooded_badlands")
        ));
    }

    #[test]
    fn vanilla_noise_configures_melon_patches() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let melon = settings
            .ore_features
            .vegetation_patches
            .iter()
            .find(|feature| feature.feature_index == 12)
            .unwrap();
        let sparse = settings
            .ore_features
            .vegetation_patches
            .iter()
            .find(|feature| feature.feature_index == 13)
            .unwrap();

        assert_eq!(melon.step_index, 9);
        assert_eq!(melon.outer_count, 1);
        assert_eq!(melon.rarity, 6);
        assert_eq!(melon.inner_count, 64);
        assert!(melon.block.lower.is("minecraft:melon"));
        assert_eq!(melon.required_support, Some("minecraft:grass_block"));
        assert!(matches!(
            melon.biome_filter,
            FeatureBiomeFilter::Include(biomes)
                if biomes.contains(&"minecraft:jungle")
                    && biomes.contains(&"minecraft:bamboo_jungle")
        ));

        assert_eq!(sparse.rarity, 64);
        assert_eq!(sparse.inner_count, 64);
        assert_eq!(sparse.required_support, Some("minecraft:grass_block"));
        assert!(matches!(
            sparse.biome_filter,
            FeatureBiomeFilter::Include(biomes) if biomes == MELON_SPARSE_PATCH_BIOMES
        ));
    }

    #[test]
    fn vanilla_noise_configures_block_column_plants() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let normal_cane = settings
            .ore_features
            .block_columns
            .iter()
            .find(|feature| feature.feature_index == 14)
            .unwrap();
        let badlands_cane = settings
            .ore_features
            .block_columns
            .iter()
            .find(|feature| feature.feature_index == 15)
            .unwrap();
        let desert_cane = settings
            .ore_features
            .block_columns
            .iter()
            .find(|feature| feature.feature_index == 16)
            .unwrap();
        let swamp_cane = settings
            .ore_features
            .block_columns
            .iter()
            .find(|feature| feature.feature_index == 17)
            .unwrap();
        let desert_cactus = settings
            .ore_features
            .block_columns
            .iter()
            .find(|feature| feature.feature_index == 18)
            .unwrap();
        let badlands_cactus = settings
            .ore_features
            .block_columns
            .iter()
            .find(|feature| feature.feature_index == 19)
            .unwrap();

        assert_eq!(normal_cane.step_index, 9);
        assert_eq!(normal_cane.rarity, 6);
        assert_eq!(normal_cane.inner_count, 20);
        assert_eq!(normal_cane.xz_offset.min, -4);
        assert_eq!(normal_cane.xz_offset.max, 4);
        assert_eq!(normal_cane.y_offset.min, 0);
        assert_eq!(normal_cane.y_offset.max, 0);
        assert!(normal_cane.column.block.is("minecraft:sugar_cane"));
        assert_eq!(normal_cane.column.height.min, 2);
        assert_eq!(normal_cane.column.height.max, 4);
        assert!(normal_cane.column.tip.is_none());
        assert!(matches!(
            normal_cane.biome_filter,
            FeatureBiomeFilter::Include(biomes)
                if biomes.contains(&"minecraft:plains")
                    && biomes.contains(&"minecraft:jungle")
        ));

        assert_eq!(badlands_cane.rarity, 5);
        assert!(matches!(
            badlands_cane.biome_filter,
            FeatureBiomeFilter::Include(biomes)
                if biomes.contains(&"minecraft:badlands")
                    && biomes.contains(&"minecraft:wooded_badlands")
        ));
        assert_eq!(desert_cane.rarity, 1);
        assert!(matches!(
            desert_cane.biome_filter,
            FeatureBiomeFilter::Include(biomes) if biomes == SUGAR_CANE_DESERT_BIOMES
        ));
        assert_eq!(swamp_cane.rarity, 3);
        assert!(matches!(
            swamp_cane.biome_filter,
            FeatureBiomeFilter::Include(biomes) if biomes == SUGAR_CANE_SWAMP_BIOMES
        ));

        assert_eq!(desert_cactus.rarity, 6);
        assert_eq!(desert_cactus.inner_count, 10);
        assert_eq!(desert_cactus.xz_offset.min, -7);
        assert_eq!(desert_cactus.y_offset.min, -3);
        assert!(desert_cactus.column.block.is("minecraft:cactus"));
        assert_eq!(desert_cactus.column.height.min, 1);
        assert_eq!(desert_cactus.column.height.max, 3);
        assert!(
            desert_cactus
                .column
                .tip
                .as_ref()
                .is_some_and(|tip| tip.block.is("minecraft:cactus_flower"))
        );
        assert!(matches!(
            desert_cactus.biome_filter,
            FeatureBiomeFilter::Include(biomes) if biomes == CACTUS_DESERT_BIOMES
        ));
        assert_eq!(badlands_cactus.rarity, 13);
        assert!(matches!(
            badlands_cactus.biome_filter,
            FeatureBiomeFilter::Include(biomes)
                if biomes.contains(&"minecraft:badlands")
                    && biomes.contains(&"minecraft:eroded_badlands")
        ));
    }

    #[test]
    fn vanilla_noise_configures_additional_grass_patches() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let normal = settings
            .ore_features
            .vegetation_patches
            .iter()
            .find(|feature| feature.feature_index == 20)
            .unwrap();
        let forest = settings
            .ore_features
            .vegetation_patches
            .iter()
            .find(|feature| feature.feature_index == 21)
            .unwrap();
        let badlands = settings
            .ore_features
            .vegetation_patches
            .iter()
            .find(|feature| feature.feature_index == 22)
            .unwrap();
        let savanna = settings
            .ore_features
            .vegetation_patches
            .iter()
            .find(|feature| feature.feature_index == 23)
            .unwrap();
        let taiga = settings
            .ore_features
            .vegetation_patches
            .iter()
            .find(|feature| feature.feature_index == 24)
            .unwrap();
        let taiga_2 = settings
            .ore_features
            .vegetation_patches
            .iter()
            .find(|feature| feature.feature_index == 25)
            .unwrap();
        let jungle = settings
            .ore_features
            .vegetation_patches
            .iter()
            .find(|feature| feature.feature_index == 26)
            .unwrap();
        let meadow = settings
            .ore_features
            .vegetation_patches
            .iter()
            .find(|feature| feature.feature_index == 27)
            .unwrap();
        let large_fern = settings
            .ore_features
            .vegetation_patches
            .iter()
            .find(|feature| feature.feature_index == 28)
            .unwrap();

        assert_eq!(normal.outer_count, 5);
        assert_eq!(normal.inner_count, 32);
        assert!(matches!(
            normal.biome_filter,
            FeatureBiomeFilter::Include(biomes)
                if biomes.contains(&"minecraft:swamp")
                    && biomes.contains(&"minecraft:windswept_savanna")
        ));
        assert_eq!(forest.outer_count, 2);
        assert!(matches!(
            forest.biome_filter,
            FeatureBiomeFilter::Include(biomes)
                if biomes.contains(&"minecraft:forest")
                    && biomes.contains(&"minecraft:pale_garden")
        ));
        assert_eq!(badlands.outer_count, 1);
        assert!(matches!(
            badlands.biome_filter,
            FeatureBiomeFilter::Include(biomes)
                if biomes.contains(&"minecraft:badlands")
                    && biomes.contains(&"minecraft:river")
        ));
        assert_eq!(savanna.outer_count, 20);
        assert!(matches!(
            savanna.biome_filter,
            FeatureBiomeFilter::Include(biomes) if biomes == PATCH_GRASS_SAVANNA_BIOMES
        ));
        assert_eq!(taiga.outer_count, 7);
        assert!(matches!(
            &taiga.block.provider,
            SimpleVegetationProvider::Weighted { entries }
                if entries.len() == 2
                    && entries[0].0.is("minecraft:short_grass")
                    && entries[0].1 == 1
                    && entries[1].0.is("minecraft:fern")
                    && entries[1].1 == 4
        ));
        assert_eq!(taiga_2.outer_count, 1);
        assert!(matches!(
            taiga_2.biome_filter,
            FeatureBiomeFilter::Include(biomes) if biomes == PATCH_GRASS_TAIGA_2_BIOMES
        ));
        assert_eq!(jungle.outer_count, 25);
        assert!(matches!(
            &jungle.block.provider,
            SimpleVegetationProvider::Weighted { entries }
                if entries[0].0.is("minecraft:short_grass")
                    && entries[0].1 == 3
                    && entries[1].0.is("minecraft:fern")
                    && entries[1].1 == 1
        ));
        assert_eq!(meadow.inner_count, 16);
        assert!(meadow.noise_threshold.is_some());
        assert!(large_fern.block.lower.is("minecraft:large_fern"));
        assert!(large_fern.block.upper.as_ref().is_some_and(|upper| {
            upper.is("minecraft:large_fern")
                && upper
                    .properties
                    .iter()
                    .any(|(name, value)| name == "half" && value == "upper")
        }));
    }

    #[test]
    fn vanilla_noise_configures_dry_grass_and_mushroom_variants() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let desert_dry_grass = settings
            .ore_features
            .vegetation_patches
            .iter()
            .find(|feature| feature.feature_index == 29)
            .unwrap();
        let badlands_dry_grass = settings
            .ore_features
            .vegetation_patches
            .iter()
            .find(|feature| feature.feature_index == 30)
            .unwrap();
        let brown_taiga = settings
            .ore_features
            .vegetation_patches
            .iter()
            .find(|feature| feature.feature_index == 31)
            .unwrap();
        let red_taiga = settings
            .ore_features
            .vegetation_patches
            .iter()
            .find(|feature| feature.feature_index == 32)
            .unwrap();
        let brown_old_growth = settings
            .ore_features
            .vegetation_patches
            .iter()
            .find(|feature| feature.feature_index == 33)
            .unwrap();
        let red_old_growth = settings
            .ore_features
            .vegetation_patches
            .iter()
            .find(|feature| feature.feature_index == 34)
            .unwrap();
        let brown_swamp = settings
            .ore_features
            .vegetation_patches
            .iter()
            .find(|feature| feature.feature_index == 35)
            .unwrap();
        let red_swamp = settings
            .ore_features
            .vegetation_patches
            .iter()
            .find(|feature| feature.feature_index == 36)
            .unwrap();

        assert_eq!(desert_dry_grass.rarity, 3);
        assert_eq!(desert_dry_grass.inner_count, 64);
        assert!(matches!(
            &desert_dry_grass.block.provider,
            SimpleVegetationProvider::Weighted { entries }
                if entries[0].0.is("minecraft:short_dry_grass")
                    && entries[0].1 == 1
                    && entries[1].0.is("minecraft:tall_dry_grass")
                    && entries[1].1 == 1
        ));
        assert!(matches!(
            desert_dry_grass.biome_filter,
            FeatureBiomeFilter::Include(biomes) if biomes == DRY_GRASS_DESERT_BIOMES
        ));
        assert_eq!(badlands_dry_grass.rarity, 6);
        assert!(matches!(
            badlands_dry_grass.biome_filter,
            FeatureBiomeFilter::Include(biomes)
                if biomes.contains(&"minecraft:badlands")
                    && biomes.contains(&"minecraft:wooded_badlands")
        ));

        assert_eq!(brown_taiga.rarity, 4);
        assert_eq!(brown_taiga.outer_count, 1);
        assert_eq!(red_taiga.rarity, 256);
        assert!(matches!(
            brown_taiga.biome_filter,
            FeatureBiomeFilter::Include(biomes)
                if biomes.contains(&"minecraft:taiga")
                    && biomes.contains(&"minecraft:mushroom_fields")
        ));
        assert_eq!(brown_old_growth.outer_count, 3);
        assert_eq!(brown_old_growth.rarity, 4);
        assert_eq!(red_old_growth.rarity, 171);
        assert!(matches!(
            red_old_growth.biome_filter,
            FeatureBiomeFilter::Include(biomes)
                if biomes.contains(&"minecraft:old_growth_pine_taiga")
                    && biomes.contains(&"minecraft:old_growth_spruce_taiga")
        ));
        assert_eq!(brown_swamp.outer_count, 2);
        assert_eq!(brown_swamp.rarity, 1);
        assert_eq!(red_swamp.rarity, 64);
        assert!(matches!(
            red_swamp.biome_filter,
            FeatureBiomeFilter::Include(biomes) if biomes == SWAMP_MUSHROOM_BIOMES
        ));
    }

    #[test]
    fn vanilla_noise_configures_trees_plains() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let feature = &settings.ore_features.trees_plains;

        assert_eq!(feature.step_index, 9);
        assert_eq!(feature.feature_index, 3);
        assert_eq!(feature.surface_water_depth, 0);
        assert_eq!(feature.count.entries, vec![(0, 19), (1, 1)]);
        assert_eq!(feature.count.total_weight, 20);
        assert_eq!(feature.config.fancy_chance, 0.333_333_34);
        assert_eq!(feature.config.fallen_chance, 0.0125);
        assert_eq!(feature.config.default_tree.base_height, 4);
        assert_eq!(feature.config.default_tree.height_rand_a, 2);
        assert_eq!(feature.config.default_tree.foliage_height, 3);
        assert_eq!(feature.config.default_tree.foliage_radius, 2);
        assert_eq!(feature.config.default_tree.beehive_probability, 0.05);
        assert!(feature.config.default_tree.trunk.is("minecraft:oak_log"));
        assert!(
            feature
                .config
                .default_tree
                .leaves
                .is("minecraft:oak_leaves")
        );
    }

    #[test]
    fn vanilla_noise_configures_freeze_top_layer() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let feature = &settings.ore_features.freeze_top_layer;

        assert_eq!(feature.step_index, 10);
        assert_eq!(feature.feature_index, 0);
        assert!(feature.snow_layer.is("minecraft:snow"));
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
            block_entities: Vec::new(),
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
                .all(|layer| layer.block.as_ref() != "minecraft:emerald_ore")
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
            block_entities: Vec::new(),
        };
        let mut random = FeatureRandom::new(12345);

        PlacedDiskFeature::sand(0).place(&settings, 0, 0, &mut chunk, &mut random);

        assert_eq!(chunk.ocean_floor_wg_height(0, 0, settings.min_y), 63);
        assert!(chunk.columns.iter().any(|column| {
            column
                .blocks
                .iter()
                .any(|layer| layer.block.as_ref() == "minecraft:sand")
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
            block_entities: Vec::new(),
        };
        chunk.set_layer(8, 64, 8, settings.min_y, air.clone());
        chunk.set_layer(9, 64, 8, settings.min_y, air);
        let spring = PlacedSpringFeature::water(0);

        assert!(spring.config.try_place(&settings, &mut chunk, 8, 64, 8));
        assert_eq!(
            chunk
                .layer(8, 64, 8, settings.min_y)
                .map(|layer| layer.block.as_ref()),
            Some("minecraft:water")
        );
    }

    #[test]
    fn vanilla_noise_lava_lake_carves_cavity_and_fluid() {
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
            block_entities: Vec::new(),
        };
        let mut random = FeatureRandom::new(12345);

        assert!(LakeFeatureConfig::lava().place(
            &settings,
            0,
            0,
            &mut chunk,
            &mut random,
            8,
            80,
            8,
        ));

        let mut has_lava = false;
        let mut has_cave_air = false;
        for column in &chunk.columns {
            for layer in &column.blocks {
                has_lava |= layer.is("minecraft:lava");
                has_cave_air |= layer.is("minecraft:cave_air");
            }
        }

        assert!(has_lava);
        assert!(has_cave_air);
    }

    #[test]
    fn vanilla_noise_monster_room_places_shell_spawner_and_chest_entities() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let stone = BlockLayer::new("minecraft:stone");
        let cave_air = BlockLayer::new("minecraft:cave_air");
        let mut placed = None;

        for seed in 0..10_000 {
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
                block_entities: Vec::new(),
            };
            for (x, z) in [
                (4, 8),
                (5, 8),
                (11, 8),
                (12, 8),
                (8, 4),
                (8, 5),
                (8, 11),
                (8, 12),
            ] {
                for y in 64..=65 {
                    chunk.set_layer(x, y, z, settings.min_y, cave_air.clone());
                }
            }

            let mut random = FeatureRandom::new(seed);
            if MonsterRoomFeatureConfig::new().place(
                &settings,
                0,
                0,
                &mut chunk,
                &mut random,
                8,
                64,
                8,
            ) && chunk
                .block_entities
                .iter()
                .any(|entity| entity.entity_type == CHEST_BLOCK_ENTITY_TYPE_ID)
            {
                placed = Some(chunk);
                break;
            }
        }

        let chunk = placed.expect("test seed range should include a room with a chest");

        assert_eq!(
            chunk
                .layer(8, 64, 8, settings.min_y)
                .map(|layer| layer.block.as_ref()),
            Some("minecraft:spawner")
        );
        assert!(chunk.columns.iter().any(|column| {
            column
                .blocks
                .iter()
                .any(|layer| layer.is("minecraft:mossy_cobblestone"))
        }));
        assert!(chunk.block_entities.iter().any(|entity| {
            entity.entity_type == MOB_SPAWNER_BLOCK_ENTITY_TYPE_ID && entity.position == (8, 64, 8)
        }));
        assert!(chunk.block_entities.iter().any(|entity| {
            entity.entity_type == CHEST_BLOCK_ENTITY_TYPE_ID
                && matches!(&entity.nbt, Tag::Compound(fields) if fields.contains_key("LootTable"))
        }));
    }

    #[test]
    fn vanilla_noise_glow_lichen_places_on_air_or_water_next_to_rock() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let stone = BlockLayer::new("minecraft:stone");
        let air = BlockLayer::new("minecraft:air");
        let water = BlockLayer::new("minecraft:water");
        let columns = (0..HEIGHTMAP_ENTRY_COUNT)
            .map(|_| NoiseColumnBlocks {
                blocks: (settings.min_y..settings.min_y + settings.height)
                    .map(|y| if y <= 50 { stone.clone() } else { air.clone() })
                    .collect(),
                first_available_height: 51 - settings.min_y,
            })
            .collect();
        let mut chunk = NoiseChunkBlocks {
            columns,
            biomes: Vec::new(),
            block_entities: Vec::new(),
        };
        chunk.set_layer(8, 48, 8, settings.min_y, water);
        let mut random = FeatureRandom::new(12345);

        assert!(MultifaceGrowthFeatureConfig::glow_lichen().place(
            &settings,
            0,
            0,
            &mut chunk,
            &mut random,
            8,
            48,
            8,
        ));

        let layer = chunk.layer(8, 48, 8, settings.min_y).unwrap();
        assert!(layer.is("minecraft:glow_lichen"));
        assert!(
            layer
                .properties
                .iter()
                .any(|(name, value)| name == "waterlogged" && value == "true")
        );
        assert!(layer.properties.iter().any(|(name, value)| {
            matches!(name.as_str(), "up" | "north" | "south" | "east" | "west") && value == "true"
        }));
    }

    #[test]
    fn vanilla_noise_glow_lichen_respects_ocean_floor_threshold() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let stone = BlockLayer::new("minecraft:stone");
        let air = BlockLayer::new("minecraft:air");
        let columns = (0..HEIGHTMAP_ENTRY_COUNT)
            .map(|_| NoiseColumnBlocks {
                blocks: (settings.min_y..settings.min_y + settings.height)
                    .map(|y| if y <= 80 { stone.clone() } else { air.clone() })
                    .collect(),
                first_available_height: 81 - settings.min_y,
            })
            .collect();
        let mut chunk = NoiseChunkBlocks {
            columns,
            biomes: Vec::new(),
            block_entities: Vec::new(),
        };
        let feature = PlacedMultifaceGrowthFeature::glow_lichen(0);
        let mut random = FeatureRandom::new(12345);

        feature.place(&settings, 0, 0, &mut chunk, &mut random);

        for column in &chunk.columns {
            for (index, layer) in column.blocks.iter().enumerate() {
                if layer.is("minecraft:glow_lichen") {
                    let y = settings.min_y + index as i32;
                    assert!(y <= 68);
                }
            }
        }
    }

    #[test]
    fn vanilla_noise_patch_tall_grass_places_double_plant_on_vegetation_support() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let grass_block = BlockLayer::new("minecraft:grass_block");
        let air = BlockLayer::new("minecraft:air");
        let columns = (0..HEIGHTMAP_ENTRY_COUNT)
            .map(|_| NoiseColumnBlocks {
                blocks: (settings.min_y..settings.min_y + settings.height)
                    .map(|y| {
                        if y <= 64 {
                            grass_block.clone()
                        } else {
                            air.clone()
                        }
                    })
                    .collect(),
                first_available_height: 65 - settings.min_y,
            })
            .collect();
        let mut chunk = NoiseChunkBlocks {
            columns,
            biomes: Vec::new(),
            block_entities: Vec::new(),
        };
        let mut random = FeatureRandom::new(1);

        assert!(SimpleVegetationBlock::tall_grass().place_at(
            &settings,
            0,
            0,
            &mut chunk,
            &mut random,
            8,
            65,
            8,
        ));

        let lower = chunk.layer(8, 65, 8, settings.min_y).unwrap();
        let upper = chunk.layer(8, 66, 8, settings.min_y).unwrap();
        assert!(lower.is("minecraft:tall_grass"));
        assert!(
            lower
                .properties
                .iter()
                .any(|(name, value)| { name == "half" && value == "lower" })
        );
        assert!(upper.is("minecraft:tall_grass"));
        assert!(
            upper
                .properties
                .iter()
                .any(|(name, value)| { name == "half" && value == "upper" })
        );
    }

    #[test]
    fn vanilla_noise_patch_tall_grass_requires_air_and_support() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let stone = BlockLayer::new("minecraft:stone");
        let grass_block = BlockLayer::new("minecraft:grass_block");
        let air = BlockLayer::new("minecraft:air");
        let columns = (0..HEIGHTMAP_ENTRY_COUNT)
            .map(|_| NoiseColumnBlocks {
                blocks: (settings.min_y..settings.min_y + settings.height)
                    .map(|_| air.clone())
                    .collect(),
                first_available_height: 0,
            })
            .collect();
        let mut chunk = NoiseChunkBlocks {
            columns,
            biomes: Vec::new(),
            block_entities: Vec::new(),
        };
        let mut random = FeatureRandom::new(1);
        chunk.set_layer(8, 64, 8, settings.min_y, stone.clone());
        assert!(!SimpleVegetationBlock::tall_grass().place_at(
            &settings,
            0,
            0,
            &mut chunk,
            &mut random,
            8,
            65,
            8,
        ));

        chunk.set_layer(8, 64, 8, settings.min_y, grass_block);
        chunk.set_layer(8, 66, 8, settings.min_y, stone);
        assert!(!SimpleVegetationBlock::tall_grass().place_at(
            &settings,
            0,
            0,
            &mut chunk,
            &mut random,
            8,
            65,
            8,
        ));
    }

    #[test]
    fn vanilla_noise_patch_bush_places_single_vegetation_block() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let grass_block = BlockLayer::new("minecraft:grass_block");
        let air = BlockLayer::new("minecraft:air");
        let columns = (0..HEIGHTMAP_ENTRY_COUNT)
            .map(|_| NoiseColumnBlocks {
                blocks: (settings.min_y..settings.min_y + settings.height)
                    .map(|y| {
                        if y <= 64 {
                            grass_block.clone()
                        } else {
                            air.clone()
                        }
                    })
                    .collect(),
                first_available_height: 65 - settings.min_y,
            })
            .collect();
        let mut chunk = NoiseChunkBlocks {
            columns,
            biomes: Vec::new(),
            block_entities: Vec::new(),
        };
        let mut random = FeatureRandom::new(1);

        assert!(SimpleVegetationBlock::single("minecraft:bush").place_at(
            &settings,
            0,
            0,
            &mut chunk,
            &mut random,
            8,
            65,
            8,
        ));

        assert_eq!(
            chunk
                .layer(8, 65, 8, settings.min_y)
                .map(|layer| layer.block.as_ref()),
            Some("minecraft:bush")
        );
        assert!(
            chunk
                .layer(8, 66, 8, settings.min_y)
                .is_some_and(|layer| layer.is_air)
        );
    }

    #[test]
    fn vanilla_noise_flower_plains_places_noise_selected_flower() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let grass_block = BlockLayer::new("minecraft:grass_block");
        let air = BlockLayer::new("minecraft:air");
        let columns = (0..HEIGHTMAP_ENTRY_COUNT)
            .map(|_| NoiseColumnBlocks {
                blocks: (settings.min_y..settings.min_y + settings.height)
                    .map(|y| {
                        if y <= 64 {
                            grass_block.clone()
                        } else {
                            air.clone()
                        }
                    })
                    .collect(),
                first_available_height: 65 - settings.min_y,
            })
            .collect();
        let mut chunk = NoiseChunkBlocks {
            columns,
            biomes: Vec::new(),
            block_entities: Vec::new(),
        };
        let mut random = FeatureRandom::new(1);

        assert!(SimpleVegetationBlock::plains_flower().place_at(
            &settings,
            0,
            0,
            &mut chunk,
            &mut random,
            8,
            65,
            8,
        ));

        let flower = chunk.layer(8, 65, 8, settings.min_y).unwrap();
        assert!(
            PLAINS_FLOWER_LOW_BLOCKS.contains(&flower.block.as_ref())
                || PLAINS_FLOWER_HIGH_BLOCKS.contains(&flower.block.as_ref())
                || flower.is("minecraft:dandelion")
        );
    }

    #[test]
    fn vanilla_noise_patch_grass_plain_places_short_grass() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let grass_block = BlockLayer::new("minecraft:grass_block");
        let air = BlockLayer::new("minecraft:air");
        let columns = (0..HEIGHTMAP_ENTRY_COUNT)
            .map(|_| NoiseColumnBlocks {
                blocks: (settings.min_y..settings.min_y + settings.height)
                    .map(|y| {
                        if y <= 64 {
                            grass_block.clone()
                        } else {
                            air.clone()
                        }
                    })
                    .collect(),
                first_available_height: 65 - settings.min_y,
            })
            .collect();
        let mut chunk = NoiseChunkBlocks {
            columns,
            biomes: Vec::new(),
            block_entities: Vec::new(),
        };
        let mut random = FeatureRandom::new(1);

        assert!(
            SimpleVegetationBlock::single("minecraft:short_grass").place_at(
                &settings,
                0,
                0,
                &mut chunk,
                &mut random,
                8,
                65,
                8,
            )
        );

        assert_eq!(
            chunk
                .layer(8, 65, 8, settings.min_y)
                .map(|layer| layer.block.as_ref()),
            Some("minecraft:short_grass")
        );
    }

    #[test]
    fn vanilla_noise_pumpkin_patch_requires_grass_support() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let dirt = BlockLayer::new("minecraft:dirt");
        let grass_block = BlockLayer::new("minecraft:grass_block");
        let air = BlockLayer::new("minecraft:air");
        let columns = (0..HEIGHTMAP_ENTRY_COUNT)
            .map(|_| NoiseColumnBlocks {
                blocks: (settings.min_y..settings.min_y + settings.height)
                    .map(|_| air.clone())
                    .collect(),
                first_available_height: 0,
            })
            .collect();
        let mut chunk = NoiseChunkBlocks {
            columns,
            biomes: Vec::new(),
            block_entities: Vec::new(),
        };
        let pumpkin = PlacedSimpleVegetationFeature::patch_pumpkin(0);

        chunk.set_layer(8, 64, 8, settings.min_y, dirt);
        assert!(!pumpkin.has_required_support(&chunk, 0, 0, 8, 64, 8, settings.min_y));

        chunk.set_layer(8, 64, 8, settings.min_y, grass_block);
        assert!(pumpkin.has_required_support(&chunk, 0, 0, 8, 64, 8, settings.min_y));
    }

    #[test]
    fn vanilla_noise_melon_patch_uses_grass_support() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let grass_block = BlockLayer::new("minecraft:grass_block");
        let air = BlockLayer::new("minecraft:air");
        let columns = (0..HEIGHTMAP_ENTRY_COUNT)
            .map(|_| NoiseColumnBlocks {
                blocks: (settings.min_y..settings.min_y + settings.height)
                    .map(|y| {
                        if y <= 64 {
                            grass_block.clone()
                        } else {
                            air.clone()
                        }
                    })
                    .collect(),
                first_available_height: 65 - settings.min_y,
            })
            .collect();
        let mut chunk = NoiseChunkBlocks {
            columns,
            biomes: Vec::new(),
            block_entities: Vec::new(),
        };
        let mut random = FeatureRandom::new(1);

        assert!(SimpleVegetationBlock::single("minecraft:melon").place_at(
            &settings,
            0,
            0,
            &mut chunk,
            &mut random,
            8,
            65,
            8,
        ));

        assert_eq!(
            chunk
                .layer(8, 65, 8, settings.min_y)
                .map(|layer| layer.block.as_ref()),
            Some("minecraft:melon")
        );
        assert!(
            PlacedSimpleVegetationFeature::patch_melon(0, 6).has_required_support(
                &chunk,
                0,
                0,
                8,
                64,
                8,
                settings.min_y
            )
        );
    }

    #[test]
    fn vanilla_noise_normal_mushroom_places_single_block() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let grass_block = BlockLayer::new("minecraft:grass_block");
        let air = BlockLayer::new("minecraft:air");
        let columns = (0..HEIGHTMAP_ENTRY_COUNT)
            .map(|_| NoiseColumnBlocks {
                blocks: (settings.min_y..settings.min_y + settings.height)
                    .map(|y| {
                        if y <= 64 {
                            grass_block.clone()
                        } else {
                            air.clone()
                        }
                    })
                    .collect(),
                first_available_height: 65 - settings.min_y,
            })
            .collect();
        let mut chunk = NoiseChunkBlocks {
            columns,
            biomes: Vec::new(),
            block_entities: Vec::new(),
        };
        let mut random = FeatureRandom::new(1);

        assert!(
            SimpleVegetationBlock::single("minecraft:brown_mushroom").place_at(
                &settings,
                0,
                0,
                &mut chunk,
                &mut random,
                8,
                65,
                8,
            )
        );

        assert_eq!(
            chunk
                .layer(8, 65, 8, settings.min_y)
                .map(|layer| layer.block.as_ref()),
            Some("minecraft:brown_mushroom")
        );
    }

    #[test]
    fn vanilla_noise_dead_bush_uses_dead_bush_support_rules() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let sand = BlockLayer::new("minecraft:sand");
        let air = BlockLayer::new("minecraft:air");
        let columns = (0..HEIGHTMAP_ENTRY_COUNT)
            .map(|_| NoiseColumnBlocks {
                blocks: (settings.min_y..settings.min_y + settings.height)
                    .map(|y| if y <= 64 { sand.clone() } else { air.clone() })
                    .collect(),
                first_available_height: 65 - settings.min_y,
            })
            .collect();
        let mut chunk = NoiseChunkBlocks {
            columns,
            biomes: Vec::new(),
            block_entities: Vec::new(),
        };
        let mut random = FeatureRandom::new(1);

        assert!(SimpleVegetationBlock::dead_bush().place_at(
            &settings,
            0,
            0,
            &mut chunk,
            &mut random,
            8,
            65,
            8,
        ));

        assert_eq!(
            chunk
                .layer(8, 65, 8, settings.min_y)
                .map(|layer| layer.block.as_ref()),
            Some("minecraft:dead_bush")
        );
        assert!(!supports_vegetation_layer(
            chunk.layer(8, 64, 8, settings.min_y).unwrap()
        ));
        assert!(supports_dead_bush_layer(
            chunk.layer(8, 64, 8, settings.min_y).unwrap()
        ));
    }

    #[test]
    fn vanilla_noise_weighted_grass_selects_configured_blocks() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let grass_block = BlockLayer::new("minecraft:grass_block");
        let air = BlockLayer::new("minecraft:air");
        let columns = (0..HEIGHTMAP_ENTRY_COUNT)
            .map(|_| NoiseColumnBlocks {
                blocks: (settings.min_y..settings.min_y + settings.height)
                    .map(|y| {
                        if y <= 64 {
                            grass_block.clone()
                        } else {
                            air.clone()
                        }
                    })
                    .collect(),
                first_available_height: 65 - settings.min_y,
            })
            .collect();
        let mut chunk = NoiseChunkBlocks {
            columns,
            biomes: Vec::new(),
            block_entities: Vec::new(),
        };
        let mut random = FeatureRandom::new(1);

        assert!(SimpleVegetationBlock::taiga_grass().place_at(
            &settings,
            0,
            0,
            &mut chunk,
            &mut random,
            8,
            65,
            8,
        ));

        let layer = chunk.layer(8, 65, 8, settings.min_y).unwrap();
        assert!(layer.is("minecraft:short_grass") || layer.is("minecraft:fern"));
    }

    #[test]
    fn vanilla_noise_large_fern_places_double_plant() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let grass_block = BlockLayer::new("minecraft:grass_block");
        let air = BlockLayer::new("minecraft:air");
        let columns = (0..HEIGHTMAP_ENTRY_COUNT)
            .map(|_| NoiseColumnBlocks {
                blocks: (settings.min_y..settings.min_y + settings.height)
                    .map(|y| {
                        if y <= 64 {
                            grass_block.clone()
                        } else {
                            air.clone()
                        }
                    })
                    .collect(),
                first_available_height: 65 - settings.min_y,
            })
            .collect();
        let mut chunk = NoiseChunkBlocks {
            columns,
            biomes: Vec::new(),
            block_entities: Vec::new(),
        };
        let mut random = FeatureRandom::new(1);

        assert!(SimpleVegetationBlock::large_fern().place_at(
            &settings,
            0,
            0,
            &mut chunk,
            &mut random,
            8,
            65,
            8,
        ));

        let lower = chunk.layer(8, 65, 8, settings.min_y).unwrap();
        let upper = chunk.layer(8, 66, 8, settings.min_y).unwrap();
        assert!(lower.is("minecraft:large_fern"));
        assert!(
            lower
                .properties
                .iter()
                .any(|(name, value)| name == "half" && value == "lower")
        );
        assert!(upper.is("minecraft:large_fern"));
        assert!(
            upper
                .properties
                .iter()
                .any(|(name, value)| name == "half" && value == "upper")
        );
    }

    #[test]
    fn vanilla_noise_dry_grass_uses_dry_vegetation_support_rules() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let red_sand = BlockLayer::new("minecraft:red_sand");
        let air = BlockLayer::new("minecraft:air");
        let columns = (0..HEIGHTMAP_ENTRY_COUNT)
            .map(|_| NoiseColumnBlocks {
                blocks: (settings.min_y..settings.min_y + settings.height)
                    .map(|y| {
                        if y <= 64 {
                            red_sand.clone()
                        } else {
                            air.clone()
                        }
                    })
                    .collect(),
                first_available_height: 65 - settings.min_y,
            })
            .collect();
        let mut chunk = NoiseChunkBlocks {
            columns,
            biomes: Vec::new(),
            block_entities: Vec::new(),
        };
        let mut random = FeatureRandom::new(1);

        assert!(SimpleVegetationBlock::dry_grass().place_at(
            &settings,
            0,
            0,
            &mut chunk,
            &mut random,
            8,
            65,
            8,
        ));

        let layer = chunk.layer(8, 65, 8, settings.min_y).unwrap();
        assert!(layer.is("minecraft:short_dry_grass") || layer.is("minecraft:tall_dry_grass"));
        assert!(!supports_vegetation_layer(
            chunk.layer(8, 64, 8, settings.min_y).unwrap()
        ));
        assert!(supports_dry_vegetation_layer(
            chunk.layer(8, 64, 8, settings.min_y).unwrap()
        ));
    }

    #[test]
    fn vanilla_noise_sugar_cane_requires_adjacent_water() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let sand = BlockLayer::new("minecraft:sand");
        let water = BlockLayer::new("minecraft:water");
        let air = BlockLayer::new("minecraft:air");
        let columns = (0..HEIGHTMAP_ENTRY_COUNT)
            .map(|_| NoiseColumnBlocks {
                blocks: (settings.min_y..settings.min_y + settings.height)
                    .map(|y| if y <= 64 { sand.clone() } else { air.clone() })
                    .collect(),
                first_available_height: 65 - settings.min_y,
            })
            .collect();
        let mut chunk = NoiseChunkBlocks {
            columns,
            biomes: Vec::new(),
            block_entities: Vec::new(),
        };

        assert!(!BlockColumnSupport::SugarCane.allows_at_world(
            &chunk,
            0,
            0,
            8,
            65,
            8,
            settings.min_y
        ));

        chunk.set_layer(9, 64, 8, settings.min_y, water);
        assert!(BlockColumnSupport::SugarCane.allows_at_world(
            &chunk,
            0,
            0,
            8,
            65,
            8,
            settings.min_y
        ));
    }

    #[test]
    fn vanilla_noise_sugar_cane_places_column() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let sand = BlockLayer::new("minecraft:sand");
        let water = BlockLayer::new("minecraft:water");
        let air = BlockLayer::new("minecraft:air");
        let columns = (0..HEIGHTMAP_ENTRY_COUNT)
            .map(|_| NoiseColumnBlocks {
                blocks: (settings.min_y..settings.min_y + settings.height)
                    .map(|y| if y <= 64 { sand.clone() } else { air.clone() })
                    .collect(),
                first_available_height: 65 - settings.min_y,
            })
            .collect();
        let mut chunk = NoiseChunkBlocks {
            columns,
            biomes: Vec::new(),
            block_entities: Vec::new(),
        };
        chunk.set_layer(9, 64, 8, settings.min_y, water);
        let mut random = FeatureRandom::new(1);

        assert!(BlockColumnFeatureConfig::sugar_cane().place_at(
            &settings,
            0,
            0,
            &mut chunk,
            &mut random,
            8,
            65,
            8,
        ));

        let placed = (65..=68)
            .filter(|y| {
                chunk
                    .layer(8, *y, 8, settings.min_y)
                    .is_some_and(|layer| layer.is("minecraft:sugar_cane"))
            })
            .count();
        assert!((2..=4).contains(&placed));
    }

    #[test]
    fn vanilla_noise_cactus_requires_open_sides() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let sand = BlockLayer::new("minecraft:sand");
        let stone = BlockLayer::new("minecraft:stone");
        let air = BlockLayer::new("minecraft:air");
        let columns = (0..HEIGHTMAP_ENTRY_COUNT)
            .map(|_| NoiseColumnBlocks {
                blocks: (settings.min_y..settings.min_y + settings.height)
                    .map(|y| if y <= 64 { sand.clone() } else { air.clone() })
                    .collect(),
                first_available_height: 65 - settings.min_y,
            })
            .collect();
        let mut chunk = NoiseChunkBlocks {
            columns,
            biomes: Vec::new(),
            block_entities: Vec::new(),
        };

        assert!(BlockColumnSupport::Cactus.allows_at_world(&chunk, 0, 0, 8, 65, 8, settings.min_y));

        chunk.set_layer(9, 65, 8, settings.min_y, stone);
        assert!(!BlockColumnSupport::Cactus.allows_at_world(
            &chunk,
            0,
            0,
            8,
            65,
            8,
            settings.min_y
        ));
    }

    #[test]
    fn vanilla_noise_cactus_places_column_and_optional_flower() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let sand = BlockLayer::new("minecraft:sand");
        let air = BlockLayer::new("minecraft:air");
        let columns = (0..HEIGHTMAP_ENTRY_COUNT)
            .map(|_| NoiseColumnBlocks {
                blocks: (settings.min_y..settings.min_y + settings.height)
                    .map(|y| if y <= 64 { sand.clone() } else { air.clone() })
                    .collect(),
                first_available_height: 65 - settings.min_y,
            })
            .collect();
        let mut chunk = NoiseChunkBlocks {
            columns,
            biomes: Vec::new(),
            block_entities: Vec::new(),
        };
        let mut random = FeatureRandom::new(1);

        assert!(BlockColumnFeatureConfig::cactus().place_at(
            &settings,
            0,
            0,
            &mut chunk,
            &mut random,
            8,
            65,
            8,
        ));

        assert!(chunk.columns.iter().any(|column| {
            column
                .blocks
                .iter()
                .any(|layer| layer.is("minecraft:cactus"))
        }));
        assert!(
            !chunk
                .layer(8, 65, 8, settings.min_y)
                .is_some_and(|layer| layer.is_air)
        );
    }

    #[test]
    fn vanilla_noise_sunflower_places_double_plant() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let grass_block = BlockLayer::new("minecraft:grass_block");
        let air = BlockLayer::new("minecraft:air");
        let columns = (0..HEIGHTMAP_ENTRY_COUNT)
            .map(|_| NoiseColumnBlocks {
                blocks: (settings.min_y..settings.min_y + settings.height)
                    .map(|y| {
                        if y <= 64 {
                            grass_block.clone()
                        } else {
                            air.clone()
                        }
                    })
                    .collect(),
                first_available_height: 65 - settings.min_y,
            })
            .collect();
        let mut chunk = NoiseChunkBlocks {
            columns,
            biomes: Vec::new(),
            block_entities: Vec::new(),
        };
        let mut random = FeatureRandom::new(1);

        assert!(SimpleVegetationBlock::sunflower().place_at(
            &settings,
            0,
            0,
            &mut chunk,
            &mut random,
            8,
            65,
            8,
        ));

        let lower = chunk.layer(8, 65, 8, settings.min_y).unwrap();
        let upper = chunk.layer(8, 66, 8, settings.min_y).unwrap();
        assert!(lower.is("minecraft:sunflower"));
        assert!(
            lower
                .properties
                .iter()
                .any(|(name, value)| name == "half" && value == "lower")
        );
        assert!(upper.is("minecraft:sunflower"));
        assert!(
            upper
                .properties
                .iter()
                .any(|(name, value)| name == "half" && value == "upper")
        );
    }

    #[test]
    fn vanilla_noise_patch_tall_grass_uses_biome_info_noise_threshold() {
        let placement = NoiseThresholdCount {
            noise_level: -0.8,
            below_noise: 0,
            above_noise: 7,
        };

        assert_eq!(placement.sample(0, 0), 7);
        assert_eq!(placement.sample(-2_000_000, -1_993_000), 0);
    }

    #[test]
    fn vanilla_noise_trees_plains_oak_places_logs_and_leaves() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let grass_block = BlockLayer::new("minecraft:grass_block");
        let air = BlockLayer::new("minecraft:air");
        let columns = (0..HEIGHTMAP_ENTRY_COUNT)
            .map(|_| NoiseColumnBlocks {
                blocks: (settings.min_y..settings.min_y + settings.height)
                    .map(|y| {
                        if y <= 64 {
                            grass_block.clone()
                        } else {
                            air.clone()
                        }
                    })
                    .collect(),
                first_available_height: 65 - settings.min_y,
            })
            .collect();
        let mut chunk = NoiseChunkBlocks {
            columns,
            biomes: Vec::new(),
            block_entities: Vec::new(),
        };
        let mut random = FeatureRandom::new(12345);

        assert!(OakTreeConfig::oak_bees_005().place(
            &settings,
            0,
            0,
            &mut chunk,
            &mut random,
            8,
            65,
            8,
        ));

        assert!(chunk.columns.iter().any(|column| {
            column
                .blocks
                .iter()
                .any(|layer| layer.is("minecraft:oak_log"))
        }));
        assert!(chunk.columns.iter().any(|column| {
            column
                .blocks
                .iter()
                .any(|layer| layer.is("minecraft:oak_leaves"))
        }));
        assert_eq!(
            chunk
                .layer(8, 64, 8, settings.min_y)
                .map(|layer| layer.block.as_ref()),
            Some("minecraft:dirt")
        );
    }

    #[test]
    fn vanilla_noise_trees_plains_can_place_beehive_entity() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let grass_block = BlockLayer::new("minecraft:grass_block");
        let air = BlockLayer::new("minecraft:air");
        let columns = (0..HEIGHTMAP_ENTRY_COUNT)
            .map(|_| NoiseColumnBlocks {
                blocks: (settings.min_y..settings.min_y + settings.height)
                    .map(|y| {
                        if y <= 64 {
                            grass_block.clone()
                        } else {
                            air.clone()
                        }
                    })
                    .collect(),
                first_available_height: 65 - settings.min_y,
            })
            .collect();
        let mut chunk = NoiseChunkBlocks {
            columns,
            biomes: Vec::new(),
            block_entities: Vec::new(),
        };
        let mut tree = OakTreeConfig::oak_bees_005();
        tree.beehive_probability = 1.0;
        let mut random = FeatureRandom::new(1);

        assert!(tree.place(&settings, 0, 0, &mut chunk, &mut random, 8, 65, 8));

        assert!(chunk.columns.iter().any(|column| {
            column
                .blocks
                .iter()
                .any(|layer| layer.is("minecraft:bee_nest"))
        }));
        assert!(chunk.block_entities.iter().any(|entity| {
            entity.entity_type == BEEHIVE_BLOCK_ENTITY_TYPE_ID
                && matches!(&entity.nbt, Tag::Compound(fields) if fields.contains_key("bees"))
        }));
    }

    #[test]
    fn vanilla_noise_block_entities_are_written_to_chunk_packet() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let mut chunk = settings.generate_chunk(0, 0);
        chunk.push_block_entity(
            1,
            64,
            2,
            CHEST_BLOCK_ENTITY_TYPE_ID,
            chest_block_entity_nbt(0),
        );

        let packet_entities = chunk.block_entities_as_packet(0, 0);

        assert_eq!(packet_entities.len(), 1);
        assert_eq!(packet_entities[0].xz, 0x12);
        assert_eq!(packet_entities[0].y, 64);
        assert_eq!(
            packet_entities[0].entity_type,
            VarInt(CHEST_BLOCK_ENTITY_TYPE_ID)
        );
        assert!(matches!(packet_entities[0].nbt, OptionalNbt(Some(_))));
    }

    #[test]
    fn vanilla_noise_amethyst_geode_places_layered_blocks() {
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
            block_entities: Vec::new(),
        };
        let mut random = FeatureRandom::new(12345);

        assert!(GeodeFeatureConfig::amethyst().place(
            &settings,
            0,
            0,
            &mut chunk,
            &mut random,
            8,
            0,
            8,
        ));

        let mut found = HashSet::new();
        for column in &chunk.columns {
            for layer in &column.blocks {
                match layer.block.as_ref() {
                    "minecraft:smooth_basalt"
                    | "minecraft:calcite"
                    | "minecraft:amethyst_block"
                    | "minecraft:budding_amethyst" => {
                        found.insert(layer.block.clone());
                    }
                    _ => {}
                }
            }
        }

        assert!(found.contains("minecraft:smooth_basalt"));
        assert!(found.contains("minecraft:calcite"));
        assert!(
            found.contains("minecraft:amethyst_block")
                || found.contains("minecraft:budding_amethyst")
        );
    }

    #[test]
    fn vanilla_noise_freeze_top_layer_places_ice_and_snow_in_cold_biomes() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let Some((origin_x, origin_z)) = (-64..=64).step_by(16).find_map(|chunk_x| {
            (-64..=64).step_by(16).find_map(|chunk_z| {
                is_freezing_biome(settings.density.biome(chunk_x, 64, chunk_z))
                    .then_some((chunk_x, chunk_z))
            })
        }) else {
            return;
        };
        let water = BlockLayer::new("minecraft:water");
        let air = BlockLayer::new("minecraft:air");
        let stone = BlockLayer::new("minecraft:stone");
        let columns = (0..HEIGHTMAP_ENTRY_COUNT)
            .map(|_| NoiseColumnBlocks {
                blocks: (settings.min_y..settings.min_y + settings.height)
                    .map(|y| {
                        if y < 62 {
                            stone.clone()
                        } else if y == 62 {
                            water.clone()
                        } else {
                            air.clone()
                        }
                    })
                    .collect(),
                first_available_height: settings.height,
            })
            .collect();
        let mut chunk = NoiseChunkBlocks {
            columns,
            biomes: Vec::new(),
            block_entities: Vec::new(),
        };

        settings
            .ore_features
            .freeze_top_layer
            .place(&settings, origin_x, origin_z, &mut chunk);

        assert_eq!(
            chunk
                .layer(0, 62, 0, settings.min_y)
                .map(|layer| layer.block.as_ref()),
            Some("minecraft:ice")
        );
        assert_eq!(
            chunk
                .layer(0, 63, 0, settings.min_y)
                .map(|layer| layer.block.as_ref()),
            Some("minecraft:snow")
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

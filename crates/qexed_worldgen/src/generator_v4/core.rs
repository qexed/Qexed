pub(crate) struct GeneratedChunk {
    pub packet: MapChunk,
    pub light_dampening: Vec<u8>,
    pub region_chunk: Option<super::region::ChunkData>,
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

    fn region_chunk(
        &self,
        dimension: &str,
        chunk_x: i32,
        chunk_z: i32,
    ) -> Result<Option<super::region::ChunkData>>;
}

pub(crate) fn from_config(config: &WorldConfig) -> Arc<dyn WorldChunkGenerator> {
    let generator: Arc<dyn WorldChunkGenerator> = match config.generator {
        WorldGeneratorConfig::Empty => Arc::new(EmptyWorldGenerator),
        WorldGeneratorConfig::VanillaFlat => Arc::new(VanillaFlatGenerator::from_preset(
            config.generator_preset.trim(),
        )),
        WorldGeneratorConfig::VanillaNoise => Arc::new(VanillaNoiseGenerator::from_config(config)),
    };
    SpawnPlatformGenerator::from_config(config, generator)
}

#[derive(Debug)]
struct SpawnPlatformGenerator {
    inner: Arc<dyn WorldChunkGenerator>,
    platform: SpawnPlatform,
}

#[derive(Debug, Clone)]
struct SpawnPlatform {
    dimension: String,
    block_state: i32,
    min_x: i32,
    max_x: i32,
    y: i32,
    min_z: i32,
    max_z: i32,
    /// Template blocks: (x, y, z, block_name). Overrides the flat platform.
    template_blocks: Vec<(i32, i32, i32, String)>,
}

impl SpawnPlatformGenerator {
    fn from_config(
        config: &WorldConfig,
        inner: Arc<dyn WorldChunkGenerator>,
    ) -> Arc<dyn WorldChunkGenerator> {
        let Some(platform) = SpawnPlatform::from_config(config) else {
            return inner;
        };
        Arc::new(Self { inner, platform })
    }

    fn platform_blocks_for_chunk(
        &self,
        dimension: &str,
        chunk_x: i32,
        chunk_z: i32,
    ) -> Vec<(qexed_packet::net_types::Position, i32, Option<i32>)> {
        if dimension.trim() != self.platform.dimension {
            return Vec::new();
        }

        let chunk_min_x = chunk_x * 16;
        let chunk_max_x = chunk_min_x + 15;
        let chunk_min_z = chunk_z * 16;
        let chunk_max_z = chunk_min_z + 15;

        // Template mode: place specific blocks at specific positions
        if !self.platform.template_blocks.is_empty() {
            let mut blocks = Vec::new();
            for (x, y, z, block_name) in &self.platform.template_blocks {
                if *x < chunk_min_x || *x > chunk_max_x || *z < chunk_min_z || *z > chunk_max_z {
                    continue;
                }
                if let Some(block_state) = chunk_nbt::default_block_state_id_if_known(block_name) {
                    let position = qexed_packet::net_types::Position {
                        x: *x,
                        y: *y,
                        z: *z,
                    };
                    blocks.push((
                        position.clone(),
                        block_state,
                        self.inner.block_state_at(dimension, &position),
                    ));
                }
            }
            return blocks;
        }

        // Flat platform mode
        let min_x = self.platform.min_x.max(chunk_min_x);
        let max_x = self.platform.max_x.min(chunk_max_x);
        let min_z = self.platform.min_z.max(chunk_min_z);
        let max_z = self.platform.max_z.min(chunk_max_z);
        if min_x > max_x || min_z > max_z {
            return Vec::new();
        }

        let mut blocks = Vec::with_capacity(((max_x - min_x + 1) * (max_z - min_z + 1)) as usize);
        for z in min_z..=max_z {
            for x in min_x..=max_x {
                let position = qexed_packet::net_types::Position {
                    x,
                    y: self.platform.y,
                    z,
                };
                blocks.push((
                    position.clone(),
                    self.platform.block_state,
                    self.inner.block_state_at(dimension, &position),
                ));
            }
        }
        blocks
    }
}

impl SpawnPlatform {
    fn from_config(config: &WorldConfig) -> Option<Self> {
        let configured = &config.spawn_platform;
        if !configured.enable {
            return None;
        }
        let dimension = configured.dimension.trim();
        if dimension.is_empty() {
            return None;
        }

        // Skyblock mode: predefined island template
        if configured.block.trim() == "skyblock" {
            return Some(Self {
                dimension: dimension.to_string(),
                block_state: 0, // unused in template mode
                min_x: -3,
                max_x: 3,
                y: configured.y,
                min_z: -3,
                max_z: 3,
                template_blocks: skyblock_island_template(configured.y),
            });
        }

        if !(super::WORLD_MIN_Y..=super::WORLD_MAX_Y).contains(&configured.y) {
            log::warn!(
                "spawn platform y is outside world height and will be ignored: y={}",
                configured.y
            );
            return None;
        }
        let Some(block_state) =
            chunk_nbt::default_block_state_id_if_known(configured.block.trim())
        else {
            log::warn!(
                "spawn platform block is unknown and will be ignored: block={}",
                configured.block
            );
            return None;
        };
        Some(Self {
            dimension: dimension.to_string(),
            block_state,
            min_x: configured.min_x.min(configured.max_x),
            max_x: configured.min_x.max(configured.max_x),
            y: configured.y,
            min_z: configured.min_z.min(configured.max_z),
            max_z: configured.min_z.max(configured.max_z),
            template_blocks: Vec::new(),
        })
    }

    fn contains(&self, dimension: &str, position: &qexed_packet::net_types::Position) -> bool {
        dimension.trim() == self.dimension
            && position.y == self.y
            && (self.min_x..=self.max_x).contains(&position.x)
            && (self.min_z..=self.max_z).contains(&position.z)
    }

    fn template_block_state_at(
        &self,
        dimension: &str,
        position: &qexed_packet::net_types::Position,
    ) -> Option<i32> {
        if dimension.trim() != self.dimension || self.template_blocks.is_empty() {
            return None;
        }
        self.template_blocks
            .iter()
            .find(|(x, y, z, _)| *x == position.x && *y == position.y && *z == position.z)
            .and_then(|(_, _, _, block)| chunk_nbt::default_block_state_id_if_known(block))
    }
}

/// Skyblock island template.
/// Produces a 7x7 dirt/grass platform with a central oak tree and a water pool.
fn skyblock_island_template(platform_y: i32) -> Vec<(i32, i32, i32, String)> {
    let y = platform_y;
    let mut blocks = Vec::new();

    // ── Platform layer (y): grass_block perimeter, dirt under tree area ──
    for x in -3..=3i32 {
        for z in -3..=3i32 {
            let block = if x == 0 && (z == 0 || z == 1) {
                // Dirt under tree roots
                "minecraft:dirt"
            } else {
                "minecraft:grass_block"
            };
            blocks.push((x, y, z, block.to_string()));
        }
    }

    // ── Oak tree at (0, 0) ──
    let log = "minecraft:oak_log";
    let leaves = "minecraft:oak_leaves";

    // Trunk: y+1 to y+4
    for ty in 1..=4 {
        blocks.push((0, y + ty, 0, log.to_string()));
    }
    // Canopy layer 1: y+2 expanded ring
    for tx in -2..=2i32 {
        for tz in -2..=2i32 {
            // skip trunk, skip outer corners
            if tx == 0 && tz == 0 { continue; }
            if tx.abs() == 2 && tz.abs() == 2 { continue; }
            blocks.push((tx, y + 2, tz, leaves.to_string()));
        }
    }
    // Canopy layer 2-3: y+3 to y+4
    for ty in 3..=4 {
        for tx in -2..=2i32 {
            for tz in -2..=2i32 {
                if tx == 0 && tz == 0
                    && ty < 4 { continue; } // trunk occupies y+3 center
                if tx.abs() == 2 && tz.abs() == 2 { continue; }
                blocks.push((tx, y + ty, tz, leaves.to_string()));
            }
        }
    }
    // Top: y+5
    for tx in -1..=1i32 {
        for tz in -1..=1i32 {
            if tx == 0 && tz == 0 { continue; }
            blocks.push((tx, y + 5, tz, leaves.to_string()));
        }
    }
    blocks.push((0, y + 5, 0, leaves.to_string()));

    // ── Water pool at (2, 2) ──
    blocks.push((2, y + 1, 2, "minecraft:water".to_string()));

    // ── Dirt patches for farming (dirt + farmland-ready) ──
    blocks.push((-2, y + 1, 0, "minecraft:dirt".to_string()));
    blocks.push((-1, y + 1, 0, "minecraft:dirt".to_string()));

    blocks
}

impl WorldChunkGenerator for SpawnPlatformGenerator {
    fn generate(
        &self,
        dimension: &str,
        chunk_x: i32,
        chunk_z: i32,
        light_algorithm: WorldLightAlgorithm,
    ) -> Result<GeneratedChunk> {
        let generated = self
            .inner
            .generate(dimension, chunk_x, chunk_z, light_algorithm)?;
        let blocks = self.platform_blocks_for_chunk(dimension, chunk_x, chunk_z);
        if blocks.is_empty() {
            return Ok(generated);
        }

        let region_chunk = chunk_nbt::set_block_states_in_region(
            chunk_x,
            chunk_z,
            generated.region_chunk.as_ref(),
            &blocks,
        )?;
        let (packet, light_dampening) = chunk_nbt::network_chunk_and_light_dampening_from_region(
            chunk_x,
            chunk_z,
            &region_chunk,
            light_algorithm,
        )?;
        Ok(GeneratedChunk {
            packet,
            light_dampening,
            region_chunk: Some(region_chunk),
        })
    }

    fn light_dampening(
        &self,
        dimension: &str,
        chunk_x: i32,
        chunk_z: i32,
        light_algorithm: WorldLightAlgorithm,
    ) -> Result<Vec<u8>> {
        if self
            .platform_blocks_for_chunk(dimension, chunk_x, chunk_z)
            .is_empty()
        {
            return self
                .inner
                .light_dampening(dimension, chunk_x, chunk_z, light_algorithm);
        }
        Ok(self
            .generate(dimension, chunk_x, chunk_z, light_algorithm)?
            .light_dampening)
    }

    fn block_state_at(
        &self,
        dimension: &str,
        position: &qexed_packet::net_types::Position,
    ) -> Option<i32> {
        if let Some(block_state) = self.platform.template_block_state_at(dimension, position) {
            return Some(block_state);
        }
        if self.platform.contains(dimension, position) {
            return Some(self.platform.block_state);
        }
        self.inner.block_state_at(dimension, position)
    }

    fn region_chunk(
        &self,
        dimension: &str,
        chunk_x: i32,
        chunk_z: i32,
    ) -> Result<Option<super::region::ChunkData>> {
        let blocks = self.platform_blocks_for_chunk(dimension, chunk_x, chunk_z);
        if blocks.is_empty() {
            return self.inner.region_chunk(dimension, chunk_x, chunk_z);
        }
        let base = self.inner.region_chunk(dimension, chunk_x, chunk_z)?;
        Ok(Some(chunk_nbt::set_block_states_in_region(
            chunk_x,
            chunk_z,
            base.as_ref(),
            &blocks,
        )?))
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
            region_chunk: None,
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

    fn region_chunk(
        &self,
        _dimension: &str,
        _chunk_x: i32,
        _chunk_z: i32,
    ) -> Result<Option<super::region::ChunkData>> {
        Ok(None)
    }
}

#[derive(Debug)]
pub(crate) struct VanillaFlatGenerator {
    layers: Vec<FlatLayer>,
    biome: String,
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
        Self { layers, biome }
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
        let root = flat_chunk_root(chunk_x, chunk_z, &self.layers, &self.biome);
        let (packet, light_dampening) = chunk_nbt::network_chunk_and_light_dampening_from_nbt(
            chunk_x,
            chunk_z,
            &root,
            light_algorithm,
        )?;
        Ok(GeneratedChunk {
            packet,
            light_dampening,
            region_chunk: Some(chunk_nbt::region_chunk_from_nbt(
                chunk_x, chunk_z, &root,
            )?),
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

    fn region_chunk(
        &self,
        _dimension: &str,
        chunk_x: i32,
        chunk_z: i32,
    ) -> Result<Option<super::region::ChunkData>> {
        let root = flat_chunk_root(chunk_x, chunk_z, &self.layers, &self.biome);
        Ok(Some(chunk_nbt::region_chunk_from_nbt(
            chunk_x, chunk_z, &root,
        )?))
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

    fn from_settings(settings: NoiseSettings, _config: &WorldConfig) -> Self {
        Self { settings }
    }

    fn supported_dimension(dimension: &str) -> Option<NoiseDimension> {
        NoiseDimension::from_name(dimension)
    }
}

impl WorldChunkGenerator for VanillaNoiseGenerator {
    fn generate(
        &self,
        dimension: &str,
        chunk_x: i32,
        chunk_z: i32,
        light_algorithm: WorldLightAlgorithm,
    ) -> Result<GeneratedChunk> {
        let Some(dimension) = Self::supported_dimension(dimension) else {
            return Ok(GeneratedChunk {
                packet: empty_chunk_packet(chunk_x, chunk_z, super::WorldLightMode::Static),
                light_dampening: vec![0; CHUNK_DAMPENING_LEN],
                region_chunk: None,
            });
        };
        if dimension != NoiseDimension::Overworld {
            let chunk =
                basic_dimension_chunk(dimension, self.settings.seed(), chunk_x, chunk_z);
            let root = noise_chunk_root(chunk_x, chunk_z, &chunk, dimension.biome());
            let region_chunk = chunk_nbt::region_chunk_from_nbt(chunk_x, chunk_z, &root)?;
            let (mut packet, light_dampening) =
                chunk_nbt::network_chunk_and_light_dampening_from_nbt(
                    chunk_x,
                    chunk_z,
                    &root,
                    light_algorithm,
                )?;
            packet.data.block_entities = chunk.block_entities_as_packet(chunk_x, chunk_z);
            return Ok(GeneratedChunk {
                packet,
                light_dampening,
                region_chunk: Some(region_chunk),
            });
        }

        let total_start = Instant::now();
        let (chunk, timings) = self.settings.generate_chunk_profiled(chunk_x, chunk_z);

        let root_start = Instant::now();
        let mut root = noise_chunk_root(chunk_x, chunk_z, &chunk, self.settings.biome.as_str());
        append_block_entities_to_chunk_root(&mut root, &chunk);
        let root_elapsed = root_start.elapsed();

        let region_chunk = chunk_nbt::region_chunk_from_nbt(chunk_x, chunk_z, &root)?;

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
            region_chunk: Some(region_chunk),
        })
    }

    fn block_state_at(
        &self,
        dimension: &str,
        position: &qexed_packet::net_types::Position,
    ) -> Option<i32> {
        match Self::supported_dimension(dimension)? {
            NoiseDimension::Overworld => self
                .settings
                .block_state_at(position.x, position.y, position.z),
            dimension => basic_dimension_block_state_at(
                dimension,
                self.settings.seed(),
                position.x,
                position.y,
                position.z,
            ),
        }
    }

    fn region_chunk(
        &self,
        dimension: &str,
        chunk_x: i32,
        chunk_z: i32,
    ) -> Result<Option<super::region::ChunkData>> {
        let Some(dimension) = Self::supported_dimension(dimension) else {
            return Ok(None);
        };
        if dimension != NoiseDimension::Overworld {
            let chunk =
                basic_dimension_chunk(dimension, self.settings.seed(), chunk_x, chunk_z);
            let root = noise_chunk_root(chunk_x, chunk_z, &chunk, dimension.biome());
            return Ok(Some(chunk_nbt::region_chunk_from_nbt(
                chunk_x, chunk_z, &root,
            )?));
        }

        let (chunk, _) = self.settings.generate_chunk_profiled(chunk_x, chunk_z);
        let mut root = noise_chunk_root(chunk_x, chunk_z, &chunk, self.settings.biome.as_str());
        append_block_entities_to_chunk_root(&mut root, &chunk);
        Ok(Some(chunk_nbt::region_chunk_from_nbt(
            chunk_x, chunk_z, &root,
        )?))
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
    seed: i64,
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
    feature_source_cache: FeatureSourceCache,
}

impl NoiseSettings {
    fn overworld(seed: i64, noise_kind: vanilla_noise::OverworldNoiseKind) -> Self {
        let surface_rules = vanilla_noise::OverworldSurfaceRules::new(seed);
        let sea_level = 63;
        Self {
            seed,
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
            feature_source_cache: FeatureSourceCache::default(),
        }
    }

    fn seed(&self) -> i64 {
        self.seed
    }

    #[cfg(test)]
    fn generate_chunk(&self, chunk_x: i32, chunk_z: i32) -> NoiseChunkBlocks {
        self.generate_chunk_profiled(chunk_x, chunk_z).0
    }

    fn generate_chunk_profiled(
        &self,
        chunk_x: i32,
        chunk_z: i32,
    ) -> (NoiseChunkBlocks, NoiseChunkTimings) {
        let base_start = Instant::now();
        let (mut chunk, preliminary_surfaces) = self.generate_base_chunk(chunk_x, chunk_z);
        let base = base_start.elapsed();

        let carvers_start = Instant::now();
        self.carvers
            .carve_chunk(self, chunk_x, chunk_z, &preliminary_surfaces, &mut chunk);
        let carvers = carvers_start.elapsed();

        self.feature_source_cache
            .insert_generated(chunk_x, chunk_z, chunk.clone());

        let features_start = Instant::now();
        self.ore_features
            .place_chunk(self, chunk_x, chunk_z, &mut chunk);
        let features = features_start.elapsed();

        let heightmap_start = Instant::now();
        chunk.recompute_first_available_heights(self.min_y, self.height);
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

    fn feature_source_chunk(&self, chunk_x: i32, chunk_z: i32) -> NoiseChunkBlocks {
        self.feature_source_cache
            .get_or_insert_with(chunk_x, chunk_z, || {
                let (mut chunk, preliminary_surfaces) = self.generate_base_chunk(chunk_x, chunk_z);
                self.carvers.carve_chunk(
                    self,
                    chunk_x,
                    chunk_z,
                    &preliminary_surfaces,
                    &mut chunk,
                );
                chunk
            })
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
                let quart_x = chunk_x * 4 + local_x;
                let quart_y = section_y * 4 + local_y;
                let quart_z = chunk_z * 4 + local_z;
                self.density.biome_at_quart(quart_x, quart_y, quart_z)
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
        let water_height = self.water_height_from_density_cache(
            world_x,
            world_z,
            preliminary_surface,
            density_cache,
        );
        for y in self.min_y..self.min_y + self.height {
            blocks.push(self.layer_at_with_density(
                world_x,
                y,
                world_z,
                density_cache.density_at(y, self.min_y),
                surface_height,
                preliminary_surface,
                surface_slope,
                water_height,
            ));
        }
        NoiseColumnBlocks {
            blocks,
            first_available_height: self.first_available_height(surface_height),
        }
    }

    fn block_state_at(&self, x: i32, y: i32, z: i32) -> Option<i32> {
        let layer = self.terrain_layer_at(x, y, z)?;
        (!layer.is_air).then_some(layer.block_state_id)
    }

    fn terrain_layer_at(&self, x: i32, y: i32, z: i32) -> Option<BlockLayer> {
        if !(self.min_y..self.min_y + self.height).contains(&y) {
            return None;
        }
        let profile = self.density.profile(x, z);
        let surface_height = self.surface_height_with_profile(x, z, &profile);
        let preliminary_surface = self.preliminary_surface_with_profile(x, z, &profile);
        let surface_slope = self.surface_slope(x, z, surface_height);
        Some(self.layer_at(
            x,
            y,
            z,
            surface_height,
            preliminary_surface,
            surface_slope,
            self.water_height(x, z, surface_height, preliminary_surface, &profile),
            &profile,
        ))
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
        water_height: Option<i32>,
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
            water_height,
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
        water_height: Option<i32>,
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
                above_water: self.above_water(y, water_height),
                sea_level: self.sea_level,
                min_y: self.min_y,
                biome,
                slope: surface_slope,
            };

            if let Some(block) = self
                .surface_rules
                .block_at_with_preliminary_surface(surface_context, preliminary_surface)
            {
                self.surface_block_layer(block)
            } else if self.surface_rules.is_deepslate(x, y, z) {
                self.ore_vein_at(x, y, z)
                    .unwrap_or_else(|| self.deepslate_block.clone())
            } else {
                self.ore_vein_at(x, y, z)
                    .unwrap_or_else(|| self.default_block.clone())
            }
        } else {
            if self.is_open_surface_water(y, surface_height) {
                return self.default_fluid.clone();
            }

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

    fn above_water(&self, y: i32, water_height: Option<i32>) -> bool {
        water_height.is_none_or(|height| y >= height)
    }

    fn is_open_surface_water(&self, y: i32, surface_height: i32) -> bool {
        y < self.sea_level && y > surface_height
    }

    fn water_height(
        &self,
        x: i32,
        z: i32,
        surface_height: i32,
        preliminary_surface: i32,
        profile: &vanilla_noise::OverworldTerrainProfile,
    ) -> Option<i32> {
        let start = self.first_available_height(surface_height) + self.min_y - 1;
        (self.min_y..=start).rev().find(|y| {
            if self.is_open_surface_water(*y, surface_height) {
                return true;
            }

            let density = self.density.sample_with_profile(x, *y, z, profile);
            if density > 0.0 {
                return false;
            }
            matches!(
                self.aquifer
                    .substance_at(x, *y, z, density, preliminary_surface),
                vanilla_noise::AquiferSubstance::Fluid(vanilla_noise::AquiferFluid::Water)
            )
        })
    }

    fn water_height_from_density_cache(
        &self,
        x: i32,
        z: i32,
        preliminary_surface: i32,
        density_cache: &ColumnDensityCache,
    ) -> Option<i32> {
        let start = density_cache
            .surface_height
            .max(self.sea_level)
            .min(self.min_y + self.height - 1);
        (self.min_y..=start).rev().find(|y| {
            if self.is_open_surface_water(*y, density_cache.surface_height) {
                return true;
            }

            let density = density_cache.density_at(*y, self.min_y);
            if density > 0.0 {
                return false;
            }
            matches!(
                self.aquifer
                    .substance_at(x, *y, z, density, preliminary_surface),
                vanilla_noise::AquiferSubstance::Fluid(vanilla_noise::AquiferFluid::Water)
            )
        })
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
        let height = surface_height.max(self.sea_level - 1) + 1;
        (height - self.min_y).clamp(0, self.height)
    }

    fn air_layer(&self) -> BlockLayer {
        self.air_block.clone()
    }
}

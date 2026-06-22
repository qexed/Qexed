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

fn log_feature_source_cache_snapshot(
    chunk_x: i32,
    chunk_z: i32,
    cache: &FeatureSourceCache,
) {
    if !log::log_enabled!(log::Level::Debug) {
        return;
    }

    let snapshot = cache.snapshot();
    log::debug!(
        "vanilla_noise feature source cache: chunk=({chunk_x}, {chunk_z}), len={}, in_progress={}, hits={}, misses={}, waits={}, evictions={}, generated_inserts={}",
        snapshot.len,
        snapshot.in_progress,
        snapshot.hits,
        snapshot.misses,
        snapshot.waits,
        snapshot.evictions,
        snapshot.generated_inserts
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
            height: WORLD_SECTION_COUNT as i32 * SECTION_HEIGHT,
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

        log_feature_source_cache_snapshot(chunk_x, chunk_z, &self.feature_source_cache);

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

    fn feature_source_chunk(&self, chunk_x: i32, chunk_z: i32) -> Arc<NoiseChunkBlocks> {
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

    fn terrain_ocean_floor_wg_height(&self, x: i32, z: i32) -> i32 {
        let profile = self.density.profile(x, z);
        self.surface_height_with_profile(x, z, &profile) + 1
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

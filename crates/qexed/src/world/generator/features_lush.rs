#[derive(Debug, Clone)]
struct PlacedEnvironmentScanFeature {
    step_index: i32,
    feature_index: i32,
    count: OrePlacementCount,
    height: OreHeight,
    search: EnvironmentScan,
    random_y_offset: i32,
    config: EnvironmentFeatureConfig,
    biome_filter: FeatureBiomeFilter,
}

impl PlacedEnvironmentScanFeature {
    fn lush_caves_ceiling_vegetation(feature_index: i32) -> Self {
        Self {
            step_index: 9,
            feature_index,
            count: OrePlacementCount::Constant(125),
            height: OreHeight::Uniform(HeightAnchor::AboveBottom(0), HeightAnchor::Absolute(256)),
            search: EnvironmentScan::Up { max_steps: 12 },
            random_y_offset: -1,
            config: EnvironmentFeatureConfig::moss_patch_ceiling(),
            biome_filter: FeatureBiomeFilter::Include(LUSH_CAVES_BIOMES),
        }
    }

    fn lush_caves_clay(feature_index: i32) -> Self {
        Self {
            step_index: 9,
            feature_index,
            count: OrePlacementCount::Constant(62),
            height: OreHeight::Uniform(HeightAnchor::AboveBottom(0), HeightAnchor::Absolute(256)),
            search: EnvironmentScan::Down { max_steps: 12 },
            random_y_offset: 1,
            config: EnvironmentFeatureConfig::lush_caves_clay(),
            biome_filter: FeatureBiomeFilter::Include(LUSH_CAVES_BIOMES),
        }
    }

    fn lush_caves_vegetation(feature_index: i32) -> Self {
        Self {
            step_index: 9,
            feature_index,
            count: OrePlacementCount::Constant(125),
            height: OreHeight::Uniform(HeightAnchor::AboveBottom(0), HeightAnchor::Absolute(256)),
            search: EnvironmentScan::Down { max_steps: 12 },
            random_y_offset: 1,
            config: EnvironmentFeatureConfig::moss_patch(),
            biome_filter: FeatureBiomeFilter::Include(LUSH_CAVES_BIOMES),
        }
    }

    fn rooted_azalea_tree(feature_index: i32) -> Self {
        Self {
            step_index: 9,
            feature_index,
            count: OrePlacementCount::Uniform { min: 1, max: 2 },
            height: OreHeight::Uniform(HeightAnchor::AboveBottom(0), HeightAnchor::Absolute(256)),
            search: EnvironmentScan::Up { max_steps: 12 },
            random_y_offset: -1,
            config: EnvironmentFeatureConfig::rooted_azalea_tree(),
            biome_filter: FeatureBiomeFilter::Include(LUSH_CAVES_BIOMES),
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
            let start_y = self.height.sample(settings, random);
            let Some(anchor_y) = self.search.find(settings, origin_x, origin_z, chunk, world_x, start_y, world_z)
            else {
                continue;
            };
            let world_y = anchor_y + self.random_y_offset;
            if !self
                .biome_filter
                .allows_at(&settings.density, world_x, world_y, world_z)
            {
                continue;
            }
            self.config
                .place(settings, origin_x, origin_z, chunk, random, world_x, world_y, world_z);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn place_with_spillover(
        &self,
        settings: &NoiseSettings,
        source_origin_x: i32,
        source_origin_z: i32,
        target_origin_x: i32,
        target_origin_z: i32,
        source_chunk: &mut NoiseChunkBlocks,
        target_chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
    ) {
        for _ in 0..self.count.sample(random) {
            let world_x = source_origin_x + random.next_int(16);
            let world_z = source_origin_z + random.next_int(16);
            let start_y = self.height.sample(settings, random);
            let Some(anchor_y) = self.search.find(
                settings,
                source_origin_x,
                source_origin_z,
                source_chunk,
                world_x,
                start_y,
                world_z,
            ) else {
                continue;
            };
            let world_y = anchor_y + self.random_y_offset;
            if !self
                .biome_filter
                .allows_at(&settings.density, world_x, world_y, world_z)
            {
                continue;
            }

            self.config.place_spillover(
                settings,
                source_origin_x,
                source_origin_z,
                target_origin_x,
                target_origin_z,
                source_chunk,
                target_chunk,
                random,
                world_x,
                world_y,
                world_z,
            );
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum EnvironmentScan {
    Down { max_steps: i32 },
    Up { max_steps: i32 },
}

impl EnvironmentScan {
    #[allow(clippy::too_many_arguments)]
    fn find(
        self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &NoiseChunkBlocks,
        world_x: i32,
        origin_y: i32,
        world_z: i32,
    ) -> Option<i32> {
        match self {
            Self::Down { max_steps } => {
                for step in 0..=max_steps {
                    let air_y = origin_y - step;
                    if !(settings.min_y..settings.min_y + settings.height).contains(&air_y)
                        || air_y - 1 < settings.min_y
                    {
                        return None;
                    }
                    let current = layer_at_world(
                        chunk,
                        chunk_min_x,
                        chunk_min_z,
                        world_x,
                        air_y,
                        world_z,
                        settings.min_y,
                    )?;
                    if !current.is_air {
                        return None;
                    }
                    if is_solid_at_world(
                        chunk,
                        chunk_min_x,
                        chunk_min_z,
                        world_x,
                        air_y - 1,
                        world_z,
                        settings.min_y,
                    ) {
                        return Some(air_y - 1);
                    }
                }
                None
            }
            Self::Up { max_steps } => scan_up_to_solid(
                settings,
                chunk,
                chunk_min_x,
                chunk_min_z,
                world_x,
                origin_y,
                world_z,
                max_steps,
            ),
        }
    }
}

#[derive(Debug, Clone)]
enum EnvironmentFeatureConfig {
    VegetationPatch(VegetationPatchConfig),
    RootedAzaleaTree(RootedAzaleaTreeConfig),
}

impl EnvironmentFeatureConfig {
    fn moss_patch() -> Self {
        Self::VegetationPatch(VegetationPatchConfig::moss_patch())
    }

    fn moss_patch_ceiling() -> Self {
        Self::VegetationPatch(VegetationPatchConfig::moss_patch_ceiling())
    }

    fn lush_caves_clay() -> Self {
        Self::VegetationPatch(VegetationPatchConfig::lush_caves_clay())
    }

    fn rooted_azalea_tree() -> Self {
        Self::RootedAzaleaTree(RootedAzaleaTreeConfig::new())
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
        match self {
            Self::VegetationPatch(config) => config.place(
                settings,
                chunk_min_x,
                chunk_min_z,
                chunk,
                random,
                world_x,
                world_y,
                world_z,
            ),
            Self::RootedAzaleaTree(config) => config.place(
                settings,
                chunk_min_x,
                chunk_min_z,
                chunk,
                random,
                world_x,
                world_y,
                world_z,
            ),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn place_spillover(
        &self,
        settings: &NoiseSettings,
        source_origin_x: i32,
        source_origin_z: i32,
        target_origin_x: i32,
        target_origin_z: i32,
        source_chunk: &mut NoiseChunkBlocks,
        target_chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
        world_x: i32,
        world_y: i32,
        world_z: i32,
    ) -> bool {
        match self {
            Self::VegetationPatch(config) => config.place_spillover(
                settings,
                source_origin_x,
                source_origin_z,
                target_origin_x,
                target_origin_z,
                source_chunk,
                target_chunk,
                random,
                world_x,
                world_y,
                world_z,
            ),
            Self::RootedAzaleaTree(config) => {
                let mut replay_random = random.clone();
                if config.place(
                    settings,
                    source_origin_x,
                    source_origin_z,
                    source_chunk,
                    random,
                    world_x,
                    world_y,
                    world_z,
                ) {
                    config.place_spillover(
                        settings,
                        target_origin_x,
                        target_origin_z,
                        target_chunk,
                        &mut replay_random,
                        world_x,
                        world_y,
                        world_z,
                    )
                } else {
                    false
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
struct VegetationPatchConfig {
    surface: PatchSurface,
    ground: BlockLayer,
    depth: UniformInt,
    vertical_range: i32,
    xz_radius: UniformInt,
    extra_edge_column_chance: f32,
    extra_bottom_block_chance: f32,
    vegetation_chance: f32,
    waterlogged: bool,
    vegetation: PatchVegetation,
    replaceable: PatchReplaceable,
}

impl VegetationPatchConfig {
    fn moss_patch() -> Self {
        Self {
            surface: PatchSurface::Floor,
            ground: BlockLayer::new("minecraft:moss_block"),
            depth: UniformInt { min: 1, max: 1 },
            vertical_range: 5,
            xz_radius: UniformInt { min: 4, max: 7 },
            extra_edge_column_chance: 0.3,
            extra_bottom_block_chance: 0.0,
            vegetation_chance: 0.8,
            waterlogged: false,
            vegetation: PatchVegetation::moss(),
            replaceable: PatchReplaceable::Moss,
        }
    }

    fn moss_patch_ceiling() -> Self {
        Self {
            surface: PatchSurface::Ceiling,
            ground: BlockLayer::new("minecraft:moss_block"),
            depth: UniformInt { min: 1, max: 2 },
            vertical_range: 5,
            xz_radius: UniformInt { min: 4, max: 7 },
            extra_edge_column_chance: 0.3,
            extra_bottom_block_chance: 0.0,
            vegetation_chance: 0.08,
            waterlogged: false,
            vegetation: PatchVegetation::cave_vines_in_moss(),
            replaceable: PatchReplaceable::Moss,
        }
    }

    fn lush_caves_clay() -> Self {
        Self {
            surface: PatchSurface::Floor,
            ground: BlockLayer::new("minecraft:clay"),
            depth: UniformInt { min: 3, max: 3 },
            vertical_range: 5,
            xz_radius: UniformInt { min: 4, max: 7 },
            extra_edge_column_chance: 0.7,
            extra_bottom_block_chance: 0.8,
            vegetation_chance: 0.075,
            waterlogged: true,
            vegetation: PatchVegetation::dripleaf(),
            replaceable: PatchReplaceable::LushGround,
        }
    }

    fn pale_moss_patch() -> Self {
        Self {
            surface: PatchSurface::Floor,
            ground: BlockLayer::new("minecraft:pale_moss_block"),
            depth: UniformInt { min: 1, max: 1 },
            vertical_range: 5,
            xz_radius: UniformInt { min: 2, max: 4 },
            extra_edge_column_chance: 0.75,
            extra_bottom_block_chance: 0.0,
            vegetation_chance: 0.3,
            waterlogged: false,
            vegetation: PatchVegetation::pale_moss(),
            replaceable: PatchReplaceable::Moss,
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
        let radius_x = self.xz_radius.sample(random);
        let radius_z = self.xz_radius.sample(random);
        let mut placed_ground = Vec::new();

        for dx in -radius_x..=radius_x {
            for dz in -radius_z..=radius_z {
                let edge = dx.abs() == radius_x || dz.abs() == radius_z;
                if edge && random.next_float() >= self.extra_edge_column_chance {
                    continue;
                }
                if (dx * dx * radius_z + dz * dz * radius_x) > radius_x * radius_z * 2 {
                    continue;
                }
                let world_x = origin_x + dx;
                let world_z = origin_z + dz;
                if let Some(ground_y) =
                    self.find_surface_y(settings, chunk_min_x, chunk_min_z, chunk, world_x, origin_y, world_z)
                    && self.place_ground_column(
                        settings,
                        chunk_min_x,
                        chunk_min_z,
                        chunk,
                        random,
                        world_x,
                        ground_y,
                        world_z,
                    )
                {
                    placed_ground.push((world_x, ground_y, world_z));
                }
            }
        }

        let mut placed_any = !placed_ground.is_empty();
        for (world_x, ground_y, world_z) in placed_ground {
            if random.next_float() >= self.vegetation_chance {
                continue;
            }
            let vegetation_y = match self.surface {
                PatchSurface::Floor => ground_y + 1,
                PatchSurface::Ceiling => ground_y - 1,
            };
            if self.vegetation.place(
                settings,
                chunk_min_x,
                chunk_min_z,
                chunk,
                random,
                world_x,
                vegetation_y,
                world_z,
            ) {
                placed_any = true;
            }
        }

        placed_any
    }

    #[allow(clippy::too_many_arguments)]
    fn place_spillover(
        &self,
        settings: &NoiseSettings,
        source_origin_x: i32,
        source_origin_z: i32,
        target_origin_x: i32,
        target_origin_z: i32,
        source_chunk: &mut NoiseChunkBlocks,
        target_chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
        origin_x: i32,
        origin_y: i32,
        origin_z: i32,
    ) -> bool {
        let radius_x = self.xz_radius.sample(random);
        let radius_z = self.xz_radius.sample(random);
        let mut placed_ground = Vec::new();

        for dx in -radius_x..=radius_x {
            for dz in -radius_z..=radius_z {
                let edge = dx.abs() == radius_x || dz.abs() == radius_z;
                if edge && random.next_float() >= self.extra_edge_column_chance {
                    continue;
                }
                if (dx * dx * radius_z + dz * dz * radius_x) > radius_x * radius_z * 2 {
                    continue;
                }
                let world_x = origin_x + dx;
                let world_z = origin_z + dz;
                if overlaps_chunk(world_x, world_z, source_origin_x, source_origin_z) {
                    if let Some(ground_y) = self.find_surface_y(
                        settings,
                        source_origin_x,
                        source_origin_z,
                        source_chunk,
                        world_x,
                        origin_y,
                        world_z,
                    ) && self.place_ground_column(
                        settings,
                        source_origin_x,
                        source_origin_z,
                        source_chunk,
                        random,
                        world_x,
                        ground_y,
                        world_z,
                    ) {
                        placed_ground.push((world_x, ground_y, world_z, false));
                    }
                } else if overlaps_chunk(world_x, world_z, target_origin_x, target_origin_z)
                    && let Some(ground_y) = self.find_surface_y(
                        settings,
                        target_origin_x,
                        target_origin_z,
                        target_chunk,
                        world_x,
                        origin_y,
                        world_z,
                    )
                    && self.place_ground_column(
                        settings,
                        target_origin_x,
                        target_origin_z,
                        target_chunk,
                        random,
                        world_x,
                        ground_y,
                        world_z,
                    )
                {
                    placed_ground.push((world_x, ground_y, world_z, true));
                }
            }
        }

        let mut placed_any = !placed_ground.is_empty();
        for (world_x, ground_y, world_z, in_target) in placed_ground {
            if random.next_float() >= self.vegetation_chance {
                continue;
            }
            let vegetation_y = match self.surface {
                PatchSurface::Floor => ground_y + 1,
                PatchSurface::Ceiling => ground_y - 1,
            };
            if in_target {
                if self.vegetation.place(
                    settings,
                    target_origin_x,
                    target_origin_z,
                    target_chunk,
                    random,
                    world_x,
                    vegetation_y,
                    world_z,
                ) {
                    placed_any = true;
                }
            } else if self.vegetation.place(
                settings,
                source_origin_x,
                source_origin_z,
                source_chunk,
                random,
                world_x,
                vegetation_y,
                world_z,
            ) {
                placed_any = true;
            }
        }

        placed_any
    }

    #[allow(clippy::too_many_arguments)]
    fn find_surface_y(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &NoiseChunkBlocks,
        world_x: i32,
        origin_y: i32,
        world_z: i32,
    ) -> Option<i32> {
        for offset in 0..=self.vertical_range {
            for y in [origin_y - offset, origin_y + offset] {
                if !(settings.min_y..settings.min_y + settings.height).contains(&y) {
                    continue;
                }
                let Some(current) =
                    layer_at_world(chunk, chunk_min_x, chunk_min_z, world_x, y, world_z, settings.min_y)
                else {
                    continue;
                };
                if !self.replaceable.matches(current) {
                    continue;
                }
                let open_y = match self.surface {
                    PatchSurface::Floor => y + 1,
                    PatchSurface::Ceiling => y - 1,
                };
                let Some(open) = layer_at_world(
                    chunk,
                    chunk_min_x,
                    chunk_min_z,
                    world_x,
                    open_y,
                    world_z,
                    settings.min_y,
                ) else {
                    continue;
                };
                if open.is_air || (self.waterlogged && open.is("minecraft:water")) {
                    return Some(y);
                }
            }
        }
        None
    }

    #[allow(clippy::too_many_arguments)]
    fn place_ground_column(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
        world_x: i32,
        ground_y: i32,
        world_z: i32,
    ) -> bool {
        let depth = self.depth.sample(random)
            + (random.next_float() < self.extra_bottom_block_chance) as i32;
        let direction = match self.surface {
            PatchSurface::Floor => -1,
            PatchSurface::Ceiling => 1,
        };
        let mut placed = false;
        for step in 0..depth {
            let y = ground_y + direction * step;
            if !(settings.min_y..settings.min_y + settings.height).contains(&y) {
                break;
            }
            let Some((local_x, local_z)) = local_coords(world_x, world_z, chunk_min_x, chunk_min_z)
            else {
                break;
            };
            let Some(current) = chunk.layer(local_x, y, local_z, settings.min_y) else {
                break;
            };
            if !self.replaceable.matches(current) {
                break;
            }
            chunk.set_layer(local_x, y, local_z, settings.min_y, self.ground.clone());
            placed = true;
        }
        placed
    }
}

#[derive(Debug, Clone, Copy)]
enum PatchSurface {
    Floor,
    Ceiling,
}

#[derive(Debug, Clone, Copy)]
enum PatchReplaceable {
    Moss,
    LushGround,
}

impl PatchReplaceable {
    fn matches(self, layer: &BlockLayer) -> bool {
        match self {
            Self::Moss => is_moss_replaceable_layer(layer),
            Self::LushGround => is_lush_ground_replaceable_layer(layer),
        }
    }
}

#[derive(Debug, Clone)]
enum PatchVegetation {
    Simple(SimpleVegetationBlock),
    CaveVinesInMoss(CaveVinesFeatureConfig),
    Dripleaf(DripleafFeatureConfig),
}

impl PatchVegetation {
    fn moss() -> Self {
        Self::Simple(SimpleVegetationBlock::moss_vegetation())
    }

    fn cave_vines_in_moss() -> Self {
        Self::CaveVinesInMoss(CaveVinesFeatureConfig::in_moss())
    }

    fn dripleaf() -> Self {
        Self::Dripleaf(DripleafFeatureConfig::new())
    }

    fn pale_moss() -> Self {
        Self::Simple(SimpleVegetationBlock::pale_moss_vegetation())
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
        match self {
            Self::Simple(block) => block.place_at(
                settings,
                chunk_min_x,
                chunk_min_z,
                chunk,
                random,
                world_x,
                world_y,
                world_z,
            ),
            Self::CaveVinesInMoss(config) => {
                config.place(settings, chunk_min_x, chunk_min_z, chunk, random, world_x, world_y, world_z)
            }
            Self::Dripleaf(config) => {
                config.place(settings, chunk_min_x, chunk_min_z, chunk, random, world_x, world_y, world_z)
            }
        }
    }
}

#[derive(Debug, Clone)]
struct DripleafFeatureConfig {
    small_lower: Vec<BlockLayer>,
    small_upper: Vec<BlockLayer>,
    big_stem: Vec<BlockLayer>,
    big_tip: Vec<BlockLayer>,
    big_stem_height: WeightedHeight,
}

impl DripleafFeatureConfig {
    fn new() -> Self {
        let directions = ["east", "west", "north", "south"];
        Self {
            small_lower: directions
                .iter()
                .map(|facing| {
                    BlockLayer::with_properties(
                        "minecraft:small_dripleaf",
                        &[("facing", facing), ("half", "lower"), ("waterlogged", "false")],
                    )
                })
                .collect(),
            small_upper: directions
                .iter()
                .map(|facing| {
                    BlockLayer::with_properties(
                        "minecraft:small_dripleaf",
                        &[("facing", facing), ("half", "upper"), ("waterlogged", "false")],
                    )
                })
                .collect(),
            big_stem: directions
                .iter()
                .map(|facing| {
                    BlockLayer::with_properties(
                        "minecraft:big_dripleaf_stem",
                        &[("facing", facing), ("waterlogged", "false")],
                    )
                })
                .collect(),
            big_tip: directions
                .iter()
                .map(|facing| {
                    BlockLayer::with_properties(
                        "minecraft:big_dripleaf",
                        &[
                            ("facing", facing),
                            ("tilt", "none"),
                            ("waterlogged", "false"),
                        ],
                    )
                })
                .collect(),
            big_stem_height: WeightedHeight::big_dripleaf_stem(),
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
        if random.next_int(5) == 0 {
            self.place_small(settings, chunk_min_x, chunk_min_z, chunk, random, world_x, world_y, world_z)
        } else {
            self.place_big(settings, chunk_min_x, chunk_min_z, chunk, random, world_x, world_y, world_z)
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn place_small(
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
        let Some((local_x, local_z)) = local_coords(world_x, world_z, chunk_min_x, chunk_min_z)
        else {
            return false;
        };
        if !is_air_or_water_at_world(
            chunk,
            chunk_min_x,
            chunk_min_z,
            world_x,
            world_y,
            world_z,
            settings.min_y,
        ) || !is_air_or_water_at_world(
            chunk,
            chunk_min_x,
            chunk_min_z,
            world_x,
            world_y + 1,
            world_z,
            settings.min_y,
        ) {
            return false;
        }
        let index = random.next_int(self.small_lower.len() as i32) as usize;
        let lower = self.small_lower[index].clone().with_property(
            "waterlogged",
            waterlogged_at_world(chunk, chunk_min_x, chunk_min_z, world_x, world_y, world_z, settings.min_y),
        );
        let upper = self.small_upper[index].clone().with_property(
            "waterlogged",
            waterlogged_at_world(
                chunk,
                chunk_min_x,
                chunk_min_z,
                world_x,
                world_y + 1,
                world_z,
                settings.min_y,
            ),
        );
        chunk.set_layer(local_x, world_y, local_z, settings.min_y, lower);
        chunk.set_layer(local_x, world_y + 1, local_z, settings.min_y, upper);
        true
    }

    #[allow(clippy::too_many_arguments)]
    fn place_big(
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
        let Some((local_x, local_z)) = local_coords(world_x, world_z, chunk_min_x, chunk_min_z)
        else {
            return false;
        };
        let stem_height = self.big_stem_height.sample(random);
        let height = stem_height + 1;
        for dy in 0..height {
            if !is_air_or_water_at_world(
                chunk,
                chunk_min_x,
                chunk_min_z,
                world_x,
                world_y + dy,
                world_z,
                settings.min_y,
            ) {
                return false;
            }
        }

        let index = random.next_int(self.big_stem.len() as i32) as usize;
        for dy in 0..stem_height {
            let block = self.big_stem[index].clone().with_property(
                "waterlogged",
                waterlogged_at_world(
                    chunk,
                    chunk_min_x,
                    chunk_min_z,
                    world_x,
                    world_y + dy,
                    world_z,
                    settings.min_y,
                ),
            );
            chunk.set_layer(local_x, world_y + dy, local_z, settings.min_y, block);
        }
        let tip_y = world_y + stem_height;
        let tip = self.big_tip[index].clone().with_property(
            "waterlogged",
            waterlogged_at_world(chunk, chunk_min_x, chunk_min_z, world_x, tip_y, world_z, settings.min_y),
        );
        chunk.set_layer(local_x, tip_y, local_z, settings.min_y, tip);
        true
    }
}

#[derive(Debug, Clone)]
struct RootedAzaleaTreeConfig {
    tree: OakTreeConfig,
    root: BlockLayer,
    hanging_root: BlockLayer,
    root_radius: i32,
    root_column_max_height: i32,
    root_placement_attempts: i32,
    hanging_root_placement_attempts: i32,
    hanging_root_radius: i32,
    hanging_roots_vertical_span: i32,
}

impl RootedAzaleaTreeConfig {
    fn new() -> Self {
        Self {
            tree: OakTreeConfig::azalea(),
            root: BlockLayer::new("minecraft:rooted_dirt"),
            hanging_root: BlockLayer::with_properties(
                "minecraft:hanging_roots",
                &[("waterlogged", "false")],
            ),
            root_radius: 3,
            root_column_max_height: 100,
            root_placement_attempts: 20,
            hanging_root_placement_attempts: 20,
            hanging_root_radius: 3,
            hanging_roots_vertical_span: 2,
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
        let Some((local_x, local_z)) = local_coords(world_x, world_z, chunk_min_x, chunk_min_z)
        else {
            return false;
        };
        if !chunk
            .layer(local_x, world_y, local_z, settings.min_y)
            .is_some_and(valid_tree_position_layer)
            || !supports_azalea_at_world(
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
        if !self
            .tree
            .place(settings, chunk_min_x, chunk_min_z, chunk, random, world_x, world_y, world_z)
        {
            return false;
        }
        self.place_roots(
            settings,
            chunk_min_x,
            chunk_min_z,
            chunk,
            random,
            world_x,
            world_y - 1,
            world_z,
        );
        self.place_hanging_roots(
            settings,
            chunk_min_x,
            chunk_min_z,
            chunk,
            random,
            world_x,
            world_y,
            world_z,
        );
        true
    }

    #[allow(clippy::too_many_arguments)]
    fn place_spillover(
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
        self.tree.place_spillover(
            settings,
            chunk_min_x,
            chunk_min_z,
            chunk,
            random,
            world_x,
            world_y,
            world_z,
        );
        self.place_roots(
            settings,
            chunk_min_x,
            chunk_min_z,
            chunk,
            random,
            world_x,
            world_y - 1,
            world_z,
        );
        self.place_hanging_roots(
            settings,
            chunk_min_x,
            chunk_min_z,
            chunk,
            random,
            world_x,
            world_y,
            world_z,
        );
        true
    }

    #[allow(clippy::too_many_arguments)]
    fn place_roots(
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
        for dy in 0..self.root_column_max_height {
            let y = origin_y - dy;
            let Some((local_x, local_z)) = local_coords(origin_x, origin_z, chunk_min_x, chunk_min_z)
            else {
                break;
            };
            let Some((replaceable, was_air)) = chunk
                .layer(local_x, y, local_z, settings.min_y)
                .map(|current| (is_azalea_root_replaceable_layer(current), current.is_air))
            else {
                break;
            };
            if !replaceable {
                break;
            }
            chunk.set_layer(local_x, y, local_z, settings.min_y, self.root.clone());
            if was_air {
                break;
            }
        }

        for _ in 0..self.root_placement_attempts {
            let world_x = origin_x + random.next_int(self.root_radius * 2 + 1) - self.root_radius;
            let world_y = origin_y - random.next_int(8);
            let world_z = origin_z + random.next_int(self.root_radius * 2 + 1) - self.root_radius;
            let Some((local_x, local_z)) = local_coords(world_x, world_z, chunk_min_x, chunk_min_z)
            else {
                continue;
            };
            if chunk
                .layer(local_x, world_y, local_z, settings.min_y)
                .is_some_and(is_azalea_root_replaceable_layer)
            {
                chunk.set_layer(local_x, world_y, local_z, settings.min_y, self.root.clone());
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn place_hanging_roots(
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
        for _ in 0..self.hanging_root_placement_attempts {
            let world_x = origin_x + random.next_int(self.hanging_root_radius * 2 + 1)
                - self.hanging_root_radius;
            let world_y = origin_y
                - random.next_int(self.hanging_roots_vertical_span * 2 + 1)
                - 1;
            let world_z = origin_z + random.next_int(self.hanging_root_radius * 2 + 1)
                - self.hanging_root_radius;
            let Some((local_x, local_z)) = local_coords(world_x, world_z, chunk_min_x, chunk_min_z)
            else {
                continue;
            };
            if is_air_at_world(
                chunk,
                chunk_min_x,
                chunk_min_z,
                world_x,
                world_y,
                world_z,
                settings.min_y,
            ) && is_solid_at_world(
                chunk,
                chunk_min_x,
                chunk_min_z,
                world_x,
                world_y + 1,
                world_z,
                settings.min_y,
            ) {
                chunk.set_layer(
                    local_x,
                    world_y,
                    local_z,
                    settings.min_y,
                    self.hanging_root.clone(),
                );
            }
        }
    }
}

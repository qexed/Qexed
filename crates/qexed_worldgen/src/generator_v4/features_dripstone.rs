#[derive(Debug, Clone)]
enum PlacedDripstoneFeature {
    Large(PlacedLargeDripstoneFeature),
    Cluster(PlacedDripstoneClusterFeature),
    Pointed(PlacedPointedDripstoneFeature),
}

impl PlacedDripstoneFeature {
    fn large(feature_index: i32) -> Self {
        Self::Large(PlacedLargeDripstoneFeature::new(feature_index))
    }

    fn cluster(feature_index: i32) -> Self {
        Self::Cluster(PlacedDripstoneClusterFeature::new(feature_index))
    }

    fn pointed(feature_index: i32) -> Self {
        Self::Pointed(PlacedPointedDripstoneFeature::new(feature_index))
    }

    fn step_index(&self) -> i32 {
        match self {
            Self::Large(feature) => feature.step_index,
            Self::Cluster(feature) => feature.step_index,
            Self::Pointed(feature) => feature.step_index,
        }
    }

    fn feature_index(&self) -> i32 {
        match self {
            Self::Large(feature) => feature.feature_index,
            Self::Cluster(feature) => feature.feature_index,
            Self::Pointed(feature) => feature.feature_index,
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
        match self {
            Self::Large(feature) => feature.place(settings, origin_x, origin_z, chunk, random),
            Self::Cluster(feature) => feature.place(settings, origin_x, origin_z, chunk, random),
            Self::Pointed(feature) => feature.place(settings, origin_x, origin_z, chunk, random),
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
        match self {
            Self::Pointed(feature) => feature.place_with_spillover(
                settings,
                source_origin_x,
                source_origin_z,
                target_origin_x,
                target_origin_z,
                source_chunk,
                target_chunk,
                random,
            ),
            Self::Large(feature) => feature.place_with_spillover(
                settings,
                source_origin_x,
                source_origin_z,
                target_origin_x,
                target_origin_z,
                source_chunk,
                target_chunk,
                random,
            ),
            Self::Cluster(feature) => feature.place_with_spillover(
                settings,
                source_origin_x,
                source_origin_z,
                target_origin_x,
                target_origin_z,
                source_chunk,
                target_chunk,
                random,
            ),
        }
    }

    fn may_spill_into(
        &self,
        settings: &NoiseSettings,
        source_origin_x: i32,
        source_origin_z: i32,
        target_origin_x: i32,
        target_origin_z: i32,
        random: &mut FeatureRandom,
    ) -> bool {
        match self {
            Self::Large(feature) => feature.may_spill_into(
                settings,
                source_origin_x,
                source_origin_z,
                target_origin_x,
                target_origin_z,
                random,
            ),
            Self::Cluster(_) | Self::Pointed(_) => true,
        }
    }
}

#[derive(Debug, Clone)]
struct PlacedLargeDripstoneFeature {
    step_index: i32,
    feature_index: i32,
    count: OrePlacementCount,
    height: OreHeight,
    config: LargeDripstoneFeatureConfig,
    biome_filter: FeatureBiomeFilter,
}

impl PlacedLargeDripstoneFeature {
    fn new(feature_index: i32) -> Self {
        Self {
            step_index: 2,
            feature_index,
            count: OrePlacementCount::Uniform { min: 10, max: 48 },
            height: OreHeight::Uniform(HeightAnchor::AboveBottom(0), HeightAnchor::Absolute(256)),
            config: LargeDripstoneFeatureConfig::new(),
            biome_filter: FeatureBiomeFilter::Include(DRIPSTONE_CAVES_BIOMES),
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
        let count = self.count.sample(random);
        for _ in 0..count {
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
        let count = self.count.sample(random);
        for _ in 0..count {
            let world_x = source_origin_x + random.next_int(16);
            let world_z = source_origin_z + random.next_int(16);
            let world_y = self.height.sample(settings, random);
            if !self
                .biome_filter
                .allows_at(&settings.density, world_x, world_y, world_z)
            {
                continue;
            }
            self.config.place_with_spillover(
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

    fn may_spill_into(
        &self,
        settings: &NoiseSettings,
        source_origin_x: i32,
        source_origin_z: i32,
        target_origin_x: i32,
        target_origin_z: i32,
        random: &mut FeatureRandom,
    ) -> bool {
        for _ in 0..self.count.sample(random) {
            let world_x = source_origin_x + random.next_int(16);
            let world_z = source_origin_z + random.next_int(16);
            let world_y = self.height.sample(settings, random);
            if !self
                .biome_filter
                .allows_at(&settings.density, world_x, world_y, world_z)
            {
                continue;
            }
            if horizontal_box_reaches_chunk(
                world_x - self.config.column_radius.max,
                world_x + self.config.column_radius.max,
                world_z - self.config.column_radius.max,
                world_z + self.config.column_radius.max,
                target_origin_x,
                target_origin_z,
            ) {
                return true;
            }
        }
        false
    }
}

#[derive(Debug, Clone)]
struct PlacedDripstoneClusterFeature {
    step_index: i32,
    feature_index: i32,
    count: OrePlacementCount,
    height: OreHeight,
    config: DripstoneClusterFeatureConfig,
    biome_filter: FeatureBiomeFilter,
}

impl PlacedDripstoneClusterFeature {
    fn new(feature_index: i32) -> Self {
        Self {
            step_index: 7,
            feature_index,
            count: OrePlacementCount::Uniform { min: 48, max: 96 },
            height: OreHeight::Uniform(HeightAnchor::AboveBottom(0), HeightAnchor::Absolute(256)),
            config: DripstoneClusterFeatureConfig::new(),
            biome_filter: FeatureBiomeFilter::Include(DRIPSTONE_CAVES_BIOMES),
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
            let world_y = self.height.sample(settings, random);
            if !self
                .biome_filter
                .allows_at(&settings.density, world_x, world_y, world_z)
            {
                continue;
            }
            self.config.place_with_spillover(
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

#[derive(Debug, Clone)]
struct PlacedPointedDripstoneFeature {
    step_index: i32,
    feature_index: i32,
    outer_count: OrePlacementCount,
    height: OreHeight,
    inner_count: OrePlacementCount,
    xz_offset: ClampedNormalInt,
    y_offset: ClampedNormalInt,
    config: PointedDripstoneFeatureConfig,
    biome_filter: FeatureBiomeFilter,
}

impl PlacedPointedDripstoneFeature {
    fn new(feature_index: i32) -> Self {
        Self {
            step_index: 7,
            feature_index,
            outer_count: OrePlacementCount::Uniform { min: 192, max: 256 },
            height: OreHeight::Uniform(HeightAnchor::AboveBottom(0), HeightAnchor::Absolute(256)),
            inner_count: OrePlacementCount::Uniform { min: 1, max: 5 },
            xz_offset: ClampedNormalInt {
                mean: 0.0,
                deviation: 3.0,
                min: -10,
                max: 10,
            },
            y_offset: ClampedNormalInt {
                mean: 0.0,
                deviation: 0.6,
                min: -2,
                max: 2,
            },
            config: PointedDripstoneFeatureConfig::new(),
            biome_filter: FeatureBiomeFilter::Include(DRIPSTONE_CAVES_BIOMES),
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
        for _ in 0..self.outer_count.sample(random) {
            let base_x = origin_x + random.next_int(16);
            let base_z = origin_z + random.next_int(16);
            let base_y = self.height.sample(settings, random);
            for _ in 0..self.inner_count.sample(random) {
                let world_x = base_x + self.xz_offset.sample(random);
                let world_y = base_y + self.y_offset.sample(random);
                let world_z = base_z + self.xz_offset.sample(random);
                if !self
                    .biome_filter
                    .allows_at(&settings.density, world_x, world_y, world_z)
                {
                    continue;
                }
                self.place_selected(
                    settings, origin_x, origin_z, chunk, random, world_x, world_y, world_z,
                );
            }
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
        for _ in 0..self.outer_count.sample(random) {
            let base_x = source_origin_x + random.next_int(16);
            let base_z = source_origin_z + random.next_int(16);
            let base_y = self.height.sample(settings, random);
            for _ in 0..self.inner_count.sample(random) {
                let world_x = base_x + self.xz_offset.sample(random);
                let world_y = base_y + self.y_offset.sample(random);
                let world_z = base_z + self.xz_offset.sample(random);
                if !self
                    .biome_filter
                    .allows_at(&settings.density, world_x, world_y, world_z)
                {
                    continue;
                }

                let candidate_random = random.clone();
                self.place_selected(
                    settings,
                    source_origin_x,
                    source_origin_z,
                    source_chunk,
                    random,
                    world_x,
                    world_y,
                    world_z,
                );

                let mut replay_random = candidate_random;
                self.place_selected_spillover_target(
                    settings,
                    source_origin_x,
                    source_origin_z,
                    source_chunk,
                    target_origin_x,
                    target_origin_z,
                    target_chunk,
                    &mut replay_random,
                    world_x,
                    world_y,
                    world_z,
                );
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn place_selected(
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
        let direction = if random.next_bool() {
            DripstoneDirection::Up
        } else {
            DripstoneDirection::Down
        };
        let Some(target_y) = scan_air_or_water_to_solid(
            settings,
            chunk,
            chunk_min_x,
            chunk_min_z,
            world_x,
            world_y,
            world_z,
            direction.scan_sign(),
            12,
        ) else {
            return false;
        };
        let placement_y = target_y - direction.scan_sign();
        self.config.place_at(
            settings,
            chunk_min_x,
            chunk_min_z,
            chunk,
            random,
            world_x,
            placement_y,
            world_z,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn place_selected_spillover_target(
        &self,
        settings: &NoiseSettings,
        source_min_x: i32,
        source_min_z: i32,
        source_chunk: &NoiseChunkBlocks,
        target_min_x: i32,
        target_min_z: i32,
        target_chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
        world_x: i32,
        world_y: i32,
        world_z: i32,
    ) -> bool {
        let direction = if random.next_bool() {
            DripstoneDirection::Up
        } else {
            DripstoneDirection::Down
        };
        let Some(target_y) = scan_air_or_water_to_solid_in_context(
            settings,
            source_min_x,
            source_min_z,
            source_chunk,
            target_min_x,
            target_min_z,
            target_chunk,
            world_x,
            world_y,
            world_z,
            direction.scan_sign(),
            12,
        ) else {
            return false;
        };
        let placement_y = target_y - direction.scan_sign();
        self.config.place_at_spillover_target(
            settings,
            source_min_x,
            source_min_z,
            source_chunk,
            target_min_x,
            target_min_z,
            target_chunk,
            random,
            world_x,
            placement_y,
            world_z,
        )
    }
}

#[derive(Debug, Clone)]
struct LargeDripstoneFeatureConfig {
    column_radius: UniformInt,
    floor_to_ceiling_search_range: i32,
    height_scale: FeatureUniformFloat,
    max_column_radius_to_cave_height_ratio: f32,
    stalactite_bluntness: FeatureUniformFloat,
    stalagmite_bluntness: FeatureUniformFloat,
}

#[derive(Debug, Clone, Copy)]
struct LargeDripstoneResolved {
    floor_y: i32,
    ceiling_y: i32,
    radius: i32,
    scale: f64,
    stalactite_bluntness: f64,
    stalagmite_bluntness: f64,
}

impl LargeDripstoneFeatureConfig {
    fn new() -> Self {
        Self {
            column_radius: UniformInt { min: 3, max: 19 },
            floor_to_ceiling_search_range: 30,
            height_scale: FeatureUniformFloat { min: 0.4, max: 2.0 },
            max_column_radius_to_cave_height_ratio: 0.33,
            stalactite_bluntness: FeatureUniformFloat { min: 0.3, max: 0.9 },
            stalagmite_bluntness: FeatureUniformFloat { min: 0.4, max: 1.0 },
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
        if !is_empty_or_water_at_world(
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

        let Some(column) = scan_dripstone_column(
            settings,
            chunk,
            chunk_min_x,
            chunk_min_z,
            world_x,
            world_y,
            world_z,
            self.floor_to_ceiling_search_range,
            is_dripstone_base_or_lava_layer,
        ) else {
            return false;
        };
        let (Some(floor_y), Some(ceiling_y)) = (column.floor, column.ceiling) else {
            return false;
        };
        let Some(resolved) = self.sample_resolved(random, floor_y, ceiling_y) else {
            return false;
        };

        self.place_resolved(
            settings,
            chunk_min_x,
            chunk_min_z,
            chunk,
            random,
            world_x,
            world_z,
            resolved,
        )
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
        world_x: i32,
        world_y: i32,
        world_z: i32,
    ) {
        if !is_empty_or_water_at_world(
            source_chunk,
            source_origin_x,
            source_origin_z,
            world_x,
            world_y,
            world_z,
            settings.min_y,
        ) {
            return;
        }

        let Some(column) = scan_dripstone_column(
            settings,
            source_chunk,
            source_origin_x,
            source_origin_z,
            world_x,
            world_y,
            world_z,
            self.floor_to_ceiling_search_range,
            is_dripstone_base_or_lava_layer,
        ) else {
            return;
        };
        let (Some(floor_y), Some(ceiling_y)) = (column.floor, column.ceiling) else {
            return;
        };
        let Some(resolved) = self.sample_resolved(random, floor_y, ceiling_y) else {
            return;
        };

        let mut replay_random = random.clone();
        self.place_resolved(
            settings,
            source_origin_x,
            source_origin_z,
            source_chunk,
            random,
            world_x,
            world_z,
            resolved,
        );
        self.place_resolved(
            settings,
            target_origin_x,
            target_origin_z,
            target_chunk,
            &mut replay_random,
            world_x,
            world_z,
            resolved,
        );
    }

    fn sample_resolved(
        &self,
        random: &mut FeatureRandom,
        floor_y: i32,
        ceiling_y: i32,
    ) -> Option<LargeDripstoneResolved> {
        let cave_height = ceiling_y - floor_y - 1;
        if cave_height < 4 {
            return None;
        }
        let radius_limit = ((cave_height as f32) * self.max_column_radius_to_cave_height_ratio)
            .floor() as i32;
        let max_radius = radius_limit.clamp(self.column_radius.min, self.column_radius.max);
        let radius =
            self.column_radius.min + random.next_int(max_radius - self.column_radius.min + 1);
        Some(LargeDripstoneResolved {
            floor_y,
            ceiling_y,
            radius,
            scale: self.height_scale.sample(random) as f64,
            stalactite_bluntness: self.stalactite_bluntness.sample(random) as f64,
            stalagmite_bluntness: self.stalagmite_bluntness.sample(random) as f64,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn place_resolved(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
        world_x: i32,
        world_z: i32,
        resolved: LargeDripstoneResolved,
    ) -> bool {
        let stalactite = LargeDripstoneCone {
            root_y: resolved.ceiling_y - 1,
            pointing_up: false,
            radius: resolved.radius,
            max_height: resolved.ceiling_y - resolved.floor_y - 1,
            bluntness: resolved.stalactite_bluntness,
            scale: resolved.scale,
        };
        let stalagmite = LargeDripstoneCone {
            root_y: resolved.floor_y + 1,
            pointing_up: true,
            radius: resolved.radius,
            max_height: resolved.ceiling_y - resolved.floor_y - 1,
            bluntness: resolved.stalagmite_bluntness,
            scale: resolved.scale,
        };

        let placed_stalactite = stalactite.place_blocks(
            settings,
            chunk_min_x,
            chunk_min_z,
            chunk,
            random,
            world_x,
            world_z,
        );
        let placed_stalagmite = stalagmite.place_blocks(
            settings,
            chunk_min_x,
            chunk_min_z,
            chunk,
            random,
            world_x,
            world_z,
        );
        placed_stalactite || placed_stalagmite
    }
}

#[derive(Debug, Clone)]
struct DripstoneClusterFeatureConfig {
    chance_of_dripstone_column_at_max_distance_from_center: f64,
    density: FeatureUniformFloat,
    dripstone_block_layer_thickness: UniformInt,
    floor_to_ceiling_search_range: i32,
    height: UniformInt,
    height_deviation: f64,
    max_distance_from_center_affecting_height_bias: i32,
    max_distance_from_edge_affecting_chance_of_dripstone_column: i32,
    max_stalagmite_stalactite_height_diff: i32,
    radius: UniformInt,
    wetness: ClampedNormalFloat,
}

#[derive(Debug, Clone, Copy)]
struct DripstoneClusterResolved {
    cluster_height: i32,
    wetness: f32,
    density: f32,
    x_radius: i32,
    z_radius: i32,
}

impl DripstoneClusterFeatureConfig {
    fn new() -> Self {
        Self {
            chance_of_dripstone_column_at_max_distance_from_center: 0.1,
            density: FeatureUniformFloat { min: 0.3, max: 0.7 },
            dripstone_block_layer_thickness: UniformInt { min: 2, max: 4 },
            floor_to_ceiling_search_range: 12,
            height: UniformInt { min: 3, max: 6 },
            height_deviation: 3.0,
            max_distance_from_center_affecting_height_bias: 8,
            max_distance_from_edge_affecting_chance_of_dripstone_column: 3,
            max_stalagmite_stalactite_height_diff: 1,
            radius: UniformInt { min: 2, max: 8 },
            wetness: ClampedNormalFloat {
                mean: 0.1,
                deviation: 0.3,
                min: 0.1,
                max: 0.9,
            },
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
        if !is_empty_or_water_at_world(
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

        let resolved = self.sample_resolved(random);
        self.place_resolved(
            settings,
            chunk_min_x,
            chunk_min_z,
            chunk,
            random,
            world_x,
            world_y,
            world_z,
            resolved,
        )
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
        world_x: i32,
        world_y: i32,
        world_z: i32,
    ) {
        if !is_empty_or_water_at_world(
            source_chunk,
            source_origin_x,
            source_origin_z,
            world_x,
            world_y,
            world_z,
            settings.min_y,
        ) {
            return;
        }

        let resolved = self.sample_resolved(random);
        for dx in -resolved.x_radius..=resolved.x_radius {
            for dz in -resolved.z_radius..=resolved.z_radius {
                let chance = self.chance_of_stalagmite_or_stalactite(
                    resolved.x_radius,
                    resolved.z_radius,
                    dx,
                    dz,
                );
                let column_random = random.clone();
                let mut source_random = column_random.clone();
                self.place_column(
                    settings,
                    source_origin_x,
                    source_origin_z,
                    source_chunk,
                    &mut source_random,
                    world_x + dx,
                    world_y,
                    world_z + dz,
                    dx,
                    dz,
                    resolved.wetness,
                    chance,
                    resolved.cluster_height,
                    resolved.density,
                );

                let mut replay_random = column_random;
                self.place_column(
                    settings,
                    target_origin_x,
                    target_origin_z,
                    target_chunk,
                    &mut replay_random,
                    world_x + dx,
                    world_y,
                    world_z + dz,
                    dx,
                    dz,
                    resolved.wetness,
                    chance,
                    resolved.cluster_height,
                    resolved.density,
                );
                *random = source_random;
            }
        }
    }

    fn sample_resolved(&self, random: &mut FeatureRandom) -> DripstoneClusterResolved {
        DripstoneClusterResolved {
            cluster_height: self.height.sample(random),
            wetness: self.wetness.sample(random),
            density: self.density.sample(random),
            x_radius: self.radius.sample(random),
            z_radius: self.radius.sample(random),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn place_resolved(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
        world_x: i32,
        world_y: i32,
        world_z: i32,
        resolved: DripstoneClusterResolved,
    ) -> bool {
        let mut placed = false;

        for dx in -resolved.x_radius..=resolved.x_radius {
            for dz in -resolved.z_radius..=resolved.z_radius {
                let chance = self.chance_of_stalagmite_or_stalactite(
                    resolved.x_radius,
                    resolved.z_radius,
                    dx,
                    dz,
                );
                placed |= self.place_column(
                    settings,
                    chunk_min_x,
                    chunk_min_z,
                    chunk,
                    random,
                    world_x + dx,
                    world_y,
                    world_z + dz,
                    dx,
                    dz,
                    resolved.wetness,
                    chance,
                    resolved.cluster_height,
                    resolved.density,
                );
            }
        }

        placed
    }

    #[allow(clippy::too_many_arguments)]
    fn place_column(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
        world_x: i32,
        world_y: i32,
        world_z: i32,
        dx: i32,
        dz: i32,
        chance_of_water: f32,
        chance_of_column: f64,
        cluster_height: i32,
        density: f32,
    ) -> bool {
        let Some(mut column) = scan_dripstone_column(
            settings,
            chunk,
            chunk_min_x,
            chunk_min_z,
            world_x,
            world_y,
            world_z,
            self.floor_to_ceiling_search_range,
            |layer| !is_air_or_water_layer(layer),
        ) else {
            return false;
        };
        if column.ceiling.is_none() && column.floor.is_none() {
            return false;
        }

        if random.next_float() < chance_of_water {
            if let Some(floor_y) = column.floor {
                if self.can_place_pool(settings, chunk, chunk_min_x, chunk_min_z, world_x, floor_y, world_z) {
                    if let Some((local_x, local_z)) =
                        local_coords(world_x, world_z, chunk_min_x, chunk_min_z)
                    {
                        chunk.set_layer(
                            local_x,
                            floor_y,
                            local_z,
                            settings.min_y,
                            BlockLayer::new("minecraft:water"),
                        );
                        column.floor = Some(floor_y - 1);
                    }
                }
            }
        }

        let mut stalactite_height = 0;
        if let Some(ceiling_y) = column.ceiling {
            if random.next_double() < chance_of_column
                && !is_lava_at_world(
                    chunk,
                    chunk_min_x,
                    chunk_min_z,
                    world_x,
                    ceiling_y,
                    world_z,
                    settings.min_y,
                )
            {
                self.replace_blocks_with_dripstone_blocks(
                    settings,
                    chunk_min_x,
                    chunk_min_z,
                    chunk,
                    world_x,
                    ceiling_y,
                    world_z,
                    self.dripstone_block_layer_thickness.sample(random),
                    DripstoneDirection::Up,
                );
                let max_height = column
                    .floor
                    .map_or(cluster_height, |floor_y| cluster_height.min(ceiling_y - floor_y));
                stalactite_height =
                    self.dripstone_height(random, dx, dz, density, max_height.max(0));
            }
        }

        let mut stalagmite_height = 0;
        if let Some(floor_y) = column.floor {
            if random.next_double() < chance_of_column
                && !is_lava_at_world(
                    chunk,
                    chunk_min_x,
                    chunk_min_z,
                    world_x,
                    floor_y,
                    world_z,
                    settings.min_y,
                )
            {
                self.replace_blocks_with_dripstone_blocks(
                    settings,
                    chunk_min_x,
                    chunk_min_z,
                    chunk,
                    world_x,
                    floor_y,
                    world_z,
                    self.dripstone_block_layer_thickness.sample(random),
                    DripstoneDirection::Down,
                );
                stalagmite_height = if column.ceiling.is_some() {
                    let diff = random_between_inclusive(
                        random,
                        -self.max_stalagmite_stalactite_height_diff,
                        self.max_stalagmite_stalactite_height_diff,
                    );
                    (stalactite_height + diff).max(0)
                } else {
                    self.dripstone_height(random, dx, dz, density, cluster_height)
                };
            }
        }

        let (actual_stalactite_height, actual_stalagmite_height) =
            self.trim_overlapping_tips(random, column, stalactite_height, stalagmite_height);
        let merge_tips = random.next_bool()
            && actual_stalactite_height > 0
            && actual_stalagmite_height > 0
            && column
                .height()
                .is_some_and(|height| actual_stalactite_height + actual_stalagmite_height == height);

        let mut placed = false;
        if let Some(ceiling_y) = column.ceiling {
            placed |= grow_pointed_dripstone(
                settings,
                chunk_min_x,
                chunk_min_z,
                chunk,
                world_x,
                ceiling_y - 1,
                world_z,
                DripstoneDirection::Down,
                actual_stalactite_height,
                merge_tips,
            );
        }
        if let Some(floor_y) = column.floor {
            placed |= grow_pointed_dripstone(
                settings,
                chunk_min_x,
                chunk_min_z,
                chunk,
                world_x,
                floor_y + 1,
                world_z,
                DripstoneDirection::Up,
                actual_stalagmite_height,
                merge_tips,
            );
        }
        placed
    }

    #[allow(clippy::too_many_arguments)]
    fn can_place_pool(
        &self,
        settings: &NoiseSettings,
        chunk: &NoiseChunkBlocks,
        chunk_min_x: i32,
        chunk_min_z: i32,
        world_x: i32,
        world_y: i32,
        world_z: i32,
    ) -> bool {
        let Some(state) = layer_at_world(
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
        if state.is("minecraft:water")
            || state.is("minecraft:dripstone_block")
            || state.is("minecraft:pointed_dripstone")
        {
            return false;
        }
        if is_water_at_world(
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
        for (dx, dz) in horizontal_directions() {
            if !can_dripstone_pool_touch_water(
                chunk,
                chunk_min_x,
                chunk_min_z,
                world_x + dx,
                world_y,
                world_z + dz,
                settings.min_y,
            ) {
                return false;
            }
        }
        can_dripstone_pool_touch_water(
            chunk,
            chunk_min_x,
            chunk_min_z,
            world_x,
            world_y - 1,
            world_z,
            settings.min_y,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn replace_blocks_with_dripstone_blocks(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        world_x: i32,
        world_y: i32,
        world_z: i32,
        max_count: i32,
        direction: DripstoneDirection,
    ) {
        for step in 0..max_count {
            let y = world_y + direction.dy() * step;
            if !place_dripstone_block_if_possible(
                settings,
                chunk_min_x,
                chunk_min_z,
                chunk,
                world_x,
                y,
                world_z,
            ) {
                return;
            }
        }
    }

    fn dripstone_height(
        &self,
        random: &mut FeatureRandom,
        dx: i32,
        dz: i32,
        density: f32,
        max_height: i32,
    ) -> i32 {
        if max_height <= 0 || random.next_float() > density {
            return 0;
        }

        let distance_from_center = dx.abs() + dz.abs();
        let height_mean = clamped_map_f64(
            distance_from_center as f64,
            0.0,
            self.max_distance_from_center_affecting_height_bias as f64,
            max_height as f64 / 2.0,
            0.0,
        );
        sample_clamped_normal(random, height_mean, self.height_deviation, 0.0, max_height as f64)
            .floor() as i32
    }

    fn chance_of_stalagmite_or_stalactite(
        &self,
        x_radius: i32,
        z_radius: i32,
        dx: i32,
        dz: i32,
    ) -> f64 {
        let x_distance_from_edge = x_radius - dx.abs();
        let z_distance_from_edge = z_radius - dz.abs();
        let distance_from_edge = x_distance_from_edge.min(z_distance_from_edge);
        clamped_map_f64(
            distance_from_edge as f64,
            0.0,
            self.max_distance_from_edge_affecting_chance_of_dripstone_column as f64,
            self.chance_of_dripstone_column_at_max_distance_from_center,
            1.0,
        )
    }

    fn trim_overlapping_tips(
        &self,
        random: &mut FeatureRandom,
        column: DripstoneColumn,
        stalactite_height: i32,
        stalagmite_height: i32,
    ) -> (i32, i32) {
        let (Some(floor_y), Some(ceiling_y)) = (column.floor, column.ceiling) else {
            return (stalactite_height, stalagmite_height);
        };
        if ceiling_y - stalactite_height > floor_y + stalagmite_height {
            return (stalactite_height, stalagmite_height);
        }

        let lowest_stalactite_bottom = (ceiling_y - stalactite_height).max(floor_y + 1);
        let highest_stalagmite_top = (floor_y + stalagmite_height).min(ceiling_y - 1);
        if lowest_stalactite_bottom > highest_stalagmite_top + 1 {
            return (stalactite_height, stalagmite_height);
        }
        let actual_stalactite_bottom = random_between_inclusive(
            random,
            lowest_stalactite_bottom,
            highest_stalagmite_top + 1,
        );
        let actual_stalagmite_top = actual_stalactite_bottom - 1;
        (
            ceiling_y - actual_stalactite_bottom,
            actual_stalagmite_top - floor_y,
        )
    }
}

#[derive(Debug, Clone)]
struct PointedDripstoneFeatureConfig {
    chance_of_directional_spread: f32,
    chance_of_spread_radius2: f32,
    chance_of_spread_radius3: f32,
    chance_of_taller_dripstone: f32,
}

impl PointedDripstoneFeatureConfig {
    fn new() -> Self {
        Self {
            chance_of_directional_spread: 0.7,
            chance_of_spread_radius2: 0.5,
            chance_of_spread_radius3: 0.5,
            chance_of_taller_dripstone: 0.2,
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
        if !is_air_or_water_at_world(
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

        let can_place_above = is_dripstone_base_at_world(
            chunk,
            chunk_min_x,
            chunk_min_z,
            world_x,
            world_y + 1,
            world_z,
            settings.min_y,
        );
        let can_place_below = is_dripstone_base_at_world(
            chunk,
            chunk_min_x,
            chunk_min_z,
            world_x,
            world_y - 1,
            world_z,
            settings.min_y,
        );
        let Some(tip_direction) =
            dripstone_tip_direction(can_place_above, can_place_below, random)
        else {
            return false;
        };

        let root_y = world_y - tip_direction.dy();
        self.create_patch_of_dripstone_blocks(
            settings,
            chunk_min_x,
            chunk_min_z,
            chunk,
            random,
            world_x,
            root_y,
            world_z,
        );
        let height = if random.next_float() < self.chance_of_taller_dripstone
            && is_air_or_water_at_world(
                chunk,
                chunk_min_x,
                chunk_min_z,
                world_x,
                world_y + tip_direction.dy(),
                world_z,
                settings.min_y,
            ) {
            2
        } else {
            1
        };

        grow_pointed_dripstone(
            settings,
            chunk_min_x,
            chunk_min_z,
            chunk,
            world_x,
            world_y,
            world_z,
            tip_direction,
            height,
            false,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn create_patch_of_dripstone_blocks(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
        world_x: i32,
        world_y: i32,
        world_z: i32,
    ) {
        place_dripstone_block_if_possible(
            settings,
            chunk_min_x,
            chunk_min_z,
            chunk,
            world_x,
            world_y,
            world_z,
        );

        for (dx, dz) in horizontal_directions() {
            if random.next_float() > self.chance_of_directional_spread {
                continue;
            }
            let pos1 = (world_x + dx, world_y, world_z + dz);
            place_dripstone_block_if_possible(
                settings,
                chunk_min_x,
                chunk_min_z,
                chunk,
                pos1.0,
                pos1.1,
                pos1.2,
            );
            if random.next_float() > self.chance_of_spread_radius2 {
                continue;
            }
            let spread2 = random_direction_offset(random);
            let pos2 = (pos1.0 + spread2.0, pos1.1 + spread2.1, pos1.2 + spread2.2);
            place_dripstone_block_if_possible(
                settings,
                chunk_min_x,
                chunk_min_z,
                chunk,
                pos2.0,
                pos2.1,
                pos2.2,
            );
            if random.next_float() > self.chance_of_spread_radius3 {
                continue;
            }
            let spread3 = random_direction_offset(random);
            place_dripstone_block_if_possible(
                settings,
                chunk_min_x,
                chunk_min_z,
                chunk,
                pos2.0 + spread3.0,
                pos2.1 + spread3.1,
                pos2.2 + spread3.2,
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn place_at_spillover_target(
        &self,
        settings: &NoiseSettings,
        source_min_x: i32,
        source_min_z: i32,
        source_chunk: &NoiseChunkBlocks,
        target_min_x: i32,
        target_min_z: i32,
        target_chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
        world_x: i32,
        world_y: i32,
        world_z: i32,
    ) -> bool {
        if !is_air_or_water_at_world_in_context(
            source_chunk,
            source_min_x,
            source_min_z,
            target_chunk,
            target_min_x,
            target_min_z,
            world_x,
            world_y,
            world_z,
            settings.min_y,
        ) {
            return false;
        }

        let can_place_above = is_dripstone_base_at_world_in_context(
            source_chunk,
            source_min_x,
            source_min_z,
            target_chunk,
            target_min_x,
            target_min_z,
            world_x,
            world_y + 1,
            world_z,
            settings.min_y,
        );
        let can_place_below = is_dripstone_base_at_world_in_context(
            source_chunk,
            source_min_x,
            source_min_z,
            target_chunk,
            target_min_x,
            target_min_z,
            world_x,
            world_y - 1,
            world_z,
            settings.min_y,
        );
        let Some(tip_direction) =
            dripstone_tip_direction(can_place_above, can_place_below, random)
        else {
            return false;
        };

        let root_y = world_y - tip_direction.dy();
        self.create_patch_of_dripstone_blocks_spillover_target(
            settings,
            target_min_x,
            target_min_z,
            target_chunk,
            random,
            world_x,
            root_y,
            world_z,
        );
        let height = if random.next_float() < self.chance_of_taller_dripstone
            && is_air_or_water_at_world_in_context(
                source_chunk,
                source_min_x,
                source_min_z,
                target_chunk,
                target_min_x,
                target_min_z,
                world_x,
                world_y + tip_direction.dy(),
                world_z,
                settings.min_y,
            ) {
            2
        } else {
            1
        };

        grow_pointed_dripstone_spillover_target(
            settings,
            source_min_x,
            source_min_z,
            source_chunk,
            target_min_x,
            target_min_z,
            target_chunk,
            world_x,
            world_y,
            world_z,
            tip_direction,
            height,
            false,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn create_patch_of_dripstone_blocks_spillover_target(
        &self,
        settings: &NoiseSettings,
        target_min_x: i32,
        target_min_z: i32,
        target_chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
        world_x: i32,
        world_y: i32,
        world_z: i32,
    ) {
        place_dripstone_block_if_possible(
            settings,
            target_min_x,
            target_min_z,
            target_chunk,
            world_x,
            world_y,
            world_z,
        );

        for (dx, dz) in horizontal_directions() {
            if random.next_float() > self.chance_of_directional_spread {
                continue;
            }
            let pos1 = (world_x + dx, world_y, world_z + dz);
            place_dripstone_block_if_possible(
                settings,
                target_min_x,
                target_min_z,
                target_chunk,
                pos1.0,
                pos1.1,
                pos1.2,
            );
            if random.next_float() > self.chance_of_spread_radius2 {
                continue;
            }
            let spread2 = random_direction_offset(random);
            let pos2 = (pos1.0 + spread2.0, pos1.1 + spread2.1, pos1.2 + spread2.2);
            place_dripstone_block_if_possible(
                settings,
                target_min_x,
                target_min_z,
                target_chunk,
                pos2.0,
                pos2.1,
                pos2.2,
            );
            if random.next_float() > self.chance_of_spread_radius3 {
                continue;
            }
            let spread3 = random_direction_offset(random);
            place_dripstone_block_if_possible(
                settings,
                target_min_x,
                target_min_z,
                target_chunk,
                pos2.0 + spread3.0,
                pos2.1 + spread3.1,
                pos2.2 + spread3.2,
            );
        }
    }
}

#[derive(Debug, Clone)]
struct LargeDripstoneCone {
    root_y: i32,
    pointing_up: bool,
    radius: i32,
    max_height: i32,
    bluntness: f64,
    scale: f64,
}

impl LargeDripstoneCone {
    #[allow(clippy::too_many_arguments)]
    fn place_blocks(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
        root_x: i32,
        root_z: i32,
    ) -> bool {
        let direction = if self.pointing_up {
            DripstoneDirection::Up
        } else {
            DripstoneDirection::Down
        };
        let mut placed = false;
        for dx in -self.radius..=self.radius {
            for dz in -self.radius..=self.radius {
                let world_x = root_x + dx;
                let world_z = root_z + dz;
                let Some((local_x, local_z)) =
                    local_coords(world_x, world_z, chunk_min_x, chunk_min_z)
                else {
                    continue;
                };

                let current_radius = ((dx * dx + dz * dz) as f64).sqrt();
                if current_radius > self.radius as f64 {
                    continue;
                }

                let mut height = self.height_at_radius(current_radius).min(self.max_height);
                if height <= 0 {
                    continue;
                }
                if random.next_float() < 0.2 {
                    height = ((height as f32) * random_float_between(random, 0.8, 1.0)) as i32;
                }

                let mut has_been_out_of_stone = false;
                for step in 0..height {
                    let world_y = self.root_y + direction.dy() * step;
                    if !(settings.min_y..settings.min_y + settings.height).contains(&world_y) {
                        break;
                    }
                    let Some(current) = layer_at_world(
                        chunk,
                        chunk_min_x,
                        chunk_min_z,
                        world_x,
                        world_y,
                        world_z,
                        settings.min_y,
                    ) else {
                        continue;
                    };
                    if is_empty_or_water_or_lava_layer(current)
                        || current.is("minecraft:dripstone_block")
                    {
                        has_been_out_of_stone = true;
                        chunk.set_layer(
                            local_x,
                            world_y,
                            local_z,
                            settings.min_y,
                            BlockLayer::new("minecraft:dripstone_block"),
                        );
                        placed = true;
                    } else if has_been_out_of_stone && is_base_stone_overworld(current) {
                        break;
                    }
                }
            }
        }
        placed
    }

    fn height_at_radius(&self, xz_distance_from_center: f64) -> i32 {
        dripstone_height(
            xz_distance_from_center,
            self.radius as f64,
            self.scale,
            self.bluntness,
        ) as i32
    }
}

#[derive(Clone, Copy, Debug)]
struct DripstoneColumn {
    floor: Option<i32>,
    ceiling: Option<i32>,
}

impl DripstoneColumn {
    fn height(self) -> Option<i32> {
        let (Some(floor_y), Some(ceiling_y)) = (self.floor, self.ceiling) else {
            return None;
        };
        Some((ceiling_y - floor_y - 1).max(0))
    }
}

#[derive(Clone, Copy, Debug)]
enum DripstoneDirection {
    Up,
    Down,
}

impl DripstoneDirection {
    fn dy(self) -> i32 {
        match self {
            Self::Up => 1,
            Self::Down => -1,
        }
    }

    fn scan_sign(self) -> i32 {
        self.dy()
    }

    fn vertical_property(self) -> &'static str {
        match self {
            Self::Up => "up",
            Self::Down => "down",
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct FeatureUniformFloat {
    min: f32,
    max: f32,
}

impl FeatureUniformFloat {
    fn sample(self, random: &mut FeatureRandom) -> f32 {
        random_float_between(random, self.min, self.max)
    }
}

#[derive(Clone, Copy, Debug)]
struct ClampedNormalFloat {
    mean: f64,
    deviation: f64,
    min: f64,
    max: f64,
}

impl ClampedNormalFloat {
    fn sample(self, random: &mut FeatureRandom) -> f32 {
        sample_clamped_normal(random, self.mean, self.deviation, self.min, self.max) as f32
    }
}

#[derive(Clone, Copy, Debug)]
struct ClampedNormalInt {
    mean: f64,
    deviation: f64,
    min: i32,
    max: i32,
}

impl ClampedNormalInt {
    fn sample(self, random: &mut FeatureRandom) -> i32 {
        (sample_clamped_normal(
            random,
            self.mean,
            self.deviation,
            self.min as f64,
            self.max as f64,
        )
        .round() as i32)
            .clamp(self.min, self.max)
    }
}

#[allow(clippy::too_many_arguments)]
fn scan_dripstone_column(
    settings: &NoiseSettings,
    chunk: &NoiseChunkBlocks,
    chunk_min_x: i32,
    chunk_min_z: i32,
    world_x: i32,
    world_y: i32,
    world_z: i32,
    range: i32,
    target: fn(&BlockLayer) -> bool,
) -> Option<DripstoneColumn> {
    if !is_empty_or_water_at_world(
        chunk,
        chunk_min_x,
        chunk_min_z,
        world_x,
        world_y,
        world_z,
        settings.min_y,
    ) {
        return None;
    }

    let floor = scan_dripstone_column_direction(
        settings,
        chunk,
        chunk_min_x,
        chunk_min_z,
        world_x,
        world_y,
        world_z,
        -1,
        range,
        target,
    );
    let ceiling = scan_dripstone_column_direction(
        settings,
        chunk,
        chunk_min_x,
        chunk_min_z,
        world_x,
        world_y,
        world_z,
        1,
        range,
        target,
    );
    (floor.is_some() || ceiling.is_some()).then_some(DripstoneColumn { floor, ceiling })
}

#[allow(clippy::too_many_arguments)]
fn scan_dripstone_column_direction(
    settings: &NoiseSettings,
    chunk: &NoiseChunkBlocks,
    chunk_min_x: i32,
    chunk_min_z: i32,
    world_x: i32,
    world_y: i32,
    world_z: i32,
    dy: i32,
    range: i32,
    target: fn(&BlockLayer) -> bool,
) -> Option<i32> {
    for step in 1..=range {
        let y = world_y + dy * step;
        if !(settings.min_y..settings.min_y + settings.height).contains(&y) {
            return None;
        }
        let layer = layer_at_world(chunk, chunk_min_x, chunk_min_z, world_x, y, world_z, settings.min_y)?;
        if is_air_or_water_layer(layer) {
            continue;
        }
        return target(layer).then_some(y);
    }
    None
}

#[allow(clippy::too_many_arguments)]
fn scan_air_or_water_to_solid(
    settings: &NoiseSettings,
    chunk: &NoiseChunkBlocks,
    chunk_min_x: i32,
    chunk_min_z: i32,
    world_x: i32,
    world_y: i32,
    world_z: i32,
    dy: i32,
    range: i32,
) -> Option<i32> {
    for step in 0..=range {
        let y = world_y + dy * step;
        if !(settings.min_y..settings.min_y + settings.height).contains(&y) {
            return None;
        }
        let current =
            layer_at_world(chunk, chunk_min_x, chunk_min_z, world_x, y, world_z, settings.min_y)?;
        if !is_air_or_water_layer(current) {
            return None;
        }
        let target_y = y + dy;
        if !(settings.min_y..settings.min_y + settings.height).contains(&target_y) {
            return None;
        }
        if is_solid_at_world(
            chunk,
            chunk_min_x,
            chunk_min_z,
            world_x,
            target_y,
            world_z,
            settings.min_y,
        ) {
            return Some(target_y);
        }
    }
    None
}

#[allow(clippy::too_many_arguments)]
fn scan_air_or_water_to_solid_in_context(
    settings: &NoiseSettings,
    source_min_x: i32,
    source_min_z: i32,
    source_chunk: &NoiseChunkBlocks,
    target_min_x: i32,
    target_min_z: i32,
    target_chunk: &NoiseChunkBlocks,
    world_x: i32,
    world_y: i32,
    world_z: i32,
    dy: i32,
    range: i32,
) -> Option<i32> {
    for step in 0..=range {
        let y = world_y + dy * step;
        if !(settings.min_y..settings.min_y + settings.height).contains(&y) {
            return None;
        }
        let current = dripstone_context_layer(
            source_chunk,
            source_min_x,
            source_min_z,
            target_chunk,
            target_min_x,
            target_min_z,
            world_x,
            y,
            world_z,
            settings.min_y,
        )?;
        if !is_air_or_water_layer(current) {
            return None;
        }
        let target_y = y + dy;
        if !(settings.min_y..settings.min_y + settings.height).contains(&target_y) {
            return None;
        }
        if is_dripstone_solid_at_world_in_context(
            source_chunk,
            source_min_x,
            source_min_z,
            target_chunk,
            target_min_x,
            target_min_z,
            world_x,
            target_y,
            world_z,
            settings.min_y,
        ) {
            return Some(target_y);
        }
    }
    None
}

#[allow(clippy::too_many_arguments)]
fn grow_pointed_dripstone(
    settings: &NoiseSettings,
    chunk_min_x: i32,
    chunk_min_z: i32,
    chunk: &mut NoiseChunkBlocks,
    world_x: i32,
    world_y: i32,
    world_z: i32,
    direction: DripstoneDirection,
    height: i32,
    merged_tip: bool,
) -> bool {
    if height <= 0
        || !is_dripstone_base_at_world(
            chunk,
            chunk_min_x,
            chunk_min_z,
            world_x,
            world_y - direction.dy(),
            world_z,
            settings.min_y,
        )
    {
        return false;
    }

    let mut placed = false;
    for (offset, thickness) in dripstone_thicknesses(height, merged_tip) {
        let y = world_y + direction.dy() * offset;
        let Some(current) =
            layer_at_world(chunk, chunk_min_x, chunk_min_z, world_x, y, world_z, settings.min_y)
        else {
            continue;
        };
        if !is_air_or_water_layer(current) && !current.is("minecraft:pointed_dripstone") {
            continue;
        }
        let Some((local_x, local_z)) = local_coords(world_x, world_z, chunk_min_x, chunk_min_z)
        else {
            continue;
        };
        let block = pointed_dripstone_block(
            direction,
            thickness,
            waterlogged_at_world(
                chunk,
                chunk_min_x,
                chunk_min_z,
                world_x,
                y,
                world_z,
                settings.min_y,
            ),
        );
        chunk.set_layer(local_x, y, local_z, settings.min_y, block);
        placed = true;
    }
    placed
}

#[allow(clippy::too_many_arguments)]
fn grow_pointed_dripstone_spillover_target(
    settings: &NoiseSettings,
    source_min_x: i32,
    source_min_z: i32,
    source_chunk: &NoiseChunkBlocks,
    target_min_x: i32,
    target_min_z: i32,
    target_chunk: &mut NoiseChunkBlocks,
    world_x: i32,
    world_y: i32,
    world_z: i32,
    direction: DripstoneDirection,
    height: i32,
    merged_tip: bool,
) -> bool {
    if height <= 0
        || !is_dripstone_base_at_world_in_context(
            source_chunk,
            source_min_x,
            source_min_z,
            target_chunk,
            target_min_x,
            target_min_z,
            world_x,
            world_y - direction.dy(),
            world_z,
            settings.min_y,
        )
    {
        return false;
    }

    let mut placed = false;
    for (offset, thickness) in dripstone_thicknesses(height, merged_tip) {
        let y = world_y + direction.dy() * offset;
        let Some(current) = dripstone_context_layer(
            source_chunk,
            source_min_x,
            source_min_z,
            target_chunk,
            target_min_x,
            target_min_z,
            world_x,
            y,
            world_z,
            settings.min_y,
        ) else {
            continue;
        };
        if !is_air_or_water_layer(current) && !current.is("minecraft:pointed_dripstone") {
            continue;
        }
        let Some((local_x, local_z)) = local_coords(world_x, world_z, target_min_x, target_min_z)
        else {
            continue;
        };
        let block = pointed_dripstone_block(
            direction,
            thickness,
            waterlogged_at_world_in_context(
                source_chunk,
                source_min_x,
                source_min_z,
                target_chunk,
                target_min_x,
                target_min_z,
                world_x,
                y,
                world_z,
                settings.min_y,
            ),
        );
        target_chunk.set_layer(local_x, y, local_z, settings.min_y, block);
        placed = true;
    }
    placed
}

fn dripstone_thicknesses(height: i32, merged_tip: bool) -> Vec<(i32, &'static str)> {
    let mut result = Vec::new();
    if height >= 3 {
        result.push((0, "base"));
        for offset in 1..height - 2 {
            result.push((offset, "middle"));
        }
    }
    if height >= 2 {
        result.push((height - 2, "frustum"));
    }
    if height >= 1 {
        result.push((height - 1, if merged_tip { "tip_merge" } else { "tip" }));
    }
    result
}

fn pointed_dripstone_block(
    direction: DripstoneDirection,
    thickness: &str,
    waterlogged: &str,
) -> BlockLayer {
    BlockLayer::with_properties(
        "minecraft:pointed_dripstone",
        &[
            ("thickness", thickness),
            ("vertical_direction", direction.vertical_property()),
            ("waterlogged", waterlogged),
        ],
    )
}

#[allow(clippy::too_many_arguments)]
fn place_dripstone_block_if_possible(
    settings: &NoiseSettings,
    chunk_min_x: i32,
    chunk_min_z: i32,
    chunk: &mut NoiseChunkBlocks,
    world_x: i32,
    world_y: i32,
    world_z: i32,
) -> bool {
    if !(settings.min_y..settings.min_y + settings.height).contains(&world_y) {
        return false;
    }
    let Some((local_x, local_z)) = local_coords(world_x, world_z, chunk_min_x, chunk_min_z) else {
        return false;
    };
    let Some(current) = chunk.layer(local_x, world_y, local_z, settings.min_y) else {
        return false;
    };
    if !is_dripstone_replaceable_layer(current) {
        return false;
    }
    chunk.set_layer(
        local_x,
        world_y,
        local_z,
        settings.min_y,
        BlockLayer::new("minecraft:dripstone_block"),
    );
    true
}

fn dripstone_tip_direction(
    can_place_above: bool,
    can_place_below: bool,
    random: &mut FeatureRandom,
) -> Option<DripstoneDirection> {
    match (can_place_above, can_place_below) {
        (true, true) => Some(if random.next_bool() {
            DripstoneDirection::Down
        } else {
            DripstoneDirection::Up
        }),
        (true, false) => Some(DripstoneDirection::Down),
        (false, true) => Some(DripstoneDirection::Up),
        (false, false) => None,
    }
}

fn random_direction_offset(random: &mut FeatureRandom) -> (i32, i32, i32) {
    match random.next_int(6) {
        0 => (0, -1, 0),
        1 => (0, 1, 0),
        2 => (0, 0, -1),
        3 => (0, 0, 1),
        4 => (-1, 0, 0),
        _ => (1, 0, 0),
    }
}

fn dripstone_height(
    mut xz_distance_from_center: f64,
    dripstone_radius: f64,
    scale: f64,
    bluntness: f64,
) -> f64 {
    if xz_distance_from_center < bluntness {
        xz_distance_from_center = bluntness;
    }
    let r = xz_distance_from_center / dripstone_radius * 0.384;
    let part1 = 0.75 * r.powf(4.0 / 3.0);
    let part2 = r.powf(2.0 / 3.0);
    let part3 = (1.0 / 3.0) * r.ln();
    let height_relative_to_max_radius = (scale * (part1 - part2 - part3)).max(0.0);
    height_relative_to_max_radius / 0.384 * dripstone_radius
}

fn random_between_inclusive(random: &mut FeatureRandom, min: i32, max: i32) -> i32 {
    if min >= max {
        min
    } else {
        min + random.next_int(max - min + 1)
    }
}

fn random_float_between(random: &mut FeatureRandom, min: f32, max_exclusive: f32) -> f32 {
    random.next_float() * (max_exclusive - min) + min
}

fn sample_clamped_normal(
    random: &mut FeatureRandom,
    mean: f64,
    deviation: f64,
    min: f64,
    max: f64,
) -> f64 {
    let sample = mean + gaussian(random) * deviation;
    sample.clamp(min, max)
}

fn gaussian(random: &mut FeatureRandom) -> f64 {
    let u1 = (1.0 - random.next_double()).max(f64::MIN_POSITIVE);
    let u2 = random.next_double();
    (-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos()
}

fn clamped_map_f64(value: f64, from_min: f64, from_max: f64, to_min: f64, to_max: f64) -> f64 {
    if value <= from_min {
        return to_min;
    }
    if value >= from_max {
        return to_max;
    }
    let delta = (value - from_min) / (from_max - from_min);
    to_min + delta * (to_max - to_min)
}

fn is_empty_or_water_at_world(
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
    .is_some_and(is_air_or_water_layer)
}

fn horizontal_box_reaches_chunk(
    min_x: i32,
    max_x: i32,
    min_z: i32,
    max_z: i32,
    chunk_min_x: i32,
    chunk_min_z: i32,
) -> bool {
    max_x >= chunk_min_x
        && min_x <= chunk_min_x + 15
        && max_z >= chunk_min_z
        && min_z <= chunk_min_z + 15
}

fn is_lava_at_world(
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
    .is_some_and(|layer| layer.is("minecraft:lava"))
}

fn is_dripstone_base_at_world(
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
    .is_some_and(is_dripstone_base_layer)
}

#[allow(clippy::too_many_arguments)]
fn is_air_or_water_at_world_in_context(
    source_chunk: &NoiseChunkBlocks,
    source_min_x: i32,
    source_min_z: i32,
    target_chunk: &NoiseChunkBlocks,
    target_min_x: i32,
    target_min_z: i32,
    world_x: i32,
    world_y: i32,
    world_z: i32,
    min_y: i32,
) -> bool {
    dripstone_context_layer(
        source_chunk,
        source_min_x,
        source_min_z,
        target_chunk,
        target_min_x,
        target_min_z,
        world_x,
        world_y,
        world_z,
        min_y,
    )
    .is_some_and(is_air_or_water_layer)
}

#[allow(clippy::too_many_arguments)]
fn is_dripstone_solid_at_world_in_context(
    source_chunk: &NoiseChunkBlocks,
    source_min_x: i32,
    source_min_z: i32,
    target_chunk: &NoiseChunkBlocks,
    target_min_x: i32,
    target_min_z: i32,
    world_x: i32,
    world_y: i32,
    world_z: i32,
    min_y: i32,
) -> bool {
    dripstone_context_layer(
        source_chunk,
        source_min_x,
        source_min_z,
        target_chunk,
        target_min_x,
        target_min_z,
        world_x,
        world_y,
        world_z,
        min_y,
    )
    .is_some_and(is_full_solid_layer)
}

#[allow(clippy::too_many_arguments)]
fn is_dripstone_base_at_world_in_context(
    source_chunk: &NoiseChunkBlocks,
    source_min_x: i32,
    source_min_z: i32,
    target_chunk: &NoiseChunkBlocks,
    target_min_x: i32,
    target_min_z: i32,
    world_x: i32,
    world_y: i32,
    world_z: i32,
    min_y: i32,
) -> bool {
    dripstone_context_layer(
        source_chunk,
        source_min_x,
        source_min_z,
        target_chunk,
        target_min_x,
        target_min_z,
        world_x,
        world_y,
        world_z,
        min_y,
    )
    .is_some_and(is_dripstone_base_layer)
}

#[allow(clippy::too_many_arguments)]
fn waterlogged_at_world_in_context(
    source_chunk: &NoiseChunkBlocks,
    source_min_x: i32,
    source_min_z: i32,
    target_chunk: &NoiseChunkBlocks,
    target_min_x: i32,
    target_min_z: i32,
    world_x: i32,
    world_y: i32,
    world_z: i32,
    min_y: i32,
) -> &'static str {
    if dripstone_context_layer(
        source_chunk,
        source_min_x,
        source_min_z,
        target_chunk,
        target_min_x,
        target_min_z,
        world_x,
        world_y,
        world_z,
        min_y,
    )
    .is_some_and(is_water_layer)
    {
        "true"
    } else {
        "false"
    }
}

#[allow(clippy::too_many_arguments)]
fn dripstone_context_layer<'a>(
    source_chunk: &'a NoiseChunkBlocks,
    source_min_x: i32,
    source_min_z: i32,
    target_chunk: &'a NoiseChunkBlocks,
    target_min_x: i32,
    target_min_z: i32,
    world_x: i32,
    world_y: i32,
    world_z: i32,
    min_y: i32,
) -> Option<&'a BlockLayer> {
    layer_at_world(
        source_chunk,
        source_min_x,
        source_min_z,
        world_x,
        world_y,
        world_z,
        min_y,
    )
    .or_else(|| {
        layer_at_world(
            target_chunk,
            target_min_x,
            target_min_z,
            world_x,
            world_y,
            world_z,
            min_y,
        )
    })
}

fn is_dripstone_base_or_lava_layer(layer: &BlockLayer) -> bool {
    is_dripstone_base_layer(layer) || layer.is("minecraft:lava")
}

fn is_dripstone_base_layer(layer: &BlockLayer) -> bool {
    layer.is("minecraft:dripstone_block") || is_dripstone_replaceable_layer(layer)
}

fn is_dripstone_replaceable_layer(layer: &BlockLayer) -> bool {
    is_base_stone_overworld(layer)
        || matches!(
            layer.block.as_ref(),
            "minecraft:calcite" | "minecraft:dirt" | "minecraft:coarse_dirt" | "minecraft:gravel"
        )
}

fn is_empty_or_water_or_lava_layer(layer: &BlockLayer) -> bool {
    layer.is_air || layer.is("minecraft:water") || layer.is("minecraft:lava")
}

fn can_dripstone_pool_touch_water(
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
    .is_some_and(|layer| is_base_stone_overworld(layer) || layer.is("minecraft:water"))
}

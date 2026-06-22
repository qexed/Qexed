#[derive(Debug, Clone)]
struct PlacedSimpleVegetationFeature {
    step_index: i32,
    feature_index: i32,
    outer_count: i32,
    count_provider: SimpleVegetationCountProvider,
    heightmap: SimpleVegetationHeightmap,
    noise_threshold: Option<NoiseThresholdCount>,
    rarity: i32,
    inner_count: i32,
    xz_offset: TrapezoidInt,
    y_offset: TrapezoidInt,
    block: SimpleVegetationBlock,
    required_support: Option<&'static str>,
    placement_predicate: SimpleVegetationPlacementPredicate,
    biome_filter: FeatureBiomeFilter,
}

impl PlacedSimpleVegetationFeature {
    fn patch_tall_grass_2(feature_index: i32) -> Self {
        Self {
            step_index: 9,
            feature_index,
            outer_count: 1,
            count_provider: SimpleVegetationCountProvider::Fixed,
            heightmap: SimpleVegetationHeightmap::WorldSurfaceWg,
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
            placement_predicate: SimpleVegetationPlacementPredicate::Air,
            biome_filter: FeatureBiomeFilter::All,
        }
    }

    fn patch_tall_grass(feature_index: i32) -> Self {
        Self::simple_patch(
            feature_index,
            1,
            5,
            96,
            SimpleVegetationBlock::tall_grass(),
            None,
        )
    }

    fn patch_bush(feature_index: i32) -> Self {
        Self {
            step_index: 9,
            feature_index,
            outer_count: 1,
            count_provider: SimpleVegetationCountProvider::Fixed,
            heightmap: SimpleVegetationHeightmap::MotionBlocking,
            noise_threshold: None,
            rarity: 4,
            inner_count: 24,
            xz_offset: TrapezoidInt::new(-5, 5, 0),
            y_offset: TrapezoidInt::new(-3, 3, 0),
            block: SimpleVegetationBlock::single("minecraft:bush"),
            required_support: None,
            placement_predicate: SimpleVegetationPlacementPredicate::Air,
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
            count_provider: SimpleVegetationCountProvider::Fixed,
            heightmap: SimpleVegetationHeightmap::WorldSurfaceWg,
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
            placement_predicate: SimpleVegetationPlacementPredicate::Air,
            biome_filter: FeatureBiomeFilter::All,
        }
    }

    fn flower_default(feature_index: i32) -> Self {
        Self::single_surface_patch(feature_index, 32, SimpleVegetationBlock::default_flower())
    }

    fn flower_warm(feature_index: i32) -> Self {
        Self::single_surface_patch(feature_index, 16, SimpleVegetationBlock::default_flower())
    }

    fn flower_swamp(feature_index: i32) -> Self {
        Self::patch_with_offsets(
            feature_index,
            None,
            32,
            64,
            TrapezoidInt::new(-6, 6, 0),
            TrapezoidInt::new(-2, 2, 0),
            SimpleVegetationBlock::single("minecraft:blue_orchid"),
            None,
        )
    }

    fn flower_cherry(feature_index: i32) -> Self {
        Self::patch_with_offsets(
            feature_index,
            Some(NoiseThresholdCount {
                noise_level: -0.8,
                below_noise: 5,
                above_noise: 10,
            }),
            1,
            96,
            TrapezoidInt::new(-6, 6, 0),
            TrapezoidInt::new(-2, 2, 0),
            SimpleVegetationBlock::cherry_flower(),
            None,
        )
    }

    fn flower_pale_garden(feature_index: i32) -> Self {
        Self::single_surface_patch(
            feature_index,
            32,
            SimpleVegetationBlock::single("minecraft:closed_eyeblossom"),
        )
    }

    fn flower_meadow(feature_index: i32) -> Self {
        Self::patch_with_offsets(
            feature_index,
            None,
            1,
            96,
            TrapezoidInt::new(-6, 6, 0),
            TrapezoidInt::new(-2, 2, 0),
            SimpleVegetationBlock::meadow_flower(),
            None,
        )
    }

    fn flower_flower_forest(feature_index: i32) -> Self {
        Self::patch_with_offsets(
            feature_index,
            None,
            2,
            96,
            TrapezoidInt::new(-6, 6, 0),
            TrapezoidInt::new(-2, 2, 0),
            SimpleVegetationBlock::flower_forest_flower(),
            None,
        )
        .with_outer_count(3)
    }

    fn forest_flowers(feature_index: i32, min_outer_count: i32, max_outer_count: i32) -> Self {
        Self::patch_with_offsets(
            feature_index,
            None,
            7,
            96,
            TrapezoidInt::new(-7, 7, 0),
            TrapezoidInt::new(-3, 3, 0),
            SimpleVegetationBlock::forest_flower(),
            None,
        )
        .with_outer_count_provider(SimpleVegetationCountProvider::ClampedUniform {
            min: min_outer_count,
            max: max_outer_count,
            clamp_min: 0,
            clamp_max: max_outer_count,
        })
        .with_heightmap(SimpleVegetationHeightmap::MotionBlocking)
    }

    fn patch_leaf_litter(feature_index: i32) -> Self {
        Self::simple_patch(
            feature_index,
            2,
            1,
            32,
            SimpleVegetationBlock::leaf_litter(),
            Some("minecraft:grass_block"),
        )
    }

    fn wildflowers_meadow(feature_index: i32) -> Self {
        Self::patch_with_offsets(
            feature_index,
            Some(NoiseThresholdCount {
                noise_level: -0.8,
                below_noise: 5,
                above_noise: 10,
            }),
            1,
            8,
            TrapezoidInt::new(-6, 6, 0),
            TrapezoidInt::new(-2, 2, 0),
            SimpleVegetationBlock::wildflowers(),
            None,
        )
    }

    fn wildflowers_birch_forest(feature_index: i32) -> Self {
        Self::patch_with_offsets(
            feature_index,
            None,
            2,
            64,
            TrapezoidInt::new(-6, 6, 0),
            TrapezoidInt::new(-2, 2, 0),
            SimpleVegetationBlock::wildflowers(),
            None,
        )
        .with_outer_count(3)
    }

    fn pale_garden_flowers(feature_index: i32) -> Self {
        Self::simple_patch(
            feature_index,
            1,
            8,
            96,
            SimpleVegetationBlock::single("minecraft:closed_eyeblossom"),
            None,
        )
    }

    fn patch_waterlily(feature_index: i32) -> Self {
        Self::simple_patch(
            feature_index,
            4,
            1,
            10,
            SimpleVegetationBlock::waterlily(),
            None,
        )
    }

    fn patch_berry_common(feature_index: i32) -> Self {
        Self::simple_patch(
            feature_index,
            1,
            32,
            96,
            SimpleVegetationBlock::sweet_berry_bush(),
            Some("minecraft:grass_block"),
        )
    }

    fn patch_berry_rare(feature_index: i32) -> Self {
        Self::simple_patch(
            feature_index,
            1,
            384,
            96,
            SimpleVegetationBlock::sweet_berry_bush(),
            Some("minecraft:grass_block"),
        )
    }

    fn patch_firefly_bush_swamp(feature_index: i32) -> Self {
        Self::patch_with_offsets(
            feature_index,
            None,
            8,
            20,
            TrapezoidInt::new(-4, 4, 0),
            TrapezoidInt::new(-3, 3, 0),
            SimpleVegetationBlock::single("minecraft:firefly_bush"),
            None,
        )
    }

    fn patch_firefly_bush_near_water(feature_index: i32, outer_count: i32) -> Self {
        Self::patch_with_offsets(
            feature_index,
            None,
            1,
            20,
            TrapezoidInt::new(-4, 4, 0),
            TrapezoidInt::new(-3, 3, 0),
            SimpleVegetationBlock::single("minecraft:firefly_bush"),
            None,
        )
        .with_outer_count(outer_count)
        .with_placement_predicate(SimpleVegetationPlacementPredicate::AirSurvivesNearWater)
    }

    fn patch_grass_plain(feature_index: i32) -> Self {
        Self {
            step_index: 9,
            feature_index,
            outer_count: 1,
            count_provider: SimpleVegetationCountProvider::Fixed,
            heightmap: SimpleVegetationHeightmap::WorldSurfaceWg,
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
            placement_predicate: SimpleVegetationPlacementPredicate::Air,
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
            count_provider: SimpleVegetationCountProvider::Fixed,
            heightmap: SimpleVegetationHeightmap::WorldSurfaceWg,
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
            placement_predicate: SimpleVegetationPlacementPredicate::Air,
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
        Self::patch_with_offsets(
            feature_index,
            None,
            rarity,
            inner_count,
            TrapezoidInt::new(-7, 7, 0),
            TrapezoidInt::new(-3, 3, 0),
            block,
            required_support,
        )
        .with_outer_count(outer_count)
    }

    fn single_surface_patch(feature_index: i32, rarity: i32, block: SimpleVegetationBlock) -> Self {
        Self::patch_with_offsets(
            feature_index,
            None,
            rarity,
            1,
            TrapezoidInt::new(0, 0, 0),
            TrapezoidInt::new(0, 0, 0),
            block,
            None,
        )
    }

    fn patch_with_offsets(
        feature_index: i32,
        noise_threshold: Option<NoiseThresholdCount>,
        rarity: i32,
        inner_count: i32,
        xz_offset: TrapezoidInt,
        y_offset: TrapezoidInt,
        block: SimpleVegetationBlock,
        required_support: Option<&'static str>,
    ) -> Self {
        Self {
            step_index: 9,
            feature_index,
            outer_count: 1,
            count_provider: SimpleVegetationCountProvider::Fixed,
            heightmap: SimpleVegetationHeightmap::WorldSurfaceWg,
            noise_threshold,
            rarity,
            inner_count,
            xz_offset,
            y_offset,
            block,
            required_support,
            placement_predicate: SimpleVegetationPlacementPredicate::Air,
            biome_filter: FeatureBiomeFilter::All,
        }
    }

    fn with_outer_count(mut self, outer_count: i32) -> Self {
        self.outer_count = outer_count;
        self.count_provider = SimpleVegetationCountProvider::Fixed;
        self
    }

    fn with_outer_count_provider(mut self, provider: SimpleVegetationCountProvider) -> Self {
        self.outer_count = provider.display_count(self.outer_count);
        self.count_provider = provider;
        self
    }

    fn with_heightmap(mut self, heightmap: SimpleVegetationHeightmap) -> Self {
        self.heightmap = heightmap;
        self
    }

    fn with_biome_filter(mut self, biome_filter: FeatureBiomeFilter) -> Self {
        self.biome_filter = biome_filter;
        self
    }

    fn with_placement_predicate(
        mut self,
        placement_predicate: SimpleVegetationPlacementPredicate,
    ) -> Self {
        self.placement_predicate = placement_predicate;
        self
    }

    fn max_horizontal_spillover(&self) -> i32 {
        let support_radius = match self.placement_predicate {
            SimpleVegetationPlacementPredicate::Air => 0,
            SimpleVegetationPlacementPredicate::AirSurvivesNearWater => 1,
        };
        self.xz_offset
            .min
            .abs()
            .max(self.xz_offset.max.abs())
            .max(support_radius)
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
            .unwrap_or_else(|| self.count_provider.sample(self.outer_count, random));
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
            let base_y = self
                .heightmap
                .height(settings, chunk, base_local_x, base_local_z);
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
                self.place_candidate(
                    settings, origin_x, origin_z, chunk, random, world_x, world_y, world_z,
                );
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn place_with_neighbors(
        &self,
        settings: &NoiseSettings,
        origin_x: i32,
        origin_z: i32,
        chunk: &mut NoiseChunkBlocks,
        neighbors: &[(i32, i32, &NoiseChunkBlocks)],
        random: &mut FeatureRandom,
    ) {
        let outer_count = self
            .noise_threshold
            .as_ref()
            .map(|threshold| threshold.sample(origin_x, origin_z))
            .unwrap_or_else(|| self.count_provider.sample(self.outer_count, random));
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
            let base_y = self
                .heightmap
                .height(settings, chunk, base_local_x, base_local_z);
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
                self.place_candidate_with_neighbors(
                    settings, origin_x, origin_z, chunk, neighbors, random, world_x, world_y,
                    world_z,
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
        let outer_count = self
            .noise_threshold
            .as_ref()
            .map(|threshold| threshold.sample(source_origin_x, source_origin_z))
            .unwrap_or_else(|| self.count_provider.sample(self.outer_count, random));
        for _ in 0..outer_count {
            if random.next_float() >= 1.0 / self.rarity as f32 {
                continue;
            }

            let base_x = source_origin_x + random.next_int(16);
            let base_z = source_origin_z + random.next_int(16);
            let Some((base_local_x, base_local_z)) =
                local_coords(base_x, base_z, source_origin_x, source_origin_z)
            else {
                continue;
            };
            let base_y =
                self.heightmap
                    .height(settings, source_chunk, base_local_x, base_local_z);
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

                let candidate_random = random.clone();
                let mut source_random = candidate_random.clone();
                let in_source = overlaps_chunk(world_x, world_z, source_origin_x, source_origin_z);
                if in_source {
                    self.place_candidate_with_neighbors(
                        settings,
                        source_origin_x,
                        source_origin_z,
                        source_chunk,
                        &[(target_origin_x, target_origin_z, &*target_chunk)],
                        &mut source_random,
                        world_x,
                        world_y,
                        world_z,
                    );
                }

                let mut target_random = candidate_random;
                let in_target = overlaps_chunk(world_x, world_z, target_origin_x, target_origin_z);
                self.place_candidate_with_neighbors(
                    settings,
                    target_origin_x,
                    target_origin_z,
                    target_chunk,
                    &[(source_origin_x, source_origin_z, &*source_chunk)],
                    &mut target_random,
                    world_x,
                    world_y,
                    world_z,
                );

                if in_source {
                    *random = source_random;
                } else if in_target {
                    *random = target_random;
                }
            }
        }
    }

    fn may_spill_into(
        &self,
        source_origin_x: i32,
        source_origin_z: i32,
        target_origin_x: i32,
        target_origin_z: i32,
        random: &mut FeatureRandom,
    ) -> bool {
        let outer_count = self
            .noise_threshold
            .as_ref()
            .map(|threshold| threshold.sample(source_origin_x, source_origin_z))
            .unwrap_or_else(|| self.count_provider.sample(self.outer_count, random));
        let radius = self.max_horizontal_spillover();
        for _ in 0..outer_count {
            if random.next_float() >= 1.0 / self.rarity as f32 {
                continue;
            }

            let base_x = source_origin_x + random.next_int(16);
            let base_z = source_origin_z + random.next_int(16);
            for _ in 0..self.inner_count {
                let world_x = base_x + self.xz_offset.sample(random);
                let _world_y = self.y_offset.sample(random);
                let world_z = base_z + self.xz_offset.sample(random);
                if horizontal_box_overlaps_chunk(
                    world_x - radius,
                    world_x + radius,
                    world_z - radius,
                    world_z + radius,
                    target_origin_x,
                    target_origin_z,
                ) {
                    return true;
                }
            }
        }
        false
    }

    #[allow(clippy::too_many_arguments)]
    fn place_candidate(
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
        if !self.placement_predicate.allows(
            chunk,
            chunk_min_x,
            chunk_min_z,
            world_x,
            world_y,
            world_z,
            settings.min_y,
        ) || !self.has_required_support(
            chunk,
            chunk_min_x,
            chunk_min_z,
            world_x,
            world_y - 1,
            world_z,
            settings.min_y,
        ) {
            return false;
        }
        self.block.place_at(
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

    #[allow(clippy::too_many_arguments)]
    fn place_candidate_with_neighbors(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        neighbors: &[(i32, i32, &NoiseChunkBlocks)],
        random: &mut FeatureRandom,
        world_x: i32,
        world_y: i32,
        world_z: i32,
    ) -> bool {
        if !self.placement_predicate.allows_with_neighbors(
            chunk,
            chunk_min_x,
            chunk_min_z,
            neighbors,
            world_x,
            world_y,
            world_z,
            settings.min_y,
        ) || !self.has_required_support_with_neighbors(
            chunk,
            chunk_min_x,
            chunk_min_z,
            neighbors,
            world_x,
            world_y - 1,
            world_z,
            settings.min_y,
        ) {
            return false;
        }
        self.block.place_at_with_neighbors(
            settings,
            chunk_min_x,
            chunk_min_z,
            chunk,
            neighbors,
            random,
            world_x,
            world_y,
            world_z,
        )
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

    fn has_required_support_with_neighbors(
        &self,
        chunk: &NoiseChunkBlocks,
        chunk_min_x: i32,
        chunk_min_z: i32,
        neighbors: &[(i32, i32, &NoiseChunkBlocks)],
        world_x: i32,
        world_y: i32,
        world_z: i32,
        min_y: i32,
    ) -> bool {
        let Some(required_support) = self.required_support else {
            return true;
        };
        vegetation_context_layer(
            chunk,
            chunk_min_x,
            chunk_min_z,
            neighbors,
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

    fn default_flower() -> Self {
        Self::weighted_single(
            vec![
                (BlockLayer::new("minecraft:poppy"), 2),
                (BlockLayer::new("minecraft:dandelion"), 1),
            ],
            SimpleVegetationSupport::Vegetation,
        )
    }

    fn cherry_flower() -> Self {
        let mut entries = Vec::with_capacity(16);
        for flower_amount in ["1", "2", "3", "4"] {
            for facing in ["north", "east", "south", "west"] {
                entries.push((
                    BlockLayer::with_properties(
                        "minecraft:pink_petals",
                        &[("facing", facing), ("flower_amount", flower_amount)],
                    ),
                    1,
                ));
            }
        }

        Self::weighted_single(entries, SimpleVegetationSupport::Vegetation)
    }

    fn wildflowers() -> Self {
        let mut entries = Vec::with_capacity(16);
        for flower_amount in ["1", "2", "3", "4"] {
            for facing in ["north", "east", "south", "west"] {
                entries.push((
                    BlockLayer::with_properties(
                        "minecraft:wildflowers",
                        &[("facing", facing), ("flower_amount", flower_amount)],
                    ),
                    1,
                ));
            }
        }

        Self::weighted_single(entries, SimpleVegetationSupport::Vegetation)
    }

    fn leaf_litter() -> Self {
        let mut entries = Vec::with_capacity(12);
        for segment_amount in ["1", "2", "3"] {
            for facing in ["north", "east", "south", "west"] {
                entries.push((
                    BlockLayer::with_properties(
                        "minecraft:leaf_litter",
                        &[("facing", facing), ("segment_amount", segment_amount)],
                    ),
                    1,
                ));
            }
        }

        Self::weighted_single(entries, SimpleVegetationSupport::Vegetation)
    }

    fn meadow_flower() -> Self {
        Self::weighted_single(
            vec![
                (
                    BlockLayer::with_properties("minecraft:tall_grass", &[("half", "lower")]),
                    1,
                ),
                (BlockLayer::new("minecraft:allium"), 1),
                (BlockLayer::new("minecraft:poppy"), 1),
                (BlockLayer::new("minecraft:azure_bluet"), 1),
                (BlockLayer::new("minecraft:dandelion"), 1),
                (BlockLayer::new("minecraft:cornflower"), 1),
                (BlockLayer::new("minecraft:oxeye_daisy"), 1),
                (BlockLayer::new("minecraft:short_grass"), 1),
            ],
            SimpleVegetationSupport::Vegetation,
        )
    }

    fn flower_forest_flower() -> Self {
        Self::weighted_single(
            vec![
                (BlockLayer::new("minecraft:dandelion"), 1),
                (BlockLayer::new("minecraft:poppy"), 1),
                (BlockLayer::new("minecraft:allium"), 1),
                (BlockLayer::new("minecraft:azure_bluet"), 1),
                (BlockLayer::new("minecraft:red_tulip"), 1),
                (BlockLayer::new("minecraft:orange_tulip"), 1),
                (BlockLayer::new("minecraft:white_tulip"), 1),
                (BlockLayer::new("minecraft:pink_tulip"), 1),
                (BlockLayer::new("minecraft:oxeye_daisy"), 1),
                (BlockLayer::new("minecraft:cornflower"), 1),
                (BlockLayer::new("minecraft:lily_of_the_valley"), 1),
            ],
            SimpleVegetationSupport::Vegetation,
        )
    }

    fn forest_flower() -> Self {
        Self::weighted_single(
            vec![
                (
                    BlockLayer::with_properties("minecraft:lilac", &[("half", "lower")]),
                    1,
                ),
                (
                    BlockLayer::with_properties("minecraft:rose_bush", &[("half", "lower")]),
                    1,
                ),
                (
                    BlockLayer::with_properties("minecraft:peony", &[("half", "lower")]),
                    1,
                ),
                (BlockLayer::new("minecraft:lily_of_the_valley"), 1),
            ],
            SimpleVegetationSupport::Vegetation,
        )
    }

    fn waterlily() -> Self {
        Self {
            lower: BlockLayer::new("minecraft:lily_pad"),
            upper: None,
            provider: SimpleVegetationProvider::Fixed,
            support: SimpleVegetationSupport::WaterSurface,
        }
    }

    fn sweet_berry_bush() -> Self {
        Self {
            lower: BlockLayer::with_properties("minecraft:sweet_berry_bush", &[("age", "3")]),
            upper: None,
            provider: SimpleVegetationProvider::Fixed,
            support: SimpleVegetationSupport::Vegetation,
        }
    }

    fn moss_vegetation() -> Self {
        Self::weighted_single(
            vec![
                (BlockLayer::new("minecraft:flowering_azalea"), 4),
                (BlockLayer::new("minecraft:azalea"), 7),
                (BlockLayer::new("minecraft:moss_carpet"), 25),
                (BlockLayer::new("minecraft:short_grass"), 50),
                (
                    BlockLayer::with_properties("minecraft:tall_grass", &[("half", "lower")]),
                    10,
                ),
            ],
            SimpleVegetationSupport::Vegetation,
        )
    }

    fn pale_moss_vegetation() -> Self {
        Self::weighted_single(
            vec![
                (
                    BlockLayer::with_properties(
                        "minecraft:pale_moss_carpet",
                        &[
                            ("bottom", "true"),
                            ("east", "none"),
                            ("north", "none"),
                            ("south", "none"),
                            ("west", "none"),
                        ],
                    ),
                    25,
                ),
                (BlockLayer::new("minecraft:short_grass"), 25),
                (
                    BlockLayer::with_properties("minecraft:tall_grass", &[("half", "lower")]),
                    10,
                ),
            ],
            SimpleVegetationSupport::Vegetation,
        )
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
    fn place_at_with_neighbors(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        neighbors: &[(i32, i32, &NoiseChunkBlocks)],
        random: &mut FeatureRandom,
        world_x: i32,
        world_y: i32,
        world_z: i32,
    ) -> bool {
        let lower = self
            .provider
            .block_at(&self.lower, random, world_x, world_z);
        self.place_selected_with_neighbors(
            settings,
            chunk_min_x,
            chunk_min_z,
            chunk,
            neighbors,
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
        let selected_upper = self
            .upper
            .clone()
            .or_else(|| double_plant_upper_for(&lower));
        let lower = if selected_upper.is_some() {
            lower.with_property("half", "lower")
        } else {
            lower
        };
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

        if let Some(upper) = selected_upper {
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
            chunk.set_layer(local_x, world_y + 1, local_z, settings.min_y, upper);
        } else {
            chunk.set_layer(local_x, world_y, local_z, settings.min_y, lower);
        }
        true
    }

    #[allow(clippy::too_many_arguments)]
    fn place_selected_with_neighbors(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        neighbors: &[(i32, i32, &NoiseChunkBlocks)],
        lower: BlockLayer,
        world_x: i32,
        world_y: i32,
        world_z: i32,
    ) -> bool {
        let selected_upper = self
            .upper
            .clone()
            .or_else(|| double_plant_upper_for(&lower));
        let lower = if selected_upper.is_some() {
            lower.with_property("half", "lower")
        } else {
            lower
        };
        let Some((local_x, local_z)) = local_coords(world_x, world_z, chunk_min_x, chunk_min_z)
        else {
            return false;
        };
        if !chunk
            .layer(local_x, world_y, local_z, settings.min_y)
            .is_some_and(|layer| layer.is_air)
            || !self.support.allows_at_world_with_neighbors(
                chunk,
                chunk_min_x,
                chunk_min_z,
                neighbors,
                world_x,
                world_y - 1,
                world_z,
                settings.min_y,
            )
        {
            return false;
        }

        if let Some(upper) = selected_upper {
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
            chunk.set_layer(local_x, world_y + 1, local_z, settings.min_y, upper);
        } else {
            chunk.set_layer(local_x, world_y, local_z, settings.min_y, lower);
        }
        true
    }
}

#[derive(Debug, Clone, Copy)]
enum SimpleVegetationCountProvider {
    Fixed,
    ClampedUniform {
        min: i32,
        max: i32,
        clamp_min: i32,
        clamp_max: i32,
    },
}

impl SimpleVegetationCountProvider {
    fn sample(self, fixed: i32, random: &mut FeatureRandom) -> i32 {
        match self {
            Self::Fixed => fixed,
            Self::ClampedUniform {
                min,
                max,
                clamp_min,
                clamp_max,
            } => (min + random.next_int(max - min + 1)).clamp(clamp_min, clamp_max),
        }
    }

    fn display_count(self, fixed: i32) -> i32 {
        match self {
            Self::Fixed => fixed,
            Self::ClampedUniform { clamp_max, .. } => clamp_max,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SimpleVegetationHeightmap {
    WorldSurfaceWg,
    MotionBlocking,
}

impl SimpleVegetationHeightmap {
    fn height(
        self,
        settings: &NoiseSettings,
        chunk: &NoiseChunkBlocks,
        local_x: usize,
        local_z: usize,
    ) -> i32 {
        match self {
            Self::WorldSurfaceWg => chunk
                .column(local_x, local_z)
                .blocks
                .iter()
                .rposition(|layer| !layer.is_air)
                .map(|index| settings.min_y + index as i32 + 1)
                .unwrap_or(settings.min_y),
            Self::MotionBlocking => chunk
                .column(local_x, local_z)
                .blocks
                .iter()
                .rposition(is_motion_blocking_heightmap_layer)
                .map(|index| settings.min_y + index as i32 + 1)
                .unwrap_or(settings.min_y),
        }
    }
}

fn is_motion_blocking_heightmap_layer(layer: &BlockLayer) -> bool {
    is_fluid_layer(layer) || (!layer.is_air && !is_non_motion_blocking_vegetation_layer(layer))
}

fn is_non_motion_blocking_vegetation_layer(layer: &BlockLayer) -> bool {
    is_small_flower_layer(layer)
        || matches!(
            layer.block.as_ref(),
            "minecraft:short_grass"
                | "minecraft:tall_grass"
                | "minecraft:fern"
                | "minecraft:large_fern"
                | "minecraft:dead_bush"
                | "minecraft:bush"
                | "minecraft:firefly_bush"
                | "minecraft:leaf_litter"
                | "minecraft:short_dry_grass"
                | "minecraft:tall_dry_grass"
                | "minecraft:seagrass"
                | "minecraft:tall_seagrass"
                | "minecraft:kelp"
                | "minecraft:kelp_plant"
                | "minecraft:vine"
                | "minecraft:glow_lichen"
                | "minecraft:hanging_roots"
                | "minecraft:crimson_roots"
                | "minecraft:warped_roots"
                | "minecraft:nether_sprouts"
                | "minecraft:moss_carpet"
                | "minecraft:pale_moss_carpet"
                | "minecraft:wildflowers"
                | "minecraft:pink_petals"
        )
}

#[derive(Debug, Clone, Copy)]
enum SimpleVegetationPlacementPredicate {
    Air,
    AirSurvivesNearWater,
}

impl SimpleVegetationPlacementPredicate {
    #[allow(clippy::too_many_arguments)]
    fn allows(
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
            Self::Air => true,
            Self::AirSurvivesNearWater => {
                supports_vegetation_at_world(
                    chunk,
                    chunk_min_x,
                    chunk_min_z,
                    world_x,
                    world_y - 1,
                    world_z,
                    min_y,
                ) && horizontal_directions().iter().any(|(dx, dz)| {
                    is_water_at_world(
                        chunk,
                        chunk_min_x,
                        chunk_min_z,
                        world_x + dx,
                        world_y - 1,
                        world_z + dz,
                        min_y,
                    )
                })
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn allows_with_neighbors(
        self,
        chunk: &NoiseChunkBlocks,
        chunk_min_x: i32,
        chunk_min_z: i32,
        neighbors: &[(i32, i32, &NoiseChunkBlocks)],
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
            Self::Air => true,
            Self::AirSurvivesNearWater => {
                vegetation_context_layer(
                    chunk,
                    chunk_min_x,
                    chunk_min_z,
                    neighbors,
                    world_x,
                    world_y - 1,
                    world_z,
                    min_y,
                )
                .is_some_and(|layer| supports_vegetation_layer(&layer))
                    && horizontal_directions().iter().any(|(dx, dz)| {
                        vegetation_context_layer(
                            chunk,
                            chunk_min_x,
                            chunk_min_z,
                            neighbors,
                            world_x + dx,
                            world_y - 1,
                            world_z + dz,
                            min_y,
                        )
                        .is_some_and(|layer| is_water_layer(&layer))
                    })
            }
        }
    }
}

fn double_plant_upper_for(layer: &BlockLayer) -> Option<BlockLayer> {
    matches!(
        layer.block.as_ref(),
        "minecraft:lilac" | "minecraft:rose_bush" | "minecraft:peony"
    )
    .then(|| layer.with_property("half", "upper"))
}

#[derive(Debug, Clone, Copy)]
enum SimpleVegetationSupport {
    Vegetation,
    DeadBush,
    DryVegetation,
    WaterSurface,
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
            Self::WaterSurface => supports_lily_pad_layer(layer),
        }
    }

    fn allows_at_world_with_neighbors(
        self,
        chunk: &NoiseChunkBlocks,
        chunk_min_x: i32,
        chunk_min_z: i32,
        neighbors: &[(i32, i32, &NoiseChunkBlocks)],
        world_x: i32,
        world_y: i32,
        world_z: i32,
        min_y: i32,
    ) -> bool {
        let Some(layer) = vegetation_context_layer(
            chunk,
            chunk_min_x,
            chunk_min_z,
            neighbors,
            world_x,
            world_y,
            world_z,
            min_y,
        ) else {
            return false;
        };
        match self {
            Self::Vegetation => supports_vegetation_layer(&layer),
            Self::DeadBush => supports_dead_bush_layer(&layer),
            Self::DryVegetation => supports_dry_vegetation_layer(&layer),
            Self::WaterSurface => supports_lily_pad_layer(&layer),
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn vegetation_context_layer(
    chunk: &NoiseChunkBlocks,
    chunk_min_x: i32,
    chunk_min_z: i32,
    neighbors: &[(i32, i32, &NoiseChunkBlocks)],
    world_x: i32,
    world_y: i32,
    world_z: i32,
    min_y: i32,
) -> Option<BlockLayer> {
    if let Some(layer) = layer_at_world(
        chunk,
        chunk_min_x,
        chunk_min_z,
        world_x,
        world_y,
        world_z,
        min_y,
    ) {
        return Some(layer.clone());
    }

    neighbors.iter().find_map(|(neighbor_min_x, neighbor_min_z, neighbor_chunk)| {
        layer_at_world(
            neighbor_chunk,
            *neighbor_min_x,
            *neighbor_min_z,
            world_x,
            world_y,
            world_z,
            min_y,
        )
        .cloned()
    })
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

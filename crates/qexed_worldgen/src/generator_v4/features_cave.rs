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

    #[allow(clippy::too_many_arguments)]
    fn place_with_lazy_neighbors(
        &self,
        settings: &NoiseSettings,
        origin_x: i32,
        origin_z: i32,
        chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
        neighbor_sources: &mut NeighborFeatureSources,
        profile: &mut Option<FeaturePlacementProfile>,
        feature_name: &'static str,
    ) -> Duration {
        let started = Instant::now();
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

            let shape = self.config.sample_shape(random);
            if shape.fits_chunk(world_x, world_z, origin_x, origin_z) {
                self.config.place_resolved(
                    settings, origin_x, origin_z, chunk, random, world_x, world_y, world_z, shape,
                );
                continue;
            }

            let (min_x, max_x, min_z, max_z) = shape.bounds(world_x, world_z);
            let context_start = Instant::now();
            let neighbor_chunks =
                neighbor_sources.context_for_box(settings, min_x, max_x, min_z, max_z);
            if let Some(profile) = profile.as_mut() {
                profile.record(
                    feature_name,
                    FeatureProfilePhase::NeighborLoad,
                    context_start.elapsed(),
                );
            }
            self.config.place_resolved_with_neighbors(
                settings,
                origin_x,
                origin_z,
                chunk,
                &neighbor_chunks,
                random,
                world_x,
                world_y,
                world_z,
                shape,
            );
        }
        started.elapsed()
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

            let shape = self.config.sample_shape(random);
            if !shape.overlaps_chunk(world_x, world_z, target_origin_x, target_origin_z) {
                self.config.place_resolved(
                    settings,
                    source_origin_x,
                    source_origin_z,
                    source_chunk,
                    random,
                    world_x,
                    world_y,
                    world_z,
                    shape,
                );
                continue;
            }
            let mut replay_random = random.clone();
            if self.config.place_resolved(
                settings,
                source_origin_x,
                source_origin_z,
                source_chunk,
                random,
                world_x,
                world_y,
                world_z,
                shape,
            ) {
                self.config.place_spillover(
                    settings,
                    target_origin_x,
                    target_origin_z,
                    target_chunk,
                    &mut replay_random,
                    world_x,
                    world_y,
                    world_z,
                    shape,
                );
            }
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

            let shape = self.config.sample_shape(random);
            if shape.overlaps_chunk(world_x, world_z, target_origin_x, target_origin_z) {
                return true;
            }
        }
        false
    }

    #[allow(clippy::too_many_arguments)]
    fn target_precheck_prevents_spillover(
        &self,
        settings: &NoiseSettings,
        source_origin_x: i32,
        source_origin_z: i32,
        target_origin_x: i32,
        target_origin_z: i32,
        target_chunk: &NoiseChunkBlocks,
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

            let shape = self.config.sample_shape(random);
            if !shape.overlaps_chunk(world_x, world_z, target_origin_x, target_origin_z) {
                return false;
            }
            if !self.config.known_chunk_prevents_place(
                settings,
                target_origin_x,
                target_origin_z,
                target_chunk,
                world_x,
                world_y,
                world_z,
                shape,
            ) {
                return false;
            }
        }

        true
    }

    #[allow(clippy::too_many_arguments)]
    fn place_with_spillover_lazy_neighbors(
        &self,
        settings: &NoiseSettings,
        source_origin_x: i32,
        source_origin_z: i32,
        target_origin_x: i32,
        target_origin_z: i32,
        source_chunk: &mut NoiseChunkBlocks,
        target_chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
        neighbor_sources: &mut NeighborFeatureSources,
        profile: &mut Option<FeaturePlacementProfile>,
        feature_name: &'static str,
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

            let shape = self.config.sample_shape(random);
            if !shape.overlaps_chunk(world_x, world_z, target_origin_x, target_origin_z) {
                self.config.place_resolved(
                    settings,
                    source_origin_x,
                    source_origin_z,
                    source_chunk,
                    random,
                    world_x,
                    world_y,
                    world_z,
                    shape,
                );
                continue;
            }

            let mut replay_random = random.clone();
            let (min_x, max_x, min_z, max_z) = shape.bounds(world_x, world_z);
            let context_start = Instant::now();
            let source_neighbors =
                neighbor_sources.context_for_box(settings, min_x, max_x, min_z, max_z);
            if let Some(profile) = profile.as_mut() {
                profile.record(
                    feature_name,
                    FeatureProfilePhase::NeighborLoad,
                    context_start.elapsed(),
                );
            }
            let source_context: Vec<_> = source_neighbors
                .iter()
                .copied()
                .chain(std::iter::once((target_origin_x, target_origin_z, &*target_chunk)))
                .collect();
            if self.config.place_resolved_with_neighbors(
                settings,
                source_origin_x,
                source_origin_z,
                source_chunk,
                &source_context,
                random,
                world_x,
                world_y,
                world_z,
                shape,
            ) {
                let target_context: Vec<_> = source_neighbors
                    .iter()
                    .copied()
                    .chain(std::iter::once((source_origin_x, source_origin_z, &*source_chunk)))
                    .collect();
                self.config.place_spillover_with_neighbors(
                    settings,
                    target_origin_x,
                    target_origin_z,
                    target_chunk,
                    &target_context,
                    &mut replay_random,
                    world_x,
                    world_y,
                    world_z,
                    shape,
                );
            }
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

#[derive(Debug, Clone)]
struct PlacedCaveVinesFeature {
    step_index: i32,
    feature_index: i32,
    count: OrePlacementCount,
    height: OreHeight,
    search_range: i32,
    random_y_offset: i32,
    config: CaveVinesFeatureConfig,
    biome_filter: FeatureBiomeFilter,
}

impl PlacedCaveVinesFeature {
    fn new(feature_index: i32) -> Self {
        Self {
            step_index: 9,
            feature_index,
            count: OrePlacementCount::Constant(188),
            height: OreHeight::Uniform(HeightAnchor::AboveBottom(0), HeightAnchor::Absolute(256)),
            search_range: 12,
            random_y_offset: -1,
            config: CaveVinesFeatureConfig::new(),
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
            let Some(ceiling_y) = scan_up_to_solid(
                settings,
                chunk,
                origin_x,
                origin_z,
                world_x,
                start_y,
                world_z,
                self.search_range,
            ) else {
                continue;
            };
            let world_y = ceiling_y + self.random_y_offset;
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
}

#[derive(Debug, Clone)]
struct PlacedSporeBlossomFeature {
    step_index: i32,
    feature_index: i32,
    count: OrePlacementCount,
    height: OreHeight,
    search_range: i32,
    random_y_offset: i32,
    block: BlockLayer,
    biome_filter: FeatureBiomeFilter,
}

#[derive(Debug, Clone)]
struct PlacedClassicVinesFeature {
    step_index: i32,
    feature_index: i32,
    count: OrePlacementCount,
    height: OreHeight,
    block: BlockLayer,
    biome_filter: FeatureBiomeFilter,
}

impl PlacedClassicVinesFeature {
    fn cave(feature_index: i32) -> Self {
        Self {
            step_index: 9,
            feature_index,
            count: OrePlacementCount::Constant(256),
            height: OreHeight::Uniform(HeightAnchor::AboveBottom(0), HeightAnchor::Absolute(256)),
            block: BlockLayer::with_properties(
                "minecraft:vine",
                &[
                    ("east", "false"),
                    ("north", "false"),
                    ("south", "false"),
                    ("up", "false"),
                    ("west", "false"),
                ],
            ),
            biome_filter: FeatureBiomeFilter::Include(LUSH_CAVES_BIOMES),
        }
    }

    fn surface(feature_index: i32) -> Self {
        Self {
            step_index: 9,
            feature_index,
            count: OrePlacementCount::Constant(127),
            height: OreHeight::Uniform(HeightAnchor::Absolute(64), HeightAnchor::Absolute(100)),
            block: BlockLayer::with_properties(
                "minecraft:vine",
                &[
                    ("east", "false"),
                    ("north", "false"),
                    ("south", "false"),
                    ("up", "false"),
                    ("west", "false"),
                ],
            ),
            biome_filter: FeatureBiomeFilter::Include(SURFACE_VINES_BIOMES),
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
            self.place_at(
                settings, origin_x, origin_z, chunk, random, world_x, world_y, world_z,
            );
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
            self.place_at_with_neighbors(
                settings, origin_x, origin_z, chunk, neighbors, random, world_x, world_y, world_z,
            );
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
        self.place_at_in_context(
            settings,
            chunk_min_x,
            chunk_min_z,
            chunk,
            &[],
            false,
            random,
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
        self.place_at_in_context(
            settings,
            chunk_min_x,
            chunk_min_z,
            chunk,
            neighbors,
            true,
            random,
            world_x,
            world_y,
            world_z,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn place_at_in_context(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        neighbors: &[(i32, i32, &NoiseChunkBlocks)],
        use_terrain_fallback: bool,
        random: &mut FeatureRandom,
        world_x: i32,
        world_y: i32,
        world_z: i32,
    ) -> bool {
        if !is_air_at_world(
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

        for face in shuffled_horizontal_vine_faces(random) {
            let neighbor = direction_by_name(direction_opposite_name(face));
            if !is_solid_at_world_in_context(
                settings,
                chunk,
                chunk_min_x,
                chunk_min_z,
                neighbors,
                use_terrain_fallback,
                world_x + neighbor.0,
                world_y + neighbor.1,
                world_z + neighbor.2,
            ) {
                continue;
            }
            let Some((local_x, local_z)) = local_coords(world_x, world_z, chunk_min_x, chunk_min_z)
            else {
                return false;
            };
            chunk.set_layer(
                local_x,
                world_y,
                local_z,
                settings.min_y,
                self.block.with_property(face, "true"),
            );
            return true;
        }

        false
    }
}

#[allow(clippy::too_many_arguments)]
fn is_solid_at_world_in_context(
    settings: &NoiseSettings,
    chunk: &NoiseChunkBlocks,
    chunk_min_x: i32,
    chunk_min_z: i32,
    neighbors: &[(i32, i32, &NoiseChunkBlocks)],
    use_terrain_fallback: bool,
    world_x: i32,
    world_y: i32,
    world_z: i32,
) -> bool {
    if let Some(layer) = layer_at_world(
        chunk,
        chunk_min_x,
        chunk_min_z,
        world_x,
        world_y,
        world_z,
        settings.min_y,
    ) {
        return is_full_solid_layer(layer);
    }

    for (neighbor_min_x, neighbor_min_z, neighbor_chunk) in neighbors {
        if let Some(layer) = layer_at_world(
            neighbor_chunk,
            *neighbor_min_x,
            *neighbor_min_z,
            world_x,
            world_y,
            world_z,
            settings.min_y,
        ) {
            return is_full_solid_layer(layer);
        }
    }

    use_terrain_fallback
        && settings
            .terrain_layer_at(world_x, world_y, world_z)
            .is_some_and(|layer| is_full_solid_layer(&layer))
}

impl PlacedSporeBlossomFeature {
    fn new(feature_index: i32) -> Self {
        Self {
            step_index: 9,
            feature_index,
            count: OrePlacementCount::Constant(25),
            height: OreHeight::Uniform(HeightAnchor::AboveBottom(0), HeightAnchor::Absolute(256)),
            search_range: 12,
            random_y_offset: -1,
            block: BlockLayer::new("minecraft:spore_blossom"),
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
            let Some(ceiling_y) = scan_up_to_solid(
                settings,
                chunk,
                origin_x,
                origin_z,
                world_x,
                start_y,
                world_z,
                self.search_range,
            ) else {
                continue;
            };
            let world_y = ceiling_y + self.random_y_offset;
            if !self
                .biome_filter
                .allows_at(&settings.density, world_x, world_y, world_z)
                || !is_air_at_world(
                    chunk,
                    origin_x,
                    origin_z,
                    world_x,
                    world_y,
                    world_z,
                    settings.min_y,
                )
            {
                continue;
            }
            let Some((local_x, local_z)) = local_coords(world_x, world_z, origin_x, origin_z)
            else {
                continue;
            };
            chunk.set_layer(local_x, world_y, local_z, settings.min_y, self.block.clone());
        }
    }
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
            self.config.place_with_neighbors(
                settings, origin_x, origin_z, chunk, neighbors, random, world_x, world_y, world_z,
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
            let local_x = (world_x - source_origin_x) as usize;
            let local_z = (world_z - source_origin_z) as usize;
            let ocean_floor = source_chunk.ocean_floor_wg_height(local_x, local_z, settings.min_y);
            if world_y > ocean_floor + self.max_below_ocean_floor
                || !self
                    .biome_filter
                    .allows_at(&settings.density, world_x, world_y, world_z)
            {
                continue;
            }

            let mut replay_random = random.clone();
            self.config.place_with_neighbors(
                settings,
                source_origin_x,
                source_origin_z,
                source_chunk,
                &[(target_origin_x, target_origin_z, &*target_chunk)],
                random,
                world_x,
                world_y,
                world_z,
            );
            self.config.replay_place_with_neighbors(
                settings,
                target_origin_x,
                target_origin_z,
                target_chunk,
                &[(source_origin_x, source_origin_z, &*source_chunk)],
                &mut replay_random,
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
            let _world_y = self.height.sample(settings, random);
            let radius = self.config.search_range;
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
        false
    }
}

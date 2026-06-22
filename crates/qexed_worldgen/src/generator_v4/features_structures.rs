#[derive(Debug, Clone)]
enum PlacedStructureFeature {
    DesertWell(PlacedDesertWellFeature),
    Fossil(PlacedFossilFeature),
}

impl PlacedStructureFeature {
    fn desert_well(feature_index: i32) -> Self {
        Self::DesertWell(PlacedDesertWellFeature::new(feature_index))
    }

    fn fossil_upper(feature_index: i32) -> Self {
        Self::Fossil(PlacedFossilFeature::upper(feature_index))
    }

    fn fossil_lower(feature_index: i32) -> Self {
        Self::Fossil(PlacedFossilFeature::lower(feature_index))
    }

    fn step_index(&self) -> i32 {
        match self {
            Self::DesertWell(feature) => feature.step_index,
            Self::Fossil(feature) => feature.step_index,
        }
    }

    fn feature_index(&self) -> i32 {
        match self {
            Self::DesertWell(feature) => feature.feature_index,
            Self::Fossil(feature) => feature.feature_index,
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
            Self::DesertWell(feature) => feature.place(settings, origin_x, origin_z, chunk, random),
            Self::Fossil(feature) => feature.place(settings, origin_x, origin_z, chunk, random),
        }
    }

    fn place_with_neighbors(
        &self,
        settings: &NoiseSettings,
        origin_x: i32,
        origin_z: i32,
        chunk: &mut NoiseChunkBlocks,
        neighbors: &[(i32, i32, &NoiseChunkBlocks)],
        random: &mut FeatureRandom,
    ) {
        match self {
            Self::DesertWell(feature) => {
                feature.place_with_neighbors(settings, origin_x, origin_z, chunk, neighbors, random)
            }
            Self::Fossil(feature) => feature.place(settings, origin_x, origin_z, chunk, random),
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
            Self::DesertWell(feature) => feature.place_with_spillover(
                settings,
                source_origin_x,
                source_origin_z,
                target_origin_x,
                target_origin_z,
                source_chunk,
                target_chunk,
                random,
            ),
            Self::Fossil(feature) => feature.place_with_spillover(
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

    #[allow(clippy::too_many_arguments)]
    fn place_with_spillover_neighbors(
        &self,
        settings: &NoiseSettings,
        source_origin_x: i32,
        source_origin_z: i32,
        target_origin_x: i32,
        target_origin_z: i32,
        source_chunk: &mut NoiseChunkBlocks,
        target_chunk: &mut NoiseChunkBlocks,
        source_neighbors: &[(i32, i32, &NoiseChunkBlocks)],
        random: &mut FeatureRandom,
    ) {
        match self {
            Self::DesertWell(feature) => feature.place_with_spillover_neighbors(
                settings,
                source_origin_x,
                source_origin_z,
                target_origin_x,
                target_origin_z,
                source_chunk,
                target_chunk,
                source_neighbors,
                random,
            ),
            Self::Fossil(feature) => feature.place_with_spillover(
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
            Self::DesertWell(feature) => feature.may_spill_into(
                source_origin_x,
                source_origin_z,
                target_origin_x,
                target_origin_z,
                random,
            ),
            Self::Fossil(feature) => feature.may_spill_into(
                settings,
                source_origin_x,
                source_origin_z,
                target_origin_x,
                target_origin_z,
                random,
            ),
        }
    }
}

#[derive(Debug, Clone)]
struct PlacedDesertWellFeature {
    step_index: i32,
    feature_index: i32,
    rarity: i32,
    config: DesertWellFeatureConfig,
    biome_filter: FeatureBiomeFilter,
}

impl PlacedDesertWellFeature {
    fn new(feature_index: i32) -> Self {
        Self {
            step_index: 4,
            feature_index,
            rarity: 1000,
            config: DesertWellFeatureConfig::new(),
            biome_filter: FeatureBiomeFilter::Include(DESERT_WELL_BIOMES),
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
        let Some((local_x, local_z)) = local_coords(world_x, world_z, origin_x, origin_z) else {
            return;
        };
        let world_y = chunk.world_surface_wg_height(local_x, local_z, settings.min_y);
        if world_y <= settings.min_y
            || !self
                .biome_filter
                .allows_at(&settings.density, world_x, world_y, world_z)
        {
            return;
        }
        self.config.place(
            settings, origin_x, origin_z, chunk, random, world_x, world_y, world_z,
        );
    }

    fn place_with_neighbors(
        &self,
        settings: &NoiseSettings,
        origin_x: i32,
        origin_z: i32,
        chunk: &mut NoiseChunkBlocks,
        neighbors: &[(i32, i32, &NoiseChunkBlocks)],
        random: &mut FeatureRandom,
    ) {
        if random.next_float() >= 1.0 / self.rarity as f32 {
            return;
        }
        let world_x = origin_x + random.next_int(16);
        let world_z = origin_z + random.next_int(16);
        let Some((local_x, local_z)) = local_coords(world_x, world_z, origin_x, origin_z) else {
            return;
        };
        let world_y = chunk.world_surface_wg_height(local_x, local_z, settings.min_y);
        if world_y <= settings.min_y
            || !self
                .biome_filter
                .allows_at(&settings.density, world_x, world_y, world_z)
        {
            return;
        }
        self.config.place_with_neighbors(
            settings, origin_x, origin_z, chunk, neighbors, random, world_x, world_y, world_z,
        );
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
        if random.next_float() >= 1.0 / self.rarity as f32 {
            return;
        }
        let world_x = source_origin_x + random.next_int(16);
        let world_z = source_origin_z + random.next_int(16);
        let Some((local_x, local_z)) =
            local_coords(world_x, world_z, source_origin_x, source_origin_z)
        else {
            return;
        };
        let world_y = source_chunk.world_surface_wg_height(local_x, local_z, settings.min_y);
        if world_y <= settings.min_y
            || !self
                .biome_filter
                .allows_at(&settings.density, world_x, world_y, world_z)
        {
            return;
        }
        let Some(origin_y) = self.config.resolve_origin_y(
            settings,
            source_origin_x,
            source_origin_z,
            source_chunk,
            world_x,
            world_y,
            world_z,
        ) else {
            return;
        };

        let mut replay_random = random.clone();
        self.config.place_resolved(
            settings,
            source_origin_x,
            source_origin_z,
            source_chunk,
            random,
            world_x,
            origin_y,
            world_z,
        );
        self.config.place_resolved(
            settings,
            target_origin_x,
            target_origin_z,
            target_chunk,
            &mut replay_random,
            world_x,
            origin_y,
            world_z,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn place_with_spillover_neighbors(
        &self,
        settings: &NoiseSettings,
        source_origin_x: i32,
        source_origin_z: i32,
        target_origin_x: i32,
        target_origin_z: i32,
        source_chunk: &mut NoiseChunkBlocks,
        target_chunk: &mut NoiseChunkBlocks,
        source_neighbors: &[(i32, i32, &NoiseChunkBlocks)],
        random: &mut FeatureRandom,
    ) {
        if random.next_float() >= 1.0 / self.rarity as f32 {
            return;
        }
        let world_x = source_origin_x + random.next_int(16);
        let world_z = source_origin_z + random.next_int(16);
        let Some((local_x, local_z)) =
            local_coords(world_x, world_z, source_origin_x, source_origin_z)
        else {
            return;
        };
        let world_y = source_chunk.world_surface_wg_height(local_x, local_z, settings.min_y);
        if world_y <= settings.min_y
            || !self
                .biome_filter
                .allows_at(&settings.density, world_x, world_y, world_z)
        {
            return;
        }

        let source_context: Vec<_> = source_neighbors
            .iter()
            .copied()
            .chain(std::iter::once((target_origin_x, target_origin_z, &*target_chunk)))
            .collect();
        let Some(origin_y) = self.config.resolve_origin_y_with_neighbors(
            settings,
            source_origin_x,
            source_origin_z,
            source_chunk,
            &source_context,
            world_x,
            world_y,
            world_z,
        ) else {
            return;
        };

        let mut replay_random = random.clone();
        self.config.place_resolved(
            settings,
            source_origin_x,
            source_origin_z,
            source_chunk,
            random,
            world_x,
            origin_y,
            world_z,
        );
        self.config.place_resolved(
            settings,
            target_origin_x,
            target_origin_z,
            target_chunk,
            &mut replay_random,
            world_x,
            origin_y,
            world_z,
        );
    }

    fn may_spill_into(
        &self,
        source_origin_x: i32,
        source_origin_z: i32,
        target_origin_x: i32,
        target_origin_z: i32,
        random: &mut FeatureRandom,
    ) -> bool {
        if random.next_float() >= 1.0 / self.rarity as f32 {
            return false;
        }
        let world_x = source_origin_x + random.next_int(16);
        let world_z = source_origin_z + random.next_int(16);
        horizontal_box_overlaps_chunk(
            world_x - 2,
            world_x + 2,
            world_z - 2,
            world_z + 2,
            target_origin_x,
            target_origin_z,
        )
    }
}

#[derive(Debug, Clone)]
struct PlacedFossilFeature {
    step_index: i32,
    feature_index: i32,
    rarity: i32,
    height: OreHeight,
    config: FossilFeatureConfig,
    biome_filter: FeatureBiomeFilter,
}

impl PlacedFossilFeature {
    fn upper(feature_index: i32) -> Self {
        Self {
            step_index: 3,
            feature_index,
            rarity: 64,
            height: OreHeight::Uniform(HeightAnchor::Absolute(0), HeightAnchor::BelowTop(0)),
            config: FossilFeatureConfig::new("minecraft:coal_ore"),
            biome_filter: FeatureBiomeFilter::Include(FOSSIL_BIOMES),
        }
    }

    fn lower(feature_index: i32) -> Self {
        Self {
            step_index: 3,
            feature_index,
            rarity: 64,
            height: OreHeight::Uniform(HeightAnchor::AboveBottom(0), HeightAnchor::Absolute(-8)),
            config: FossilFeatureConfig::new("minecraft:diamond_ore"),
            biome_filter: FeatureBiomeFilter::Include(FOSSIL_BIOMES),
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
        if !self
            .biome_filter
            .allows_at(&settings.density, world_x, world_y, world_z)
        {
            return;
        }
        self.config.place(
            settings, origin_x, origin_z, chunk, random, world_x, world_y, world_z,
        );
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
        if random.next_float() >= 1.0 / self.rarity as f32 {
            return;
        }
        let world_x = source_origin_x + random.next_int(16);
        let world_z = source_origin_z + random.next_int(16);
        let world_y = self.height.sample(settings, random);
        if !self
            .biome_filter
            .allows_at(&settings.density, world_x, world_y, world_z)
        {
            return;
        }

        let mut replay_random = random.clone();
        self.config.place(
            settings,
            source_origin_x,
            source_origin_z,
            source_chunk,
            random,
            world_x,
            world_y,
            world_z,
        );
        self.config.place(
            settings,
            target_origin_x,
            target_origin_z,
            target_chunk,
            &mut replay_random,
            world_x,
            world_y,
            world_z,
        );
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
        if random.next_float() >= 1.0 / self.rarity as f32 {
            return false;
        }
        let world_x = source_origin_x + random.next_int(16);
        let world_z = source_origin_z + random.next_int(16);
        let world_y = self.height.sample(settings, random);
        if !self
            .biome_filter
            .allows_at(&settings.density, world_x, world_y, world_z)
        {
            return false;
        }
        horizontal_box_overlaps_chunk(
            world_x - 5,
            world_x + 5,
            world_z - 5,
            world_z + 5,
            target_origin_x,
            target_origin_z,
        )
    }
}

#[derive(Debug, Clone)]
struct DesertWellFeatureConfig {
    sand: BlockLayer,
    sandstone: BlockLayer,
    sand_slab: BlockLayer,
    water: BlockLayer,
    suspicious_sand: BlockLayer,
}

impl DesertWellFeatureConfig {
    fn new() -> Self {
        Self {
            sand: BlockLayer::new("minecraft:sand"),
            sandstone: BlockLayer::new("minecraft:sandstone"),
            sand_slab: BlockLayer::new("minecraft:sandstone_slab"),
            water: BlockLayer::new("minecraft:water"),
            suspicious_sand: BlockLayer::new("minecraft:suspicious_sand"),
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
        let Some(origin_y) = self.resolve_origin_y(
            settings,
            chunk_min_x,
            chunk_min_z,
            chunk,
            world_x,
            world_y,
            world_z,
        ) else {
            return false;
        };

        self.place_resolved(
            settings,
            chunk_min_x,
            chunk_min_z,
            chunk,
            random,
            world_x,
            origin_y,
            world_z,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn place_with_neighbors(
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
        let Some(origin_y) = self.resolve_origin_y_with_neighbors(
            settings,
            chunk_min_x,
            chunk_min_z,
            chunk,
            neighbors,
            world_x,
            world_y,
            world_z,
        ) else {
            return false;
        };

        self.place_resolved(
            settings,
            chunk_min_x,
            chunk_min_z,
            chunk,
            random,
            world_x,
            origin_y,
            world_z,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn resolve_origin_y(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &NoiseChunkBlocks,
        world_x: i32,
        world_y: i32,
        world_z: i32,
    ) -> Option<i32> {
        let mut origin_y = world_y + 1;
        while origin_y > settings.min_y + 2
            && is_air_at_world(
                chunk,
                chunk_min_x,
                chunk_min_z,
                world_x,
                origin_y,
                world_z,
                settings.min_y,
            )
        {
            origin_y -= 1;
        }

        if !layer_at_world(
            chunk,
            chunk_min_x,
            chunk_min_z,
            world_x,
            origin_y,
            world_z,
            settings.min_y,
        )
        .is_some_and(|layer| layer.is("minecraft:sand"))
        {
            return None;
        }

        for dx in -2..=2 {
            for dz in -2..=2 {
                let empty_below_1 = is_air_at_world(
                    chunk,
                    chunk_min_x,
                    chunk_min_z,
                    world_x + dx,
                    origin_y - 1,
                    world_z + dz,
                    settings.min_y,
                );
                let empty_below_2 = is_air_at_world(
                    chunk,
                    chunk_min_x,
                    chunk_min_z,
                    world_x + dx,
                    origin_y - 2,
                    world_z + dz,
                    settings.min_y,
                );
                if empty_below_1 && empty_below_2 {
                    return None;
                }
            }
        }

        Some(origin_y)
    }

    #[allow(clippy::too_many_arguments)]
    fn resolve_origin_y_with_neighbors(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &NoiseChunkBlocks,
        neighbors: &[(i32, i32, &NoiseChunkBlocks)],
        world_x: i32,
        world_y: i32,
        world_z: i32,
    ) -> Option<i32> {
        let mut origin_y = world_y + 1;
        while origin_y > settings.min_y + 2
            && desert_well_context_layer(
                settings,
                chunk,
                chunk_min_x,
                chunk_min_z,
                neighbors,
                world_x,
                origin_y,
                world_z,
            )
            .is_some_and(|layer| layer.is_air)
        {
            origin_y -= 1;
        }

        if !desert_well_context_layer(
            settings,
            chunk,
            chunk_min_x,
            chunk_min_z,
            neighbors,
            world_x,
            origin_y,
            world_z,
        )
        .is_some_and(|layer| layer.is("minecraft:sand"))
        {
            return None;
        }

        for dx in -2..=2 {
            for dz in -2..=2 {
                let empty_below_1 = desert_well_context_layer(
                    settings,
                    chunk,
                    chunk_min_x,
                    chunk_min_z,
                    neighbors,
                    world_x + dx,
                    origin_y - 1,
                    world_z + dz,
                )
                .is_some_and(|layer| layer.is_air);
                let empty_below_2 = desert_well_context_layer(
                    settings,
                    chunk,
                    chunk_min_x,
                    chunk_min_z,
                    neighbors,
                    world_x + dx,
                    origin_y - 2,
                    world_z + dz,
                )
                .is_some_and(|layer| layer.is_air);
                if empty_below_1 && empty_below_2 {
                    return None;
                }
            }
        }

        Some(origin_y)
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
        origin_y: i32,
        world_z: i32,
    ) -> bool {
        let mut placed = false;

        for dy in -2..=0 {
            for dx in -2..=2 {
                for dz in -2..=2 {
                    placed |= self.set_block(
                        settings,
                        chunk_min_x,
                        chunk_min_z,
                        chunk,
                        world_x + dx,
                        origin_y + dy,
                        world_z + dz,
                        self.sandstone.clone(),
                    );
                }
            }
        }

        placed |= self.set_block(
            settings,
            chunk_min_x,
            chunk_min_z,
            chunk,
            world_x,
            origin_y,
            world_z,
            self.water.clone(),
        );
        for (dx, dz) in horizontal_directions() {
            placed |= self.set_block(
                settings,
                chunk_min_x,
                chunk_min_z,
                chunk,
                world_x + dx,
                origin_y,
                world_z + dz,
                self.water.clone(),
            );
        }

        placed |= self.set_block(
            settings,
            chunk_min_x,
            chunk_min_z,
            chunk,
            world_x,
            origin_y - 1,
            world_z,
            self.sand.clone(),
        );
        for (dx, dz) in horizontal_directions() {
            placed |= self.set_block(
                settings,
                chunk_min_x,
                chunk_min_z,
                chunk,
                world_x + dx,
                origin_y - 1,
                world_z + dz,
                self.sand.clone(),
            );
        }

        for dx in -2..=2 {
            for dz in -2..=2 {
                if dx == -2 || dx == 2 || dz == -2 || dz == 2 {
                    placed |= self.set_block(
                        settings,
                        chunk_min_x,
                        chunk_min_z,
                        chunk,
                        world_x + dx,
                        origin_y + 1,
                        world_z + dz,
                        self.sandstone.clone(),
                    );
                }
            }
        }
        for (dx, dz) in [(2, 0), (-2, 0), (0, 2), (0, -2)] {
            placed |= self.set_block(
                settings,
                chunk_min_x,
                chunk_min_z,
                chunk,
                world_x + dx,
                origin_y + 1,
                world_z + dz,
                self.sand_slab.clone(),
            );
        }

        for dx in -1..=1 {
            for dz in -1..=1 {
                let block = if dx == 0 && dz == 0 {
                    self.sandstone.clone()
                } else {
                    self.sand_slab.clone()
                };
                placed |= self.set_block(
                    settings,
                    chunk_min_x,
                    chunk_min_z,
                    chunk,
                    world_x + dx,
                    origin_y + 4,
                    world_z + dz,
                    block,
                );
            }
        }

        for dy in 1..=3 {
            for (dx, dz) in [(-1, -1), (-1, 1), (1, -1), (1, 1)] {
                placed |= self.set_block(
                    settings,
                    chunk_min_x,
                    chunk_min_z,
                    chunk,
                    world_x + dx,
                    origin_y + dy,
                    world_z + dz,
                    self.sandstone.clone(),
                );
            }
        }

        let water_positions = [(0, 0), (1, 0), (0, 1), (-1, 0), (0, -1)];
        let first = water_positions[random.next_int(water_positions.len() as i32) as usize];
        let second = water_positions[random.next_int(water_positions.len() as i32) as usize];
        placed |= self.place_suspicious_sand(
            settings,
            chunk_min_x,
            chunk_min_z,
            chunk,
            world_x + first.0,
            origin_y - 1,
            world_z + first.1,
            random.next_long(),
        );
        placed |= self.place_suspicious_sand(
            settings,
            chunk_min_x,
            chunk_min_z,
            chunk,
            world_x + second.0,
            origin_y - 2,
            world_z + second.1,
            random.next_long(),
        );
        placed
    }

    #[allow(clippy::too_many_arguments)]
    fn set_block(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        world_x: i32,
        world_y: i32,
        world_z: i32,
        block: BlockLayer,
    ) -> bool {
        let Some((local_x, local_z)) = local_coords(world_x, world_z, chunk_min_x, chunk_min_z)
        else {
            return false;
        };
        if chunk.layer(local_x, world_y, local_z, settings.min_y).is_none() {
            return false;
        }
        chunk.set_layer(local_x, world_y, local_z, settings.min_y, block);
        true
    }

    #[allow(clippy::too_many_arguments)]
    fn place_suspicious_sand(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        world_x: i32,
        world_y: i32,
        world_z: i32,
        loot_table_seed: i64,
    ) -> bool {
        let Some((local_x, local_z)) = local_coords(world_x, world_z, chunk_min_x, chunk_min_z)
        else {
            return false;
        };
        if chunk.layer(local_x, world_y, local_z, settings.min_y).is_none() {
            return false;
        }
        chunk.set_layer(
            local_x,
            world_y,
            local_z,
            settings.min_y,
            self.suspicious_sand.clone(),
        );
        chunk.push_block_entity(
            world_x,
            world_y,
            world_z,
            BRUSHABLE_BLOCK_ENTITY_TYPE_ID,
            suspicious_sand_block_entity_nbt(loot_table_seed),
        );
        true
    }
}

#[derive(Debug, Clone)]
struct FossilFeatureConfig {
    overlay_block: BlockLayer,
    bone_block: BlockLayer,
}

impl FossilFeatureConfig {
    fn new(overlay_block: &str) -> Self {
        Self {
            overlay_block: BlockLayer::new(overlay_block),
            bone_block: BlockLayer::new("minecraft:bone_block"),
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
        let shape = FossilShape::sample(random);
        let rotation = random.next_int(4);
        let mut placed = false;
        for &(dx, dy, dz, overlay) in shape.blocks {
            let (rotated_x, rotated_z) = rotate_offset(dx, dz, rotation);
            let block = if overlay {
                self.overlay_block.clone()
            } else {
                self.bone_block.clone()
            };
            placed |= self.set_if_embedded(
                settings,
                chunk_min_x,
                chunk_min_z,
                chunk,
                world_x + rotated_x,
                world_y + dy,
                world_z + rotated_z,
                block,
            );
        }
        placed
    }

    #[allow(clippy::too_many_arguments)]
    fn set_if_embedded(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        world_x: i32,
        world_y: i32,
        world_z: i32,
        block: BlockLayer,
    ) -> bool {
        let Some((local_x, local_z)) = local_coords(world_x, world_z, chunk_min_x, chunk_min_z)
        else {
            return false;
        };
        let Some(current) = chunk.layer(local_x, world_y, local_z, settings.min_y) else {
            return false;
        };
        if current.is_air || current.is("minecraft:water") || current.is("minecraft:lava") {
            return false;
        }
        chunk.set_layer(local_x, world_y, local_z, settings.min_y, block);
        true
    }
}

#[derive(Clone, Copy, Debug)]
struct FossilShape {
    blocks: &'static [(i32, i32, i32, bool)],
}

impl FossilShape {
    fn sample(random: &mut FeatureRandom) -> Self {
        let shapes = [
            FossilShape {
                blocks: FOSSIL_SPINE_1,
            },
            FossilShape {
                blocks: FOSSIL_SPINE_2,
            },
            FossilShape {
                blocks: FOSSIL_SKULL_1,
            },
            FossilShape {
                blocks: FOSSIL_SKULL_2,
            },
        ];
        shapes[random.next_int(shapes.len() as i32) as usize]
    }
}

#[allow(clippy::too_many_arguments)]
fn desert_well_context_layer(
    settings: &NoiseSettings,
    chunk: &NoiseChunkBlocks,
    chunk_min_x: i32,
    chunk_min_z: i32,
    neighbors: &[(i32, i32, &NoiseChunkBlocks)],
    world_x: i32,
    world_y: i32,
    world_z: i32,
) -> Option<BlockLayer> {
    if let Some(layer) = layer_at_world(
        chunk,
        chunk_min_x,
        chunk_min_z,
        world_x,
        world_y,
        world_z,
        settings.min_y,
    ) {
        return Some(layer.clone());
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
            return Some(layer.clone());
        }
    }

    settings.terrain_layer_at(world_x, world_y, world_z)
}

fn rotate_offset(x: i32, z: i32, rotation: i32) -> (i32, i32) {
    match rotation.rem_euclid(4) {
        0 => (x, z),
        1 => (-z, x),
        2 => (-x, -z),
        _ => (z, -x),
    }
}

fn suspicious_sand_block_entity_nbt(loot_table_seed: i64) -> Tag {
    let mut fields = HashMap::new();
    fields.insert(
        "LootTable".to_string(),
        Tag::String(Arc::from("minecraft:archaeology/desert_well")),
    );
    fields.insert("LootTableSeed".to_string(), Tag::Long(loot_table_seed));
    Tag::Compound(Arc::new(fields))
}

const FOSSIL_SPINE_1: &[(i32, i32, i32, bool)] = &[
    (-4, 0, 0, false),
    (-3, 0, 0, false),
    (-2, 0, 0, false),
    (-1, 0, 0, false),
    (0, 0, 0, false),
    (1, 0, 0, false),
    (2, 0, 0, false),
    (3, 0, 0, false),
    (4, 0, 0, false),
    (-3, 0, -1, true),
    (-1, 0, -1, true),
    (1, 0, -1, true),
    (3, 0, -1, true),
    (-3, 0, 1, true),
    (-1, 0, 1, true),
    (1, 0, 1, true),
    (3, 0, 1, true),
    (-3, 1, -2, false),
    (-1, 1, -2, false),
    (1, 1, -2, false),
    (3, 1, -2, false),
    (-3, 1, 2, false),
    (-1, 1, 2, false),
    (1, 1, 2, false),
    (3, 1, 2, false),
];
const FOSSIL_SPINE_2: &[(i32, i32, i32, bool)] = &[
    (-5, 0, 0, false),
    (-4, 0, 0, false),
    (-3, 0, 0, false),
    (-2, 0, 0, false),
    (-1, 0, 0, false),
    (0, 0, 0, false),
    (1, 0, 0, false),
    (2, 0, 0, false),
    (3, 0, 0, false),
    (4, 0, 0, false),
    (5, 0, 0, false),
    (-4, 1, 0, true),
    (-2, 1, 0, true),
    (0, 1, 0, true),
    (2, 1, 0, true),
    (4, 1, 0, true),
    (-4, 0, -2, false),
    (-2, 0, -3, false),
    (0, 0, -3, false),
    (2, 0, -3, false),
    (4, 0, -2, false),
    (-4, 0, 2, false),
    (-2, 0, 3, false),
    (0, 0, 3, false),
    (2, 0, 3, false),
    (4, 0, 2, false),
];
const FOSSIL_SKULL_1: &[(i32, i32, i32, bool)] = &[
    (-2, 0, -2, false),
    (-1, 0, -2, false),
    (0, 0, -2, false),
    (1, 0, -2, false),
    (2, 0, -2, false),
    (-2, 0, -1, false),
    (2, 0, -1, false),
    (-2, 0, 0, false),
    (2, 0, 0, false),
    (-2, 0, 1, false),
    (-1, 0, 1, false),
    (0, 0, 1, false),
    (1, 0, 1, false),
    (2, 0, 1, false),
    (-1, 1, -1, true),
    (1, 1, -1, true),
    (-1, 1, 0, false),
    (0, 1, 0, false),
    (1, 1, 0, false),
];
const FOSSIL_SKULL_2: &[(i32, i32, i32, bool)] = &[
    (-3, 0, -2, false),
    (-2, 0, -2, false),
    (-1, 0, -2, false),
    (0, 0, -2, false),
    (1, 0, -2, false),
    (2, 0, -2, false),
    (3, 0, -2, false),
    (-3, 0, -1, false),
    (3, 0, -1, false),
    (-3, 0, 0, false),
    (3, 0, 0, false),
    (-2, 0, 1, false),
    (-1, 0, 1, false),
    (0, 0, 1, false),
    (1, 0, 1, false),
    (2, 0, 1, false),
    (-2, 1, -1, true),
    (2, 1, -1, true),
    (-1, 1, 0, false),
    (0, 1, 0, false),
    (1, 1, 0, false),
];

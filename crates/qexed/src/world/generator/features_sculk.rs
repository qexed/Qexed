#[derive(Debug, Clone)]
enum PlacedSculkFeature {
    Vein(PlacedSculkVeinFeature),
    Patch(PlacedSculkPatchFeature),
}

impl PlacedSculkFeature {
    fn vein(feature_index: i32) -> Self {
        Self::Vein(PlacedSculkVeinFeature::new(feature_index))
    }

    fn deep_dark_patch(feature_index: i32) -> Self {
        Self::Patch(PlacedSculkPatchFeature::deep_dark(feature_index))
    }

    fn step_index(&self) -> i32 {
        match self {
            Self::Vein(feature) => feature.step_index,
            Self::Patch(feature) => feature.step_index,
        }
    }

    fn feature_index(&self) -> i32 {
        match self {
            Self::Vein(feature) => feature.feature_index,
            Self::Patch(feature) => feature.feature_index,
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
            Self::Vein(feature) => feature.place(settings, origin_x, origin_z, chunk, random),
            Self::Patch(feature) => feature.place(settings, origin_x, origin_z, chunk, random),
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
            Self::Vein(feature) => feature.place_with_spillover(
                settings,
                source_origin_x,
                source_origin_z,
                target_origin_x,
                target_origin_z,
                source_chunk,
                target_chunk,
                random,
            ),
            Self::Patch(feature) => feature.place_with_spillover(
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
}

#[derive(Debug, Clone)]
struct PlacedSculkVeinFeature {
    step_index: i32,
    feature_index: i32,
    count: OrePlacementCount,
    height: OreHeight,
    config: MultifaceGrowthFeatureConfig,
    biome_filter: FeatureBiomeFilter,
}

impl PlacedSculkVeinFeature {
    fn new(feature_index: i32) -> Self {
        Self {
            step_index: 7,
            feature_index,
            count: OrePlacementCount::Uniform { min: 204, max: 250 },
            height: OreHeight::Uniform(HeightAnchor::AboveBottom(0), HeightAnchor::Absolute(256)),
            config: sculk_vein_growth_config(),
            biome_filter: FeatureBiomeFilter::Include(DEEP_DARK_BIOMES),
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
}

#[derive(Debug, Clone)]
struct PlacedSculkPatchFeature {
    step_index: i32,
    feature_index: i32,
    count: OrePlacementCount,
    height: OreHeight,
    config: SculkPatchFeatureConfig,
    biome_filter: FeatureBiomeFilter,
}

impl PlacedSculkPatchFeature {
    fn deep_dark(feature_index: i32) -> Self {
        Self {
            step_index: 7,
            feature_index,
            count: OrePlacementCount::Constant(256),
            height: OreHeight::Uniform(HeightAnchor::AboveBottom(0), HeightAnchor::Absolute(256)),
            config: SculkPatchFeatureConfig::deep_dark(),
            biome_filter: FeatureBiomeFilter::Include(DEEP_DARK_BIOMES),
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
struct SculkPatchFeatureConfig {
    charge_count: i32,
    amount_per_charge: i32,
    spread_attempts: i32,
    growth_rounds: i32,
    spread_rounds: i32,
    extra_rare_growths: i32,
    catalyst_chance: f32,
}

impl SculkPatchFeatureConfig {
    fn deep_dark() -> Self {
        Self {
            charge_count: 10,
            amount_per_charge: 32,
            spread_attempts: 64,
            growth_rounds: 0,
            spread_rounds: 1,
            extra_rare_growths: 0,
            catalyst_chance: 0.5,
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
        if !self.can_spread_from(
            settings,
            chunk,
            chunk_min_x,
            chunk_min_z,
            world_x,
            world_y,
            world_z,
        ) {
            return false;
        }

        let rounds = (self.spread_rounds + self.growth_rounds).max(1);
        let radius = (self.spread_attempts / 8).clamp(3, 8);
        let mut placed = false;
        for _ in 0..rounds {
            for _ in 0..self.charge_count {
                for _ in 0..self.amount_per_charge {
                    let candidate_x = world_x + random.next_int(radius * 2 + 1) - radius;
                    let candidate_y = world_y + random.next_int(7) - 3;
                    let candidate_z = world_z + random.next_int(radius * 2 + 1) - radius;
                    placed |= self.try_place_sculk(
                        settings,
                        chunk_min_x,
                        chunk_min_z,
                        chunk,
                        random,
                        candidate_x,
                        candidate_y,
                        candidate_z,
                    );
                }
            }
        }

        if random.next_float() <= self.catalyst_chance
            && is_solid_at_world(
                chunk,
                chunk_min_x,
                chunk_min_z,
                world_x,
                world_y - 1,
                world_z,
                settings.min_y,
            )
        {
            placed |= set_sculk_block(
                settings,
                chunk_min_x,
                chunk_min_z,
                chunk,
                world_x,
                world_y,
                world_z,
                sculk_catalyst_block(),
            );
        }

        for _ in 0..self.extra_rare_growths {
            let candidate_x = world_x + random.next_int(5) - 2;
            let candidate_z = world_z + random.next_int(5) - 2;
            if is_air_at_world(
                chunk,
                chunk_min_x,
                chunk_min_z,
                candidate_x,
                world_y,
                candidate_z,
                settings.min_y,
            ) && is_solid_at_world(
                chunk,
                chunk_min_x,
                chunk_min_z,
                candidate_x,
                world_y - 1,
                candidate_z,
                settings.min_y,
            ) {
                placed |= set_sculk_block(
                    settings,
                    chunk_min_x,
                    chunk_min_z,
                    chunk,
                    candidate_x,
                    world_y,
                    candidate_z,
                    sculk_shrieker_block(),
                );
            }
        }

        placed
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
    ) -> bool {
        if !self.can_spread_from_in_context(
            settings,
            source_chunk,
            source_origin_x,
            source_origin_z,
            target_chunk,
            target_origin_x,
            target_origin_z,
            world_x,
            world_y,
            world_z,
        ) {
            return false;
        }

        let rounds = (self.spread_rounds + self.growth_rounds).max(1);
        let radius = (self.spread_attempts / 8).clamp(3, 8);
        let mut placed = false;
        for _ in 0..rounds {
            for _ in 0..self.charge_count {
                for _ in 0..self.amount_per_charge {
                    let candidate_x = world_x + random.next_int(radius * 2 + 1) - radius;
                    let candidate_y = world_y + random.next_int(7) - 3;
                    let candidate_z = world_z + random.next_int(radius * 2 + 1) - radius;
                    placed |= self.try_place_sculk_in_context(
                        settings,
                        source_origin_x,
                        source_origin_z,
                        source_chunk,
                        target_origin_x,
                        target_origin_z,
                        target_chunk,
                        random,
                        candidate_x,
                        candidate_y,
                        candidate_z,
                    );
                }
            }
        }

        let catalyst_random = random.next_float();
        if catalyst_random <= self.catalyst_chance
            && is_sculk_solid_at_world_in_context(
                source_chunk,
                source_origin_x,
                source_origin_z,
                target_chunk,
                target_origin_x,
                target_origin_z,
                world_x,
                world_y - 1,
                world_z,
                settings.min_y,
            )
        {
            placed |= set_sculk_block_in_context(
                settings,
                source_origin_x,
                source_origin_z,
                source_chunk,
                target_origin_x,
                target_origin_z,
                target_chunk,
                world_x,
                world_y,
                world_z,
                sculk_catalyst_block(),
            );
        }

        for _ in 0..self.extra_rare_growths {
            let candidate_x = world_x + random.next_int(5) - 2;
            let candidate_z = world_z + random.next_int(5) - 2;
            if is_sculk_air_at_world_in_context(
                source_chunk,
                source_origin_x,
                source_origin_z,
                target_chunk,
                target_origin_x,
                target_origin_z,
                candidate_x,
                world_y,
                candidate_z,
                settings.min_y,
            ) && is_sculk_solid_at_world_in_context(
                source_chunk,
                source_origin_x,
                source_origin_z,
                target_chunk,
                target_origin_x,
                target_origin_z,
                candidate_x,
                world_y - 1,
                candidate_z,
                settings.min_y,
            ) {
                placed |= set_sculk_block_in_context(
                    settings,
                    source_origin_x,
                    source_origin_z,
                    source_chunk,
                    target_origin_x,
                    target_origin_z,
                    target_chunk,
                    candidate_x,
                    world_y,
                    candidate_z,
                    sculk_shrieker_block(),
                );
            }
        }

        placed
    }

    #[allow(clippy::too_many_arguments)]
    fn can_spread_from_in_context(
        &self,
        settings: &NoiseSettings,
        source_chunk: &NoiseChunkBlocks,
        source_min_x: i32,
        source_min_z: i32,
        target_chunk: &NoiseChunkBlocks,
        target_min_x: i32,
        target_min_z: i32,
        world_x: i32,
        world_y: i32,
        world_z: i32,
    ) -> bool {
        let Some(start) = sculk_context_layer(
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
        ) else {
            return false;
        };
        if is_sculk_layer(start) {
            return true;
        }
        if !is_air_or_water_layer(start) {
            return false;
        }
        all_directions().into_iter().any(|direction| {
            is_sculk_solid_at_world_in_context(
                source_chunk,
                source_min_x,
                source_min_z,
                target_chunk,
                target_min_x,
                target_min_z,
                world_x + direction.0,
                world_y + direction.1,
                world_z + direction.2,
                settings.min_y,
            )
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn can_spread_from(
        &self,
        settings: &NoiseSettings,
        chunk: &NoiseChunkBlocks,
        chunk_min_x: i32,
        chunk_min_z: i32,
        world_x: i32,
        world_y: i32,
        world_z: i32,
    ) -> bool {
        let Some(start) = layer_at_world(
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
        if is_sculk_layer(start) {
            return true;
        }
        if !is_air_or_water_layer(start) {
            return false;
        }
        all_directions().into_iter().any(|direction| {
            is_solid_at_world(
                chunk,
                chunk_min_x,
                chunk_min_z,
                world_x + direction.0,
                world_y + direction.1,
                world_z + direction.2,
                settings.min_y,
            )
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn try_place_sculk(
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
        let Some(current) = layer_at_world(
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

        if is_sculk_replaceable_layer(current)
            && is_adjacent_to_sculk_open_space(
                settings,
                chunk,
                chunk_min_x,
                chunk_min_z,
                world_x,
                world_y,
                world_z,
            )
        {
            let placed = set_sculk_block(
                settings,
                chunk_min_x,
                chunk_min_z,
                chunk,
                world_x,
                world_y,
                world_z,
                BlockLayer::new("minecraft:sculk"),
            );
            if placed {
                self.spread_veins_from_sculk(
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
            return placed;
        }

        if is_air_or_water_layer(current) || current.is("minecraft:sculk_vein") {
            return place_sculk_vein(
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

        false
    }

    #[allow(clippy::too_many_arguments)]
    fn try_place_sculk_in_context(
        &self,
        settings: &NoiseSettings,
        source_min_x: i32,
        source_min_z: i32,
        source_chunk: &mut NoiseChunkBlocks,
        target_min_x: i32,
        target_min_z: i32,
        target_chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
        world_x: i32,
        world_y: i32,
        world_z: i32,
    ) -> bool {
        let Some(current) = sculk_context_layer(
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
        )
        .cloned()
        else {
            return false;
        };

        if is_sculk_replaceable_layer(&current)
            && is_adjacent_to_sculk_open_space_in_context(
                settings,
                source_chunk,
                source_min_x,
                source_min_z,
                target_chunk,
                target_min_x,
                target_min_z,
                world_x,
                world_y,
                world_z,
            )
        {
            let placed = set_sculk_block_in_context(
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
                BlockLayer::new("minecraft:sculk"),
            );
            if placed {
                self.spread_veins_from_sculk_in_context(
                    settings,
                    source_min_x,
                    source_min_z,
                    source_chunk,
                    target_min_x,
                    target_min_z,
                    target_chunk,
                    random,
                    world_x,
                    world_y,
                    world_z,
                );
            }
            return placed;
        }

        if is_air_or_water_layer(&current) || current.is("minecraft:sculk_vein") {
            return place_sculk_vein_in_context(
                settings,
                source_min_x,
                source_min_z,
                source_chunk,
                target_min_x,
                target_min_z,
                target_chunk,
                random,
                world_x,
                world_y,
                world_z,
            );
        }

        false
    }

    #[allow(clippy::too_many_arguments)]
    fn spread_veins_from_sculk(
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
        for direction in shuffled_all_directions(random) {
            if random.next_float() > 0.75 {
                continue;
            }
            place_sculk_vein_face(
                settings,
                chunk_min_x,
                chunk_min_z,
                chunk,
                world_x + direction.0,
                world_y + direction.1,
                world_z + direction.2,
                direction_opposite_name(direction.3),
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn spread_veins_from_sculk_in_context(
        &self,
        settings: &NoiseSettings,
        source_min_x: i32,
        source_min_z: i32,
        source_chunk: &mut NoiseChunkBlocks,
        target_min_x: i32,
        target_min_z: i32,
        target_chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
        world_x: i32,
        world_y: i32,
        world_z: i32,
    ) {
        for direction in shuffled_all_directions(random) {
            if random.next_float() > 0.75 {
                continue;
            }
            place_sculk_vein_face_in_context(
                settings,
                source_min_x,
                source_min_z,
                source_chunk,
                target_min_x,
                target_min_z,
                target_chunk,
                world_x + direction.0,
                world_y + direction.1,
                world_z + direction.2,
                direction_opposite_name(direction.3),
            );
        }
    }
}

fn sculk_vein_growth_config() -> MultifaceGrowthFeatureConfig {
    MultifaceGrowthFeatureConfig {
        block: sculk_vein_block(None, "false"),
        search_range: 20,
        can_place_on_floor: true,
        can_place_on_ceiling: true,
        can_place_on_wall: true,
        chance_of_spreading: 1.0,
        can_be_placed_on: GLOW_LICHEN_CAN_BE_PLACED_ON,
    }
}

#[allow(clippy::too_many_arguments)]
fn place_sculk_vein(
    settings: &NoiseSettings,
    chunk_min_x: i32,
    chunk_min_z: i32,
    chunk: &mut NoiseChunkBlocks,
    random: &mut FeatureRandom,
    world_x: i32,
    world_y: i32,
    world_z: i32,
) -> bool {
    for direction in shuffled_all_directions(random) {
        if is_sculk_vein_support_at_world(
            chunk,
            chunk_min_x,
            chunk_min_z,
            world_x + direction.0,
            world_y + direction.1,
            world_z + direction.2,
            settings.min_y,
        ) && place_sculk_vein_face(
            settings,
            chunk_min_x,
            chunk_min_z,
            chunk,
            world_x,
            world_y,
            world_z,
            direction.3,
        ) {
            return true;
        }
    }
    false
}

#[allow(clippy::too_many_arguments)]
fn place_sculk_vein_in_context(
    settings: &NoiseSettings,
    source_min_x: i32,
    source_min_z: i32,
    source_chunk: &mut NoiseChunkBlocks,
    target_min_x: i32,
    target_min_z: i32,
    target_chunk: &mut NoiseChunkBlocks,
    random: &mut FeatureRandom,
    world_x: i32,
    world_y: i32,
    world_z: i32,
) -> bool {
    for direction in shuffled_all_directions(random) {
        if is_sculk_vein_support_at_world_in_context(
            source_chunk,
            source_min_x,
            source_min_z,
            target_chunk,
            target_min_x,
            target_min_z,
            world_x + direction.0,
            world_y + direction.1,
            world_z + direction.2,
            settings.min_y,
        ) && place_sculk_vein_face_in_context(
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
            direction.3,
        ) {
            return true;
        }
    }
    false
}

#[allow(clippy::too_many_arguments)]
fn place_sculk_vein_face(
    settings: &NoiseSettings,
    chunk_min_x: i32,
    chunk_min_z: i32,
    chunk: &mut NoiseChunkBlocks,
    world_x: i32,
    world_y: i32,
    world_z: i32,
    face: &str,
) -> bool {
    let Some((local_x, local_z)) = local_coords(world_x, world_z, chunk_min_x, chunk_min_z) else {
        return false;
    };
    let Some(current) = chunk.layer(local_x, world_y, local_z, settings.min_y) else {
        return false;
    };
    if !is_air_or_water_layer(current) && !current.is("minecraft:sculk_vein") {
        return false;
    }
    let waterlogged = if current.is("minecraft:water") { "true" } else { "false" };
    let block = if current.is("minecraft:sculk_vein") {
        current
            .with_property(face, "true")
            .with_property("waterlogged", waterlogged)
    } else {
        sculk_vein_block(Some(face), waterlogged)
    };
    chunk.set_layer(local_x, world_y, local_z, settings.min_y, block);
    true
}

#[allow(clippy::too_many_arguments)]
fn place_sculk_vein_face_in_context(
    settings: &NoiseSettings,
    source_min_x: i32,
    source_min_z: i32,
    source_chunk: &mut NoiseChunkBlocks,
    target_min_x: i32,
    target_min_z: i32,
    target_chunk: &mut NoiseChunkBlocks,
    world_x: i32,
    world_y: i32,
    world_z: i32,
    face: &str,
) -> bool {
    let Some(current) = sculk_context_layer(
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
    )
    .cloned()
    else {
        return false;
    };
    if !is_air_or_water_layer(&current) && !current.is("minecraft:sculk_vein") {
        return false;
    }
    let waterlogged = if current.is("minecraft:water") {
        "true"
    } else {
        "false"
    };
    let block = if current.is("minecraft:sculk_vein") {
        current
            .with_property(face, "true")
            .with_property("waterlogged", waterlogged)
    } else {
        sculk_vein_block(Some(face), waterlogged)
    };
    set_sculk_block_in_context(
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
        block,
    )
}

#[allow(clippy::too_many_arguments)]
fn set_sculk_block(
    settings: &NoiseSettings,
    chunk_min_x: i32,
    chunk_min_z: i32,
    chunk: &mut NoiseChunkBlocks,
    world_x: i32,
    world_y: i32,
    world_z: i32,
    block: BlockLayer,
) -> bool {
    let Some((local_x, local_z)) = local_coords(world_x, world_z, chunk_min_x, chunk_min_z) else {
        return false;
    };
    if chunk.layer(local_x, world_y, local_z, settings.min_y).is_none() {
        return false;
    }
    chunk.set_layer(local_x, world_y, local_z, settings.min_y, block);
    true
}

#[allow(clippy::too_many_arguments)]
fn set_sculk_block_in_context(
    settings: &NoiseSettings,
    source_min_x: i32,
    source_min_z: i32,
    source_chunk: &mut NoiseChunkBlocks,
    target_min_x: i32,
    target_min_z: i32,
    target_chunk: &mut NoiseChunkBlocks,
    world_x: i32,
    world_y: i32,
    world_z: i32,
    block: BlockLayer,
) -> bool {
    if overlaps_chunk(world_x, world_z, source_min_x, source_min_z) {
        set_sculk_block(
            settings,
            source_min_x,
            source_min_z,
            source_chunk,
            world_x,
            world_y,
            world_z,
            block,
        )
    } else if overlaps_chunk(world_x, world_z, target_min_x, target_min_z) {
        set_sculk_block(
            settings,
            target_min_x,
            target_min_z,
            target_chunk,
            world_x,
            world_y,
            world_z,
            block,
        )
    } else {
        false
    }
}

#[allow(clippy::too_many_arguments)]
fn is_adjacent_to_sculk_open_space(
    settings: &NoiseSettings,
    chunk: &NoiseChunkBlocks,
    chunk_min_x: i32,
    chunk_min_z: i32,
    world_x: i32,
    world_y: i32,
    world_z: i32,
) -> bool {
    all_directions().into_iter().any(|direction| {
        layer_at_world(
            chunk,
            chunk_min_x,
            chunk_min_z,
            world_x + direction.0,
            world_y + direction.1,
            world_z + direction.2,
            settings.min_y,
        )
        .is_some_and(is_air_or_water_layer)
    })
}

#[allow(clippy::too_many_arguments)]
fn is_adjacent_to_sculk_open_space_in_context(
    settings: &NoiseSettings,
    source_chunk: &NoiseChunkBlocks,
    source_min_x: i32,
    source_min_z: i32,
    target_chunk: &NoiseChunkBlocks,
    target_min_x: i32,
    target_min_z: i32,
    world_x: i32,
    world_y: i32,
    world_z: i32,
) -> bool {
    all_directions().into_iter().any(|direction| {
        sculk_context_layer(
            source_chunk,
            source_min_x,
            source_min_z,
            target_chunk,
            target_min_x,
            target_min_z,
            world_x + direction.0,
            world_y + direction.1,
            world_z + direction.2,
            settings.min_y,
        )
        .is_some_and(is_air_or_water_layer)
    })
}

fn is_sculk_vein_support_at_world(
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
    .is_some_and(|layer| GLOW_LICHEN_CAN_BE_PLACED_ON.contains(&layer.block.as_ref()))
}

#[allow(clippy::too_many_arguments)]
fn is_sculk_vein_support_at_world_in_context(
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
    sculk_context_layer(
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
    .is_some_and(|layer| GLOW_LICHEN_CAN_BE_PLACED_ON.contains(&layer.block.as_ref()))
}

#[allow(clippy::too_many_arguments)]
fn is_sculk_air_at_world_in_context(
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
    sculk_context_layer(
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
    .is_some_and(|layer| layer.is_air)
}

#[allow(clippy::too_many_arguments)]
fn is_sculk_solid_at_world_in_context(
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
    sculk_context_layer(
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
fn sculk_context_layer<'a>(
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

fn is_sculk_replaceable_layer(layer: &BlockLayer) -> bool {
    is_base_stone_overworld(layer) || layer.is("minecraft:dripstone_block")
}

fn is_sculk_layer(layer: &BlockLayer) -> bool {
    matches!(
        layer.block.as_ref(),
        "minecraft:sculk"
            | "minecraft:sculk_vein"
            | "minecraft:sculk_catalyst"
            | "minecraft:sculk_sensor"
            | "minecraft:sculk_shrieker"
    )
}

fn sculk_vein_block(face: Option<&str>, waterlogged: &str) -> BlockLayer {
    let mut properties = [
        ("down", "false"),
        ("east", "false"),
        ("north", "false"),
        ("south", "false"),
        ("up", "false"),
        ("waterlogged", waterlogged),
        ("west", "false"),
    ];
    if let Some(face) = face {
        if let Some((_, value)) = properties.iter_mut().find(|(name, _)| *name == face) {
            *value = "true";
        }
    }
    BlockLayer::with_properties("minecraft:sculk_vein", &properties)
}

fn sculk_catalyst_block() -> BlockLayer {
    BlockLayer::with_properties("minecraft:sculk_catalyst", &[("bloom", "false")])
}

fn sculk_shrieker_block() -> BlockLayer {
    BlockLayer::with_properties(
        "minecraft:sculk_shrieker",
        &[
            ("can_summon", "true"),
            ("shrieking", "false"),
            ("waterlogged", "false"),
        ],
    )
}

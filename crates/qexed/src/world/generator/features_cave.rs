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

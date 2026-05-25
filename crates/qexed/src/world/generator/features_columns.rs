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

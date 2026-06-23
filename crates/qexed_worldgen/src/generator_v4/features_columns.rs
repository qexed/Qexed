#[derive(Debug, Clone)]
struct PlacedBlockColumnFeature {
    step_index: i32,
    feature_index: i32,
    rarity: i32,
    outer_count: BlockColumnOuterCount,
    inner_count: i32,
    xz_offset: TrapezoidInt,
    y_offset: TrapezoidInt,
    heightmap: BlockColumnHeightmap,
    column: BlockColumnFeatureConfig,
    biome_filter: FeatureBiomeFilter,
}

impl PlacedBlockColumnFeature {
    fn sugar_cane(feature_index: i32, rarity: i32) -> Self {
        Self {
            step_index: 9,
            feature_index,
            rarity,
            outer_count: BlockColumnOuterCount::Fixed(1),
            inner_count: 20,
            xz_offset: TrapezoidInt::new(-4, 4, 0),
            y_offset: TrapezoidInt::new(0, 0, 0),
            heightmap: BlockColumnHeightmap::MotionBlocking,
            column: BlockColumnFeatureConfig::sugar_cane(),
            biome_filter: FeatureBiomeFilter::All,
        }
    }

    fn cactus(feature_index: i32, rarity: i32) -> Self {
        Self {
            step_index: 9,
            feature_index,
            rarity,
            outer_count: BlockColumnOuterCount::Fixed(1),
            inner_count: 10,
            xz_offset: TrapezoidInt::new(-7, 7, 0),
            y_offset: TrapezoidInt::new(-3, 3, 0),
            heightmap: BlockColumnHeightmap::MotionBlocking,
            column: BlockColumnFeatureConfig::cactus(),
            biome_filter: FeatureBiomeFilter::All,
        }
    }

    fn bamboo_light(feature_index: i32) -> Self {
        Self {
            step_index: 9,
            feature_index,
            rarity: 4,
            outer_count: BlockColumnOuterCount::Fixed(1),
            inner_count: 1,
            xz_offset: TrapezoidInt::new(0, 0, 0),
            y_offset: TrapezoidInt::new(0, 0, 0),
            heightmap: BlockColumnHeightmap::MotionBlocking,
            column: BlockColumnFeatureConfig::bamboo(0.0),
            biome_filter: FeatureBiomeFilter::All,
        }
    }

    fn bamboo_some_podzol(feature_index: i32) -> Self {
        Self {
            step_index: 9,
            feature_index,
            rarity: 1,
            outer_count: BlockColumnOuterCount::NoiseBased {
                noise_to_count_ratio: 160,
                noise_factor: 80.0,
                noise_offset: 0.3,
            },
            inner_count: 1,
            xz_offset: TrapezoidInt::new(0, 0, 0),
            y_offset: TrapezoidInt::new(0, 0, 0),
            heightmap: BlockColumnHeightmap::WorldSurfaceWg,
            column: BlockColumnFeatureConfig::bamboo(0.2),
            biome_filter: FeatureBiomeFilter::All,
        }
    }

    fn with_biome_filter(mut self, biome_filter: FeatureBiomeFilter) -> Self {
        self.biome_filter = biome_filter;
        self
    }

    fn max_horizontal_spillover(&self) -> i32 {
        let side_effect_radius = match self.column.kind {
            BlockColumnKind::Bamboo { podzol_probability } if podzol_probability > 0.0 => 4,
            _ => 0,
        };
        self.xz_offset
            .min
            .abs()
            .max(self.xz_offset.max.abs())
            .max(side_effect_radius)
    }

    fn place(
        &self,
        settings: &NoiseSettings,
        origin_x: i32,
        origin_z: i32,
        chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
    ) {
        for _ in 0..self.outer_count.sample(origin_x, origin_z) {
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
        for _ in 0..self.outer_count.sample(origin_x, origin_z) {
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
        for _ in 0..self.outer_count.sample(source_origin_x, source_origin_z) {
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
                let mut source_placed = false;
                if in_source {
                    source_placed = self.place_candidate_with_neighbors(
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
                if source_placed {
                    let mut spillover_random = random.clone();
                    self.column.place_side_effect_spillover(
                        settings,
                        target_origin_x,
                        target_origin_z,
                        target_chunk,
                        &mut spillover_random,
                        world_x,
                        world_z,
                    );
                }

                if in_source {
                    *random = source_random;
                } else if in_target {
                    *random = target_random;
                }
            }
        }
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
        self.column.place_at(
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
        self.column.place_at_with_neighbors(
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
}

#[derive(Debug, Clone, Copy)]
enum BlockColumnOuterCount {
    Fixed(i32),
    NoiseBased {
        noise_to_count_ratio: i32,
        noise_factor: f64,
        noise_offset: f64,
    },
}

impl BlockColumnOuterCount {
    fn sample(self, origin_x: i32, origin_z: i32) -> i32 {
        match self {
            Self::Fixed(count) => count,
            Self::NoiseBased {
                noise_to_count_ratio,
                noise_factor,
                noise_offset,
            } => ((biome_info_noise(origin_x as f64 / noise_factor, origin_z as f64 / noise_factor)
                + noise_offset)
                * noise_to_count_ratio as f64)
                .ceil()
                .max(0.0) as i32,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BlockColumnHeightmap {
    WorldSurfaceWg,
    MotionBlocking,
}

impl BlockColumnHeightmap {
    fn height(
        self,
        settings: &NoiseSettings,
        chunk: &NoiseChunkBlocks,
        local_x: usize,
        local_z: usize,
    ) -> i32 {
        match self {
            Self::WorldSurfaceWg => {
                chunk.world_surface_wg_height(local_x, local_z, settings.min_y)
            }
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

#[derive(Debug, Clone)]
struct BlockColumnFeatureConfig {
    block: BlockLayer,
    height: BiasedToBottomInt,
    tip: Option<BlockColumnTip>,
    support: BlockColumnSupport,
    kind: BlockColumnKind,
}

impl BlockColumnFeatureConfig {
    fn sugar_cane() -> Self {
        Self {
            block: BlockLayer::with_properties("minecraft:sugar_cane", &[("age", "0")]),
            height: BiasedToBottomInt { min: 2, max: 4 },
            tip: None,
            support: BlockColumnSupport::SugarCane,
            kind: BlockColumnKind::Basic,
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
            kind: BlockColumnKind::Basic,
        }
    }

    fn bamboo(podzol_probability: f32) -> Self {
        Self {
            block: BlockLayer::with_properties(
                "minecraft:bamboo",
                &[("age", "1"), ("leaves", "none"), ("stage", "0")],
            ),
            height: BiasedToBottomInt { min: 5, max: 16 },
            tip: Some(BlockColumnTip {
                block: BlockLayer::with_properties(
                    "minecraft:bamboo",
                    &[("age", "1"), ("leaves", "small"), ("stage", "0")],
                ),
                count: WeightedInt::new(&[(0, 1)]),
            }),
            support: BlockColumnSupport::Bamboo,
            kind: BlockColumnKind::Bamboo { podzol_probability },
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
        if let BlockColumnKind::Bamboo { podzol_probability } = self.kind {
            self.place_bamboo(
                settings,
                chunk_min_x,
                chunk_min_z,
                chunk,
                random,
                world_x,
                world_y,
                world_z,
                podzol_probability,
            )
        } else {
            self.place_basic(
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
        if !self.support.allows_at_world_with_neighbors(
            chunk,
            chunk_min_x,
            chunk_min_z,
            neighbors,
            world_x,
            world_y,
            world_z,
            settings.min_y,
        ) {
            return false;
        }
        if let BlockColumnKind::Bamboo { podzol_probability } = self.kind {
            self.place_bamboo(
                settings,
                chunk_min_x,
                chunk_min_z,
                chunk,
                random,
                world_x,
                world_y,
                world_z,
                podzol_probability,
            )
        } else {
            self.place_basic(
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
    }

    #[allow(clippy::too_many_arguments)]
    fn place_basic(
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

    #[allow(clippy::too_many_arguments)]
    fn place_bamboo(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
        world_x: i32,
        world_y: i32,
        world_z: i32,
        podzol_probability: f32,
    ) -> bool {
        let Some((local_x, local_z)) = local_coords(world_x, world_z, chunk_min_x, chunk_min_z)
        else {
            return false;
        };
        let height = random.next_int(12) + 5;
        let mut placed = 0;

        if random.next_float() < podzol_probability {
            self.place_bamboo_podzol(
                settings,
                chunk_min_x,
                chunk_min_z,
                chunk,
                random,
                world_x,
                world_z,
            );
        }

        for dy in 0..height {
            let y = world_y + dy;
            let Some(layer) = chunk.layer(local_x, y, local_z, settings.min_y) else {
                break;
            };
            if !layer.is_air {
                break;
            }
            chunk.set_layer(
                local_x,
                y,
                local_z,
                settings.min_y,
                self.block.clone(),
            );
            placed += 1;
        }

        if placed >= 3 {
            chunk.set_layer(
                local_x,
                world_y + placed,
                local_z,
                settings.min_y,
                BlockLayer::with_properties(
                    "minecraft:bamboo",
                    &[("age", "1"), ("leaves", "large"), ("stage", "1")],
                ),
            );
            chunk.set_layer(
                local_x,
                world_y + placed - 1,
                local_z,
                settings.min_y,
                BlockLayer::with_properties(
                    "minecraft:bamboo",
                    &[("age", "1"), ("leaves", "large"), ("stage", "0")],
                ),
            );
            chunk.set_layer(
                local_x,
                world_y + placed - 2,
                local_z,
                settings.min_y,
                BlockLayer::with_properties(
                    "minecraft:bamboo",
                    &[("age", "1"), ("leaves", "small"), ("stage", "0")],
                ),
            );
        }

        placed > 0
    }

    #[allow(clippy::too_many_arguments)]
    fn place_bamboo_podzol(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
        origin_x: i32,
        origin_z: i32,
    ) {
        let radius = random.next_int(4) + 1;
        self.place_bamboo_podzol_radius(
            settings,
            chunk_min_x,
            chunk_min_z,
            chunk,
            origin_x,
            origin_z,
            radius,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn place_side_effect_spillover(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
        origin_x: i32,
        origin_z: i32,
    ) -> bool {
        let BlockColumnKind::Bamboo { podzol_probability } = self.kind else {
            return false;
        };
        random.next_int(12);
        if random.next_float() >= podzol_probability {
            return false;
        }
        let radius = random.next_int(4) + 1;
        self.place_bamboo_podzol_radius(
            settings,
            chunk_min_x,
            chunk_min_z,
            chunk,
            origin_x,
            origin_z,
            radius,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn place_bamboo_podzol_radius(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        origin_x: i32,
        origin_z: i32,
        radius: i32,
    ) -> bool {
        let podzol = BlockLayer::new("minecraft:podzol");
        let mut placed = false;
        for world_x in origin_x - radius..=origin_x + radius {
            for world_z in origin_z - radius..=origin_z + radius {
                let dx = world_x - origin_x;
                let dz = world_z - origin_z;
                if dx * dx + dz * dz > radius * radius {
                    continue;
                }
                let Some((local_x, local_z)) =
                    local_coords(world_x, world_z, chunk_min_x, chunk_min_z)
                else {
                    continue;
                };
                let y = chunk.world_surface_wg_height(local_x, local_z, settings.min_y) - 1;
                if chunk
                    .layer(local_x, y, local_z, settings.min_y)
                    .is_some_and(supports_vegetation_layer)
                {
                    chunk.set_layer(local_x, y, local_z, settings.min_y, podzol.clone());
                    placed = true;
                }
            }
        }
        placed
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
    Bamboo,
}

#[derive(Debug, Clone, Copy)]
enum BlockColumnKind {
    Basic,
    Bamboo { podzol_probability: f32 },
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
                    layer_at_world(
                        chunk,
                        chunk_min_x,
                        chunk_min_z,
                        world_x + dx,
                        below_y,
                        world_z + dz,
                        min_y,
                    )
                    .is_some_and(supports_sugar_cane_adjacently_layer)
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
                    .is_none_or(cactus_side_is_clear)
                })
            }
            Self::Bamboo => layer_at_world(
                chunk,
                chunk_min_x,
                chunk_min_z,
                world_x,
                world_y - 1,
                world_z,
                min_y,
            )
            .is_some_and(supports_bamboo_layer),
        }
    }

    #[allow(clippy::too_many_arguments)]
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
                let Some(below) = column_context_layer(
                    chunk,
                    chunk_min_x,
                    chunk_min_z,
                    neighbors,
                    world_x,
                    below_y,
                    world_z,
                    min_y,
                ) else {
                    return false;
                };
                supports_sugar_cane_layer(&below)
                    && horizontal_directions().iter().any(|(dx, dz)| {
                        column_context_layer(
                            chunk,
                            chunk_min_x,
                            chunk_min_z,
                            neighbors,
                            world_x + dx,
                            below_y,
                            world_z + dz,
                            min_y,
                        )
                        .is_some_and(|layer| supports_sugar_cane_adjacently_layer(&layer))
                    })
            }
            Self::Cactus => {
                let Some(below) = column_context_layer(
                    chunk,
                    chunk_min_x,
                    chunk_min_z,
                    neighbors,
                    world_x,
                    world_y - 1,
                    world_z,
                    min_y,
                ) else {
                    return false;
                };
                supports_cactus_layer(&below)
                    && horizontal_directions().iter().all(|(dx, dz)| {
                        column_context_layer(
                            chunk,
                            chunk_min_x,
                            chunk_min_z,
                            neighbors,
                            world_x + dx,
                            world_y,
                            world_z + dz,
                            min_y,
                        )
                        .is_none_or(|layer| cactus_side_is_clear(&layer))
                    })
            }
            Self::Bamboo => column_context_layer(
                chunk,
                chunk_min_x,
                chunk_min_z,
                neighbors,
                world_x,
                world_y - 1,
                world_z,
                min_y,
            )
            .is_some_and(|layer| supports_bamboo_layer(&layer)),
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn column_context_layer(
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

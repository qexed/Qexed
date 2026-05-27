#[derive(Debug, Clone)]
struct PlacedAquaticFeature {
    step_index: i32,
    feature_index: i32,
    placement: AquaticPlacement,
    config: AquaticFeatureConfig,
    biome_filter: FeatureBiomeFilter,
}

impl PlacedAquaticFeature {
    fn seagrass(feature_index: i32, count: i32, tall_probability: f32) -> Self {
        Self {
            step_index: 9,
            feature_index,
            placement: AquaticPlacement::Count(count),
            config: AquaticFeatureConfig::seagrass(tall_probability),
            biome_filter: FeatureBiomeFilter::All,
        }
    }

    fn kelp(feature_index: i32, noise_to_count_ratio: i32) -> Self {
        Self {
            step_index: 9,
            feature_index,
            placement: AquaticPlacement::NoiseBasedCount {
                noise_to_count_ratio,
                noise_factor: 80.0,
                noise_offset: 0.0,
            },
            config: AquaticFeatureConfig::kelp(),
            biome_filter: FeatureBiomeFilter::All,
        }
    }

    fn sea_pickle(feature_index: i32) -> Self {
        Self {
            step_index: 9,
            feature_index,
            placement: AquaticPlacement::Rarity { chance: 16 },
            config: AquaticFeatureConfig::sea_pickle(20),
            biome_filter: FeatureBiomeFilter::All,
        }
    }

    fn warm_ocean_vegetation(feature_index: i32) -> Self {
        Self {
            step_index: 9,
            feature_index,
            placement: AquaticPlacement::NoiseBasedCount {
                noise_to_count_ratio: 20,
                noise_factor: 400.0,
                noise_offset: 0.0,
            },
            config: AquaticFeatureConfig::coral(),
            biome_filter: FeatureBiomeFilter::All,
        }
    }

    fn with_biome_filter(mut self, biome_filter: FeatureBiomeFilter) -> Self {
        self.biome_filter = biome_filter;
        self
    }

    fn max_horizontal_spillover(&self) -> i32 {
        match &self.config {
            AquaticFeatureConfig::Coral(_) => 8,
            _ => 0,
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
        for _ in 0..self.placement.sample(origin_x, origin_z, random) {
            let world_x = origin_x + random.next_int(16);
            let world_z = origin_z + random.next_int(16);
            let Some((local_x, local_z)) = local_coords(world_x, world_z, origin_x, origin_z)
            else {
                continue;
            };
            let world_y = chunk.ocean_floor_wg_height(local_x, local_z, settings.min_y);
            if world_y <= settings.min_y
                || !self
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
        for _ in 0..self.placement.sample(source_origin_x, source_origin_z, random) {
            let world_x = source_origin_x + random.next_int(16);
            let world_z = source_origin_z + random.next_int(16);
            let Some((local_x, local_z)) =
                local_coords(world_x, world_z, source_origin_x, source_origin_z)
            else {
                continue;
            };
            let world_y = source_chunk.ocean_floor_wg_height(local_x, local_z, settings.min_y);
            if world_y <= settings.min_y
                || !self
                    .biome_filter
                    .allows_at(&settings.density, world_x, world_y, world_z)
            {
                continue;
            }

            match &self.config {
                AquaticFeatureConfig::Coral(config) => {
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
                            Some((source_origin_x, source_origin_z, &*source_chunk)),
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
                _ => {
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
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum AquaticPlacement {
    Count(i32),
    Rarity {
        chance: i32,
    },
    NoiseBasedCount {
        noise_to_count_ratio: i32,
        noise_factor: f64,
        noise_offset: f64,
    },
}

impl AquaticPlacement {
    fn sample(self, origin_x: i32, origin_z: i32, random: &mut FeatureRandom) -> i32 {
        match self {
            Self::Count(count) => count,
            Self::Rarity { chance } => (random.next_float() < 1.0 / chance as f32) as i32,
            Self::NoiseBasedCount {
                noise_to_count_ratio,
                noise_factor,
                noise_offset,
            } => {
                let noise = biome_info_noise(origin_x as f64 / noise_factor, origin_z as f64 / noise_factor);
                ((noise + noise_offset + 1.0) / 2.0 * noise_to_count_ratio as f64).ceil() as i32
            }
        }
    }
}

#[derive(Debug, Clone)]
enum AquaticFeatureConfig {
    Seagrass {
        short: BlockLayer,
        tall_lower: BlockLayer,
        tall_upper: BlockLayer,
        tall_probability: f32,
    },
    Kelp {
        body: BlockLayer,
        top: BlockLayer,
    },
    SeaPickle {
        block: BlockLayer,
        count: i32,
    },
    Coral(CoralFeatureConfig),
}

impl AquaticFeatureConfig {
    fn seagrass(tall_probability: f32) -> Self {
        Self::Seagrass {
            short: BlockLayer::new("minecraft:seagrass"),
            tall_lower: BlockLayer::with_properties("minecraft:tall_seagrass", &[("half", "lower")]),
            tall_upper: BlockLayer::with_properties("minecraft:tall_seagrass", &[("half", "upper")]),
            tall_probability,
        }
    }

    fn kelp() -> Self {
        Self::Kelp {
            body: BlockLayer::new("minecraft:kelp_plant"),
            top: BlockLayer::with_properties("minecraft:kelp", &[("age", "20")]),
        }
    }

    fn sea_pickle(count: i32) -> Self {
        Self::SeaPickle {
            block: BlockLayer::with_properties(
                "minecraft:sea_pickle",
                &[("pickles", "1"), ("waterlogged", "true")],
            ),
            count,
        }
    }

    fn coral() -> Self {
        Self::Coral(CoralFeatureConfig::new())
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
            Self::Seagrass {
                short,
                tall_lower,
                tall_upper,
                tall_probability,
            } => place_seagrass(
                settings,
                chunk_min_x,
                chunk_min_z,
                chunk,
                random,
                world_x,
                world_y,
                world_z,
                short,
                tall_lower,
                tall_upper,
                *tall_probability,
            ),
            Self::Kelp { body, top } => place_kelp(
                settings,
                chunk_min_x,
                chunk_min_z,
                chunk,
                random,
                world_x,
                world_y,
                world_z,
                body,
                top,
            ),
            Self::SeaPickle { block, count } => place_sea_pickle(
                settings,
                chunk_min_x,
                chunk_min_z,
                chunk,
                random,
                world_x,
                world_y,
                world_z,
                block,
                *count,
            ),
            Self::Coral(config) => config.place(
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

}

#[allow(clippy::too_many_arguments)]
fn place_seagrass(
    settings: &NoiseSettings,
    chunk_min_x: i32,
    chunk_min_z: i32,
    chunk: &mut NoiseChunkBlocks,
    random: &mut FeatureRandom,
    world_x: i32,
    world_y: i32,
    world_z: i32,
    short: &BlockLayer,
    tall_lower: &BlockLayer,
    tall_upper: &BlockLayer,
    tall_probability: f32,
) -> bool {
    let Some((local_x, local_z)) = local_coords(world_x, world_z, chunk_min_x, chunk_min_z) else {
        return false;
    };
    if !supports_underwater_vegetation_at_world(
        chunk,
        chunk_min_x,
        chunk_min_z,
        world_x,
        world_y - 1,
        world_z,
        settings.min_y,
    ) || !is_water_at_world(chunk, chunk_min_x, chunk_min_z, world_x, world_y, world_z, settings.min_y)
    {
        return false;
    }

    if random.next_float() < tall_probability
        && is_water_at_world(
            chunk,
            chunk_min_x,
            chunk_min_z,
            world_x,
            world_y + 1,
            world_z,
            settings.min_y,
        )
    {
        chunk.set_layer(local_x, world_y, local_z, settings.min_y, tall_lower.clone());
        chunk.set_layer(
            local_x,
            world_y + 1,
            local_z,
            settings.min_y,
            tall_upper.clone(),
        );
    } else {
        chunk.set_layer(local_x, world_y, local_z, settings.min_y, short.clone());
    }
    true
}

#[allow(clippy::too_many_arguments)]
fn place_kelp(
    settings: &NoiseSettings,
    chunk_min_x: i32,
    chunk_min_z: i32,
    chunk: &mut NoiseChunkBlocks,
    random: &mut FeatureRandom,
    world_x: i32,
    world_y: i32,
    world_z: i32,
    body: &BlockLayer,
    top: &BlockLayer,
) -> bool {
    let Some((local_x, local_z)) = local_coords(world_x, world_z, chunk_min_x, chunk_min_z) else {
        return false;
    };
    if !supports_underwater_vegetation_at_world(
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

    let max_height = 1 + random.next_int(10);
    let mut height = 0;
    for dy in 0..max_height {
        let y = world_y + dy;
        if !is_water_at_world(chunk, chunk_min_x, chunk_min_z, world_x, y, world_z, settings.min_y)
        {
            break;
        }
        height += 1;
    }
    if height == 0 {
        return false;
    }

    for dy in 0..height {
        let block = if dy == height - 1 {
            top.with_property("age", &(20 + random.next_int(4)).to_string())
        } else {
            body.clone()
        };
        chunk.set_layer(local_x, world_y + dy, local_z, settings.min_y, block);
    }
    true
}

#[allow(clippy::too_many_arguments)]
fn place_sea_pickle(
    settings: &NoiseSettings,
    chunk_min_x: i32,
    chunk_min_z: i32,
    chunk: &mut NoiseChunkBlocks,
    random: &mut FeatureRandom,
    world_x: i32,
    world_y: i32,
    world_z: i32,
    block: &BlockLayer,
    count: i32,
) -> bool {
    let Some((local_x, local_z)) = local_coords(world_x, world_z, chunk_min_x, chunk_min_z) else {
        return false;
    };
    if !supports_underwater_vegetation_at_world(
        chunk,
        chunk_min_x,
        chunk_min_z,
        world_x,
        world_y - 1,
        world_z,
        settings.min_y,
    ) || !is_water_at_world(chunk, chunk_min_x, chunk_min_z, world_x, world_y, world_z, settings.min_y)
    {
        return false;
    }

    let pickles = 1 + random.next_int(count.min(4));
    chunk.set_layer(
        local_x,
        world_y,
        local_z,
        settings.min_y,
        block.with_property("pickles", &pickles.to_string()),
    );
    true
}

#[derive(Debug, Clone)]
struct CoralFeatureConfig {
    coral_blocks: &'static [&'static str],
    coral_fans: &'static [&'static str],
    coral_wall_fans: &'static [&'static str],
}

impl CoralFeatureConfig {
    fn new() -> Self {
        Self {
            coral_blocks: CORAL_BLOCKS,
            coral_fans: CORAL_FANS,
            coral_wall_fans: CORAL_WALL_FANS,
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
        let coral_block = self.random_coral_block(random);
        match random.next_int(3) {
            0 => self.place_tree(
                settings,
                chunk_min_x,
                chunk_min_z,
                chunk,
                random,
                world_x,
                world_y,
                world_z,
                coral_block,
            ),
            1 => self.place_claw(
                settings,
                chunk_min_x,
                chunk_min_z,
                chunk,
                random,
                world_x,
                world_y,
                world_z,
                coral_block,
            ),
            _ => self.place_mushroom(
                settings,
                chunk_min_x,
                chunk_min_z,
                chunk,
                random,
                world_x,
                world_y,
                world_z,
                coral_block,
            ),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn place_spillover(
        &self,
        settings: &NoiseSettings,
        source_context: Option<(i32, i32, &NoiseChunkBlocks)>,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
        world_x: i32,
        world_y: i32,
        world_z: i32,
    ) -> bool {
        let coral_block = self.random_coral_block(random);
        match random.next_int(3) {
            0 => self.place_tree_spillover(
                settings,
                source_context,
                chunk_min_x,
                chunk_min_z,
                chunk,
                random,
                world_x,
                world_y,
                world_z,
                coral_block,
            ),
            1 => self.place_claw_spillover(
                settings,
                source_context,
                chunk_min_x,
                chunk_min_z,
                chunk,
                random,
                world_x,
                world_y,
                world_z,
                coral_block,
            ),
            _ => self.place_mushroom_spillover(
                settings,
                source_context,
                chunk_min_x,
                chunk_min_z,
                chunk,
                random,
                world_x,
                world_y,
                world_z,
                coral_block,
            ),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn place_tree(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
        world_x: i32,
        world_y: i32,
        world_z: i32,
        coral_block: &str,
    ) -> bool {
        let trunk_height = random.next_int(3) + 1;
        let mut y = world_y;
        let mut placed = false;
        for _ in 0..trunk_height {
            if !self.place_coral_block(
                settings,
                chunk_min_x,
                chunk_min_z,
                chunk,
                random,
                world_x,
                y,
                world_z,
                coral_block,
            ) {
                return placed;
            }
            placed = true;
            y += 1;
        }

        let top_y = y;
        let branches = random.next_int(3) + 2;
        let mut directions = horizontal_directions().to_vec();
        shuffle_horizontal_directions(&mut directions, random);
        for (dx, dz) in directions.into_iter().take(branches as usize) {
            let mut branch_x = world_x + dx;
            let mut branch_y = top_y;
            let mut branch_z = world_z + dz;
            let branch_height = random.next_int(5) + 2;
            let mut segment_length = 0;
            for j in 0..branch_height {
                if !self.place_coral_block(
                    settings,
                    chunk_min_x,
                    chunk_min_z,
                    chunk,
                    random,
                    branch_x,
                    branch_y,
                    branch_z,
                    coral_block,
                ) {
                    break;
                }
                placed = true;
                segment_length += 1;
                branch_y += 1;
                if j == 0 || segment_length >= 2 && random.next_float() < 0.25 {
                    branch_x += dx;
                    branch_z += dz;
                    segment_length = 0;
                }
            }
        }
        placed
    }

    #[allow(clippy::too_many_arguments)]
    fn place_tree_spillover(
        &self,
        settings: &NoiseSettings,
        source_context: Option<(i32, i32, &NoiseChunkBlocks)>,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
        world_x: i32,
        world_y: i32,
        world_z: i32,
        coral_block: &str,
    ) -> bool {
        let trunk_height = random.next_int(3) + 1;
        let mut y = world_y;
        let mut placed = false;
        for _ in 0..trunk_height {
            if !self.place_coral_block_spillover(
                settings,
                source_context,
                chunk_min_x,
                chunk_min_z,
                chunk,
                random,
                world_x,
                y,
                world_z,
                coral_block,
            ) {
                return placed;
            }
            placed = true;
            y += 1;
        }

        let top_y = y;
        let branches = random.next_int(3) + 2;
        let mut directions = horizontal_directions().to_vec();
        shuffle_horizontal_directions(&mut directions, random);
        for (dx, dz) in directions.into_iter().take(branches as usize) {
            let mut branch_x = world_x + dx;
            let mut branch_y = top_y;
            let mut branch_z = world_z + dz;
            let branch_height = random.next_int(5) + 2;
            let mut segment_length = 0;
            for j in 0..branch_height {
                if !self.place_coral_block_spillover(
                    settings,
                    source_context,
                    chunk_min_x,
                    chunk_min_z,
                    chunk,
                    random,
                    branch_x,
                    branch_y,
                    branch_z,
                    coral_block,
                ) {
                    break;
                }
                placed = true;
                segment_length += 1;
                branch_y += 1;
                if j == 0 || segment_length >= 2 && random.next_float() < 0.25 {
                    branch_x += dx;
                    branch_z += dz;
                    segment_length = 0;
                }
            }
        }
        placed
    }

    #[allow(clippy::too_many_arguments)]
    fn place_claw(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
        world_x: i32,
        world_y: i32,
        world_z: i32,
        coral_block: &str,
    ) -> bool {
        if !self.place_coral_block(
            settings,
            chunk_min_x,
            chunk_min_z,
            chunk,
            random,
            world_x,
            world_y,
            world_z,
            coral_block,
        ) {
            return false;
        }

        let claw_direction = horizontal_directions()[random.next_int(4) as usize];
        let branches = random.next_int(2) + 2;
        let mut possible = [
            claw_direction,
            rotate_horizontal_direction(claw_direction, true),
            rotate_horizontal_direction(claw_direction, false),
        ];
        shuffle_horizontal_directions(&mut possible, random);

        for branch_direction in possible.into_iter().take(branches as usize) {
            let mut x = world_x + branch_direction.0;
            let mut y = world_y;
            let mut z = world_z + branch_direction.1;
            let sideway_length = random.next_int(2) + 1;
            let (segment_direction, inway_length) = if branch_direction == claw_direction {
                (claw_direction, random.next_int(3) + 2)
            } else {
                y += 1;
                let direction = if random.next_bool() {
                    branch_direction
                } else {
                    (0, 0)
                };
                (direction, random.next_int(3) + 3)
            };

            for _ in 0..sideway_length {
                if !self.place_coral_block(
                    settings,
                    chunk_min_x,
                    chunk_min_z,
                    chunk,
                    random,
                    x,
                    y,
                    z,
                    coral_block,
                ) {
                    break;
                }
                x += segment_direction.0;
                z += segment_direction.1;
                if segment_direction == (0, 0) {
                    y += 1;
                }
            }

            x -= segment_direction.0;
            z -= segment_direction.1;
            if segment_direction == (0, 0) {
                y -= 1;
            }
            y += 1;
            for _ in 0..inway_length {
                x += claw_direction.0;
                z += claw_direction.1;
                if !self.place_coral_block(
                    settings,
                    chunk_min_x,
                    chunk_min_z,
                    chunk,
                    random,
                    x,
                    y,
                    z,
                    coral_block,
                ) {
                    break;
                }
                if random.next_float() < 0.25 {
                    y += 1;
                }
            }
        }
        true
    }

    #[allow(clippy::too_many_arguments)]
    fn place_claw_spillover(
        &self,
        settings: &NoiseSettings,
        source_context: Option<(i32, i32, &NoiseChunkBlocks)>,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
        world_x: i32,
        world_y: i32,
        world_z: i32,
        coral_block: &str,
    ) -> bool {
        if !self.place_coral_block_spillover(
            settings,
            source_context,
            chunk_min_x,
            chunk_min_z,
            chunk,
            random,
            world_x,
            world_y,
            world_z,
            coral_block,
        ) {
            return false;
        }

        let claw_direction = horizontal_directions()[random.next_int(4) as usize];
        let branches = random.next_int(2) + 2;
        let mut possible = [
            claw_direction,
            rotate_horizontal_direction(claw_direction, true),
            rotate_horizontal_direction(claw_direction, false),
        ];
        shuffle_horizontal_directions(&mut possible, random);

        for branch_direction in possible.into_iter().take(branches as usize) {
            let mut x = world_x + branch_direction.0;
            let mut y = world_y;
            let mut z = world_z + branch_direction.1;
            let sideway_length = random.next_int(2) + 1;
            let (segment_direction, inway_length) = if branch_direction == claw_direction {
                (claw_direction, random.next_int(3) + 2)
            } else {
                y += 1;
                let direction = if random.next_bool() {
                    branch_direction
                } else {
                    (0, 0)
                };
                (direction, random.next_int(3) + 3)
            };

            for _ in 0..sideway_length {
                if !self.place_coral_block_spillover(
                    settings,
                    source_context,
                    chunk_min_x,
                    chunk_min_z,
                    chunk,
                    random,
                    x,
                    y,
                    z,
                    coral_block,
                ) {
                    break;
                }
                x += segment_direction.0;
                z += segment_direction.1;
                if segment_direction == (0, 0) {
                    y += 1;
                }
            }

            x -= segment_direction.0;
            z -= segment_direction.1;
            if segment_direction == (0, 0) {
                y -= 1;
            }
            y += 1;
            for _ in 0..inway_length {
                x += claw_direction.0;
                z += claw_direction.1;
                if !self.place_coral_block_spillover(
                    settings,
                    source_context,
                    chunk_min_x,
                    chunk_min_z,
                    chunk,
                    random,
                    x,
                    y,
                    z,
                    coral_block,
                ) {
                    break;
                }
                if random.next_float() < 0.25 {
                    y += 1;
                }
            }
        }
        true
    }

    #[allow(clippy::too_many_arguments)]
    fn place_mushroom(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
        world_x: i32,
        world_y: i32,
        world_z: i32,
        coral_block: &str,
    ) -> bool {
        let height = random.next_int(3) + 3;
        let width = random.next_int(3) + 3;
        let length = random.next_int(3) + 3;
        let sink = random.next_int(3) + 1;
        let mut placed = false;

        for dx in 0..=width {
            for dy in 0..=height {
                for dz in 0..=length {
                    let on_shell = dx == 0
                        || dx == width
                        || dy == 0
                        || dy == height
                        || dz == 0
                        || dz == length;
                    let not_corner = (dx != 0 && dx != width || dy != 0 && dy != height)
                        && (dz != 0 && dz != length || dy != 0 && dy != height)
                        && (dx != 0 && dx != width || dz != 0 && dz != length);
                    if on_shell && not_corner && random.next_float() >= 0.1 {
                        placed |= self.place_coral_block(
                            settings,
                            chunk_min_x,
                            chunk_min_z,
                            chunk,
                            random,
                            world_x + dx,
                            world_y + dy - sink,
                            world_z + dz,
                            coral_block,
                        );
                    }
                }
            }
        }
        placed
    }

    #[allow(clippy::too_many_arguments)]
    fn place_mushroom_spillover(
        &self,
        settings: &NoiseSettings,
        source_context: Option<(i32, i32, &NoiseChunkBlocks)>,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
        world_x: i32,
        world_y: i32,
        world_z: i32,
        coral_block: &str,
    ) -> bool {
        let height = random.next_int(3) + 3;
        let width = random.next_int(3) + 3;
        let length = random.next_int(3) + 3;
        let sink = random.next_int(3) + 1;
        let mut placed = false;

        for dx in 0..=width {
            for dy in 0..=height {
                for dz in 0..=length {
                    let on_shell = dx == 0
                        || dx == width
                        || dy == 0
                        || dy == height
                        || dz == 0
                        || dz == length;
                    let not_corner = (dx != 0 && dx != width || dy != 0 && dy != height)
                        && (dz != 0 && dz != length || dy != 0 && dy != height)
                        && (dx != 0 && dx != width || dz != 0 && dz != length);
                    if on_shell && not_corner && random.next_float() >= 0.1 {
                        placed |= self.place_coral_block_spillover(
                            settings,
                            source_context,
                            chunk_min_x,
                            chunk_min_z,
                            chunk,
                            random,
                            world_x + dx,
                            world_y + dy - sink,
                            world_z + dz,
                            coral_block,
                        );
                    }
                }
            }
        }
        placed
    }

    #[allow(clippy::too_many_arguments)]
    fn place_coral_block(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
        world_x: i32,
        world_y: i32,
        world_z: i32,
        coral_block: &str,
    ) -> bool {
        let Some((local_x, local_z)) = local_coords(world_x, world_z, chunk_min_x, chunk_min_z)
        else {
            return false;
        };
        let Some(target) = chunk.layer(local_x, world_y, local_z, settings.min_y) else {
            return false;
        };
        if !target.is("minecraft:water") && !is_coral_layer(target) {
            return false;
        }
        if !is_water_at_world(
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

        chunk.set_layer(
            local_x,
            world_y,
            local_z,
            settings.min_y,
            BlockLayer::new(coral_block),
        );
        if random.next_float() < 0.25 {
            self.place_coral_fan(
                settings,
                chunk_min_x,
                chunk_min_z,
                chunk,
                random,
                world_x,
                world_y + 1,
                world_z,
            );
        } else if random.next_float() < 0.05 {
            place_coral_sea_pickle(
                settings,
                chunk_min_x,
                chunk_min_z,
                chunk,
                random,
                world_x,
                world_y + 1,
                world_z,
            );
        }

        for (dx, dz, facing) in [
            (0, -1, "north"),
            (0, 1, "south"),
            (-1, 0, "west"),
            (1, 0, "east"),
        ] {
            if random.next_float() < 0.2
                && is_water_at_world(
                    chunk,
                    chunk_min_x,
                    chunk_min_z,
                    world_x + dx,
                    world_y,
                    world_z + dz,
                    settings.min_y,
                )
            {
                self.place_wall_coral_fan(
                    settings,
                    chunk_min_x,
                    chunk_min_z,
                    chunk,
                    random,
                    world_x + dx,
                    world_y,
                    world_z + dz,
                    facing,
                );
            }
        }
        true
    }

    #[allow(clippy::too_many_arguments)]
    fn place_coral_block_spillover(
        &self,
        settings: &NoiseSettings,
        source_context: Option<(i32, i32, &NoiseChunkBlocks)>,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
        world_x: i32,
        world_y: i32,
        world_z: i32,
        coral_block: &str,
    ) -> bool {
        if !(settings.min_y..settings.min_y + settings.height).contains(&world_y) {
            return false;
        }
        let target_local = local_coords(world_x, world_z, chunk_min_x, chunk_min_z);
        if !is_coral_replaceable_at_world_with_context(
            settings,
            source_context,
            chunk,
            chunk_min_x,
            chunk_min_z,
            world_x,
            world_y,
            world_z,
        ) {
            return false;
        }
        if !is_water_at_world_with_context(
            settings,
            source_context,
            chunk,
            chunk_min_x,
            chunk_min_z,
            world_x,
            world_y + 1,
            world_z,
        ) {
            return false;
        }

        if let Some((local_x, local_z)) = target_local {
            chunk.set_layer(
                local_x,
                world_y,
                local_z,
                settings.min_y,
                BlockLayer::new(coral_block),
            );
        }
        if random.next_float() < 0.25 {
            self.place_coral_fan_with_context(
                settings,
                source_context,
                chunk_min_x,
                chunk_min_z,
                chunk,
                random,
                world_x,
                world_y + 1,
                world_z,
            );
        } else if random.next_float() < 0.05 {
            place_coral_sea_pickle_with_context(
                settings,
                source_context,
                chunk_min_x,
                chunk_min_z,
                chunk,
                random,
                world_x,
                world_y + 1,
                world_z,
            );
        }

        for (dx, dz, facing) in [
            (0, -1, "north"),
            (0, 1, "south"),
            (-1, 0, "west"),
            (1, 0, "east"),
        ] {
            if random.next_float() < 0.2
                && is_water_at_world_with_context(
                    settings,
                    source_context,
                    chunk,
                    chunk_min_x,
                    chunk_min_z,
                    world_x + dx,
                    world_y,
                    world_z + dz,
                )
            {
                self.place_wall_coral_fan_with_context(
                    settings,
                    source_context,
                    chunk_min_x,
                    chunk_min_z,
                    chunk,
                    random,
                    world_x + dx,
                    world_y,
                    world_z + dz,
                    facing,
                );
            }
        }
        true
    }

    #[allow(clippy::too_many_arguments)]
    fn place_coral_fan(
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
        self.place_coral_fan_with_context(
            settings, None, chunk_min_x, chunk_min_z, chunk, random, world_x, world_y, world_z,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn place_coral_fan_with_context(
        &self,
        settings: &NoiseSettings,
        source_context: Option<(i32, i32, &NoiseChunkBlocks)>,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
        world_x: i32,
        world_y: i32,
        world_z: i32,
    ) -> bool {
        if !is_water_at_world_with_context(
            settings,
            source_context,
            chunk,
            chunk_min_x,
            chunk_min_z,
            world_x,
            world_y,
            world_z,
        ) {
            return false;
        }
        let block = BlockLayer::new(self.random_coral_fan(random));
        if let Some((local_x, local_z)) = local_coords(world_x, world_z, chunk_min_x, chunk_min_z) {
            chunk.set_layer(local_x, world_y, local_z, settings.min_y, block);
        }
        true
    }

    #[allow(clippy::too_many_arguments)]
    fn place_wall_coral_fan(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
        world_x: i32,
        world_y: i32,
        world_z: i32,
        facing: &str,
    ) -> bool {
        self.place_wall_coral_fan_with_context(
            settings, None, chunk_min_x, chunk_min_z, chunk, random, world_x, world_y, world_z,
            facing,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn place_wall_coral_fan_with_context(
        &self,
        settings: &NoiseSettings,
        source_context: Option<(i32, i32, &NoiseChunkBlocks)>,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
        world_x: i32,
        world_y: i32,
        world_z: i32,
        facing: &str,
    ) -> bool {
        if !is_water_at_world_with_context(
            settings,
            source_context,
            chunk,
            chunk_min_x,
            chunk_min_z,
            world_x,
            world_y,
            world_z,
        ) {
            return false;
        }
        let block =
            BlockLayer::with_properties(self.random_coral_wall_fan(random), &[("facing", facing)]);
        if let Some((local_x, local_z)) = local_coords(world_x, world_z, chunk_min_x, chunk_min_z) {
            chunk.set_layer(local_x, world_y, local_z, settings.min_y, block);
        }
        true
    }

    fn random_coral_block<'a>(&self, random: &mut FeatureRandom) -> &'a str {
        self.coral_blocks[random.next_int(self.coral_blocks.len() as i32) as usize]
    }

    fn random_coral_fan<'a>(&self, random: &mut FeatureRandom) -> &'a str {
        self.coral_fans[random.next_int(self.coral_fans.len() as i32) as usize]
    }

    fn random_coral_wall_fan<'a>(&self, random: &mut FeatureRandom) -> &'a str {
        self.coral_wall_fans[random.next_int(self.coral_wall_fans.len() as i32) as usize]
    }
}

#[allow(clippy::too_many_arguments)]
fn place_coral_sea_pickle(
    settings: &NoiseSettings,
    chunk_min_x: i32,
    chunk_min_z: i32,
    chunk: &mut NoiseChunkBlocks,
    random: &mut FeatureRandom,
    world_x: i32,
    world_y: i32,
    world_z: i32,
) -> bool {
    place_coral_sea_pickle_with_context(
        settings, None, chunk_min_x, chunk_min_z, chunk, random, world_x, world_y, world_z,
    )
}

#[allow(clippy::too_many_arguments)]
fn place_coral_sea_pickle_with_context(
    settings: &NoiseSettings,
    source_context: Option<(i32, i32, &NoiseChunkBlocks)>,
    chunk_min_x: i32,
    chunk_min_z: i32,
    chunk: &mut NoiseChunkBlocks,
    random: &mut FeatureRandom,
    world_x: i32,
    world_y: i32,
    world_z: i32,
) -> bool {
    let Some((local_x, local_z)) = local_coords(world_x, world_z, chunk_min_x, chunk_min_z) else {
        return false;
    };
    if !is_water_at_world_with_context(
        settings,
        source_context,
        chunk,
        chunk_min_x,
        chunk_min_z,
        world_x,
        world_y,
        world_z,
    ) {
        return false;
    }
    let pickles = 1 + random.next_int(4);
    chunk.set_layer(
        local_x,
        world_y,
        local_z,
        settings.min_y,
        BlockLayer::with_properties(
            "minecraft:sea_pickle",
            &[("pickles", &pickles.to_string()), ("waterlogged", "true")],
        ),
    );
    true
}

#[allow(clippy::too_many_arguments)]
fn is_water_at_world_with_context(
    settings: &NoiseSettings,
    source_context: Option<(i32, i32, &NoiseChunkBlocks)>,
    chunk: &NoiseChunkBlocks,
    chunk_min_x: i32,
    chunk_min_z: i32,
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
        return is_water_layer(layer);
    }
    if let Some((source_min_x, source_min_z, source_chunk)) = source_context
        && let Some(layer) = layer_at_world(
            source_chunk,
            source_min_x,
            source_min_z,
            world_x,
            world_y,
            world_z,
            settings.min_y,
        )
    {
        return is_water_layer(layer);
    }
    settings
        .terrain_layer_at(world_x, world_y, world_z)
        .is_some_and(|layer| is_water_layer(&layer))
}

#[allow(clippy::too_many_arguments)]
fn is_coral_replaceable_at_world_with_context(
    settings: &NoiseSettings,
    source_context: Option<(i32, i32, &NoiseChunkBlocks)>,
    chunk: &NoiseChunkBlocks,
    chunk_min_x: i32,
    chunk_min_z: i32,
    world_x: i32,
    world_y: i32,
    world_z: i32,
) -> bool {
    let matches_coral_target =
        |layer: &BlockLayer| layer.is("minecraft:water") || is_coral_layer(layer);
    if let Some(layer) = layer_at_world(
        chunk,
        chunk_min_x,
        chunk_min_z,
        world_x,
        world_y,
        world_z,
        settings.min_y,
    ) {
        return matches_coral_target(layer);
    }
    if let Some((source_min_x, source_min_z, source_chunk)) = source_context
        && let Some(layer) = layer_at_world(
            source_chunk,
            source_min_x,
            source_min_z,
            world_x,
            world_y,
            world_z,
            settings.min_y,
        )
    {
        return matches_coral_target(layer);
    }
    settings
        .terrain_layer_at(world_x, world_y, world_z)
        .is_some_and(|layer| matches_coral_target(&layer))
}

fn rotate_horizontal_direction(direction: (i32, i32), clockwise: bool) -> (i32, i32) {
    if clockwise {
        (-direction.1, direction.0)
    } else {
        (direction.1, -direction.0)
    }
}

fn shuffle_horizontal_directions(directions: &mut [(i32, i32)], random: &mut FeatureRandom) {
    for index in (1..directions.len()).rev() {
        let swap = random.next_int(index as i32 + 1) as usize;
        directions.swap(index, swap);
    }
}

fn is_coral_layer(layer: &BlockLayer) -> bool {
    CORAL_BLOCKS.contains(&layer.block.as_ref())
        || CORAL_FANS.contains(&layer.block.as_ref())
        || CORAL_WALL_FANS.contains(&layer.block.as_ref())
}

const CORAL_BLOCKS: &[&str] = &[
    "minecraft:tube_coral_block",
    "minecraft:brain_coral_block",
    "minecraft:bubble_coral_block",
    "minecraft:fire_coral_block",
    "minecraft:horn_coral_block",
];
const CORAL_FANS: &[&str] = &[
    "minecraft:tube_coral_fan",
    "minecraft:brain_coral_fan",
    "minecraft:bubble_coral_fan",
    "minecraft:fire_coral_fan",
    "minecraft:horn_coral_fan",
];
const CORAL_WALL_FANS: &[&str] = &[
    "minecraft:tube_coral_wall_fan",
    "minecraft:brain_coral_wall_fan",
    "minecraft:bubble_coral_wall_fan",
    "minecraft:fire_coral_wall_fan",
    "minecraft:horn_coral_wall_fan",
];

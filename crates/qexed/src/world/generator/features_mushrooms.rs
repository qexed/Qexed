#[derive(Debug, Clone)]
struct PlacedHugeMushroomFeature {
    step_index: i32,
    feature_index: i32,
    count: OrePlacementCount,
    config: HugeMushroomFeatureConfig,
    biome_filter: FeatureBiomeFilter,
}

impl PlacedHugeMushroomFeature {
    fn mushroom_island_vegetation(feature_index: i32) -> Self {
        Self {
            step_index: 9,
            feature_index,
            count: OrePlacementCount::Constant(1),
            config: HugeMushroomFeatureConfig::random_boolean(),
            biome_filter: FeatureBiomeFilter::Include(MUSHROOM_ISLAND_VEGETATION_BIOMES),
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
            let Some((local_x, local_z)) = local_coords(world_x, world_z, origin_x, origin_z)
            else {
                continue;
            };
            let world_y = chunk.world_surface_wg_height(local_x, local_z, settings.min_y);
            if world_y <= settings.min_y
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
            let Some((local_x, local_z)) =
                local_coords(world_x, world_z, source_origin_x, source_origin_z)
            else {
                continue;
            };
            let world_y = source_chunk.world_surface_wg_height(local_x, local_z, settings.min_y);
            if world_y <= settings.min_y
                || !self
                    .biome_filter
                    .allows_at(&settings.density, world_x, world_y, world_z)
            {
                continue;
            }

            let mut replay_random = random.clone();
            if self.config.place(
                settings,
                source_origin_x,
                source_origin_z,
                source_chunk,
                random,
                world_x,
                world_y,
                world_z,
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
                );
            }
        }
    }
}

#[derive(Debug, Clone)]
struct HugeMushroomFeatureConfig {
    selector: HugeMushroomSelector,
}

impl HugeMushroomFeatureConfig {
    fn brown() -> Self {
        Self {
            selector: HugeMushroomSelector::Single(HugeMushroomKind::Brown),
        }
    }

    fn red() -> Self {
        Self {
            selector: HugeMushroomSelector::Single(HugeMushroomKind::Red),
        }
    }

    fn random_boolean() -> Self {
        Self {
            selector: HugeMushroomSelector::RandomBoolean,
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
        self.place_for_chunk(
            settings,
            chunk_min_x,
            chunk_min_z,
            chunk,
            random,
            world_x,
            world_y,
            world_z,
            true,
        )
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
        self.place_for_chunk(
            settings,
            chunk_min_x,
            chunk_min_z,
            chunk,
            random,
            world_x,
            world_y,
            world_z,
            false,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn place_for_chunk(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
        world_x: i32,
        world_y: i32,
        world_z: i32,
        require_support: bool,
    ) -> bool {
        let kind = self.selector.select(random);
        kind.place(
            settings,
            chunk_min_x,
            chunk_min_z,
            chunk,
            random,
            world_x,
            world_y,
            world_z,
            require_support,
        )
    }
}

#[derive(Debug, Clone, Copy)]
enum HugeMushroomSelector {
    Single(HugeMushroomKind),
    RandomBoolean,
}

impl HugeMushroomSelector {
    fn select(self, random: &mut FeatureRandom) -> HugeMushroomKind {
        match self {
            Self::Single(kind) => kind,
            Self::RandomBoolean => {
                if random.next_bool() {
                    HugeMushroomKind::Red
                } else {
                    HugeMushroomKind::Brown
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HugeMushroomKind {
    Brown,
    Red,
}

impl HugeMushroomKind {
    fn cap_block(self) -> &'static str {
        match self {
            Self::Brown => "minecraft:brown_mushroom_block",
            Self::Red => "minecraft:red_mushroom_block",
        }
    }

    fn foliage_radius(self) -> i32 {
        match self {
            Self::Brown => 3,
            Self::Red => 2,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn place(
        self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
        world_x: i32,
        world_y: i32,
        world_z: i32,
        require_support: bool,
    ) -> bool {
        let mut height = 4 + random.next_int(3);
        if random.next_int(12) == 0 {
            height *= 2;
        }

        if !self.is_valid_position(
            settings,
            chunk_min_x,
            chunk_min_z,
            chunk,
            world_x,
            world_y,
            world_z,
            height,
            require_support,
        ) {
            return false;
        }

        match self {
            Self::Brown => self.make_brown_cap(
                settings,
                chunk_min_x,
                chunk_min_z,
                chunk,
                world_x,
                world_y,
                world_z,
                height,
            ),
            Self::Red => self.make_red_cap(
                settings,
                chunk_min_x,
                chunk_min_z,
                chunk,
                world_x,
                world_y,
                world_z,
                height,
            ),
        }
        self.place_trunk(
            settings,
            chunk_min_x,
            chunk_min_z,
            chunk,
            world_x,
            world_y,
            world_z,
            height,
        );
        true
    }

    #[allow(clippy::too_many_arguments)]
    fn is_valid_position(
        self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &NoiseChunkBlocks,
        world_x: i32,
        world_y: i32,
        world_z: i32,
        height: i32,
        require_support: bool,
    ) -> bool {
        if world_y < settings.min_y + 1 || world_y + height + 1 > settings.min_y + settings.height
        {
            return false;
        }
        if require_support
            && !layer_at_world(
                chunk,
                chunk_min_x,
                chunk_min_z,
                world_x,
                world_y - 1,
                world_z,
                settings.min_y,
            )
            .is_some_and(supports_huge_mushroom_layer)
        {
            return false;
        }

        for dy in 0..=height {
            let radius = self.radius_for_height(height, dy);
            for dx in -radius..=radius {
                for dz in -radius..=radius {
                    let Some(layer) = layer_at_world(
                        chunk,
                        chunk_min_x,
                        chunk_min_z,
                        world_x + dx,
                        world_y + dy,
                        world_z + dz,
                        settings.min_y,
                    ) else {
                        continue;
                    };
                    if !layer.is_air && !is_leaf_layer(layer) {
                        return false;
                    }
                }
            }
        }

        true
    }

    fn radius_for_height(self, height: i32, y_offset: i32) -> i32 {
        match self {
            Self::Brown => {
                if y_offset <= 3 {
                    0
                } else {
                    self.foliage_radius()
                }
            }
            Self::Red => {
                if y_offset < height && y_offset >= height - 3 || y_offset == height {
                    self.foliage_radius()
                } else {
                    0
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn place_trunk(
        self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        world_x: i32,
        world_y: i32,
        world_z: i32,
        height: i32,
    ) {
        for dy in 0..height {
            self.place_mushroom_block(
                settings,
                chunk_min_x,
                chunk_min_z,
                chunk,
                world_x,
                world_y + dy,
                world_z,
                mushroom_stem_block(),
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn make_brown_cap(
        self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        world_x: i32,
        world_y: i32,
        world_z: i32,
        height: i32,
    ) {
        let radius = self.foliage_radius();
        for dx in -radius..=radius {
            for dz in -radius..=radius {
                let min_x = dx == -radius;
                let max_x = dx == radius;
                let min_z = dz == -radius;
                let max_z = dz == radius;
                let x_edge = min_x || max_x;
                let z_edge = min_z || max_z;
                if x_edge && z_edge {
                    continue;
                }

                let block = mushroom_cap_block(
                    self.cap_block(),
                    min_x || z_edge && dx == 1 - radius,
                    max_x || z_edge && dx == radius - 1,
                    min_z || x_edge && dz == 1 - radius,
                    max_z || x_edge && dz == radius - 1,
                    true,
                );
                self.place_mushroom_block(
                    settings,
                    chunk_min_x,
                    chunk_min_z,
                    chunk,
                    world_x + dx,
                    world_y + height,
                    world_z + dz,
                    block,
                );
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn make_red_cap(
        self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        world_x: i32,
        world_y: i32,
        world_z: i32,
        height: i32,
    ) {
        let foliage_radius = self.foliage_radius();
        let center = foliage_radius - 2;
        for y in height - 3..=height {
            let radius = if y < height {
                foliage_radius
            } else {
                foliage_radius - 1
            };
            for dx in -radius..=radius {
                for dz in -radius..=radius {
                    let min_x = dx == -radius;
                    let max_x = dx == radius;
                    let min_z = dz == -radius;
                    let max_z = dz == radius;
                    let x_edge = min_x || max_x;
                    let z_edge = min_z || max_z;
                    if y < height && x_edge == z_edge {
                        continue;
                    }

                    let block = mushroom_cap_block(
                        self.cap_block(),
                        dx < -center,
                        dx > center,
                        dz < -center,
                        dz > center,
                        y >= height - 1,
                    );
                    self.place_mushroom_block(
                        settings,
                        chunk_min_x,
                        chunk_min_z,
                        chunk,
                        world_x + dx,
                        world_y + y,
                        world_z + dz,
                        block,
                    );
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn place_mushroom_block(
        self,
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
        if !current.is_air && !is_replaceable_by_mushrooms_layer(current) {
            return false;
        }
        chunk.set_layer(local_x, world_y, local_z, settings.min_y, block);
        true
    }
}

fn mushroom_stem_block() -> BlockLayer {
    BlockLayer::with_properties(
        "minecraft:mushroom_stem",
        &[
            ("down", "false"),
            ("east", "true"),
            ("north", "true"),
            ("south", "true"),
            ("up", "false"),
            ("west", "true"),
        ],
    )
}

fn mushroom_cap_block(
    block: &str,
    west: bool,
    east: bool,
    north: bool,
    south: bool,
    up: bool,
) -> BlockLayer {
    BlockLayer::with_properties(
        block,
        &[
            ("down", "false"),
            ("east", bool_property(east)),
            ("north", bool_property(north)),
            ("south", bool_property(south)),
            ("up", bool_property(up)),
            ("west", bool_property(west)),
        ],
    )
}

fn bool_property(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}

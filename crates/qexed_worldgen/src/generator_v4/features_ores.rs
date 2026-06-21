#[derive(Debug, Clone)]
struct PlacedOreFeature {
    step_index: i32,
    feature_index: i32,
    count: OrePlacementCount,
    height: OreHeight,
    ore: OreFeatureConfig,
    biome_filter: FeatureBiomeFilter,
}

impl PlacedOreFeature {
    fn new(
        feature_index: i32,
        count: OrePlacementCount,
        height: OreHeight,
        ore: OreFeatureConfig,
    ) -> Self {
        Self {
            step_index: 6,
            feature_index,
            count,
            height,
            ore,
            biome_filter: FeatureBiomeFilter::All,
        }
    }

    fn with_step_index(mut self, step_index: i32) -> Self {
        self.step_index = step_index;
        self
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
        for _ in 0..self.count.sample(random) {
            let x = origin_x + random.next_int(16);
            let z = origin_z + random.next_int(16);
            let y = self.height.sample(settings, random);
            if !self.biome_filter.allows_at(&settings.density, x, y, z) {
                continue;
            }
            self.ore
                .place(settings, origin_x, origin_z, chunk, random, x, y, z);
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
            let x = source_origin_x + random.next_int(16);
            let z = source_origin_z + random.next_int(16);
            let y = self.height.sample(settings, random);
            if !self.biome_filter.allows_at(&settings.density, x, y, z) {
                continue;
            }

            let mut replay_random = random.clone();
            self.ore.place_with_neighbor(
                settings,
                source_origin_x,
                source_origin_z,
                source_chunk,
                Some((target_origin_x, target_origin_z, &*target_chunk)),
                random,
                x,
                y,
                z,
            );
            self.ore.place_with_neighbor(
                settings,
                target_origin_x,
                target_origin_z,
                target_chunk,
                Some((source_origin_x, source_origin_z, &*source_chunk)),
                &mut replay_random,
                x,
                y,
                z,
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
            let x = source_origin_x + random.next_int(16);
            let z = source_origin_z + random.next_int(16);
            let y = self.height.sample(settings, random);
            let mut replay_random = random.clone();
            if self.ore.may_spill_into(
                target_origin_x,
                target_origin_z,
                &mut replay_random,
                x,
                y,
                z,
            ) {
                return true;
            }
        }
        false
    }
}

#[derive(Debug, Clone, Copy)]
enum FeatureBiomeFilter {
    All,
    Include(&'static [&'static str]),
    Exclude(&'static [&'static str]),
}

impl FeatureBiomeFilter {
    fn allows(self, biome: &str) -> bool {
        match self {
            Self::All => true,
            Self::Include(biomes) => biomes.contains(&biome),
            Self::Exclude(biomes) => !biomes.contains(&biome),
        }
    }

    fn allows_at(self, density: &TerrainDensity, x: i32, y: i32, z: i32) -> bool {
        match self {
            Self::All => true,
            Self::Include(_) | Self::Exclude(_) => self.allows(density.biome(x, y, z)),
        }
    }

    fn can_match_chunk(self, chunk: &NoiseChunkBlocks) -> bool {
        if chunk.biomes.is_empty() {
            return true;
        }

        self.can_match_biomes(chunk.biomes.iter().copied())
    }

    fn can_match_biomes(self, mut biomes: impl Iterator<Item = &'static str>) -> bool {
        match self {
            Self::All => true,
            Self::Include(allowed) => biomes.any(|biome| allowed.contains(&biome)),
            Self::Exclude(excluded) => biomes.any(|biome| !excluded.contains(&biome)),
        }
    }
}

#[derive(Debug, Clone)]
struct PlacedUnderwaterMagmaFeature {
    step_index: i32,
    feature_index: i32,
    count: OrePlacementCount,
    height: OreHeight,
    floor_search_range: i32,
    placement_radius_around_floor: i32,
    placement_probability_per_valid_position: f32,
    magma_block: BlockLayer,
    biome_filter: FeatureBiomeFilter,
}

impl PlacedUnderwaterMagmaFeature {
    fn new(feature_index: i32) -> Self {
        Self {
            step_index: 6,
            feature_index,
            count: OrePlacementCount::Uniform { min: 44, max: 52 },
            height: OreHeight::Uniform(HeightAnchor::AboveBottom(0), HeightAnchor::Absolute(256)),
            floor_search_range: 5,
            placement_radius_around_floor: 1,
            placement_probability_per_valid_position: 0.5,
            magma_block: BlockLayer::new("minecraft:magma_block"),
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
            let x = origin_x + random.next_int(16);
            let z = origin_z + random.next_int(16);
            let y = self.height.sample(settings, random);
            let local_x = (x - origin_x) as usize;
            let local_z = (z - origin_z) as usize;
            let ocean_floor = chunk.ocean_floor_wg_height(local_x, local_z, settings.min_y);
            if y > ocean_floor - 2 || !self.biome_filter.allows_at(&settings.density, x, y, z) {
                continue;
            }

            if let Some(floor_y) = self.find_floor_y(settings, chunk, local_x, y, local_z) {
                self.place_around_floor(settings, origin_x, origin_z, chunk, random, x, floor_y, z);
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
        for _ in 0..self.count.sample(random) {
            let x = source_origin_x + random.next_int(16);
            let z = source_origin_z + random.next_int(16);
            let y = self.height.sample(settings, random);
            let local_x = (x - source_origin_x) as usize;
            let local_z = (z - source_origin_z) as usize;
            let ocean_floor = source_chunk.ocean_floor_wg_height(local_x, local_z, settings.min_y);
            if y > ocean_floor - 2 || !self.biome_filter.allows_at(&settings.density, x, y, z) {
                continue;
            }

            if let Some(floor_y) = self.find_floor_y(settings, source_chunk, local_x, y, local_z) {
                self.place_around_floor_with_context(
                    settings,
                    source_origin_x,
                    source_origin_z,
                    target_origin_x,
                    target_origin_z,
                    source_chunk,
                    target_chunk,
                    random,
                    x,
                    floor_y,
                    z,
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
            let floor_x = source_origin_x + random.next_int(16);
            let floor_z = source_origin_z + random.next_int(16);
            let _floor_y = self.height.sample(settings, random);
            let radius = self.placement_radius_around_floor;
            if horizontal_box_overlaps_chunk(
                floor_x - radius,
                floor_x + radius,
                floor_z - radius,
                floor_z + radius,
                target_origin_x,
                target_origin_z,
            ) {
                return true;
            }
        }
        false
    }

    fn find_floor_y(
        &self,
        settings: &NoiseSettings,
        chunk: &NoiseChunkBlocks,
        local_x: usize,
        origin_y: i32,
        local_z: usize,
    ) -> Option<i32> {
        if !is_water_at(chunk, local_x, origin_y, local_z, settings.min_y) {
            return None;
        }

        let mut y = origin_y;
        for _ in 1..self.floor_search_range {
            if !is_water_at(chunk, local_x, y, local_z, settings.min_y) {
                break;
            }
            y -= 1;
        }

        let layer = chunk.layer(local_x, y, local_z, settings.min_y)?;
        (!is_water_layer(layer)).then_some(y)
    }

    #[allow(clippy::too_many_arguments)]
    fn place_around_floor(
        &self,
        settings: &NoiseSettings,
        origin_x: i32,
        origin_z: i32,
        chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
        floor_x: i32,
        floor_y: i32,
        floor_z: i32,
    ) {
        let radius = self.placement_radius_around_floor;
        for world_x in floor_x - radius..=floor_x + radius {
            for world_y in floor_y - radius..=floor_y + radius {
                for world_z in floor_z - radius..=floor_z + radius {
                    if random.next_float() >= self.placement_probability_per_valid_position {
                        continue;
                    }
                    let Some(local_x) = local_coord(world_x, origin_x) else {
                        continue;
                    };
                    let Some(local_z) = local_coord(world_z, origin_z) else {
                        continue;
                    };
                    if self.is_valid_placement(settings, chunk, local_x, world_y, local_z) {
                        chunk.set_layer(
                            local_x,
                            world_y,
                            local_z,
                            settings.min_y,
                            self.magma_block.clone(),
                        );
                    }
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn place_around_floor_with_context(
        &self,
        settings: &NoiseSettings,
        source_origin_x: i32,
        source_origin_z: i32,
        target_origin_x: i32,
        target_origin_z: i32,
        source_chunk: &mut NoiseChunkBlocks,
        target_chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
        floor_x: i32,
        floor_y: i32,
        floor_z: i32,
    ) -> bool {
        let radius = self.placement_radius_around_floor;
        let mut placed = false;
        for world_x in floor_x - radius..=floor_x + radius {
            for world_y in floor_y - radius..=floor_y + radius {
                for world_z in floor_z - radius..=floor_z + radius {
                    if random.next_float() >= self.placement_probability_per_valid_position
                        || !self.is_valid_placement_in_context(
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
                        )
                    {
                        continue;
                    }

                    if overlaps_chunk(world_x, world_z, source_origin_x, source_origin_z) {
                        let (local_x, local_z) =
                            local_coords(world_x, world_z, source_origin_x, source_origin_z)
                                .expect("overlaps_chunk guarantees local coordinates");
                        source_chunk.set_layer(
                            local_x,
                            world_y,
                            local_z,
                            settings.min_y,
                            self.magma_block.clone(),
                        );
                        placed = true;
                    } else if overlaps_chunk(world_x, world_z, target_origin_x, target_origin_z) {
                        let (local_x, local_z) =
                            local_coords(world_x, world_z, target_origin_x, target_origin_z)
                                .expect("overlaps_chunk guarantees local coordinates");
                        target_chunk.set_layer(
                            local_x,
                            world_y,
                            local_z,
                            settings.min_y,
                            self.magma_block.clone(),
                        );
                        placed = true;
                    }
                }
            }
        }
        placed
    }

    fn is_valid_placement(
        &self,
        settings: &NoiseSettings,
        chunk: &NoiseChunkBlocks,
        local_x: usize,
        world_y: i32,
        local_z: usize,
    ) -> bool {
        let Some(current) = chunk.layer(local_x, world_y, local_z, settings.min_y) else {
            return false;
        };
        if is_water_or_air_layer(current) {
            return false;
        }
        let Some(below) = chunk.layer(local_x, world_y - 1, local_z, settings.min_y) else {
            return false;
        };
        if !is_full_solid_layer(below) {
            return false;
        }

        for (dx, dz) in [(-1_i32, 0_i32), (1, 0), (0, -1), (0, 1)] {
            let neighbor_x = local_x as i32 + dx;
            let neighbor_z = local_z as i32 + dz;
            if !(0..16).contains(&neighbor_x) || !(0..16).contains(&neighbor_z) {
                return false;
            }
            let Some(neighbor) =
                chunk.layer(neighbor_x as usize, world_y, neighbor_z as usize, settings.min_y)
            else {
                return false;
            };
            if !is_full_solid_layer(neighbor) {
                return false;
            }
        }

        true
    }

    #[allow(clippy::too_many_arguments)]
    fn is_valid_placement_in_context(
        &self,
        settings: &NoiseSettings,
        source_chunk: &NoiseChunkBlocks,
        source_origin_x: i32,
        source_origin_z: i32,
        target_chunk: &NoiseChunkBlocks,
        target_origin_x: i32,
        target_origin_z: i32,
        world_x: i32,
        world_y: i32,
        world_z: i32,
    ) -> bool {
        let Some(current) = magma_context_layer(
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
        ) else {
            return false;
        };
        if is_water_or_air_layer(&current) {
            return false;
        }

        let Some(below) = magma_context_layer(
            settings,
            source_chunk,
            source_origin_x,
            source_origin_z,
            target_chunk,
            target_origin_x,
            target_origin_z,
            world_x,
            world_y - 1,
            world_z,
        ) else {
            return false;
        };
        if !is_full_solid_layer(&below) {
            return false;
        }

        for (dx, dz) in [(-1_i32, 0_i32), (1, 0), (0, -1), (0, 1)] {
            let Some(neighbor) = magma_context_layer(
                settings,
                source_chunk,
                source_origin_x,
                source_origin_z,
                target_chunk,
                target_origin_x,
                target_origin_z,
                world_x + dx,
                world_y,
                world_z + dz,
            ) else {
                return false;
            };
            if !is_full_solid_layer(&neighbor) {
                return false;
            }
        }

        true
    }
}

#[allow(clippy::too_many_arguments)]
fn magma_context_layer(
    settings: &NoiseSettings,
    source_chunk: &NoiseChunkBlocks,
    source_origin_x: i32,
    source_origin_z: i32,
    target_chunk: &NoiseChunkBlocks,
    target_origin_x: i32,
    target_origin_z: i32,
    world_x: i32,
    world_y: i32,
    world_z: i32,
) -> Option<BlockLayer> {
    layer_at_world(
        source_chunk,
        source_origin_x,
        source_origin_z,
        world_x,
        world_y,
        world_z,
        settings.min_y,
    )
    .or_else(|| {
        layer_at_world(
            target_chunk,
            target_origin_x,
            target_origin_z,
            world_x,
            world_y,
            world_z,
            settings.min_y,
        )
    })
    .cloned()
    .or_else(|| settings.terrain_layer_at(world_x, world_y, world_z))
}

#[derive(Debug, Clone)]
struct PlacedDiskFeature {
    step_index: i32,
    feature_index: i32,
    count: OrePlacementCount,
    half_height: i32,
    radius: UniformInt,
    target_blocks: &'static [&'static str],
    state_provider: DiskStateProvider,
    surface_anchor: Option<SurfaceDiskAnchor>,
    biome_filter: FeatureBiomeFilter,
}

impl PlacedDiskFeature {
    fn sand(feature_index: i32) -> Self {
        Self {
            step_index: 6,
            feature_index,
            count: OrePlacementCount::Constant(3),
            half_height: 2,
            radius: UniformInt { min: 2, max: 6 },
            target_blocks: DISK_DIRT_GRASS_TARGETS,
            state_provider: DiskStateProvider::Sand {
                sand: BlockLayer::new("minecraft:sand"),
                sandstone: BlockLayer::new("minecraft:sandstone"),
            },
            surface_anchor: None,
            biome_filter: FeatureBiomeFilter::All,
        }
    }

    fn clay(feature_index: i32) -> Self {
        Self::simple(
            feature_index,
            1,
            UniformInt { min: 2, max: 3 },
            DISK_DIRT_CLAY_TARGETS,
            "minecraft:clay",
        )
    }

    fn gravel(feature_index: i32) -> Self {
        Self::simple(
            feature_index,
            2,
            UniformInt { min: 2, max: 5 },
            DISK_DIRT_GRASS_TARGETS,
            "minecraft:gravel",
        )
    }

    fn grass(feature_index: i32) -> Self {
        Self {
            step_index: 6,
            feature_index,
            count: OrePlacementCount::Constant(1),
            half_height: 2,
            radius: UniformInt { min: 2, max: 6 },
            target_blocks: DISK_DIRT_MUD_TARGETS,
            state_provider: DiskStateProvider::Grass {
                dirt: BlockLayer::new("minecraft:dirt"),
                grass: BlockLayer::with_properties("minecraft:grass_block", &[("snowy", "false")]),
            },
            surface_anchor: None,
            biome_filter: FeatureBiomeFilter::All,
        }
    }

    fn with_surface_anchor(mut self, block: &'static str, offset_y: i32) -> Self {
        self.surface_anchor = Some(SurfaceDiskAnchor { block, offset_y });
        self
    }

    fn simple(
        feature_index: i32,
        half_height: i32,
        radius: UniformInt,
        target_blocks: &'static [&'static str],
        block: &str,
    ) -> Self {
        Self {
            step_index: 6,
            feature_index,
            count: OrePlacementCount::Constant(1),
            half_height,
            radius,
            target_blocks,
            state_provider: DiskStateProvider::Simple(BlockLayer::new(block)),
            surface_anchor: None,
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
        for _ in 0..self.count.sample(random) {
            let x = origin_x + random.next_int(16);
            let z = origin_z + random.next_int(16);
            let local_x = (x - origin_x) as usize;
            let local_z = (z - origin_z) as usize;
            let y = chunk.ocean_floor_wg_height(local_x, local_z, settings.min_y);
            if y <= settings.min_y
                || !self.biome_filter.allows_at(&settings.density, x, y, z)
                || !self.can_start_at(chunk, local_x, y, local_z, settings.min_y)
            {
                continue;
            }

            self.place_disk(settings, origin_x, origin_z, chunk, random, x, y, z);
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
            let x = source_origin_x + random.next_int(16);
            let z = source_origin_z + random.next_int(16);
            let local_x = (x - source_origin_x) as usize;
            let local_z = (z - source_origin_z) as usize;
            let y = source_chunk.ocean_floor_wg_height(local_x, local_z, settings.min_y);
            if y <= settings.min_y
                || !self.biome_filter.allows_at(&settings.density, x, y, z)
                || !self.can_start_at(source_chunk, local_x, y, local_z, settings.min_y)
            {
                continue;
            }

            let radius = self.radius.sample(random);
            if horizontal_box_overlaps_chunk(
                x - radius,
                x + radius,
                z - radius,
                z + radius,
                target_origin_x,
                target_origin_z,
            ) {
                self.place_disk_with_context(
                    settings,
                    source_origin_x,
                    source_origin_z,
                    target_origin_x,
                    target_origin_z,
                    source_chunk,
                    target_chunk,
                    radius,
                    x,
                    y,
                    z,
                );
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
        for _ in 0..self.count.sample(random) {
            let x = source_origin_x + random.next_int(16);
            let z = source_origin_z + random.next_int(16);
            let radius = self.radius.sample(random);
            if horizontal_box_overlaps_chunk(
                x - radius,
                x + radius,
                z - radius,
                z + radius,
                target_origin_x,
                target_origin_z,
            ) {
                return true;
            }
        }
        false
    }

    fn can_start_at(
        &self,
        chunk: &NoiseChunkBlocks,
        local_x: usize,
        world_y: i32,
        local_z: usize,
        min_y: i32,
    ) -> bool {
        if let Some(anchor) = self.surface_anchor {
            chunk
                .layer(local_x, world_y + anchor.offset_y, local_z, min_y)
                .is_some_and(|layer| layer.is(anchor.block))
        } else {
            is_water_at(chunk, local_x, world_y, local_z, min_y)
        }
    }

    fn place_disk(
        &self,
        settings: &NoiseSettings,
        origin_x: i32,
        origin_z: i32,
        chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
        center_x: i32,
        center_y: i32,
        center_z: i32,
    ) {
        let radius = self.radius.sample(random);
        let min_y = (center_y - self.half_height).max(settings.min_y);
        let max_y = (center_y + self.half_height).min(settings.min_y + settings.height - 1);
        if min_y > max_y {
            return;
        }

        for world_x in center_x - radius..=center_x + radius {
            let dx = world_x - center_x;
            for world_z in center_z - radius..=center_z + radius {
                let dz = world_z - center_z;
                if dx * dx + dz * dz > radius * radius {
                    continue;
                }
                let Some(local_x) = local_coord(world_x, origin_x) else {
                    continue;
                };
                let Some(local_z) = local_coord(world_z, origin_z) else {
                    continue;
                };
                for world_y in (min_y..=max_y).rev() {
                    let Some(current) = chunk.layer(local_x, world_y, local_z, settings.min_y)
                    else {
                        continue;
                    };
                    if !self.target_blocks.contains(&current.block.as_ref()) {
                        continue;
                    }

                    let replacement = self.state_provider.block_at(
                        chunk,
                        local_x,
                        world_y,
                        local_z,
                        settings.min_y,
                    );
                    chunk.set_layer(local_x, world_y, local_z, settings.min_y, replacement);
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn place_disk_with_context(
        &self,
        settings: &NoiseSettings,
        source_origin_x: i32,
        source_origin_z: i32,
        target_origin_x: i32,
        target_origin_z: i32,
        source_chunk: &mut NoiseChunkBlocks,
        target_chunk: &mut NoiseChunkBlocks,
        radius: i32,
        center_x: i32,
        center_y: i32,
        center_z: i32,
    ) {
        let min_y = (center_y - self.half_height).max(settings.min_y);
        let max_y = (center_y + self.half_height).min(settings.min_y + settings.height - 1);
        if min_y > max_y {
            return;
        }

        let min_x = (center_x - radius).max(source_origin_x.min(target_origin_x));
        let max_x = (center_x + radius).min((source_origin_x + 15).max(target_origin_x + 15));
        let min_z = (center_z - radius).max(source_origin_z.min(target_origin_z));
        let max_z = (center_z + radius).min((source_origin_z + 15).max(target_origin_z + 15));
        if min_x > max_x || min_z > max_z {
            return;
        }

        for world_x in min_x..=max_x {
            let dx = world_x - center_x;
            for world_z in min_z..=max_z {
                let dz = world_z - center_z;
                if dx * dx + dz * dz > radius * radius {
                    continue;
                }
                for world_y in (min_y..=max_y).rev() {
                    let Some(current) = ore_context_layer(
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
                    ) else {
                        continue;
                    };
                    if !self.target_blocks.contains(&current.block.as_ref()) {
                        continue;
                    }

                    let replacement = self.state_provider.block_at_world(
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
                    );

                    if let Some((local_x, local_z)) =
                        local_coords(world_x, world_z, source_origin_x, source_origin_z)
                    {
                        source_chunk.set_layer(
                            local_x,
                            world_y,
                            local_z,
                            settings.min_y,
                            replacement,
                        );
                    } else if let Some((local_x, local_z)) =
                        local_coords(world_x, world_z, target_origin_x, target_origin_z)
                    {
                        target_chunk.set_layer(
                            local_x,
                            world_y,
                            local_z,
                            settings.min_y,
                            replacement,
                        );
                    }
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
enum DiskStateProvider {
    Simple(BlockLayer),
    Sand {
        sand: BlockLayer,
        sandstone: BlockLayer,
    },
    Grass {
        dirt: BlockLayer,
        grass: BlockLayer,
    },
}

impl DiskStateProvider {
    fn block_at(
        &self,
        chunk: &NoiseChunkBlocks,
        local_x: usize,
        world_y: i32,
        local_z: usize,
        min_y: i32,
    ) -> BlockLayer {
        match self {
            Self::Simple(block) => block.clone(),
            Self::Sand { sand, sandstone } => {
                if chunk
                    .layer(local_x, world_y - 1, local_z, min_y)
                    .is_some_and(|layer| layer.is_air)
                {
                    sandstone.clone()
                } else {
                    sand.clone()
                }
            }
            Self::Grass { dirt, grass } => {
                if !chunk
                    .layer(local_x, world_y + 1, local_z, min_y)
                    .is_some_and(|layer| is_full_solid_layer(layer) || is_water_layer(layer))
                {
                    grass.clone()
                } else {
                    dirt.clone()
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn block_at_world(
        &self,
        settings: &NoiseSettings,
        source_chunk: &NoiseChunkBlocks,
        source_origin_x: i32,
        source_origin_z: i32,
        target_chunk: &NoiseChunkBlocks,
        target_origin_x: i32,
        target_origin_z: i32,
        world_x: i32,
        world_y: i32,
        world_z: i32,
    ) -> BlockLayer {
        match self {
            Self::Simple(block) => block.clone(),
            Self::Sand { sand, sandstone } => {
                if ore_context_layer(
                    settings,
                    source_chunk,
                    source_origin_x,
                    source_origin_z,
                    target_chunk,
                    target_origin_x,
                    target_origin_z,
                    world_x,
                    world_y - 1,
                    world_z,
                )
                .is_some_and(|layer| layer.is_air)
                {
                    sandstone.clone()
                } else {
                    sand.clone()
                }
            }
            Self::Grass { dirt, grass } => {
                if !ore_context_layer(
                    settings,
                    source_chunk,
                    source_origin_x,
                    source_origin_z,
                    target_chunk,
                    target_origin_x,
                    target_origin_z,
                    world_x,
                    world_y + 1,
                    world_z,
                )
                .is_some_and(|layer| is_full_solid_layer(&layer) || is_water_layer(&layer))
                {
                    grass.clone()
                } else {
                    dirt.clone()
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn ore_context_layer(
    settings: &NoiseSettings,
    source_chunk: &NoiseChunkBlocks,
    source_origin_x: i32,
    source_origin_z: i32,
    target_chunk: &NoiseChunkBlocks,
    target_origin_x: i32,
    target_origin_z: i32,
    world_x: i32,
    world_y: i32,
    world_z: i32,
) -> Option<BlockLayer> {
    layer_at_world(
        source_chunk,
        source_origin_x,
        source_origin_z,
        world_x,
        world_y,
        world_z,
        settings.min_y,
    )
    .or_else(|| {
        layer_at_world(
            target_chunk,
            target_origin_x,
            target_origin_z,
            world_x,
            world_y,
            world_z,
            settings.min_y,
        )
    })
    .cloned()
    .or_else(|| settings.terrain_layer_at(world_x, world_y, world_z))
}

#[derive(Debug, Clone, Copy)]
struct SurfaceDiskAnchor {
    block: &'static str,
    offset_y: i32,
}

#[derive(Debug, Clone)]
struct PlacedSpringFeature {
    step_index: i32,
    feature_index: i32,
    count: OrePlacementCount,
    height: OreHeight,
    config: SpringFeatureConfig,
    biome_filter: FeatureBiomeFilter,
}

impl PlacedSpringFeature {
    fn water(feature_index: i32) -> Self {
        Self {
            step_index: 8,
            feature_index,
            count: OrePlacementCount::Constant(25),
            height: OreHeight::Uniform(HeightAnchor::AboveBottom(0), HeightAnchor::Absolute(192)),
            config: SpringFeatureConfig {
                state: BlockLayer::new("minecraft:water"),
                rock_count: 4,
                hole_count: 1,
                requires_block_below: true,
                valid_blocks: SPRING_WATER_VALID_BLOCKS,
            },
            biome_filter: FeatureBiomeFilter::All,
        }
    }

    fn lava_overworld(feature_index: i32) -> Self {
        Self {
            step_index: 8,
            feature_index,
            count: OrePlacementCount::Constant(20),
            height: OreHeight::VeryBiasedToBottom {
                min: HeightAnchor::AboveBottom(0),
                max: HeightAnchor::BelowTop(8),
                inner: 8,
            },
            config: SpringFeatureConfig {
                state: BlockLayer::new("minecraft:lava"),
                rock_count: 4,
                hole_count: 1,
                requires_block_below: true,
                valid_blocks: SPRING_LAVA_VALID_BLOCKS,
            },
            biome_filter: FeatureBiomeFilter::All,
        }
    }

    fn lava_frozen(feature_index: i32) -> Self {
        Self {
            step_index: 8,
            feature_index,
            count: OrePlacementCount::Constant(20),
            height: OreHeight::VeryBiasedToBottom {
                min: HeightAnchor::AboveBottom(0),
                max: HeightAnchor::BelowTop(8),
                inner: 8,
            },
            config: SpringFeatureConfig {
                state: BlockLayer::new("minecraft:lava"),
                rock_count: 4,
                hole_count: 1,
                requires_block_below: true,
                valid_blocks: SPRING_FROZEN_LAVA_VALID_BLOCKS,
            },
            biome_filter: FeatureBiomeFilter::Include(FROZEN_LAVA_SPRING_BIOMES),
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
            let Some(local_x) = local_coord(world_x, origin_x) else {
                continue;
            };
            let Some(local_z) = local_coord(world_z, origin_z) else {
                continue;
            };
            debug_assert_eq!(local_x, (world_x - origin_x) as usize);
            debug_assert_eq!(local_z, (world_z - origin_z) as usize);
            self.config.try_place_at_world(
                settings, origin_x, origin_z, chunk, world_x, world_y, world_z,
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
        target_chunk: &NoiseChunkBlocks,
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
            self.config.try_place_at_world_with_neighbor(
                settings,
                source_origin_x,
                source_origin_z,
                source_chunk,
                Some((target_origin_x, target_origin_z, target_chunk)),
                world_x,
                world_y,
                world_z,
            );
        }
    }
}

#[derive(Debug, Clone)]
struct SpringFeatureConfig {
    state: BlockLayer,
    rock_count: i32,
    hole_count: i32,
    requires_block_below: bool,
    valid_blocks: &'static [&'static str],
}

impl SpringFeatureConfig {
    #[cfg(test)]
    fn try_place(
        &self,
        settings: &NoiseSettings,
        chunk: &mut NoiseChunkBlocks,
        local_x: usize,
        world_y: i32,
        local_z: usize,
    ) -> bool {
        self.try_place_at_world(
            settings,
            0,
            0,
            chunk,
            local_x as i32,
            world_y,
            local_z as i32,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn try_place_at_world(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        world_x: i32,
        world_y: i32,
        world_z: i32,
    ) -> bool {
        self.try_place_at_world_with_neighbor(
            settings,
            chunk_min_x,
            chunk_min_z,
            chunk,
            None,
            world_x,
            world_y,
            world_z,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn try_place_at_world_with_neighbor(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        neighbor: Option<(i32, i32, &NoiseChunkBlocks)>,
        world_x: i32,
        world_y: i32,
        world_z: i32,
    ) -> bool {
        let Some((local_x, local_z)) = local_coords(world_x, world_z, chunk_min_x, chunk_min_z)
        else {
            return false;
        };

        if !self.is_valid_block_at_world(
            settings,
            chunk,
            chunk_min_x,
            chunk_min_z,
            neighbor,
            world_x,
            world_y + 1,
            world_z,
        ) {
            return false;
        }
        if self.requires_block_below
            && !self.is_valid_block_at_world(
                settings,
                chunk,
                chunk_min_x,
                chunk_min_z,
                neighbor,
                world_x,
                world_y - 1,
                world_z,
            )
        {
            return false;
        }

        let Some(current) =
            self.context_layer(
                settings,
                chunk,
                chunk_min_x,
                chunk_min_z,
                neighbor,
                world_x,
                world_y,
                world_z,
            )
        else {
            return false;
        };
        if !current.is_air && !self.valid_blocks.contains(&current.block.as_ref()) {
            return false;
        }

        let mut rock_count = 0;
        let mut hole_count = 0;
        for (dx, dy, dz) in [
            (-1_i32, 0_i32, 0_i32),
            (1, 0, 0),
            (0, 0, -1),
            (0, 0, 1),
            (0, -1, 0),
        ] {
            let y = world_y + dy;
            let x = world_x + dx;
            let z = world_z + dz;
            if self.is_valid_block_at_world(
                settings,
                chunk,
                chunk_min_x,
                chunk_min_z,
                neighbor,
                x,
                y,
                z,
            ) {
                rock_count += 1;
            }
            if self
                .context_layer(settings, chunk, chunk_min_x, chunk_min_z, neighbor, x, y, z)
                .is_some_and(|layer| layer.is_air)
            {
                hole_count += 1;
            }
        }

        if rock_count == self.rock_count && hole_count == self.hole_count {
            chunk.set_layer(
                local_x,
                world_y,
                local_z,
                settings.min_y,
                self.state.clone(),
            );
            true
        } else {
            false
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn is_valid_block_at_world(
        &self,
        settings: &NoiseSettings,
        chunk: &NoiseChunkBlocks,
        chunk_min_x: i32,
        chunk_min_z: i32,
        neighbor: Option<(i32, i32, &NoiseChunkBlocks)>,
        world_x: i32,
        world_y: i32,
        world_z: i32,
    ) -> bool {
        self.context_layer(
            settings,
            chunk,
            chunk_min_x,
            chunk_min_z,
            neighbor,
            world_x,
            world_y,
            world_z,
        )
            .is_some_and(|layer| self.valid_blocks.contains(&layer.block.as_ref()))
    }

    #[allow(clippy::too_many_arguments)]
    fn context_layer(
        &self,
        settings: &NoiseSettings,
        chunk: &NoiseChunkBlocks,
        chunk_min_x: i32,
        chunk_min_z: i32,
        neighbor: Option<(i32, i32, &NoiseChunkBlocks)>,
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
        if let Some((neighbor_min_x, neighbor_min_z, neighbor_chunk)) = neighbor
            && let Some(layer) = layer_at_world(
                neighbor_chunk,
                neighbor_min_x,
                neighbor_min_z,
                world_x,
                world_y,
                world_z,
                settings.min_y,
            )
        {
            return Some(layer.clone());
        }
        settings.terrain_layer_at(world_x, world_y, world_z)
    }

}

#[derive(Debug, Clone)]
struct OreFeatureConfig {
    size: i32,
    discard_chance_on_air_exposure: f32,
    targets: Vec<OreFeatureTarget>,
}

#[derive(Debug, Clone)]
struct OreFeatureTarget {
    predicate: OreTargetPredicate,
    block: BlockLayer,
}

#[derive(Debug, Clone, Copy)]
enum OreTargetPredicate {
    StoneOreReplaceables,
    DeepslateOreReplaceables,
    BaseStoneOverworld,
}

impl OreFeatureConfig {
    fn new(
        size: i32,
        discard_chance_on_air_exposure: f32,
        stone_ore: &str,
        deepslate_ore: &str,
    ) -> Self {
        Self {
            size,
            discard_chance_on_air_exposure,
            targets: vec![
                OreFeatureTarget {
                    predicate: OreTargetPredicate::StoneOreReplaceables,
                    block: BlockLayer::new(stone_ore),
                },
                OreFeatureTarget {
                    predicate: OreTargetPredicate::DeepslateOreReplaceables,
                    block: BlockLayer::new(deepslate_ore),
                },
            ],
        }
    }

    fn base_stone(size: i32, block: &str) -> Self {
        Self {
            size,
            discard_chance_on_air_exposure: 0.0,
            targets: vec![OreFeatureTarget {
                predicate: OreTargetPredicate::BaseStoneOverworld,
                block: BlockLayer::new(block),
            }],
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
        origin_x: i32,
        origin_y: i32,
        origin_z: i32,
    ) -> bool {
        self.place_with_neighbor(
            settings,
            chunk_min_x,
            chunk_min_z,
            chunk,
            None,
            random,
            origin_x,
            origin_y,
            origin_z,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn place_with_neighbor(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        neighbor: Option<(i32, i32, &NoiseChunkBlocks)>,
        random: &mut FeatureRandom,
        origin_x: i32,
        origin_y: i32,
        origin_z: i32,
    ) -> bool {
        let direction = random.next_float() * std::f32::consts::PI;
        let spread_xy = self.size as f64 / 8.0;
        let x_spread = (direction as f64).sin() * spread_xy;
        let z_spread = (direction as f64).cos() * spread_xy;
        let x0 = origin_x as f64 + x_spread;
        let x1 = origin_x as f64 - x_spread;
        let z0 = origin_z as f64 + z_spread;
        let z1 = origin_z as f64 - z_spread;
        let y0 = origin_y as f64 + random.next_int(3) as f64 - 2.0;
        let y1 = origin_y as f64 + random.next_int(3) as f64 - 2.0;
        let mut spheres = vec![[0.0; 4]; self.size as usize];

        for i in 0..self.size {
            let step = i as f64 / self.size as f64;
            let radius_noise = random.next_double() * self.size as f64 / 16.0;
            let radius =
                ((mth_sin(std::f32::consts::PI as f64 * step) + 1.0) as f64 * radius_noise + 1.0)
                    / 2.0;
            spheres[i as usize] = [
                lerp_f64(step, x0, x1),
                lerp_f64(step, y0, y1),
                lerp_f64(step, z0, z1),
                radius,
            ];
        }

        for i1 in 0..self.size as usize {
            if spheres[i1][3] <= 0.0 {
                continue;
            }
            for i2 in i1 + 1..self.size as usize {
                if spheres[i2][3] <= 0.0 {
                    continue;
                }
                let dx = spheres[i1][0] - spheres[i2][0];
                let dy = spheres[i1][1] - spheres[i2][1];
                let dz = spheres[i1][2] - spheres[i2][2];
                let dr = spheres[i1][3] - spheres[i2][3];
                if dr * dr > dx * dx + dy * dy + dz * dz {
                    if dr > 0.0 {
                        spheres[i2][3] = -1.0;
                    } else {
                        spheres[i1][3] = -1.0;
                    }
                }
            }
        }

        let mut tested = HashSet::<(i32, i32, i32)>::new();
        let mut placed = false;
        for sphere in spheres {
            let [x, y, z, radius] = sphere;
            if radius < 0.0 {
                continue;
            }

            let min_x = (x - radius).floor() as i32;
            let max_x = ((x + radius).floor() as i32).max(min_x);
            let min_y = (y - radius).floor() as i32;
            let max_y = ((y + radius).floor() as i32).max(min_y);
            let min_z = (z - radius).floor() as i32;
            let max_z = ((z + radius).floor() as i32).max(min_z);

            for world_x in min_x..=max_x {
                let xd = (world_x as f64 + 0.5 - x) / radius;
                if xd * xd >= 1.0 {
                    continue;
                }

                for world_y in
                    min_y.max(settings.min_y)..=max_y.min(settings.min_y + settings.height - 1)
                {
                    let yd = (world_y as f64 + 0.5 - y) / radius;
                    if xd * xd + yd * yd >= 1.0 {
                        continue;
                    }

                    for world_z in min_z..=max_z {
                        let zd = (world_z as f64 + 0.5 - z) / radius;
                        if xd * xd + yd * yd + zd * zd >= 1.0 {
                            continue;
                        }
                        if tested.insert((world_x, world_y, world_z))
                            && self.try_place_block_with_neighbor(
                                settings,
                                chunk_min_x,
                                chunk_min_z,
                                chunk,
                                neighbor,
                                random,
                                world_x,
                                world_y,
                                world_z,
                            )
                        {
                            placed = true;
                        }
                    }
                }
            }
        }

        placed
    }

    fn may_spill_into(
        &self,
        target_origin_x: i32,
        target_origin_z: i32,
        random: &mut FeatureRandom,
        origin_x: i32,
        origin_y: i32,
        origin_z: i32,
    ) -> bool {
        let direction = random.next_float() * std::f32::consts::PI;
        let spread_xy = self.size as f64 / 8.0;
        let x_spread = (direction as f64).sin() * spread_xy;
        let z_spread = (direction as f64).cos() * spread_xy;
        let x0 = origin_x as f64 + x_spread;
        let x1 = origin_x as f64 - x_spread;
        let z0 = origin_z as f64 + z_spread;
        let z1 = origin_z as f64 - z_spread;
        let y0 = origin_y as f64 + random.next_int(3) as f64 - 2.0;
        let y1 = origin_y as f64 + random.next_int(3) as f64 - 2.0;
        let mut min_x = i32::MAX;
        let mut max_x = i32::MIN;
        let mut min_z = i32::MAX;
        let mut max_z = i32::MIN;

        for i in 0..self.size {
            let step = i as f64 / self.size as f64;
            let radius_noise = random.next_double() * self.size as f64 / 16.0;
            let radius =
                ((mth_sin(std::f32::consts::PI as f64 * step) + 1.0) as f64 * radius_noise + 1.0)
                    / 2.0;
            let x = lerp_f64(step, x0, x1);
            let _y = lerp_f64(step, y0, y1);
            let z = lerp_f64(step, z0, z1);
            min_x = min_x.min((x - radius).floor() as i32);
            max_x = max_x.max(((x + radius).floor() as i32).max(min_x));
            min_z = min_z.min((z - radius).floor() as i32);
            max_z = max_z.max(((z + radius).floor() as i32).max(min_z));
        }

        horizontal_box_overlaps_chunk(min_x, max_x, min_z, max_z, target_origin_x, target_origin_z)
    }

    #[allow(clippy::too_many_arguments)]
    fn try_place_block_with_neighbor(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        neighbor: Option<(i32, i32, &NoiseChunkBlocks)>,
        random: &mut FeatureRandom,
        world_x: i32,
        world_y: i32,
        world_z: i32,
    ) -> bool {
        let Some((local_x, local_z)) = local_coords(world_x, world_z, chunk_min_x, chunk_min_z)
        else {
            return false;
        };
        let Some(current) = chunk.layer(local_x, world_y, local_z, settings.min_y) else {
            return false;
        };
        let Some(ore) = self.target_ore(current).cloned() else {
            return false;
        };
        if !self.should_skip_air_check(random)
            && is_adjacent_to_air_with_neighbor(
                settings,
                chunk,
                chunk_min_x,
                chunk_min_z,
                neighbor,
                world_x,
                world_y,
                world_z,
            )
        {
            return false;
        }

        chunk.set_layer(local_x, world_y, local_z, settings.min_y, ore);
        true
    }

    fn target_ore(&self, current: &BlockLayer) -> Option<&BlockLayer> {
        self.targets
            .iter()
            .find(|target| target.predicate.matches(current))
            .map(|target| &target.block)
    }

    fn should_skip_air_check(&self, random: &mut FeatureRandom) -> bool {
        if self.discard_chance_on_air_exposure <= 0.0 {
            true
        } else if self.discard_chance_on_air_exposure >= 1.0 {
            false
        } else {
            random.next_float() >= self.discard_chance_on_air_exposure
        }
    }

    fn max_horizontal_spillover(&self) -> i32 {
        (self.size / 8 + self.size / 16 + 2).max(2)
    }
}

#[allow(clippy::too_many_arguments)]
fn is_adjacent_to_air_with_neighbor(
    settings: &NoiseSettings,
    chunk: &NoiseChunkBlocks,
    chunk_min_x: i32,
    chunk_min_z: i32,
    neighbor: Option<(i32, i32, &NoiseChunkBlocks)>,
    world_x: i32,
    world_y: i32,
    world_z: i32,
) -> bool {
    const DIRECTIONS: [(i32, i32, i32); 6] = [
        (1, 0, 0),
        (-1, 0, 0),
        (0, 1, 0),
        (0, -1, 0),
        (0, 0, 1),
        (0, 0, -1),
    ];

    DIRECTIONS.into_iter().any(|(dx, dy, dz)| {
        let x = world_x + dx;
        let y = world_y + dy;
        let z = world_z + dz;
        if !(settings.min_y..settings.min_y + settings.height).contains(&y) {
            return false;
        }

        if let Some(layer) = layer_at_world(chunk, chunk_min_x, chunk_min_z, x, y, z, settings.min_y)
        {
            return layer.is_air;
        }
        if let Some((neighbor_min_x, neighbor_min_z, neighbor_chunk)) = neighbor
            && let Some(layer) =
                layer_at_world(neighbor_chunk, neighbor_min_x, neighbor_min_z, x, y, z, settings.min_y)
        {
            return layer.is_air;
        }
        false
    })
}

fn mth_sin(value: f64) -> f32 {
    let index = ((value * 10_430.378_350_470_453) as i64 & 65_535) as f64;
    (index * std::f64::consts::TAU / 65_536.0).sin() as f32
}

impl OreTargetPredicate {
    fn matches(self, layer: &BlockLayer) -> bool {
        match self {
            Self::StoneOreReplaceables => is_stone_ore_replaceable(layer),
            Self::DeepslateOreReplaceables => is_deepslate_ore_replaceable(layer),
            Self::BaseStoneOverworld => is_base_stone_overworld(layer),
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum OrePlacementCount {
    Constant(i32),
    Uniform { min: i32, max: i32 },
    Rarity(i32),
}

impl OrePlacementCount {
    fn sample(self, random: &mut FeatureRandom) -> i32 {
        match self {
            Self::Constant(value) => value,
            Self::Uniform { min, max } => min + random.next_int(max - min + 1),
            Self::Rarity(chance) => (random.next_float() < 1.0 / chance as f32) as i32,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct UniformInt {
    min: i32,
    max: i32,
}

impl UniformInt {
    fn sample(self, random: &mut FeatureRandom) -> i32 {
        self.min + random.next_int(self.max - self.min + 1)
    }
}

#[derive(Clone, Copy, Debug)]
struct TrapezoidInt {
    min: i32,
    max: i32,
    plateau: i32,
}

impl TrapezoidInt {
    fn new(min: i32, max: i32, plateau: i32) -> Self {
        Self { min, max, plateau }
    }

    fn sample(self, random: &mut FeatureRandom) -> i32 {
        if self.plateau == 0 && self.max == -self.min {
            return random.next_int(self.max + 1) - random.next_int(self.max + 1);
        }

        let range = self.max - self.min;
        if self.plateau == range {
            return self.min + random.next_int(range + 1);
        }

        let plateau_start = (range - self.plateau) / 2;
        let plateau_end = range - plateau_start;
        self.min + random.next_int(plateau_end + 1) + random.next_int(plateau_start + 1)
    }
}

#[derive(Clone, Copy, Debug)]
enum OreHeight {
    Uniform(HeightAnchor, HeightAnchor),
    Trapezoid(HeightAnchor, HeightAnchor),
    VeryBiasedToBottom {
        min: HeightAnchor,
        max: HeightAnchor,
        inner: i32,
    },
}

impl OreHeight {
    fn sample(self, settings: &NoiseSettings, random: &mut FeatureRandom) -> i32 {
        match self {
            Self::Uniform(min, max) => {
                let min = min.resolve(settings);
                let max = max.resolve(settings);
                if min > max {
                    min
                } else {
                    min + random.next_int(max - min + 1)
                }
            }
            Self::Trapezoid(min, max) => {
                let min = min.resolve(settings);
                let max = max.resolve(settings);
                if min > max {
                    return min;
                }
                let range = max - min;
                let plateau_start = range / 2;
                let plateau_end = range - plateau_start;
                min + random.next_int(plateau_end + 1) + random.next_int(plateau_start + 1)
            }
            Self::VeryBiasedToBottom { min, max, inner } => {
                let min = min.resolve(settings);
                let max = max.resolve(settings);
                if max - min - inner + 1 <= 0 {
                    return min;
                }
                let upper_inclusive = min + inner + random.next_int(max - min - inner + 1);
                let biased_upper_inclusive = min + random.next_int(upper_inclusive - min);
                min + random.next_int(biased_upper_inclusive - min + inner)
            }
        }
    }
}

#[derive(Clone, Debug)]
struct FeatureRandom {
    source: vanilla_noise::XoroshiroRandomSource,
}

impl FeatureRandom {
    fn new(seed: i64) -> Self {
        Self {
            source: vanilla_noise::XoroshiroRandomSource::new(seed),
        }
    }

    fn decoration_seed(world_seed: i64, origin_x: i32, origin_z: i32) -> i64 {
        let mut random = Self::new(world_seed);
        random.set_seed(world_seed);
        let x_seed = random.next_long() | 1;
        let z_seed = random.next_long() | 1;
        let seed = (origin_x as i64)
            .wrapping_mul(x_seed)
            .wrapping_add((origin_z as i64).wrapping_mul(z_seed))
            ^ world_seed;
        random.set_seed(seed);
        seed
    }

    fn for_feature(decoration_seed: i64, feature_index: i32, step_index: i32) -> Self {
        Self::new(
            decoration_seed
                .wrapping_add(feature_index as i64)
                .wrapping_add((10_000 * step_index) as i64),
        )
    }

    fn set_seed(&mut self, seed: i64) {
        self.source.set_seed(seed);
    }

    fn next_bits(&mut self, bits: i32) -> i32 {
        (self.source.next_long() >> (64 - bits)) as i32
    }

    fn next_int(&mut self, bound: i32) -> i32 {
        assert!(bound > 0);
        if bound & (bound - 1) == 0 {
            return (((bound as i64) * (self.next_bits(31) as i64)) >> 31) as i32;
        }

        loop {
            let bits = self.next_bits(31);
            let value = bits % bound;
            if bits.wrapping_sub(value).wrapping_add(bound - 1) >= 0 {
                return value;
            }
        }
    }

    fn next_long(&mut self) -> i64 {
        let upper = self.next_bits(32) as i64;
        let lower = self.next_bits(32) as i64;
        (upper << 32).wrapping_add(lower)
    }

    fn next_float(&mut self) -> f32 {
        self.next_bits(24) as f32 * 5.960_464_5e-8_f32
    }

    fn next_bool(&mut self) -> bool {
        self.next_bits(1) != 0
    }

    fn next_double(&mut self) -> f64 {
        let upper = self.next_bits(26) as i64;
        let lower = self.next_bits(27) as i64;
        ((upper << 27) + lower) as f64 * (1.110_223_024_625_156_5e-16_f64)
    }
}

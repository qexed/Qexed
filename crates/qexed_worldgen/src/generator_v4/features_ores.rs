#[derive(Debug, Clone)]
struct PlacedOreFeature {
    step_index: i32,
    feature_index: i32,
    count: OrePlacementCount,
    height: OreHeight,
    ore: OreFeatureConfig,
    biome_filter: FeatureBiomeFilter,
}

#[derive(Debug, Default)]
struct OreSpilloverDiagnostic {
    attempts: i32,
    biome_skips: i32,
    shape_reaches_target: i32,
    precheck_passes: i32,
    precheck_scans: usize,
    source_spheres: usize,
    source_scans: usize,
    source_writes: usize,
    target_spheres: usize,
    target_scans: usize,
    target_writes: usize,
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
        for attempt in 0..self.count.sample(random) {
            let x = origin_x + random.next_int(16);
            let z = origin_z + random.next_int(16);
            let y = self.height.sample(settings, random);
            update_feature_write_trace_attempt(attempt, x, y, z);
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
        self.place_with_spillover_context(
            settings,
            source_origin_x,
            source_origin_z,
            target_origin_x,
            target_origin_z,
            source_chunk,
            target_chunk,
            &[],
            random,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn place_with_spillover_context(
        &self,
        settings: &NoiseSettings,
        source_origin_x: i32,
        source_origin_z: i32,
        target_origin_x: i32,
        target_origin_z: i32,
        source_chunk: &mut NoiseChunkBlocks,
        target_chunk: &mut NoiseChunkBlocks,
        context_chunks: &[(i32, i32, &NoiseChunkBlocks)],
        random: &mut FeatureRandom,
    ) {
        let diagnose = ore_spillover_diagnostic_enabled(self.step_index, self.feature_index);
        let mut diagnostic = diagnose.then(OreSpilloverDiagnostic::default);
        let mut owned_context_chunks = None;
        let mut owned_context_refs = Vec::new();

        for attempt in 0..self.count.sample(random) {
            let x = source_origin_x + random.next_int(16);
            let z = source_origin_z + random.next_int(16);
            let y = self.height.sample(settings, random);
            update_feature_write_trace_attempt(attempt, x, y, z);
            if let Some(diagnostic) = diagnostic.as_mut() {
                diagnostic.attempts += 1;
            }
            if !self.biome_filter.allows_at(&settings.density, x, y, z) {
                if let Some(diagnostic) = diagnostic.as_mut() {
                    diagnostic.biome_skips += 1;
                }
                continue;
            }
            let prefix = self.ore.sample_blob_prefix(random, x, y, z);
            let mut spill_random = random.clone();
            let spill_shape = self
                .ore
                .sample_blob_shape(&mut spill_random, prefix.clone());
            let reaches_target =
                self.ore
                    .shape_may_spill_into(target_origin_x, target_origin_z, &spill_shape);
            if reaches_target && let Some(diagnostic) = diagnostic.as_mut() {
                diagnostic.shape_reaches_target += 1;
            }
            let (precheck_passes, precheck_scans) = self.ore.precheck_passes_counted(
                settings,
                source_origin_x,
                source_origin_z,
                source_chunk,
                Some((target_origin_x, target_origin_z, &*target_chunk)),
                x,
                y,
                z,
            );
            if let Some(diagnostic) = diagnostic.as_mut() {
                diagnostic.precheck_scans += precheck_scans;
                if precheck_passes {
                    diagnostic.precheck_passes += 1;
                }
            }
            if !precheck_passes {
                continue;
            }
            let shape = self.ore.sample_blob_shape(random, prefix);
            if !reaches_target && self.ore.can_skip_non_spilling_blob_replay() {
                continue;
            }
            self.ore.start_count_trace_if_enabled();
            if self.ore.needs_source_spillover_replay() {
                if owned_context_chunks.is_none() {
                    owned_context_chunks = Some(source_region_context_chunks(
                        settings,
                        source_origin_x,
                        source_origin_z,
                        target_origin_x,
                        target_origin_z,
                    ));
                    owned_context_refs = owned_context_chunks
                        .as_ref()
                        .expect("source region context chunks are initialized")
                        .iter()
                        .map(|(origin_x, origin_z, chunk)| (*origin_x, *origin_z, chunk.as_ref()))
                        .collect();
                }
                let replay_context_chunks = if owned_context_refs.is_empty() {
                    context_chunks
                } else {
                    owned_context_refs.as_slice()
                };
                let stats = self.ore.shape_scan_stats_for_context(
                    settings,
                    &shape,
                    source_origin_x,
                    source_origin_z,
                    target_origin_x,
                    target_origin_z,
                    replay_context_chunks,
                );
                let (source_placed, target_placed) = self.ore.place_shape_with_context_chunks(
                    settings,
                    source_origin_x,
                    source_origin_z,
                    source_chunk,
                    target_origin_x,
                    target_origin_z,
                    target_chunk,
                    replay_context_chunks,
                    random,
                    &shape,
                );
                if let Some(diagnostic) = diagnostic.as_mut() {
                    diagnostic.source_spheres += stats.spheres;
                    diagnostic.source_scans += stats.scans;
                    diagnostic.source_writes += usize::from(source_placed);
                    diagnostic.target_spheres += stats.spheres;
                    diagnostic.target_scans += stats.scans;
                    diagnostic.target_writes += usize::from(target_placed);
                }
            } else if reaches_target {
                let stats =
                    self.ore
                        .shape_scan_stats(settings, target_origin_x, target_origin_z, &shape);
                let placed = self.ore.place_shape_with_neighbor(
                    settings,
                    target_origin_x,
                    target_origin_z,
                    target_chunk,
                    Some((source_origin_x, source_origin_z, &*source_chunk)),
                    random,
                    &shape,
                );
                if let Some(diagnostic) = diagnostic.as_mut() {
                    diagnostic.target_spheres += stats.spheres;
                    diagnostic.target_scans += stats.scans;
                    diagnostic.target_writes += usize::from(placed);
                }
            }
            self.ore.finish_count_trace_if_enabled();
        }
        if let Some(diagnostic) = diagnostic {
            print_ore_spillover_diagnostic(
                self.step_index,
                self.feature_index,
                source_origin_x,
                source_origin_z,
                target_origin_x,
                target_origin_z,
                &diagnostic,
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
        for attempt in 0..self.count.sample(random) {
            let x = source_origin_x + random.next_int(16);
            let z = source_origin_z + random.next_int(16);
            let y = self.height.sample(settings, random);
            update_feature_write_trace_attempt(attempt, x, y, z);
            if !self.biome_filter.allows_at(&settings.density, x, y, z) {
                continue;
            }
            if self
                .ore
                .may_spill_into(target_origin_x, target_origin_z, random, x, y, z)
            {
                return true;
            }
        }
        false
    }
}

fn ore_spillover_diagnostic_enabled(step_index: i32, feature_index: i32) -> bool {
    const ENV: &str = "QEXED_WORLDGEN_ORE_SPILLOVER_DIAG";
    std::env::var(ENV).ok().is_some_and(|value| {
        let value = value.trim();
        value == "1" || value == format!("{step_index}:{feature_index}")
    })
}

fn print_ore_spillover_diagnostic(
    step_index: i32,
    feature_index: i32,
    source_origin_x: i32,
    source_origin_z: i32,
    target_origin_x: i32,
    target_origin_z: i32,
    diagnostic: &OreSpilloverDiagnostic,
) {
    FEATURE_WRITE_TRACE_CONTEXT.with(|current| {
        let context = current.borrow();
        let (chunk_x, chunk_z, ordinal, name, source_chunk_x, source_chunk_z, target_chunk_x, target_chunk_z) =
            context
                .as_ref()
                .map(|context| {
                    (
                        context.chunk_x,
                        context.chunk_z,
                        context.ordinal,
                        context.feature_name,
                        context.source_chunk_x,
                        context.source_chunk_z,
                        context.target_chunk_x,
                        context.target_chunk_z,
                    )
                })
                .unwrap_or((0, 0, 0, "unknown", source_origin_x / 16, source_origin_z / 16, target_origin_x / 16, target_origin_z / 16));
        eprintln!(
            "ore spillover diag: chunk=({chunk_x},{chunk_z}) ordinal={ordinal} name={name} step={step_index} index={feature_index} source_chunk=({source_chunk_x},{source_chunk_z}) source_origin=({source_origin_x},{source_origin_z}) target_chunk=({target_chunk_x},{target_chunk_z}) target_origin=({target_origin_x},{target_origin_z}) attempts={} biome_skips={} shape_reaches_target={} precheck_passes={} precheck_scans={} source_spheres={} source_scans={} source_writes={} target_spheres={} target_scans={} target_writes={}",
            diagnostic.attempts,
            diagnostic.biome_skips,
            diagnostic.shape_reaches_target,
            diagnostic.precheck_passes,
            diagnostic.precheck_scans,
            diagnostic.source_spheres,
            diagnostic.source_scans,
            diagnostic.source_writes,
            diagnostic.target_spheres,
            diagnostic.target_scans,
            diagnostic.target_writes,
        );
    });
}

fn trace_ore_target_predicate_match(
    world_x: i32,
    world_y: i32,
    world_z: i32,
    current: &BlockLayer,
) {
    ORE_PLACEMENT_COUNT_TRACE.with(|trace| {
        if let Some(trace) = trace.borrow_mut().as_mut() {
            trace.target_predicate_matches += 1;
            eprintln!(
                "rust ore count predicate match: coord=({world_x},{world_y},{world_z}) current={}",
                current.block
            );
        }
    });
}

fn trace_ore_should_skip_air_check_next_float() {
    ORE_PLACEMENT_COUNT_TRACE.with(|trace| {
        if let Some(trace) = trace.borrow_mut().as_mut() {
            trace.should_skip_air_check_next_float_calls += 1;
        }
    });
}

fn trace_ore_set_block_state_write() {
    ORE_PLACEMENT_COUNT_TRACE.with(|trace| {
        if let Some(trace) = trace.borrow_mut().as_mut() {
            trace.set_block_state_writes += 1;
        }
    });
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
            let Some(neighbor) = chunk.layer(
                neighbor_x as usize,
                world_y,
                neighbor_z as usize,
                settings.min_y,
            ) else {
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
        for attempt in 0..self.count.sample(random) {
            let x = origin_x + random.next_int(16);
            let z = origin_z + random.next_int(16);
            let local_x = (x - origin_x) as usize;
            let local_z = (z - origin_z) as usize;
            let y = chunk.ocean_floor_wg_height(local_x, local_z, settings.min_y);
            update_feature_write_trace_attempt(attempt, x, y, z);
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
        for attempt in 0..self.count.sample(random) {
            let x = source_origin_x + random.next_int(16);
            let z = source_origin_z + random.next_int(16);
            let local_x = (x - source_origin_x) as usize;
            let local_z = (z - source_origin_z) as usize;
            let y = source_chunk.ocean_floor_wg_height(local_x, local_z, settings.min_y);
            update_feature_write_trace_attempt(attempt, x, y, z);
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
                    trace_feature_write_at_target(
                        world_x,
                        world_y,
                        world_z,
                        origin_x,
                        origin_z,
                        local_x,
                        local_z,
                        current,
                        &replacement,
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
                        if let Some(previous) =
                            source_chunk.layer(local_x, world_y, local_z, settings.min_y)
                        {
                            trace_feature_write_at_target(
                                world_x,
                                world_y,
                                world_z,
                                source_origin_x,
                                source_origin_z,
                                local_x,
                                local_z,
                                previous,
                                &replacement,
                            );
                        }
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
                        if let Some(previous) =
                            target_chunk.layer(local_x, world_y, local_z, settings.min_y)
                        {
                            trace_feature_write_at_target(
                                world_x,
                                world_y,
                                world_z,
                                target_origin_x,
                                target_origin_z,
                                local_x,
                                local_z,
                                previous,
                                &replacement,
                            );
                        }
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

        let Some(current) = self.context_layer(
            settings,
            chunk,
            chunk_min_x,
            chunk_min_z,
            neighbor,
            world_x,
            world_y,
            world_z,
        ) else {
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

#[derive(Debug, Clone)]
struct OreBlobPrefix {
    x0: f64,
    x1: f64,
    y0: f64,
    y1: f64,
    z0: f64,
    z1: f64,
    min_box_x: i32,
    min_box_y: i32,
    min_box_z: i32,
    tested_size_x: usize,
    tested_size_y: usize,
    tested_size_z: usize,
    tested_stride_x: usize,
    tested_stride_y: usize,
}

#[derive(Debug, Clone)]
struct OreBlobShape {
    spheres: Vec<[f64; 4]>,
    min_box_x: i32,
    min_box_y: i32,
    min_box_z: i32,
    tested_size_x: usize,
    tested_size_y: usize,
    tested_size_z: usize,
    tested_stride_x: usize,
    tested_stride_y: usize,
}

#[derive(Debug, Clone, Copy, Default)]
struct OreShapeScanStats {
    spheres: usize,
    scans: usize,
}

#[derive(Debug, Clone, Copy)]
struct OreSphereTraceBounds {
    floor_min_x: i32,
    floor_max_x: i32,
    floor_min_y: i32,
    floor_max_y: i32,
    floor_min_z: i32,
    floor_max_z: i32,
    raw_min_x: i32,
    raw_max_x: i32,
    raw_min_y: i32,
    raw_max_y: i32,
    raw_min_z: i32,
    raw_max_z: i32,
    iter_min_x: i32,
    iter_max_x: i32,
    iter_min_y: i32,
    iter_max_y: i32,
    iter_min_z: i32,
    iter_max_z: i32,
}

#[derive(Debug, Clone, Copy)]
struct ChunkContextBounds {
    min_x: i32,
    max_x: i32,
    min_z: i32,
    max_z: i32,
}

#[cfg(test)]
#[derive(Debug, Clone)]
struct OreSphereDiagnostic {
    index: i32,
    step: f32,
    sin: f32,
    radius_noise: f64,
    x: f64,
    y: f64,
    z: f64,
    radius_before_cull: f64,
    radius_after_cull: f64,
}

#[cfg(test)]
#[derive(Debug, Clone)]
struct OreBlobShapeDiagnostic {
    prefix: OreBlobPrefix,
    spheres: Vec<OreSphereDiagnostic>,
    shape: OreBlobShape,
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
        let prefix = self.sample_blob_prefix(random, origin_x, origin_y, origin_z);
        let precheck_passes = if neighbor.is_some() {
            self.precheck_passes(
                settings,
                chunk_min_x,
                chunk_min_z,
                chunk,
                neighbor,
                origin_x,
                origin_y,
                origin_z,
            )
        } else {
            self.precheck_passes_in_chunk(
                settings,
                chunk_min_x,
                chunk_min_z,
                chunk,
                origin_x,
                origin_y,
                origin_z,
            )
        };
        self.trace_precheck_if_enabled(precheck_passes);
        if !precheck_passes {
            return false;
        }

        let shape = self.sample_blob_shape(random, prefix);
        self.start_count_trace_if_enabled();
        let placed = self.place_shape_with_neighbor(
            settings,
            chunk_min_x,
            chunk_min_z,
            chunk,
            neighbor,
            random,
            &shape,
        );
        self.finish_count_trace_if_enabled();
        placed
    }

    #[allow(clippy::too_many_arguments)]
    fn place_shape_with_neighbor(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        neighbor: Option<(i32, i32, &NoiseChunkBlocks)>,
        random: &mut FeatureRandom,
        shape: &OreBlobShape,
    ) -> bool {
        let mut tested = None;
        let mut placed = false;
        for (sphere_index, sphere) in shape.spheres.iter().copied().enumerate() {
            let [x, y, z, radius] = sphere;
            if radius < 0.0 {
                self.trace_shape_sphere_at_target(
                    settings,
                    chunk_min_x,
                    chunk_min_z,
                    chunk,
                    shape,
                    sphere_index,
                    sphere,
                    None,
                );
                continue;
            }

            let raw_min_x = mth_floor(x - radius).max(shape.min_box_x);
            let raw_max_x = mth_floor(x + radius).max(raw_min_x);
            let min_x = raw_min_x.max(chunk_min_x);
            let max_x = raw_max_x.min(chunk_min_x + 15);
            let raw_min_y = mth_floor(y - radius).max(shape.min_box_y);
            let raw_max_y = mth_floor(y + radius).max(raw_min_y);
            let min_y = raw_min_y.max(settings.min_y);
            let max_y = raw_max_y.min(settings.min_y + settings.height - 1);
            let raw_min_z = mth_floor(z - radius).max(shape.min_box_z);
            let raw_max_z = mth_floor(z + radius).max(raw_min_z);
            let min_z = raw_min_z.max(chunk_min_z);
            let max_z = raw_max_z.min(chunk_min_z + 15);
            if min_x > max_x || min_y > max_y || min_z > max_z {
                continue;
            }
            self.trace_shape_sphere_at_target(
                settings,
                chunk_min_x,
                chunk_min_z,
                chunk,
                shape,
                sphere_index,
                sphere,
                Some(OreSphereTraceBounds {
                    floor_min_x: mth_floor(x - radius),
                    floor_max_x: mth_floor(x + radius),
                    floor_min_y: mth_floor(y - radius),
                    floor_max_y: mth_floor(y + radius),
                    floor_min_z: mth_floor(z - radius),
                    floor_max_z: mth_floor(z + radius),
                    raw_min_x,
                    raw_max_x,
                    raw_min_y,
                    raw_max_y,
                    raw_min_z,
                    raw_max_z,
                    iter_min_x: min_x,
                    iter_max_x: max_x,
                    iter_min_y: min_y,
                    iter_max_y: max_y,
                    iter_min_z: min_z,
                    iter_max_z: max_z,
                }),
            );

            for world_x in min_x..=max_x {
                let xd = (world_x as f64 + 0.5 - x) / radius;
                if xd * xd >= 1.0 {
                    continue;
                }

                for world_y in min_y..=max_y {
                    let yd = (world_y as f64 + 0.5 - y) / radius;
                    if xd * xd + yd * yd >= 1.0 {
                        continue;
                    }

                    for world_z in min_z..=max_z {
                        let zd = (world_z as f64 + 0.5 - z) / radius;
                        if xd * xd + yd * yd + zd * zd >= 1.0 {
                            continue;
                        }
                        let tested_x = (world_x - shape.min_box_x) as usize;
                        let tested_y = (world_y - shape.min_box_y) as usize;
                        let tested_z = (world_z - shape.min_box_z) as usize;
                        let tested_index = tested_x
                            + tested_y * shape.tested_stride_x
                            + tested_z * shape.tested_stride_x * shape.tested_stride_y;
                        let tested = tested.get_or_insert_with(|| {
                            vec![
                                false;
                                shape.tested_size_x * shape.tested_size_y * shape.tested_size_z
                            ]
                        });
                        self.trace_tested_bit_at_target(
                            settings,
                            chunk_min_x,
                            chunk_min_z,
                            chunk,
                            neighbor,
                            &[],
                            world_x,
                            world_y,
                            world_z,
                            sphere_index,
                            tested_index,
                            tested[tested_index],
                        );
                        if !tested[tested_index] {
                            tested[tested_index] = true;
                        } else {
                            continue;
                        }
                        if self.try_place_block_with_neighbor(
                            settings,
                            chunk_min_x,
                            chunk_min_z,
                            chunk,
                            neighbor,
                            random,
                            world_x,
                            world_y,
                            world_z,
                        ) {
                            placed = true;
                        }
                    }
                }
            }
        }

        placed
    }

    #[allow(clippy::too_many_arguments)]
    fn place_shape_with_context_chunks(
        &self,
        settings: &NoiseSettings,
        source_min_x: i32,
        source_min_z: i32,
        source_chunk: &mut NoiseChunkBlocks,
        target_min_x: i32,
        target_min_z: i32,
        target_chunk: &mut NoiseChunkBlocks,
        context_chunks: &[(i32, i32, &NoiseChunkBlocks)],
        random: &mut FeatureRandom,
        shape: &OreBlobShape,
    ) -> (bool, bool) {
        let mut tested = None;
        let mut source_placed = false;
        let mut target_placed = false;
        let bounds = chunk_context_bounds(
            source_min_x,
            source_min_z,
            target_min_x,
            target_min_z,
            context_chunks,
        );

        for (sphere_index, sphere) in shape.spheres.iter().copied().enumerate() {
            let [x, y, z, radius] = sphere;
            if radius < 0.0 {
                self.trace_shape_sphere_at_target(
                    settings,
                    target_min_x,
                    target_min_z,
                    target_chunk,
                    shape,
                    sphere_index,
                    sphere,
                    None,
                );
                continue;
            }

            let raw_min_x = mth_floor(x - radius).max(shape.min_box_x);
            let raw_max_x = mth_floor(x + radius).max(raw_min_x);
            let min_x = raw_min_x.max(bounds.min_x);
            let max_x = raw_max_x.min(bounds.max_x);
            let raw_min_y = mth_floor(y - radius).max(shape.min_box_y);
            let raw_max_y = mth_floor(y + radius).max(raw_min_y);
            let min_y = raw_min_y.max(settings.min_y);
            let max_y = raw_max_y.min(settings.min_y + settings.height - 1);
            let raw_min_z = mth_floor(z - radius).max(shape.min_box_z);
            let raw_max_z = mth_floor(z + radius).max(raw_min_z);
            let min_z = raw_min_z.max(bounds.min_z);
            let max_z = raw_max_z.min(bounds.max_z);
            if min_x > max_x || min_y > max_y || min_z > max_z {
                continue;
            }

            self.trace_shape_sphere_at_target(
                settings,
                target_min_x,
                target_min_z,
                target_chunk,
                shape,
                sphere_index,
                sphere,
                Some(OreSphereTraceBounds {
                    floor_min_x: mth_floor(x - radius),
                    floor_max_x: mth_floor(x + radius),
                    floor_min_y: mth_floor(y - radius),
                    floor_max_y: mth_floor(y + radius),
                    floor_min_z: mth_floor(z - radius),
                    floor_max_z: mth_floor(z + radius),
                    raw_min_x,
                    raw_max_x,
                    raw_min_y,
                    raw_max_y,
                    raw_min_z,
                    raw_max_z,
                    iter_min_x: min_x,
                    iter_max_x: max_x,
                    iter_min_y: min_y,
                    iter_max_y: max_y,
                    iter_min_z: min_z,
                    iter_max_z: max_z,
                }),
            );

            for world_x in min_x..=max_x {
                let xd = (world_x as f64 + 0.5 - x) / radius;
                if xd * xd >= 1.0 {
                    continue;
                }

                for world_y in min_y..=max_y {
                    let yd = (world_y as f64 + 0.5 - y) / radius;
                    if xd * xd + yd * yd >= 1.0 {
                        continue;
                    }

                    for world_z in min_z..=max_z {
                        let zd = (world_z as f64 + 0.5 - z) / radius;
                        if xd * xd + yd * yd + zd * zd >= 1.0 {
                            continue;
                        }
                        let tested_x = (world_x - shape.min_box_x) as usize;
                        let tested_y = (world_y - shape.min_box_y) as usize;
                        let tested_z = (world_z - shape.min_box_z) as usize;
                        let tested_index = tested_x
                            + tested_y * shape.tested_stride_x
                            + tested_z * shape.tested_stride_x * shape.tested_stride_y;
                        let tested = tested.get_or_insert_with(|| {
                            vec![
                                false;
                                shape.tested_size_x * shape.tested_size_y * shape.tested_size_z
                            ]
                        });
                        self.trace_tested_bit_at_target(
                            settings,
                            source_min_x,
                            source_min_z,
                            source_chunk,
                            Some((target_min_x, target_min_z, &*target_chunk)),
                            context_chunks,
                            world_x,
                            world_y,
                            world_z,
                            sphere_index,
                            tested_index,
                            tested[tested_index],
                        );
                        if !tested[tested_index] {
                            tested[tested_index] = true;
                        } else {
                            continue;
                        }

                        if local_coords(world_x, world_z, source_min_x, source_min_z).is_some() {
                            if self.try_place_block_with_neighbor(
                                settings,
                                source_min_x,
                                source_min_z,
                                source_chunk,
                                Some((target_min_x, target_min_z, &*target_chunk)),
                                random,
                                world_x,
                                world_y,
                                world_z,
                            ) {
                                source_placed = true;
                            }
                        } else if local_coords(world_x, world_z, target_min_x, target_min_z)
                            .is_some()
                            && self.try_place_block_with_neighbor(
                                settings,
                                target_min_x,
                                target_min_z,
                                target_chunk,
                                Some((source_min_x, source_min_z, &*source_chunk)),
                                random,
                                world_x,
                                world_y,
                                world_z,
                            )
                        {
                            target_placed = true;
                        } else if let Some((context_min_x, context_min_z, context_chunk)) =
                            context_chunk_at(context_chunks, world_x, world_z)
                        {
                            self.try_consume_block_with_context(
                                settings,
                                context_min_x,
                                context_min_z,
                                context_chunk,
                                context_chunks,
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

        (source_placed, target_placed)
    }

    #[allow(clippy::too_many_arguments)]
    fn trace_shape_sphere_at_target(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &NoiseChunkBlocks,
        shape: &OreBlobShape,
        sphere_index: usize,
        sphere: [f64; 4],
        bounds: Option<OreSphereTraceBounds>,
    ) {
        let Some(target) = FeatureWriteTraceTarget::from_env() else {
            return;
        };
        if !self.trace_context_is_target_attempt() {
            return;
        }

        let [x, y, z, radius] = sphere;
        let inside = bounds.is_some_and(|bounds| {
            if target.x < bounds.iter_min_x
                || target.x > bounds.iter_max_x
                || target.y < bounds.iter_min_y
                || target.y > bounds.iter_max_y
                || target.z < bounds.iter_min_z
                || target.z > bounds.iter_max_z
                || radius <= 0.0
            {
                return false;
            }
            let xd = (target.x as f64 + 0.5 - x) / radius;
            let yd = (target.y as f64 + 0.5 - y) / radius;
            let zd = (target.z as f64 + 0.5 - z) / radius;
            xd * xd < 1.0 && xd * xd + yd * yd < 1.0 && xd * xd + yd * yd + zd * zd < 1.0
        });
        let target_index = if target.x >= shape.min_box_x
            && target.y >= shape.min_box_y
            && target.z >= shape.min_box_z
        {
            Some(
                (target.x - shape.min_box_x) as usize
                    + (target.y - shape.min_box_y) as usize * shape.tested_stride_x
                    + (target.z - shape.min_box_z) as usize
                        * shape.tested_stride_x
                        * shape.tested_stride_y,
            )
        } else {
            None
        };
        let previous = local_coords(target.x, target.z, chunk_min_x, chunk_min_z)
            .and_then(|(local_x, local_z)| chunk.layer(local_x, target.y, local_z, settings.min_y));
        let target_predicate = previous
            .and_then(|layer| {
                self.targets
                    .iter()
                    .find(|target| target.predicate.matches(layer))
                    .map(|target| format!("{:?}", target.predicate))
            })
            .unwrap_or_else(|| "none".to_string());
        let previous = previous
            .map(|layer| layer.block.as_ref())
            .unwrap_or("outside_or_missing");
        let bounds = bounds
            .map(|bounds| {
                format!(
                    "raw=(x={}..{} y={}..{} z={}..{}) clamped=(x={}..{} y={}..{} z={}..{}) iter=(x={}..{} y={}..{} z={}..{})",
                    bounds.floor_min_x,
                    bounds.floor_max_x,
                    bounds.floor_min_y,
                    bounds.floor_max_y,
                    bounds.floor_min_z,
                    bounds.floor_max_z,
                    bounds.raw_min_x,
                    bounds.raw_max_x,
                    bounds.raw_min_y,
                    bounds.raw_max_y,
                    bounds.raw_min_z,
                    bounds.raw_max_z,
                    bounds.iter_min_x,
                    bounds.iter_max_x,
                    bounds.iter_min_y,
                    bounds.iter_max_y,
                    bounds.iter_min_z,
                    bounds.iter_max_z
                )
            })
            .unwrap_or_else(|| "culled".to_string());

        eprintln!(
            "ore blob trace sphere: target=({},{},{}) sphere={} center=({:.6},{:.6},{:.6}) radius={:.6} bounds={} contains_target={} target_bit={:?} tested_dims=({}, {}, {}) tested_strides=({}, {}) previous={} target_predicate={}",
            target.x,
            target.y,
            target.z,
            sphere_index,
            x,
            y,
            z,
            radius,
            bounds,
            inside,
            target_index,
            shape.tested_size_x,
            shape.tested_size_y,
            shape.tested_size_z,
            shape.tested_stride_x,
            shape.tested_stride_y,
            previous,
            target_predicate
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn trace_tested_bit_at_target(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &NoiseChunkBlocks,
        neighbor: Option<(i32, i32, &NoiseChunkBlocks)>,
        context_chunks: &[(i32, i32, &NoiseChunkBlocks)],
        world_x: i32,
        world_y: i32,
        world_z: i32,
        sphere_index: usize,
        tested_index: usize,
        tested_before: bool,
    ) {
        if !FeatureWriteTraceTarget::from_env()
            .is_some_and(|target| target.matches(world_x, world_y, world_z))
            || !self.trace_context_is_target_attempt()
        {
            return;
        }

        let previous = layer_at_world(
            chunk,
            chunk_min_x,
            chunk_min_z,
            world_x,
            world_y,
            world_z,
            settings.min_y,
        )
        .or_else(|| {
            neighbor.and_then(|(neighbor_min_x, neighbor_min_z, neighbor_chunk)| {
                layer_at_world(
                    neighbor_chunk,
                    neighbor_min_x,
                    neighbor_min_z,
                    world_x,
                    world_y,
                    world_z,
                    settings.min_y,
                )
            })
        })
        .or_else(|| {
            context_chunk_at(context_chunks, world_x, world_z).and_then(
                |(context_min_x, context_min_z, context_chunk)| {
                    layer_at_world(
                        context_chunk,
                        context_min_x,
                        context_min_z,
                        world_x,
                        world_y,
                        world_z,
                        settings.min_y,
                    )
                },
            )
        });
        let target_predicate = previous
            .and_then(|layer| {
                self.targets
                    .iter()
                    .find(|target| target.predicate.matches(layer))
                    .map(|target| format!("{:?}", target.predicate))
            })
            .unwrap_or_else(|| "none".to_string());
        let previous = previous
            .map(|layer| layer.block.as_ref())
            .unwrap_or("outside_or_missing");
        eprintln!(
            "ore blob trace target bit: coord=({world_x},{world_y},{world_z}) sphere={} tested_index={} tested_before={} previous={} target_predicate={}",
            sphere_index, tested_index, tested_before, previous, target_predicate
        );
    }

    fn trace_context_is_target_attempt(&self) -> bool {
        FEATURE_WRITE_TRACE_CONTEXT.with(|current| {
            current.borrow().as_ref().is_some_and(|context| {
                FeatureWriteTraceFilter::from_env().is_none_or(|filter| filter.matches(context))
            })
        })
    }

    fn sample_blob_prefix(
        &self,
        random: &mut FeatureRandom,
        origin_x: i32,
        origin_y: i32,
        origin_z: i32,
    ) -> OreBlobPrefix {
        let direction = random.next_float() * std::f32::consts::PI;
        let spread_xy = self.size as f64 / 8.0;
        let precheck_radius = ((self.size as f32 / 16.0) * 2.0 + 1.0) / 2.0;
        let precheck_radius = precheck_radius.ceil() as i32;
        let spread_xy_ceil = spread_xy.ceil() as i32;
        let x_spread = (direction as f64).sin() * spread_xy;
        let z_spread = (direction as f64).cos() * spread_xy;

        OreBlobPrefix {
            x0: origin_x as f64 + x_spread,
            x1: origin_x as f64 - x_spread,
            y0: origin_y as f64 + random.next_int(3) as f64 - 2.0,
            y1: origin_y as f64 + random.next_int(3) as f64 - 2.0,
            z0: origin_z as f64 + z_spread,
            z1: origin_z as f64 - z_spread,
            min_box_x: origin_x - spread_xy_ceil - precheck_radius,
            min_box_y: origin_y - 2 - precheck_radius,
            min_box_z: origin_z - spread_xy_ceil - precheck_radius,
            tested_size_x: (2 * (spread_xy_ceil + precheck_radius)) as usize,
            tested_size_y: (2 * (2 + precheck_radius)) as usize,
            tested_size_z: (2 * (spread_xy_ceil + precheck_radius)) as usize,
            tested_stride_x: (2 * (spread_xy_ceil + precheck_radius)) as usize,
            tested_stride_y: (2 * (2 + precheck_radius)) as usize,
        }
    }

    fn sample_blob_shape(&self, random: &mut FeatureRandom, prefix: OreBlobPrefix) -> OreBlobShape {
        let mut spheres = vec![[0.0; 4]; self.size as usize];

        for i in 0..self.size {
            let step = i as f32 / self.size as f32;
            let radius_noise = random.next_double() * self.size as f64 / 16.0;
            let radius =
                ((mth_sin(std::f32::consts::PI * step) + 1.0) as f64 * radius_noise + 1.0) / 2.0;
            let x = lerp_f64(step as f64, prefix.x0, prefix.x1);
            let y = lerp_f64(step as f64, prefix.y0, prefix.y1);
            let z = lerp_f64(step as f64, prefix.z0, prefix.z1);
            spheres[i as usize] = [x, y, z, radius];
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

        OreBlobShape {
            spheres,
            min_box_x: prefix.min_box_x,
            min_box_y: prefix.min_box_y,
            min_box_z: prefix.min_box_z,
            tested_size_x: prefix.tested_size_x,
            tested_size_y: prefix.tested_size_y,
            tested_size_z: prefix.tested_size_z,
            tested_stride_x: prefix.tested_stride_x,
            tested_stride_y: prefix.tested_stride_y,
        }
    }

    #[cfg(test)]
    fn sample_blob_shape_diagnostic(
        &self,
        random: &mut FeatureRandom,
        prefix: OreBlobPrefix,
    ) -> OreBlobShapeDiagnostic {
        let mut spheres = vec![[0.0; 4]; self.size as usize];
        let mut diagnostics = Vec::with_capacity(self.size as usize);

        for i in 0..self.size {
            let step = i as f32 / self.size as f32;
            let sin = mth_sin(std::f32::consts::PI * step);
            let radius_noise = random.next_double() * self.size as f64 / 16.0;
            let radius = ((sin + 1.0) as f64 * radius_noise + 1.0) / 2.0;
            let x = lerp_f64(step as f64, prefix.x0, prefix.x1);
            let y = lerp_f64(step as f64, prefix.y0, prefix.y1);
            let z = lerp_f64(step as f64, prefix.z0, prefix.z1);
            spheres[i as usize] = [x, y, z, radius];
            diagnostics.push(OreSphereDiagnostic {
                index: i,
                step,
                sin,
                radius_noise,
                x,
                y,
                z,
                radius_before_cull: radius,
                radius_after_cull: radius,
            });
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
                        diagnostics[i2].radius_after_cull = -1.0;
                    } else {
                        spheres[i1][3] = -1.0;
                        diagnostics[i1].radius_after_cull = -1.0;
                    }
                }
            }
        }

        OreBlobShapeDiagnostic {
            prefix: prefix.clone(),
            spheres: diagnostics,
            shape: OreBlobShape {
                spheres,
                min_box_x: prefix.min_box_x,
                min_box_y: prefix.min_box_y,
                min_box_z: prefix.min_box_z,
                tested_size_x: prefix.tested_size_x,
                tested_size_y: prefix.tested_size_y,
                tested_size_z: prefix.tested_size_z,
                tested_stride_x: prefix.tested_stride_x,
                tested_stride_y: prefix.tested_stride_y,
            },
        }
    }

    fn shape_may_spill_into(
        &self,
        target_origin_x: i32,
        target_origin_z: i32,
        shape: &OreBlobShape,
    ) -> bool {
        let mut min_x = i32::MAX;
        let mut max_x = i32::MIN;
        let mut min_z = i32::MAX;
        let mut max_z = i32::MIN;

        for [x, _y, z, radius] in shape.spheres.iter().copied() {
            if radius < 0.0 {
                continue;
            }
            min_x = min_x.min(mth_floor(x - radius));
            max_x = max_x.max(mth_floor(x + radius).max(min_x));
            min_z = min_z.min(mth_floor(z - radius));
            max_z = max_z.max(mth_floor(z + radius).max(min_z));
        }

        if min_x == i32::MAX {
            return false;
        }

        horizontal_box_overlaps_chunk(min_x, max_x, min_z, max_z, target_origin_x, target_origin_z)
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
        let prefix = self.sample_blob_prefix(random, origin_x, origin_y, origin_z);
        let shape = self.sample_blob_shape(random, prefix);
        self.shape_may_spill_into(target_origin_x, target_origin_z, &shape)
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
        self.trace_candidate_block(
            settings,
            chunk_min_x,
            chunk_min_z,
            chunk,
            "mutable",
            world_x,
            world_y,
            world_z,
        );
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
        trace_ore_target_predicate_match(world_x, world_y, world_z, current);
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

        trace_feature_write_at_target(
            world_x,
            world_y,
            world_z,
            chunk_min_x,
            chunk_min_z,
            local_x,
            local_z,
            current,
            &ore,
        );
        chunk.set_layer(local_x, world_y, local_z, settings.min_y, ore);
        trace_ore_set_block_state_write();
        true
    }

    fn target_ore(&self, current: &BlockLayer) -> Option<&BlockLayer> {
        self.targets
            .iter()
            .find(|target| target.predicate.matches(current))
            .map(|target| &target.block)
    }

    #[allow(clippy::too_many_arguments)]
    fn try_consume_block_with_context(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &NoiseChunkBlocks,
        context_chunks: &[(i32, i32, &NoiseChunkBlocks)],
        random: &mut FeatureRandom,
        world_x: i32,
        world_y: i32,
        world_z: i32,
    ) {
        self.trace_candidate_block(
            settings,
            chunk_min_x,
            chunk_min_z,
            chunk,
            "context",
            world_x,
            world_y,
            world_z,
        );
        let Some((local_x, local_z)) = local_coords(world_x, world_z, chunk_min_x, chunk_min_z)
        else {
            return;
        };
        let Some(current) = chunk.layer(local_x, world_y, local_z, settings.min_y) else {
            return;
        };
        if self.target_ore(current).is_none() {
            return;
        }
        trace_ore_target_predicate_match(world_x, world_y, world_z, current);
        let _ = self.should_skip_air_check(random)
            || is_adjacent_to_air_in_context(
                settings,
                chunk,
                chunk_min_x,
                chunk_min_z,
                context_chunks,
                world_x,
                world_y,
                world_z,
            );
    }

    fn should_skip_air_check(&self, random: &mut FeatureRandom) -> bool {
        if self.discard_chance_on_air_exposure <= 0.0 {
            true
        } else if self.discard_chance_on_air_exposure >= 1.0 {
            false
        } else {
            trace_ore_should_skip_air_check_next_float();
            random.next_float() >= self.discard_chance_on_air_exposure
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn trace_candidate_block(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &NoiseChunkBlocks,
        owner: &str,
        world_x: i32,
        world_y: i32,
        world_z: i32,
    ) {
        let Some(target) = FeatureWriteTraceTarget::from_env() else {
            return;
        };
        if !self.trace_context_is_target_attempt() || !target.near(world_x, world_y, world_z, 64) {
            return;
        }

        let local = local_coords(world_x, world_z, chunk_min_x, chunk_min_z);
        let current = local
            .and_then(|(local_x, local_z)| chunk.layer(local_x, world_y, local_z, settings.min_y));
        let predicate = current
            .and_then(|layer| {
                self.targets
                    .iter()
                    .find(|target| target.predicate.matches(layer))
                    .map(|target| format!("{:?}", target.predicate))
            })
            .unwrap_or_else(|| "none".to_string());
        let block = current
            .map(|layer| layer.block.as_ref())
            .unwrap_or("outside_or_missing");
        eprintln!(
            "ore blob trace candidate: owner={owner} chunk_origin=({chunk_min_x},{chunk_min_z}) coord=({world_x},{world_y},{world_z}) local={local:?} block={block} target_predicate={predicate}"
        );
    }

    fn start_count_trace_if_enabled(&self) {
        FEATURE_WRITE_TRACE_CONTEXT.with(|current| {
            let Some(context) = current.borrow().clone() else {
                return;
            };
            if FeatureWriteTraceFilter::from_env().is_some_and(|filter| filter.matches(&context)) {
                eprintln!(
                    "rust ore count start: source_origin=({},{}) source_chunk=({},{}) step={} index={} attempt={} origin={}",
                    context.source_origin_x,
                    context.source_origin_z,
                    context.source_chunk_x,
                    context.source_chunk_z,
                    context.step_index,
                    context.feature_index,
                    context
                        .attempt
                        .map(|attempt| attempt.to_string())
                        .unwrap_or_else(|| "none".to_string()),
                    context
                        .attempt_origin
                        .map(|(x, y, z)| format!("({x},{y},{z})"))
                        .unwrap_or_else(|| "none".to_string())
                );
                ORE_PLACEMENT_COUNT_TRACE
                    .with(|trace| trace.replace(Some(OrePlacementCountTrace::default())));
            }
        });
    }

    fn finish_count_trace_if_enabled(&self) {
        FEATURE_WRITE_TRACE_CONTEXT.with(|current| {
            let Some(context) = current.borrow().clone() else {
                return;
            };
            ORE_PLACEMENT_COUNT_TRACE.with(|trace| {
                let Some(count) = trace.replace(None) else {
                    return;
                };
                eprintln!(
                    "rust ore count summary: source_origin=({},{}) source_chunk=({},{}) step={} index={} attempt={} origin={} target_predicate_matches={} shouldSkipAirCheck_nextFloat_calls={} setBlockState_writes={}",
                    context.source_origin_x,
                    context.source_origin_z,
                    context.source_chunk_x,
                    context.source_chunk_z,
                    context.step_index,
                    context.feature_index,
                    context
                        .attempt
                        .map(|attempt| attempt.to_string())
                        .unwrap_or_else(|| "none".to_string()),
                    context
                        .attempt_origin
                        .map(|(x, y, z)| format!("({x},{y},{z})"))
                        .unwrap_or_else(|| "none".to_string()),
                    count.target_predicate_matches,
                    count.should_skip_air_check_next_float_calls,
                    count.set_block_state_writes
                );
            });
        });
    }

    fn trace_precheck_if_enabled(&self, precheck_passes: bool) {
        FEATURE_WRITE_TRACE_CONTEXT.with(|current| {
            let current = current.borrow();
            let Some(context) = current.as_ref() else {
                return;
            };
            if FeatureWriteTraceFilter::from_env().is_some_and(|filter| filter.matches(context)) {
                eprintln!(
                    "rust ore precheck: source_origin=({},{}) source_chunk=({},{}) step={} index={} phase={} attempt={} origin={} passes={}",
                    context.source_origin_x,
                    context.source_origin_z,
                    context.source_chunk_x,
                    context.source_chunk_z,
                    context.step_index,
                    context.feature_index,
                    context.phase,
                    context
                        .attempt
                        .map(|attempt| attempt.to_string())
                        .unwrap_or_else(|| "none".to_string()),
                    context
                        .attempt_origin
                        .map(|(x, y, z)| format!("({x},{y},{z})"))
                        .unwrap_or_else(|| "none".to_string()),
                    precheck_passes
                );
            }
        });
    }

    fn can_skip_non_spilling_blob_replay(&self) -> bool {
        self.discard_chance_on_air_exposure <= 0.0
    }

    fn needs_source_spillover_replay(&self) -> bool {
        self.discard_chance_on_air_exposure > 0.0
    }

    #[allow(clippy::too_many_arguments)]
    fn precheck_passes(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &NoiseChunkBlocks,
        neighbor: Option<(i32, i32, &NoiseChunkBlocks)>,
        origin_x: i32,
        origin_y: i32,
        origin_z: i32,
    ) -> bool {
        self.precheck_passes_counted(
            settings,
            chunk_min_x,
            chunk_min_z,
            chunk,
            neighbor,
            origin_x,
            origin_y,
            origin_z,
        )
        .0
    }

    #[allow(clippy::too_many_arguments)]
    fn precheck_passes_counted(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &NoiseChunkBlocks,
        neighbor: Option<(i32, i32, &NoiseChunkBlocks)>,
        origin_x: i32,
        origin_y: i32,
        origin_z: i32,
    ) -> (bool, usize) {
        let spread_xy = self.size as f32 / 8.0;
        let precheck_radius = ((self.size as f32 / 16.0) * 2.0 + 1.0) / 2.0;
        let precheck_radius = precheck_radius.ceil() as i32;
        let spread_xy = spread_xy.ceil() as i32;
        let min_x = origin_x - spread_xy - precheck_radius;
        let min_y = origin_y - 2 - precheck_radius;
        let min_z = origin_z - spread_xy - precheck_radius;
        let horizontal_size = 2 * (spread_xy + precheck_radius);
        let mut scans = 0;

        for world_x in min_x..=min_x + horizontal_size {
            for world_z in min_z..=min_z + horizontal_size {
                scans += 1;
                if min_y
                    <= ocean_floor_wg_height_at(
                        settings,
                        chunk_min_x,
                        chunk_min_z,
                        chunk,
                        neighbor,
                        world_x,
                        world_z,
                    )
                {
                    return (true, scans);
                }
            }
        }

        (false, scans)
    }

    #[allow(clippy::too_many_arguments)]
    fn precheck_passes_in_chunk(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &NoiseChunkBlocks,
        origin_x: i32,
        origin_y: i32,
        origin_z: i32,
    ) -> bool {
        self.precheck_passes_counted(
            settings,
            chunk_min_x,
            chunk_min_z,
            chunk,
            None,
            origin_x,
            origin_y,
            origin_z,
        )
        .0
    }

    fn max_horizontal_spillover(&self) -> i32 {
        (self.size / 8 + self.size / 16 + 2).max(2)
    }

    fn shape_scan_stats(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        shape: &OreBlobShape,
    ) -> OreShapeScanStats {
        let mut stats = OreShapeScanStats::default();
        for sphere in shape.spheres.iter().copied() {
            let [x, y, z, radius] = sphere;
            if radius < 0.0 {
                continue;
            }

            let raw_min_x = mth_floor(x - radius).max(shape.min_box_x);
            let raw_max_x = mth_floor(x + radius).max(raw_min_x);
            let min_x = raw_min_x.max(chunk_min_x);
            let max_x = raw_max_x.min(chunk_min_x + 15);
            let raw_min_y = mth_floor(y - radius).max(shape.min_box_y);
            let raw_max_y = mth_floor(y + radius).max(raw_min_y);
            let min_y = raw_min_y.max(settings.min_y);
            let max_y = raw_max_y.min(settings.min_y + settings.height - 1);
            let raw_min_z = mth_floor(z - radius).max(shape.min_box_z);
            let raw_max_z = mth_floor(z + radius).max(raw_min_z);
            let min_z = raw_min_z.max(chunk_min_z);
            let max_z = raw_max_z.min(chunk_min_z + 15);
            if min_x > max_x || min_y > max_y || min_z > max_z {
                continue;
            }

            stats.spheres += 1;
            stats.scans += (max_x - min_x + 1) as usize
                * (max_y - min_y + 1) as usize
                * (max_z - min_z + 1) as usize;
        }
        stats
    }

    fn shape_scan_stats_for_context(
        &self,
        settings: &NoiseSettings,
        shape: &OreBlobShape,
        source_min_x: i32,
        source_min_z: i32,
        target_min_x: i32,
        target_min_z: i32,
        context_chunks: &[(i32, i32, &NoiseChunkBlocks)],
    ) -> OreShapeScanStats {
        let bounds = chunk_context_bounds(
            source_min_x,
            source_min_z,
            target_min_x,
            target_min_z,
            context_chunks,
        );
        let mut stats = OreShapeScanStats::default();
        for sphere in shape.spheres.iter().copied() {
            let [x, y, z, radius] = sphere;
            if radius < 0.0 {
                continue;
            }

            let raw_min_x = mth_floor(x - radius).max(shape.min_box_x);
            let raw_max_x = mth_floor(x + radius).max(raw_min_x);
            let min_x = raw_min_x.max(bounds.min_x);
            let max_x = raw_max_x.min(bounds.max_x);
            let raw_min_y = mth_floor(y - radius).max(shape.min_box_y);
            let raw_max_y = mth_floor(y + radius).max(raw_min_y);
            let min_y = raw_min_y.max(settings.min_y);
            let max_y = raw_max_y.min(settings.min_y + settings.height - 1);
            let raw_min_z = mth_floor(z - radius).max(shape.min_box_z);
            let raw_max_z = mth_floor(z + radius).max(raw_min_z);
            let min_z = raw_min_z.max(bounds.min_z);
            let max_z = raw_max_z.min(bounds.max_z);
            if min_x > max_x || min_y > max_y || min_z > max_z {
                continue;
            }

            stats.spheres += 1;
            stats.scans += (max_x - min_x + 1) as usize
                * (max_y - min_y + 1) as usize
                * (max_z - min_z + 1) as usize;
        }
        stats
    }
}

#[allow(clippy::too_many_arguments)]
fn ocean_floor_wg_height_at(
    settings: &NoiseSettings,
    chunk_min_x: i32,
    chunk_min_z: i32,
    chunk: &NoiseChunkBlocks,
    neighbor: Option<(i32, i32, &NoiseChunkBlocks)>,
    world_x: i32,
    world_z: i32,
) -> i32 {
    if let Some((local_x, local_z)) = local_coords(world_x, world_z, chunk_min_x, chunk_min_z) {
        return chunk.ocean_floor_wg_height(local_x, local_z, settings.min_y);
    }
    if let Some((neighbor_min_x, neighbor_min_z, neighbor_chunk)) = neighbor
        && let Some((local_x, local_z)) =
            local_coords(world_x, world_z, neighbor_min_x, neighbor_min_z)
    {
        return neighbor_chunk.ocean_floor_wg_height(local_x, local_z, settings.min_y);
    }

    settings.terrain_ocean_floor_wg_height(world_x, world_z)
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

        if let Some(layer) =
            layer_at_world(chunk, chunk_min_x, chunk_min_z, x, y, z, settings.min_y)
        {
            return layer.is_air;
        }
        if let Some((neighbor_min_x, neighbor_min_z, neighbor_chunk)) = neighbor
            && let Some(layer) = layer_at_world(
                neighbor_chunk,
                neighbor_min_x,
                neighbor_min_z,
                x,
                y,
                z,
                settings.min_y,
            )
        {
            return layer.is_air;
        }
        false
    })
}

fn is_adjacent_to_air_in_context(
    settings: &NoiseSettings,
    chunk: &NoiseChunkBlocks,
    chunk_min_x: i32,
    chunk_min_z: i32,
    context_chunks: &[(i32, i32, &NoiseChunkBlocks)],
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

        if let Some(layer) =
            layer_at_world(chunk, chunk_min_x, chunk_min_z, x, y, z, settings.min_y)
        {
            return layer.is_air;
        }
        for (context_min_x, context_min_z, context_chunk) in context_chunks.iter().copied() {
            if let Some(layer) = layer_at_world(
                context_chunk,
                context_min_x,
                context_min_z,
                x,
                y,
                z,
                settings.min_y,
            ) {
                return layer.is_air;
            }
        }
        false
    })
}

fn chunk_context_bounds(
    source_min_x: i32,
    source_min_z: i32,
    target_min_x: i32,
    target_min_z: i32,
    context_chunks: &[(i32, i32, &NoiseChunkBlocks)],
) -> ChunkContextBounds {
    let mut bounds = ChunkContextBounds {
        min_x: source_min_x.min(target_min_x),
        max_x: (source_min_x + 15).max(target_min_x + 15),
        min_z: source_min_z.min(target_min_z),
        max_z: (source_min_z + 15).max(target_min_z + 15),
    };
    for (chunk_min_x, chunk_min_z, _) in context_chunks.iter().copied() {
        bounds.min_x = bounds.min_x.min(chunk_min_x);
        bounds.max_x = bounds.max_x.max(chunk_min_x + 15);
        bounds.min_z = bounds.min_z.min(chunk_min_z);
        bounds.max_z = bounds.max_z.max(chunk_min_z + 15);
    }
    bounds
}

fn context_chunk_at<'a>(
    context_chunks: &'a [(i32, i32, &'a NoiseChunkBlocks)],
    world_x: i32,
    world_z: i32,
) -> Option<(i32, i32, &'a NoiseChunkBlocks)> {
    context_chunks
        .iter()
        .copied()
        .find(|(chunk_min_x, chunk_min_z, _)| {
            local_coords(world_x, world_z, *chunk_min_x, *chunk_min_z).is_some()
        })
}

fn source_region_context_chunks(
    settings: &NoiseSettings,
    source_origin_x: i32,
    source_origin_z: i32,
    target_origin_x: i32,
    target_origin_z: i32,
) -> Vec<(i32, i32, std::sync::Arc<NoiseChunkBlocks>)> {
    let source_chunk_x = source_origin_x.div_euclid(16);
    let source_chunk_z = source_origin_z.div_euclid(16);
    let mut chunks = Vec::with_capacity(7);

    for dx in -1..=1 {
        for dz in -1..=1 {
            let chunk_x = source_chunk_x + dx;
            let chunk_z = source_chunk_z + dz;
            let origin_x = chunk_x * 16;
            let origin_z = chunk_z * 16;
            if (origin_x == source_origin_x && origin_z == source_origin_z)
                || (origin_x == target_origin_x && origin_z == target_origin_z)
            {
                continue;
            }
            chunks.push((
                origin_x,
                origin_z,
                settings.feature_source_chunk(chunk_x, chunk_z),
            ));
        }
    }

    chunks
}

fn mth_sin(value: f32) -> f32 {
    let index = (value * 10_430.378_f32) as i32 & 65_535;
    ((index as f64 * std::f64::consts::TAU) / 65_536.0).sin() as f32
}

fn mth_cos(value: f32) -> f32 {
    mth_sin(value + std::f32::consts::FRAC_PI_2)
}

fn mth_floor(value: f64) -> i32 {
    let truncated = value as i32;
    if value < truncated as f64 {
        truncated - 1
    } else {
        truncated
    }
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
        random.set_decoration_seed(world_seed, origin_x, origin_z)
    }

    fn for_feature(decoration_seed: i64, feature_index: i32, step_index: i32) -> Self {
        let mut random = Self::new(decoration_seed);
        random.set_feature_seed(decoration_seed, feature_index, step_index);
        random
    }

    fn set_seed(&mut self, seed: i64) {
        self.source.set_seed(seed);
    }

    fn set_decoration_seed(&mut self, world_seed: i64, x: i32, z: i32) -> i64 {
        self.set_seed(world_seed);
        let x_seed = self.next_long() | 1;
        let z_seed = self.next_long() | 1;
        let seed = (x as i64)
            .wrapping_mul(x_seed)
            .wrapping_add((z as i64).wrapping_mul(z_seed))
            ^ world_seed;
        self.set_seed(seed);
        seed
    }

    fn set_feature_seed(&mut self, decoration_seed: i64, feature_index: i32, step_index: i32) {
        self.set_seed(
            decoration_seed
                .wrapping_add(feature_index as i64)
                .wrapping_add((10_000 * step_index) as i64),
        );
    }

    fn next_int(&mut self, bound: i32) -> i32 {
        assert!(bound > 0);
        if bound & (bound - 1) == 0 {
            return (((bound as i64) * (self.next_bits(31) as i64)) >> 31) as i32;
        }

        loop {
            let sample = self.next_bits(31) as i32;
            let modulo = sample % bound;
            if sample.wrapping_sub(modulo).wrapping_add(bound - 1) >= 0 {
                return modulo;
            }
        }
    }

    fn next_long(&mut self) -> i64 {
        let high = (self.next_bits(32) as i32 as i64) << 32;
        let low = self.next_bits(32) as i32 as i64;
        high.wrapping_add(low)
    }

    fn next_bits(&mut self, bits: u32) -> u32 {
        self.source.next_bits(bits)
    }

    fn next_float(&mut self) -> f32 {
        self.source.next_float()
    }

    fn next_bool(&mut self) -> bool {
        self.next_bits(1) != 0
    }

    fn next_double(&mut self) -> f64 {
        let high = self.next_bits(26) as u64;
        let low = self.next_bits(27) as u64;
        ((high << 27) | low) as f64 * (1.0 / ((1_u64 << 53) as f64))
    }
}

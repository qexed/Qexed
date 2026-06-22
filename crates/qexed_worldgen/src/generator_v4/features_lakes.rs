#[derive(Debug, Clone)]
struct PlacedLakeFeature {
    step_index: i32,
    feature_index: i32,
    placement: LakePlacement,
    biome_filter: FeatureBiomeFilter,
}

impl PlacedLakeFeature {
    fn lava_underground(feature_index: i32) -> Self {
        Self {
            step_index: 1,
            feature_index,
            placement: LakePlacement::Underground {
                rarity: 9,
                height: OreHeight::Uniform(HeightAnchor::Absolute(0), HeightAnchor::BelowTop(0)),
                max_scan_steps: 32,
            },
            biome_filter: FeatureBiomeFilter::All,
        }
    }

    fn lava_surface(feature_index: i32) -> Self {
        Self {
            step_index: 1,
            feature_index,
            placement: LakePlacement::Surface { rarity: 200 },
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
        let Some((world_x, world_y, world_z)) =
            self.sample_origin(settings, origin_x, origin_z, chunk, random)
        else {
            return;
        };

        if self
            .biome_filter
            .allows_at(&settings.density, world_x, world_y, world_z)
        {
            LakeFeatureConfig::lava().place(
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
        let Some((world_x, world_y, world_z)) =
            self.sample_origin(settings, source_origin_x, source_origin_z, source_chunk, random)
        else {
            return;
        };

        if !self
            .biome_filter
            .allows_at(&settings.density, world_x, world_y, world_z)
        {
            return;
        }

        let config = LakeFeatureConfig::lava();
        config.place_with_spillover(
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

    fn may_spill_into(
        &self,
        settings: &NoiseSettings,
        source_origin_x: i32,
        source_origin_z: i32,
        target_origin_x: i32,
        target_origin_z: i32,
        random: &mut FeatureRandom,
    ) -> bool {
        let Some((world_x, _world_y, world_z)) =
            self.sample_origin_position(settings, source_origin_x, source_origin_z, random)
        else {
            return false;
        };
        horizontal_box_overlaps_chunk(
            world_x - 8,
            world_x + 7,
            world_z - 8,
            world_z + 7,
            target_origin_x,
            target_origin_z,
        )
    }

    fn sample_origin(
        &self,
        settings: &NoiseSettings,
        origin_x: i32,
        origin_z: i32,
        chunk: &NoiseChunkBlocks,
        random: &mut FeatureRandom,
    ) -> Option<(i32, i32, i32)> {
        match self.placement {
            LakePlacement::Underground {
                rarity,
                height,
                max_scan_steps,
            } => {
                if random.next_float() >= 1.0 / rarity as f32 {
                    return None;
                }
                let world_x = origin_x + random.next_int(16);
                let world_z = origin_z + random.next_int(16);
                let sampled_y = height.sample(settings, random);
                let local_x = (world_x - origin_x) as usize;
                let local_z = (world_z - origin_z) as usize;
                let world_y = scan_down_to_solid(
                    settings,
                    chunk,
                    local_x,
                    sampled_y,
                    local_z,
                    max_scan_steps,
                )?;
                let ocean_floor = chunk.ocean_floor_wg_height(local_x, local_z, settings.min_y);
                (world_y <= ocean_floor - 5).then_some((world_x, world_y, world_z))
            }
            LakePlacement::Surface { rarity } => {
                if random.next_float() >= 1.0 / rarity as f32 {
                    return None;
                }
                let world_x = origin_x + random.next_int(16);
                let world_z = origin_z + random.next_int(16);
                let local_x = (world_x - origin_x) as usize;
                let local_z = (world_z - origin_z) as usize;
                let world_y = chunk.world_surface_wg_height(local_x, local_z, settings.min_y);
                (world_y > settings.min_y).then_some((world_x, world_y, world_z))
            }
        }
    }

    fn sample_origin_position(
        &self,
        settings: &NoiseSettings,
        origin_x: i32,
        origin_z: i32,
        random: &mut FeatureRandom,
    ) -> Option<(i32, i32, i32)> {
        match self.placement {
            LakePlacement::Underground { rarity, height, .. } => {
                if random.next_float() >= 1.0 / rarity as f32 {
                    return None;
                }
                Some((
                    origin_x + random.next_int(16),
                    height.sample(settings, random),
                    origin_z + random.next_int(16),
                ))
            }
            LakePlacement::Surface { rarity } => {
                if random.next_float() >= 1.0 / rarity as f32 {
                    return None;
                }
                Some((
                    origin_x + random.next_int(16),
                    settings.sea_level,
                    origin_z + random.next_int(16),
                ))
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum LakePlacement {
    Underground {
        rarity: i32,
        height: OreHeight,
        max_scan_steps: i32,
    },
    Surface {
        rarity: i32,
    },
}

#[derive(Debug, Clone)]
struct LakeFeatureConfig;

impl LakeFeatureConfig {
    fn lava() -> Self {
        Self
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
        if origin_y <= settings.min_y + 4 {
            return false;
        }

        let base_x = origin_x - 8;
        let base_y = origin_y - 4;
        let base_z = origin_z - 8;
        let mut grid = vec![false; 16 * 16 * 8];
        let spots = random.next_int(4) + 4;

        for _ in 0..spots {
            let xr = random.next_double() * 6.0 + 3.0;
            let yr = random.next_double() * 4.0 + 2.0;
            let zr = random.next_double() * 6.0 + 3.0;
            let xp = random.next_double() * (16.0 - xr - 2.0) + 1.0 + xr / 2.0;
            let yp = random.next_double() * (8.0 - yr - 4.0) + 2.0 + yr / 2.0;
            let zp = random.next_double() * (16.0 - zr - 2.0) + 1.0 + zr / 2.0;

            for xx in 1..15 {
                for zz in 1..15 {
                    for yy in 1..7 {
                        let xd = (xx as f64 - xp) / (xr / 2.0);
                        let yd = (yy as f64 - yp) / (yr / 2.0);
                        let zd = (zz as f64 - zp) / (zr / 2.0);
                        if xd * xd + yd * yd + zd * zd < 1.0 {
                            grid[lake_index(xx, yy, zz)] = true;
                        }
                    }
                }
            }
        }

        if !self.can_place(
            settings,
            chunk_min_x,
            chunk_min_z,
            chunk,
            base_x,
            base_y,
            base_z,
            &grid,
        ) {
            return false;
        }

        for xx in 0..16 {
            for zz in 0..16 {
                for yy in 0..8 {
                    if !grid[lake_index(xx, yy, zz)] {
                        continue;
                    }
                    let world_x = base_x + xx as i32;
                    let world_y = base_y + yy as i32;
                    let world_z = base_z + zz as i32;
                    let Some(local_x) = local_coord(world_x, chunk_min_x) else {
                        continue;
                    };
                    let Some(local_z) = local_coord(world_z, chunk_min_z) else {
                        continue;
                    };
                    if chunk
                        .layer(local_x, world_y, local_z, settings.min_y)
                        .is_some_and(can_lake_replace_block)
                    {
                        let layer = if yy >= 4 {
                            settings.cave_air_block.clone()
                        } else {
                            settings.lava_lake_fluid_block.clone()
                        };
                        chunk.set_layer(local_x, world_y, local_z, settings.min_y, layer);
                    }
                }
            }
        }

        for xx in 0..16 {
            for zz in 0..16 {
                for yy in 0..8 {
                    if grid[lake_index(xx, yy, zz)]
                        || !is_lake_boundary(&grid, xx, yy, zz)
                        || (yy >= 4 && random.next_int(2) == 0)
                    {
                        continue;
                    }
                    let world_x = base_x + xx as i32;
                    let world_y = base_y + yy as i32;
                    let world_z = base_z + zz as i32;
                    let Some(local_x) = local_coord(world_x, chunk_min_x) else {
                        continue;
                    };
                    let Some(local_z) = local_coord(world_z, chunk_min_z) else {
                        continue;
                    };
                    if chunk
                        .layer(local_x, world_y, local_z, settings.min_y)
                        .is_some_and(can_lava_lake_barrier_replace_block)
                    {
                        chunk.set_layer(
                            local_x,
                            world_y,
                            local_z,
                            settings.min_y,
                            settings.lava_lake_barrier_block.clone(),
                        );
                    }
                }
            }
        }

        true
    }

    #[allow(clippy::too_many_arguments)]
    fn place_with_spillover(
        &self,
        settings: &NoiseSettings,
        source_min_x: i32,
        source_min_z: i32,
        target_min_x: i32,
        target_min_z: i32,
        source_chunk: &mut NoiseChunkBlocks,
        target_chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
        origin_x: i32,
        origin_y: i32,
        origin_z: i32,
    ) -> bool {
        if origin_y <= settings.min_y + 4 {
            return false;
        }

        let base_x = origin_x - 8;
        let base_y = origin_y - 4;
        let base_z = origin_z - 8;
        let grid = self.sample_grid(random);

        if !self.can_place_in_context(
            settings,
            source_min_x,
            source_min_z,
            source_chunk,
            target_min_x,
            target_min_z,
            target_chunk,
            base_x,
            base_y,
            base_z,
            &grid,
        ) {
            return false;
        }

        self.place_body_in_context(
            settings,
            source_min_x,
            source_min_z,
            source_chunk,
            target_min_x,
            target_min_z,
            target_chunk,
            base_x,
            base_y,
            base_z,
            &grid,
        );
        self.place_barrier_in_context(
            settings,
            source_min_x,
            source_min_z,
            source_chunk,
            target_min_x,
            target_min_z,
            target_chunk,
            random,
            base_x,
            base_y,
            base_z,
            &grid,
        );
        true
    }

    fn sample_grid(&self, random: &mut FeatureRandom) -> Vec<bool> {
        let mut grid = vec![false; 16 * 16 * 8];
        let spots = random.next_int(4) + 4;

        for _ in 0..spots {
            let xr = random.next_double() * 6.0 + 3.0;
            let yr = random.next_double() * 4.0 + 2.0;
            let zr = random.next_double() * 6.0 + 3.0;
            let xp = random.next_double() * (16.0 - xr - 2.0) + 1.0 + xr / 2.0;
            let yp = random.next_double() * (8.0 - yr - 4.0) + 2.0 + yr / 2.0;
            let zp = random.next_double() * (16.0 - zr - 2.0) + 1.0 + zr / 2.0;

            for xx in 1..15 {
                for zz in 1..15 {
                    for yy in 1..7 {
                        let xd = (xx as f64 - xp) / (xr / 2.0);
                        let yd = (yy as f64 - yp) / (yr / 2.0);
                        let zd = (zz as f64 - zp) / (zr / 2.0);
                        if xd * xd + yd * yd + zd * zd < 1.0 {
                            grid[lake_index(xx, yy, zz)] = true;
                        }
                    }
                }
            }
        }

        grid
    }

    #[allow(clippy::too_many_arguments)]
    fn can_place(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &NoiseChunkBlocks,
        base_x: i32,
        base_y: i32,
        base_z: i32,
        grid: &[bool],
    ) -> bool {
        for xx in 0..16 {
            for zz in 0..16 {
                for yy in 0..8 {
                    if grid[lake_index(xx, yy, zz)] || !is_lake_boundary(grid, xx, yy, zz) {
                        continue;
                    }
                    let world_x = base_x + xx as i32;
                    let world_y = base_y + yy as i32;
                    let world_z = base_z + zz as i32;
                    let Some(local_x) = local_coord(world_x, chunk_min_x) else {
                        continue;
                    };
                    let Some(local_z) = local_coord(world_z, chunk_min_z) else {
                        continue;
                    };
                    let Some(layer) = chunk.layer(local_x, world_y, local_z, settings.min_y) else {
                        return false;
                    };
                    if yy >= 4 && is_fluid_layer(layer) {
                        return false;
                    }
                    if yy < 4 && !is_full_solid_layer(layer) && !layer.is("minecraft:lava") {
                        return false;
                    }
                }
            }
        }

        true
    }

    #[allow(clippy::too_many_arguments)]
    fn can_place_in_context(
        &self,
        settings: &NoiseSettings,
        source_min_x: i32,
        source_min_z: i32,
        source_chunk: &NoiseChunkBlocks,
        target_min_x: i32,
        target_min_z: i32,
        target_chunk: &NoiseChunkBlocks,
        base_x: i32,
        base_y: i32,
        base_z: i32,
        grid: &[bool],
    ) -> bool {
        for xx in 0..16 {
            for zz in 0..16 {
                for yy in 0..8 {
                    if grid[lake_index(xx, yy, zz)] || !is_lake_boundary(grid, xx, yy, zz) {
                        continue;
                    }
                    let world_x = base_x + xx as i32;
                    let world_y = base_y + yy as i32;
                    let world_z = base_z + zz as i32;
                    let Some(layer) = lake_context_layer(
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
                        if overlaps_chunk(world_x, world_z, source_min_x, source_min_z)
                            || overlaps_chunk(world_x, world_z, target_min_x, target_min_z)
                        {
                            return false;
                        }
                        continue;
                    };
                    if yy >= 4 && is_fluid_layer(layer) {
                        return false;
                    }
                    if yy < 4 && !is_full_solid_layer(layer) && !layer.is("minecraft:lava") {
                        return false;
                    }
                }
            }
        }

        true
    }

    #[allow(clippy::too_many_arguments)]
    fn place_body_in_context(
        &self,
        settings: &NoiseSettings,
        source_min_x: i32,
        source_min_z: i32,
        source_chunk: &mut NoiseChunkBlocks,
        target_min_x: i32,
        target_min_z: i32,
        target_chunk: &mut NoiseChunkBlocks,
        base_x: i32,
        base_y: i32,
        base_z: i32,
        grid: &[bool],
    ) {
        for xx in 0..16 {
            for zz in 0..16 {
                for yy in 0..8 {
                    if !grid[lake_index(xx, yy, zz)] {
                        continue;
                    }
                    let world_x = base_x + xx as i32;
                    let world_y = base_y + yy as i32;
                    let world_z = base_z + zz as i32;
                    if lake_context_layer(
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
                    .is_some_and(can_lake_replace_block)
                    {
                        let layer = if yy >= 4 {
                            settings.cave_air_block.clone()
                        } else {
                            settings.lava_lake_fluid_block.clone()
                        };
                        set_lake_block_in_context(
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
                            layer,
                        );
                    }
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn place_barrier_in_context(
        &self,
        settings: &NoiseSettings,
        source_min_x: i32,
        source_min_z: i32,
        source_chunk: &mut NoiseChunkBlocks,
        target_min_x: i32,
        target_min_z: i32,
        target_chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
        base_x: i32,
        base_y: i32,
        base_z: i32,
        grid: &[bool],
    ) {
        for xx in 0..16 {
            for zz in 0..16 {
                for yy in 0..8 {
                    if grid[lake_index(xx, yy, zz)]
                        || !is_lake_boundary(grid, xx, yy, zz)
                        || (yy >= 4 && random.next_int(2) == 0)
                    {
                        continue;
                    }
                    let world_x = base_x + xx as i32;
                    let world_y = base_y + yy as i32;
                    let world_z = base_z + zz as i32;
                    if lake_context_layer(
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
                    .is_some_and(can_lava_lake_barrier_replace_block)
                    {
                        set_lake_block_in_context(
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
                            settings.lava_lake_barrier_block.clone(),
                        );
                    }
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn lake_context_layer<'a>(
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

#[allow(clippy::too_many_arguments)]
fn set_lake_block_in_context(
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
        set_lake_block(
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
        set_lake_block(
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
fn set_lake_block(
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

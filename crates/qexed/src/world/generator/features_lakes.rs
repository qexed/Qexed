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
                        .is_some_and(is_full_solid_layer)
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
}

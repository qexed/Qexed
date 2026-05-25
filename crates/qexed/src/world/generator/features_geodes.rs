#[derive(Debug, Clone)]
struct PlacedGeodeFeature {
    step_index: i32,
    feature_index: i32,
    rarity: i32,
    height: OreHeight,
    config: GeodeFeatureConfig,
    biome_filter: FeatureBiomeFilter,
}

impl PlacedGeodeFeature {
    fn amethyst(feature_index: i32) -> Self {
        Self {
            step_index: 2,
            feature_index,
            rarity: 24,
            height: OreHeight::Uniform(HeightAnchor::AboveBottom(6), HeightAnchor::Absolute(30)),
            config: GeodeFeatureConfig::amethyst(),
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
        if random.next_float() >= 1.0 / self.rarity as f32 {
            return;
        }
        let world_x = origin_x + random.next_int(16);
        let world_z = origin_z + random.next_int(16);
        let world_y = self.height.sample(settings, random);
        if self
            .biome_filter
            .allows_at(&settings.density, world_x, world_y, world_z)
        {
            self.config.place(
                settings, origin_x, origin_z, chunk, random, world_x, world_y, world_z,
            );
        }
    }
}

#[derive(Debug, Clone)]
struct GeodeFeatureConfig {
    filling_block: BlockLayer,
    inner_block: BlockLayer,
    alternate_inner_block: BlockLayer,
    middle_block: BlockLayer,
    outer_block: BlockLayer,
    inner_placements: Vec<BlockLayer>,
    outer_wall_distance: UniformInt,
    distribution_points: UniformInt,
    point_offset: UniformInt,
    min_gen_offset: i32,
    max_gen_offset: i32,
    noise_multiplier: f64,
    invalid_blocks_threshold: i32,
    filling: f64,
    inner_layer: f64,
    middle_layer: f64,
    outer_layer: f64,
    use_potential_placements_chance: f32,
    use_alternate_layer0_chance: f32,
    placements_require_layer0_alternate: bool,
    generate_crack_chance: f32,
    base_crack_size: f64,
    crack_point_offset: i32,
}

impl GeodeFeatureConfig {
    fn amethyst() -> Self {
        Self {
            filling_block: BlockLayer::new("minecraft:air"),
            inner_block: BlockLayer::new("minecraft:amethyst_block"),
            alternate_inner_block: BlockLayer::new("minecraft:budding_amethyst"),
            middle_block: BlockLayer::new("minecraft:calcite"),
            outer_block: BlockLayer::new("minecraft:smooth_basalt"),
            inner_placements: vec![
                amethyst_cluster_block("minecraft:small_amethyst_bud"),
                amethyst_cluster_block("minecraft:medium_amethyst_bud"),
                amethyst_cluster_block("minecraft:large_amethyst_bud"),
                amethyst_cluster_block("minecraft:amethyst_cluster"),
            ],
            outer_wall_distance: UniformInt { min: 4, max: 6 },
            distribution_points: UniformInt { min: 3, max: 4 },
            point_offset: UniformInt { min: 1, max: 2 },
            min_gen_offset: -16,
            max_gen_offset: 16,
            noise_multiplier: 0.05,
            invalid_blocks_threshold: 1,
            filling: 1.7,
            inner_layer: 2.2,
            middle_layer: 3.2,
            outer_layer: 4.2,
            use_potential_placements_chance: 0.35,
            use_alternate_layer0_chance: 0.083,
            placements_require_layer0_alternate: true,
            generate_crack_chance: 0.95,
            base_crack_size: 2.0,
            crack_point_offset: 2,
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
        let num_points = self.distribution_points.sample(random);
        let mut points = Vec::with_capacity(num_points as usize);
        let mut invalid_points = 0;
        for _ in 0..num_points {
            let point = (
                origin_x + self.outer_wall_distance.sample(random),
                origin_y + self.outer_wall_distance.sample(random),
                origin_z + self.outer_wall_distance.sample(random),
            );
            if let Some((local_x, local_z)) =
                local_coords(point.0, point.2, chunk_min_x, chunk_min_z)
                && chunk
                    .layer(local_x, point.1, local_z, settings.min_y)
                    .is_some_and(is_geode_invalid_block)
            {
                invalid_points += 1;
                if invalid_points > self.invalid_blocks_threshold {
                    return false;
                }
            }
            points.push((point, self.point_offset.sample(random)));
        }

        let crack_size_adjustment = num_points as f64 / self.outer_wall_distance.max as f64;
        let inner_air = 1.0 / self.filling.sqrt();
        let innermost_block_layer = 1.0 / (self.inner_layer + crack_size_adjustment).sqrt();
        let inner_crust = 1.0 / (self.middle_layer + crack_size_adjustment).sqrt();
        let outer_crust = 1.0 / (self.outer_layer + crack_size_adjustment).sqrt();
        let crack_size = 1.0
            / (self.base_crack_size
                + random.next_double() / 2.0
                + if num_points > 3 {
                    crack_size_adjustment
                } else {
                    0.0
                })
            .sqrt();
        let should_generate_crack = random.next_float() < self.generate_crack_chance;
        let crack_points = if should_generate_crack {
            self.crack_points(random, origin_x, origin_y, origin_z, num_points)
        } else {
            Vec::new()
        };
        let mut potential_crystal_placements = Vec::new();

        for world_x in origin_x + self.min_gen_offset..=origin_x + self.max_gen_offset {
            let Some(local_x) = local_coord(world_x, chunk_min_x) else {
                continue;
            };
            for world_z in origin_z + self.min_gen_offset..=origin_z + self.max_gen_offset {
                let Some(local_z) = local_coord(world_z, chunk_min_z) else {
                    continue;
                };
                for world_y in origin_y + self.min_gen_offset..=origin_y + self.max_gen_offset {
                    if !(settings.min_y..settings.min_y + settings.height).contains(&world_y) {
                        continue;
                    }
                    let noise_offset =
                        geode_noise(world_x, world_y, world_z) * self.noise_multiplier;
                    let mut dist_sum_shell = 0.0;
                    let mut dist_sum_crack = 0.0;
                    for (point, offset) in &points {
                        dist_sum_shell += inv_sqrt_distance(
                            world_x, world_y, world_z, point.0, point.1, point.2, *offset,
                        ) + noise_offset;
                    }
                    for point in &crack_points {
                        dist_sum_crack += inv_sqrt_distance(
                            world_x,
                            world_y,
                            world_z,
                            point.0,
                            point.1,
                            point.2,
                            self.crack_point_offset,
                        ) + noise_offset;
                    }

                    if dist_sum_shell < outer_crust {
                        continue;
                    }
                    let replacement = if should_generate_crack
                        && dist_sum_crack >= crack_size
                        && dist_sum_shell < inner_air
                    {
                        Some(self.filling_block.clone())
                    } else if dist_sum_shell >= inner_air {
                        Some(self.filling_block.clone())
                    } else if dist_sum_shell >= innermost_block_layer {
                        let use_alternate = random.next_float() < self.use_alternate_layer0_chance;
                        let block = if use_alternate {
                            self.alternate_inner_block.clone()
                        } else {
                            self.inner_block.clone()
                        };
                        if (!self.placements_require_layer0_alternate || use_alternate)
                            && random.next_float() < self.use_potential_placements_chance
                        {
                            potential_crystal_placements.push((world_x, world_y, world_z));
                        }
                        Some(block)
                    } else if dist_sum_shell >= inner_crust {
                        Some(self.middle_block.clone())
                    } else if dist_sum_shell >= outer_crust {
                        Some(self.outer_block.clone())
                    } else {
                        None
                    };

                    if let Some(block) = replacement
                        && chunk
                            .layer(local_x, world_y, local_z, settings.min_y)
                            .is_some_and(can_geode_replace_block)
                    {
                        chunk.set_layer(local_x, world_y, local_z, settings.min_y, block);
                    }
                }
            }
        }

        for (world_x, world_y, world_z) in potential_crystal_placements {
            let block = self.inner_placements
                [random.next_int(self.inner_placements.len() as i32) as usize]
                .clone();
            for (dx, dy, dz, facing) in [
                (0, -1, 0, "down"),
                (0, 1, 0, "up"),
                (0, 0, -1, "north"),
                (0, 0, 1, "south"),
                (-1, 0, 0, "west"),
                (1, 0, 0, "east"),
            ] {
                let place_x = world_x + dx;
                let place_y = world_y + dy;
                let place_z = world_z + dz;
                let Some((local_x, local_z)) =
                    local_coords(place_x, place_z, chunk_min_x, chunk_min_z)
                else {
                    continue;
                };
                let Some(place_state) = chunk.layer(local_x, place_y, local_z, settings.min_y)
                else {
                    continue;
                };
                if can_amethyst_cluster_grow_at(place_state) {
                    let waterlogged = if place_state.is("minecraft:water") {
                        "true"
                    } else {
                        "false"
                    };
                    chunk.set_layer(
                        local_x,
                        place_y,
                        local_z,
                        settings.min_y,
                        block
                            .with_property("facing", facing)
                            .with_property("waterlogged", waterlogged),
                    );
                    break;
                }
            }
        }

        true
    }

    fn crack_points(
        &self,
        random: &mut FeatureRandom,
        origin_x: i32,
        origin_y: i32,
        origin_z: i32,
        num_points: i32,
    ) -> Vec<(i32, i32, i32)> {
        let crack_offset = num_points * 2 + 1;
        match random.next_int(4) {
            0 => vec![
                (origin_x + crack_offset, origin_y + 7, origin_z),
                (origin_x + crack_offset, origin_y + 5, origin_z),
                (origin_x + crack_offset, origin_y + 1, origin_z),
            ],
            1 => vec![
                (origin_x, origin_y + 7, origin_z + crack_offset),
                (origin_x, origin_y + 5, origin_z + crack_offset),
                (origin_x, origin_y + 1, origin_z + crack_offset),
            ],
            2 => vec![
                (
                    origin_x + crack_offset,
                    origin_y + 7,
                    origin_z + crack_offset,
                ),
                (
                    origin_x + crack_offset,
                    origin_y + 5,
                    origin_z + crack_offset,
                ),
                (
                    origin_x + crack_offset,
                    origin_y + 1,
                    origin_z + crack_offset,
                ),
            ],
            _ => vec![
                (origin_x, origin_y + 7, origin_z),
                (origin_x, origin_y + 5, origin_z),
                (origin_x, origin_y + 1, origin_z),
            ],
        }
    }
}

#[derive(Debug, Clone)]
struct PlacedFreezeTopLayerFeature {
    step_index: i32,
    feature_index: i32,
    snow_layer: BlockLayer,
}

impl PlacedFreezeTopLayerFeature {
    fn new(feature_index: i32) -> Self {
        Self {
            step_index: 10,
            feature_index,
            snow_layer: BlockLayer::new("minecraft:snow"),
        }
    }

    fn place(
        &self,
        settings: &NoiseSettings,
        origin_x: i32,
        origin_z: i32,
        chunk: &mut NoiseChunkBlocks,
    ) {
        for local_z in 0..16 {
            for local_x in 0..16 {
                let world_x = origin_x + local_x as i32;
                let world_z = origin_z + local_z as i32;
                let top_y = chunk.world_surface_wg_height(local_x, local_z, settings.min_y);
                if top_y <= settings.min_y || top_y >= settings.min_y + settings.height {
                    continue;
                }
                let biome = settings.density.biome(world_x, top_y, world_z);
                if !is_freezing_biome(biome) {
                    continue;
                }

                let below_y = top_y - 1;
                if chunk
                    .layer(local_x, below_y, local_z, settings.min_y)
                    .is_some_and(is_water_layer)
                {
                    chunk.set_layer(
                        local_x,
                        below_y,
                        local_z,
                        settings.min_y,
                        settings.ice_block.clone(),
                    );
                }

                if chunk
                    .layer(local_x, top_y, local_z, settings.min_y)
                    .is_some_and(|layer| layer.is_air || layer.is("minecraft:snow"))
                    && chunk
                        .layer(local_x, below_y, local_z, settings.min_y)
                        .is_some_and(is_full_solid_layer)
                {
                    chunk.set_layer(
                        local_x,
                        top_y,
                        local_z,
                        settings.min_y,
                        self.snow_layer.clone(),
                    );
                }
            }
        }
    }
}

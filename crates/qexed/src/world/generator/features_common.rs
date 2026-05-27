#[derive(Debug, Clone)]
struct WeightedInt {
    entries: Vec<(i32, i32)>,
    total_weight: i32,
}

impl WeightedInt {
    fn new(entries: &[(i32, i32)]) -> Self {
        Self {
            entries: entries.to_vec(),
            total_weight: entries.iter().map(|(_, weight)| *weight).sum(),
        }
    }

    fn sample(&self, random: &mut FeatureRandom) -> i32 {
        let mut selection = random.next_int(self.total_weight);
        for (value, weight) in &self.entries {
            selection -= *weight;
            if selection < 0 {
                return *value;
            }
        }
        0
    }
}

#[derive(Debug, Clone)]
struct MultifaceGrowthFeatureConfig {
    block: BlockLayer,
    search_range: i32,
    can_place_on_floor: bool,
    can_place_on_ceiling: bool,
    can_place_on_wall: bool,
    chance_of_spreading: f32,
    can_be_placed_on: &'static [&'static str],
}

impl MultifaceGrowthFeatureConfig {
    fn glow_lichen() -> Self {
        Self {
            block: BlockLayer::with_properties(
                "minecraft:glow_lichen",
                &[
                    ("down", "false"),
                    ("east", "false"),
                    ("north", "false"),
                    ("south", "false"),
                    ("up", "false"),
                    ("waterlogged", "false"),
                    ("west", "false"),
                ],
            ),
            search_range: 20,
            can_place_on_floor: false,
            can_place_on_ceiling: true,
            can_place_on_wall: true,
            chance_of_spreading: 0.5,
            can_be_placed_on: GLOW_LICHEN_CAN_BE_PLACED_ON,
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
        let Some(origin_state) = layer_at_world(
            chunk,
            chunk_min_x,
            chunk_min_z,
            origin_x,
            origin_y,
            origin_z,
            settings.min_y,
        ) else {
            return false;
        };
        if !is_air_or_water_layer(origin_state) {
            return false;
        }

        let search_directions = self.shuffled_directions(random);
        if self.place_growth_if_possible(
            settings,
            chunk_min_x,
            chunk_min_z,
            chunk,
            random,
            origin_x,
            origin_y,
            origin_z,
            &search_directions,
        ) {
            return true;
        }

        for search_direction in &search_directions {
            let placement_directions =
                self.shuffled_directions_except(random, direction_opposite(*search_direction));
            for step in 1..=self.search_range {
                let world_x = origin_x + search_direction.0 * step;
                let world_y = origin_y + search_direction.1 * step;
                let world_z = origin_z + search_direction.2 * step;
                let Some(state) = layer_at_world(
                    chunk,
                    chunk_min_x,
                    chunk_min_z,
                    world_x,
                    world_y,
                    world_z,
                    settings.min_y,
                ) else {
                    break;
                };
                if !is_air_or_water_layer(state) && !state.is(self.block.block.as_ref()) {
                    break;
                }
                if self.place_growth_if_possible(
                    settings,
                    chunk_min_x,
                    chunk_min_z,
                    chunk,
                    random,
                    world_x,
                    world_y,
                    world_z,
                    &placement_directions,
                ) {
                    return true;
                }
            }
        }

        false
    }

    #[allow(clippy::too_many_arguments)]
    fn place_with_neighbors(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        neighbors: &[(i32, i32, &NoiseChunkBlocks)],
        random: &mut FeatureRandom,
        origin_x: i32,
        origin_y: i32,
        origin_z: i32,
    ) -> bool {
        let Some(origin_state) = layer_at_world(
            chunk,
            chunk_min_x,
            chunk_min_z,
            origin_x,
            origin_y,
            origin_z,
            settings.min_y,
        ) else {
            return false;
        };
        if !is_air_or_water_layer(origin_state) {
            return false;
        }

        let search_directions = self.shuffled_directions(random);
        if self.place_growth_if_possible_with_neighbors(
            settings,
            chunk_min_x,
            chunk_min_z,
            chunk,
            neighbors,
            random,
            origin_x,
            origin_y,
            origin_z,
            &search_directions,
        ) {
            return true;
        }

        for search_direction in &search_directions {
            let placement_directions =
                self.shuffled_directions_except(random, direction_opposite(*search_direction));
            for step in 1..=self.search_range {
                let world_x = origin_x + search_direction.0 * step;
                let world_y = origin_y + search_direction.1 * step;
                let world_z = origin_z + search_direction.2 * step;
                let Some(state) = layer_at_world(
                    chunk,
                    chunk_min_x,
                    chunk_min_z,
                    world_x,
                    world_y,
                    world_z,
                    settings.min_y,
                ) else {
                    break;
                };
                if !is_air_or_water_layer(state) && !state.is(self.block.block.as_ref()) {
                    break;
                }
                if self.place_growth_if_possible_with_neighbors(
                    settings,
                    chunk_min_x,
                    chunk_min_z,
                    chunk,
                    neighbors,
                    random,
                    world_x,
                    world_y,
                    world_z,
                    &placement_directions,
                ) {
                    return true;
                }
            }
        }

        false
    }

    #[allow(clippy::too_many_arguments)]
    fn place_growth_if_possible(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
        world_x: i32,
        world_y: i32,
        world_z: i32,
        directions: &[(i32, i32, i32, &'static str)],
    ) -> bool {
        for direction in directions {
            let neighbor_x = world_x + direction.0;
            let neighbor_y = world_y + direction.1;
            let neighbor_z = world_z + direction.2;
            let Some(neighbor) = layer_at_world(
                chunk,
                chunk_min_x,
                chunk_min_z,
                neighbor_x,
                neighbor_y,
                neighbor_z,
                settings.min_y,
            ) else {
                continue;
            };
            if !self.can_be_placed_on.contains(&neighbor.block.as_ref()) {
                continue;
            }
            if !self.try_place_face(
                settings,
                chunk_min_x,
                chunk_min_z,
                chunk,
                world_x,
                world_y,
                world_z,
                direction.3,
            ) {
                continue;
            }
            if random.next_float() < self.chance_of_spreading {
                self.spread_once(
                    settings,
                    chunk_min_x,
                    chunk_min_z,
                    chunk,
                    random,
                    world_x,
                    world_y,
                    world_z,
                    *direction,
                );
            }
            return true;
        }
        false
    }

    #[allow(clippy::too_many_arguments)]
    fn place_growth_if_possible_with_neighbors(
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
        directions: &[(i32, i32, i32, &'static str)],
    ) -> bool {
        for direction in directions {
            let neighbor_x = world_x + direction.0;
            let neighbor_y = world_y + direction.1;
            let neighbor_z = world_z + direction.2;
            if !self.can_be_placed_on_at_world(
                chunk,
                chunk_min_x,
                chunk_min_z,
                neighbors,
                neighbor_x,
                neighbor_y,
                neighbor_z,
                settings.min_y,
            ) {
                continue;
            }
            if !self.try_place_face(
                settings,
                chunk_min_x,
                chunk_min_z,
                chunk,
                world_x,
                world_y,
                world_z,
                direction.3,
            ) {
                continue;
            }
            if random.next_float() < self.chance_of_spreading {
                self.spread_once_with_neighbors(
                    settings,
                    chunk_min_x,
                    chunk_min_z,
                    chunk,
                    neighbors,
                    random,
                    world_x,
                    world_y,
                    world_z,
                    *direction,
                );
            }
            return true;
        }
        false
    }

    #[allow(clippy::too_many_arguments)]
    fn replay_growth_if_possible_with_neighbors(
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
        directions: &[(i32, i32, i32, &'static str)],
    ) -> bool {
        for direction in directions {
            let neighbor_x = world_x + direction.0;
            let neighbor_y = world_y + direction.1;
            let neighbor_z = world_z + direction.2;
            if (local_coords(neighbor_x, neighbor_z, chunk_min_x, chunk_min_z).is_some()
                || neighbors.iter().any(|(neighbor_min_x, neighbor_min_z, _)| {
                    local_coords(neighbor_x, neighbor_z, *neighbor_min_x, *neighbor_min_z).is_some()
                }))
                && !self.can_be_placed_on_at_world(
                    chunk,
                    chunk_min_x,
                    chunk_min_z,
                    neighbors,
                    neighbor_x,
                    neighbor_y,
                    neighbor_z,
                    settings.min_y,
                )
            {
                continue;
            }
            let visible_target =
                local_coords(world_x, world_z, chunk_min_x, chunk_min_z).is_some()
                    && (settings.min_y..settings.min_y + settings.height).contains(&world_y);
            let placed = visible_target
                && self.try_place_face(
                    settings,
                    chunk_min_x,
                    chunk_min_z,
                    chunk,
                    world_x,
                    world_y,
                    world_z,
                    direction.3,
                );
            if !placed && !visible_target {
                let _ = self.try_place_face(
                    settings,
                    chunk_min_x,
                    chunk_min_z,
                    chunk,
                    world_x,
                    world_y,
                    world_z,
                    direction.3,
                );
            }
            if random.next_float() < self.chance_of_spreading {
                self.spread_once_with_neighbors(
                    settings,
                    chunk_min_x,
                    chunk_min_z,
                    chunk,
                    neighbors,
                    random,
                    world_x,
                    world_y,
                    world_z,
                    *direction,
                );
            }
            return placed;
        }
        false
    }

    #[allow(clippy::too_many_arguments)]
    fn replay_place_with_neighbors(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        neighbors: &[(i32, i32, &NoiseChunkBlocks)],
        random: &mut FeatureRandom,
        origin_x: i32,
        origin_y: i32,
        origin_z: i32,
    ) -> bool {
        let search_directions = self.shuffled_directions(random);
        let origin_visible = local_coords(origin_x, origin_z, chunk_min_x, chunk_min_z).is_some();
        if (!origin_visible
            || layer_at_world(
                chunk,
                chunk_min_x,
                chunk_min_z,
                origin_x,
                origin_y,
                origin_z,
                settings.min_y,
            )
            .is_some_and(is_air_or_water_layer))
            && self.replay_growth_if_possible_with_neighbors(
                settings,
                chunk_min_x,
                chunk_min_z,
                chunk,
                neighbors,
                random,
                origin_x,
                origin_y,
                origin_z,
                &search_directions,
            )
        {
            return true;
        }

        for search_direction in &search_directions {
            let placement_directions =
                self.shuffled_directions_except(random, direction_opposite(*search_direction));
            for step in 1..=self.search_range {
                let world_x = origin_x + search_direction.0 * step;
                let world_y = origin_y + search_direction.1 * step;
                let world_z = origin_z + search_direction.2 * step;
                if local_coords(world_x, world_z, chunk_min_x, chunk_min_z).is_some() {
                    let Some(state) = layer_at_world(
                        chunk,
                        chunk_min_x,
                        chunk_min_z,
                        world_x,
                        world_y,
                        world_z,
                        settings.min_y,
                    ) else {
                        break;
                    };
                    if !is_air_or_water_layer(state) && !state.is(self.block.block.as_ref()) {
                        break;
                    }
                }
                if self.replay_growth_if_possible_with_neighbors(
                    settings,
                    chunk_min_x,
                    chunk_min_z,
                    chunk,
                    neighbors,
                    random,
                    world_x,
                    world_y,
                    world_z,
                    &placement_directions,
                ) {
                    return true;
                }
            }
        }
        false
    }

    #[allow(clippy::too_many_arguments)]
    fn try_place_face(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        world_x: i32,
        world_y: i32,
        world_z: i32,
        face: &str,
    ) -> bool {
        let Some((local_x, local_z)) = local_coords(world_x, world_z, chunk_min_x, chunk_min_z)
        else {
            return false;
        };
        let Some(current) = chunk.layer(local_x, world_y, local_z, settings.min_y) else {
            return false;
        };
        if !is_air_or_water_layer(current) && !current.is(self.block.block.as_ref()) {
            return false;
        }
        if current.is(self.block.block.as_ref())
            && current
                .properties
                .iter()
                .any(|(name, value)| name == face && value == "true")
        {
            return false;
        }

        let waterlogged = if current.is("minecraft:water") {
            "true"
        } else {
            "false"
        };
        let new_state = if current.is(self.block.block.as_ref()) {
            current.with_property(face, "true")
        } else {
            self.block
                .with_property("waterlogged", waterlogged)
                .with_property(face, "true")
        };
        chunk.set_layer(local_x, world_y, local_z, settings.min_y, new_state);
        true
    }

    #[allow(clippy::too_many_arguments)]
    fn spread_once(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
        world_x: i32,
        world_y: i32,
        world_z: i32,
        from_face: (i32, i32, i32, &'static str),
    ) -> bool {
        for spread_direction in shuffled_all_directions(random) {
            if spread_direction_axis(spread_direction) == spread_direction_axis(from_face) {
                continue;
            }
            for (target_x, target_y, target_z, target_face) in [
                (world_x, world_y, world_z, spread_direction.3),
                (
                    world_x + spread_direction.0,
                    world_y + spread_direction.1,
                    world_z + spread_direction.2,
                    from_face.3,
                ),
                (
                    world_x + spread_direction.0 + from_face.0,
                    world_y + spread_direction.1 + from_face.1,
                    world_z + spread_direction.2 + from_face.2,
                    direction_opposite_name(spread_direction.3),
                ),
            ] {
                let neighbor = direction_by_name(target_face);
                let Some(neighbor_state) = layer_at_world(
                    chunk,
                    chunk_min_x,
                    chunk_min_z,
                    target_x + neighbor.0,
                    target_y + neighbor.1,
                    target_z + neighbor.2,
                    settings.min_y,
                ) else {
                    continue;
                };
                if self
                    .can_be_placed_on
                    .contains(&neighbor_state.block.as_ref())
                    && self.try_place_face(
                        settings,
                        chunk_min_x,
                        chunk_min_z,
                        chunk,
                        target_x,
                        target_y,
                        target_z,
                        target_face,
                    )
                {
                    return true;
                }
            }
        }
        false
    }

    #[allow(clippy::too_many_arguments)]
    fn spread_once_with_neighbors(
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
        from_face: (i32, i32, i32, &'static str),
    ) -> bool {
        for spread_direction in shuffled_all_directions(random) {
            if spread_direction_axis(spread_direction) == spread_direction_axis(from_face) {
                continue;
            }
            for (target_x, target_y, target_z, target_face) in [
                (world_x, world_y, world_z, spread_direction.3),
                (
                    world_x + spread_direction.0,
                    world_y + spread_direction.1,
                    world_z + spread_direction.2,
                    from_face.3,
                ),
                (
                    world_x + spread_direction.0 + from_face.0,
                    world_y + spread_direction.1 + from_face.1,
                    world_z + spread_direction.2 + from_face.2,
                    direction_opposite_name(spread_direction.3),
                ),
            ] {
                let neighbor = direction_by_name(target_face);
                if self.can_be_placed_on_at_world(
                    chunk,
                    chunk_min_x,
                    chunk_min_z,
                    neighbors,
                    target_x + neighbor.0,
                    target_y + neighbor.1,
                    target_z + neighbor.2,
                    settings.min_y,
                ) && self.try_place_face(
                    settings,
                    chunk_min_x,
                    chunk_min_z,
                    chunk,
                    target_x,
                    target_y,
                    target_z,
                    target_face,
                ) {
                    return true;
                }
            }
        }
        false
    }

    #[allow(clippy::too_many_arguments)]
    fn can_be_placed_on_at_world(
        &self,
        chunk: &NoiseChunkBlocks,
        chunk_min_x: i32,
        chunk_min_z: i32,
        neighbors: &[(i32, i32, &NoiseChunkBlocks)],
        world_x: i32,
        world_y: i32,
        world_z: i32,
        min_y: i32,
    ) -> bool {
        if let Some(layer) = layer_at_world(
            chunk,
            chunk_min_x,
            chunk_min_z,
            world_x,
            world_y,
            world_z,
            min_y,
        ) {
            return self.can_be_placed_on.contains(&layer.block.as_ref());
        }

        neighbors.iter().any(|(neighbor_min_x, neighbor_min_z, neighbor_chunk)| {
            layer_at_world(
                neighbor_chunk,
                *neighbor_min_x,
                *neighbor_min_z,
                world_x,
                world_y,
                world_z,
                min_y,
            )
            .is_some_and(|layer| self.can_be_placed_on.contains(&layer.block.as_ref()))
        })
    }

    fn shuffled_directions(
        &self,
        random: &mut FeatureRandom,
    ) -> Vec<(i32, i32, i32, &'static str)> {
        let mut directions = Vec::with_capacity(5);
        if self.can_place_on_ceiling {
            directions.push((0, 1, 0, "up"));
        }
        if self.can_place_on_floor {
            directions.push((0, -1, 0, "down"));
        }
        if self.can_place_on_wall {
            directions.extend([
                (0, 0, -1, "north"),
                (1, 0, 0, "east"),
                (0, 0, 1, "south"),
                (-1, 0, 0, "west"),
            ]);
        }
        shuffle_directions(&mut directions, random);
        directions
    }

    fn shuffled_directions_except(
        &self,
        random: &mut FeatureRandom,
        excluded: (i32, i32, i32, &'static str),
    ) -> Vec<(i32, i32, i32, &'static str)> {
        let mut directions = self.shuffled_directions(random);
        directions.retain(|direction| *direction != excluded);
        directions
    }
}

#[derive(Debug, Clone)]
struct MonsterRoomFeatureConfig {
    cave_air: BlockLayer,
    cobblestone: BlockLayer,
    mossy_cobblestone: BlockLayer,
    chest: BlockLayer,
    spawner: BlockLayer,
}

#[derive(Debug, Clone, Copy)]
struct MonsterRoomShape {
    x_radius: i32,
    z_radius: i32,
    min_x: i32,
    max_x: i32,
    min_z: i32,
    max_z: i32,
}

impl MonsterRoomShape {
    fn overlaps_chunk(self, origin_x: i32, origin_z: i32, chunk_min_x: i32, chunk_min_z: i32) -> bool {
        horizontal_box_overlaps_chunk(
            origin_x + self.min_x,
            origin_x + self.max_x,
            origin_z + self.min_z,
            origin_z + self.max_z,
            chunk_min_x,
            chunk_min_z,
        )
    }
}

impl MonsterRoomFeatureConfig {
    fn new() -> Self {
        Self {
            cave_air: BlockLayer::new("minecraft:cave_air"),
            cobblestone: BlockLayer::new("minecraft:cobblestone"),
            mossy_cobblestone: BlockLayer::new("minecraft:mossy_cobblestone"),
            chest: BlockLayer::new("minecraft:chest"),
            spawner: BlockLayer::new("minecraft:spawner"),
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
        let shape = self.sample_shape(random);
        self.place_resolved(
            settings,
            chunk_min_x,
            chunk_min_z,
            chunk,
            random,
            origin_x,
            origin_y,
            origin_z,
            shape,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn place_with_neighbors(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        neighbors: &[(i32, i32, &NoiseChunkBlocks)],
        random: &mut FeatureRandom,
        origin_x: i32,
        origin_y: i32,
        origin_z: i32,
    ) -> bool {
        let shape = self.sample_shape(random);
        self.place_resolved_with_neighbors(
            settings,
            chunk_min_x,
            chunk_min_z,
            chunk,
            neighbors,
            random,
            origin_x,
            origin_y,
            origin_z,
            shape,
        )
    }

    fn sample_shape(&self, random: &mut FeatureRandom) -> MonsterRoomShape {
        let x_radius = random.next_int(2) + 2;
        let z_radius = random.next_int(2) + 2;
        MonsterRoomShape {
            x_radius,
            z_radius,
            min_x: -x_radius - 1,
            max_x: x_radius + 1,
            min_z: -z_radius - 1,
            max_z: z_radius + 1,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn place_resolved(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
        origin_x: i32,
        origin_y: i32,
        origin_z: i32,
        shape: MonsterRoomShape,
    ) -> bool {
        if !self.can_place(
            settings,
            chunk_min_x,
            chunk_min_z,
            chunk,
            origin_x,
            origin_y,
            origin_z,
            shape,
        ) {
            return false;
        }

        self.place_spillover(
            settings,
            chunk_min_x,
            chunk_min_z,
            chunk,
            random,
            origin_x,
            origin_y,
            origin_z,
            shape,
        );
        true
    }

    #[allow(clippy::too_many_arguments)]
    fn place_resolved_with_neighbors(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        neighbors: &[(i32, i32, &NoiseChunkBlocks)],
        random: &mut FeatureRandom,
        origin_x: i32,
        origin_y: i32,
        origin_z: i32,
        shape: MonsterRoomShape,
    ) -> bool {
        if !self.can_place_with_neighbors(
            settings,
            chunk_min_x,
            chunk_min_z,
            chunk,
            neighbors,
            origin_x,
            origin_y,
            origin_z,
            shape,
        ) {
            return false;
        }

        self.place_spillover_with_neighbors(
            settings,
            chunk_min_x,
            chunk_min_z,
            chunk,
            neighbors,
            random,
            origin_x,
            origin_y,
            origin_z,
            shape,
        );
        true
    }

    #[allow(clippy::too_many_arguments)]
    fn place_spillover(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
        origin_x: i32,
        origin_y: i32,
        origin_z: i32,
        shape: MonsterRoomShape,
    ) {
        self.place_shell_and_room(
            settings,
            chunk_min_x,
            chunk_min_z,
            chunk,
            random,
            origin_x,
            origin_y,
            origin_z,
            shape.min_x,
            shape.max_x,
            shape.min_z,
            shape.max_z,
        );
        self.place_chests(
            settings,
            chunk_min_x,
            chunk_min_z,
            chunk,
            random,
            origin_x,
            origin_y,
            origin_z,
            shape.x_radius,
            shape.z_radius,
        );
        self.place_spawner(
            settings,
            chunk_min_x,
            chunk_min_z,
            chunk,
            random,
            origin_x,
            origin_y,
            origin_z,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn place_spillover_with_neighbors(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        neighbors: &[(i32, i32, &NoiseChunkBlocks)],
        random: &mut FeatureRandom,
        origin_x: i32,
        origin_y: i32,
        origin_z: i32,
        shape: MonsterRoomShape,
    ) {
        self.place_shell_and_room_with_neighbors(
            settings,
            chunk_min_x,
            chunk_min_z,
            chunk,
            neighbors,
            random,
            origin_x,
            origin_y,
            origin_z,
            shape.min_x,
            shape.max_x,
            shape.min_z,
            shape.max_z,
        );
        self.place_chests_with_neighbors(
            settings,
            chunk_min_x,
            chunk_min_z,
            chunk,
            neighbors,
            random,
            origin_x,
            origin_y,
            origin_z,
            shape,
        );
        self.place_spawner(
            settings,
            chunk_min_x,
            chunk_min_z,
            chunk,
            random,
            origin_x,
            origin_y,
            origin_z,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn can_place(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &NoiseChunkBlocks,
        origin_x: i32,
        origin_y: i32,
        origin_z: i32,
        shape: MonsterRoomShape,
    ) -> bool {
        let mut hole_count = 0;
        let mut skipped_horizontal_bounds = false;
        for dx in shape.min_x..=shape.max_x {
            for dy in -1..=4 {
                for dz in shape.min_z..=shape.max_z {
                    let world_x = origin_x + dx;
                    let world_y = origin_y + dy;
                    let world_z = origin_z + dz;
                    let Some((local_x, local_z)) =
                        local_coords(world_x, world_z, chunk_min_x, chunk_min_z)
                    else {
                        skipped_horizontal_bounds = true;
                        continue;
                    };
                    let Some(layer) = chunk.layer(local_x, world_y, local_z, settings.min_y) else {
                        return false;
                    };
                    let solid = is_full_solid_layer(layer);
                    if (dy == -1 || dy == 4) && !solid {
                        return false;
                    }
                    if (dx == shape.min_x
                        || dx == shape.max_x
                        || dz == shape.min_z
                        || dz == shape.max_z)
                        && dy == 0
                        && layer.is_air
                        && is_air_at_world(
                            chunk,
                            chunk_min_x,
                            chunk_min_z,
                            world_x,
                            world_y + 1,
                            world_z,
                            settings.min_y,
                        )
                    {
                        hole_count += 1;
                    }
                }
            }
        }

        (1..=5).contains(&hole_count) || (skipped_horizontal_bounds && hole_count <= 5)
    }

    #[allow(clippy::too_many_arguments)]
    fn can_place_with_neighbors(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &NoiseChunkBlocks,
        neighbors: &[(i32, i32, &NoiseChunkBlocks)],
        origin_x: i32,
        origin_y: i32,
        origin_z: i32,
        shape: MonsterRoomShape,
    ) -> bool {
        let mut hole_count = 0;
        for dx in shape.min_x..=shape.max_x {
            for dy in -1..=4 {
                for dz in shape.min_z..=shape.max_z {
                    let world_x = origin_x + dx;
                    let world_y = origin_y + dy;
                    let world_z = origin_z + dz;
                    let Some(layer) = monster_room_context_layer(
                        settings,
                        chunk,
                        chunk_min_x,
                        chunk_min_z,
                        neighbors,
                        world_x,
                        world_y,
                        world_z,
                    ) else {
                        return false;
                    };
                    let solid = is_full_solid_layer(&layer);
                    if (dy == -1 || dy == 4) && !solid {
                        return false;
                    }
                    if (dx == shape.min_x
                        || dx == shape.max_x
                        || dz == shape.min_z
                        || dz == shape.max_z)
                        && dy == 0
                        && layer.is_air
                        && monster_room_context_layer(
                            settings,
                            chunk,
                            chunk_min_x,
                            chunk_min_z,
                            neighbors,
                            world_x,
                            world_y + 1,
                            world_z,
                        )
                        .is_some_and(|layer| layer.is_air)
                    {
                        hole_count += 1;
                    }
                }
            }
        }

        (1..=5).contains(&hole_count)
    }

    #[allow(clippy::too_many_arguments)]
    fn place_shell_and_room(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
        origin_x: i32,
        origin_y: i32,
        origin_z: i32,
        min_x: i32,
        max_x: i32,
        min_z: i32,
        max_z: i32,
    ) {
        for dx in min_x..=max_x {
            for dy in (-1..=4).rev() {
                for dz in min_z..=max_z {
                    let world_x = origin_x + dx;
                    let world_y = origin_y + dy;
                    let world_z = origin_z + dz;
                    let Some((local_x, local_z)) =
                        local_coords(world_x, world_z, chunk_min_x, chunk_min_z)
                    else {
                        continue;
                    };
                    let Some(current) = chunk.layer(local_x, world_y, local_z, settings.min_y)
                    else {
                        continue;
                    };
                    let boundary = dx == min_x
                        || dy == -1
                        || dz == min_z
                        || dx == max_x
                        || dy == 4
                        || dz == max_z;
                    if boundary {
                        if world_y >= settings.min_y
                            && !is_solid_at_world(
                                chunk,
                                chunk_min_x,
                                chunk_min_z,
                                world_x,
                                world_y - 1,
                                world_z,
                                settings.min_y,
                            )
                        {
                            self.set_room_block(
                                chunk,
                                local_x,
                                world_y,
                                local_z,
                                settings.min_y,
                                self.cave_air.clone(),
                            );
                        } else if is_full_solid_layer(current) && !current.is("minecraft:chest") {
                            let block = if dy == -1 && random.next_int(4) != 0 {
                                self.mossy_cobblestone.clone()
                            } else {
                                self.cobblestone.clone()
                            };
                            self.set_room_block(
                                chunk,
                                local_x,
                                world_y,
                                local_z,
                                settings.min_y,
                                block,
                            );
                        }
                    } else if !current.is("minecraft:chest") && !current.is("minecraft:spawner") {
                        self.set_room_block(
                            chunk,
                            local_x,
                            world_y,
                            local_z,
                            settings.min_y,
                            self.cave_air.clone(),
                        );
                    }
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn place_shell_and_room_with_neighbors(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        neighbors: &[(i32, i32, &NoiseChunkBlocks)],
        random: &mut FeatureRandom,
        origin_x: i32,
        origin_y: i32,
        origin_z: i32,
        min_x: i32,
        max_x: i32,
        min_z: i32,
        max_z: i32,
    ) {
        for dx in min_x..=max_x {
            for dy in (-1..=4).rev() {
                for dz in min_z..=max_z {
                    let world_x = origin_x + dx;
                    let world_y = origin_y + dy;
                    let world_z = origin_z + dz;
                    let Some(current) = monster_room_context_layer(
                        settings,
                        chunk,
                        chunk_min_x,
                        chunk_min_z,
                        neighbors,
                        world_x,
                        world_y,
                        world_z,
                    ) else {
                        continue;
                    };
                    let local = local_coords(world_x, world_z, chunk_min_x, chunk_min_z);
                    let boundary = dx == min_x
                        || dy == -1
                        || dz == min_z
                        || dx == max_x
                        || dy == 4
                        || dz == max_z;
                    if boundary {
                        if world_y >= settings.min_y
                            && !monster_room_context_is_solid(
                                settings,
                                chunk,
                                chunk_min_x,
                                chunk_min_z,
                                neighbors,
                                world_x,
                                world_y - 1,
                                world_z,
                            )
                        {
                            if let Some((local_x, local_z)) = local {
                                self.set_room_block(
                                    chunk,
                                    local_x,
                                    world_y,
                                    local_z,
                                    settings.min_y,
                                    self.cave_air.clone(),
                                );
                            }
                        } else if is_full_solid_layer(&current) && !current.is("minecraft:chest") {
                            let block = if dy == -1 && random.next_int(4) != 0 {
                                self.mossy_cobblestone.clone()
                            } else {
                                self.cobblestone.clone()
                            };
                            if let Some((local_x, local_z)) = local {
                                self.set_room_block(
                                    chunk,
                                    local_x,
                                    world_y,
                                    local_z,
                                    settings.min_y,
                                    block,
                                );
                            }
                        }
                    } else if !current.is("minecraft:chest") && !current.is("minecraft:spawner") {
                        if let Some((local_x, local_z)) = local {
                            self.set_room_block(
                                chunk,
                                local_x,
                                world_y,
                                local_z,
                                settings.min_y,
                                self.cave_air.clone(),
                            );
                        }
                    }
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn place_chests(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
        origin_x: i32,
        origin_y: i32,
        origin_z: i32,
        x_radius: i32,
        z_radius: i32,
    ) {
        for _ in 0..2 {
            for _ in 0..3 {
                let world_x = origin_x + random.next_int(x_radius * 2 + 1) - x_radius;
                let world_z = origin_z + random.next_int(z_radius * 2 + 1) - z_radius;
                let Some((local_x, local_z)) =
                    local_coords(world_x, world_z, chunk_min_x, chunk_min_z)
                else {
                    continue;
                };
                if !chunk
                    .layer(local_x, origin_y, local_z, settings.min_y)
                    .is_some_and(|layer| layer.is_air)
                {
                    continue;
                }

                let wall_count = horizontal_directions()
                    .iter()
                    .filter(|(dx, dz)| {
                        is_solid_at_world(
                            chunk,
                            chunk_min_x,
                            chunk_min_z,
                            world_x + dx,
                            origin_y,
                            world_z + dz,
                            settings.min_y,
                        )
                    })
                    .count();
                if wall_count != 1 {
                    continue;
                }

                let chest = self
                    .chest
                    .with_property(
                        "facing",
                        chest_facing(
                            chunk,
                            chunk_min_x,
                            chunk_min_z,
                            world_x,
                            origin_y,
                            world_z,
                            settings.min_y,
                        ),
                    )
                    .with_property("waterlogged", "false")
                    .with_property("type", "single");
                chunk.set_layer(local_x, origin_y, local_z, settings.min_y, chest);
                chunk.push_block_entity(
                    world_x,
                    origin_y,
                    world_z,
                    CHEST_BLOCK_ENTITY_TYPE_ID,
                    chest_block_entity_nbt(random.next_long()),
                );
                break;
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn place_chests_with_neighbors(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        neighbors: &[(i32, i32, &NoiseChunkBlocks)],
        random: &mut FeatureRandom,
        origin_x: i32,
        origin_y: i32,
        origin_z: i32,
        shape: MonsterRoomShape,
    ) {
        for _ in 0..2 {
            for _ in 0..3 {
                let world_x = origin_x + random.next_int(shape.x_radius * 2 + 1) - shape.x_radius;
                let world_z = origin_z + random.next_int(shape.z_radius * 2 + 1) - shape.z_radius;
                if !monster_room_after_shell_is_air(
                    settings,
                    chunk,
                    chunk_min_x,
                    chunk_min_z,
                    neighbors,
                    origin_x,
                    origin_y,
                    origin_z,
                    shape,
                    world_x,
                    origin_y,
                    world_z,
                ) {
                    continue;
                }

                let wall_count = horizontal_directions()
                    .iter()
                    .filter(|(dx, dz)| {
                        monster_room_after_shell_is_solid(
                            settings,
                            chunk,
                            chunk_min_x,
                            chunk_min_z,
                            neighbors,
                            origin_x,
                            origin_y,
                            origin_z,
                            shape,
                            world_x + dx,
                            origin_y,
                            world_z + dz,
                        )
                    })
                    .count();
                if wall_count != 1 {
                    continue;
                }

                let loot_table_seed = random.next_long();
                if let Some((local_x, local_z)) =
                    local_coords(world_x, world_z, chunk_min_x, chunk_min_z)
                {
                    let chest = self
                        .chest
                        .with_property(
                            "facing",
                            monster_room_chest_facing_with_neighbors(
                                settings,
                                chunk,
                                chunk_min_x,
                                chunk_min_z,
                                neighbors,
                                origin_x,
                                origin_y,
                                origin_z,
                                shape,
                                world_x,
                                origin_y,
                                world_z,
                            ),
                        )
                        .with_property("waterlogged", "false")
                        .with_property("type", "single");
                    chunk.set_layer(local_x, origin_y, local_z, settings.min_y, chest);
                    chunk.push_block_entity(
                        world_x,
                        origin_y,
                        world_z,
                        CHEST_BLOCK_ENTITY_TYPE_ID,
                        chest_block_entity_nbt(loot_table_seed),
                    );
                }
                break;
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn place_spawner(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
        origin_x: i32,
        origin_y: i32,
        origin_z: i32,
    ) {
        let Some((local_x, local_z)) = local_coords(origin_x, origin_z, chunk_min_x, chunk_min_z)
        else {
            return;
        };
        chunk.set_layer(
            local_x,
            origin_y,
            local_z,
            settings.min_y,
            self.spawner.clone(),
        );
        chunk.push_block_entity(
            origin_x,
            origin_y,
            origin_z,
            MOB_SPAWNER_BLOCK_ENTITY_TYPE_ID,
            spawner_block_entity_nbt(random_monster_room_entity(random)),
        );
    }

    fn set_room_block(
        &self,
        chunk: &mut NoiseChunkBlocks,
        local_x: usize,
        world_y: i32,
        local_z: usize,
        min_y: i32,
        block: BlockLayer,
    ) {
        chunk.set_layer(local_x, world_y, local_z, min_y, block);
    }
}

#[allow(clippy::too_many_arguments)]
fn monster_room_context_layer(
    settings: &NoiseSettings,
    chunk: &NoiseChunkBlocks,
    chunk_min_x: i32,
    chunk_min_z: i32,
    neighbors: &[(i32, i32, &NoiseChunkBlocks)],
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

    for (neighbor_min_x, neighbor_min_z, neighbor_chunk) in neighbors {
        if let Some(layer) = layer_at_world(
            neighbor_chunk,
            *neighbor_min_x,
            *neighbor_min_z,
            world_x,
            world_y,
            world_z,
            settings.min_y,
        ) {
            return Some(layer.clone());
        }
    }

    settings.terrain_layer_at(world_x, world_y, world_z)
}

#[allow(clippy::too_many_arguments)]
fn monster_room_context_is_solid(
    settings: &NoiseSettings,
    chunk: &NoiseChunkBlocks,
    chunk_min_x: i32,
    chunk_min_z: i32,
    neighbors: &[(i32, i32, &NoiseChunkBlocks)],
    world_x: i32,
    world_y: i32,
    world_z: i32,
) -> bool {
    monster_room_context_layer(
        settings,
        chunk,
        chunk_min_x,
        chunk_min_z,
        neighbors,
        world_x,
        world_y,
        world_z,
    )
    .is_some_and(|layer| is_full_solid_layer(&layer))
}

#[allow(clippy::too_many_arguments)]
fn monster_room_after_shell_layer(
    settings: &NoiseSettings,
    chunk: &NoiseChunkBlocks,
    chunk_min_x: i32,
    chunk_min_z: i32,
    neighbors: &[(i32, i32, &NoiseChunkBlocks)],
    origin_x: i32,
    origin_y: i32,
    origin_z: i32,
    shape: MonsterRoomShape,
    world_x: i32,
    world_y: i32,
    world_z: i32,
) -> Option<BlockLayer> {
    let dx = world_x - origin_x;
    let dy = world_y - origin_y;
    let dz = world_z - origin_z;
    if !(shape.min_x..=shape.max_x).contains(&dx)
        || !(-1..=4).contains(&dy)
        || !(shape.min_z..=shape.max_z).contains(&dz)
    {
        return monster_room_context_layer(
            settings,
            chunk,
            chunk_min_x,
            chunk_min_z,
            neighbors,
            world_x,
            world_y,
            world_z,
        );
    }

    let current = monster_room_context_layer(
        settings,
        chunk,
        chunk_min_x,
        chunk_min_z,
        neighbors,
        world_x,
        world_y,
        world_z,
    )?;
    let boundary = dx == shape.min_x
        || dy == -1
        || dz == shape.min_z
        || dx == shape.max_x
        || dy == 4
        || dz == shape.max_z;

    if boundary {
        if world_y >= settings.min_y
            && !monster_room_context_is_solid(
                settings,
                chunk,
                chunk_min_x,
                chunk_min_z,
                neighbors,
                world_x,
                world_y - 1,
                world_z,
            )
        {
            return Some(BlockLayer::new("minecraft:cave_air"));
        }
        if is_full_solid_layer(&current) && !current.is("minecraft:chest") {
            return Some(BlockLayer::new("minecraft:cobblestone"));
        }
        return Some(current);
    }

    if current.is("minecraft:chest") || current.is("minecraft:spawner") {
        Some(current)
    } else {
        Some(BlockLayer::new("minecraft:cave_air"))
    }
}

#[allow(clippy::too_many_arguments)]
fn monster_room_after_shell_is_air(
    settings: &NoiseSettings,
    chunk: &NoiseChunkBlocks,
    chunk_min_x: i32,
    chunk_min_z: i32,
    neighbors: &[(i32, i32, &NoiseChunkBlocks)],
    origin_x: i32,
    origin_y: i32,
    origin_z: i32,
    shape: MonsterRoomShape,
    world_x: i32,
    world_y: i32,
    world_z: i32,
) -> bool {
    monster_room_after_shell_layer(
        settings,
        chunk,
        chunk_min_x,
        chunk_min_z,
        neighbors,
        origin_x,
        origin_y,
        origin_z,
        shape,
        world_x,
        world_y,
        world_z,
    )
    .is_some_and(|layer| layer.is_air)
}

#[allow(clippy::too_many_arguments)]
fn monster_room_after_shell_is_solid(
    settings: &NoiseSettings,
    chunk: &NoiseChunkBlocks,
    chunk_min_x: i32,
    chunk_min_z: i32,
    neighbors: &[(i32, i32, &NoiseChunkBlocks)],
    origin_x: i32,
    origin_y: i32,
    origin_z: i32,
    shape: MonsterRoomShape,
    world_x: i32,
    world_y: i32,
    world_z: i32,
) -> bool {
    monster_room_after_shell_layer(
        settings,
        chunk,
        chunk_min_x,
        chunk_min_z,
        neighbors,
        origin_x,
        origin_y,
        origin_z,
        shape,
        world_x,
        world_y,
        world_z,
    )
    .is_some_and(|layer| is_full_solid_layer(&layer))
}

#[allow(clippy::too_many_arguments)]
fn monster_room_chest_facing_with_neighbors(
    settings: &NoiseSettings,
    chunk: &NoiseChunkBlocks,
    chunk_min_x: i32,
    chunk_min_z: i32,
    neighbors: &[(i32, i32, &NoiseChunkBlocks)],
    origin_x: i32,
    origin_y: i32,
    origin_z: i32,
    shape: MonsterRoomShape,
    world_x: i32,
    world_y: i32,
    world_z: i32,
) -> &'static str {
    for (dx, dz, facing) in [
        (0, -1, "south"),
        (1, 0, "west"),
        (0, 1, "north"),
        (-1, 0, "east"),
    ] {
        if monster_room_after_shell_is_solid(
            settings,
            chunk,
            chunk_min_x,
            chunk_min_z,
            neighbors,
            origin_x,
            origin_y,
            origin_z,
            shape,
            world_x + dx,
            world_y,
            world_z + dz,
        ) {
            return facing;
        }
    }
    "north"
}

#[derive(Debug, Clone)]
struct CaveVinesFeatureConfig {
    body_false: BlockLayer,
    body_true: BlockLayer,
    tip_false: BlockLayer,
    tip_true: BlockLayer,
    body_height: WeightedHeight,
}

impl CaveVinesFeatureConfig {
    fn new() -> Self {
        Self {
            body_false: BlockLayer::with_properties(
                "minecraft:cave_vines_plant",
                &[("berries", "false")],
            ),
            body_true: BlockLayer::with_properties(
                "minecraft:cave_vines_plant",
                &[("berries", "true")],
            ),
            tip_false: BlockLayer::with_properties(
                "minecraft:cave_vines",
                &[("age", "23"), ("berries", "false")],
            ),
            tip_true: BlockLayer::with_properties(
                "minecraft:cave_vines",
                &[("age", "23"), ("berries", "true")],
            ),
            body_height: WeightedHeight::cave_vines(),
        }
    }

    fn in_moss() -> Self {
        Self {
            body_height: WeightedHeight::cave_vines_in_moss(),
            ..Self::new()
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
        if !is_air_at_world(
            chunk,
            chunk_min_x,
            chunk_min_z,
            world_x,
            world_y,
            world_z,
            settings.min_y,
        ) || !is_solid_at_world(
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
        let Some((local_x, local_z)) = local_coords(world_x, world_z, chunk_min_x, chunk_min_z)
        else {
            return false;
        };

        let mut body_height = self.body_height.sample(random);
        while body_height > 0
            && !is_air_at_world(
                chunk,
                chunk_min_x,
                chunk_min_z,
                world_x,
                world_y - body_height,
                world_z,
                settings.min_y,
            )
        {
            body_height -= 1;
        }

        for dy in 0..body_height {
            let block = if random.next_int(5) == 0 {
                self.body_true.clone()
            } else {
                self.body_false.clone()
            };
            chunk.set_layer(local_x, world_y - dy, local_z, settings.min_y, block);
        }

        let tip_y = world_y - body_height;
        if !is_air_at_world(
            chunk,
            chunk_min_x,
            chunk_min_z,
            world_x,
            tip_y,
            world_z,
            settings.min_y,
        ) {
            return body_height > 0;
        }
        let age = 23 + random.next_int(3);
        let tip = if random.next_int(5) == 0 {
            self.tip_true.clone()
        } else {
            self.tip_false.clone()
        }
        .with_property("age", &age.to_string());
        chunk.set_layer(local_x, tip_y, local_z, settings.min_y, tip);
        true
    }
}

#[derive(Debug, Clone)]
struct WeightedHeight {
    entries: Vec<(UniformInt, i32)>,
    total_weight: i32,
}

impl WeightedHeight {
    fn cave_vines() -> Self {
        Self {
            entries: vec![
                (UniformInt { min: 0, max: 19 }, 2),
                (UniformInt { min: 0, max: 2 }, 3),
                (UniformInt { min: 0, max: 6 }, 10),
            ],
            total_weight: 15,
        }
    }

    fn cave_vines_in_moss() -> Self {
        Self {
            entries: vec![
                (UniformInt { min: 0, max: 3 }, 5),
                (UniformInt { min: 1, max: 7 }, 1),
            ],
            total_weight: 6,
        }
    }

    fn big_dripleaf_stem() -> Self {
        Self {
            entries: vec![(UniformInt { min: 0, max: 4 }, 2), (UniformInt { min: 0, max: 0 }, 1)],
            total_weight: 3,
        }
    }

    fn sample(&self, random: &mut FeatureRandom) -> i32 {
        let mut selected = random.next_int(self.total_weight);
        for (height, weight) in &self.entries {
            selected -= *weight;
            if selected < 0 {
                return height.sample(random);
            }
        }
        0
    }
}

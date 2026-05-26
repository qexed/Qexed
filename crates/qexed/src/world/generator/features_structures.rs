#[derive(Debug, Clone)]
enum PlacedStructureFeature {
    DesertWell(PlacedDesertWellFeature),
    Fossil(PlacedFossilFeature),
}

impl PlacedStructureFeature {
    fn desert_well(feature_index: i32) -> Self {
        Self::DesertWell(PlacedDesertWellFeature::new(feature_index))
    }

    fn fossil_upper(feature_index: i32) -> Self {
        Self::Fossil(PlacedFossilFeature::upper(feature_index))
    }

    fn fossil_lower(feature_index: i32) -> Self {
        Self::Fossil(PlacedFossilFeature::lower(feature_index))
    }

    fn step_index(&self) -> i32 {
        match self {
            Self::DesertWell(feature) => feature.step_index,
            Self::Fossil(feature) => feature.step_index,
        }
    }

    fn feature_index(&self) -> i32 {
        match self {
            Self::DesertWell(feature) => feature.feature_index,
            Self::Fossil(feature) => feature.feature_index,
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
        match self {
            Self::DesertWell(feature) => feature.place(settings, origin_x, origin_z, chunk, random),
            Self::Fossil(feature) => feature.place(settings, origin_x, origin_z, chunk, random),
        }
    }
}

#[derive(Debug, Clone)]
struct PlacedDesertWellFeature {
    step_index: i32,
    feature_index: i32,
    rarity: i32,
    config: DesertWellFeatureConfig,
    biome_filter: FeatureBiomeFilter,
}

impl PlacedDesertWellFeature {
    fn new(feature_index: i32) -> Self {
        Self {
            step_index: 4,
            feature_index,
            rarity: 1000,
            config: DesertWellFeatureConfig::new(),
            biome_filter: FeatureBiomeFilter::Include(DESERT_WELL_BIOMES),
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
        let Some((local_x, local_z)) = local_coords(world_x, world_z, origin_x, origin_z) else {
            return;
        };
        let world_y = chunk.world_surface_wg_height(local_x, local_z, settings.min_y);
        if world_y <= settings.min_y
            || !self
                .biome_filter
                .allows_at(&settings.density, world_x, world_y, world_z)
        {
            return;
        }
        self.config.place(
            settings, origin_x, origin_z, chunk, random, world_x, world_y, world_z,
        );
    }
}

#[derive(Debug, Clone)]
struct PlacedFossilFeature {
    step_index: i32,
    feature_index: i32,
    rarity: i32,
    height: OreHeight,
    config: FossilFeatureConfig,
    biome_filter: FeatureBiomeFilter,
}

impl PlacedFossilFeature {
    fn upper(feature_index: i32) -> Self {
        Self {
            step_index: 3,
            feature_index,
            rarity: 64,
            height: OreHeight::Uniform(HeightAnchor::Absolute(0), HeightAnchor::BelowTop(0)),
            config: FossilFeatureConfig::new("minecraft:coal_ore"),
            biome_filter: FeatureBiomeFilter::Include(FOSSIL_BIOMES),
        }
    }

    fn lower(feature_index: i32) -> Self {
        Self {
            step_index: 3,
            feature_index,
            rarity: 64,
            height: OreHeight::Uniform(HeightAnchor::AboveBottom(0), HeightAnchor::Absolute(-8)),
            config: FossilFeatureConfig::new("minecraft:diamond_ore"),
            biome_filter: FeatureBiomeFilter::Include(FOSSIL_BIOMES),
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
        if !self
            .biome_filter
            .allows_at(&settings.density, world_x, world_y, world_z)
        {
            return;
        }
        self.config.place(
            settings, origin_x, origin_z, chunk, random, world_x, world_y, world_z,
        );
    }
}

#[derive(Debug, Clone)]
struct DesertWellFeatureConfig {
    sand: BlockLayer,
    sandstone: BlockLayer,
    sand_slab: BlockLayer,
    water: BlockLayer,
    suspicious_sand: BlockLayer,
}

impl DesertWellFeatureConfig {
    fn new() -> Self {
        Self {
            sand: BlockLayer::new("minecraft:sand"),
            sandstone: BlockLayer::new("minecraft:sandstone"),
            sand_slab: BlockLayer::new("minecraft:sandstone_slab"),
            water: BlockLayer::new("minecraft:water"),
            suspicious_sand: BlockLayer::new("minecraft:suspicious_sand"),
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
        let mut origin_y = world_y + 1;
        while origin_y > settings.min_y + 2
            && is_air_at_world(
                chunk,
                chunk_min_x,
                chunk_min_z,
                world_x,
                origin_y,
                world_z,
                settings.min_y,
            )
        {
            origin_y -= 1;
        }

        if !layer_at_world(
            chunk,
            chunk_min_x,
            chunk_min_z,
            world_x,
            origin_y,
            world_z,
            settings.min_y,
        )
        .is_some_and(|layer| layer.is("minecraft:sand"))
        {
            return false;
        }

        for dx in -2..=2 {
            for dz in -2..=2 {
                let empty_below_1 = is_air_at_world(
                    chunk,
                    chunk_min_x,
                    chunk_min_z,
                    world_x + dx,
                    origin_y - 1,
                    world_z + dz,
                    settings.min_y,
                );
                let empty_below_2 = is_air_at_world(
                    chunk,
                    chunk_min_x,
                    chunk_min_z,
                    world_x + dx,
                    origin_y - 2,
                    world_z + dz,
                    settings.min_y,
                );
                if empty_below_1 && empty_below_2 {
                    return false;
                }
            }
        }

        for dy in -2..=0 {
            for dx in -2..=2 {
                for dz in -2..=2 {
                    self.set_block(
                        settings,
                        chunk_min_x,
                        chunk_min_z,
                        chunk,
                        world_x + dx,
                        origin_y + dy,
                        world_z + dz,
                        self.sandstone.clone(),
                    );
                }
            }
        }

        self.set_block(
            settings,
            chunk_min_x,
            chunk_min_z,
            chunk,
            world_x,
            origin_y,
            world_z,
            self.water.clone(),
        );
        for (dx, dz) in horizontal_directions() {
            self.set_block(
                settings,
                chunk_min_x,
                chunk_min_z,
                chunk,
                world_x + dx,
                origin_y,
                world_z + dz,
                self.water.clone(),
            );
        }

        self.set_block(
            settings,
            chunk_min_x,
            chunk_min_z,
            chunk,
            world_x,
            origin_y - 1,
            world_z,
            self.sand.clone(),
        );
        for (dx, dz) in horizontal_directions() {
            self.set_block(
                settings,
                chunk_min_x,
                chunk_min_z,
                chunk,
                world_x + dx,
                origin_y - 1,
                world_z + dz,
                self.sand.clone(),
            );
        }

        for dx in -2..=2 {
            for dz in -2..=2 {
                if dx == -2 || dx == 2 || dz == -2 || dz == 2 {
                    self.set_block(
                        settings,
                        chunk_min_x,
                        chunk_min_z,
                        chunk,
                        world_x + dx,
                        origin_y + 1,
                        world_z + dz,
                        self.sandstone.clone(),
                    );
                }
            }
        }
        for (dx, dz) in [(2, 0), (-2, 0), (0, 2), (0, -2)] {
            self.set_block(
                settings,
                chunk_min_x,
                chunk_min_z,
                chunk,
                world_x + dx,
                origin_y + 1,
                world_z + dz,
                self.sand_slab.clone(),
            );
        }

        for dx in -1..=1 {
            for dz in -1..=1 {
                let block = if dx == 0 && dz == 0 {
                    self.sandstone.clone()
                } else {
                    self.sand_slab.clone()
                };
                self.set_block(
                    settings,
                    chunk_min_x,
                    chunk_min_z,
                    chunk,
                    world_x + dx,
                    origin_y + 4,
                    world_z + dz,
                    block,
                );
            }
        }

        for dy in 1..=3 {
            for (dx, dz) in [(-1, -1), (-1, 1), (1, -1), (1, 1)] {
                self.set_block(
                    settings,
                    chunk_min_x,
                    chunk_min_z,
                    chunk,
                    world_x + dx,
                    origin_y + dy,
                    world_z + dz,
                    self.sandstone.clone(),
                );
            }
        }

        let water_positions = [(0, 0), (1, 0), (0, 1), (-1, 0), (0, -1)];
        let first = water_positions[random.next_int(water_positions.len() as i32) as usize];
        let second = water_positions[random.next_int(water_positions.len() as i32) as usize];
        self.place_suspicious_sand(
            settings,
            chunk_min_x,
            chunk_min_z,
            chunk,
            world_x + first.0,
            origin_y - 1,
            world_z + first.1,
            random.next_long(),
        );
        self.place_suspicious_sand(
            settings,
            chunk_min_x,
            chunk_min_z,
            chunk,
            world_x + second.0,
            origin_y - 2,
            world_z + second.1,
            random.next_long(),
        );
        true
    }

    #[allow(clippy::too_many_arguments)]
    fn set_block(
        &self,
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
        if chunk.layer(local_x, world_y, local_z, settings.min_y).is_none() {
            return false;
        }
        chunk.set_layer(local_x, world_y, local_z, settings.min_y, block);
        true
    }

    #[allow(clippy::too_many_arguments)]
    fn place_suspicious_sand(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        world_x: i32,
        world_y: i32,
        world_z: i32,
        loot_table_seed: i64,
    ) -> bool {
        let Some((local_x, local_z)) = local_coords(world_x, world_z, chunk_min_x, chunk_min_z)
        else {
            return false;
        };
        if chunk.layer(local_x, world_y, local_z, settings.min_y).is_none() {
            return false;
        }
        chunk.set_layer(
            local_x,
            world_y,
            local_z,
            settings.min_y,
            self.suspicious_sand.clone(),
        );
        chunk.push_block_entity(
            world_x,
            world_y,
            world_z,
            BRUSHABLE_BLOCK_ENTITY_TYPE_ID,
            suspicious_sand_block_entity_nbt(loot_table_seed),
        );
        true
    }
}

#[derive(Debug, Clone)]
struct FossilFeatureConfig {
    overlay_block: BlockLayer,
    bone_block: BlockLayer,
}

impl FossilFeatureConfig {
    fn new(overlay_block: &str) -> Self {
        Self {
            overlay_block: BlockLayer::new(overlay_block),
            bone_block: BlockLayer::new("minecraft:bone_block"),
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
        let shape = FossilShape::sample(random);
        let rotation = random.next_int(4);
        let mut placed = false;
        for &(dx, dy, dz, overlay) in shape.blocks {
            let (rotated_x, rotated_z) = rotate_offset(dx, dz, rotation);
            let block = if overlay {
                self.overlay_block.clone()
            } else {
                self.bone_block.clone()
            };
            placed |= self.set_if_embedded(
                settings,
                chunk_min_x,
                chunk_min_z,
                chunk,
                world_x + rotated_x,
                world_y + dy,
                world_z + rotated_z,
                block,
            );
        }
        placed
    }

    #[allow(clippy::too_many_arguments)]
    fn set_if_embedded(
        &self,
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
        if current.is_air || current.is("minecraft:water") || current.is("minecraft:lava") {
            return false;
        }
        chunk.set_layer(local_x, world_y, local_z, settings.min_y, block);
        true
    }
}

#[derive(Clone, Copy, Debug)]
struct FossilShape {
    blocks: &'static [(i32, i32, i32, bool)],
}

impl FossilShape {
    fn sample(random: &mut FeatureRandom) -> Self {
        let shapes = [
            FossilShape {
                blocks: FOSSIL_SPINE_1,
            },
            FossilShape {
                blocks: FOSSIL_SPINE_2,
            },
            FossilShape {
                blocks: FOSSIL_SKULL_1,
            },
            FossilShape {
                blocks: FOSSIL_SKULL_2,
            },
        ];
        shapes[random.next_int(shapes.len() as i32) as usize]
    }
}

fn rotate_offset(x: i32, z: i32, rotation: i32) -> (i32, i32) {
    match rotation.rem_euclid(4) {
        0 => (x, z),
        1 => (-z, x),
        2 => (-x, -z),
        _ => (z, -x),
    }
}

fn suspicious_sand_block_entity_nbt(loot_table_seed: i64) -> Tag {
    let mut fields = HashMap::new();
    fields.insert(
        "LootTable".to_string(),
        Tag::String(Arc::from("minecraft:archaeology/desert_well")),
    );
    fields.insert("LootTableSeed".to_string(), Tag::Long(loot_table_seed));
    Tag::Compound(Arc::new(fields))
}

const FOSSIL_SPINE_1: &[(i32, i32, i32, bool)] = &[
    (-4, 0, 0, false),
    (-3, 0, 0, false),
    (-2, 0, 0, false),
    (-1, 0, 0, false),
    (0, 0, 0, false),
    (1, 0, 0, false),
    (2, 0, 0, false),
    (3, 0, 0, false),
    (4, 0, 0, false),
    (-3, 0, -1, true),
    (-1, 0, -1, true),
    (1, 0, -1, true),
    (3, 0, -1, true),
    (-3, 0, 1, true),
    (-1, 0, 1, true),
    (1, 0, 1, true),
    (3, 0, 1, true),
    (-3, 1, -2, false),
    (-1, 1, -2, false),
    (1, 1, -2, false),
    (3, 1, -2, false),
    (-3, 1, 2, false),
    (-1, 1, 2, false),
    (1, 1, 2, false),
    (3, 1, 2, false),
];
const FOSSIL_SPINE_2: &[(i32, i32, i32, bool)] = &[
    (-5, 0, 0, false),
    (-4, 0, 0, false),
    (-3, 0, 0, false),
    (-2, 0, 0, false),
    (-1, 0, 0, false),
    (0, 0, 0, false),
    (1, 0, 0, false),
    (2, 0, 0, false),
    (3, 0, 0, false),
    (4, 0, 0, false),
    (5, 0, 0, false),
    (-4, 1, 0, true),
    (-2, 1, 0, true),
    (0, 1, 0, true),
    (2, 1, 0, true),
    (4, 1, 0, true),
    (-4, 0, -2, false),
    (-2, 0, -3, false),
    (0, 0, -3, false),
    (2, 0, -3, false),
    (4, 0, -2, false),
    (-4, 0, 2, false),
    (-2, 0, 3, false),
    (0, 0, 3, false),
    (2, 0, 3, false),
    (4, 0, 2, false),
];
const FOSSIL_SKULL_1: &[(i32, i32, i32, bool)] = &[
    (-2, 0, -2, false),
    (-1, 0, -2, false),
    (0, 0, -2, false),
    (1, 0, -2, false),
    (2, 0, -2, false),
    (-2, 0, -1, false),
    (2, 0, -1, false),
    (-2, 0, 0, false),
    (2, 0, 0, false),
    (-2, 0, 1, false),
    (-1, 0, 1, false),
    (0, 0, 1, false),
    (1, 0, 1, false),
    (2, 0, 1, false),
    (-1, 1, -1, true),
    (1, 1, -1, true),
    (-1, 1, 0, false),
    (0, 1, 0, false),
    (1, 1, 0, false),
];
const FOSSIL_SKULL_2: &[(i32, i32, i32, bool)] = &[
    (-3, 0, -2, false),
    (-2, 0, -2, false),
    (-1, 0, -2, false),
    (0, 0, -2, false),
    (1, 0, -2, false),
    (2, 0, -2, false),
    (3, 0, -2, false),
    (-3, 0, -1, false),
    (3, 0, -1, false),
    (-3, 0, 0, false),
    (3, 0, 0, false),
    (-2, 0, 1, false),
    (-1, 0, 1, false),
    (0, 0, 1, false),
    (1, 0, 1, false),
    (2, 0, 1, false),
    (-2, 1, -1, true),
    (2, 1, -1, true),
    (-1, 1, 0, false),
    (0, 1, 0, false),
    (1, 1, 0, false),
];

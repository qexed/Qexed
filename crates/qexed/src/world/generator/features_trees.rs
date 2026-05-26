#[derive(Debug, Clone)]
struct PlacedTreeFeature {
    step_index: i32,
    feature_index: i32,
    count: WeightedInt,
    surface_water_depth: i32,
    config: TreeFeatureConfig,
    biome_filter: FeatureBiomeFilter,
}

impl PlacedTreeFeature {
    fn trees_plains(feature_index: i32) -> Self {
        Self {
            step_index: 9,
            feature_index,
            count: WeightedInt::new(&[(0, 19), (1, 1)]),
            surface_water_depth: 0,
            config: TreeFeatureConfig::oak_bees_005(),
            biome_filter: FeatureBiomeFilter::All,
        }
    }

    fn trees_birch(feature_index: i32) -> Self {
        Self {
            step_index: 9,
            feature_index,
            count: WeightedInt::new(&[(10, 9), (11, 1)]),
            surface_water_depth: 0,
            config: TreeFeatureConfig::birch_bees_0002(),
            biome_filter: FeatureBiomeFilter::All,
        }
    }

    fn trees_tall_birch(feature_index: i32) -> Self {
        Self {
            step_index: 9,
            feature_index,
            count: WeightedInt::new(&[(10, 9), (11, 1)]),
            surface_water_depth: 0,
            config: TreeFeatureConfig::tall_birch_bees_0002(),
            biome_filter: FeatureBiomeFilter::All,
        }
    }

    fn trees_taiga(feature_index: i32) -> Self {
        Self {
            step_index: 9,
            feature_index,
            count: WeightedInt::new(&[(10, 9), (11, 1)]),
            surface_water_depth: 0,
            config: TreeFeatureConfig::taiga_spruce(),
            biome_filter: FeatureBiomeFilter::All,
        }
    }

    fn trees_snowy(feature_index: i32) -> Self {
        Self {
            step_index: 9,
            feature_index,
            count: WeightedInt::new(&[(0, 9), (1, 1)]),
            surface_water_depth: 0,
            config: TreeFeatureConfig::snowy_spruce(),
            biome_filter: FeatureBiomeFilter::All,
        }
    }

    fn trees_savanna(feature_index: i32) -> Self {
        Self {
            step_index: 9,
            feature_index,
            count: WeightedInt::new(&[(1, 9), (2, 1)]),
            surface_water_depth: 0,
            config: TreeFeatureConfig::savanna_trees(),
            biome_filter: FeatureBiomeFilter::All,
        }
    }

    fn trees_windswept_savanna(feature_index: i32) -> Self {
        Self {
            step_index: 9,
            feature_index,
            count: WeightedInt::new(&[(2, 9), (3, 1)]),
            surface_water_depth: 0,
            config: TreeFeatureConfig::savanna_trees(),
            biome_filter: FeatureBiomeFilter::All,
        }
    }

    fn dark_forest_vegetation(feature_index: i32) -> Self {
        Self {
            step_index: 9,
            feature_index,
            count: WeightedInt::new(&[(16, 1)]),
            surface_water_depth: 0,
            config: TreeFeatureConfig::dark_forest_vegetation(),
            biome_filter: FeatureBiomeFilter::All,
        }
    }

    fn pale_garden_vegetation(feature_index: i32) -> Self {
        Self {
            step_index: 9,
            feature_index,
            count: WeightedInt::new(&[(16, 1)]),
            surface_water_depth: 0,
            config: TreeFeatureConfig::pale_garden_vegetation(),
            biome_filter: FeatureBiomeFilter::All,
        }
    }

    fn trees_flower_forest(feature_index: i32) -> Self {
        Self {
            step_index: 9,
            feature_index,
            count: WeightedInt::new(&[(6, 9), (7, 1)]),
            surface_water_depth: 0,
            config: TreeFeatureConfig::flower_forest_trees(),
            biome_filter: FeatureBiomeFilter::All,
        }
    }

    fn trees_meadow(feature_index: i32) -> Self {
        Self {
            step_index: 9,
            feature_index,
            count: WeightedInt::new(&[(0, 99), (1, 1)]),
            surface_water_depth: 0,
            config: TreeFeatureConfig::meadow_trees(),
            biome_filter: FeatureBiomeFilter::All,
        }
    }

    fn trees_cherry(feature_index: i32) -> Self {
        Self {
            step_index: 9,
            feature_index,
            count: WeightedInt::new(&[(10, 9), (11, 1)]),
            surface_water_depth: 0,
            config: TreeFeatureConfig::cherry_trees(),
            biome_filter: FeatureBiomeFilter::All,
        }
    }

    fn trees_grove(feature_index: i32) -> Self {
        Self {
            step_index: 9,
            feature_index,
            count: WeightedInt::new(&[(10, 9), (11, 1)]),
            surface_water_depth: 0,
            config: TreeFeatureConfig::grove_trees(),
            biome_filter: FeatureBiomeFilter::All,
        }
    }

    fn trees_badlands(feature_index: i32) -> Self {
        Self {
            step_index: 9,
            feature_index,
            count: WeightedInt::new(&[(5, 9), (6, 1)]),
            surface_water_depth: 0,
            config: TreeFeatureConfig::badlands_trees(),
            biome_filter: FeatureBiomeFilter::All,
        }
    }

    fn trees_swamp(feature_index: i32) -> Self {
        Self {
            step_index: 9,
            feature_index,
            count: WeightedInt::new(&[(2, 9), (3, 1)]),
            surface_water_depth: 2,
            config: TreeFeatureConfig::swamp_trees(),
            biome_filter: FeatureBiomeFilter::All,
        }
    }

    fn trees_windswept_hills(feature_index: i32) -> Self {
        Self {
            step_index: 9,
            feature_index,
            count: WeightedInt::new(&[(0, 9), (1, 1)]),
            surface_water_depth: 0,
            config: TreeFeatureConfig::windswept_hills_trees(),
            biome_filter: FeatureBiomeFilter::All,
        }
    }

    fn trees_windswept_forest(feature_index: i32) -> Self {
        Self {
            step_index: 9,
            feature_index,
            count: WeightedInt::new(&[(3, 9), (4, 1)]),
            surface_water_depth: 0,
            config: TreeFeatureConfig::windswept_hills_trees(),
            biome_filter: FeatureBiomeFilter::All,
        }
    }

    fn trees_water(feature_index: i32) -> Self {
        Self {
            step_index: 9,
            feature_index,
            count: WeightedInt::new(&[(0, 9), (1, 1)]),
            surface_water_depth: 0,
            config: TreeFeatureConfig::water_trees(),
            biome_filter: FeatureBiomeFilter::All,
        }
    }

    fn trees_birch_and_oak_leaf_litter(feature_index: i32) -> Self {
        Self {
            step_index: 9,
            feature_index,
            count: WeightedInt::new(&[(10, 9), (11, 1)]),
            surface_water_depth: 0,
            config: TreeFeatureConfig::birch_and_oak_leaf_litter_trees(),
            biome_filter: FeatureBiomeFilter::All,
        }
    }

    fn trees_sparse_jungle(feature_index: i32) -> Self {
        Self {
            step_index: 9,
            feature_index,
            count: WeightedInt::new(&[(2, 9), (3, 1)]),
            surface_water_depth: 0,
            config: TreeFeatureConfig::sparse_jungle_trees(),
            biome_filter: FeatureBiomeFilter::All,
        }
    }

    fn trees_old_growth_spruce_taiga(feature_index: i32) -> Self {
        Self {
            step_index: 9,
            feature_index,
            count: WeightedInt::new(&[(10, 9), (11, 1)]),
            surface_water_depth: 0,
            config: TreeFeatureConfig::old_growth_spruce_taiga_trees(),
            biome_filter: FeatureBiomeFilter::All,
        }
    }

    fn trees_old_growth_pine_taiga(feature_index: i32) -> Self {
        Self {
            step_index: 9,
            feature_index,
            count: WeightedInt::new(&[(10, 9), (11, 1)]),
            surface_water_depth: 0,
            config: TreeFeatureConfig::old_growth_pine_taiga_trees(),
            biome_filter: FeatureBiomeFilter::All,
        }
    }

    fn trees_jungle(feature_index: i32) -> Self {
        Self {
            step_index: 9,
            feature_index,
            count: WeightedInt::new(&[(50, 9), (51, 1)]),
            surface_water_depth: 0,
            config: TreeFeatureConfig::jungle_trees(),
            biome_filter: FeatureBiomeFilter::All,
        }
    }

    fn trees_mangrove(feature_index: i32) -> Self {
        Self {
            step_index: 9,
            feature_index,
            count: WeightedInt::new(&[(25, 1)]),
            surface_water_depth: 5,
            config: TreeFeatureConfig::mangrove_trees(),
            biome_filter: FeatureBiomeFilter::All,
        }
    }

    fn bamboo_vegetation(feature_index: i32) -> Self {
        Self {
            step_index: 9,
            feature_index,
            count: WeightedInt::new(&[(30, 9), (31, 1)]),
            surface_water_depth: 0,
            config: TreeFeatureConfig::bamboo_vegetation(),
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
            let world_x = origin_x + random.next_int(16);
            let world_z = origin_z + random.next_int(16);
            let Some((local_x, local_z)) = local_coords(world_x, world_z, origin_x, origin_z)
            else {
                continue;
            };
            if chunk.world_surface_wg_height(local_x, local_z, settings.min_y)
                - chunk.ocean_floor_wg_height(local_x, local_z, settings.min_y)
                > self.surface_water_depth
            {
                continue;
            }
            let world_y = chunk.ocean_floor_wg_height(local_x, local_z, settings.min_y);
            if world_y <= settings.min_y
                || !sapling_would_survive_at(
                    chunk,
                    origin_x,
                    origin_z,
                    world_x,
                    world_y,
                    world_z,
                    settings.min_y,
                )
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
            if source_chunk.world_surface_wg_height(local_x, local_z, settings.min_y)
                - source_chunk.ocean_floor_wg_height(local_x, local_z, settings.min_y)
                > self.surface_water_depth
            {
                continue;
            }
            let world_y = source_chunk.ocean_floor_wg_height(local_x, local_z, settings.min_y);
            if world_y <= settings.min_y
                || !sapling_would_survive_at(
                    source_chunk,
                    source_origin_x,
                    source_origin_z,
                    world_x,
                    world_y,
                    world_z,
                    settings.min_y,
                )
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
struct TreeFeatureConfig {
    default_tree: OakTreeConfig,
    mushroom_variants: Vec<HugeMushroomTreeVariant>,
    variants: Vec<TreeFeatureVariant>,
}

impl TreeFeatureConfig {
    fn oak_bees_005() -> Self {
        let default_tree = OakTreeConfig::oak_bees_005();
        Self {
            default_tree: default_tree.clone(),
            mushroom_variants: Vec::new(),
            variants: vec![
                TreeFeatureVariant::standing(0.333_333_34, default_tree.clone()),
                TreeFeatureVariant::fallen(0.0125, default_tree),
            ],
        }
    }

    fn birch_bees_0002() -> Self {
        let default_tree = OakTreeConfig::birch_bees_0002();
        Self {
            default_tree: default_tree.clone(),
            mushroom_variants: Vec::new(),
            variants: vec![TreeFeatureVariant::fallen(0.0125, default_tree)],
        }
    }

    fn tall_birch_bees_0002() -> Self {
        let default_tree = OakTreeConfig::birch_bees_0002();
        let super_birch = OakTreeConfig::super_birch_bees_0002();
        Self {
            default_tree: default_tree.clone(),
            mushroom_variants: Vec::new(),
            variants: vec![
                TreeFeatureVariant::fallen(0.00625, super_birch.clone()),
                TreeFeatureVariant::standing(0.5, super_birch),
                TreeFeatureVariant::fallen(0.0125, default_tree),
            ],
        }
    }

    fn taiga_spruce() -> Self {
        let default_tree = OakTreeConfig::spruce();
        Self {
            default_tree: default_tree.clone(),
            mushroom_variants: Vec::new(),
            variants: vec![
                TreeFeatureVariant::standing(0.333_333_34, OakTreeConfig::pine()),
                TreeFeatureVariant::fallen(0.0125, OakTreeConfig::fallen_spruce()),
            ],
        }
    }

    fn snowy_spruce() -> Self {
        let default_tree = OakTreeConfig::spruce();
        Self {
            default_tree,
            mushroom_variants: Vec::new(),
            variants: vec![TreeFeatureVariant::fallen(
                0.0125,
                OakTreeConfig::fallen_spruce(),
            )],
        }
    }

    fn savanna_trees() -> Self {
        let default_tree = OakTreeConfig::oak();
        Self {
            default_tree,
            mushroom_variants: Vec::new(),
            variants: vec![
                TreeFeatureVariant::standing(0.8, OakTreeConfig::acacia()),
                TreeFeatureVariant::fallen(0.0125, OakTreeConfig::oak()),
            ],
        }
    }

    fn dark_forest_vegetation() -> Self {
        let default_tree = OakTreeConfig::oak();
        Self {
            default_tree,
            mushroom_variants: vec![
                HugeMushroomTreeVariant::new(0.025, HugeMushroomFeatureConfig::brown()),
                HugeMushroomTreeVariant::new(0.05, HugeMushroomFeatureConfig::red()),
            ],
            variants: vec![
                TreeFeatureVariant::standing(0.666_666_7, OakTreeConfig::dark_oak()),
                TreeFeatureVariant::fallen(0.0025, OakTreeConfig::birch_bees_0002()),
                TreeFeatureVariant::standing(0.2, OakTreeConfig::birch_bees_0002()),
                TreeFeatureVariant::fallen(0.0125, OakTreeConfig::oak()),
                TreeFeatureVariant::standing(0.1, OakTreeConfig::fancy_oak()),
            ],
        }
    }

    fn pale_garden_vegetation() -> Self {
        let default_tree = OakTreeConfig::pale_oak();
        Self {
            default_tree: default_tree.clone(),
            mushroom_variants: Vec::new(),
            variants: vec![
                TreeFeatureVariant::standing(0.1, default_tree.clone()),
                TreeFeatureVariant::standing(0.9, default_tree),
            ],
        }
    }

    fn flower_forest_trees() -> Self {
        let default_tree = OakTreeConfig::oak_bees_002();
        Self {
            default_tree,
            mushroom_variants: Vec::new(),
            variants: vec![
                TreeFeatureVariant::fallen(0.0025, OakTreeConfig::birch_bees_0002()),
                TreeFeatureVariant::standing(0.2, OakTreeConfig::birch_bees_002()),
                TreeFeatureVariant::standing(0.1, OakTreeConfig::fancy_oak_bees_002()),
            ],
        }
    }

    fn meadow_trees() -> Self {
        Self {
            default_tree: OakTreeConfig::super_birch_bees(),
            mushroom_variants: Vec::new(),
            variants: vec![TreeFeatureVariant::standing(
                0.5,
                OakTreeConfig::fancy_oak_bees(),
            )],
        }
    }

    fn cherry_trees() -> Self {
        Self {
            default_tree: OakTreeConfig::cherry_bees_005(),
            mushroom_variants: Vec::new(),
            variants: Vec::new(),
        }
    }

    fn grove_trees() -> Self {
        Self {
            default_tree: OakTreeConfig::spruce(),
            mushroom_variants: Vec::new(),
            variants: vec![TreeFeatureVariant::standing(
                0.333_333_34,
                OakTreeConfig::pine(),
            )],
        }
    }

    fn badlands_trees() -> Self {
        Self {
            default_tree: OakTreeConfig::oak(),
            mushroom_variants: Vec::new(),
            variants: vec![TreeFeatureVariant::fallen(0.0125, OakTreeConfig::oak())],
        }
    }

    fn swamp_trees() -> Self {
        Self {
            default_tree: OakTreeConfig::swamp_oak(),
            mushroom_variants: Vec::new(),
            variants: Vec::new(),
        }
    }

    fn windswept_hills_trees() -> Self {
        Self {
            default_tree: OakTreeConfig::oak(),
            mushroom_variants: Vec::new(),
            variants: vec![
                TreeFeatureVariant::fallen(0.008325, OakTreeConfig::fallen_spruce()),
                TreeFeatureVariant::standing(0.666, OakTreeConfig::spruce()),
                TreeFeatureVariant::standing(0.1, OakTreeConfig::fancy_oak()),
                TreeFeatureVariant::fallen(0.0125, OakTreeConfig::oak()),
            ],
        }
    }

    fn water_trees() -> Self {
        Self {
            default_tree: OakTreeConfig::oak(),
            mushroom_variants: Vec::new(),
            variants: vec![TreeFeatureVariant::standing(
                0.1,
                OakTreeConfig::fancy_oak(),
            )],
        }
    }

    fn birch_and_oak_leaf_litter_trees() -> Self {
        Self {
            default_tree: OakTreeConfig::oak_bees_0002(),
            mushroom_variants: Vec::new(),
            variants: vec![
                TreeFeatureVariant::fallen(0.0025, OakTreeConfig::birch_bees_0002()),
                TreeFeatureVariant::standing(0.2, OakTreeConfig::birch_bees_0002()),
                TreeFeatureVariant::standing(0.1, OakTreeConfig::fancy_oak_bees_0002()),
                TreeFeatureVariant::fallen(0.0125, OakTreeConfig::oak()),
            ],
        }
    }

    fn sparse_jungle_trees() -> Self {
        Self {
            default_tree: OakTreeConfig::jungle_tree(),
            mushroom_variants: Vec::new(),
            variants: vec![
                TreeFeatureVariant::standing(0.1, OakTreeConfig::fancy_oak()),
                TreeFeatureVariant::standing(0.5, OakTreeConfig::jungle_bush()),
                TreeFeatureVariant::fallen(0.0125, OakTreeConfig::fallen_jungle()),
            ],
        }
    }

    fn old_growth_spruce_taiga_trees() -> Self {
        Self {
            default_tree: OakTreeConfig::spruce(),
            mushroom_variants: Vec::new(),
            variants: vec![
                TreeFeatureVariant::standing(0.333_333_34, OakTreeConfig::mega_spruce()),
                TreeFeatureVariant::standing(0.333_333_34, OakTreeConfig::pine()),
                TreeFeatureVariant::fallen(0.0125, OakTreeConfig::fallen_spruce()),
            ],
        }
    }

    fn old_growth_pine_taiga_trees() -> Self {
        Self {
            default_tree: OakTreeConfig::spruce(),
            mushroom_variants: Vec::new(),
            variants: vec![
                TreeFeatureVariant::standing(0.025_641_026, OakTreeConfig::mega_spruce()),
                TreeFeatureVariant::standing(0.307_692_32, OakTreeConfig::mega_pine()),
                TreeFeatureVariant::standing(0.333_333_34, OakTreeConfig::pine()),
                TreeFeatureVariant::fallen(0.0125, OakTreeConfig::fallen_spruce()),
            ],
        }
    }

    fn jungle_trees() -> Self {
        Self {
            default_tree: OakTreeConfig::jungle_tree(),
            mushroom_variants: Vec::new(),
            variants: vec![
                TreeFeatureVariant::standing(0.1, OakTreeConfig::fancy_oak()),
                TreeFeatureVariant::standing(0.5, OakTreeConfig::jungle_bush()),
                TreeFeatureVariant::standing(0.333_333_34, OakTreeConfig::mega_jungle_tree()),
                TreeFeatureVariant::fallen(0.0125, OakTreeConfig::fallen_jungle()),
            ],
        }
    }

    fn mangrove_trees() -> Self {
        Self {
            default_tree: OakTreeConfig::mangrove(),
            mushroom_variants: Vec::new(),
            variants: vec![TreeFeatureVariant::standing(
                0.85,
                OakTreeConfig::tall_mangrove(),
            )],
        }
    }

    fn bamboo_vegetation() -> Self {
        Self {
            default_tree: OakTreeConfig::jungle_bush(),
            mushroom_variants: Vec::new(),
            variants: vec![
                TreeFeatureVariant::standing(0.05, OakTreeConfig::fancy_oak()),
                TreeFeatureVariant::standing(0.15, OakTreeConfig::jungle_bush()),
                TreeFeatureVariant::standing(0.7, OakTreeConfig::mega_jungle_tree()),
            ],
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
        for variant in &self.mushroom_variants {
            if random.next_float() < variant.chance {
                return variant.place(
                    settings,
                    chunk_min_x,
                    chunk_min_z,
                    chunk,
                    random,
                    world_x,
                    world_y,
                    world_z,
                );
            }
        }
        for variant in &self.variants {
            if random.next_float() < variant.chance {
                return variant.place(
                    settings,
                    chunk_min_x,
                    chunk_min_z,
                    chunk,
                    random,
                    world_x,
                    world_y,
                    world_z,
                );
            }
        }
        self.default_tree.place(
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
        for variant in &self.mushroom_variants {
            if random.next_float() < variant.chance {
                return variant.place_spillover(
                    settings,
                    chunk_min_x,
                    chunk_min_z,
                    chunk,
                    random,
                    world_x,
                    world_y,
                    world_z,
                );
            }
        }
        for variant in &self.variants {
            if random.next_float() < variant.chance {
                return variant.place_spillover(
                    settings,
                    chunk_min_x,
                    chunk_min_z,
                    chunk,
                    random,
                    world_x,
                    world_y,
                    world_z,
                );
            }
        }
        self.default_tree.place_spillover(
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

#[derive(Debug, Clone)]
struct HugeMushroomTreeVariant {
    chance: f32,
    config: HugeMushroomFeatureConfig,
}

impl HugeMushroomTreeVariant {
    fn new(chance: f32, config: HugeMushroomFeatureConfig) -> Self {
        Self { chance, config }
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
        self.config.place(
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
        self.config.place_spillover(
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

#[derive(Debug, Clone)]
struct TreeFeatureVariant {
    chance: f32,
    tree: OakTreeConfig,
    placement: TreePlacementKind,
}

impl TreeFeatureVariant {
    fn standing(chance: f32, tree: OakTreeConfig) -> Self {
        Self {
            chance,
            tree,
            placement: TreePlacementKind::Standing,
        }
    }

    fn fallen(chance: f32, tree: OakTreeConfig) -> Self {
        Self {
            chance,
            tree,
            placement: TreePlacementKind::Fallen,
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
        match self.placement {
            TreePlacementKind::Standing => self.tree.place(
                settings,
                chunk_min_x,
                chunk_min_z,
                chunk,
                random,
                world_x,
                world_y,
                world_z,
            ),
            TreePlacementKind::Fallen => self.tree.place_fallen(
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
        match self.placement {
            TreePlacementKind::Standing => self.tree.place_spillover(
                settings,
                chunk_min_x,
                chunk_min_z,
                chunk,
                random,
                world_x,
                world_y,
                world_z,
            ),
            TreePlacementKind::Fallen => self.tree.place_fallen(
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TreePlacementKind {
    Standing,
    Fallen,
}

#[derive(Debug, Clone, Copy)]
enum TreeFoliageConfig {
    Blob,
    Spruce {
        radius: UniformInt,
        offset: UniformInt,
        trunk_height: UniformInt,
    },
    Pine {
        radius: UniformInt,
        offset: UniformInt,
        height: UniformInt,
    },
    Acacia {
        radius: UniformInt,
        offset: UniformInt,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TreeTrunkConfig {
    Straight,
    Forking,
    Giant,
}

#[derive(Debug, Clone, Copy)]
struct FoliageOrigin {
    x: i32,
    y: i32,
    z: i32,
    radius_offset: i32,
}

#[derive(Debug, Clone)]
struct OakTreeConfig {
    trunk: BlockLayer,
    leaves: BlockLayer,
    dirt: BlockLayer,
    bee_nest: BlockLayer,
    base_height: i32,
    height_rand_a: i32,
    height_rand_b: i32,
    trunk_placer: TreeTrunkConfig,
    foliage_height: i32,
    foliage_radius: i32,
    foliage: TreeFoliageConfig,
    beehive_probability: f32,
    fallen_min_length: i32,
    fallen_max_length: i32,
}

impl OakTreeConfig {
    fn log(block: &str) -> BlockLayer {
        BlockLayer::with_properties(block, &[("axis", "y")])
    }

    fn leaves(block: &str) -> BlockLayer {
        BlockLayer::with_properties(
            block,
            &[
                ("distance", "7"),
                ("persistent", "false"),
                ("waterlogged", "false"),
            ],
        )
    }

    fn with_beehive_probability(mut self, beehive_probability: f32) -> Self {
        self.beehive_probability = beehive_probability;
        self
    }

    fn oak_bees_005() -> Self {
        Self {
            trunk: BlockLayer::with_properties("minecraft:oak_log", &[("axis", "y")]),
            leaves: BlockLayer::with_properties(
                "minecraft:oak_leaves",
                &[
                    ("distance", "7"),
                    ("persistent", "false"),
                    ("waterlogged", "false"),
                ],
            ),
            dirt: BlockLayer::new("minecraft:dirt"),
            bee_nest: BlockLayer::with_properties(
                "minecraft:bee_nest",
                &[("facing", "south"), ("honey_level", "0")],
            ),
            base_height: 4,
            height_rand_a: 2,
            height_rand_b: 0,
            trunk_placer: TreeTrunkConfig::Straight,
            foliage_height: 3,
            foliage_radius: 2,
            foliage: TreeFoliageConfig::Blob,
            beehive_probability: 0.05,
            fallen_min_length: 4,
            fallen_max_length: 7,
        }
    }

    fn oak_bees_0002() -> Self {
        Self::oak_bees_005().with_beehive_probability(0.002)
    }

    fn oak_bees_002() -> Self {
        Self::oak_bees_005().with_beehive_probability(0.02)
    }

    fn oak() -> Self {
        Self {
            beehive_probability: 0.0,
            ..Self::oak_bees_005()
        }
    }

    fn birch_bees_0002() -> Self {
        Self {
            trunk: BlockLayer::with_properties("minecraft:birch_log", &[("axis", "y")]),
            leaves: BlockLayer::with_properties(
                "minecraft:birch_leaves",
                &[
                    ("distance", "7"),
                    ("persistent", "false"),
                    ("waterlogged", "false"),
                ],
            ),
            dirt: BlockLayer::new("minecraft:dirt"),
            bee_nest: BlockLayer::with_properties(
                "minecraft:bee_nest",
                &[("facing", "south"), ("honey_level", "0")],
            ),
            base_height: 5,
            height_rand_a: 2,
            height_rand_b: 0,
            trunk_placer: TreeTrunkConfig::Straight,
            foliage_height: 3,
            foliage_radius: 2,
            foliage: TreeFoliageConfig::Blob,
            beehive_probability: 0.002,
            fallen_min_length: 5,
            fallen_max_length: 8,
        }
    }

    fn birch_bees_002() -> Self {
        Self::birch_bees_0002().with_beehive_probability(0.02)
    }

    fn super_birch_bees_0002() -> Self {
        Self {
            height_rand_b: 6,
            fallen_max_length: 15,
            ..Self::birch_bees_0002()
        }
    }

    fn super_birch_bees() -> Self {
        Self::super_birch_bees_0002().with_beehive_probability(1.0)
    }

    fn spruce() -> Self {
        Self {
            trunk: BlockLayer::with_properties("minecraft:spruce_log", &[("axis", "y")]),
            leaves: BlockLayer::with_properties(
                "minecraft:spruce_leaves",
                &[
                    ("distance", "7"),
                    ("persistent", "false"),
                    ("waterlogged", "false"),
                ],
            ),
            dirt: BlockLayer::new("minecraft:dirt"),
            bee_nest: BlockLayer::with_properties(
                "minecraft:bee_nest",
                &[("facing", "south"), ("honey_level", "0")],
            ),
            base_height: 5,
            height_rand_a: 2,
            height_rand_b: 1,
            trunk_placer: TreeTrunkConfig::Straight,
            foliage_height: 4,
            foliage_radius: 3,
            foliage: TreeFoliageConfig::Spruce {
                radius: UniformInt { min: 2, max: 3 },
                offset: UniformInt { min: 0, max: 2 },
                trunk_height: UniformInt { min: 1, max: 2 },
            },
            beehive_probability: 0.0,
            fallen_min_length: 6,
            fallen_max_length: 10,
        }
    }

    fn acacia() -> Self {
        Self {
            trunk: BlockLayer::with_properties("minecraft:acacia_log", &[("axis", "y")]),
            leaves: BlockLayer::with_properties(
                "minecraft:acacia_leaves",
                &[
                    ("distance", "7"),
                    ("persistent", "false"),
                    ("waterlogged", "false"),
                ],
            ),
            dirt: BlockLayer::new("minecraft:dirt"),
            bee_nest: BlockLayer::with_properties(
                "minecraft:bee_nest",
                &[("facing", "south"), ("honey_level", "0")],
            ),
            base_height: 5,
            height_rand_a: 2,
            height_rand_b: 2,
            trunk_placer: TreeTrunkConfig::Forking,
            foliage_height: 0,
            foliage_radius: 2,
            foliage: TreeFoliageConfig::Acacia {
                radius: UniformInt { min: 2, max: 2 },
                offset: UniformInt { min: 0, max: 0 },
            },
            beehive_probability: 0.0,
            fallen_min_length: 4,
            fallen_max_length: 7,
        }
    }

    fn pine() -> Self {
        Self {
            base_height: 6,
            height_rand_a: 4,
            height_rand_b: 0,
            foliage_height: 4,
            foliage_radius: 1,
            foliage: TreeFoliageConfig::Pine {
                radius: UniformInt { min: 1, max: 1 },
                offset: UniformInt { min: 1, max: 1 },
                height: UniformInt { min: 3, max: 4 },
            },
            ..Self::spruce()
        }
    }

    fn fallen_spruce() -> Self {
        Self {
            fallen_min_length: 6,
            fallen_max_length: 10,
            ..Self::spruce()
        }
    }

    fn fancy_oak() -> Self {
        Self {
            base_height: 3,
            height_rand_a: 11,
            height_rand_b: 0,
            foliage_height: 4,
            foliage_radius: 3,
            fallen_min_length: 4,
            fallen_max_length: 7,
            ..Self::oak()
        }
    }

    fn fancy_oak_bees_0002() -> Self {
        Self::fancy_oak().with_beehive_probability(0.002)
    }

    fn fancy_oak_bees_002() -> Self {
        Self::fancy_oak().with_beehive_probability(0.02)
    }

    fn fancy_oak_bees() -> Self {
        Self::fancy_oak().with_beehive_probability(1.0)
    }

    fn dark_oak() -> Self {
        Self {
            trunk: Self::log("minecraft:dark_oak_log"),
            leaves: Self::leaves("minecraft:dark_oak_leaves"),
            base_height: 6,
            height_rand_a: 2,
            height_rand_b: 1,
            trunk_placer: TreeTrunkConfig::Giant,
            foliage_height: 4,
            foliage_radius: 3,
            fallen_min_length: 4,
            fallen_max_length: 7,
            ..Self::oak()
        }
    }

    fn pale_oak() -> Self {
        Self {
            trunk: Self::log("minecraft:pale_oak_log"),
            leaves: Self::leaves("minecraft:pale_oak_leaves"),
            base_height: 6,
            height_rand_a: 2,
            height_rand_b: 1,
            trunk_placer: TreeTrunkConfig::Giant,
            foliage_height: 4,
            foliage_radius: 3,
            fallen_min_length: 4,
            fallen_max_length: 7,
            ..Self::oak()
        }
    }

    fn cherry_bees_005() -> Self {
        Self {
            trunk: Self::log("minecraft:cherry_log"),
            leaves: Self::leaves("minecraft:cherry_leaves"),
            base_height: 7,
            height_rand_a: 1,
            height_rand_b: 0,
            foliage_height: 5,
            foliage_radius: 4,
            fallen_min_length: 4,
            fallen_max_length: 8,
            beehive_probability: 0.05,
            ..Self::oak()
        }
    }

    fn jungle_tree() -> Self {
        Self {
            trunk: Self::log("minecraft:jungle_log"),
            leaves: Self::leaves("minecraft:jungle_leaves"),
            base_height: 4,
            height_rand_a: 8,
            height_rand_b: 0,
            foliage_height: 3,
            foliage_radius: 2,
            fallen_min_length: 4,
            fallen_max_length: 11,
            ..Self::oak()
        }
    }

    fn jungle_bush() -> Self {
        Self {
            trunk: Self::log("minecraft:jungle_log"),
            leaves: Self::leaves("minecraft:oak_leaves"),
            base_height: 1,
            height_rand_a: 0,
            height_rand_b: 0,
            foliage_height: 2,
            foliage_radius: 2,
            fallen_min_length: 4,
            fallen_max_length: 11,
            ..Self::oak()
        }
    }

    fn mega_jungle_tree() -> Self {
        Self {
            trunk: Self::log("minecraft:jungle_log"),
            leaves: Self::leaves("minecraft:jungle_leaves"),
            base_height: 10,
            height_rand_a: 2,
            height_rand_b: 19,
            trunk_placer: TreeTrunkConfig::Giant,
            foliage_height: 4,
            foliage_radius: 3,
            fallen_min_length: 4,
            fallen_max_length: 11,
            ..Self::oak()
        }
    }

    fn mega_spruce() -> Self {
        Self {
            base_height: 13,
            height_rand_a: 2,
            height_rand_b: 14,
            trunk_placer: TreeTrunkConfig::Giant,
            foliage_height: 8,
            foliage_radius: 3,
            foliage: TreeFoliageConfig::Spruce {
                radius: UniformInt { min: 3, max: 4 },
                offset: UniformInt { min: 0, max: 1 },
                trunk_height: UniformInt { min: 8, max: 12 },
            },
            fallen_min_length: 6,
            fallen_max_length: 10,
            ..Self::spruce()
        }
    }

    fn mega_pine() -> Self {
        Self {
            foliage: TreeFoliageConfig::Spruce {
                radius: UniformInt { min: 2, max: 3 },
                offset: UniformInt { min: 0, max: 1 },
                trunk_height: UniformInt { min: 10, max: 14 },
            },
            ..Self::mega_spruce()
        }
    }

    fn swamp_oak() -> Self {
        Self {
            base_height: 5,
            height_rand_a: 3,
            height_rand_b: 0,
            foliage_radius: 3,
            foliage_height: 3,
            ..Self::oak()
        }
    }

    fn mangrove() -> Self {
        Self {
            trunk: Self::log("minecraft:mangrove_log"),
            leaves: Self::leaves("minecraft:mangrove_leaves"),
            base_height: 4,
            height_rand_a: 2,
            height_rand_b: 2,
            trunk_placer: TreeTrunkConfig::Forking,
            foliage_height: 3,
            foliage_radius: 3,
            beehive_probability: 0.01,
            fallen_min_length: 4,
            fallen_max_length: 9,
            ..Self::oak()
        }
    }

    fn azalea() -> Self {
        Self {
            trunk: Self::log("minecraft:oak_log"),
            leaves: Self::leaves("minecraft:azalea_leaves"),
            dirt: BlockLayer::new("minecraft:rooted_dirt"),
            base_height: 4,
            height_rand_a: 2,
            height_rand_b: 0,
            trunk_placer: TreeTrunkConfig::Forking,
            foliage_height: 2,
            foliage_radius: 3,
            fallen_min_length: 4,
            fallen_max_length: 7,
            ..Self::oak()
        }
    }

    fn tall_mangrove() -> Self {
        Self {
            base_height: 8,
            height_rand_a: 4,
            height_rand_b: 3,
            foliage_height: 4,
            foliage_radius: 3,
            ..Self::mangrove()
        }
    }

    fn fallen_jungle() -> Self {
        Self {
            fallen_min_length: 4,
            fallen_max_length: 11,
            ..Self::jungle_tree()
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
        require_local_logs: bool,
    ) -> bool {
        let mut tree_height = self.base_height + random.next_int(self.height_rand_a + 1);
        if self.height_rand_b > 0 {
            tree_height += random.next_int(self.height_rand_b + 1);
        }
        if !self.has_space(
            settings,
            chunk_min_x,
            chunk_min_z,
            chunk,
            world_x,
            world_y,
            world_z,
            tree_height,
        ) {
            return false;
        }

        if let Some((local_x, local_z)) = local_coords(world_x, world_z, chunk_min_x, chunk_min_z)
            && chunk
                .layer(local_x, world_y - 1, local_z, settings.min_y)
                .is_some_and(|layer| !cannot_replace_below_tree_trunk(layer))
        {
            chunk.set_layer(
                local_x,
                world_y - 1,
                local_z,
                settings.min_y,
                self.dirt.clone(),
            );
        }

        let (logs, foliage_origins) = self.place_trunk(
            settings,
            chunk_min_x,
            chunk_min_z,
            chunk,
            random,
            world_x,
            world_y,
            world_z,
            tree_height,
            !require_local_logs,
        );
        if require_local_logs && logs.is_empty() {
            return false;
        }

        let leaves = self.place_foliage(
            settings,
            chunk_min_x,
            chunk_min_z,
            chunk,
            random,
            tree_height,
            &foliage_origins,
        );

        self.try_place_beehive(
            settings,
            chunk_min_x,
            chunk_min_z,
            chunk,
            random,
            &logs,
            &leaves,
        );

        true
    }

    #[allow(clippy::too_many_arguments)]
    fn place_trunk(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
        world_x: i32,
        world_y: i32,
        world_z: i32,
        tree_height: i32,
        record_all_foliage_origins: bool,
    ) -> (Vec<(i32, i32, i32)>, Vec<FoliageOrigin>) {
        match self.trunk_placer {
            TreeTrunkConfig::Straight => self.place_straight_trunk(
                settings,
                chunk_min_x,
                chunk_min_z,
                chunk,
                world_x,
                world_y,
                world_z,
                tree_height,
            ),
            TreeTrunkConfig::Forking => self.place_forking_trunk(
                settings,
                chunk_min_x,
                chunk_min_z,
                chunk,
                random,
                world_x,
                world_y,
                world_z,
                tree_height,
                record_all_foliage_origins,
            ),
            TreeTrunkConfig::Giant => self.place_giant_trunk(
                settings,
                chunk_min_x,
                chunk_min_z,
                chunk,
                world_x,
                world_y,
                world_z,
                tree_height,
            ),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn place_straight_trunk(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        world_x: i32,
        world_y: i32,
        world_z: i32,
        tree_height: i32,
    ) -> (Vec<(i32, i32, i32)>, Vec<FoliageOrigin>) {
        let mut logs = Vec::new();
        for dy in 0..tree_height {
            if self.try_place_log(
                settings,
                chunk_min_x,
                chunk_min_z,
                chunk,
                world_x,
                world_y + dy,
                world_z,
            ) {
                logs.push((world_x, world_y + dy, world_z));
            }
        }

        (
            logs,
            vec![FoliageOrigin {
                x: world_x,
                y: world_y + tree_height,
                z: world_z,
                radius_offset: 0,
            }],
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn place_forking_trunk(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
        world_x: i32,
        world_y: i32,
        world_z: i32,
        tree_height: i32,
        record_all_foliage_origins: bool,
    ) -> (Vec<(i32, i32, i32)>, Vec<FoliageOrigin>) {
        let mut logs = Vec::new();
        let mut foliage_origins = Vec::new();
        let lean_direction = horizontal_directions()[random.next_int(4) as usize];
        let lean_height = tree_height - random.next_int(4) - 1;
        let mut lean_steps = 3 - random.next_int(3);
        let mut trunk_x = world_x;
        let mut trunk_z = world_z;
        let mut end_y = None;

        for y_offset in 0..tree_height {
            let log_y = world_y + y_offset;
            if y_offset >= lean_height && lean_steps > 0 {
                trunk_x += lean_direction.0;
                trunk_z += lean_direction.1;
                lean_steps -= 1;
            }
            let placed = self.try_place_log(
                settings,
                chunk_min_x,
                chunk_min_z,
                chunk,
                trunk_x,
                log_y,
                trunk_z,
            );
            if placed {
                logs.push((trunk_x, log_y, trunk_z));
            }
            if placed || record_all_foliage_origins {
                end_y = Some(log_y + 1);
            }
        }
        if let Some(end_y) = end_y {
            foliage_origins.push(FoliageOrigin {
                x: trunk_x,
                y: end_y,
                z: trunk_z,
                radius_offset: 1,
            });
        }

        trunk_x = world_x;
        trunk_z = world_z;
        let branch_direction = horizontal_directions()[random.next_int(4) as usize];
        if branch_direction != lean_direction {
            let branch_pos = lean_height - random.next_int(2) - 1;
            let mut branch_steps = 1 + random.next_int(3);
            end_y = None;

            let mut y_offset = branch_pos;
            while y_offset < tree_height && branch_steps > 0 {
                if y_offset >= 1 {
                    let log_y = world_y + y_offset;
                    trunk_x += branch_direction.0;
                    trunk_z += branch_direction.1;
                    let placed = self.try_place_log(
                        settings,
                        chunk_min_x,
                        chunk_min_z,
                        chunk,
                        trunk_x,
                        log_y,
                        trunk_z,
                    );
                    if placed {
                        logs.push((trunk_x, log_y, trunk_z));
                    }
                    if placed || record_all_foliage_origins {
                        end_y = Some(log_y + 1);
                    }
                    branch_steps -= 1;
                }
                y_offset += 1;
            }

            if let Some(end_y) = end_y {
                foliage_origins.push(FoliageOrigin {
                    x: trunk_x,
                    y: end_y,
                    z: trunk_z,
                    radius_offset: 0,
                });
            }
        }

        (logs, foliage_origins)
    }

    #[allow(clippy::too_many_arguments)]
    fn place_giant_trunk(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        world_x: i32,
        world_y: i32,
        world_z: i32,
        tree_height: i32,
    ) -> (Vec<(i32, i32, i32)>, Vec<FoliageOrigin>) {
        let mut logs = Vec::new();
        for dy in 0..tree_height {
            for (dx, dz) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                if self.try_place_log(
                    settings,
                    chunk_min_x,
                    chunk_min_z,
                    chunk,
                    world_x + dx,
                    world_y + dy,
                    world_z + dz,
                ) {
                    logs.push((world_x + dx, world_y + dy, world_z + dz));
                }
            }
        }

        (
            logs,
            vec![FoliageOrigin {
                x: world_x + 1,
                y: world_y + tree_height,
                z: world_z + 1,
                radius_offset: 1,
            }],
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn place_foliage(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
        tree_height: i32,
        foliage_origins: &[FoliageOrigin],
    ) -> Vec<(i32, i32, i32)> {
        match self.foliage {
            TreeFoliageConfig::Blob => self.place_blob_foliage(
                settings,
                chunk_min_x,
                chunk_min_z,
                chunk,
                random,
                tree_height,
                foliage_origins,
            ),
            TreeFoliageConfig::Spruce {
                radius,
                offset,
                trunk_height,
            } => {
                let foliage_height = 4.max(tree_height - trunk_height.sample(random));
                let leaf_radius = radius.sample(random);
                let offset = offset.sample(random);
                self.place_spruce_foliage(
                    settings,
                    chunk_min_x,
                    chunk_min_z,
                    chunk,
                    random,
                    tree_height,
                    foliage_height,
                    leaf_radius,
                    offset,
                    foliage_origins,
                )
            }
            TreeFoliageConfig::Pine {
                radius,
                offset,
                height,
            } => {
                let foliage_height = height.sample(random);
                let trunk_height = tree_height - foliage_height;
                let leaf_radius =
                    radius.sample(random) + random.next_int((trunk_height + 1).max(1));
                let offset = offset.sample(random);
                self.place_pine_foliage(
                    settings,
                    chunk_min_x,
                    chunk_min_z,
                    chunk,
                    tree_height,
                    foliage_height,
                    leaf_radius,
                    offset,
                    foliage_origins,
                )
            }
            TreeFoliageConfig::Acacia { radius, offset } => {
                let leaf_radius = radius.sample(random);
                let offset = offset.sample(random);
                self.place_acacia_foliage(
                    settings,
                    chunk_min_x,
                    chunk_min_z,
                    chunk,
                    tree_height,
                    leaf_radius,
                    offset,
                    foliage_origins,
                )
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn place_blob_foliage(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
        _tree_height: i32,
        foliage_origins: &[FoliageOrigin],
    ) -> Vec<(i32, i32, i32)> {
        let mut leaves = Vec::new();
        for origin in foliage_origins {
            for y_offset in (-self.foliage_height..=0).rev() {
                let radius = (self.foliage_radius + origin.radius_offset - 1 - y_offset / 2).max(0);
                for dx in -radius..=radius {
                    for dz in -radius..=radius {
                        if dx.abs() == radius
                            && dz.abs() == radius
                            && (random.next_int(2) == 0 || y_offset == 0)
                        {
                            continue;
                        }
                        if self.try_place_leaf_row_block(
                            settings,
                            chunk_min_x,
                            chunk_min_z,
                            chunk,
                            origin.x,
                            origin.y,
                            origin.z,
                            dx,
                            y_offset,
                            dz,
                        ) {
                            leaves.push((origin.x + dx, origin.y + y_offset, origin.z + dz));
                        }
                    }
                }
            }
        }
        leaves
    }

    #[allow(clippy::too_many_arguments)]
    fn place_spruce_foliage(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
        _tree_height: i32,
        foliage_height: i32,
        leaf_radius: i32,
        offset: i32,
        foliage_origins: &[FoliageOrigin],
    ) -> Vec<(i32, i32, i32)> {
        let mut leaves = Vec::new();
        for origin in foliage_origins {
            let mut current_radius = random.next_int(2);
            let mut max_radius = 1;
            let mut min_radius = 0;

            for y_offset in (-foliage_height..=offset).rev() {
                self.place_conifer_leaf_row(
                    settings,
                    chunk_min_x,
                    chunk_min_z,
                    chunk,
                    origin.x,
                    origin.y,
                    origin.z,
                    y_offset,
                    current_radius,
                    &mut leaves,
                );
                if current_radius >= max_radius {
                    current_radius = min_radius;
                    min_radius = 1;
                    max_radius = (max_radius + 1).min(leaf_radius + origin.radius_offset);
                } else {
                    current_radius += 1;
                }
            }
        }
        leaves
    }

    #[allow(clippy::too_many_arguments)]
    fn place_pine_foliage(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        _tree_height: i32,
        foliage_height: i32,
        leaf_radius: i32,
        offset: i32,
        foliage_origins: &[FoliageOrigin],
    ) -> Vec<(i32, i32, i32)> {
        let mut leaves = Vec::new();
        for origin in foliage_origins {
            let mut current_radius = 0;

            for y_offset in (offset - foliage_height..=offset).rev() {
                self.place_conifer_leaf_row(
                    settings,
                    chunk_min_x,
                    chunk_min_z,
                    chunk,
                    origin.x,
                    origin.y,
                    origin.z,
                    y_offset,
                    current_radius,
                    &mut leaves,
                );
                if current_radius >= 1 && y_offset == offset - foliage_height + 1 {
                    current_radius -= 1;
                } else if current_radius < leaf_radius + origin.radius_offset {
                    current_radius += 1;
                }
            }
        }
        leaves
    }

    #[allow(clippy::too_many_arguments)]
    fn place_acacia_foliage(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        _tree_height: i32,
        leaf_radius: i32,
        offset: i32,
        foliage_origins: &[FoliageOrigin],
    ) -> Vec<(i32, i32, i32)> {
        let mut leaves = Vec::new();
        for origin in foliage_origins {
            let leaf_origin_y = origin.y + offset;
            self.place_acacia_leaf_row(
                settings,
                chunk_min_x,
                chunk_min_z,
                chunk,
                origin.x,
                leaf_origin_y,
                origin.z,
                -1,
                leaf_radius + origin.radius_offset,
                &mut leaves,
            );
            self.place_acacia_leaf_row(
                settings,
                chunk_min_x,
                chunk_min_z,
                chunk,
                origin.x,
                leaf_origin_y,
                origin.z,
                0,
                leaf_radius + origin.radius_offset - 1,
                &mut leaves,
            );
        }
        leaves
    }

    #[allow(clippy::too_many_arguments)]
    fn place_acacia_leaf_row(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        origin_x: i32,
        origin_y: i32,
        origin_z: i32,
        y_offset: i32,
        radius: i32,
        leaves: &mut Vec<(i32, i32, i32)>,
    ) {
        for dx in -radius..=radius {
            for dz in -radius..=radius {
                if if y_offset == 0 {
                    (dx.abs() > 1 || dz.abs() > 1) && dx != 0 && dz != 0
                } else {
                    dx.abs() == radius && dz.abs() == radius && radius > 0
                } {
                    continue;
                }
                if self.try_place_leaf_row_block(
                    settings,
                    chunk_min_x,
                    chunk_min_z,
                    chunk,
                    origin_x,
                    origin_y,
                    origin_z,
                    dx,
                    y_offset,
                    dz,
                ) {
                    leaves.push((origin_x + dx, origin_y + y_offset, origin_z + dz));
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn place_conifer_leaf_row(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        world_x: i32,
        leaf_origin_y: i32,
        world_z: i32,
        y_offset: i32,
        radius: i32,
        leaves: &mut Vec<(i32, i32, i32)>,
    ) {
        for dx in -radius..=radius {
            for dz in -radius..=radius {
                if dx.abs() == radius && dz.abs() == radius && radius > 0 {
                    continue;
                }
                if self.try_place_leaf_row_block(
                    settings,
                    chunk_min_x,
                    chunk_min_z,
                    chunk,
                    world_x,
                    leaf_origin_y,
                    world_z,
                    dx,
                    y_offset,
                    dz,
                ) {
                    leaves.push((world_x + dx, leaf_origin_y + y_offset, world_z + dz));
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn try_place_leaf_row_block(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        origin_x: i32,
        origin_y: i32,
        origin_z: i32,
        offset_x: i32,
        offset_y: i32,
        offset_z: i32,
    ) -> bool {
        self.try_place_leaf(
            settings,
            chunk_min_x,
            chunk_min_z,
            chunk,
            origin_x + offset_x,
            origin_y + offset_y,
            origin_z + offset_z,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn try_place_log(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        world_x: i32,
        world_y: i32,
        world_z: i32,
    ) -> bool {
        let Some((local_x, local_z)) = local_coords(world_x, world_z, chunk_min_x, chunk_min_z)
        else {
            return false;
        };
        if !chunk
            .layer(local_x, world_y, local_z, settings.min_y)
            .is_some_and(|layer| valid_tree_position_layer(layer) || is_log_layer(layer))
        {
            return false;
        }
        chunk.set_layer(
            local_x,
            world_y,
            local_z,
            settings.min_y,
            self.trunk.clone(),
        );
        true
    }

    #[allow(clippy::too_many_arguments)]
    fn place_fallen(
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
        let direction = horizontal_directions()[random.next_int(4) as usize];
        let length = self.fallen_min_length
            + random.next_int(self.fallen_max_length - self.fallen_min_length + 1);
        let start_x = world_x + direction.0 * (2 + random.next_int(2));
        let start_z = world_z + direction.1 * (2 + random.next_int(2));
        let axis = if direction.0 != 0 { "x" } else { "z" };
        let sideways_trunk = self.trunk.with_property("axis", axis);
        let mut placed_any = false;

        if let Some((local_x, local_z)) = local_coords(world_x, world_z, chunk_min_x, chunk_min_z)
            && chunk
                .layer(local_x, world_y, local_z, settings.min_y)
                .is_some_and(valid_fallen_log_position_layer)
            && is_fallen_tree_support_at_world(
                chunk,
                chunk_min_x,
                chunk_min_z,
                world_x,
                world_y - 1,
                world_z,
                settings.min_y,
            )
        {
            chunk.set_layer(
                local_x,
                world_y,
                local_z,
                settings.min_y,
                self.trunk.clone(),
            );
            placed_any = true;
        }

        let mut current_x = start_x;
        let mut current_z = start_z;
        for _ in 0..length {
            let Some((local_x, local_z)) =
                local_coords(current_x, current_z, chunk_min_x, chunk_min_z)
            else {
                current_x += direction.0;
                current_z += direction.1;
                continue;
            };
            let y = chunk.ocean_floor_wg_height(local_x, local_z, settings.min_y);
            if y > settings.min_y
                && chunk
                    .layer(local_x, y, local_z, settings.min_y)
                    .is_some_and(valid_fallen_log_position_layer)
                && is_fallen_tree_support_at_world(
                    chunk,
                    chunk_min_x,
                    chunk_min_z,
                    current_x,
                    y - 1,
                    current_z,
                    settings.min_y,
                )
            {
                chunk.set_layer(local_x, y, local_z, settings.min_y, sideways_trunk.clone());
                placed_any = true;
            }
            current_x += direction.0;
            current_z += direction.1;
        }
        placed_any
    }

    #[allow(clippy::too_many_arguments)]
    fn has_space(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &NoiseChunkBlocks,
        world_x: i32,
        world_y: i32,
        world_z: i32,
        tree_height: i32,
    ) -> bool {
        if world_y < settings.min_y + 1
            || world_y + tree_height + 1 > settings.min_y + settings.height
        {
            return false;
        }

        for dy in 0..=tree_height + 1 {
            let radius = if dy < 1 { 0 } else { 1 };
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
                    if !valid_tree_position_layer(layer) && !is_log_layer(layer) {
                        return false;
                    }
                }
            }
        }
        true
    }

    #[allow(clippy::too_many_arguments)]
    fn try_place_leaf(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        world_x: i32,
        world_y: i32,
        world_z: i32,
    ) -> bool {
        let Some((local_x, local_z)) = local_coords(world_x, world_z, chunk_min_x, chunk_min_z)
        else {
            return false;
        };
        if !chunk
            .layer(local_x, world_y, local_z, settings.min_y)
            .is_some_and(valid_tree_position_layer)
        {
            return false;
        }
        chunk.set_layer(
            local_x,
            world_y,
            local_z,
            settings.min_y,
            self.leaves.clone(),
        );
        true
    }

    #[allow(clippy::too_many_arguments)]
    fn try_place_beehive(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
        logs: &[(i32, i32, i32)],
        leaves: &[(i32, i32, i32)],
    ) {
        if logs.is_empty() || random.next_float() >= self.beehive_probability {
            return;
        }
        let lowest_log_y = logs.iter().map(|log| log.1).min().unwrap_or(logs[0].1);
        let highest_log_y = logs.iter().map(|log| log.1).max().unwrap_or(logs[0].1);
        let hive_y = leaves
            .iter()
            .map(|leaf| leaf.1)
            .min()
            .map(|leaf_y| (leaf_y - 1).max(lowest_log_y + 1))
            .unwrap_or_else(|| (lowest_log_y + 1 + random.next_int(3)).min(highest_log_y));
        let mut candidates = Vec::new();
        for &(log_x, log_y, log_z) in logs {
            if log_y != hive_y {
                continue;
            }
            candidates.push((log_x, log_y, log_z + 1));
            candidates.push((log_x + 1, log_y, log_z));
            candidates.push((log_x - 1, log_y, log_z));
        }
        shuffle_positions(&mut candidates, random);
        for (hive_x, hive_y, hive_z) in candidates {
            if !is_air_at_world(
                chunk,
                chunk_min_x,
                chunk_min_z,
                hive_x,
                hive_y,
                hive_z,
                settings.min_y,
            ) || !is_air_at_world(
                chunk,
                chunk_min_x,
                chunk_min_z,
                hive_x,
                hive_y,
                hive_z + 1,
                settings.min_y,
            ) {
                continue;
            }
            let Some((local_x, local_z)) = local_coords(hive_x, hive_z, chunk_min_x, chunk_min_z)
            else {
                continue;
            };
            chunk.set_layer(
                local_x,
                hive_y,
                local_z,
                settings.min_y,
                self.bee_nest.clone(),
            );
            chunk.push_block_entity(
                hive_x,
                hive_y,
                hive_z,
                BEEHIVE_BLOCK_ENTITY_TYPE_ID,
                beehive_block_entity_nbt(random),
            );
            return;
        }
    }
}

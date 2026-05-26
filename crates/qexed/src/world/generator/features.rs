#[derive(Debug, Clone)]
struct OverworldOreFeatures {
    seed: i64,
    features: Vec<PlacedOreFeature>,
    underwater_magma: PlacedUnderwaterMagmaFeature,
    disks: Vec<PlacedDiskFeature>,
    springs: Vec<PlacedSpringFeature>,
    lakes: Vec<PlacedLakeFeature>,
    geodes: Vec<PlacedGeodeFeature>,
    dripstone_features: Vec<PlacedDripstoneFeature>,
    sculk_features: Vec<PlacedSculkFeature>,
    structure_features: Vec<PlacedStructureFeature>,
    surface_features: Vec<PlacedSurfaceFeature>,
    monster_rooms: Vec<PlacedMonsterRoomFeature>,
    glow_lichen: PlacedMultifaceGrowthFeature,
    cave_vines: PlacedCaveVinesFeature,
    classic_vines: PlacedClassicVinesFeature,
    spore_blossom: PlacedSporeBlossomFeature,
    environment_scan_features: Vec<PlacedEnvironmentScanFeature>,
    aquatic_features: Vec<PlacedAquaticFeature>,
    huge_mushrooms: Vec<PlacedHugeMushroomFeature>,
    vegetation_patches: Vec<PlacedSimpleVegetationFeature>,
    surface_vines: PlacedClassicVinesFeature,
    block_columns: Vec<PlacedBlockColumnFeature>,
    trees: Vec<PlacedTreeFeature>,
    freeze_top_layer: PlacedFreezeTopLayerFeature,
}

impl OverworldOreFeatures {
    fn new(seed: i64) -> Self {
        let dirt = OreFeatureConfig::base_stone(33, "minecraft:dirt");
        let gravel = OreFeatureConfig::base_stone(33, "minecraft:gravel");
        let clay_ore = OreFeatureConfig::base_stone(33, "minecraft:clay");
        let granite = OreFeatureConfig::base_stone(64, "minecraft:granite");
        let diorite = OreFeatureConfig::base_stone(64, "minecraft:diorite");
        let andesite = OreFeatureConfig::base_stone(64, "minecraft:andesite");
        let tuff = OreFeatureConfig::base_stone(64, "minecraft:tuff");
        let coal = OreFeatureConfig::new(
            17,
            0.0,
            "minecraft:coal_ore",
            "minecraft:deepslate_coal_ore",
        );
        let coal_buried = OreFeatureConfig::new(
            17,
            0.5,
            "minecraft:coal_ore",
            "minecraft:deepslate_coal_ore",
        );
        let iron =
            OreFeatureConfig::new(9, 0.0, "minecraft:iron_ore", "minecraft:deepslate_iron_ore");
        let iron_small =
            OreFeatureConfig::new(4, 0.0, "minecraft:iron_ore", "minecraft:deepslate_iron_ore");
        let gold =
            OreFeatureConfig::new(9, 0.5, "minecraft:gold_ore", "minecraft:deepslate_gold_ore");
        let redstone = OreFeatureConfig::new(
            8,
            0.0,
            "minecraft:redstone_ore",
            "minecraft:deepslate_redstone_ore",
        );
        let diamond_small = OreFeatureConfig::new(
            4,
            0.5,
            "minecraft:diamond_ore",
            "minecraft:deepslate_diamond_ore",
        );
        let diamond_medium = OreFeatureConfig::new(
            8,
            0.5,
            "minecraft:diamond_ore",
            "minecraft:deepslate_diamond_ore",
        );
        let diamond_large = OreFeatureConfig::new(
            12,
            0.7,
            "minecraft:diamond_ore",
            "minecraft:deepslate_diamond_ore",
        );
        let diamond_buried = OreFeatureConfig::new(
            8,
            1.0,
            "minecraft:diamond_ore",
            "minecraft:deepslate_diamond_ore",
        );
        let lapis = OreFeatureConfig::new(
            7,
            0.0,
            "minecraft:lapis_ore",
            "minecraft:deepslate_lapis_ore",
        );
        let lapis_buried = OreFeatureConfig::new(
            7,
            1.0,
            "minecraft:lapis_ore",
            "minecraft:deepslate_lapis_ore",
        );
        let copper = OreFeatureConfig::new(
            10,
            0.0,
            "minecraft:copper_ore",
            "minecraft:deepslate_copper_ore",
        );
        let copper_large = OreFeatureConfig::new(
            20,
            0.0,
            "minecraft:copper_ore",
            "minecraft:deepslate_copper_ore",
        );
        let emerald = OreFeatureConfig::new(
            3,
            0.0,
            "minecraft:emerald_ore",
            "minecraft:deepslate_emerald_ore",
        );
        let infested = OreFeatureConfig::new(
            9,
            0.0,
            "minecraft:infested_stone",
            "minecraft:infested_deepslate",
        );

        Self {
            seed,
            features: vec![
                PlacedOreFeature::new(
                    0,
                    OrePlacementCount::Constant(7),
                    OreHeight::Uniform(HeightAnchor::Absolute(0), HeightAnchor::Absolute(160)),
                    dirt,
                ),
                PlacedOreFeature::new(
                    1,
                    OrePlacementCount::Constant(14),
                    OreHeight::Uniform(HeightAnchor::AboveBottom(0), HeightAnchor::BelowTop(0)),
                    gravel,
                ),
                PlacedOreFeature::new(
                    2,
                    OrePlacementCount::Constant(46),
                    OreHeight::Uniform(HeightAnchor::AboveBottom(0), HeightAnchor::Absolute(256)),
                    clay_ore,
                )
                .with_biome_filter(FeatureBiomeFilter::Include(LUSH_CAVES_ORE_BIOMES)),
                PlacedOreFeature::new(
                    2,
                    OrePlacementCount::Rarity(6),
                    OreHeight::Uniform(HeightAnchor::Absolute(64), HeightAnchor::Absolute(128)),
                    granite.clone(),
                ),
                PlacedOreFeature::new(
                    3,
                    OrePlacementCount::Constant(2),
                    OreHeight::Uniform(HeightAnchor::Absolute(0), HeightAnchor::Absolute(60)),
                    granite,
                ),
                PlacedOreFeature::new(
                    4,
                    OrePlacementCount::Rarity(6),
                    OreHeight::Uniform(HeightAnchor::Absolute(64), HeightAnchor::Absolute(128)),
                    diorite.clone(),
                ),
                PlacedOreFeature::new(
                    5,
                    OrePlacementCount::Constant(2),
                    OreHeight::Uniform(HeightAnchor::Absolute(0), HeightAnchor::Absolute(60)),
                    diorite,
                ),
                PlacedOreFeature::new(
                    6,
                    OrePlacementCount::Rarity(6),
                    OreHeight::Uniform(HeightAnchor::Absolute(64), HeightAnchor::Absolute(128)),
                    andesite.clone(),
                ),
                PlacedOreFeature::new(
                    7,
                    OrePlacementCount::Constant(2),
                    OreHeight::Uniform(HeightAnchor::Absolute(0), HeightAnchor::Absolute(60)),
                    andesite,
                ),
                PlacedOreFeature::new(
                    8,
                    OrePlacementCount::Constant(2),
                    OreHeight::Uniform(HeightAnchor::AboveBottom(0), HeightAnchor::Absolute(0)),
                    tuff,
                ),
                PlacedOreFeature::new(
                    9,
                    OrePlacementCount::Constant(30),
                    OreHeight::Uniform(HeightAnchor::Absolute(136), HeightAnchor::BelowTop(0)),
                    coal,
                ),
                PlacedOreFeature::new(
                    10,
                    OrePlacementCount::Constant(20),
                    OreHeight::Trapezoid(HeightAnchor::Absolute(0), HeightAnchor::Absolute(192)),
                    coal_buried,
                ),
                PlacedOreFeature::new(
                    11,
                    OrePlacementCount::Constant(90),
                    OreHeight::Trapezoid(HeightAnchor::Absolute(80), HeightAnchor::Absolute(384)),
                    iron.clone(),
                ),
                PlacedOreFeature::new(
                    12,
                    OrePlacementCount::Constant(10),
                    OreHeight::Trapezoid(HeightAnchor::Absolute(-24), HeightAnchor::Absolute(56)),
                    iron,
                ),
                PlacedOreFeature::new(
                    13,
                    OrePlacementCount::Constant(10),
                    OreHeight::Uniform(HeightAnchor::AboveBottom(0), HeightAnchor::Absolute(72)),
                    iron_small,
                ),
                PlacedOreFeature::new(
                    14,
                    OrePlacementCount::Constant(4),
                    OreHeight::Trapezoid(HeightAnchor::Absolute(-64), HeightAnchor::Absolute(32)),
                    gold.clone(),
                ),
                PlacedOreFeature::new(
                    15,
                    OrePlacementCount::Uniform { min: 0, max: 1 },
                    OreHeight::Uniform(HeightAnchor::Absolute(-64), HeightAnchor::Absolute(-48)),
                    gold.clone(),
                ),
                PlacedOreFeature::new(
                    16,
                    OrePlacementCount::Constant(4),
                    OreHeight::Uniform(HeightAnchor::AboveBottom(0), HeightAnchor::Absolute(15)),
                    redstone.clone(),
                ),
                PlacedOreFeature::new(
                    17,
                    OrePlacementCount::Constant(8),
                    OreHeight::Trapezoid(
                        HeightAnchor::AboveBottom(-32),
                        HeightAnchor::AboveBottom(32),
                    ),
                    redstone,
                ),
                PlacedOreFeature::new(
                    18,
                    OrePlacementCount::Constant(7),
                    OreHeight::Trapezoid(
                        HeightAnchor::AboveBottom(-80),
                        HeightAnchor::AboveBottom(80),
                    ),
                    diamond_small,
                ),
                PlacedOreFeature::new(
                    19,
                    OrePlacementCount::Constant(2),
                    OreHeight::Uniform(HeightAnchor::Absolute(-64), HeightAnchor::Absolute(-4)),
                    diamond_medium,
                ),
                PlacedOreFeature::new(
                    20,
                    OrePlacementCount::Rarity(9),
                    OreHeight::Trapezoid(
                        HeightAnchor::AboveBottom(-80),
                        HeightAnchor::AboveBottom(80),
                    ),
                    diamond_large,
                ),
                PlacedOreFeature::new(
                    21,
                    OrePlacementCount::Constant(4),
                    OreHeight::Trapezoid(
                        HeightAnchor::AboveBottom(-80),
                        HeightAnchor::AboveBottom(80),
                    ),
                    diamond_buried,
                ),
                PlacedOreFeature::new(
                    22,
                    OrePlacementCount::Constant(2),
                    OreHeight::Trapezoid(HeightAnchor::Absolute(-32), HeightAnchor::Absolute(32)),
                    lapis,
                ),
                PlacedOreFeature::new(
                    23,
                    OrePlacementCount::Constant(4),
                    OreHeight::Uniform(HeightAnchor::AboveBottom(0), HeightAnchor::Absolute(64)),
                    lapis_buried,
                ),
                PlacedOreFeature::new(
                    24,
                    OrePlacementCount::Constant(16),
                    OreHeight::Trapezoid(HeightAnchor::Absolute(-16), HeightAnchor::Absolute(112)),
                    copper,
                )
                .with_biome_filter(FeatureBiomeFilter::Exclude(DRIPSTONE_CAVES_ORE_BIOMES)),
                PlacedOreFeature::new(
                    24,
                    OrePlacementCount::Constant(16),
                    OreHeight::Trapezoid(HeightAnchor::Absolute(-16), HeightAnchor::Absolute(112)),
                    copper_large,
                )
                .with_biome_filter(FeatureBiomeFilter::Include(DRIPSTONE_CAVES_ORE_BIOMES)),
                PlacedOreFeature::new(
                    26,
                    OrePlacementCount::Constant(50),
                    OreHeight::Uniform(HeightAnchor::Absolute(32), HeightAnchor::Absolute(256)),
                    gold,
                )
                .with_biome_filter(FeatureBiomeFilter::Include(BADLANDS_ORE_BIOMES)),
                PlacedOreFeature::new(
                    29,
                    OrePlacementCount::Constant(100),
                    OreHeight::Trapezoid(HeightAnchor::Absolute(-16), HeightAnchor::Absolute(480)),
                    emerald,
                )
                .with_biome_filter(FeatureBiomeFilter::Include(EMERALD_ORE_BIOMES)),
                PlacedOreFeature::new(
                    0,
                    OrePlacementCount::Constant(14),
                    OreHeight::Uniform(HeightAnchor::AboveBottom(0), HeightAnchor::Absolute(63)),
                    infested,
                )
                .with_step_index(7)
                .with_biome_filter(FeatureBiomeFilter::Include(EMERALD_ORE_BIOMES)),
            ],
            underwater_magma: PlacedUnderwaterMagmaFeature::new(25),
            disks: vec![
                PlacedDiskFeature::sand(26)
                    .with_biome_filter(FeatureBiomeFilter::Exclude(BADLANDS_ORE_BIOMES)),
                PlacedDiskFeature::sand(27)
                    .with_biome_filter(FeatureBiomeFilter::Include(BADLANDS_ORE_BIOMES)),
                PlacedDiskFeature::clay(27)
                    .with_biome_filter(FeatureBiomeFilter::Exclude(BADLANDS_ORE_BIOMES)),
                PlacedDiskFeature::clay(28)
                    .with_biome_filter(FeatureBiomeFilter::Include(BADLANDS_ORE_BIOMES)),
                PlacedDiskFeature::gravel(28)
                    .with_biome_filter(FeatureBiomeFilter::Exclude(BADLANDS_ORE_BIOMES)),
                PlacedDiskFeature::gravel(29)
                    .with_biome_filter(FeatureBiomeFilter::Include(BADLANDS_ORE_BIOMES)),
                PlacedDiskFeature::grass(30)
                    .with_surface_anchor("minecraft:mud", -1)
                    .with_biome_filter(FeatureBiomeFilter::Include(MANGROVE_TREE_BIOMES)),
            ],
            springs: vec![
                PlacedSpringFeature::water(0),
                PlacedSpringFeature::lava_overworld(1),
                PlacedSpringFeature::lava_frozen(2),
            ],
            lakes: vec![
                PlacedLakeFeature::lava_underground(0),
                PlacedLakeFeature::lava_surface(1),
            ],
            geodes: vec![PlacedGeodeFeature::amethyst(0)],
            dripstone_features: vec![
                PlacedDripstoneFeature::large(1),
                PlacedDripstoneFeature::cluster(0),
                PlacedDripstoneFeature::pointed(1),
            ],
            sculk_features: vec![
                PlacedSculkFeature::vein(0),
                PlacedSculkFeature::deep_dark_patch(1),
            ],
            structure_features: vec![
                PlacedStructureFeature::fossil_upper(2),
                PlacedStructureFeature::fossil_lower(3),
                PlacedStructureFeature::desert_well(0),
            ],
            surface_features: vec![
                PlacedSurfaceFeature::forest_rock(1),
                PlacedSurfaceFeature::iceberg_packed(2),
                PlacedSurfaceFeature::iceberg_blue(3),
                PlacedSurfaceFeature::ice_spike(0),
                PlacedSurfaceFeature::ice_patch(1),
                PlacedSurfaceFeature::blue_ice(4),
                PlacedSurfaceFeature::pale_moss_patch(90),
            ],
            monster_rooms: vec![
                PlacedMonsterRoomFeature::regular(0),
                PlacedMonsterRoomFeature::deep(1),
            ],
            glow_lichen: PlacedMultifaceGrowthFeature::glow_lichen(0),
            cave_vines: PlacedCaveVinesFeature::new(77),
            classic_vines: PlacedClassicVinesFeature::cave(83),
            spore_blossom: PlacedSporeBlossomFeature::new(78),
            environment_scan_features: vec![
                PlacedEnvironmentScanFeature::lush_caves_ceiling_vegetation(79),
                PlacedEnvironmentScanFeature::lush_caves_clay(80),
                PlacedEnvironmentScanFeature::lush_caves_vegetation(81),
                PlacedEnvironmentScanFeature::rooted_azalea_tree(82),
            ],
            aquatic_features: vec![
                PlacedAquaticFeature::seagrass(66, 48, 0.3)
                    .with_biome_filter(FeatureBiomeFilter::Include(SEAGRASS_NORMAL_BIOMES)),
                PlacedAquaticFeature::seagrass(67, 32, 0.3)
                    .with_biome_filter(FeatureBiomeFilter::Include(SEAGRASS_COLD_BIOMES)),
                PlacedAquaticFeature::seagrass(68, 48, 0.8)
                    .with_biome_filter(FeatureBiomeFilter::Include(SEAGRASS_DEEP_BIOMES)),
                PlacedAquaticFeature::seagrass(69, 48, 0.8)
                    .with_biome_filter(FeatureBiomeFilter::Include(SEAGRASS_DEEP_COLD_BIOMES)),
                PlacedAquaticFeature::seagrass(70, 48, 0.8)
                    .with_biome_filter(FeatureBiomeFilter::Include(SEAGRASS_DEEP_WARM_BIOMES)),
                PlacedAquaticFeature::seagrass(71, 80, 0.3)
                    .with_biome_filter(FeatureBiomeFilter::Include(SEAGRASS_WARM_BIOMES)),
                PlacedAquaticFeature::seagrass(72, 64, 0.6)
                    .with_biome_filter(FeatureBiomeFilter::Include(SEAGRASS_SWAMP_BIOMES)),
                PlacedAquaticFeature::seagrass(73, 48, 0.4)
                    .with_biome_filter(FeatureBiomeFilter::Include(SEAGRASS_RIVER_BIOMES)),
                PlacedAquaticFeature::kelp(74, 120)
                    .with_biome_filter(FeatureBiomeFilter::Include(KELP_COLD_BIOMES)),
                PlacedAquaticFeature::kelp(75, 80)
                    .with_biome_filter(FeatureBiomeFilter::Include(KELP_WARM_BIOMES)),
                PlacedAquaticFeature::sea_pickle(76)
                    .with_biome_filter(FeatureBiomeFilter::Include(SEA_PICKLE_BIOMES)),
                PlacedAquaticFeature::warm_ocean_vegetation(102)
                    .with_biome_filter(FeatureBiomeFilter::Include(WARM_OCEAN_VEGETATION_BIOMES)),
            ],
            huge_mushrooms: vec![PlacedHugeMushroomFeature::mushroom_island_vegetation(101)],
            vegetation_patches: vec![
                PlacedSimpleVegetationFeature::patch_tall_grass_2(1)
                    .with_biome_filter(FeatureBiomeFilter::Include(PATCH_TALL_GRASS_2_BIOMES)),
                PlacedSimpleVegetationFeature::patch_tall_grass(0)
                    .with_biome_filter(FeatureBiomeFilter::Include(PATCH_TALL_GRASS_BIOMES)),
                PlacedSimpleVegetationFeature::patch_bush(2)
                    .with_biome_filter(FeatureBiomeFilter::Include(PATCH_BUSH_BIOMES)),
                PlacedSimpleVegetationFeature::patch_sunflower(3)
                    .with_biome_filter(FeatureBiomeFilter::Include(SUNFLOWER_PATCH_BIOMES)),
                PlacedSimpleVegetationFeature::flower_plains(4)
                    .with_biome_filter(FeatureBiomeFilter::Include(FLOWER_PLAINS_BIOMES)),
                PlacedSimpleVegetationFeature::patch_grass_plain(5)
                    .with_biome_filter(FeatureBiomeFilter::Include(PATCH_GRASS_PLAIN_BIOMES)),
                PlacedSimpleVegetationFeature::brown_mushroom_normal(6)
                    .with_biome_filter(FeatureBiomeFilter::Include(NORMAL_MUSHROOM_BIOMES)),
                PlacedSimpleVegetationFeature::red_mushroom_normal(7)
                    .with_biome_filter(FeatureBiomeFilter::Include(NORMAL_MUSHROOM_BIOMES)),
                PlacedSimpleVegetationFeature::patch_pumpkin(8)
                    .with_biome_filter(FeatureBiomeFilter::Include(PUMPKIN_PATCH_BIOMES)),
                PlacedSimpleVegetationFeature::patch_dead_bush(9, 1)
                    .with_biome_filter(FeatureBiomeFilter::Include(DEAD_BUSH_NORMAL_BIOMES)),
                PlacedSimpleVegetationFeature::patch_dead_bush(10, 2)
                    .with_biome_filter(FeatureBiomeFilter::Include(DEAD_BUSH_DESERT_BIOMES)),
                PlacedSimpleVegetationFeature::patch_dead_bush(11, 20)
                    .with_biome_filter(FeatureBiomeFilter::Include(DEAD_BUSH_BADLANDS_BIOMES)),
                PlacedSimpleVegetationFeature::patch_melon(12, 6)
                    .with_biome_filter(FeatureBiomeFilter::Include(MELON_PATCH_BIOMES)),
                PlacedSimpleVegetationFeature::patch_melon(13, 64)
                    .with_biome_filter(FeatureBiomeFilter::Include(MELON_SPARSE_PATCH_BIOMES)),
                PlacedSimpleVegetationFeature::patch_grass_normal(20)
                    .with_biome_filter(FeatureBiomeFilter::Include(PATCH_GRASS_NORMAL_BIOMES)),
                PlacedSimpleVegetationFeature::patch_grass_forest(21)
                    .with_biome_filter(FeatureBiomeFilter::Include(PATCH_GRASS_FOREST_BIOMES)),
                PlacedSimpleVegetationFeature::patch_grass_badlands(22)
                    .with_biome_filter(FeatureBiomeFilter::Include(PATCH_GRASS_BADLANDS_BIOMES)),
                PlacedSimpleVegetationFeature::patch_grass_savanna(23)
                    .with_biome_filter(FeatureBiomeFilter::Include(PATCH_GRASS_SAVANNA_BIOMES)),
                PlacedSimpleVegetationFeature::patch_grass_taiga(24)
                    .with_biome_filter(FeatureBiomeFilter::Include(PATCH_GRASS_TAIGA_BIOMES)),
                PlacedSimpleVegetationFeature::patch_grass_taiga_2(25)
                    .with_biome_filter(FeatureBiomeFilter::Include(PATCH_GRASS_TAIGA_2_BIOMES)),
                PlacedSimpleVegetationFeature::patch_grass_jungle(26)
                    .with_biome_filter(FeatureBiomeFilter::Include(PATCH_GRASS_JUNGLE_BIOMES)),
                PlacedSimpleVegetationFeature::patch_grass_meadow(27)
                    .with_biome_filter(FeatureBiomeFilter::Include(PATCH_GRASS_MEADOW_BIOMES)),
                PlacedSimpleVegetationFeature::patch_large_fern(28)
                    .with_biome_filter(FeatureBiomeFilter::Include(PATCH_LARGE_FERN_BIOMES)),
                PlacedSimpleVegetationFeature::patch_dry_grass(29, 3)
                    .with_biome_filter(FeatureBiomeFilter::Include(DRY_GRASS_DESERT_BIOMES)),
                PlacedSimpleVegetationFeature::patch_dry_grass(30, 6)
                    .with_biome_filter(FeatureBiomeFilter::Include(DRY_GRASS_BADLANDS_BIOMES)),
                PlacedSimpleVegetationFeature::brown_mushroom_taiga(31)
                    .with_biome_filter(FeatureBiomeFilter::Include(TAIGA_MUSHROOM_BIOMES)),
                PlacedSimpleVegetationFeature::red_mushroom_taiga(32)
                    .with_biome_filter(FeatureBiomeFilter::Include(TAIGA_MUSHROOM_BIOMES)),
                PlacedSimpleVegetationFeature::brown_mushroom_old_growth(33)
                    .with_biome_filter(FeatureBiomeFilter::Include(OLD_GROWTH_MUSHROOM_BIOMES)),
                PlacedSimpleVegetationFeature::red_mushroom_old_growth(34)
                    .with_biome_filter(FeatureBiomeFilter::Include(OLD_GROWTH_MUSHROOM_BIOMES)),
                PlacedSimpleVegetationFeature::brown_mushroom_swamp(35)
                    .with_biome_filter(FeatureBiomeFilter::Include(SWAMP_MUSHROOM_BIOMES)),
                PlacedSimpleVegetationFeature::red_mushroom_swamp(36)
                    .with_biome_filter(FeatureBiomeFilter::Include(SWAMP_MUSHROOM_BIOMES)),
                PlacedSimpleVegetationFeature::flower_default(37)
                    .with_biome_filter(FeatureBiomeFilter::Include(FLOWER_DEFAULT_BIOMES)),
                PlacedSimpleVegetationFeature::flower_warm(38)
                    .with_biome_filter(FeatureBiomeFilter::Include(FLOWER_WARM_BIOMES)),
                PlacedSimpleVegetationFeature::flower_swamp(39)
                    .with_biome_filter(FeatureBiomeFilter::Include(FLOWER_SWAMP_BIOMES)),
                PlacedSimpleVegetationFeature::flower_cherry(40)
                    .with_biome_filter(FeatureBiomeFilter::Include(FLOWER_CHERRY_BIOMES)),
                PlacedSimpleVegetationFeature::flower_pale_garden(41)
                    .with_biome_filter(FeatureBiomeFilter::Include(FLOWER_PALE_GARDEN_BIOMES)),
                PlacedSimpleVegetationFeature::flower_meadow(91)
                    .with_biome_filter(FeatureBiomeFilter::Include(FLOWER_MEADOW_BIOMES)),
                PlacedSimpleVegetationFeature::flower_flower_forest(92)
                    .with_biome_filter(FeatureBiomeFilter::Include(FLOWER_FLOWER_FOREST_BIOMES)),
                PlacedSimpleVegetationFeature::forest_flowers(93, -3, 1)
                    .with_biome_filter(FeatureBiomeFilter::Include(FOREST_FLOWERS_BIOMES)),
                PlacedSimpleVegetationFeature::forest_flowers(94, -1, 3)
                    .with_biome_filter(FeatureBiomeFilter::Include(FLOWER_FOREST_FLOWERS_BIOMES)),
                PlacedSimpleVegetationFeature::patch_leaf_litter(95)
                    .with_biome_filter(FeatureBiomeFilter::Include(PATCH_LEAF_LITTER_BIOMES)),
                PlacedSimpleVegetationFeature::wildflowers_meadow(96)
                    .with_biome_filter(FeatureBiomeFilter::Include(WILDFLOWERS_MEADOW_BIOMES)),
                PlacedSimpleVegetationFeature::wildflowers_birch_forest(97).with_biome_filter(
                    FeatureBiomeFilter::Include(WILDFLOWERS_BIRCH_FOREST_BIOMES),
                ),
                PlacedSimpleVegetationFeature::pale_garden_flowers(98)
                    .with_biome_filter(FeatureBiomeFilter::Include(PALE_GARDEN_FLOWERS_BIOMES)),
                PlacedSimpleVegetationFeature::patch_berry_common(85)
                    .with_biome_filter(FeatureBiomeFilter::Include(BERRY_COMMON_BIOMES)),
                PlacedSimpleVegetationFeature::patch_berry_rare(86)
                    .with_biome_filter(FeatureBiomeFilter::Include(BERRY_RARE_BIOMES)),
                PlacedSimpleVegetationFeature::patch_firefly_bush_swamp(87)
                    .with_biome_filter(FeatureBiomeFilter::Include(FIREFLY_BUSH_SWAMP_BIOMES)),
                PlacedSimpleVegetationFeature::patch_firefly_bush_near_water(88, 2)
                    .with_biome_filter(FeatureBiomeFilter::Include(FIREFLY_BUSH_NEAR_WATER_BIOMES)),
                PlacedSimpleVegetationFeature::patch_firefly_bush_near_water(89, 3)
                    .with_biome_filter(FeatureBiomeFilter::Include(FIREFLY_BUSH_SWAMP_BIOMES)),
                PlacedSimpleVegetationFeature::patch_waterlily(84)
                    .with_biome_filter(FeatureBiomeFilter::Include(WATERLILY_BIOMES)),
            ],
            surface_vines: PlacedClassicVinesFeature::surface(85),
            block_columns: vec![
                PlacedBlockColumnFeature::sugar_cane(14, 6)
                    .with_biome_filter(FeatureBiomeFilter::Include(SUGAR_CANE_NORMAL_BIOMES)),
                PlacedBlockColumnFeature::sugar_cane(15, 5)
                    .with_biome_filter(FeatureBiomeFilter::Include(SUGAR_CANE_BADLANDS_BIOMES)),
                PlacedBlockColumnFeature::sugar_cane(16, 1)
                    .with_biome_filter(FeatureBiomeFilter::Include(SUGAR_CANE_DESERT_BIOMES)),
                PlacedBlockColumnFeature::sugar_cane(17, 3)
                    .with_biome_filter(FeatureBiomeFilter::Include(SUGAR_CANE_SWAMP_BIOMES)),
                PlacedBlockColumnFeature::cactus(18, 6)
                    .with_biome_filter(FeatureBiomeFilter::Include(CACTUS_DESERT_BIOMES)),
                PlacedBlockColumnFeature::cactus(19, 13)
                    .with_biome_filter(FeatureBiomeFilter::Include(CACTUS_DECORATED_BIOMES)),
                PlacedBlockColumnFeature::bamboo_light(99)
                    .with_biome_filter(FeatureBiomeFilter::Include(BAMBOO_LIGHT_BIOMES)),
                PlacedBlockColumnFeature::bamboo_some_podzol(100)
                    .with_biome_filter(FeatureBiomeFilter::Include(BAMBOO_SOME_PODZOL_BIOMES)),
            ],
            trees: vec![
                PlacedTreeFeature::trees_plains(3)
                    .with_biome_filter(FeatureBiomeFilter::Include(PLAINS_TREE_BIOMES)),
                PlacedTreeFeature::trees_taiga(44)
                    .with_biome_filter(FeatureBiomeFilter::Include(TAIGA_TREE_BIOMES)),
                PlacedTreeFeature::trees_snowy(45)
                    .with_biome_filter(FeatureBiomeFilter::Include(SNOWY_TREE_BIOMES)),
                PlacedTreeFeature::trees_savanna(46)
                    .with_biome_filter(FeatureBiomeFilter::Include(SAVANNA_TREE_BIOMES)),
                PlacedTreeFeature::trees_windswept_savanna(47)
                    .with_biome_filter(FeatureBiomeFilter::Include(WINDSWEPT_SAVANNA_TREE_BIOMES)),
                PlacedTreeFeature::trees_birch(42)
                    .with_biome_filter(FeatureBiomeFilter::Include(BIRCH_TREE_BIOMES)),
                PlacedTreeFeature::trees_tall_birch(43)
                    .with_biome_filter(FeatureBiomeFilter::Include(TALL_BIRCH_TREE_BIOMES)),
                PlacedTreeFeature::dark_forest_vegetation(48)
                    .with_biome_filter(FeatureBiomeFilter::Include(DARK_FOREST_TREE_BIOMES)),
                PlacedTreeFeature::pale_garden_vegetation(49)
                    .with_biome_filter(FeatureBiomeFilter::Include(PALE_GARDEN_TREE_BIOMES)),
                PlacedTreeFeature::trees_flower_forest(50)
                    .with_biome_filter(FeatureBiomeFilter::Include(FLOWER_FOREST_TREE_BIOMES)),
                PlacedTreeFeature::trees_meadow(51)
                    .with_biome_filter(FeatureBiomeFilter::Include(MEADOW_TREE_BIOMES)),
                PlacedTreeFeature::trees_cherry(52)
                    .with_biome_filter(FeatureBiomeFilter::Include(CHERRY_TREE_BIOMES)),
                PlacedTreeFeature::trees_grove(53)
                    .with_biome_filter(FeatureBiomeFilter::Include(GROVE_TREE_BIOMES)),
                PlacedTreeFeature::trees_badlands(54)
                    .with_biome_filter(FeatureBiomeFilter::Include(BADLANDS_TREE_BIOMES)),
                PlacedTreeFeature::trees_swamp(55)
                    .with_biome_filter(FeatureBiomeFilter::Include(SWAMP_TREE_BIOMES)),
                PlacedTreeFeature::trees_windswept_hills(56)
                    .with_biome_filter(FeatureBiomeFilter::Include(WINDSWEPT_HILLS_TREE_BIOMES)),
                PlacedTreeFeature::trees_windswept_forest(57)
                    .with_biome_filter(FeatureBiomeFilter::Include(WINDSWEPT_FOREST_TREE_BIOMES)),
                PlacedTreeFeature::trees_water(58)
                    .with_biome_filter(FeatureBiomeFilter::Include(WATER_TREE_BIOMES)),
                PlacedTreeFeature::trees_birch_and_oak_leaf_litter(59).with_biome_filter(
                    FeatureBiomeFilter::Include(BIRCH_AND_OAK_LEAF_LITTER_TREE_BIOMES),
                ),
                PlacedTreeFeature::trees_sparse_jungle(60)
                    .with_biome_filter(FeatureBiomeFilter::Include(SPARSE_JUNGLE_TREE_BIOMES)),
                PlacedTreeFeature::trees_old_growth_spruce_taiga(61).with_biome_filter(
                    FeatureBiomeFilter::Include(OLD_GROWTH_SPRUCE_TAIGA_TREE_BIOMES),
                ),
                PlacedTreeFeature::trees_old_growth_pine_taiga(62).with_biome_filter(
                    FeatureBiomeFilter::Include(OLD_GROWTH_PINE_TAIGA_TREE_BIOMES),
                ),
                PlacedTreeFeature::trees_jungle(63)
                    .with_biome_filter(FeatureBiomeFilter::Include(JUNGLE_TREE_BIOMES)),
                PlacedTreeFeature::bamboo_vegetation(64)
                    .with_biome_filter(FeatureBiomeFilter::Include(BAMBOO_JUNGLE_TREE_BIOMES)),
                PlacedTreeFeature::trees_mangrove(65)
                    .with_biome_filter(FeatureBiomeFilter::Include(MANGROVE_TREE_BIOMES)),
            ],
            freeze_top_layer: PlacedFreezeTopLayerFeature::new(0),
        }
    }

    fn ordered_features(&self) -> Vec<PlacedUndergroundFeature<'_>> {
        let mut features = Vec::with_capacity(
            self.features.len()
                + self.disks.len()
                + self.springs.len()
                + self.lakes.len()
                + self.geodes.len()
                + self.dripstone_features.len()
                + self.sculk_features.len()
                + self.structure_features.len()
                + self.surface_features.len()
                + self.monster_rooms.len()
                + self.environment_scan_features.len()
                + self.aquatic_features.len()
                + self.huge_mushrooms.len()
                + self.vegetation_patches.len()
                + self.block_columns.len()
                + self.trees.len()
                + 6,
        );
        features.extend(self.lakes.iter().map(PlacedUndergroundFeature::Lake));
        features.extend(self.geodes.iter().map(PlacedUndergroundFeature::Geode));
        features.extend(
            self.dripstone_features
                .iter()
                .map(PlacedUndergroundFeature::Dripstone),
        );
        features.extend(
            self.sculk_features
                .iter()
                .map(PlacedUndergroundFeature::Sculk),
        );
        features.extend(
            self.structure_features
                .iter()
                .map(PlacedUndergroundFeature::Structure),
        );
        features.extend(
            self.surface_features
                .iter()
                .map(PlacedUndergroundFeature::Surface),
        );
        features.extend(
            self.monster_rooms
                .iter()
                .map(PlacedUndergroundFeature::MonsterRoom),
        );
        features.extend(self.features.iter().map(PlacedUndergroundFeature::Ore));
        features.push(PlacedUndergroundFeature::UnderwaterMagma(
            &self.underwater_magma,
        ));
        features.extend(self.disks.iter().map(PlacedUndergroundFeature::Disk));
        features.extend(self.springs.iter().map(PlacedUndergroundFeature::Spring));
        features.push(PlacedUndergroundFeature::MultifaceGrowth(&self.glow_lichen));
        features.extend(
            self.environment_scan_features
                .iter()
                .map(PlacedUndergroundFeature::EnvironmentScan),
        );
        features.push(PlacedUndergroundFeature::CaveVines(&self.cave_vines));
        features.push(PlacedUndergroundFeature::SporeBlossom(
            &self.spore_blossom,
        ));
        features.push(PlacedUndergroundFeature::ClassicVines(
            &self.classic_vines,
        ));
        features.extend(
            self.aquatic_features
                .iter()
                .map(PlacedUndergroundFeature::Aquatic),
        );
        features.extend(
            self.huge_mushrooms
                .iter()
                .map(PlacedUndergroundFeature::HugeMushroom),
        );
        features.extend(
            self.vegetation_patches
                .iter()
                .map(PlacedUndergroundFeature::SimpleVegetation),
        );
        features.push(PlacedUndergroundFeature::ClassicVines(
            &self.surface_vines,
        ));
        features.extend(
            self.block_columns
                .iter()
                .map(PlacedUndergroundFeature::BlockColumn),
        );
        features.extend(self.trees.iter().map(PlacedUndergroundFeature::Tree));
        features.push(PlacedUndergroundFeature::FreezeTopLayer(
            &self.freeze_top_layer,
        ));
        features.sort_by_key(|feature| (feature.step_index(), feature.feature_index()));
        features
    }

    fn place_chunk(
        &self,
        settings: &NoiseSettings,
        chunk_x: i32,
        chunk_z: i32,
        chunk: &mut NoiseChunkBlocks,
    ) {
        let origin_x = chunk_x * 16;
        let origin_z = chunk_z * 16;
        let decoration_seed = FeatureRandom::decoration_seed(self.seed, origin_x, origin_z);
        let features = self.ordered_features();
        let mut neighbor_sources = self.neighbor_feature_sources(settings, chunk_x, chunk_z);

        for feature in features.iter().copied() {
            let mut random = FeatureRandom::for_feature(
                decoration_seed,
                feature.feature_index(),
                feature.step_index(),
            );
            match feature {
                PlacedUndergroundFeature::MultifaceGrowth(feature) => {
                    let neighbor_chunks: Vec<_> = neighbor_sources
                        .iter()
                        .map(|source| (source.origin_x, source.origin_z, &source.chunk))
                        .collect();
                    feature.place_with_neighbors(
                        settings,
                        origin_x,
                        origin_z,
                        chunk,
                        &neighbor_chunks,
                        &mut random,
                    );
                }
                PlacedUndergroundFeature::ClassicVines(feature) => {
                    let neighbor_chunks: Vec<_> = neighbor_sources
                        .iter()
                        .map(|source| (source.origin_x, source.origin_z, &source.chunk))
                        .collect();
                    feature.place_with_neighbors(
                        settings,
                        origin_x,
                        origin_z,
                        chunk,
                        &neighbor_chunks,
                        &mut random,
                    );
                }
                PlacedUndergroundFeature::BlockColumn(feature) => {
                    let neighbor_chunks: Vec<_> = neighbor_sources
                        .iter()
                        .map(|source| (source.origin_x, source.origin_z, &source.chunk))
                        .collect();
                    feature.place_with_neighbors(
                        settings,
                        origin_x,
                        origin_z,
                        chunk,
                        &neighbor_chunks,
                        &mut random,
                    );
                }
                PlacedUndergroundFeature::SimpleVegetation(feature) => {
                    let neighbor_chunks: Vec<_> = neighbor_sources
                        .iter()
                        .map(|source| (source.origin_x, source.origin_z, &source.chunk))
                        .collect();
                    feature.place_with_neighbors(
                        settings,
                        origin_x,
                        origin_z,
                        chunk,
                        &neighbor_chunks,
                        &mut random,
                    );
                }
                PlacedUndergroundFeature::MonsterRoom(feature) => {
                    let neighbor_chunks: Vec<_> = neighbor_sources
                        .iter()
                        .map(|source| (source.origin_x, source.origin_z, &source.chunk))
                        .collect();
                    feature.place_with_neighbors(
                        settings,
                        origin_x,
                        origin_z,
                        chunk,
                        &neighbor_chunks,
                        &mut random,
                    );
                }
                PlacedUndergroundFeature::Structure(feature) => {
                    let neighbor_chunks: Vec<_> = neighbor_sources
                        .iter()
                        .map(|source| (source.origin_x, source.origin_z, &source.chunk))
                        .collect();
                    feature.place_with_neighbors(
                        settings,
                        origin_x,
                        origin_z,
                        chunk,
                        &neighbor_chunks,
                        &mut random,
                    );
                }
                PlacedUndergroundFeature::HugeMushroom(feature) => {
                    let neighbor_chunks: Vec<_> = neighbor_sources
                        .iter()
                        .map(|source| (source.origin_x, source.origin_z, &source.chunk))
                        .collect();
                    feature.place_with_neighbors(
                        settings,
                        origin_x,
                        origin_z,
                        chunk,
                        &neighbor_chunks,
                        &mut random,
                    );
                }
                _ => feature.place(settings, origin_x, origin_z, chunk, &mut random),
            }

            for source_index in 0..neighbor_sources.len() {
                let (before, current_and_after) = neighbor_sources.split_at_mut(source_index);
                let Some((source, after)) = current_and_after.split_first_mut() else {
                    continue;
                };
                let mut random = FeatureRandom::for_feature(
                    source.decoration_seed,
                    feature.feature_index(),
                    feature.step_index(),
                );
                match feature {
                    PlacedUndergroundFeature::MonsterRoom(feature) => {
                        let source_neighbors: Vec<_> = before
                            .iter()
                            .chain(after.iter())
                            .map(|source| (source.origin_x, source.origin_z, &source.chunk))
                            .collect();
                        feature.place_with_spillover_neighbors(
                            settings,
                            source.origin_x,
                            source.origin_z,
                            origin_x,
                            origin_z,
                            &mut source.chunk,
                            chunk,
                            &source_neighbors,
                            &mut random,
                        );
                    }
                    PlacedUndergroundFeature::Structure(feature) => {
                        let source_neighbors: Vec<_> = before
                            .iter()
                            .chain(after.iter())
                            .map(|source| (source.origin_x, source.origin_z, &source.chunk))
                            .collect();
                        feature.place_with_spillover_neighbors(
                            settings,
                            source.origin_x,
                            source.origin_z,
                            origin_x,
                            origin_z,
                            &mut source.chunk,
                            chunk,
                            &source_neighbors,
                            &mut random,
                        );
                    }
                    PlacedUndergroundFeature::HugeMushroom(feature) => {
                        let source_neighbors: Vec<_> = before
                            .iter()
                            .chain(after.iter())
                            .map(|source| (source.origin_x, source.origin_z, &source.chunk))
                            .collect();
                        feature.place_with_spillover_neighbors(
                            settings,
                            source.origin_x,
                            source.origin_z,
                            origin_x,
                            origin_z,
                            &mut source.chunk,
                            chunk,
                            &source_neighbors,
                            &mut random,
                        );
                    }
                    _ => feature.place_spillover_from(
                        settings,
                        source.origin_x,
                        source.origin_z,
                        origin_x,
                        origin_z,
                        &mut source.chunk,
                        chunk,
                        &mut random,
                    ),
                }
            }
        }
    }

    fn neighbor_feature_sources(
        &self,
        settings: &NoiseSettings,
        chunk_x: i32,
        chunk_z: i32,
    ) -> Vec<NeighborFeatureSource> {
        let mut sources = Vec::with_capacity(8);
        for source_dx in -1..=1 {
            for source_dz in -1..=1 {
                if source_dx == 0 && source_dz == 0 {
                    continue;
                }

                let chunk_x = chunk_x + source_dx;
                let chunk_z = chunk_z + source_dz;
                let origin_x = chunk_x * 16;
                let origin_z = chunk_z * 16;
                let decoration_seed = FeatureRandom::decoration_seed(self.seed, origin_x, origin_z);
                let (mut chunk, preliminary_surfaces) =
                    settings.generate_base_chunk(chunk_x, chunk_z);
                settings.carvers.carve_chunk(
                    settings,
                    chunk_x,
                    chunk_z,
                    &preliminary_surfaces,
                    &mut chunk,
                );

                sources.push(NeighborFeatureSource {
                    origin_x,
                    origin_z,
                    decoration_seed,
                    chunk,
                });
            }
        }
        sources
    }
}

struct NeighborFeatureSource {
    origin_x: i32,
    origin_z: i32,
    decoration_seed: i64,
    chunk: NoiseChunkBlocks,
}

#[derive(Clone, Copy)]
enum PlacedUndergroundFeature<'a> {
    Lake(&'a PlacedLakeFeature),
    Geode(&'a PlacedGeodeFeature),
    Dripstone(&'a PlacedDripstoneFeature),
    Sculk(&'a PlacedSculkFeature),
    Structure(&'a PlacedStructureFeature),
    Surface(&'a PlacedSurfaceFeature),
    MonsterRoom(&'a PlacedMonsterRoomFeature),
    Ore(&'a PlacedOreFeature),
    UnderwaterMagma(&'a PlacedUnderwaterMagmaFeature),
    Disk(&'a PlacedDiskFeature),
    Spring(&'a PlacedSpringFeature),
    MultifaceGrowth(&'a PlacedMultifaceGrowthFeature),
    CaveVines(&'a PlacedCaveVinesFeature),
    ClassicVines(&'a PlacedClassicVinesFeature),
    SporeBlossom(&'a PlacedSporeBlossomFeature),
    EnvironmentScan(&'a PlacedEnvironmentScanFeature),
    Aquatic(&'a PlacedAquaticFeature),
    HugeMushroom(&'a PlacedHugeMushroomFeature),
    SimpleVegetation(&'a PlacedSimpleVegetationFeature),
    BlockColumn(&'a PlacedBlockColumnFeature),
    Tree(&'a PlacedTreeFeature),
    FreezeTopLayer(&'a PlacedFreezeTopLayerFeature),
}

impl PlacedUndergroundFeature<'_> {
    fn step_index(self) -> i32 {
        match self {
            Self::Lake(feature) => feature.step_index,
            Self::Geode(feature) => feature.step_index,
            Self::Dripstone(feature) => feature.step_index(),
            Self::Sculk(feature) => feature.step_index(),
            Self::Structure(feature) => feature.step_index(),
            Self::Surface(feature) => feature.step_index,
            Self::MonsterRoom(feature) => feature.step_index,
            Self::Ore(feature) => feature.step_index,
            Self::UnderwaterMagma(feature) => feature.step_index,
            Self::Disk(feature) => feature.step_index,
            Self::Spring(feature) => feature.step_index,
            Self::MultifaceGrowth(feature) => feature.step_index,
            Self::CaveVines(feature) => feature.step_index,
            Self::ClassicVines(feature) => feature.step_index,
            Self::SporeBlossom(feature) => feature.step_index,
            Self::EnvironmentScan(feature) => feature.step_index,
            Self::Aquatic(feature) => feature.step_index,
            Self::HugeMushroom(feature) => feature.step_index,
            Self::SimpleVegetation(feature) => feature.step_index,
            Self::BlockColumn(feature) => feature.step_index,
            Self::Tree(feature) => feature.step_index,
            Self::FreezeTopLayer(feature) => feature.step_index,
        }
    }

    fn feature_index(self) -> i32 {
        match self {
            Self::Lake(feature) => feature.feature_index,
            Self::Geode(feature) => feature.feature_index,
            Self::Dripstone(feature) => feature.feature_index(),
            Self::Sculk(feature) => feature.feature_index(),
            Self::Structure(feature) => feature.feature_index(),
            Self::Surface(feature) => feature.feature_index,
            Self::MonsterRoom(feature) => feature.feature_index,
            Self::Ore(feature) => feature.feature_index,
            Self::UnderwaterMagma(feature) => feature.feature_index,
            Self::Disk(feature) => feature.feature_index,
            Self::Spring(feature) => feature.feature_index,
            Self::MultifaceGrowth(feature) => feature.feature_index,
            Self::CaveVines(feature) => feature.feature_index,
            Self::ClassicVines(feature) => feature.feature_index,
            Self::SporeBlossom(feature) => feature.feature_index,
            Self::EnvironmentScan(feature) => feature.feature_index,
            Self::Aquatic(feature) => feature.feature_index,
            Self::HugeMushroom(feature) => feature.feature_index,
            Self::SimpleVegetation(feature) => feature.feature_index,
            Self::BlockColumn(feature) => feature.feature_index,
            Self::Tree(feature) => feature.feature_index,
            Self::FreezeTopLayer(feature) => feature.feature_index,
        }
    }

    fn place(
        self,
        settings: &NoiseSettings,
        origin_x: i32,
        origin_z: i32,
        chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
    ) {
        match self {
            Self::Lake(feature) => feature.place(settings, origin_x, origin_z, chunk, random),
            Self::Geode(feature) => feature.place(settings, origin_x, origin_z, chunk, random),
            Self::Dripstone(feature) => {
                feature.place(settings, origin_x, origin_z, chunk, random);
            }
            Self::Sculk(feature) => {
                feature.place(settings, origin_x, origin_z, chunk, random);
            }
            Self::Structure(feature) => {
                feature.place(settings, origin_x, origin_z, chunk, random);
            }
            Self::Surface(feature) => feature.place(settings, origin_x, origin_z, chunk, random),
            Self::MonsterRoom(feature) => {
                feature.place(settings, origin_x, origin_z, chunk, random);
            }
            Self::Ore(feature) => feature.place(settings, origin_x, origin_z, chunk, random),
            Self::UnderwaterMagma(feature) => {
                feature.place(settings, origin_x, origin_z, chunk, random);
            }
            Self::Disk(feature) => feature.place(settings, origin_x, origin_z, chunk, random),
            Self::Spring(feature) => feature.place(settings, origin_x, origin_z, chunk, random),
            Self::MultifaceGrowth(feature) => {
                feature.place(settings, origin_x, origin_z, chunk, random);
            }
            Self::CaveVines(feature) => {
                feature.place(settings, origin_x, origin_z, chunk, random);
            }
            Self::ClassicVines(feature) => {
                feature.place(settings, origin_x, origin_z, chunk, random);
            }
            Self::SporeBlossom(feature) => {
                feature.place(settings, origin_x, origin_z, chunk, random);
            }
            Self::EnvironmentScan(feature) => {
                feature.place(settings, origin_x, origin_z, chunk, random);
            }
            Self::Aquatic(feature) => {
                feature.place(settings, origin_x, origin_z, chunk, random);
            }
            Self::HugeMushroom(feature) => {
                feature.place(settings, origin_x, origin_z, chunk, random);
            }
            Self::SimpleVegetation(feature) => {
                feature.place(settings, origin_x, origin_z, chunk, random);
            }
            Self::BlockColumn(feature) => {
                feature.place(settings, origin_x, origin_z, chunk, random);
            }
            Self::Tree(feature) => {
                feature.place(settings, origin_x, origin_z, chunk, random);
            }
            Self::FreezeTopLayer(feature) => feature.place(settings, origin_x, origin_z, chunk),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn place_spillover_from(
        self,
        settings: &NoiseSettings,
        source_origin_x: i32,
        source_origin_z: i32,
        target_origin_x: i32,
        target_origin_z: i32,
        source_chunk: &mut NoiseChunkBlocks,
        target_chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
    ) {
        match self {
            Self::Lake(feature) => feature.place_with_spillover(
                settings,
                source_origin_x,
                source_origin_z,
                target_origin_x,
                target_origin_z,
                source_chunk,
                target_chunk,
                random,
            ),
            Self::Geode(feature) => feature.place_with_spillover(
                settings,
                source_origin_x,
                source_origin_z,
                target_origin_x,
                target_origin_z,
                source_chunk,
                target_chunk,
                random,
            ),
            Self::Dripstone(feature) => feature.place_with_spillover(
                settings,
                source_origin_x,
                source_origin_z,
                target_origin_x,
                target_origin_z,
                source_chunk,
                target_chunk,
                random,
            ),
            Self::Sculk(feature) => feature.place_with_spillover(
                settings,
                source_origin_x,
                source_origin_z,
                target_origin_x,
                target_origin_z,
                source_chunk,
                target_chunk,
                random,
            ),
            Self::Structure(feature) => feature.place_with_spillover(
                settings,
                source_origin_x,
                source_origin_z,
                target_origin_x,
                target_origin_z,
                source_chunk,
                target_chunk,
                random,
            ),
            Self::Ore(feature) => feature.place_with_spillover(
                settings,
                source_origin_x,
                source_origin_z,
                target_origin_x,
                target_origin_z,
                source_chunk,
                target_chunk,
                random,
            ),
            Self::Spring(feature) => feature.place_with_spillover(
                settings,
                source_origin_x,
                source_origin_z,
                target_origin_x,
                target_origin_z,
                source_chunk,
                target_chunk,
                random,
            ),
            Self::UnderwaterMagma(feature) => feature.place_with_spillover(
                settings,
                source_origin_x,
                source_origin_z,
                target_origin_x,
                target_origin_z,
                source_chunk,
                target_chunk,
                random,
            ),
            Self::Disk(feature) => feature.place_with_spillover(
                settings,
                source_origin_x,
                source_origin_z,
                target_origin_x,
                target_origin_z,
                source_chunk,
                target_chunk,
                random,
            ),
            Self::MultifaceGrowth(feature) => feature.place_with_spillover(
                settings,
                source_origin_x,
                source_origin_z,
                target_origin_x,
                target_origin_z,
                source_chunk,
                target_chunk,
                random,
            ),
            Self::Surface(feature) => feature.place_with_spillover(
                settings,
                source_origin_x,
                source_origin_z,
                target_origin_x,
                target_origin_z,
                source_chunk,
                target_chunk,
                random,
            ),
            Self::MonsterRoom(feature) => feature.place_with_spillover(
                settings,
                source_origin_x,
                source_origin_z,
                target_origin_x,
                target_origin_z,
                source_chunk,
                target_chunk,
                random,
            ),
            Self::EnvironmentScan(feature) => feature.place_with_spillover(
                settings,
                source_origin_x,
                source_origin_z,
                target_origin_x,
                target_origin_z,
                source_chunk,
                target_chunk,
                random,
            ),
            Self::Aquatic(feature) => feature.place_with_spillover(
                settings,
                source_origin_x,
                source_origin_z,
                target_origin_x,
                target_origin_z,
                source_chunk,
                target_chunk,
                random,
            ),
            Self::HugeMushroom(feature) => feature.place_with_spillover(
                settings,
                source_origin_x,
                source_origin_z,
                target_origin_x,
                target_origin_z,
                source_chunk,
                target_chunk,
                random,
            ),
            Self::SimpleVegetation(feature) => feature.place_with_spillover(
                settings,
                source_origin_x,
                source_origin_z,
                target_origin_x,
                target_origin_z,
                source_chunk,
                target_chunk,
                random,
            ),
            Self::BlockColumn(feature) => feature.place_with_spillover(
                settings,
                source_origin_x,
                source_origin_z,
                target_origin_x,
                target_origin_z,
                source_chunk,
                target_chunk,
                random,
            ),
            Self::ClassicVines(feature) => feature.place_with_neighbors(
                settings,
                source_origin_x,
                source_origin_z,
                source_chunk,
                &[(target_origin_x, target_origin_z, &*target_chunk)],
                random,
            ),
            Self::Tree(feature) => feature.place_with_spillover(
                settings,
                source_origin_x,
                source_origin_z,
                target_origin_x,
                target_origin_z,
                source_chunk,
                target_chunk,
                random,
            ),
            _ => self.place(settings, source_origin_x, source_origin_z, source_chunk, random),
        }
    }
}

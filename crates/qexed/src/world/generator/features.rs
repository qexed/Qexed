#[derive(Debug, Clone)]
struct OverworldOreFeatures {
    seed: i64,
    features: Vec<PlacedOreFeature>,
    underwater_magma: PlacedUnderwaterMagmaFeature,
    disks: Vec<PlacedDiskFeature>,
    springs: Vec<PlacedSpringFeature>,
    lakes: Vec<PlacedLakeFeature>,
    geodes: Vec<PlacedGeodeFeature>,
    monster_rooms: Vec<PlacedMonsterRoomFeature>,
    glow_lichen: PlacedMultifaceGrowthFeature,
    vegetation_patches: Vec<PlacedSimpleVegetationFeature>,
    block_columns: Vec<PlacedBlockColumnFeature>,
    trees: Vec<PlacedTreeFeature>,
    freeze_top_layer: PlacedFreezeTopLayerFeature,
}

impl OverworldOreFeatures {
    fn new(seed: i64) -> Self {
        let dirt = OreFeatureConfig::base_stone(33, "minecraft:dirt");
        let gravel = OreFeatureConfig::base_stone(33, "minecraft:gravel");
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
            ],
            springs: vec![
                PlacedSpringFeature::water(0),
                PlacedSpringFeature::lava_overworld(1),
            ],
            lakes: vec![
                PlacedLakeFeature::lava_underground(0),
                PlacedLakeFeature::lava_surface(1),
            ],
            geodes: vec![PlacedGeodeFeature::amethyst(0)],
            monster_rooms: vec![
                PlacedMonsterRoomFeature::regular(0),
                PlacedMonsterRoomFeature::deep(1),
            ],
            glow_lichen: PlacedMultifaceGrowthFeature::glow_lichen(0),
            vegetation_patches: vec![
                PlacedSimpleVegetationFeature::patch_tall_grass_2(1)
                    .with_biome_filter(FeatureBiomeFilter::Include(PATCH_TALL_GRASS_2_BIOMES)),
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
            ],
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
            ],
            trees: vec![
                PlacedTreeFeature::trees_plains(3)
                    .with_biome_filter(FeatureBiomeFilter::Include(PLAINS_TREE_BIOMES)),
                PlacedTreeFeature::trees_taiga(44)
                    .with_biome_filter(FeatureBiomeFilter::Include(TAIGA_TREE_BIOMES)),
                PlacedTreeFeature::trees_snowy(45)
                    .with_biome_filter(FeatureBiomeFilter::Include(SNOWY_TREE_BIOMES)),
                PlacedTreeFeature::trees_birch(42)
                    .with_biome_filter(FeatureBiomeFilter::Include(BIRCH_TREE_BIOMES)),
                PlacedTreeFeature::trees_tall_birch(43)
                    .with_biome_filter(FeatureBiomeFilter::Include(TALL_BIRCH_TREE_BIOMES)),
            ],
            freeze_top_layer: PlacedFreezeTopLayerFeature::new(0),
        }
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

        let mut features = Vec::with_capacity(
            self.features.len()
                + self.disks.len()
                + self.springs.len()
                + self.lakes.len()
                + self.geodes.len()
                + self.monster_rooms.len()
                + self.vegetation_patches.len()
                + self.block_columns.len()
                + self.trees.len()
                + 3,
        );
        features.extend(self.lakes.iter().map(PlacedUndergroundFeature::Lake));
        features.extend(self.geodes.iter().map(PlacedUndergroundFeature::Geode));
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
            self.vegetation_patches
                .iter()
                .map(PlacedUndergroundFeature::SimpleVegetation),
        );
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

        for feature in features {
            let mut random = FeatureRandom::for_feature(
                decoration_seed,
                feature.feature_index(),
                feature.step_index(),
            );
            feature.place(settings, origin_x, origin_z, chunk, &mut random);
        }
    }
}

#[derive(Clone, Copy)]
enum PlacedUndergroundFeature<'a> {
    Lake(&'a PlacedLakeFeature),
    Geode(&'a PlacedGeodeFeature),
    MonsterRoom(&'a PlacedMonsterRoomFeature),
    Ore(&'a PlacedOreFeature),
    UnderwaterMagma(&'a PlacedUnderwaterMagmaFeature),
    Disk(&'a PlacedDiskFeature),
    Spring(&'a PlacedSpringFeature),
    MultifaceGrowth(&'a PlacedMultifaceGrowthFeature),
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
            Self::MonsterRoom(feature) => feature.step_index,
            Self::Ore(feature) => feature.step_index,
            Self::UnderwaterMagma(feature) => feature.step_index,
            Self::Disk(feature) => feature.step_index,
            Self::Spring(feature) => feature.step_index,
            Self::MultifaceGrowth(feature) => feature.step_index,
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
            Self::MonsterRoom(feature) => feature.feature_index,
            Self::Ore(feature) => feature.feature_index,
            Self::UnderwaterMagma(feature) => feature.feature_index,
            Self::Disk(feature) => feature.feature_index,
            Self::Spring(feature) => feature.feature_index,
            Self::MultifaceGrowth(feature) => feature.feature_index,
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
}

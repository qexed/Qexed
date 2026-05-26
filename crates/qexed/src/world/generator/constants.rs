const DEFAULT_FLAT_PRESET: &str = "minecraft:classic_flat";
const FLAT_PRESET_ROOT: &str =
    "assets/decompiled_source/src/data/minecraft/worldgen/flat_level_generator_preset";
const DEFAULT_NOISE_PRESET: &str = "minecraft:overworld";
const NOISE_SETTINGS_ROOT: &str =
    "assets/decompiled_source/src/data/minecraft/worldgen/noise_settings";
const HEIGHTMAP_BITS: usize = 9;
const HEIGHTMAP_ENTRY_COUNT: usize = 16 * 16;
const BLOCK_ENTRY_COUNT: usize = 16 * 16 * 16;
const SLOW_NOISE_CHUNK_LOG_THRESHOLD: Duration = Duration::from_millis(200);
const CARVER_RANGE: i32 = 4;
const CARVER_SOURCE_RANGE: i32 = 8;
const EMERALD_ORE_BIOMES: &[&str] = &[
    "minecraft:cherry_grove",
    "minecraft:frozen_peaks",
    "minecraft:grove",
    "minecraft:jagged_peaks",
    "minecraft:meadow",
    "minecraft:snowy_slopes",
    "minecraft:stony_peaks",
    "minecraft:windswept_gravelly_hills",
    "minecraft:windswept_hills",
    "minecraft:windswept_forest",
];
const BADLANDS_ORE_BIOMES: &[&str] = &[
    "minecraft:badlands",
    "minecraft:eroded_badlands",
    "minecraft:wooded_badlands",
];
const DRIPSTONE_CAVES_ORE_BIOMES: &[&str] = &["minecraft:dripstone_caves"];
const LUSH_CAVES_ORE_BIOMES: &[&str] = &["minecraft:lush_caves"];
const DRIPSTONE_CAVES_BIOMES: &[&str] = &["minecraft:dripstone_caves"];
const DEEP_DARK_BIOMES: &[&str] = &["minecraft:deep_dark"];
const DESERT_WELL_BIOMES: &[&str] = &["minecraft:desert"];
const FOSSIL_BIOMES: &[&str] = &["minecraft:desert", "minecraft:mangrove_swamp", "minecraft:swamp"];
const FLOWER_PLAINS_BIOMES: &[&str] = &[
    "minecraft:plains",
    "minecraft:sunflower_plains",
    "minecraft:deep_dark",
    "minecraft:dripstone_caves",
];
const FLOWER_DEFAULT_BIOMES: &[&str] = &[
    "minecraft:beach",
    "minecraft:birch_forest",
    "minecraft:cold_ocean",
    "minecraft:dark_forest",
    "minecraft:deep_cold_ocean",
    "minecraft:deep_frozen_ocean",
    "minecraft:deep_lukewarm_ocean",
    "minecraft:deep_ocean",
    "minecraft:desert",
    "minecraft:forest",
    "minecraft:frozen_ocean",
    "minecraft:frozen_river",
    "minecraft:ice_spikes",
    "minecraft:lukewarm_ocean",
    "minecraft:ocean",
    "minecraft:old_growth_birch_forest",
    "minecraft:old_growth_pine_taiga",
    "minecraft:old_growth_spruce_taiga",
    "minecraft:river",
    "minecraft:snowy_beach",
    "minecraft:snowy_plains",
    "minecraft:snowy_taiga",
    "minecraft:stony_shore",
    "minecraft:taiga",
    "minecraft:warm_ocean",
    "minecraft:windswept_forest",
    "minecraft:windswept_gravelly_hills",
    "minecraft:windswept_hills",
    "minecraft:windswept_savanna",
];
const FLOWER_WARM_BIOMES: &[&str] = &[
    "minecraft:bamboo_jungle",
    "minecraft:jungle",
    "minecraft:savanna",
    "minecraft:savanna_plateau",
    "minecraft:sparse_jungle",
];
const FLOWER_SWAMP_BIOMES: &[&str] = &["minecraft:swamp"];
const FLOWER_CHERRY_BIOMES: &[&str] = &["minecraft:cherry_grove"];
const FLOWER_PALE_GARDEN_BIOMES: &[&str] = &["minecraft:pale_garden"];
const FLOWER_MEADOW_BIOMES: &[&str] = &["minecraft:meadow"];
const FLOWER_FLOWER_FOREST_BIOMES: &[&str] = &["minecraft:flower_forest"];
const FOREST_FLOWERS_BIOMES: &[&str] = &[
    "minecraft:birch_forest",
    "minecraft:dark_forest",
    "minecraft:forest",
    "minecraft:old_growth_birch_forest",
];
const FLOWER_FOREST_FLOWERS_BIOMES: &[&str] = &["minecraft:flower_forest"];
const PATCH_LEAF_LITTER_BIOMES: &[&str] = &["minecraft:dark_forest"];
const WILDFLOWERS_MEADOW_BIOMES: &[&str] = &["minecraft:meadow"];
const WILDFLOWERS_BIRCH_FOREST_BIOMES: &[&str] =
    &["minecraft:birch_forest", "minecraft:old_growth_birch_forest"];
const PATCH_GRASS_PLAIN_BIOMES: &[&str] = &[
    "minecraft:plains",
    "minecraft:sunflower_plains",
    "minecraft:deep_dark",
    "minecraft:dripstone_caves",
    "minecraft:cherry_grove",
];
const PATCH_TALL_GRASS_2_BIOMES: &[&str] = &[
    "minecraft:cherry_grove",
    "minecraft:deep_dark",
    "minecraft:dripstone_caves",
    "minecraft:lush_caves",
    "minecraft:meadow",
    "minecraft:plains",
    "minecraft:sunflower_plains",
];
const PATCH_TALL_GRASS_BIOMES: &[&str] =
    &["minecraft:savanna", "minecraft:savanna_plateau"];
const PATCH_BUSH_BIOMES: &[&str] = &[
    "minecraft:birch_forest",
    "minecraft:forest",
    "minecraft:frozen_river",
    "minecraft:old_growth_birch_forest",
    "minecraft:plains",
    "minecraft:river",
    "minecraft:windswept_forest",
    "minecraft:windswept_gravelly_hills",
    "minecraft:windswept_hills",
];
const PATCH_GRASS_NORMAL_BIOMES: &[&str] = &[
    "minecraft:mangrove_swamp",
    "minecraft:swamp",
    "minecraft:windswept_savanna",
];
const PATCH_GRASS_FOREST_BIOMES: &[&str] = &[
    "minecraft:birch_forest",
    "minecraft:dark_forest",
    "minecraft:forest",
    "minecraft:old_growth_birch_forest",
    "minecraft:pale_garden",
];
const PATCH_GRASS_BADLANDS_BIOMES: &[&str] = &[
    "minecraft:badlands",
    "minecraft:beach",
    "minecraft:cold_ocean",
    "minecraft:deep_cold_ocean",
    "minecraft:deep_frozen_ocean",
    "minecraft:deep_lukewarm_ocean",
    "minecraft:deep_ocean",
    "minecraft:desert",
    "minecraft:eroded_badlands",
    "minecraft:flower_forest",
    "minecraft:frozen_ocean",
    "minecraft:frozen_river",
    "minecraft:ice_spikes",
    "minecraft:lukewarm_ocean",
    "minecraft:ocean",
    "minecraft:river",
    "minecraft:snowy_beach",
    "minecraft:snowy_plains",
    "minecraft:stony_shore",
    "minecraft:warm_ocean",
    "minecraft:windswept_forest",
    "minecraft:windswept_gravelly_hills",
    "minecraft:windswept_hills",
    "minecraft:wooded_badlands",
];
const PATCH_GRASS_SAVANNA_BIOMES: &[&str] = &["minecraft:savanna", "minecraft:savanna_plateau"];
const PATCH_GRASS_TAIGA_BIOMES: &[&str] = &[
    "minecraft:old_growth_pine_taiga",
    "minecraft:old_growth_spruce_taiga",
];
const PATCH_GRASS_TAIGA_2_BIOMES: &[&str] = &["minecraft:snowy_taiga", "minecraft:taiga"];
const PATCH_GRASS_JUNGLE_BIOMES: &[&str] = &[
    "minecraft:bamboo_jungle",
    "minecraft:jungle",
    "minecraft:sparse_jungle",
];
const PATCH_GRASS_MEADOW_BIOMES: &[&str] = &["minecraft:meadow"];
const PATCH_LARGE_FERN_BIOMES: &[&str] = &[
    "minecraft:old_growth_pine_taiga",
    "minecraft:old_growth_spruce_taiga",
    "minecraft:snowy_taiga",
    "minecraft:taiga",
];
const NORMAL_MUSHROOM_BIOMES: &[&str] = &[
    "minecraft:badlands",
    "minecraft:bamboo_jungle",
    "minecraft:beach",
    "minecraft:birch_forest",
    "minecraft:cold_ocean",
    "minecraft:dark_forest",
    "minecraft:deep_cold_ocean",
    "minecraft:deep_dark",
    "minecraft:deep_frozen_ocean",
    "minecraft:deep_lukewarm_ocean",
    "minecraft:deep_ocean",
    "minecraft:desert",
    "minecraft:dripstone_caves",
    "minecraft:eroded_badlands",
    "minecraft:flower_forest",
    "minecraft:forest",
    "minecraft:frozen_ocean",
    "minecraft:frozen_river",
    "minecraft:ice_spikes",
    "minecraft:jungle",
    "minecraft:lukewarm_ocean",
    "minecraft:ocean",
    "minecraft:old_growth_birch_forest",
    "minecraft:old_growth_pine_taiga",
    "minecraft:old_growth_spruce_taiga",
    "minecraft:plains",
    "minecraft:river",
    "minecraft:savanna",
    "minecraft:savanna_plateau",
    "minecraft:snowy_beach",
    "minecraft:snowy_plains",
    "minecraft:sparse_jungle",
    "minecraft:stony_shore",
    "minecraft:sunflower_plains",
    "minecraft:swamp",
    "minecraft:warm_ocean",
    "minecraft:windswept_forest",
    "minecraft:windswept_gravelly_hills",
    "minecraft:windswept_hills",
    "minecraft:windswept_savanna",
    "minecraft:wooded_badlands",
];
const PUMPKIN_PATCH_BIOMES: &[&str] = &[
    "minecraft:badlands",
    "minecraft:bamboo_jungle",
    "minecraft:beach",
    "minecraft:birch_forest",
    "minecraft:cold_ocean",
    "minecraft:dark_forest",
    "minecraft:deep_cold_ocean",
    "minecraft:deep_dark",
    "minecraft:deep_frozen_ocean",
    "minecraft:deep_lukewarm_ocean",
    "minecraft:deep_ocean",
    "minecraft:desert",
    "minecraft:dripstone_caves",
    "minecraft:eroded_badlands",
    "minecraft:flower_forest",
    "minecraft:forest",
    "minecraft:frozen_ocean",
    "minecraft:frozen_river",
    "minecraft:grove",
    "minecraft:ice_spikes",
    "minecraft:jungle",
    "minecraft:lukewarm_ocean",
    "minecraft:ocean",
    "minecraft:old_growth_birch_forest",
    "minecraft:old_growth_pine_taiga",
    "minecraft:old_growth_spruce_taiga",
    "minecraft:pale_garden",
    "minecraft:plains",
    "minecraft:river",
    "minecraft:savanna",
    "minecraft:savanna_plateau",
    "minecraft:snowy_beach",
    "minecraft:snowy_plains",
    "minecraft:snowy_slopes",
    "minecraft:snowy_taiga",
    "minecraft:sparse_jungle",
    "minecraft:stony_shore",
    "minecraft:sunflower_plains",
    "minecraft:swamp",
    "minecraft:taiga",
    "minecraft:warm_ocean",
    "minecraft:windswept_forest",
    "minecraft:windswept_gravelly_hills",
    "minecraft:windswept_hills",
    "minecraft:windswept_savanna",
    "minecraft:wooded_badlands",
];
const SUNFLOWER_PATCH_BIOMES: &[&str] = &["minecraft:sunflower_plains"];
const DEAD_BUSH_NORMAL_BIOMES: &[&str] = &[
    "minecraft:mangrove_swamp",
    "minecraft:old_growth_pine_taiga",
    "minecraft:old_growth_spruce_taiga",
    "minecraft:swamp",
];
const DEAD_BUSH_DESERT_BIOMES: &[&str] = &["minecraft:desert"];
const DEAD_BUSH_BADLANDS_BIOMES: &[&str] = &[
    "minecraft:badlands",
    "minecraft:eroded_badlands",
    "minecraft:wooded_badlands",
];
const MELON_PATCH_BIOMES: &[&str] = &["minecraft:bamboo_jungle", "minecraft:jungle"];
const MELON_SPARSE_PATCH_BIOMES: &[&str] = &["minecraft:sparse_jungle"];
const SUGAR_CANE_NORMAL_BIOMES: &[&str] = &[
    "minecraft:bamboo_jungle",
    "minecraft:beach",
    "minecraft:birch_forest",
    "minecraft:cold_ocean",
    "minecraft:dark_forest",
    "minecraft:deep_cold_ocean",
    "minecraft:deep_frozen_ocean",
    "minecraft:deep_lukewarm_ocean",
    "minecraft:deep_ocean",
    "minecraft:flower_forest",
    "minecraft:forest",
    "minecraft:frozen_ocean",
    "minecraft:frozen_river",
    "minecraft:ice_spikes",
    "minecraft:jungle",
    "minecraft:lukewarm_ocean",
    "minecraft:mushroom_fields",
    "minecraft:ocean",
    "minecraft:old_growth_birch_forest",
    "minecraft:old_growth_pine_taiga",
    "minecraft:old_growth_spruce_taiga",
    "minecraft:pale_garden",
    "minecraft:plains",
    "minecraft:river",
    "minecraft:savanna",
    "minecraft:savanna_plateau",
    "minecraft:snowy_beach",
    "minecraft:snowy_plains",
    "minecraft:snowy_taiga",
    "minecraft:sparse_jungle",
    "minecraft:stony_shore",
    "minecraft:sunflower_plains",
    "minecraft:taiga",
    "minecraft:warm_ocean",
    "minecraft:windswept_forest",
    "minecraft:windswept_gravelly_hills",
    "minecraft:windswept_hills",
    "minecraft:windswept_savanna",
];
const SUGAR_CANE_BADLANDS_BIOMES: &[&str] = &[
    "minecraft:badlands",
    "minecraft:eroded_badlands",
    "minecraft:wooded_badlands",
];
const SUGAR_CANE_DESERT_BIOMES: &[&str] = &["minecraft:desert"];
const SUGAR_CANE_SWAMP_BIOMES: &[&str] = &["minecraft:swamp"];
const CACTUS_DESERT_BIOMES: &[&str] = &["minecraft:desert"];
const CACTUS_DECORATED_BIOMES: &[&str] = &[
    "minecraft:badlands",
    "minecraft:eroded_badlands",
    "minecraft:wooded_badlands",
];
const DRY_GRASS_DESERT_BIOMES: &[&str] = &["minecraft:desert"];
const DRY_GRASS_BADLANDS_BIOMES: &[&str] = &[
    "minecraft:badlands",
    "minecraft:eroded_badlands",
    "minecraft:wooded_badlands",
];
const TAIGA_MUSHROOM_BIOMES: &[&str] = &[
    "minecraft:mushroom_fields",
    "minecraft:snowy_taiga",
    "minecraft:taiga",
];
const OLD_GROWTH_MUSHROOM_BIOMES: &[&str] = &[
    "minecraft:old_growth_pine_taiga",
    "minecraft:old_growth_spruce_taiga",
];
const SWAMP_MUSHROOM_BIOMES: &[&str] = &["minecraft:swamp"];
const MUSHROOM_ISLAND_VEGETATION_BIOMES: &[&str] = &["minecraft:mushroom_fields"];
const SEAGRASS_NORMAL_BIOMES: &[&str] = &["minecraft:ocean"];
const SEAGRASS_COLD_BIOMES: &[&str] = &["minecraft:cold_ocean"];
const SEAGRASS_DEEP_BIOMES: &[&str] = &["minecraft:deep_ocean"];
const SEAGRASS_DEEP_COLD_BIOMES: &[&str] = &["minecraft:deep_cold_ocean"];
const SEAGRASS_DEEP_WARM_BIOMES: &[&str] = &["minecraft:deep_lukewarm_ocean"];
const SEAGRASS_WARM_BIOMES: &[&str] = &["minecraft:lukewarm_ocean", "minecraft:warm_ocean"];
const SEAGRASS_SWAMP_BIOMES: &[&str] = &["minecraft:mangrove_swamp", "minecraft:swamp"];
const SEAGRASS_RIVER_BIOMES: &[&str] = &["minecraft:river"];
const WARM_OCEAN_VEGETATION_BIOMES: &[&str] = &["minecraft:warm_ocean"];
const KELP_COLD_BIOMES: &[&str] = &[
    "minecraft:cold_ocean",
    "minecraft:deep_cold_ocean",
    "minecraft:deep_frozen_ocean",
    "minecraft:deep_ocean",
    "minecraft:frozen_ocean",
    "minecraft:ocean",
];
const KELP_WARM_BIOMES: &[&str] = &["minecraft:deep_lukewarm_ocean", "minecraft:lukewarm_ocean"];
const SEA_PICKLE_BIOMES: &[&str] = &["minecraft:warm_ocean"];
const WATERLILY_BIOMES: &[&str] = &["minecraft:mangrove_swamp", "minecraft:swamp"];
const SURFACE_VINES_BIOMES: &[&str] = &[
    "minecraft:bamboo_jungle",
    "minecraft:jungle",
    "minecraft:sparse_jungle",
];
const FROZEN_LAVA_SPRING_BIOMES: &[&str] = &[
    "minecraft:frozen_peaks",
    "minecraft:grove",
    "minecraft:jagged_peaks",
    "minecraft:snowy_slopes",
];
const PLAINS_TREE_BIOMES: &[&str] = &["minecraft:plains", "minecraft:sunflower_plains"];
const BIRCH_TREE_BIOMES: &[&str] = &["minecraft:birch_forest"];
const TALL_BIRCH_TREE_BIOMES: &[&str] = &["minecraft:old_growth_birch_forest"];
const TAIGA_TREE_BIOMES: &[&str] = &["minecraft:snowy_taiga", "minecraft:taiga"];
const SNOWY_TREE_BIOMES: &[&str] = &["minecraft:ice_spikes", "minecraft:snowy_plains"];
const SAVANNA_TREE_BIOMES: &[&str] = &["minecraft:savanna", "minecraft:savanna_plateau"];
const WINDSWEPT_SAVANNA_TREE_BIOMES: &[&str] = &["minecraft:windswept_savanna"];
const DARK_FOREST_TREE_BIOMES: &[&str] = &["minecraft:dark_forest"];
const PALE_GARDEN_TREE_BIOMES: &[&str] = &["minecraft:pale_garden"];
const FLOWER_FOREST_TREE_BIOMES: &[&str] = &["minecraft:flower_forest"];
const MEADOW_TREE_BIOMES: &[&str] = &["minecraft:meadow"];
const CHERRY_TREE_BIOMES: &[&str] = &["minecraft:cherry_grove"];
const GROVE_TREE_BIOMES: &[&str] = &["minecraft:grove"];
const BADLANDS_TREE_BIOMES: &[&str] = &["minecraft:wooded_badlands"];
const SWAMP_TREE_BIOMES: &[&str] = &["minecraft:swamp"];
const WINDSWEPT_HILLS_TREE_BIOMES: &[&str] = &[
    "minecraft:windswept_hills",
    "minecraft:windswept_gravelly_hills",
];
const WINDSWEPT_FOREST_TREE_BIOMES: &[&str] = &["minecraft:windswept_forest"];
const WATER_TREE_BIOMES: &[&str] = &[
    "minecraft:cold_ocean",
    "minecraft:deep_cold_ocean",
    "minecraft:deep_frozen_ocean",
    "minecraft:deep_lukewarm_ocean",
    "minecraft:deep_ocean",
    "minecraft:frozen_ocean",
    "minecraft:frozen_river",
    "minecraft:lukewarm_ocean",
    "minecraft:ocean",
    "minecraft:river",
    "minecraft:warm_ocean",
];
const BIRCH_AND_OAK_LEAF_LITTER_TREE_BIOMES: &[&str] = &["minecraft:forest"];
const SPARSE_JUNGLE_TREE_BIOMES: &[&str] = &["minecraft:sparse_jungle"];
const OLD_GROWTH_SPRUCE_TAIGA_TREE_BIOMES: &[&str] = &["minecraft:old_growth_spruce_taiga"];
const OLD_GROWTH_PINE_TAIGA_TREE_BIOMES: &[&str] = &["minecraft:old_growth_pine_taiga"];
const JUNGLE_TREE_BIOMES: &[&str] = &["minecraft:jungle"];
const BAMBOO_JUNGLE_TREE_BIOMES: &[&str] = &["minecraft:bamboo_jungle"];
const MANGROVE_TREE_BIOMES: &[&str] = &["minecraft:mangrove_swamp"];
const PLAINS_FLOWER_LOW_BLOCKS: &[&str] = &[
    "minecraft:orange_tulip",
    "minecraft:red_tulip",
    "minecraft:pink_tulip",
    "minecraft:white_tulip",
];
const PLAINS_FLOWER_HIGH_BLOCKS: &[&str] = &[
    "minecraft:poppy",
    "minecraft:azure_bluet",
    "minecraft:oxeye_daisy",
    "minecraft:cornflower",
];
const DISK_DIRT_GRASS_TARGETS: &[&str] = &["minecraft:dirt", "minecraft:grass_block"];
const DISK_DIRT_CLAY_TARGETS: &[&str] = &["minecraft:dirt", "minecraft:clay"];
const DISK_DIRT_MUD_TARGETS: &[&str] = &["minecraft:dirt", "minecraft:mud"];
const ICE_PATCH_DISK_TARGETS: &[&str] = &[
    "minecraft:dirt",
    "minecraft:grass_block",
    "minecraft:podzol",
    "minecraft:coarse_dirt",
    "minecraft:mycelium",
    "minecraft:snow_block",
    "minecraft:ice",
];
const FOREST_ROCK_BIOMES: &[&str] = &[
    "minecraft:old_growth_pine_taiga",
    "minecraft:old_growth_spruce_taiga",
];
const ICEBERG_BIOMES: &[&str] = &["minecraft:frozen_ocean", "minecraft:deep_frozen_ocean"];
const ICE_SPIKE_BIOMES: &[&str] = &["minecraft:ice_spikes"];
const PALE_MOSS_PATCH_BIOMES: &[&str] = &["minecraft:pale_garden"];
const PALE_GARDEN_FLOWERS_BIOMES: &[&str] = &["minecraft:pale_garden"];
const BERRY_COMMON_BIOMES: &[&str] = &[
    "minecraft:old_growth_pine_taiga",
    "minecraft:old_growth_spruce_taiga",
    "minecraft:taiga",
];
const BAMBOO_LIGHT_BIOMES: &[&str] = &["minecraft:jungle"];
const BAMBOO_SOME_PODZOL_BIOMES: &[&str] = &["minecraft:bamboo_jungle"];
const BERRY_RARE_BIOMES: &[&str] = &["minecraft:snowy_taiga"];
const FIREFLY_BUSH_SWAMP_BIOMES: &[&str] = &["minecraft:swamp"];
const FIREFLY_BUSH_NEAR_WATER_BIOMES: &[&str] = &[
    "minecraft:badlands",
    "minecraft:bamboo_jungle",
    "minecraft:beach",
    "minecraft:birch_forest",
    "minecraft:cold_ocean",
    "minecraft:dark_forest",
    "minecraft:deep_cold_ocean",
    "minecraft:deep_frozen_ocean",
    "minecraft:deep_lukewarm_ocean",
    "minecraft:deep_ocean",
    "minecraft:eroded_badlands",
    "minecraft:flower_forest",
    "minecraft:forest",
    "minecraft:frozen_ocean",
    "minecraft:frozen_river",
    "minecraft:ice_spikes",
    "minecraft:jungle",
    "minecraft:lukewarm_ocean",
    "minecraft:mangrove_swamp",
    "minecraft:mushroom_fields",
    "minecraft:ocean",
    "minecraft:old_growth_birch_forest",
    "minecraft:old_growth_pine_taiga",
    "minecraft:old_growth_spruce_taiga",
    "minecraft:pale_garden",
    "minecraft:plains",
    "minecraft:river",
    "minecraft:savanna",
    "minecraft:savanna_plateau",
    "minecraft:snowy_beach",
    "minecraft:snowy_plains",
    "minecraft:snowy_taiga",
    "minecraft:sparse_jungle",
    "minecraft:stony_shore",
    "minecraft:sunflower_plains",
    "minecraft:taiga",
    "minecraft:warm_ocean",
    "minecraft:windswept_forest",
    "minecraft:windswept_gravelly_hills",
    "minecraft:windswept_hills",
    "minecraft:windswept_savanna",
    "minecraft:wooded_badlands",
];
const SPRING_WATER_VALID_BLOCKS: &[&str] = &[
    "minecraft:stone",
    "minecraft:granite",
    "minecraft:diorite",
    "minecraft:andesite",
    "minecraft:deepslate",
    "minecraft:tuff",
    "minecraft:calcite",
    "minecraft:dirt",
    "minecraft:snow_block",
    "minecraft:powder_snow",
    "minecraft:packed_ice",
];
const SPRING_LAVA_VALID_BLOCKS: &[&str] = &[
    "minecraft:stone",
    "minecraft:granite",
    "minecraft:diorite",
    "minecraft:andesite",
    "minecraft:deepslate",
    "minecraft:tuff",
    "minecraft:calcite",
    "minecraft:dirt",
];
const SPRING_FROZEN_LAVA_VALID_BLOCKS: &[&str] = &[
    "minecraft:snow_block",
    "minecraft:powder_snow",
    "minecraft:packed_ice",
];
const GLOW_LICHEN_CAN_BE_PLACED_ON: &[&str] = &[
    "minecraft:stone",
    "minecraft:andesite",
    "minecraft:diorite",
    "minecraft:granite",
    "minecraft:dripstone_block",
    "minecraft:calcite",
    "minecraft:tuff",
    "minecraft:deepslate",
];
const LUSH_CAVES_BIOMES: &[&str] = &["minecraft:lush_caves"];
const SUPPORTS_VEGETATION_BLOCKS: &[&str] = &[
    "minecraft:dirt",
    "minecraft:coarse_dirt",
    "minecraft:rooted_dirt",
    "minecraft:mud",
    "minecraft:muddy_mangrove_roots",
    "minecraft:moss_block",
    "minecraft:pale_moss_block",
    "minecraft:grass_block",
    "minecraft:podzol",
    "minecraft:mycelium",
    "minecraft:farmland",
];
const CHEST_BLOCK_ENTITY_TYPE_ID: i32 = 1;
const MOB_SPAWNER_BLOCK_ENTITY_TYPE_ID: i32 = 9;
const BEEHIVE_BLOCK_ENTITY_TYPE_ID: i32 = 34;
const BRUSHABLE_BLOCK_ENTITY_TYPE_ID: i32 = 37;

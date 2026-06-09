use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct EnchantingConfig {
    #[serde(default = "default_enchanting_enable")]
    pub enable: bool,

    #[serde(default = "default_enchanting_title")]
    pub title: String,

    #[serde(default = "default_enchanting_lapis_item")]
    pub lapis_item: String,

    #[serde(default = "default_enchanting_creative_free")]
    pub creative_free: bool,

    #[serde(default = "default_enchanting_allow_reenchanting")]
    pub allow_reenchanting: bool,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<EnchantingOptionConfig>,
}

impl Default for EnchantingConfig {
    fn default() -> Self {
        Self {
            enable: default_enchanting_enable(),
            title: default_enchanting_title(),
            lapis_item: default_enchanting_lapis_item(),
            creative_free: default_enchanting_creative_free(),
            allow_reenchanting: default_enchanting_allow_reenchanting(),
            options: default_enchanting_options(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct EnchantingOptionConfig {
    #[serde(default)]
    pub id: String,

    #[serde(default)]
    pub display_name: String,

    #[serde(default = "default_enchanting_option_max_level")]
    pub max_level: i32,

    #[serde(default = "default_enchanting_option_weight")]
    pub weight: i32,

    #[serde(default)]
    pub min_player_level: i32,

    #[serde(default = "default_enchanting_option_max_player_level")]
    pub max_player_level: i32,

    #[serde(default)]
    pub min_bookshelves: i32,

    #[serde(default = "default_enchanting_option_lapis_cost")]
    pub lapis_cost: i32,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub item_suffixes: Vec<String>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub items: Vec<String>,

    #[serde(default)]
    pub plugin: bool,
}

impl Default for EnchantingOptionConfig {
    fn default() -> Self {
        Self {
            id: String::new(),
            display_name: String::new(),
            max_level: default_enchanting_option_max_level(),
            weight: default_enchanting_option_weight(),
            min_player_level: 0,
            max_player_level: default_enchanting_option_max_player_level(),
            min_bookshelves: 0,
            lapis_cost: default_enchanting_option_lapis_cost(),
            item_suffixes: Vec::new(),
            items: Vec::new(),
            plugin: false,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct QexedEnchanting {
    pub enchanting: EnchantingConfig,
}

impl Default for QexedEnchanting {
    fn default() -> Self {
        Self {
            enchanting: EnchantingConfig::default(),
        }
    }
}

impl qexed_config::tool::AppConfigTrait for QexedEnchanting {
    const PATH: &'static str = "/";
    const NAME: &'static str = "qexed_enchanting";
}

fn default_enchanting_enable() -> bool {
    true
}

fn default_enchanting_title() -> String {
    "Enchanting".to_string()
}

fn default_enchanting_lapis_item() -> String {
    "minecraft:lapis_lazuli".to_string()
}

fn default_enchanting_creative_free() -> bool {
    true
}

fn default_enchanting_allow_reenchanting() -> bool {
    false
}

fn default_enchanting_option_max_level() -> i32 {
    1
}

fn default_enchanting_option_weight() -> i32 {
    10
}

fn default_enchanting_option_max_player_level() -> i32 {
    i32::MAX
}

fn default_enchanting_option_lapis_cost() -> i32 {
    1
}

fn default_enchanting_options() -> Vec<EnchantingOptionConfig> {
    vec![
        option(
            "minecraft:efficiency",
            5,
            10,
            1,
            &["_pickaxe", "_axe", "_shovel", "_hoe"],
            &["minecraft:shears"],
            0,
        ),
        option(
            "minecraft:fortune",
            3,
            2,
            1,
            &["_pickaxe", "_axe", "_shovel", "_hoe"],
            &[],
            0,
        ),
        option(
            "minecraft:silk_touch",
            1,
            1,
            1,
            &["_pickaxe", "_axe", "_shovel", "_hoe"],
            &["minecraft:shears"],
            8,
        ),
        option("minecraft:sharpness", 5, 10, 1, &["_sword", "_axe"], &[], 0),
        option(
            "minecraft:smite",
            5,
            5,
            1,
            &["_sword", "_axe"],
            &["minecraft:mace"],
            0,
        ),
        option(
            "minecraft:bane_of_arthropods",
            5,
            5,
            1,
            &["_sword", "_axe"],
            &["minecraft:mace"],
            0,
        ),
        option("minecraft:knockback", 2, 5, 1, &["_sword"], &[], 0),
        option(
            "minecraft:fire_aspect",
            2,
            2,
            1,
            &["_sword"],
            &["minecraft:mace"],
            0,
        ),
        option("minecraft:looting", 3, 2, 1, &["_sword"], &[], 0),
        option("minecraft:sweeping_edge", 3, 2, 1, &["_sword"], &[], 0),
        option("minecraft:density", 5, 10, 1, &[], &["minecraft:mace"], 0),
        option("minecraft:breach", 4, 2, 1, &[], &["minecraft:mace"], 0),
        option("minecraft:lunge", 3, 5, 1, &["_spear"], &[], 0),
        option(
            "minecraft:protection",
            4,
            10,
            1,
            &["_helmet", "_chestplate", "_leggings", "_boots"],
            &[],
            0,
        ),
        option(
            "minecraft:fire_protection",
            4,
            5,
            1,
            &["_helmet", "_chestplate", "_leggings", "_boots"],
            &[],
            0,
        ),
        option("minecraft:feather_falling", 4, 5, 1, &["_boots"], &[], 0),
        option(
            "minecraft:blast_protection",
            4,
            2,
            1,
            &["_helmet", "_chestplate", "_leggings", "_boots"],
            &[],
            0,
        ),
        option(
            "minecraft:projectile_protection",
            4,
            5,
            1,
            &["_helmet", "_chestplate", "_leggings", "_boots"],
            &[],
            0,
        ),
        option("minecraft:respiration", 3, 2, 1, &["_helmet"], &[], 0),
        option("minecraft:aqua_affinity", 1, 2, 1, &["_helmet"], &[], 0),
        option(
            "minecraft:thorns",
            3,
            1,
            1,
            &["_helmet", "_chestplate", "_leggings", "_boots"],
            &[],
            8,
        ),
        option("minecraft:depth_strider", 3, 2, 1, &["_boots"], &[], 0),
        option("minecraft:power", 5, 10, 1, &[], &["minecraft:bow"], 0),
        option("minecraft:punch", 2, 2, 1, &[], &["minecraft:bow"], 0),
        option("minecraft:flame", 1, 2, 1, &[], &["minecraft:bow"], 0),
        option("minecraft:infinity", 1, 1, 1, &[], &["minecraft:bow"], 8),
        option(
            "minecraft:luck_of_the_sea",
            3,
            2,
            1,
            &[],
            &["minecraft:fishing_rod"],
            0,
        ),
        option(
            "minecraft:lure",
            3,
            2,
            1,
            &[],
            &["minecraft:fishing_rod"],
            0,
        ),
        option("minecraft:loyalty", 3, 5, 1, &[], &["minecraft:trident"], 0),
        option(
            "minecraft:impaling",
            5,
            2,
            1,
            &[],
            &["minecraft:trident"],
            0,
        ),
        option("minecraft:riptide", 3, 2, 1, &[], &["minecraft:trident"], 8),
        option(
            "minecraft:channeling",
            1,
            1,
            1,
            &[],
            &["minecraft:trident"],
            8,
        ),
        option(
            "minecraft:piercing",
            4,
            10,
            1,
            &[],
            &["minecraft:crossbow"],
            0,
        ),
        option(
            "minecraft:quick_charge",
            3,
            5,
            1,
            &[],
            &["minecraft:crossbow"],
            0,
        ),
        option(
            "minecraft:multishot",
            1,
            2,
            1,
            &[],
            &["minecraft:crossbow"],
            8,
        ),
        option(
            "minecraft:unbreaking",
            3,
            5,
            1,
            &[
                "_helmet",
                "_chestplate",
                "_leggings",
                "_boots",
                "_sword",
                "_pickaxe",
                "_axe",
                "_shovel",
                "_hoe",
            ],
            &[
                "minecraft:bow",
                "minecraft:crossbow",
                "minecraft:trident",
                "minecraft:mace",
                "minecraft:fishing_rod",
                "minecraft:shears",
            ],
            0,
        ),
    ]
}

fn option(
    id: &str,
    max_level: i32,
    weight: i32,
    min_player_level: i32,
    suffixes: &[&str],
    items: &[&str],
    min_bookshelves: i32,
) -> EnchantingOptionConfig {
    EnchantingOptionConfig {
        id: id.to_string(),
        max_level,
        weight,
        min_player_level,
        min_bookshelves,
        item_suffixes: suffixes.iter().map(|value| (*value).to_string()).collect(),
        items: items.iter().map(|value| (*value).to_string()).collect(),
        ..EnchantingOptionConfig::default()
    }
}

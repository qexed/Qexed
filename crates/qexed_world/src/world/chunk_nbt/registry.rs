//! 方块/生物群系注册表：v4 `world/chunk_nbt/registry.rs` 的 v6 迁移（完整版）。
//!
//! 数据源适配：v4 经 `crate::registry_sync`（workspace 根 assets/）加载；v6 改用
//! `qexed_mojang_data::registry_sync`（同一份 blocks.json / registries.json /
//! worldgen biome 目录）。generator 的 nbt_layers.rs / noise_settings.rs 经
//! generator/mod.rs 中的 `block_registry` 别名复用本文件（world-features 任务
//! 期间的 generator/block_registry.rs 子集已并入此处，原文件删除）。
//!
//! i18n：fallback/unknown 警告文案走 `qexed_language::t("qexed.world.registry.*")`
//!（world-features 任务已写入 locales）。

use std::{collections::HashMap, sync::OnceLock};

use qexed_mojang_data::registry_sync::{
    load_blocks_report, load_dynamic_registry_id_map, load_registry_id_map,
};

use crate::world::AIR_BLOCK_STATE_ID;

pub(in crate::world) fn block_state_registry() -> &'static BlockStateRegistry {
    static REGISTRY: OnceLock<BlockStateRegistry> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        load_block_state_registry().unwrap_or_else(|err| {
            log::warn!(
                "{}",
                qexed_language::t("qexed.world.registry.block_state_fallback")
                    .replace("%{error}", &format!("{err:#}"))
            );
            BlockStateRegistry::fallback()
        })
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct BlockStateDefinition {
    pub id: i32,
    pub properties: Vec<(String, String)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct BlockStateEntry {
    pub name: String,
    pub properties: Vec<(String, String)>,
}

pub(crate) fn default_block_state(name: &str) -> BlockStateDefinition {
    let name = normalize_identifier(name);
    block_state_registry()
        .default_state_by_name
        .get(&name)
        .cloned()
        .unwrap_or_else(|| {
            log::warn!(
                "{}",
                qexed_language::t("qexed.world.registry.unknown_default_state")
                    .replace("%{block}", &name)
            );
            BlockStateDefinition {
                id: AIR_BLOCK_STATE_ID,
                properties: Vec::new(),
            }
        })
}

pub(crate) fn block_state(
    name: &str,
    properties: &[(String, String)],
) -> BlockStateDefinition {
    let name = normalize_identifier(name);
    let key = state_key(&name, properties);
    block_state_registry()
        .id_by_state
        .get(&key)
        .map(|id| BlockStateDefinition {
            id: *id,
            properties: properties.to_vec(),
        })
        .unwrap_or_else(|| {
            log::warn!(
                "{}",
                qexed_language::t("qexed.world.registry.unknown_state").replace("%{key}", &key)
            );
            default_block_state(&name)
        })
}

pub(crate) fn block_state_entry(id: i32) -> BlockStateEntry {
    block_state_registry()
        .state_by_id
        .get(&id)
        .cloned()
        .unwrap_or_else(|| {
            log::warn!(
                "{}",
                qexed_language::t("qexed.world.registry.unknown_state_id")
                    .replace("%{id}", &id.to_string())
            );
            BlockStateEntry {
                name: "minecraft:air".to_string(),
                properties: Vec::new(),
            }
        })
}

pub(crate) fn default_block_state_id(name: &str) -> i32 {
    default_block_state(name).id
}

pub(crate) fn default_block_state_id_if_known(name: &str) -> Option<i32> {
    let name = normalize_identifier(name);
    block_state_registry()
        .default_state_by_name
        .get(&name)
        .map(|state| state.id)
}

pub(in crate::world) fn biome_registry() -> &'static BiomeRegistry {
    static REGISTRY: OnceLock<BiomeRegistry> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        load_biome_registry().unwrap_or_else(|err| {
            log::warn!(
                "{}",
                qexed_language::t("qexed.world.registry.biome_fallback")
                    .replace("%{error}", &format!("{err:#}"))
            );
            BiomeRegistry::fallback()
        })
    })
}

pub(in crate::world) fn block_entity_type_id(name: &str) -> Option<i32> {
    block_entity_type_registry()
        .id_by_name
        .get(normalize_identifier(name).as_str())
        .copied()
}

fn block_entity_type_registry() -> &'static BlockEntityTypeRegistry {
    static REGISTRY: OnceLock<BlockEntityTypeRegistry> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        load_block_entity_type_registry().unwrap_or_else(|err| {
            log::warn!(
                "{}",
                qexed_language::t("qexed.world.registry.block_entity_fallback")
                    .replace("%{error}", &format!("{err:#}"))
            );
            BlockEntityTypeRegistry::fallback()
        })
    })
}

fn load_block_state_registry(
) -> Result<BlockStateRegistry, qexed_mojang_data::registry_sync::RegistryError> {
    let value = load_blocks_report()?;
    let blocks = value
        .as_object()
        .ok_or_else(|| qexed_mojang_data::registry_sync::RegistryError::msg("block report root is not object"))?;

    let mut id_by_state = HashMap::new();
    let mut state_by_id = HashMap::new();
    let mut default_state_by_name = HashMap::new();
    let mut metadata_by_name = HashMap::new();
    let mut max_id = AIR_BLOCK_STATE_ID;
    for (name, block) in blocks {
        let block_type = block
            .get("definition")
            .and_then(|definition| definition.get("type"))
            .and_then(serde_json::Value::as_str)
            .map(normalize_identifier)
            .unwrap_or_else(|| "minecraft:block".to_string());
        metadata_by_name.insert(name.clone(), BlockMetadata { block_type });

        let Some(states) = block.get("states").and_then(serde_json::Value::as_array) else {
            continue;
        };

        for state in states {
            let Some(id) = state.get("id").and_then(serde_json::Value::as_i64) else {
                continue;
            };
            let Ok(id) = i32::try_from(id) else {
                continue;
            };
            let properties = json_string_properties(state.get("properties"));
            id_by_state.insert(state_key(name, &properties), id);
            state_by_id.insert(
                id,
                BlockStateEntry {
                    name: name.clone(),
                    properties: properties.clone(),
                },
            );
            if state
                .get("default")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false)
            {
                default_state_by_name.insert(
                    name.clone(),
                    BlockStateDefinition {
                        id,
                        properties: properties.clone(),
                    },
                );
            }
            max_id = max_id.max(id);
        }

        if !default_state_by_name.contains_key(name)
            && let Some(state) = states.first()
            && let Some(id) = state.get("id").and_then(serde_json::Value::as_i64)
            && let Ok(id) = i32::try_from(id)
        {
            default_state_by_name.insert(
                name.clone(),
                BlockStateDefinition {
                    id,
                    properties: json_string_properties(state.get("properties")),
                },
            );
        }
    }

    if id_by_state.is_empty() {
        return Err(qexed_mojang_data::registry_sync::RegistryError::msg(
            "block report contains no block states",
        ));
    }

    Ok(BlockStateRegistry {
        id_by_state,
        state_by_id,
        default_state_by_name,
        metadata_by_name,
        global_bits: ceil_log2((max_id as usize) + 1).max(1),
    })
}

fn load_block_entity_type_registry(
) -> Result<BlockEntityTypeRegistry, qexed_mojang_data::registry_sync::RegistryError> {
    let id_by_name = load_registry_id_map("minecraft:block_entity_type")?
        .into_iter()
        .map(|(name, protocol_id)| (normalize_identifier(&name), protocol_id))
        .collect::<HashMap<_, _>>();

    if id_by_name.is_empty() {
        return Err(qexed_mojang_data::registry_sync::RegistryError::msg(
            "block entity type registry contains no entries",
        ));
    }

    Ok(BlockEntityTypeRegistry { id_by_name })
}

fn load_biome_registry(
) -> Result<BiomeRegistry, qexed_mojang_data::registry_sync::RegistryError> {
    // v4 从 assets/decompiled_source/.../worldgen/biome 目录取动态注册表；
    // v6 load_dynamic_registry_id_map 在给定目录不存在时自动回落到
    // qexed_mojang_data 的 data_roots，传空 Path 即可。
    let id_by_name = load_dynamic_registry_id_map(std::path::Path::new(""), "worldgen/biome")?;

    if id_by_name.is_empty() {
        return Err(qexed_mojang_data::registry_sync::RegistryError::msg(
            "biome registry contains no entries",
        ));
    }

    Ok(BiomeRegistry {
        global_bits: ceil_log2(id_by_name.len()).max(1),
        id_by_name,
    })
}

fn json_string_properties(value: Option<&serde_json::Value>) -> Vec<(String, String)> {
    let Some(properties) = value.and_then(serde_json::Value::as_object) else {
        return Vec::new();
    };

    let mut properties = properties
        .iter()
        .filter_map(|(key, value)| {
            value
                .as_str()
                .map(|value| (key.to_string(), value.to_string()))
        })
        .collect::<Vec<_>>();
    properties.sort_by(|left, right| left.0.cmp(&right.0));
    properties
}

pub(in crate::world) fn state_key(name: &str, properties: &[(String, String)]) -> String {
    let mut key = normalize_identifier(name);
    key.push('|');
    for (name, value) in properties {
        key.push_str(name);
        key.push('=');
        key.push_str(value);
        key.push(';');
    }
    key
}

pub(in crate::world) fn normalize_identifier(value: &str) -> String {
    if value.contains(':') {
        value.to_string()
    } else {
        format!("minecraft:{value}")
    }
}

pub(in crate::world) fn is_air_block(name: &str) -> bool {
    matches!(
        name,
        "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air"
    )
}

pub(in crate::world) fn has_fluid(name: &str, properties: &[(String, String)]) -> bool {
    name == "minecraft:water"
        || name == "minecraft:lava"
        || is_always_water_filled_block(name)
        || properties
            .iter()
            .any(|(key, value)| key == "waterlogged" && value == "true")
}

fn is_always_water_filled_block(name: &str) -> bool {
    matches!(
        name,
        "minecraft:bubble_column"
            | "minecraft:kelp"
            | "minecraft:kelp_plant"
            | "minecraft:seagrass"
            | "minecraft:tall_seagrass"
    )
}

pub(in crate::world) fn light_dampening(name: &str, block_type: Option<&str>, has_fluid: bool) -> u8 {
    if has_fluid || is_one_light_dampening_block_type(block_type) {
        1
    } else if is_air_block(name) || is_zero_light_dampening_block_type(block_type, name) {
        0
    } else {
        15
    }
}

fn is_one_light_dampening_block_type(block_type: Option<&str>) -> bool {
    matches!(
        block_type,
        Some(
            "minecraft:liquid"
                | "minecraft:mangrove_leaves"
                | "minecraft:tinted_particle_leaves"
                | "minecraft:untinted_particle_leaves"
        )
    )
}

fn is_zero_light_dampening_block_type(block_type: Option<&str>, name: &str) -> bool {
    matches!(
        block_type,
        Some(
            "minecraft:air"
                | "minecraft:barrier"
                | "minecraft:bamboo_sapling"
                | "minecraft:bamboo_stalk"
                | "minecraft:big_dripleaf"
                | "minecraft:big_dripleaf_stem"
                | "minecraft:button"
                | "minecraft:cave_vines"
                | "minecraft:cave_vines_plant"
                | "minecraft:cross_collision"
                | "minecraft:door"
                | "minecraft:end_portal"
                | "minecraft:fence"
                | "minecraft:fence_gate"
                | "minecraft:fire"
                | "minecraft:flower_pot"
                | "minecraft:glow_lichen"
                | "minecraft:hanging_moss"
                | "minecraft:iron_bars"
                | "minecraft:ladder"
                | "minecraft:light"
                | "minecraft:mossy_carpet"
                | "minecraft:nether_sprouts"
                | "minecraft:pressure_plate"
                | "minecraft:seagrass"
                | "minecraft:sea_pickle"
                | "minecraft:short_dry_grass"
                | "minecraft:small_dripleaf"
                | "minecraft:snow_layer"
                | "minecraft:tall_dry_grass"
                | "minecraft:tall_grass"
                | "minecraft:torch"
                | "minecraft:transparent"
                | "minecraft:trapdoor"
                | "minecraft:twisting_vines"
                | "minecraft:twisting_vines_plant"
                | "minecraft:vine"
                | "minecraft:void"
                | "minecraft:wall_banner"
                | "minecraft:wall_hanging_sign"
                | "minecraft:wall_sign"
                | "minecraft:wall_skull"
                | "minecraft:wall_torch"
                | "minecraft:weeping_vines"
                | "minecraft:weeping_vines_plant"
        )
    ) || matches!(
        name,
        "minecraft:structure_void"
            | "minecraft:glass"
            | "minecraft:ice"
            | "minecraft:packed_ice"
            | "minecraft:blue_ice"
    )
}

pub(in crate::world) struct BlockStateRegistry {
    pub(in crate::world) id_by_state: HashMap<String, i32>,
    pub(in crate::world) state_by_id: HashMap<i32, BlockStateEntry>,
    pub(in crate::world) default_state_by_name: HashMap<String, BlockStateDefinition>,
    pub(in crate::world) metadata_by_name: HashMap<String, BlockMetadata>,
    pub(in crate::world) global_bits: usize,
}

pub(in crate::world) struct BlockMetadata {
    pub(in crate::world) block_type: String,
}

struct BlockEntityTypeRegistry {
    id_by_name: HashMap<String, i32>,
}

impl BlockEntityTypeRegistry {
    fn fallback() -> Self {
        Self {
            id_by_name: HashMap::from([
                ("minecraft:furnace".to_string(), 0),
                ("minecraft:chest".to_string(), 1),
                ("minecraft:trapped_chest".to_string(), 2),
                ("minecraft:ender_chest".to_string(), 3),
                ("minecraft:mob_spawner".to_string(), 9),
                ("minecraft:beehive".to_string(), 34),
                ("minecraft:brushable_block".to_string(), 41),
            ]),
        }
    }
}

impl BlockStateRegistry {
    fn fallback() -> Self {
        let mut id_by_state = HashMap::new();
        id_by_state.insert("minecraft:air|".to_string(), AIR_BLOCK_STATE_ID);
        id_by_state.insert("minecraft:stone|".to_string(), 1);
        id_by_state.insert("minecraft:water|level=0;".to_string(), 86);
        id_by_state.insert("minecraft:lava|level=0;".to_string(), 102);
        let mut state_by_id = HashMap::new();
        state_by_id.insert(
            AIR_BLOCK_STATE_ID,
            BlockStateEntry {
                name: "minecraft:air".to_string(),
                properties: Vec::new(),
            },
        );
        state_by_id.insert(
            1,
            BlockStateEntry {
                name: "minecraft:stone".to_string(),
                properties: Vec::new(),
            },
        );
        state_by_id.insert(
            86,
            BlockStateEntry {
                name: "minecraft:water".to_string(),
                properties: vec![("level".to_string(), "0".to_string())],
            },
        );
        state_by_id.insert(
            102,
            BlockStateEntry {
                name: "minecraft:lava".to_string(),
                properties: vec![("level".to_string(), "0".to_string())],
            },
        );
        let mut default_state_by_name = HashMap::new();
        default_state_by_name.insert(
            "minecraft:air".to_string(),
            BlockStateDefinition {
                id: AIR_BLOCK_STATE_ID,
                properties: Vec::new(),
            },
        );
        default_state_by_name.insert(
            "minecraft:stone".to_string(),
            BlockStateDefinition {
                id: 1,
                properties: Vec::new(),
            },
        );
        default_state_by_name.insert(
            "minecraft:water".to_string(),
            BlockStateDefinition {
                id: 86,
                properties: vec![("level".to_string(), "0".to_string())],
            },
        );
        default_state_by_name.insert(
            "minecraft:lava".to_string(),
            BlockStateDefinition {
                id: 102,
                properties: vec![("level".to_string(), "0".to_string())],
            },
        );
        let mut metadata_by_name = HashMap::new();
        metadata_by_name.insert(
            "minecraft:air".to_string(),
            BlockMetadata {
                block_type: "minecraft:air".to_string(),
            },
        );
        metadata_by_name.insert(
            "minecraft:stone".to_string(),
            BlockMetadata {
                block_type: "minecraft:block".to_string(),
            },
        );
        metadata_by_name.insert(
            "minecraft:water".to_string(),
            BlockMetadata {
                block_type: "minecraft:liquid".to_string(),
            },
        );
        metadata_by_name.insert(
            "minecraft:lava".to_string(),
            BlockMetadata {
                block_type: "minecraft:liquid".to_string(),
            },
        );
        Self {
            id_by_state,
            state_by_id,
            default_state_by_name,
            metadata_by_name,
            global_bits: 14,
        }
    }
}

pub(in crate::world) struct BiomeRegistry {
    pub(in crate::world) id_by_name: HashMap<String, i32>,
    pub(in crate::world) global_bits: usize,
}

impl BiomeRegistry {
    fn fallback() -> Self {
        let mut id_by_name = HashMap::new();
        id_by_name.insert("minecraft:plains".to_string(), crate::world::PLAINS_BIOME_ID);
        Self {
            id_by_name,
            global_bits: 6,
        }
    }
}

fn ceil_log2(count: usize) -> usize {
    if count <= 1 {
        0
    } else {
        usize::BITS as usize - (count - 1).leading_zeros() as usize
    }
}

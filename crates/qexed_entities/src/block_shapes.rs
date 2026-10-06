//! 方块碰撞箱/名称数据源（v4 inventory 域 block_item_registry 的实体域投影）。
//!
//! 数据源：assets/reports/blocks.json（qexed_mojang_data::registry_sync，
//! OnceLock 缓存，进程内只加载一次）。v6 报告不含 definition 节点，
//! 因此按 v4 definition.type 的成员方块名做无碰撞分类；
//! carpet/snow/slab/trapdoor 的精确高度沿用 v4 collision_shape_for_block。

use std::collections::{HashMap, HashSet};

use crate::context::{BlockCollisionShape, BlockShapeSource};

use qexed_mojang_data::registry_sync::load_blocks_report;

const AIR_BLOCK_STATE_ID: i32 = 0;

/// 无碰撞方块名家族（v4 definition.type 投影；与 qexed_world 语义表同源）。
const NO_COLLISION_BLOCK_NAMES: &[&str] = &[
    "minecraft:air",
    "minecraft:cave_air",
    "minecraft:void_air",
    "minecraft:water",
    "minecraft:lava",
    "minecraft:fire",
    "minecraft:soul_fire",
    "minecraft:short_grass",
    "minecraft:tall_grass",
    "minecraft:fern",
    "minecraft:large_fern",
    "minecraft:dead_bush",
    "minecraft:bush",
    "minecraft:firefly_bush",
    "minecraft:short_dry_grass",
    "minecraft:tall_dry_grass",
    "minecraft:allium",
    "minecraft:azure_bluet",
    "minecraft:blue_orchid",
    "minecraft:cornflower",
    "minecraft:dandelion",
    "minecraft:golden_dandelion",
    "minecraft:lilac",
    "minecraft:lily_of_the_valley",
    "minecraft:orange_tulip",
    "minecraft:oxeye_daisy",
    "minecraft:peony",
    "minecraft:pink_tulip",
    "minecraft:poppy",
    "minecraft:red_tulip",
    "minecraft:rose_bush",
    "minecraft:sunflower",
    "minecraft:torchflower",
    "minecraft:torchflower_crop",
    "minecraft:white_tulip",
    "minecraft:wildflowers",
    "minecraft:closed_eyeblossom",
    "minecraft:open_eyeblossom",
    "minecraft:pink_petals",
    "minecraft:leaf_litter",
    "minecraft:vine",
    "minecraft:cave_vines",
    "minecraft:cave_vines_plant",
    "minecraft:twisting_vines",
    "minecraft:twisting_vines_plant",
    "minecraft:weeping_vines",
    "minecraft:weeping_vines_plant",
    "minecraft:kelp",
    "minecraft:kelp_plant",
    "minecraft:seagrass",
    "minecraft:tall_seagrass",
];

/// 报告驱动的方块形状表（OnceLock 缓存）。
struct BlockShapeTable {
    /// 有碰撞的状态集合（无碰撞家族之外的全部已知状态）。
    collision_block_states: HashSet<i32>,
    /// 空气状态集合。
    air_block_states: HashSet<i32>,
    /// 已知状态集合（未知状态保守视为整块碰撞）。
    known_block_states: HashSet<i32>,
    /// 状态 → 方块名。
    block_name_by_state: HashMap<i32, String>,
    /// 状态 → 属性表。
    properties_by_state: HashMap<i32, HashMap<String, String>>,
}

fn block_shape_table() -> &'static BlockShapeTable {
    static TABLE: std::sync::OnceLock<BlockShapeTable> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| match load_block_shape_table() {
        Ok(table) => table,
        Err(err) => {
            log::warn!(
                "{}",
                qexed_language::t("qexed.entities.block_shapes_fallback")
                    .replace("%{error}", &format!("{err:#}"))
            );
            BlockShapeTable::fallback()
        }
    })
}

fn load_block_shape_table(
) -> Result<BlockShapeTable, qexed_mojang_data::registry_sync::RegistryError> {
    let value = load_blocks_report()?;
    let blocks = value
        .as_object()
        .ok_or_else(|| qexed_mojang_data::registry_sync::RegistryError::msg("block report root is not object"))?;

    let mut collision_block_states = HashSet::new();
    let mut air_block_states = HashSet::new();
    let mut known_block_states = HashSet::new();
    let mut block_name_by_state = HashMap::new();
    let mut properties_by_state = HashMap::new();

    for (name, block) in blocks {
        let is_air = matches!(
            name.as_str(),
            "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air"
        );
        let no_collision = NO_COLLISION_BLOCK_NAMES.contains(&name.as_str());
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
            known_block_states.insert(id);
            block_name_by_state.insert(id, name.clone());
            properties_by_state.insert(id, state_properties(state));
            if is_air {
                air_block_states.insert(id);
            }
            if !no_collision {
                collision_block_states.insert(id);
            }
        }
    }

    if known_block_states.is_empty() {
        return Err(qexed_mojang_data::registry_sync::RegistryError::msg(
            "block report contains no block states",
        ));
    }

    Ok(BlockShapeTable {
        collision_block_states,
        air_block_states,
        known_block_states,
        block_name_by_state,
        properties_by_state,
    })
}

impl BlockShapeTable {
    fn fallback() -> Self {
        Self {
            collision_block_states: HashSet::from([1]),
            air_block_states: HashSet::from([AIR_BLOCK_STATE_ID]),
            known_block_states: HashSet::from([AIR_BLOCK_STATE_ID, 1]),
            block_name_by_state: HashMap::from([(1, "minecraft:stone".to_string())]),
            properties_by_state: HashMap::new(),
        }
    }

    fn has_collision(&self, block_state: i32) -> bool {
        if self.air_block_states.contains(&block_state) {
            return false;
        }
        if self.known_block_states.contains(&block_state) {
            return self.collision_block_states.contains(&block_state);
        }
        true
    }

    fn shape(&self, block_state: i32) -> Option<BlockCollisionShape> {
        if !self.has_collision(block_state) {
            return None;
        }
        let name = self.block_name_by_state.get(&block_state)?;
        let properties = self.properties_by_state.get(&block_state);
        Some(collision_shape_for_block(name, properties))
    }
}

/// 状态属性表（v4 block_state_properties）。
fn state_properties(state: &serde_json::Value) -> HashMap<String, String> {
    state
        .get("properties")
        .and_then(serde_json::Value::as_object)
        .map(|properties| {
            properties
                .iter()
                .filter_map(|(key, value)| {
                    value.as_str().map(|value| (key.clone(), value.to_string()))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// 精确碰撞高度（v4 collision_shape_for_block 原样）。
fn collision_shape_for_block(
    name: &str,
    properties: Option<&HashMap<String, String>>,
) -> BlockCollisionShape {
    const FULL_BLOCK: BlockCollisionShape = BlockCollisionShape {
        min_x: 0.0,
        min_y: 0.0,
        min_z: 0.0,
        max_x: 1.0,
        max_y: 1.0,
        max_z: 1.0,
    };
    if name.ends_with("_carpet") || name == "minecraft:carpet" {
        return BlockCollisionShape {
            max_y: 1.0 / 16.0,
            ..FULL_BLOCK
        };
    }
    if name == "minecraft:snow" || name == "minecraft:snow_layer" {
        let layers = properties
            .and_then(|properties| properties.get("layers"))
            .and_then(|layers| layers.parse::<u8>().ok())
            .unwrap_or(1)
            .clamp(1, 8);
        return BlockCollisionShape {
            max_y: f64::from(layers) / 8.0,
            ..FULL_BLOCK
        };
    }
    if name.ends_with("_slab") {
        return match properties
            .and_then(|properties| properties.get("type"))
            .map(String::as_str)
        {
            Some("top") => BlockCollisionShape {
                min_y: 0.5,
                ..FULL_BLOCK
            },
            Some("double") => FULL_BLOCK,
            _ => BlockCollisionShape {
                max_y: 0.5,
                ..FULL_BLOCK
            },
        };
    }
    if name.ends_with("_trapdoor") {
        return match properties
            .and_then(|properties| properties.get("open"))
            .map(String::as_str)
        {
            Some("true") => FULL_BLOCK,
            _ => match properties
                .and_then(|properties| properties.get("half"))
                .map(String::as_str)
            {
                Some("top") => BlockCollisionShape {
                    min_y: 13.0 / 16.0,
                    ..FULL_BLOCK
                },
                _ => BlockCollisionShape {
                    max_y: 3.0 / 16.0,
                    ..FULL_BLOCK
                },
            },
        };
    }
    FULL_BLOCK
}

/// 报告驱动的方块形状源（server 域组装时经
/// install_block_shape_source 装入，或直接作为默认源使用）。
pub struct ReportBlockShapes;

impl BlockShapeSource for ReportBlockShapes {
    fn block_collision_shape(&self, block_state: i32) -> Option<BlockCollisionShape> {
        block_shape_table().shape(block_state)
    }

    fn block_name_for_state(&self, block_state: i32) -> Option<String> {
        block_shape_table().block_name_by_state.get(&block_state).cloned()
    }

    fn is_air_block_state(&self, block_state: i32) -> bool {
        block_shape_table().air_block_states.contains(&block_state)
    }

    fn air_block_state(&self) -> i32 {
        AIR_BLOCK_STATE_ID
    }
}

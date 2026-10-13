//! 方块状态索引与光学/流体属性。
//!
//! 数据源：Mojang 报告 `reports/blocks.json`（`qexed_registry::load_blocks_report`），
//! 形状为 `{ "<block name>": { definition, properties?, states: [{ id, properties?, default? }] } }`。
//! 索引键 = 方块名 + 排序后的属性后缀 `[k=v,...]`，与 anvil `BlockStateRef` 的
//! 属性排序规则一致。
//!
//! 网络调色板位宽规则（MC 26.1.2，对齐 qexed_anvil::palette 的实排规则）：
//! - Block：palette 0 项 → 单值(0,false)；entry_bits 1..=4 → (4,false)；
//!   5..=8 → (entry_bits,false)；否则 (max(global_bits, entry_bits), true)。
//! - storage_bits：palette 1 项 → 4；否则 entry_bits（单值 palette 无数据数组）。

use std::collections::HashMap;

use qexed_light::data::BlockLightProps;
use qexed_registry::load_blocks_report;

use crate::error::{Result, WorldError};

/// 空气系方块（不挡光、不计非空气数）。
pub fn is_air_block(name: &str) -> bool {
    matches!(
        name,
        "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air"
    )
}

/// 是否含液体（section 头第二个 i16 的计数口径）。
///
/// 直接含水/岩浆的方块 + 始终含水的水生植物（bubble_column / kelp /
/// kelp_plant / seagrass / tall_seagrass），以及任意 waterlogged=true 方块。
pub fn has_fluid(name: &str, properties: &[(String, String)]) -> bool {
    if matches!(
        name,
        "minecraft:water"
            | "minecraft:lava"
            | "minecraft:bubble_column"
            | "minecraft:kelp"
            | "minecraft:kelp_plant"
            | "minecraft:seagrass"
            | "minecraft:tall_seagrass"
    ) {
        return true;
    }
    properties
        .iter()
        .any(|(k, v)| k == "waterlogged" && v == "true")
}

/// 26.1.2 方块光学属性（opacity = light_block，luminance = light_emission）。
///
/// 表移植自 qexed_light vanilla 对拍验证过的逐格一致版本；缺省 = 全方块（挡光 15）。
pub fn block_props(name: &str) -> BlockLightProps {
    match name {
        "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air" => BlockLightProps::AIR,

        // 亮度 15
        "minecraft:lava" => BlockLightProps {
            opacity: 15,
            luminance: 15,
        },
        // 亮度 14
        "minecraft:wall_torch" | "minecraft:torch" => BlockLightProps {
            opacity: 0,
            luminance: 14,
        },
        // 荧石地衣 7
        "minecraft:glow_lichen" => BlockLightProps {
            opacity: 0,
            luminance: 7,
        },
        // 紫水晶：簇 5 / 大芽 4 / 中芽 2 / 小芽 1（26.1.2 实测 medium=2，非 3）
        "minecraft:amethyst_cluster" => BlockLightProps {
            opacity: 0,
            luminance: 5,
        },
        "minecraft:large_amethyst_bud" => BlockLightProps {
            opacity: 0,
            luminance: 4,
        },
        "minecraft:medium_amethyst_bud" => BlockLightProps {
            opacity: 0,
            luminance: 2,
        },
        "minecraft:small_amethyst_bud" => BlockLightProps {
            opacity: 0,
            luminance: 1,
        },
        // 岩浆块 15/3；蘑菇 1
        "minecraft:magma_block" => BlockLightProps {
            opacity: 15,
            luminance: 3,
        },
        "minecraft:brown_mushroom" => BlockLightProps {
            opacity: 0,
            luminance: 1,
        },
        // 红蘑菇不发光（26.1.2 实测 0）
        "minecraft:red_mushroom" => BlockLightProps {
            opacity: 0,
            luminance: 0,
        },
        // 点亮状态的红石矿石：亮度 9（未点亮的按 SOLID 默认）
        "minecraft:redstone_ore[lit=true]" => BlockLightProps {
            opacity: 15,
            luminance: 9,
        },

        // 透光衰减 1：水系（含水生植物，水格 filter 1）、树叶、蜘蛛网
        "minecraft:water"
        | "minecraft:bubble_column"
        | "minecraft:seagrass"
        | "minecraft:tall_seagrass"
        | "minecraft:kelp"
        | "minecraft:kelp_plant" => BlockLightProps {
            opacity: 1,
            luminance: 0,
        },
        "minecraft:oak_leaves" | "minecraft:birch_leaves" | "minecraft:jungle_leaves" => {
            BlockLightProps {
                opacity: 1,
                luminance: 0,
            }
        }
        "minecraft:cobweb" => BlockLightProps {
            opacity: 1,
            luminance: 0,
        },
        // 杜鹃/开花杜鹃（类树叶 filter 1）
        "minecraft:azalea" | "minecraft:flowering_azalea" => BlockLightProps {
            opacity: 1,
            luminance: 0,
        },

        // 萤火草丛：26.1.2 实测亮度 2
        "minecraft:firefly_bush" => BlockLightProps {
            opacity: 0,
            luminance: 2,
        },
        // 非完整方块/植物：不挡光、不发光
        "minecraft:bamboo"
        | "minecraft:big_dripleaf"
        | "minecraft:big_dripleaf_stem"
        | "minecraft:small_dripleaf"
        | "minecraft:bush"
        | "minecraft:fern"
        | "minecraft:short_grass"
        | "minecraft:tall_grass"
        | "minecraft:leaf_litter"
        | "minecraft:lilac"
        | "minecraft:peony"
        | "minecraft:dandelion"
        | "minecraft:moss_carpet"
        | "minecraft:sugar_cane"
        | "minecraft:vine"
        | "minecraft:oak_fence"
        | "minecraft:rail"
        | "minecraft:iron_chain"
        | "minecraft:cocoa"
        | "minecraft:chest"
        | "minecraft:spawner" => BlockLightProps::AIR,

        // 其余全部是实心方块（石头系、泥土系、矿石、原木、西瓜、苔块、蜂巢等）
        _ => BlockLightProps::SOLID,
    }
}

/// ceil(log2(n))；n <= 1 时为 0。
pub fn ceil_log2(n: usize) -> u32 {
    if n <= 1 {
        0
    } else {
        usize::BITS - (n - 1).leading_zeros()
    }
}

/// 网络调色板参数：(bits_per_entry, 是否间接携带数据数组)。
///
/// palette 为空不会出现（缺容器时上游回退单元素）；单值 palette 无数据数组。
pub fn network_palette_bits(global_bits: u32, palette_len: usize) -> (u8, bool) {
    let entry_bits = ceil_log2(palette_len);
    match entry_bits {
        0 => (0, false),
        1..=4 => (4, false),
        5..=8 => (entry_bits as u8, false),
        _ => (global_bits.max(entry_bits) as u8, true),
    }
}

/// blocks.json 报告 → 协议 id 索引。
#[derive(Debug, Clone)]
pub struct BlockIndex {
    id_by_state: HashMap<String, i32>,
    default_state_by_name: HashMap<String, i32>,
    global_bits: u32,
}

impl BlockIndex {
    /// 从缓存的 Mojang 报告加载（未 init 时报错）。
    #[allow(dead_code)]
    pub fn load() -> Result<Self> {
        Self::from_report(&load_blocks_report()?)
    }

    /// 从已解析的报告构建索引（测试 / 自带报告路径用）。
    pub fn from_report(report: &serde_json::Value) -> Result<Self> {
        let entries = report
            .as_object()
            .ok_or_else(|| WorldError::Other("blocks.json 报告不是对象".into()))?;
        let mut id_by_state = HashMap::new();
        let mut default_state_by_name = HashMap::new();
        let mut max_id = 0_i32;

        for (name, block) in entries {
            let Some(states) = block.get("states").and_then(|v| v.as_array()) else {
                log::warn!("blocks.json: {name} 缺 states，跳过");
                continue;
            };
            for state in states {
                let Some(id) = state.get("id").and_then(|v| v.as_i64()) else {
                    return Err(WorldError::Other(format!("blocks.json: {name} state 缺 id")));
                };
                let id = i32::try_from(id)
                    .map_err(|_| WorldError::Other(format!("blocks.json: {name} id 越界")))?;
                let props = properties_of(state);
                id_by_state.insert(Self::state_key(name, &props), id);
                if state.get("default").and_then(|v| v.as_bool()) == Some(true) {
                    default_state_by_name.insert(name.clone(), id);
                }
                max_id = max_id.max(id);
            }
        }

        Ok(Self {
            global_bits: ceil_log2(max_id.max(0) as usize + 1).max(1),
            id_by_state,
            default_state_by_name,
        })
    }

    /// 索引键：名称 + 排序后的属性后缀。
    pub fn state_key(name: &str, properties: &[(String, String)]) -> String {
        if properties.is_empty() {
            return name.to_string();
        }
        let mut props: Vec<&(String, String)> = properties.iter().collect();
        props.sort_by(|a, b| a.0.cmp(&b.0));
        let inner: Vec<String> = props.iter().map(|(k, v)| format!("{k}={v}")).collect();
        format!("{name}[{}]", inner.join(","))
    }

    /// 按名称+属性取协议 id（含属性的组合必须精确命中）。
    pub fn state_id(&self, name: &str, properties: &[(String, String)]) -> Option<i32> {
        self.id_by_state
            .get(&Self::state_key(name, properties))
            .copied()
    }

    /// 方块默认状态的协议 id（报告中标 default:true 的 state）。
    pub fn default_state_id(&self, name: &str) -> Option<i32> {
        self.default_state_by_name.get(name).copied()
    }

    /// 全局 palette 位宽 = ceil_log2(最大 id + 1)。
    pub fn global_bits(&self) -> u32 {
        self.global_bits
    }
}

/// 读取 state 的 properties 对象为排序无关的 (k, v) 列表。
fn properties_of(state: &serde_json::Value) -> Vec<(String, String)> {
    state
        .get("properties")
        .and_then(|v| v.as_object())
        .map(|map| {
            map.iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_builds_from_report_shape() {
        let report = serde_json::json!({
            "minecraft:air": { "definition": {}, "states": [ { "id": 0, "default": true } ] },
            "minecraft:redstone_ore": { "definition": {}, "states": [
                { "id": 6881, "properties": { "lit": "true" } },
                { "id": 6882, "properties": { "lit": "false" }, "default": true }
            ] },
            "minecraft:stone": { "definition": {}, "states": [ { "id": 9, "default": true } ] }
        });
        let index = BlockIndex::from_report(&report).expect("build index");
        assert_eq!(index.state_id("minecraft:air", &[]), Some(0));
        assert_eq!(
            index.state_id(
                "minecraft:redstone_ore",
                &[("lit".to_string(), "true".to_string())]
            ),
            Some(6881)
        );
        // 缺省态也在索引里（显式属性组合），default 指向 lit=false。
        assert_eq!(
            index.state_id(
                "minecraft:redstone_ore",
                &[("lit".to_string(), "false".to_string())]
            ),
            Some(6882)
        );
        assert_eq!(index.default_state_id("minecraft:redstone_ore"), Some(6882));
        assert_eq!(index.default_state_id("minecraft:stone"), Some(9));
        assert_eq!(index.global_bits(), 13);
    }

    #[test]
    fn state_key_sorts_properties() {
        assert_eq!(
            BlockIndex::state_key(
                "minecraft:oak_door",
                &[
                    ("hinge".to_string(), "left".to_string()),
                    ("open".to_string(), "true".to_string()),
                ]
            ),
            "minecraft:oak_door[hinge=left,open=true]"
        );
        assert_eq!(BlockIndex::state_key("minecraft:stone", &[]), "minecraft:stone");
    }

    #[test]
    fn block_props_table_known_values() {
        assert_eq!(block_props("minecraft:air"), BlockLightProps::AIR);
        assert_eq!(block_props("minecraft:lava").luminance, 15);
        assert_eq!(block_props("minecraft:torch").luminance, 14);
        assert_eq!(
            block_props("minecraft:redstone_ore[lit=true]").luminance,
            9
        );
        assert_eq!(block_props("minecraft:redstone_ore"), BlockLightProps::SOLID);
        assert_eq!(block_props("minecraft:water").opacity, 1);
        assert_eq!(block_props("minecraft:oak_leaves").opacity, 1);
        assert_eq!(block_props("minecraft:stone"), BlockLightProps::SOLID);
    }

    #[test]
    fn fluid_detection() {
        let waterlogged = [("waterlogged".to_string(), "true".to_string())];
        assert!(has_fluid("minecraft:water", &[]));
        assert!(has_fluid("minecraft:lava", &[]));
        assert!(has_fluid("minecraft:kelp", &[]));
        assert!(has_fluid("minecraft:oak_trapdoor", &waterlogged));
        assert!(!has_fluid("minecraft:oak_trapdoor", &[]));
        assert!(!has_fluid("minecraft:stone", &[]));
    }

    #[test]
    fn palette_bits_match_network_rules() {
        assert_eq!(network_palette_bits(13, 1), (0, false));
        assert_eq!(network_palette_bits(13, 8), (4, false));
        assert_eq!(network_palette_bits(13, 9), (4, false));
        assert_eq!(network_palette_bits(13, 17), (5, false));
        assert_eq!(network_palette_bits(13, 6000), (13, true));
    }
}

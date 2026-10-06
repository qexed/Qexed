//! 世界访问辅助（play 域对 qexed_world 方块状态数据的最小依赖面）。
//!
//! v4 的 play 模块直接用 `WorldManager`（block_state_at/place_block）和
//! `world::chunk_nbt` 的方块状态注册表（block_state_entry/block_state/
//! default_block_state_id）。v6 中：
//! - WorldManager 由 world-core 任务负责、尚未导出 —— play 侧定义
//!   [`WorldBlockSource`] trait，装配层用真实 manager 实现，单测用内存实现。
//! - 方块状态注册表（qexed_world::world::chunk_nbt::registry）是 pub(crate)，
//!   play 域改为直接从 qexed_mojang_data 的 blocks.json 报告构建同一张表
//!   （v4 inventory.rs 与该 registry 本就同源，这里提供 play 域共享入口）。

use std::collections::HashMap;
use std::sync::OnceLock;

use qexed_packet::net_types::Position as BlockPosition;

/// 世界方块访问面（v4 WorldManager 的 play 子集）。
pub trait WorldBlockSource: Send + Sync {
    /// 方块状态查询（未加载返回 None）。
    fn block_state_at(&self, dimension: &str, position: &BlockPosition) -> Option<i32>;

    /// 仅缓存查询（未加载返回 None；缓存语义由实现决定，默认退化为 block_state_at）。
    fn cached_block_state_at(&self, dimension: &str, position: &BlockPosition) -> Option<i32> {
        self.block_state_at(dimension, position)
    }
}

/// 方块放置与光照更新面（v4 WorldManager 的写路径 play 子集；
/// chat 的 /structure place 与 gameplay 的红石/结构放置用）。
pub trait WorldStructureSink: Send + Sync {
    /// 批量放置方块，返回需要广播的方块更新包。
    fn place_blocks(
        &self,
        dimension: &str,
        blocks: Vec<(BlockPosition, i32)>,
    ) -> Result<
        Vec<qexed_protocol::to_client::play::block_update::BlockUpdate>,
        crate::error::PlayError,
    >;

    /// 动态光照是否启用（决定方块变更后是否补发 light_update）。
    fn dynamic_light_enabled(&self) -> bool;

    /// 计算并发送指定区块的光照更新包。
    fn light_update(
        &self,
        dimension: &str,
        chunk_x: i32,
        chunk_z: i32,
    ) -> qexed_protocol::to_client::play::light_update::LightUpdate;
}

/// 方块状态条目（v4 world::chunk_nbt::registry::BlockStateEntry 同构）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockStateEntry {
    pub name: String,
    pub properties: Vec<(String, String)>,
}

/// 空世界实现（单测用）：所有查询返回 None。
#[derive(Debug, Default)]
pub struct EmptyWorld;

impl WorldBlockSource for EmptyWorld {
    fn block_state_at(&self, _dimension: &str, _position: &BlockPosition) -> Option<i32> {
        None
    }
}

/// 内存世界实现（单测用）：显式放置的方块可查。
#[derive(Debug, Default)]
pub struct InMemoryWorld {
    blocks: std::sync::Mutex<HashMap<(String, i32, i32, i32), i32>>,
}

impl InMemoryWorld {
    pub fn place(&self, dimension: &str, position: BlockPosition, block_state: i32) {
        self.blocks
            .lock()
            .expect("in-memory world poisoned")
            .insert((dimension.to_string(), position.x, position.y, position.z), block_state);
    }
}

impl WorldBlockSource for InMemoryWorld {
    fn block_state_at(&self, dimension: &str, position: &BlockPosition) -> Option<i32> {
        self.blocks
            .lock()
            .expect("in-memory world poisoned")
            .get(&(dimension.to_string(), position.x, position.y, position.z))
            .copied()
    }
}

// ─────────────────── 方块状态注册表（play 域共享） ───────────────────

struct BlockStateRegistry {
    id_by_state: HashMap<String, i32>,
    state_by_id: HashMap<i32, BlockStateEntry>,
    default_state_by_name: HashMap<String, i32>,
}

fn registry() -> &'static BlockStateRegistry {
    static REGISTRY: OnceLock<BlockStateRegistry> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        load_registry().unwrap_or_else(|err| {
            log::warn!(
                "{}",
                qexed_language::t("qexed.play.registry.block_item_fallback")
                    .replace("%{error}", &format!("{err:#}"))
            );
            fallback_registry()
        })
    })
}

fn load_registry() -> Result<BlockStateRegistry, qexed_mojang_data::registry_sync::RegistryError> {
    let value = qexed_mojang_data::registry_sync::load_blocks_report()?;
    let blocks = value
        .as_object()
        .ok_or_else(|| {
            qexed_mojang_data::registry_sync::RegistryError::msg("block report root is not object")
        })?;

    let mut id_by_state = HashMap::new();
    let mut state_by_id = HashMap::new();
    let mut default_state_by_name = HashMap::new();

    for (name, block) in blocks {
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
                default_state_by_name.insert(name.clone(), id);
            }
        }
    }

    Ok(BlockStateRegistry {
        id_by_state,
        state_by_id,
        default_state_by_name,
    })
}

fn fallback_registry() -> BlockStateRegistry {
    let entry = BlockStateEntry {
        name: "minecraft:stone".to_string(),
        properties: Vec::new(),
    };
    BlockStateRegistry {
        id_by_state: HashMap::from([("minecraft:stone|".to_string(), 1)]),
        state_by_id: HashMap::from([(1, entry)]),
        default_state_by_name: HashMap::from([("minecraft:stone".to_string(), 1)]),
    }
}

fn json_string_properties(value: Option<&serde_json::Value>) -> Vec<(String, String)> {
    value
        .and_then(serde_json::Value::as_object)
        .map(|properties| {
            properties
                .iter()
                .filter_map(|(key, value)| {
                    value
                        .as_str()
                        .map(|value| (key.clone(), value.to_string()))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn state_key(name: &str, properties: &[(String, String)]) -> String {
    let mut sorted = properties.to_vec();
    sorted.sort();
    let props = sorted
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join(",");
    format!("{name}|{props}")
}

/// 方块状态条目（v4 world::chunk_nbt::block_state_entry）。
pub fn block_state_entry(id: i32) -> BlockStateEntry {
    registry()
        .state_by_id
        .get(&id)
        .cloned()
        .unwrap_or(BlockStateEntry {
            name: "minecraft:air".to_string(),
            properties: Vec::new(),
        })
}

/// Mojang 方块报告（blocks.json）数据是否可用（单测用：无数据时依赖注册表的
/// 测试按 v4 crafting 测试的惯例静默跳过，而不是 panic）。
///
/// 判定：内置兜底表只含 minecraft:stone；用兜底表必然缺失的 oak_planks 探测。
#[cfg(test)]
pub(crate) fn block_report_available() -> bool {
    registry().default_state_by_name.contains_key("minecraft:oak_planks")
}

/// 按名称+属性查方块状态 id（v4 world::chunk_nbt::block_state；未知时退化为默认状态）。
pub fn block_state(name: &str, properties: &[(String, String)]) -> i32 {
    let normalized = normalize_identifier(name);
    let key = state_key(&normalized, properties);
    registry()
        .id_by_state
        .get(&key)
        .copied()
        .unwrap_or_else(|| default_block_state_id(&normalized))
}

/// 方块默认状态 id（v4 world::chunk_nbt::default_block_state_id；未知为 0=air）。
pub fn default_block_state_id(name: &str) -> i32 {
    registry()
        .default_state_by_name
        .get(&normalize_identifier(name))
        .copied()
        .unwrap_or(0)
}

fn normalize_identifier(value: &str) -> String {
    if value.contains(':') {
        value.to_string()
    } else {
        format!("minecraft:{value}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_memory_world_round_trips_blocks() {
        let world = InMemoryWorld::default();
        world.place(
            "minecraft:overworld",
            BlockPosition { x: 1, y: 64, z: 2 },
            42,
        );
        assert_eq!(
            world.block_state_at(
                "minecraft:overworld",
                &BlockPosition { x: 1, y: 64, z: 2 }
            ),
            Some(42)
        );
        assert_eq!(
            world.block_state_at(
                "minecraft:overworld",
                &BlockPosition { x: 3, y: 64, z: 2 }
            ),
            None
        );
    }

    #[test]
    fn fallback_registry_maps_stone() {
        // 数据目录缺失时回退表仍能解析 stone
        assert_eq!(block_state_entry(1).name, "minecraft:stone");
    }
}

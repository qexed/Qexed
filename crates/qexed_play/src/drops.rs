//! 方块掉落物（v4 play/drops.rs 迁移）。
//!
//! v6 适配：
//! - EntityPosition 用 qexed_protocol::types（v4 在 add_entity 包内）
//! - 实体类型 id 与掉落物实体改用 qexed_entities
//! - 物品/方块映射（v4 crate::inventory）由 play-gameplay 任务落地后接线，
//!   当前经 [`ItemRegistry`] trait 注入（见 context.rs 邻域约定）。
//!
//! TODO(play-gameplay)：inventory 域（is_air_block_state / picked_item_for_block_state /
//! simple_item / item_id_for_name）迁移后提供默认 ItemRegistry 实现。

// play-gameplay 接线前部分 API 暂未被本 crate 引用（供其改造 runtime.rs 时使用）。
#![allow(dead_code)]


use qexed_entities::{DroppedItemEntity, registry::entity_type_id};
use qexed_protocol::types::{EntityPosition, Slot};

use crate::bootstrap::entities_position;

use crate::error::{PlayError, Result};

/// 物品/方块注册表面（v4 crate::inventory 的 play 掉落子集）。
pub trait ItemRegistry: Send + Sync {
    /// 方块状态是否为空气。
    fn is_air_block_state(&self, block_state: i32) -> bool;
    /// 空气方块状态 id。
    fn air_block_state(&self) -> i32;
    /// 方块状态对应的掉落物品 id。
    fn picked_item_for_block_state(&self, block_state: i32) -> Option<i32>;
    /// 物品名 -> id。
    fn item_id_for_name(&self, name: &str) -> Option<i32>;
    /// 构造简单物品槽。
    fn simple_item(&self, item_id: i32, count: i32) -> Slot;
    /// 空槽。
    fn empty_slot(&self) -> Slot;
}

/// 方块破坏的掉落物预览（v4 block_drop_item）。
pub fn block_drop_item(
    items: &dyn ItemRegistry,
    block_state: i32,
    position: &qexed_packet::net_types::Position,
) -> Option<(EntityPosition, Slot)> {
    if items.is_air_block_state(block_state) {
        return None;
    }

    items.picked_item_for_block_state(block_state).map(|item_id| {
        // v4 super::mining::drop_position；mining 属 play-gameplay 任务，
        // 此处内联同语义实现（方块中心 + 0.5，y + 0.2 随机省略为固定抬升）。
        let drop_position = EntityPosition {
            x: position.x as f64 + 0.5,
            y: position.y as f64 + 0.5,
            z: position.z as f64 + 0.5,
            yaw: 0.0,
            pitch: 0.0,
            on_ground: false,
        };
        (drop_position, items.simple_item(item_id, 1))
    })
}

/// 掉落物生成包预览（v4 block_drop_preview_packets）。
pub fn block_drop_preview_packets(
    items: &dyn ItemRegistry,
    entity_id: i32,
    block_state: i32,
    position: &qexed_packet::net_types::Position,
) -> Result<Option<Vec<bytes::Bytes>>> {
    let Some((drop_position, item)) = block_drop_item(items, block_state, position) else {
        return Ok(None);
    };
    let entity = DroppedItemEntity {
        entity_id,
        uuid: uuid::Uuid::new_v4(),
        dimension: "minecraft:overworld".to_string(),
        position: entities_position(drop_position),
        item,
        pickup_ready_at: std::time::Instant::now(),
    };
    let entity_type = entity_type_id("minecraft:item")?;
    Ok(Some(entity.spawn_packets(entity_type)?))
}

/// 收集玩家附近的掉落物（v4 collect_dropped_items）。
pub fn collect_dropped_items(
    entities: &qexed_entities::EntityManager,
    dimension: &str,
    position: EntityPosition,
) -> Result<Vec<DroppedItemEntity>> {
    entities
        .collect_reachable_items(dimension, entities_position(position))
        .map_err(PlayError::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use qexed_packet::Packet;

    /// 简单注册表：id=1 泥土；掉落自身。
    struct TestItems;

    impl ItemRegistry for TestItems {
        fn is_air_block_state(&self, block_state: i32) -> bool {
            block_state == 0
        }

        fn air_block_state(&self) -> i32 {
            0
        }

        fn picked_item_for_block_state(&self, block_state: i32) -> Option<i32> {
            (block_state != 0).then_some(block_state)
        }

        fn item_id_for_name(&self, name: &str) -> Option<i32> {
            match name {
                "minecraft:dirt" => Some(1),
                _ => None,
            }
        }

        fn simple_item(&self, item_id: i32, count: i32) -> Slot {
            Slot {
                item_count: qexed_packet::net_types::VarInt(count),
                item_id: Some(qexed_packet::net_types::VarInt(item_id)),
                ..Slot::default()
            }
        }

        fn empty_slot(&self) -> Slot {
            Slot::default()
        }
    }

    #[test]
    #[ignore = "TODO(data): 需要 mojang 26.3 注册表数据落地"]
    fn block_drop_packets_spawn_item_entity_for_known_block() {
        let packets = block_drop_preview_packets(
            &TestItems,
            42,
            1,
            &qexed_packet::net_types::Position {
                x: -5,
                y: 64,
                z: 10,
            },
        )
        .unwrap()
        .unwrap();

        assert_eq!(packets.len(), 2);
        assert_eq!(
            packets[0][0],
            qexed_protocol::to_client::play::add_entity::AddEntity::ID as u8
        );
        assert_eq!(
            packets[1][0],
            qexed_protocol::to_client::play::set_entity_data::SetEntityData::ID as u8
        );
    }

    #[test]
    fn block_drop_packets_skip_air_blocks() {
        assert!(
            block_drop_preview_packets(
                &TestItems,
                42,
                0,
                &qexed_packet::net_types::Position::default(),
            )
            .unwrap()
            .is_none()
        );
    }
}

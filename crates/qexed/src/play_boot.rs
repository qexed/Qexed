//! play 会话最小启动集：空世界 + 基础物品表（组装层注入真实实现前的引导）。
//!
//! EmptyWorld 的区块响应是空超平坦（LevelChunkWithLight 空气填充），
//! 保证客户端能进入世界看到地形占位；真实 WorldManager 适配就位后替换。

use bytes::Bytes;
use qexed_packet::net_types::Position as BlockPosition;
use qexed_play::context::PlacedBlockUpdate;
use qexed_play::{PlayError, Result, SharedWorld};
use qexed_protocol::to_client::play::level_chunk_with_light::LevelChunkWithLight;

/// 空世界源（会话期静态空区块）。
pub struct EmptyWorld;

impl qexed_play::context::WorldChunkSource for EmptyWorld {
    fn cache_epoch(&self) -> u64 {
        0
    }

    fn begin_session(&self) {}

    fn end_session(&self) {}

    fn precompiled_chunk_packet(
        &self,
        _dimension: &str,
        _chunk_x: i32,
        _chunk_z: i32,
        _cache_epoch: u64,
        _compression_threshold: Option<i32>,
    ) -> Result<Option<Bytes>> {
        Ok(None)
    }

    fn saved_network_chunk_for_session(
        &self,
        _dimension: &str,
        _chunk_x: i32,
        _chunk_z: i32,
        _cache_epoch: u64,
    ) -> Result<Option<LevelChunkWithLight>> {
        Ok(None)
    }

    fn generated_network_chunk_for_session(
        &self,
        _dimension: &str,
        chunk_x: i32,
        chunk_z: i32,
        _cache_epoch: u64,
    ) -> Result<LevelChunkWithLight> {
        Ok(qexed_world::world::empty_chunk_packet(chunk_x, chunk_z, qexed_world::world::WorldLightMode::default()))
    }

    fn fluid_positions_in_chunk(
        &self,
        _dimension: &str,
        _chunk_x: i32,
        _chunk_z: i32,
    ) -> Result<Vec<(BlockPosition, i32)>> {
        Ok(Vec::new())
    }

    fn placed_block_updates(
        &self,
        _dimension: &str,
        _chunk_x: i32,
        _chunk_z: i32,
    ) -> Vec<PlacedBlockUpdate> {
        Vec::new()
    }

    fn precompiled_chunk_packets_enabled(&self) -> bool {
        false
    }

    fn precompiled_chunk_payload_includes_light(&self) -> bool {
        false
    }

    fn remember_precompiled_chunk_frame(
        &self,
        _dimension: &str,
        _chunk_x: i32,
        _chunk_z: i32,
        _frame: Bytes,
        _compression_threshold: Option<i32>,
    ) {
    }

    fn remember_precompiled_chunk_packet_without_light(
        &self,
        _dimension: &str,
        _chunk_x: i32,
        _chunk_z: i32,
        _prefix: Bytes,
        _compression_threshold: Option<i32>,
    ) {
    }

    fn block_state_at(&self, _dimension: &str, _position: &BlockPosition) -> Option<i32> {
        None
    }
}

/// 空区块（全空气超平坦占位）。
fn empty_chunk() -> Result<LevelChunkWithLight> {
    Ok(qexed_world::world::empty_chunk_packet(0, 0, qexed_world::world::WorldLightMode::default()))
}

/// 共享空世界。
pub fn shared_empty_world() -> SharedWorld {
    std::sync::Arc::new(EmptyWorld)
}

/// 静态物品表（无注册表时的最小实现）。
pub struct StaticItemSet;

impl qexed_play::ItemRegistry for StaticItemSet {
    fn is_air_block_state(&self, block_state: i32) -> bool {
        block_state == 0
    }

    fn air_block_state(&self) -> i32 {
        0
    }

    fn picked_item_for_block_state(&self, _block_state: i32) -> Option<i32> {
        None
    }

    fn item_id_for_name(&self, _name: &str) -> Option<i32> {
        None
    }

    fn simple_item(&self, item_id: i32, count: i32) -> qexed_protocol::types::Slot {
        qexed_protocol::types::Slot {
            item_count: qexed_packet::net_types::VarInt(count),
            item_id: Some(qexed_packet::net_types::VarInt(item_id)),
            number_of_components_to_add: None,
            number_of_components_to_remove: None,
            components_to_add: None,
            components_to_remove: None,
        }
    }

    fn empty_slot(&self) -> qexed_protocol::types::Slot {
        qexed_protocol::types::Slot {
            item_count: qexed_packet::net_types::VarInt(0),
            item_id: None,
            number_of_components_to_add: None,
            number_of_components_to_remove: None,
            components_to_add: None,
            components_to_remove: None,
        }
    }
}

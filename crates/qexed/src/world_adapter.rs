//! WorldManager → play 域 WorldChunkSource 的适配器（v4 ServerContext.world 的直连）。

use bytes::Bytes;
use qexed_packet::net_types::Position as BlockPosition;
use qexed_play::context::{PlacedBlockUpdate, WorldChunkSource};
use qexed_play::{PlayError, Result, SharedWorld};
use qexed_protocol::to_client::play::level_chunk_with_light::LevelChunkWithLight;
use qexed_world::world::WorldManager;

/// 真实世界源：转发到 WorldManager（区块包/缓存/流体/方块全能力）。
pub struct RealWorld(pub std::sync::Arc<WorldManager>);

impl WorldChunkSource for RealWorld {
    fn cache_epoch(&self) -> u64 {
        self.0.cache_epoch()
    }

    fn begin_session(&self) {
        // WorldSession 的 end 在 Drop；这里持有计数由 session 生命周期管理，
        // 适配层用 manager 级会话保持（v4 ServerContext 同款常驻）。
        let _session = self.0.begin_session();
        std::mem::forget(_session); // 常驻会话：服务器关闭时统一释放
    }

    fn end_session(&self) {
        // 对应 begin 的计数递减（WorldSession::end 的语义）
        // WorldManager 未暴露裸减——保守做法：不再额外递减（常驻会话）。
    }

    fn precompiled_chunk_packet(
        &self,
        dimension: &str,
        chunk_x: i32,
        chunk_z: i32,
        cache_epoch: u64,
        compression_threshold: Option<i32>,
    ) -> Result<Option<Bytes>> {
        self.0
            .precompiled_chunk_packet(dimension, chunk_x, chunk_z, cache_epoch, compression_threshold)
            .map_err(map_err)
    }

    fn saved_network_chunk_for_session(
        &self,
        dimension: &str,
        chunk_x: i32,
        chunk_z: i32,
        cache_epoch: u64,
    ) -> Result<Option<LevelChunkWithLight>> {
        self.0
            .saved_network_chunk_for_session(dimension, chunk_x, chunk_z, cache_epoch)
            .map_err(map_err)
    }

    fn generated_network_chunk_for_session(
        &self,
        dimension: &str,
        chunk_x: i32,
        chunk_z: i32,
        cache_epoch: u64,
    ) -> Result<LevelChunkWithLight> {
        self.0
            .generated_network_chunk_for_session(dimension, chunk_x, chunk_z, cache_epoch)
            .map_err(map_err)
    }

    fn fluid_positions_in_chunk(
        &self,
        dimension: &str,
        chunk_x: i32,
        chunk_z: i32,
    ) -> Result<Vec<(BlockPosition, i32)>> {
        let list = self.0.fluid_positions_in_chunk_public(dimension, chunk_x, chunk_z)
            .map_err(map_err)?;
        Ok(list)
    }

    fn placed_block_updates(
        &self,
        dimension: &str,
        chunk_x: i32,
        chunk_z: i32,
    ) -> Vec<PlacedBlockUpdate> {
        self.0
            .placed_block_updates(dimension, chunk_x, chunk_z)
            .into_iter()
            .map(|update| PlacedBlockUpdate {
                location: update.location,
                block_state: update.block_state.0,
            })
            .collect()
    }

    fn precompiled_chunk_packets_enabled(&self) -> bool {
        self.0.precompiled_chunk_packets_enabled()
    }

    fn precompiled_chunk_payload_includes_light(&self) -> bool {
        self.0.precompiled_chunk_payload_includes_light()
    }

    fn remember_precompiled_chunk_frame(
        &self,
        dimension: &str,
        chunk_x: i32,
        chunk_z: i32,
        frame: Bytes,
        compression_threshold: Option<i32>,
    ) {
        self.0.remember_precompiled_chunk_frame(dimension, chunk_x, chunk_z, frame, compression_threshold)
    }

    fn remember_precompiled_chunk_packet_without_light(
        &self,
        dimension: &str,
        chunk_x: i32,
        chunk_z: i32,
        prefix: Bytes,
        compression_threshold: Option<i32>,
    ) {
        self.0.remember_precompiled_chunk_packet_without_light(dimension, chunk_x, chunk_z, prefix, compression_threshold)
    }

    fn block_state_at(&self, dimension: &str, position: &BlockPosition) -> Option<i32> {
        self.0.block_state_at(dimension, position)
    }
}

impl qexed_play::world_access::WorldBlockSource for RealWorld {
    fn block_state_at(&self, dimension: &str, position: &BlockPosition) -> Option<i32> {
        self.0.block_state_at(dimension, position)
    }
}
impl qexed_play::world_access::WorldStructureSink for RealWorld {
    fn place_blocks(
        &self,
        dimension: &str,
        blocks: Vec<(BlockPosition, i32)>,
    ) -> qexed_play::Result<
        Vec<qexed_protocol::to_client::play::block_update::BlockUpdate>,
    > {
        self.0
            .place_blocks(dimension, blocks)
            .map_err(map_err)
    }

    fn dynamic_light_enabled(&self) -> bool {
        self.0.dynamic_light_enabled()
    }

    fn light_update(
        &self,
        dimension: &str,
        chunk_x: i32,
        chunk_z: i32,
    ) -> qexed_protocol::to_client::play::light_update::LightUpdate {
        self.0.light_update(dimension, chunk_x, chunk_z)
    }
}
fn map_err(err: qexed_world::error::WorldError) -> PlayError {
    PlayError::msg(err.to_string())
}

/// 构造共享世界源。
pub fn shared_world(manager: std::sync::Arc<WorldManager>) -> SharedWorld {
    std::sync::Arc::new(RealWorld(manager))
}

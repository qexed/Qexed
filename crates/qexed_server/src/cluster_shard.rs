//! 集群分片（shard）服务端：监听 TCP，按请求分发到本地实体/世界处理。
//! 迁移自 v4 crates/qexed/src/cluster_shard.rs。
//!
//! 适配差异（v6）：v4 的 ShardState 直接持有 crate::world::WorldManager /
//! crate::entities::EntityManager / crate::players::PlayerManager。这些在 v6 归
//! qexed_world / qexed_entities / qexed_player（尚为空壳），因此这里定义
//! ClusterShardBackend trait 承载全部本地能力；RPC 调度、连接线程模型、
//! 实体 id 基址派生等纯服务器侧逻辑完整迁移。

use std::{
    collections::{HashMap, HashSet},
    io::{BufReader, BufWriter},
    net::{TcpListener, TcpStream},
    sync::{Arc, Mutex},
    thread,
    time::Instant,
};

use crate::cluster_entities::{ClusterPlayers, cluster_shard_entity_id_base};
use crate::cluster_rpc::{
    self, ClusterEntityRendering, ClusterEntitySpawning, ClusterLightAlgorithm, ClusterPacketBatch,
    ClusterPlayerSnapshot, ClusterRequest, ClusterResponse,
};
use crate::config::ServerConfig;
use crate::error::{Result, ServerError};

/// 分片后端能力（v4 ShardState 里 world/entities 的投影）。
/// qexed_world / qexed_entities 迁移到位后实现此 trait 接回。
pub trait ClusterShardBackend: Send + Sync + std::fmt::Debug {
    /// 生成一个区块并返回序列化后的 LevelChunkWithLight 包字节与光照抑制表。
    /// v4：generator.generate(...) → packet.serialize。
    /// TODO(world): 26.3 用 level_chunk_with_light 包（v4 是 map_chunk）。
    fn load_chunk(
        &self,
        dimension: &str,
        chunk_x: i32,
        chunk_z: i32,
        light_algorithm: ClusterLightAlgorithm,
    ) -> Result<(i32, i32, Vec<u8>, Vec<u8>)>;

    /// 查询方块状态。
    fn block_state_at(&self, dimension: &str, x: i32, y: i32, z: i32) -> Result<Option<i32>>;

    /// 生成玩家可见的实体包。
    fn entity_view_packets(
        &self,
        players: &[ClusterPlayerSnapshot],
        rendering: &ClusterEntityRendering,
    ) -> Result<Vec<ClusterPacketBatch>>;

    /// 实体生成 + AI tick，返回玩家包。
    fn entity_tick(
        &self,
        players: &dyn ClusterPlayers,
        snapshots: &[ClusterPlayerSnapshot],
        rendering: &ClusterEntityRendering,
        spawning: &ClusterEntitySpawning,
        default_dimension: &str,
        ai_tick_interval_ms: u64,
    ) -> Result<Vec<ClusterPacketBatch>>;

    /// 实体受击：返回 (handled, killed)。
    fn damage_entity(
        &self,
        players: &dyn ClusterPlayers,
        snapshots: &[ClusterPlayerSnapshot],
        rendering: &ClusterEntityRendering,
        target_entity_id: i32,
        attacker_entity_id: i32,
        attacker: (f64, f64, f64),
        damage: f32,
        knockback: f32,
    ) -> Result<ClusterEntityDamageResult>;

    /// 掉落物品；返回是否产生包。
    fn drop_item(
        &self,
        players: &dyn ClusterPlayers,
        snapshots: &[ClusterPlayerSnapshot],
        rendering: &ClusterEntityRendering,
        actor_profile_id: [u8; 16],
        dimension: &str,
        position: (f64, f64, f64, f32, f32, bool),
        item: Vec<u8>,
    ) -> Result<bool>;

    /// 收集掉落物：返回 (序列化 Slot + 数量, 玩家包)。
    fn collect_items(
        &self,
        players: &dyn ClusterPlayers,
        snapshots: &[ClusterPlayerSnapshot],
        collector_profile_id: [u8; 16],
        collector_entity_id: i32,
        dimension: &str,
        position: (f64, f64, f64, f32, f32, bool),
    ) -> Result<(Vec<cluster_rpc::ClusterCollectedItem>, Vec<ClusterPacketBatch>)>;
}

/// damage_entity 的返回值。
#[derive(Debug, Clone, Copy)]
pub struct ClusterEntityDamageResult {
    pub handled: bool,
    pub killed: bool,
}

/// 虚拟玩家：主服玩家在分片侧的镜像，用于实体可见性与包分发。
#[derive(Debug, Default)]
pub struct VirtualPlayerRegistry {
    players: Mutex<HashMap<uuid::Uuid, VirtualPlayer>>,
}

#[derive(Debug, Clone)]
struct VirtualPlayer {
    snapshot: ClusterPlayerSnapshot,
    /// 尚未取走的包（v4 走 PlayerEvent channel，这里收敛为简单队列）。
    pending_packets: Vec<Vec<u8>>,
}

impl VirtualPlayerRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// 同步虚拟玩家集合：移除消失的，更新/新增存在的。
    pub fn sync(&self, players: &[ClusterPlayerSnapshot]) {
        let active = players
            .iter()
            .map(|player| uuid::Uuid::from_bytes(player.profile_id))
            .collect::<HashSet<_>>();
        let mut guard = self.players.lock().expect("virtual players poisoned");
        guard.retain(|uuid, _| active.contains(uuid));
        for player in players {
            guard
                .entry(uuid::Uuid::from_bytes(player.profile_id))
                .and_modify(|existing| existing.snapshot = player.clone())
                .or_insert_with(|| VirtualPlayer {
                    snapshot: player.clone(),
                    pending_packets: Vec::new(),
                });
        }
    }

    /// 给某玩家入队一批包。
    pub fn enqueue(&self, profile_id: uuid::Uuid, packets: Vec<Vec<u8>>) {
        let mut guard = self.players.lock().expect("virtual players poisoned");
        if let Some(player) = guard.get_mut(&profile_id) {
            player.pending_packets.extend(packets);
        }
    }

    /// 广播给全部虚拟玩家（除排除项）。
    pub fn broadcast_except(&self, except: Option<uuid::Uuid>, packets: Vec<Vec<u8>>) {
        let mut guard = self.players.lock().expect("virtual players poisoned");
        for (uuid, player) in guard.iter_mut() {
            if Some(*uuid) == except {
                continue;
            }
            player.pending_packets.extend(packets.iter().cloned());
        }
    }

    /// 取走全部玩家的待发包（转成 RPC batch）。
    pub fn drain(&self) -> Vec<ClusterPacketBatch> {
        let mut guard = self.players.lock().expect("virtual players poisoned");
        let mut batches = Vec::new();
        for (uuid, player) in guard.iter_mut() {
            if player.pending_packets.is_empty() {
                continue;
            }
            batches.push(ClusterPacketBatch {
                profile_id: *uuid.as_bytes(),
                packets: std::mem::take(&mut player.pending_packets),
            });
        }
        batches
    }

    /// 当前快照列表。
    pub fn snapshots(&self) -> Vec<ClusterPlayerSnapshot> {
        let guard = self.players.lock().expect("virtual players poisoned");
        guard.values().map(|p| p.snapshot.clone()).collect()
    }
}

/// 分片运行时状态：后端 + 虚拟玩家注册表 + id 基址。
#[derive(Debug)]
pub struct ShardState<B: ClusterShardBackend> {
    pub shard_id: String,
    pub backend: Arc<B>,
    pub virtual_players: Arc<VirtualPlayerRegistry>,
    pub entity_id_base: i32,
}

impl<B: ClusterShardBackend> ShardState<B> {
    pub fn new(shard_id: String, backend: Arc<B>) -> Self {
        Self {
            entity_id_base: cluster_shard_entity_id_base(&shard_id),
            shard_id,
            backend,
            virtual_players: Arc::new(VirtualPlayerRegistry::new()),
        }
    }
}

/// ClusterPlayers 适配：把包发进虚拟玩家队列。
impl<B: ClusterShardBackend> ClusterPlayers for ShardState<B> {
    fn player_snapshots(&self) -> Vec<ClusterPlayerSnapshot> {
        self.virtual_players.snapshots()
    }

    fn send_packets_to(&self, profile_id: uuid::Uuid, packets: Vec<bytes::Bytes>) {
        self.virtual_players.enqueue(
            profile_id,
            packets.into_iter().map(|p| p.to_vec()).collect(),
        );
    }
}

/// 启动分片监听（阻塞；v4 用独立进程/线程承载，这里保持同步循环）。
pub fn run<B: ClusterShardBackend + 'static>(
    config: &ServerConfig,
    shard_id: String,
    listen: String,
    backend: Arc<B>,
) -> Result<()> {
    let state = Arc::new(ShardState::new(shard_id.clone(), backend));
    let listener = TcpListener::bind(&listen).map_err(|source| ServerError::IoContext {
        context: format!("failed to bind cluster shard listener: {listen}"),
        source,
    })?;
    log::info!(
        "{}",
        qexed_language::t("qexed.server.cluster_shard.listening")
            .replace("%{id}", &state.shard_id)
            .replace("%{addr}", &listen)
    );
    let _ = config;
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let state = state.clone();
                thread::Builder::new()
                    .name(format!("qexed-cluster-shard-{}", state.shard_id))
                    .spawn(move || {
                        if let Err(err) = handle_connection(stream, &state) {
                            log::warn!(
                                "cluster shard request failed: id={}, error={err}",
                                state.shard_id
                            );
                        }
                    })?;
            }
            Err(err) => {
                log::warn!(
                    "cluster shard accept failed: id={}, error={err}",
                    state.shard_id
                );
            }
        }
    }
    Ok(())
}

fn handle_connection<B: ClusterShardBackend>(stream: TcpStream, state: &ShardState<B>) -> Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut writer = BufWriter::new(stream);
    let request: ClusterRequest = cluster_rpc::read_frame(&mut reader)?;
    let started = Instant::now();
    let response = dispatch(request, state);
    let elapsed = started.elapsed();
    if elapsed >= cluster_rpc::SLOW_RESPONSE_THRESHOLD {
        log::debug!(
            "cluster shard slow request: id={}, elapsed_ms={:.2}",
            state.shard_id,
            elapsed.as_secs_f64() * 1000.0
        );
    }
    let response =
        response.unwrap_or_else(|err| ClusterResponse::Error(err.to_string()));
    cluster_rpc::write_frame(&mut writer, &response)
}

fn dispatch<B: ClusterShardBackend>(
    request: ClusterRequest,
    state: &ShardState<B>,
) -> Result<ClusterResponse> {
    match request {
        ClusterRequest::LoadChunk {
            dimension,
            chunk_x,
            chunk_z,
            light_algorithm,
        } => {
            let (x, z, level_chunk_with_light, light_dampening) = state
                .backend
                .load_chunk(&dimension, chunk_x, chunk_z, light_algorithm)?;
            Ok(ClusterResponse::Chunk {
                chunk_x: x,
                chunk_z: z,
                level_chunk_with_light,
                light_dampening,
            })
        }
        ClusterRequest::BlockStateAt { dimension, x, y, z } => {
            Ok(ClusterResponse::BlockState(
                state.backend.block_state_at(&dimension, x, y, z)?,
            ))
        }
        ClusterRequest::EntityView { players, rendering } => {
            state.virtual_players.sync(&players);
            let batches = state.backend.entity_view_packets(&players, &rendering)?;
            Ok(ClusterResponse::EntityPackets(batches))
        }
        ClusterRequest::EntityTick {
            players,
            rendering,
            spawning,
            default_dimension,
            ai_tick_interval_ms,
        } => {
            state.virtual_players.sync(&players);
            let batches = state.backend.entity_tick(
                state,
                &players,
                &rendering,
                &spawning,
                &default_dimension,
                ai_tick_interval_ms,
            )?;
            Ok(ClusterResponse::EntityPackets(batches))
        }
        ClusterRequest::DamageEntity {
            players,
            rendering,
            target_entity_id,
            attacker_entity_id,
            attacker_x,
            attacker_y,
            attacker_z,
            damage,
            knockback,
        } => {
            state.virtual_players.sync(&players);
            let result = state.backend.damage_entity(
                state,
                &players,
                &rendering,
                target_entity_id,
                attacker_entity_id,
                (attacker_x, attacker_y, attacker_z),
                damage,
                knockback,
            )?;
            Ok(ClusterResponse::EntityDamage {
                handled: result.handled,
                killed: result.killed,
                packets: state.virtual_players.drain(),
            })
        }
        ClusterRequest::DropItem {
            players,
            rendering,
            actor_profile_id,
            dimension,
            x,
            y,
            z,
            yaw,
            pitch,
            on_ground,
            item,
        } => {
            state.virtual_players.sync(&players);
            let handled = state.backend.drop_item(
                state,
                &players,
                &rendering,
                actor_profile_id,
                &dimension,
                (x, y, z, yaw, pitch, on_ground),
                item,
            )?;
            let _ = handled;
            Ok(ClusterResponse::EntityPackets(
                state.virtual_players.drain(),
            ))
        }
        ClusterRequest::CollectItems {
            players,
            collector_profile_id,
            collector_entity_id,
            dimension,
            x,
            y,
            z,
            yaw,
            pitch,
            on_ground,
        } => {
            state.virtual_players.sync(&players);
            let (items, packets) = state.backend.collect_items(
                state,
                &players,
                collector_profile_id,
                collector_entity_id,
                &dimension,
                (x, y, z, yaw, pitch, on_ground),
            )?;
            Ok(ClusterResponse::CollectedItems {
                items,
                packets: {
                    let mut all = packets;
                    all.extend(state.virtual_players.drain());
                    all
                },
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(profile_id: [u8; 16], entity_id: i32) -> ClusterPlayerSnapshot {
        ClusterPlayerSnapshot {
            profile_id,
            entity_id,
            username: "Tester".to_string(),
            dimension: "minecraft:overworld".to_string(),
            x: 0.0,
            y: 64.0,
            z: 0.0,
            yaw: 0.0,
            pitch: 0.0,
            on_ground: true,
        }
    }

    #[test]
    fn virtual_registry_sync_and_drain() {
        let registry = VirtualPlayerRegistry::new();
        let a = uuid::Uuid::new_v4();
        let b = uuid::Uuid::new_v4();
        registry.sync(&[snapshot(*a.as_bytes(), 1)]);
        registry.sync(&[snapshot(*a.as_bytes(), 1), snapshot(*b.as_bytes(), 2)]);
        assert_eq!(registry.snapshots().len(), 2);

        registry.enqueue(a, vec![vec![1, 2, 3]]);
        registry.broadcast_except(Some(b), vec![vec![9]]);
        let batches = registry.drain();
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].profile_id, *a.as_bytes());
        assert_eq!(batches[0].packets.len(), 2);
        assert!(registry.drain().is_empty());
        // 玩家消失
        registry.sync(&[snapshot(*b.as_bytes(), 2)]);
        assert_eq!(registry.snapshots().len(), 1);
    }
}

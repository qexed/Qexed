//! 集群实体控制器：把实体生成/AI tick 委托给远端分片（shard）。
//! 迁移自 v4 crates/qexed/src/cluster_entities.rs。
//!
//! 适配差异（v6）：
//! - v4 的 crate::players::PlayerManager / crate::world::ClusterRouter 直接耦合；
//!   v6 qexed_player / qexed_world 还是空壳，这里抽出最小 trait：
//!   ClusterPlayers（玩家快照/包发送）+ ClusterRouter（chunk → shard 路由）。
//! - qexed_config::app::qexed::server::{WorldCluster,EntityRendering,EntitySpawning}
//!   换成本 crate config::WorldClusterConfig / cluster_rpc::ClusterEntity{Rendering,Spawning}。

use std::{
    collections::{BTreeSet, HashMap},
    time::Instant,
};

use crate::cluster_rpc::{
    self, ClusterEntityRendering, ClusterEntitySpawning, ClusterPacketBatch, ClusterPlayerSnapshot,
    ClusterRequest, ClusterResponse,
};
use crate::config::{ClusterShardConfig, WorldClusterConfig};
use crate::error::{Result, ServerError};

/// 玩家侧接口（v4 crate::players::PlayerManager 的集群投影）。
pub trait ClusterPlayers {
    /// 除排除项外的在线玩家快照（v4 list_except(nil)）。
    fn player_snapshots(&self) -> Vec<ClusterPlayerSnapshot>;
    /// 向指定玩家发送一批原始包。
    fn send_packets_to(&self, profile_id: uuid::Uuid, packets: Vec<bytes::Bytes>);
}

/// chunk → shard 路由接口（v4 crate::world::ClusterRouter 的集群投影）。
pub trait ClusterRouting: std::fmt::Debug + Send + Sync {
    fn route(&self, chunk_x: i32, chunk_z: i32) -> Option<&str>;
}

/// 基于 config 的区域路由器（v4 ClusterRouter::regions 模式的最小实现）。
#[derive(Debug, Clone)]
pub struct RegionRouter {
    regions: Vec<Region>,
}

#[derive(Debug, Clone)]
struct Region {
    shard_id: String,
    min_chunk_x: Option<i32>,
    max_chunk_x: Option<i32>,
    min_chunk_z: Option<i32>,
    max_chunk_z: Option<i32>,
}

impl RegionRouter {
    /// 从集群配置构建；无有效分片时返回 None（对应 v4 from_config 的 Option）。
    pub fn from_config(config: &WorldClusterConfig) -> Option<Self> {
        let mut regions = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for shard in &config.shards {
            let id = shard.id.trim().to_string();
            if id.is_empty() || !seen.insert(id.clone()) {
                return None;
            }
            if let Some((min, max)) = shard.x_range
                && min > max
            {
                return None;
            }
            if let Some((min, max)) = shard.z_range
                && min > max
            {
                return None;
            }
            regions.push(Region {
                shard_id: id,
                min_chunk_x: shard.x_range.map(|r| r.0),
                max_chunk_x: shard.x_range.map(|r| r.1),
                min_chunk_z: shard.z_range.map(|r| r.0),
                max_chunk_z: shard.z_range.map(|r| r.1),
            });
        }
        (!regions.is_empty()).then_some(Self { regions })
    }

    /// 分片 id 列表。
    pub fn shard_ids(&self) -> Vec<&str> {
        self.regions.iter().map(|r| r.shard_id.as_str()).collect()
    }
}

impl ClusterRouting for RegionRouter {
    fn route(&self, chunk_x: i32, chunk_z: i32) -> Option<&str> {
        self.regions
            .iter()
            .find(|region| {
                region.min_chunk_x.is_none_or(|min| chunk_x >= min)
                    && region.max_chunk_x.is_none_or(|max| chunk_x <= max)
                    && region.min_chunk_z.is_none_or(|min| chunk_z >= min)
                    && region.max_chunk_z.is_none_or(|max| chunk_z <= max)
            })
            .map(|region| region.shard_id.as_str())
    }
}

#[derive(Debug)]
pub struct ClusterEntityController<R: ClusterRouting> {
    router: R,
    endpoints: HashMap<String, String>,
}

impl<R: ClusterRouting> ClusterEntityController<R> {
    /// 集群关闭或无 tcp:// 分片时返回 None（本地实体系统接管）。
    pub fn from_config(config: &WorldClusterConfig, router: R) -> Option<Self> {
        if !config.enable {
            return None;
        }
        let endpoints = config
            .shards
            .iter()
            .filter(|shard| shard.endpoint.trim().starts_with("tcp://"))
            .map(|shard: &ClusterShardConfig| {
                (
                    shard.id.trim().to_string(),
                    shard.endpoint.trim().to_string(),
                )
            })
            .filter(|(id, _)| !id.is_empty())
            .collect::<HashMap<_, _>>();
        (!endpoints.is_empty()).then_some(Self { router, endpoints })
    }

    /// 每个游戏 tick：把玩家快照发给附近分片执行实体生成/AI。
    pub fn tick(
        &self,
        players: &dyn ClusterPlayers,
        rendering: &ClusterEntityRendering,
        spawning: &ClusterEntitySpawning,
        default_dimension: &str,
        simulation_distance: i32,
    ) -> Result<()> {
        let player_snapshots = players.player_snapshots();
        if player_snapshots.is_empty() {
            return Ok(());
        }
        let shards = self.shards_near_players(&player_snapshots, simulation_distance);
        if shards.is_empty() {
            return Ok(());
        }
        for shard_id in shards {
            let Some(endpoint) = self.endpoints.get(&shard_id) else {
                continue;
            };
            let request = ClusterRequest::EntityTick {
                players: player_snapshots.clone(),
                rendering: rendering.clone(),
                spawning: spawning.clone(),
                default_dimension: default_dimension.to_string(),
                ai_tick_interval_ms: spawning.tick_interval_ms,
            };
            match self.request(endpoint, &request) {
                Ok(ClusterResponse::EntityPackets(batches)) => {
                    log_batch_stats("cluster entity packets", &shard_id, &batches);
                    apply_packet_batches(players, batches);
                }
                Ok(ClusterResponse::Error(message)) => {
                    log::warn!(
                        "cluster entity shard tick failed: shard={shard_id}, error={message}"
                    );
                }
                Ok(_) => {
                    log::warn!(
                        "cluster entity shard returned unexpected response: shard={shard_id}"
                    );
                }
                Err(err) => {
                    log::warn!(
                        "cluster entity shard tick unavailable: shard={shard_id}, error={err}"
                    );
                }
            }
        }
        Ok(())
    }

    /// 给玩家生成远端实体的初始可见包。
    /// players 保留用于 API 对称（v4 同名参数亦未直接使用）。
    pub fn spawn_view_for_player(
        &self,
        _players: &dyn ClusterPlayers,
        player: &ClusterPlayerSnapshot,
        rendering: &ClusterEntityRendering,
        simulation_distance: i32,
    ) -> Vec<bytes::Bytes> {
        let shards = self.shards_near_players(std::slice::from_ref(player), simulation_distance);
        let mut packets = Vec::new();
        for shard_id in shards {
            let Some(endpoint) = self.endpoints.get(&shard_id) else {
                continue;
            };
            let request = ClusterRequest::EntityView {
                players: vec![player.clone()],
                rendering: rendering.clone(),
            };
            match self.request(endpoint, &request) {
                Ok(ClusterResponse::EntityPackets(batches)) => {
                    log_batch_stats("cluster entity view packets", &shard_id, &batches);
                    packets.extend(
                        batches
                            .into_iter()
                            .flat_map(|batch| batch.packets)
                            .map(bytes::Bytes::from),
                    );
                }
                Ok(ClusterResponse::Error(message)) => {
                    log::warn!("cluster entity view failed: shard={shard_id}, error={message}");
                }
                Ok(_) => {}
                Err(err) => {
                    log::warn!("cluster entity view unavailable: shard={shard_id}, error={err}");
                }
            }
        }
        packets
    }

    /// 委托远端分片处理一次实体伤害。
    pub fn damage_entity(
        &self,
        players: &dyn ClusterPlayers,
        actor: &ClusterPlayerSnapshot,
        rendering: &ClusterEntityRendering,
        simulation_distance: i32,
        target_entity_id: i32,
        damage: f32,
    ) -> Result<Option<ClusterEntityDamageOutcome>> {
        let player_snapshots = players.player_snapshots();
        if player_snapshots.is_empty() {
            return Ok(None);
        }
        let shards = self.shards_near_players(std::slice::from_ref(actor), simulation_distance);
        for shard_id in shards {
            let Some(endpoint) = self.endpoints.get(&shard_id) else {
                continue;
            };
            let request = ClusterRequest::DamageEntity {
                players: player_snapshots.clone(),
                rendering: rendering.clone(),
                target_entity_id,
                attacker_entity_id: actor.entity_id,
                attacker_x: actor.x,
                attacker_y: actor.y,
                attacker_z: actor.z,
                damage,
                knockback: 0.4,
            };
            match self.request(endpoint, &request)? {
                ClusterResponse::EntityDamage {
                    handled,
                    killed,
                    packets,
                } => {
                    apply_packet_batches(players, packets);
                    if handled {
                        return Ok(Some(ClusterEntityDamageOutcome { killed }));
                    }
                }
                ClusterResponse::Error(message) => {
                    log::warn!("cluster entity damage failed: shard={shard_id}, error={message}");
                }
                _ => {}
            }
        }
        Ok(None)
    }

    /// 委托远端分片掉落一个物品。
    pub fn drop_item(
        &self,
        players: &dyn ClusterPlayers,
        actor: uuid::Uuid,
        dimension: &str,
        x: f64,
        y: f64,
        z: f64,
        yaw: f32,
        pitch: f32,
        on_ground: bool,
        rendering: &ClusterEntityRendering,
        item: &qexed_protocol::types::Slot,
    ) -> Result<bool> {
        let Some(endpoint) = self.endpoint_for_position(x, z) else {
            return Ok(false);
        };
        let mut item_bytes = bytes::BytesMut::new();
        qexed_packet::PacketWriter::new(&mut item_bytes).serialize(item)?;
        let player_snapshots = players.player_snapshots();
        let request = ClusterRequest::DropItem {
            players: player_snapshots,
            rendering: rendering.clone(),
            actor_profile_id: *actor.as_bytes(),
            dimension: dimension.to_string(),
            x,
            y,
            z,
            yaw,
            pitch,
            on_ground,
            item: item_bytes.to_vec(),
        };
        match self.request(endpoint, &request)? {
            ClusterResponse::EntityPackets(batches) => {
                apply_packet_batches(players, batches);
                Ok(true)
            }
            ClusterResponse::Error(message) => {
                log::warn!("cluster item drop failed: error={message}");
                Ok(false)
            }
            _ => Ok(false),
        }
    }

    /// 委托远端分片收集掉落物，返回收集到的 Slot。
    pub fn collect_items(
        &self,
        players: &dyn ClusterPlayers,
        collector: &ClusterPlayerSnapshot,
        simulation_distance: i32,
    ) -> Result<Vec<qexed_protocol::types::Slot>> {
        let player_snapshots = players.player_snapshots();
        if player_snapshots.is_empty() {
            return Ok(Vec::new());
        }
        let shards =
            self.shards_near_players(std::slice::from_ref(collector), simulation_distance);
        let mut collected = Vec::new();
        for shard_id in shards {
            let Some(endpoint) = self.endpoints.get(&shard_id) else {
                continue;
            };
            let request = ClusterRequest::CollectItems {
                players: player_snapshots.clone(),
                collector_profile_id: collector.profile_id,
                collector_entity_id: collector.entity_id,
                dimension: collector.dimension.clone(),
                x: collector.x,
                y: collector.y,
                z: collector.z,
                yaw: collector.yaw,
                pitch: collector.pitch,
                on_ground: collector.on_ground,
            };
            match self.request(endpoint, &request)? {
                ClusterResponse::CollectedItems { items, packets } => {
                    for item in items {
                        let mut bytes = bytes::BytesMut::from(item.item.as_slice());
                        let mut reader = qexed_packet::PacketReader::new(&mut bytes);
                        let mut slot: qexed_protocol::types::Slot = reader.deserialize()?;
                        slot.item_count.0 = item.count.max(1);
                        collected.push(slot);
                    }
                    apply_packet_batches(players, packets);
                }
                ClusterResponse::Error(message) => {
                    log::warn!("cluster item collect failed: shard={shard_id}, error={message}");
                }
                _ => {}
            }
        }
        Ok(collected)
    }

    fn request(&self, endpoint: &str, request: &ClusterRequest) -> Result<ClusterResponse> {
        let started = Instant::now();
        let response = cluster_rpc::send_request(
            endpoint,
            request,
            cluster_rpc::DEFAULT_CONNECT_TIMEOUT,
            cluster_rpc::DEFAULT_REQUEST_TIMEOUT,
        )?;
        let elapsed = started.elapsed();
        if elapsed >= cluster_rpc::SLOW_RESPONSE_THRESHOLD {
            log::debug!(
                "cluster entity shard slow response: endpoint={}, elapsed_ms={:.2}",
                endpoint,
                elapsed.as_secs_f64() * 1000.0
            );
        }
        Ok(response)
    }

    fn endpoint_for_position(&self, x: f64, z: f64) -> Option<&str> {
        let chunk_x = (x.floor() as i32).div_euclid(16);
        let chunk_z = (z.floor() as i32).div_euclid(16);
        let shard_id = self.router.route(chunk_x, chunk_z)?;
        self.endpoints.get(shard_id).map(String::as_str)
    }

    fn shards_near_players(
        &self,
        players: &[ClusterPlayerSnapshot],
        simulation_distance: i32,
    ) -> Vec<String> {
        let radius = simulation_distance.max(1);
        let mut shards = BTreeSet::new();
        for player in players {
            let center_x = (player.x.floor() as i32).div_euclid(16);
            let center_z = (player.z.floor() as i32).div_euclid(16);
            for chunk_x in center_x - radius..=center_x + radius {
                for chunk_z in center_z - radius..=center_z + radius {
                    if let Some(shard_id) = self.router.route(chunk_x, chunk_z)
                        && self.endpoints.contains_key(shard_id)
                    {
                        shards.insert(shard_id.to_string());
                    }
                }
            }
        }
        shards.into_iter().collect()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ClusterEntityDamageOutcome {
    pub killed: bool,
}

fn log_batch_stats(label: &str, shard_id: &str, batches: &[ClusterPacketBatch]) {
    if batches.is_empty() {
        return;
    }
    let packet_count = batches.iter().map(|batch| batch.packets.len()).sum::<usize>();
    log::debug!(
        "{label}: shard={shard_id}, batches={}, packets={packet_count}",
        batches.len()
    );
}

fn apply_packet_batches(players: &dyn ClusterPlayers, batches: Vec<ClusterPacketBatch>) {
    for batch in batches {
        let profile_id = uuid::Uuid::from_bytes(batch.profile_id);
        let packets = batch
            .packets
            .into_iter()
            .map(bytes::Bytes::from)
            .collect::<Vec<_>>();
        if !packets.is_empty() {
            players.send_packets_to(profile_id, packets);
        }
    }
}

/// v4 cluster_shard_entity_id_base：按分片 id 派生互不冲突的实体 id 基址
/// （FNV-1a 哈希 → 100000 + bucket * 10000）。
pub fn cluster_shard_entity_id_base(shard_id: &str) -> i32 {
    const BASE: i32 = 100_000;
    const STRIDE: i32 = 10_000;
    const BUCKETS: i32 = 1_000;

    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in shard_id.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    BASE + i32::try_from(hash % BUCKETS as u64).unwrap_or(0) * STRIDE
}

/// 占位：把 v4 的 anyhow 错误串接到 ServerError（保留错误文本）。
#[allow(dead_code)]
fn coerce_error(err: ServerError) -> ServerError {
    err
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shard(id: &str, endpoint: &str, x: Option<(i32, i32)>, z: Option<(i32, i32)>) -> ClusterShardConfig {
        ClusterShardConfig {
            id: id.to_string(),
            endpoint: endpoint.to_string(),
            x_range: x,
            z_range: z,
        }
    }

    #[test]
    fn disabled_cluster_returns_none() {
        let config = WorldClusterConfig::default();
        // 无分片时路由器本身构建不出来（对应 v4 的 from_config Option 语义）
        assert!(RegionRouter::from_config(&config).is_none());
        // 即使路由器有效，enable=false 时控制器也为 None
        let enabled = WorldClusterConfig {
            shards: vec![shard("a", "tcp://127.0.0.1:1", None, None)],
            ..WorldClusterConfig::default()
        };
        let router = RegionRouter::from_config(&enabled).unwrap();
        assert!(ClusterEntityController::from_config(&config, router).is_none());
    }

    #[test]
    fn router_routes_by_regions() {
        let config = WorldClusterConfig {
            enable: true,
            shards: vec![
                shard("west", "tcp://127.0.0.1:1", Some((i32::MIN, -1)), None),
                shard("east", "tcp://127.0.0.1:2", Some((0, i32::MAX)), None),
            ],
        };
        let router = RegionRouter::from_config(&config).expect("router");
        assert_eq!(router.route(-5, 3), Some("west"));
        assert_eq!(router.route(7, -9), Some("east"));
        let controller =
            ClusterEntityController::from_config(&config, router).expect("controller");
        assert_eq!(controller.endpoint_for_position(-16.0, 0.0), Some("tcp://127.0.0.1:1"));
        assert_eq!(controller.endpoint_for_position(16.0, 0.0), Some("tcp://127.0.0.1:2"));
    }

    #[test]
    fn duplicate_shard_ids_are_rejected() {
        let config = WorldClusterConfig {
            enable: true,
            shards: vec![
                shard("a", "tcp://127.0.0.1:1", None, None),
                shard("a", "tcp://127.0.0.1:2", None, None),
            ],
        };
        assert!(RegionRouter::from_config(&config).is_none());
    }

    #[test]
    fn entity_id_bases_are_distinct() {
        let a = cluster_shard_entity_id_base("shard-a");
        let b = cluster_shard_entity_id_base("shard-b");
        assert_ne!(a, b);
        assert!(a >= 100_000 && b >= 100_000);
    }
}

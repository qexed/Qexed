//! 世界集群：v4 `world/cluster.rs` 的 v6 迁移。
//!
//! - `ClusterRouter`：分片路由（象限/区域模式，world-misc 起草）。
//! - `ClusteredWorldGenerator` / `RemoteShardGenerator`：generator trait 实现 +
//!   远端分片 RPC 回退（world-core，基于定稿的 WorldChunkGenerator）。
//!   26.3 适配：区块包为 `level_chunk_with_light::LevelChunkWithLight`（x/z 为
//!   i32、光照 BIT_SET 字节串）；集群帧 = 4B 大端长度 + JSON（与
//!   qexed_server::cluster_rpc 同格式），传输本地实现（v4 依赖
//!   crate::cluster_rpc；v6 依赖方向禁止 world → server，协议类型复用
//!   qexed_protocol::types::ClusterRequest/Response）。

use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
    time::Instant,
};

use bytes::BytesMut;
use qexed_packet::{Packet, PacketReader, net_types::Position};
use qexed_protocol::to_client::play::level_chunk_with_light::LevelChunkWithLight;

use crate::config::{WorldCluster, WorldClusterAxisSide, WorldClusterMode, WorldClusterShard};
use crate::error::{Result, WorldError};
use qexed_protocol::types::{ClusterLightAlgorithm, ClusterRequest, ClusterResponse};

use super::{
    CHUNK_DAMPENING_LEN, WorldLightAlgorithm,
    generator::{GeneratedChunk, WorldChunkGenerator},
};

/// 集群 RPC 超时/冷却（v4 crate::cluster_rpc 原值；26.3 帧格式与
/// qexed_server::cluster_rpc 一致：4 字节大端长度 + UTF-8 JSON）。
const DEFAULT_CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(750);
const DEFAULT_REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(1500);
const SLOW_RESPONSE_THRESHOLD: std::time::Duration = std::time::Duration::from_millis(100);
const FAILURE_COOLDOWN: std::time::Duration = std::time::Duration::from_secs(2);
const MAX_FRAME_BYTES: usize = 16 * 1024 * 1024;

/// 解析 tcp://host:port 集群 endpoint（v4 cluster_rpc::endpoint_addr）。
fn endpoint_addr(endpoint: &str) -> Result<std::net::SocketAddr> {
    use std::net::ToSocketAddrs;
    let raw = endpoint
        .strip_prefix("tcp://")
        .ok_or_else(|| WorldError::msg(format!("invalid cluster endpoint: {endpoint}")))?;
    raw.to_socket_addrs()?
        .next()
        .ok_or_else(|| WorldError::msg(format!("cluster endpoint did not resolve: {endpoint}")))
}

/// 发送一次集群 RPC 请求（v4 cluster_rpc::send_request；帧 = 4B 长度 + JSON）。
fn send_request(
    endpoint: &str,
    request: &ClusterRequest,
    connect_timeout: std::time::Duration,
    request_timeout: std::time::Duration,
) -> Result<ClusterResponse> {
    use std::io::{Read, Write};

    let addr = endpoint_addr(endpoint)?;
    let mut stream = std::net::TcpStream::connect_timeout(&addr, connect_timeout)?;
    stream.set_read_timeout(Some(request_timeout))?;
    stream.set_write_timeout(Some(request_timeout))?;
    write_frame(&mut stream, request)?;
    read_frame(&mut stream)
}

fn write_frame<T: serde::Serialize>(writer: &mut impl std::io::Write, value: &T) -> Result<()> {
    let payload = serde_json::to_vec(value)?;
    if payload.len() > MAX_FRAME_BYTES {
        return Err(WorldError::msg(format!(
            "cluster frame too large: {} bytes",
            payload.len()
        )));
    }
    let len = u32::try_from(payload.len())
        .map_err(|_| WorldError::msg(format!("cluster frame too large: {}", payload.len())))?
        .to_be_bytes();
    writer.write_all(&len)?;
    writer.write_all(&payload)?;
    writer.flush()?;
    Ok(())
}

fn read_frame<T: for<'de> serde::Deserialize<'de>>(reader: &mut impl std::io::Read) -> Result<T> {
    let mut len = [0_u8; 4];
    reader.read_exact(&mut len)?;
    let len = u32::from_be_bytes(len) as usize;
    if len > MAX_FRAME_BYTES {
        return Err(WorldError::msg(format!("cluster frame too large: {len} bytes")));
    }
    let mut payload = vec![0; len];
    reader.read_exact(&mut payload)?;
    Ok(serde_json::from_slice(&payload)?)
}

impl From<WorldLightAlgorithm> for ClusterLightAlgorithm {
    fn from(_value: WorldLightAlgorithm) -> Self {
        // v6 集群 RPC 的算法枚举收敛为 Vanilla（v4 Fast/RayTrace 都映射过来；
        // 分片侧按 Vanilla 语义执行，本地回退仍用原算法）。
        Self::Vanilla
    }
}

#[derive(Debug)]
pub(crate) struct ClusteredWorldGenerator {
    local: Arc<dyn WorldChunkGenerator>,
    router: ClusterRouter,
    shards: HashMap<String, Arc<dyn WorldChunkGenerator>>,
    route_log: Arc<Mutex<Vec<ShardRoute>>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ShardRoute {
    pub shard_id: String,
    pub dimension: String,
    pub chunk_x: i32,
    pub chunk_z: i32,
}

impl ClusteredWorldGenerator {
    pub(crate) fn from_config(
        config: &WorldCluster,
        local: Arc<dyn WorldChunkGenerator>,
    ) -> Option<Self> {
        if !config.enable {
            return None;
        }
        ClusterRouter::from_config(config)
            .map(|router| Self::new_from_config(local, router, config))
    }

    pub(crate) fn new(local: Arc<dyn WorldChunkGenerator>, router: ClusterRouter) -> Self {
        let shards = router
            .shard_ids()
            .into_iter()
            .map(|id| (id.to_string(), local.clone()))
            .collect();
        Self {
            local,
            router,
            shards,
            route_log: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub(crate) fn new_from_config(
        local: Arc<dyn WorldChunkGenerator>,
        router: ClusterRouter,
        config: &WorldCluster,
    ) -> Self {
        let endpoints = config
            .shards
            .iter()
            .filter_map(|shard| clean_id(&shard.id).map(|id| (id, shard.endpoint.trim())))
            .collect::<HashMap<_, _>>();
        let shards = router
            .shard_ids()
            .into_iter()
            .map(|id| {
                let generator: Arc<dyn WorldChunkGenerator> = endpoints
                    .get(id)
                    .and_then(|endpoint| {
                        endpoint.strip_prefix("tcp://").map(|_| {
                            Arc::new(RemoteShardGenerator::new(
                                id.to_string(),
                                (*endpoint).to_string(),
                                local.clone(),
                            )) as Arc<dyn WorldChunkGenerator>
                        })
                    })
                    .unwrap_or_else(|| local.clone());
                (id.to_string(), generator)
            })
            .collect();
        Self {
            local,
            router,
            shards,
            route_log: Arc::new(Mutex::new(Vec::new())),
        }
    }

    #[cfg(test)]
    pub(crate) fn with_shard_generators(
        local: Arc<dyn WorldChunkGenerator>,
        router: ClusterRouter,
        shards: HashMap<String, Arc<dyn WorldChunkGenerator>>,
    ) -> Self {
        Self {
            local,
            router,
            shards,
            route_log: Arc::new(Mutex::new(Vec::new())),
        }
    }

    #[cfg(test)]
    pub(crate) fn route_log(&self) -> Vec<ShardRoute> {
        self.route_log
            .lock()
            .expect("cluster route log poisoned")
            .clone()
    }

    fn shard_for(
        &self,
        chunk_x: i32,
        chunk_z: i32,
    ) -> Option<(&str, Arc<dyn WorldChunkGenerator>)> {
        let shard_id = self.router.route(chunk_x, chunk_z)?;
        let generator = self.shards.get(shard_id).cloned()?;
        Some((shard_id, generator))
    }
}

#[derive(Debug)]
struct RemoteShardGenerator {
    shard_id: String,
    endpoint: String,
    fallback: Arc<dyn WorldChunkGenerator>,
    failure_until: Mutex<Option<Instant>>,
}

impl RemoteShardGenerator {
    fn new(shard_id: String, endpoint: String, fallback: Arc<dyn WorldChunkGenerator>) -> Self {
        Self {
            shard_id,
            endpoint,
            fallback,
            failure_until: Mutex::new(None),
        }
    }

    fn in_failure_cooldown(&self) -> bool {
        let mut guard = self
            .failure_until
            .lock()
            .expect("remote shard cooldown poisoned");
        match *guard {
            Some(until) if Instant::now() < until => true,
            Some(_) => {
                *guard = None;
                false
            }
            None => false,
        }
    }

    fn mark_failure(&self, err: &WorldError) {
        *self
            .failure_until
            .lock()
            .expect("remote shard cooldown poisoned") =
            Some(Instant::now() + FAILURE_COOLDOWN);
        log::warn!(
            "remote cluster shard unavailable, falling back locally: shard={}, endpoint={}, cooldown_ms={}, error={err:#}",
            self.shard_id,
            self.endpoint,
            FAILURE_COOLDOWN.as_millis()
        );
    }

    fn request(
        &self,
        request: &ClusterRequest,
    ) -> Result<ClusterResponse> {
        if self.in_failure_cooldown() {
            return Err(WorldError::msg(format!(
                "remote shard is in failure cooldown: shard={}",
                self.shard_id
            )));
        }
        let started = Instant::now();
        let response = send_request(
            &self.endpoint,
            request,
            DEFAULT_CONNECT_TIMEOUT,
            DEFAULT_REQUEST_TIMEOUT,
        )?;
        let elapsed = started.elapsed();
        if elapsed >= SLOW_RESPONSE_THRESHOLD {
            log::debug!(
                "remote cluster shard slow response: shard={}, endpoint={}, elapsed_ms={:.2}",
                self.shard_id,
                self.endpoint,
                elapsed.as_secs_f64() * 1000.0
            );
        }
        Ok(response)
    }

    fn decode_chunk(
        &self,
        dimension: &str,
        chunk_x: i32,
        chunk_z: i32,
        light_algorithm: WorldLightAlgorithm,
        response: ClusterResponse,
    ) -> Result<GeneratedChunk> {
        match response {
            ClusterResponse::Chunk {
                chunk_x: returned_x,
                chunk_z: returned_z,
                level_chunk_with_light: map_chunk,
                light_dampening,
            } => {
                if returned_x != chunk_x || returned_z != chunk_z {
                    return Err(WorldError::msg(format!(
                        "remote shard returned wrong chunk: requested=({chunk_x},{chunk_z}), returned=({returned_x},{returned_z})"
                    )));
                }
                let mut bytes = BytesMut::from(map_chunk.as_slice());
                let mut reader = PacketReader::new(&mut bytes);
                let mut packet = LevelChunkWithLight::default();
                packet.deserialize(&mut reader)?;
                if light_dampening.len() != CHUNK_DAMPENING_LEN {
                    return Err(WorldError::msg(format!(
                        "remote shard returned invalid light dampening length: shard={}, len={}",
                        self.shard_id,
                        light_dampening.len()
                    )));
                }
                Ok(GeneratedChunk {
                    packet,
                    light_dampening,
                    region_chunk: None,
                })
            }
            ClusterResponse::Error(message) => {
                return Err(WorldError::msg(format!("remote shard error: {message}")));
            }
            _ => self
                .fallback
                .generate(dimension, chunk_x, chunk_z, light_algorithm),
        }
    }
}

impl WorldChunkGenerator for RemoteShardGenerator {
    fn generate(
        &self,
        dimension: &str,
        chunk_x: i32,
        chunk_z: i32,
        light_algorithm: WorldLightAlgorithm,
    ) -> Result<GeneratedChunk> {
        let request = ClusterRequest::LoadChunk {
            dimension: dimension.to_string(),
            chunk_x,
            chunk_z,
            light_algorithm: light_algorithm.into(),
        };
        match self.request(&request).and_then(|response| {
            self.decode_chunk(dimension, chunk_x, chunk_z, light_algorithm, response)
        }) {
            Ok(chunk) => Ok(chunk),
            Err(err) => {
                self.mark_failure(&err);
                self.fallback
                    .generate(dimension, chunk_x, chunk_z, light_algorithm)
            }
        }
    }

    fn light_dampening(
        &self,
        dimension: &str,
        chunk_x: i32,
        chunk_z: i32,
        light_algorithm: WorldLightAlgorithm,
    ) -> Result<Vec<u8>> {
        Ok(self
            .generate(dimension, chunk_x, chunk_z, light_algorithm)?
            .light_dampening)
    }

    fn block_state_at(&self, dimension: &str, position: &Position) -> Option<i32> {
        let request = ClusterRequest::BlockStateAt {
            dimension: dimension.to_string(),
            x: position.x,
            y: position.y,
            z: position.z,
        };
        match self.request(&request) {
            Ok(ClusterResponse::BlockState(value)) => value,
            Ok(ClusterResponse::Error(message)) => {
                let err = WorldError::msg(message);
                self.mark_failure(&err);
                self.fallback.block_state_at(dimension, position)
            }
            Ok(_) => self.fallback.block_state_at(dimension, position),
            Err(err) => {
                self.mark_failure(&err);
                self.fallback.block_state_at(dimension, position)
            }
        }
    }

    fn region_chunk(
        &self,
        dimension: &str,
        chunk_x: i32,
        chunk_z: i32,
    ) -> Result<Option<super::region::ChunkData>> {
        self.fallback.region_chunk(dimension, chunk_x, chunk_z)
    }
}

impl WorldChunkGenerator for ClusteredWorldGenerator {
    fn generate(
        &self,
        dimension: &str,
        chunk_x: i32,
        chunk_z: i32,
        light_algorithm: WorldLightAlgorithm,
    ) -> Result<GeneratedChunk> {
        let Some((shard_id, generator)) = self.shard_for(chunk_x, chunk_z) else {
            return self
                .local
                .generate(dimension, chunk_x, chunk_z, light_algorithm);
        };
        self.route_log
            .lock()
            .expect("cluster route log poisoned")
            .push(ShardRoute {
                shard_id: shard_id.to_string(),
                dimension: dimension.to_string(),
                chunk_x,
                chunk_z,
            });
        generator.generate(dimension, chunk_x, chunk_z, light_algorithm)
    }

    fn light_dampening(
        &self,
        dimension: &str,
        chunk_x: i32,
        chunk_z: i32,
        light_algorithm: WorldLightAlgorithm,
    ) -> Result<Vec<u8>> {
        let Some((_shard_id, generator)) = self.shard_for(chunk_x, chunk_z) else {
            return self
                .local
                .light_dampening(dimension, chunk_x, chunk_z, light_algorithm);
        };
        generator.light_dampening(dimension, chunk_x, chunk_z, light_algorithm)
    }

    fn block_state_at(
        &self,
        dimension: &str,
        position: &qexed_packet::net_types::Position,
    ) -> Option<i32> {
        let chunk_x = position.x.div_euclid(16);
        let chunk_z = position.z.div_euclid(16);
        let Some((_shard_id, generator)) = self.shard_for(chunk_x, chunk_z) else {
            return self.local.block_state_at(dimension, position);
        };
        generator.block_state_at(dimension, position)
    }

    fn region_chunk(
        &self,
        dimension: &str,
        chunk_x: i32,
        chunk_z: i32,
    ) -> Result<Option<super::region::ChunkData>> {
        let Some((_shard_id, generator)) = self.shard_for(chunk_x, chunk_z) else {
            return self.local.region_chunk(dimension, chunk_x, chunk_z);
        };
        generator.region_chunk(dimension, chunk_x, chunk_z)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ClusterRouter {
    regions: Vec<ClusterRegion>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ClusterRegion {
    shard_id: String,
    min_chunk_x: Option<i32>,
    max_chunk_x: Option<i32>,
    min_chunk_z: Option<i32>,
    max_chunk_z: Option<i32>,
}

impl ClusterRegion {
    fn contains(&self, chunk_x: i32, chunk_z: i32) -> bool {
        self.min_chunk_x.is_none_or(|min| chunk_x >= min)
            && self.max_chunk_x.is_none_or(|max| chunk_x <= max)
            && self.min_chunk_z.is_none_or(|min| chunk_z >= min)
            && self.max_chunk_z.is_none_or(|max| chunk_z <= max)
    }
}

impl ClusterRouter {
    pub(crate) fn from_config(config: &WorldCluster) -> Option<Self> {
        match config.mode {
            WorldClusterMode::Quadrant => Self::quadrant(&config.shards),
            WorldClusterMode::Regions => Self::regions(&config.shards),
        }
    }

    pub(crate) fn quadrant(shards: &[WorldClusterShard]) -> Option<Self> {
        let mut regions = Vec::new();
        let mut seen = HashSet::new();
        for shard in shards {
            let id = clean_id(&shard.id)?;
            if !seen.insert(id.clone()) {
                return None;
            }
            let x = shard.x?;
            let z = shard.z?;
            let region = ClusterRegion {
                shard_id: id,
                min_chunk_x: matches!(x, WorldClusterAxisSide::Positive).then_some(0),
                max_chunk_x: matches!(x, WorldClusterAxisSide::Negative).then_some(-1),
                min_chunk_z: matches!(z, WorldClusterAxisSide::Positive).then_some(0),
                max_chunk_z: matches!(z, WorldClusterAxisSide::Negative).then_some(-1),
            };
            regions.push(region);
        }
        let router = Self { regions };
        (router.regions.len() == 4 && router.covers_quadrants()).then_some(router)
    }

    pub(crate) fn regions(shards: &[WorldClusterShard]) -> Option<Self> {
        let mut regions = Vec::new();
        let mut seen = HashSet::new();
        for shard in shards {
            let id = clean_id(&shard.id)?;
            if !seen.insert(id.clone()) {
                return None;
            }
            if let (Some(min), Some(max)) = (shard.min_chunk_x, shard.max_chunk_x)
                && min > max
            {
                return None;
            }
            if let (Some(min), Some(max)) = (shard.min_chunk_z, shard.max_chunk_z)
                && min > max
            {
                return None;
            }
            regions.push(ClusterRegion {
                shard_id: id,
                min_chunk_x: shard.min_chunk_x,
                max_chunk_x: shard.max_chunk_x,
                min_chunk_z: shard.min_chunk_z,
                max_chunk_z: shard.max_chunk_z,
            });
        }
        (!regions.is_empty()).then_some(Self { regions })
    }

    pub(crate) fn route(&self, chunk_x: i32, chunk_z: i32) -> Option<&str> {
        self.regions
            .iter()
            .find(|region| region.contains(chunk_x, chunk_z))
            .map(|region| region.shard_id.as_str())
    }

    fn shard_ids(&self) -> Vec<&str> {
        self.regions
            .iter()
            .map(|region| region.shard_id.as_str())
            .collect()
    }

    pub(crate) fn require_quadrant(shards: &[WorldClusterShard]) -> Result<Self> {
        Self::quadrant(shards)
            .ok_or_else(|| WorldError::msg("world cluster quadrant mode requires four unique shards"))
    }

    pub(crate) fn require_regions(shards: &[WorldClusterShard]) -> Result<Self> {
        Self::regions(shards)
            .ok_or_else(|| WorldError::msg("world cluster regions mode requires at least one valid shard"))
    }

    fn covers_quadrants(&self) -> bool {
        [(-1, -1), (-1, 0), (0, -1), (0, 0)]
            .into_iter()
            .filter_map(|(x, z)| self.route(x, z))
            .collect::<HashSet<_>>()
            .len()
            == 4
    }
}

fn clean_id(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{WorldCluster, WorldClusterAxisSide, WorldClusterMode, WorldClusterShard};

    fn region_shard(
        id: &str,
        min_chunk_x: Option<i32>,
        max_chunk_x: Option<i32>,
        min_chunk_z: Option<i32>,
        max_chunk_z: Option<i32>,
    ) -> WorldClusterShard {
        WorldClusterShard {
            id: id.to_string(),
            endpoint: String::new(),
            x: None,
            z: None,
            min_chunk_x,
            max_chunk_x,
            min_chunk_z,
            max_chunk_z,
        }
    }

    #[test]
    fn clustered_router_routes_arbitrary_regions() {
        // v4 clustered_world_generator_routes_arbitrary_regions 的 Router 部分。
        let shards = vec![
            region_shard("far", Some(9), None, Some(9), None),
            region_shard("spawn", Some(0), Some(1), Some(0), Some(1)),
            region_shard("east", Some(2), Some(8), Some(0), Some(8)),
            region_shard("north", Some(0), Some(1), Some(2), Some(8)),
            region_shard("west", None, Some(-1), None, None),
            region_shard("south", Some(0), None, None, Some(-1)),
        ];
        let router = ClusterRouter::require_regions(&shards).unwrap();

        assert_eq!(router.route(0, 0), Some("spawn"));
        assert_eq!(router.route(4, 4), Some("east"));
        assert_eq!(router.route(0, 4), Some("north"));
        assert_eq!(router.route(12, 12), Some("far"));
        assert_eq!(router.route(-3, 2), Some("west"));
        assert_eq!(router.route(2, -3), Some("south"));
    }

    #[test]
    fn quadrant_router_requires_four_unique_sides() {
        let sides = [
            WorldClusterAxisSide::Negative,
            WorldClusterAxisSide::Positive,
        ];
        let mut shards = Vec::new();
        for &x in &sides {
            for &z in &sides {
                shards.push(WorldClusterShard {
                    id: format!("{x:?}_{z:?}"),
                    endpoint: String::new(),
                    x: Some(x),
                    z: Some(z),
                    min_chunk_x: None,
                    max_chunk_x: None,
                    min_chunk_z: None,
                    max_chunk_z: None,
                });
            }
        }
        let router = ClusterRouter::quadrant(&shards).unwrap();
        assert_eq!(router.route(-1, -1), Some("Negative_Negative"));
        assert_eq!(router.route(0, 0), Some("Positive_Positive"));

        // 只有三片 → 不构成四象限覆盖。
        shards.pop();
        assert!(ClusterRouter::quadrant(&shards).is_none());
    }

    #[test]
    fn from_config_dispatches_by_mode() {
        let mut config = WorldCluster {
            enable: true,
            mode: WorldClusterMode::Regions,
            shards: vec![region_shard("only", Some(0), Some(0), Some(0), Some(0))],
        };
        assert!(ClusterRouter::from_config(&config).is_some());

        config.mode = WorldClusterMode::Quadrant;
        assert!(ClusterRouter::from_config(&config).is_none());
    }
}

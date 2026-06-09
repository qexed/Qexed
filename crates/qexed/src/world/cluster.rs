use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
    time::Instant,
};

use anyhow::{Result, anyhow};
use bytes::BytesMut;
use qexed_config::app::qexed::server::{
    WorldCluster, WorldClusterAxisSide, WorldClusterMode, WorldClusterShard,
};
use qexed_packet::{Packet, PacketReader, net_types::Position};
use qexed_protocol::to_client::play::map_chunk::MapChunk;

use super::{
    CHUNK_DAMPENING_LEN, WorldLightAlgorithm,
    generator::{GeneratedChunk, WorldChunkGenerator},
};

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

    fn mark_failure(&self, err: &anyhow::Error) {
        *self
            .failure_until
            .lock()
            .expect("remote shard cooldown poisoned") =
            Some(Instant::now() + crate::cluster_rpc::FAILURE_COOLDOWN);
        log::warn!(
            "remote cluster shard unavailable, falling back locally: shard={}, endpoint={}, cooldown_ms={}, error={err:#}",
            self.shard_id,
            self.endpoint,
            crate::cluster_rpc::FAILURE_COOLDOWN.as_millis()
        );
    }

    fn request(
        &self,
        request: &crate::cluster_rpc::ClusterRequest,
    ) -> Result<crate::cluster_rpc::ClusterResponse> {
        if self.in_failure_cooldown() {
            anyhow::bail!(
                "remote shard is in failure cooldown: shard={}",
                self.shard_id
            );
        }
        let started = Instant::now();
        let response = crate::cluster_rpc::send_request(
            &self.endpoint,
            request,
            crate::cluster_rpc::DEFAULT_CONNECT_TIMEOUT,
            crate::cluster_rpc::DEFAULT_REQUEST_TIMEOUT,
        )?;
        let elapsed = started.elapsed();
        if elapsed >= crate::cluster_rpc::SLOW_RESPONSE_THRESHOLD {
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
        response: crate::cluster_rpc::ClusterResponse,
    ) -> Result<GeneratedChunk> {
        match response {
            crate::cluster_rpc::ClusterResponse::Chunk {
                chunk_x: returned_x,
                chunk_z: returned_z,
                map_chunk,
                light_dampening,
            } => {
                if returned_x != chunk_x || returned_z != chunk_z {
                    anyhow::bail!(
                        "remote shard returned wrong chunk: requested=({chunk_x},{chunk_z}), returned=({returned_x},{returned_z})"
                    );
                }
                let mut bytes = BytesMut::from(map_chunk.as_slice());
                let mut reader = PacketReader::new(&mut bytes);
                let mut packet = MapChunk::default();
                packet.deserialize(&mut reader)?;
                if light_dampening.len() != CHUNK_DAMPENING_LEN {
                    anyhow::bail!(
                        "remote shard returned invalid light dampening length: shard={}, len={}",
                        self.shard_id,
                        light_dampening.len()
                    );
                }
                Ok(GeneratedChunk {
                    packet,
                    light_dampening,
                    region_chunk: None,
                })
            }
            crate::cluster_rpc::ClusterResponse::Error(message) => {
                anyhow::bail!("remote shard error: {message}")
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
        let request = crate::cluster_rpc::ClusterRequest::LoadChunk {
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
        let request = crate::cluster_rpc::ClusterRequest::BlockStateAt {
            dimension: dimension.to_string(),
            x: position.x,
            y: position.y,
            z: position.z,
        };
        match self.request(&request) {
            Ok(crate::cluster_rpc::ClusterResponse::BlockState(value)) => value,
            Ok(crate::cluster_rpc::ClusterResponse::Error(message)) => {
                let err = anyhow!(message);
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
            .ok_or_else(|| anyhow!("world cluster quadrant mode requires four unique shards"))
    }

    pub(crate) fn require_regions(shards: &[WorldClusterShard]) -> Result<Self> {
        Self::regions(shards)
            .ok_or_else(|| anyhow!("world cluster regions mode requires at least one valid shard"))
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

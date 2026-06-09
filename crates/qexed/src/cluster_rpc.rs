use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpStream, ToSocketAddrs},
    time::Duration,
};

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};

pub(crate) const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_millis(750);
pub(crate) const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_millis(1500);
pub(crate) const SLOW_RESPONSE_THRESHOLD: Duration = Duration::from_millis(100);
pub(crate) const FAILURE_COOLDOWN: Duration = Duration::from_secs(2);

const MAX_FRAME_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug, Deserialize, Serialize)]
pub(crate) enum ClusterRequest {
    LoadChunk {
        dimension: String,
        chunk_x: i32,
        chunk_z: i32,
        light_algorithm: ClusterLightAlgorithm,
    },
    BlockStateAt {
        dimension: String,
        x: i32,
        y: i32,
        z: i32,
    },
    EntityView {
        players: Vec<ClusterPlayerSnapshot>,
        rendering: ClusterEntityRendering,
    },
    EntityTick {
        players: Vec<ClusterPlayerSnapshot>,
        rendering: ClusterEntityRendering,
        spawning: ClusterEntitySpawning,
        default_dimension: String,
        ai_tick_interval_ms: u64,
    },
    DamageEntity {
        players: Vec<ClusterPlayerSnapshot>,
        rendering: ClusterEntityRendering,
        target_entity_id: i32,
        attacker_entity_id: i32,
        attacker_x: f64,
        attacker_y: f64,
        attacker_z: f64,
        damage: f32,
        knockback: f32,
    },
    DropItem {
        players: Vec<ClusterPlayerSnapshot>,
        rendering: ClusterEntityRendering,
        actor_profile_id: [u8; 16],
        dimension: String,
        x: f64,
        y: f64,
        z: f64,
        yaw: f32,
        pitch: f32,
        on_ground: bool,
        item: Vec<u8>,
    },
    CollectItems {
        players: Vec<ClusterPlayerSnapshot>,
        collector_profile_id: [u8; 16],
        collector_entity_id: i32,
        dimension: String,
        x: f64,
        y: f64,
        z: f64,
        yaw: f32,
        pitch: f32,
        on_ground: bool,
    },
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) enum ClusterResponse {
    Chunk {
        chunk_x: i32,
        chunk_z: i32,
        map_chunk: Vec<u8>,
        light_dampening: Vec<u8>,
    },
    BlockState(Option<i32>),
    EntityPackets(Vec<ClusterPacketBatch>),
    EntityDamage {
        handled: bool,
        killed: bool,
        packets: Vec<ClusterPacketBatch>,
    },
    CollectedItems {
        items: Vec<ClusterCollectedItem>,
        packets: Vec<ClusterPacketBatch>,
    },
    Error(String),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct ClusterCollectedItem {
    pub item: Vec<u8>,
    pub count: i32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct ClusterPlayerSnapshot {
    pub profile_id: [u8; 16],
    pub entity_id: i32,
    pub username: String,
    pub dimension: String,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub yaw: f32,
    pub pitch: f32,
    pub on_ground: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct ClusterPacketBatch {
    pub profile_id: [u8; 16],
    pub packets: Vec<Vec<u8>>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct ClusterEntityRendering {
    pub default_distance: f64,
    pub player_distance: f64,
    pub npc_distance: f64,
    pub hologram_distance: f64,
    pub item_distance: f64,
    pub item_merge_radius: f64,
    pub item_merge_max_stack: i32,
    pub stack_threshold: usize,
    pub stack_radius: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct ClusterEntitySpawning {
    pub enable: bool,
    pub tick_interval_ms: u64,
    pub global_cap: usize,
    pub per_dimension_cap: usize,
    pub per_type_cap: usize,
    pub max_spawn_per_tick: usize,
    pub player_activation_range: f64,
    pub rules: Vec<qexed_config::app::qexed::server::EntitySpawnRule>,
}

impl From<&qexed_config::app::qexed::server::EntityRendering> for ClusterEntityRendering {
    fn from(value: &qexed_config::app::qexed::server::EntityRendering) -> Self {
        Self {
            default_distance: value.default_distance,
            player_distance: value.player_distance,
            npc_distance: value.npc_distance,
            hologram_distance: value.hologram_distance,
            item_distance: value.item_distance,
            item_merge_radius: value.item_merge_radius,
            item_merge_max_stack: value.item_merge_max_stack,
            stack_threshold: value.stack_threshold,
            stack_radius: value.stack_radius,
        }
    }
}

impl From<ClusterEntityRendering> for qexed_config::app::qexed::server::EntityRendering {
    fn from(value: ClusterEntityRendering) -> Self {
        Self {
            default_distance: value.default_distance,
            player_distance: value.player_distance,
            npc_distance: value.npc_distance,
            hologram_distance: value.hologram_distance,
            item_distance: value.item_distance,
            item_merge_radius: value.item_merge_radius,
            item_merge_max_stack: value.item_merge_max_stack,
            stack_threshold: value.stack_threshold,
            stack_radius: value.stack_radius,
        }
    }
}

impl From<&qexed_config::app::qexed::server::EntitySpawning> for ClusterEntitySpawning {
    fn from(value: &qexed_config::app::qexed::server::EntitySpawning) -> Self {
        Self {
            enable: value.enable,
            tick_interval_ms: value.tick_interval_ms,
            global_cap: value.global_cap,
            per_dimension_cap: value.per_dimension_cap,
            per_type_cap: value.per_type_cap,
            max_spawn_per_tick: value.max_spawn_per_tick,
            player_activation_range: value.player_activation_range,
            rules: value.rules.clone(),
        }
    }
}

impl From<ClusterEntitySpawning> for qexed_config::app::qexed::server::EntitySpawning {
    fn from(value: ClusterEntitySpawning) -> Self {
        Self {
            enable: value.enable,
            tick_interval_ms: value.tick_interval_ms,
            ai_tick_interval_ms: 50,
            global_cap: value.global_cap,
            per_dimension_cap: value.per_dimension_cap,
            per_type_cap: value.per_type_cap,
            max_spawn_per_tick: value.max_spawn_per_tick,
            player_activation_range: value.player_activation_range,
            rules: value.rules,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub(crate) enum ClusterLightAlgorithm {
    Fast,
    RayTrace,
}

impl From<crate::world::WorldLightAlgorithm> for ClusterLightAlgorithm {
    fn from(value: crate::world::WorldLightAlgorithm) -> Self {
        match value {
            crate::world::WorldLightAlgorithm::Fast => Self::Fast,
            crate::world::WorldLightAlgorithm::RayTrace => Self::RayTrace,
        }
    }
}

impl From<ClusterLightAlgorithm> for crate::world::WorldLightAlgorithm {
    fn from(value: ClusterLightAlgorithm) -> Self {
        match value {
            ClusterLightAlgorithm::Fast => Self::Fast,
            ClusterLightAlgorithm::RayTrace => Self::RayTrace,
        }
    }
}

pub(crate) fn send_request(
    endpoint: &str,
    request: &ClusterRequest,
    connect_timeout: Duration,
    request_timeout: Duration,
) -> Result<ClusterResponse> {
    let addr = endpoint_addr(endpoint)?;
    let mut stream = TcpStream::connect_timeout(&addr, connect_timeout)
        .with_context(|| format!("cluster shard connect failed: endpoint={endpoint}"))?;
    stream.set_read_timeout(Some(request_timeout))?;
    stream.set_write_timeout(Some(request_timeout))?;
    write_frame(&mut stream, request)?;
    read_frame(&mut stream)
}

pub(crate) fn write_frame<T: Serialize>(writer: &mut impl Write, value: &T) -> Result<()> {
    let payload = postcard::to_allocvec(value)?;
    if payload.len() > MAX_FRAME_BYTES {
        anyhow::bail!("cluster frame too large: {} bytes", payload.len());
    }
    let len = u32::try_from(payload.len())?.to_be_bytes();
    writer.write_all(&len)?;
    writer.write_all(&payload)?;
    writer.flush()?;
    Ok(())
}

pub(crate) fn read_frame<T: for<'de> Deserialize<'de>>(reader: &mut impl Read) -> Result<T> {
    let mut len = [0_u8; 4];
    reader.read_exact(&mut len)?;
    let len = u32::from_be_bytes(len) as usize;
    if len > MAX_FRAME_BYTES {
        anyhow::bail!("cluster frame too large: {len} bytes");
    }
    let mut payload = vec![0; len];
    reader.read_exact(&mut payload)?;
    Ok(postcard::from_bytes(&payload)?)
}

pub(crate) fn endpoint_addr(endpoint: &str) -> Result<SocketAddr> {
    let raw = endpoint
        .strip_prefix("tcp://")
        .ok_or_else(|| anyhow!("unsupported cluster endpoint: {endpoint}"))?;
    raw.to_socket_addrs()?
        .next()
        .ok_or_else(|| anyhow!("cluster endpoint did not resolve: {endpoint}"))
}

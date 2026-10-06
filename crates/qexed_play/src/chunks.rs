//! 区块发送状态机（v4 play/chunks.rs 迁移）。
//!
//! v6 适配：
//! - PacketSink/PacketStream 改用 qexed_connection::transport（v4 qexed_tcp_connect）
//! - WorldManager 改为 `Arc<dyn WorldChunkSource>`（crate::context；v4 直接持有
//!   可克隆的 WorldManager，v6 world 域未定型，经 trait 注入）
//! - MapChunk -> LevelChunkWithLight（26.3 改名；字段 x/z 为普通 i32、
//!   chunk_data/light_data 为子结构）
//! - UpdateViewPosition -> SetChunkCacheCenter
//! - anyhow 链式错误 -> PlayError::msg 消息匹配（过期会话判定）
//!
//! 帧缓存拆分（v4 precompiled 逻辑保留）：不含光照的前缀缓存 + 光照部分每次现编。

use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::{Arc, OnceLock, mpsc},
    time::{Duration, Instant},
};

use bytes::{Bytes, BytesMut};
use qexed_connection::transport::PacketSink;
use qexed_packet::{Packet, PacketCodec};
use qexed_protocol::to_client::play::{
    chunk_batch_finished::ChunkBatchFinished,
    chunk_batch_start::ChunkBatchStart,
    forget_level_chunk::ForgetLevelChunk,
    level_chunk_with_light::LevelChunkWithLight,
    set_chunk_cache_center::SetChunkCacheCenter,
};

use crate::context::{PluginEventSink, WorldChunkSource};
use crate::error::{PlayError, Result};
use crate::util::chunk_coord;

const CHUNK_UNLOAD_DELAY: Duration = Duration::from_secs(4);
const DEFAULT_CHUNK_LOAD_PARALLELISM: usize = 4;
const MAX_CHUNK_LOAD_PARALLELISM: usize = 64;
const SLOW_CHUNK_PAYLOAD_LOG_THRESHOLD: Duration = Duration::from_millis(250);
const MIN_CHUNKS_PER_TICK: f32 = 1.0;
const MAX_CHUNKS_PER_TICK: f32 = 128.0;
const START_CHUNKS_PER_TICK: f32 = 64.0;
const MAX_CHUNKS_PER_SEND_BATCH: usize = 32;
const INITIAL_MAX_UNACKNOWLEDGED_BATCHES: usize = 1;
const MAX_UNACKNOWLEDGED_BATCHES: usize = 10;
const MAX_CHUNK_LOAD_THREADS: usize = 4;
const MAX_CHUNK_GENERATE_THREADS: usize = 8;
const DEFAULT_CHUNK_CENTER_UPDATE_DELAY: Duration = Duration::from_secs(1);
const CHUNK_PAYLOAD_TIMING_LOG_THRESHOLD: Duration = Duration::from_millis(100);

#[cfg(test)]
pub const DEFAULT_PARALLELISM_FOR_TESTS: usize = DEFAULT_CHUNK_LOAD_PARALLELISM;
#[cfg(test)]
pub const MAX_PARALLELISM_FOR_TESTS: usize = MAX_CHUNK_LOAD_PARALLELISM;

pub type FluidSeed = (qexed_packet::net_types::Position, i32);
pub type SharedWorld = Arc<dyn WorldChunkSource>;

pub struct ChunkSendState {
    pub dimension: String,
    pub center_x: i32,
    pub center_z: i32,
    pub view_distance: i32,
    pub load_parallelism: usize,
    pub visible_chunks: HashSet<(i32, i32)>,
    pub pending_chunks: VecDeque<(i32, i32)>,
    pub ready_chunks: VecDeque<ChunkLoadResult>,
    pub pending_unloads: HashMap<(i32, i32), Instant>,
    pub loading_chunks: HashSet<(i32, i32)>,
    pending_center_update: Option<PendingCenterUpdate>,
    center_update_delay: Duration,
    pub desired_chunks_per_tick: f32,
    pub batch_quota: f32,
    pub unacknowledged_batches: usize,
    pub max_unacknowledged_batches: usize,
    initial_view_logged: bool,
}

pub struct ChunkLoadResult {
    pub chunk_x: i32,
    pub chunk_z: i32,
    pub fluid_seeds: Vec<FluidSeed>,
    frame: std::result::Result<Bytes, String>,
}

pub struct ChunkPayloadResult {
    frame: Bytes,
    fluid_seeds: Vec<FluidSeed>,
}

#[derive(Clone, Copy, Debug)]
struct PendingCenterUpdate {
    origin_x: i32,
    origin_z: i32,
    due_at: Instant,
}

#[derive(Clone, Debug)]
struct ChunkTaskPool {
    sender: mpsc::Sender<ChunkTask>,
}

impl ChunkTaskPool {
    fn shared() -> &'static Self {
        static POOL: OnceLock<ChunkTaskPool> = OnceLock::new();
        POOL.get_or_init(|| {
            let load_worker_count = default_chunk_load_threads();
            let generate_worker_count = default_chunk_generate_threads();
            let (sender, receiver) = mpsc::channel::<ChunkTask>();
            let (generate_sender, generate_receiver) = mpsc::channel::<ChunkGenerateTask>();
            let receiver = Arc::new(std::sync::Mutex::new(receiver));
            let generate_receiver = Arc::new(std::sync::Mutex::new(generate_receiver));
            for index in 0..load_worker_count {
                let receiver = receiver.clone();
                let generate_sender = generate_sender.clone();
                std::thread::Builder::new()
                    .name(format!("qexed-chunk-load-{index}"))
                    .spawn(move || {
                        loop {
                            let task =
                                { receiver.lock().expect("chunk task queue poisoned").recv() };
                            match task {
                                Ok(task) => task.run(&generate_sender),
                                Err(_) => break,
                            }
                        }
                    })
                    .expect("create chunk task worker");
            }
            for index in 0..generate_worker_count {
                let generate_receiver = generate_receiver.clone();
                std::thread::Builder::new()
                    .name(format!("qexed-chunk-generate-{index}"))
                    .spawn(move || {
                        loop {
                            let task = {
                                generate_receiver
                                    .lock()
                                    .expect("chunk generate queue poisoned")
                                    .recv()
                            };
                            match task {
                                Ok(task) => task.run(),
                                Err(_) => break,
                            }
                        }
                    })
                    .expect("create chunk generate worker");
            }
            log::info!(
                "{}",
                qexed_language::t("qexed.play.chunk.pools_init")
                    .replace("%{load}", &load_worker_count.to_string())
                    .replace("%{generate}", &generate_worker_count.to_string())
            );
            Self { sender }
        })
    }

    async fn build_payload(
        &self,
        world: SharedWorld,
        dimension: String,
        chunk_x: i32,
        chunk_z: i32,
        cache_epoch: u64,
        compression_threshold: Option<i32>,
    ) -> Result<ChunkPayloadResult> {
        let (sender, receiver) = tokio::sync::oneshot::channel();
        self.sender
            .send(ChunkTask::BuildPayload {
                world,
                dimension,
                chunk_x,
                chunk_z,
                cache_epoch,
                compression_threshold,
                reply: ChunkTaskReply::OneShot(sender),
            })
            .map_err(|_| PlayError::msg("queue chunk packet build task"))?;
        receiver
            .await
            .map_err(|_| PlayError::msg("join chunk packet build task"))?
    }

    fn spawn_payload(
        &self,
        world: SharedWorld,
        dimension: String,
        chunk_x: i32,
        chunk_z: i32,
        cache_epoch: u64,
        compression_threshold: Option<i32>,
        sender: tokio::sync::mpsc::UnboundedSender<ChunkLoadResult>,
    ) {
        if self
            .sender
            .send(ChunkTask::BuildPayload {
                world,
                dimension,
                chunk_x,
                chunk_z,
                cache_epoch,
                compression_threshold,
                reply: ChunkTaskReply::Channel(sender),
            })
            .is_err()
        {
            log::warn!("{}", qexed_language::t("qexed.play.chunk.pool_stopped"));
        }
    }
}

enum ChunkTask {
    BuildPayload {
        world: SharedWorld,
        dimension: String,
        chunk_x: i32,
        chunk_z: i32,
        cache_epoch: u64,
        compression_threshold: Option<i32>,
        reply: ChunkTaskReply,
    },
}

impl ChunkTask {
    fn run(self, generate_sender: &mpsc::Sender<ChunkGenerateTask>) {
        match self {
            ChunkTask::BuildPayload {
                world,
                dimension,
                chunk_x,
                chunk_z,
                cache_epoch,
                compression_threshold,
                reply,
            } => {
                let started_at = Instant::now();
                let loaded = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    build_saved_chunk_payload_sync(
                        world.as_ref(),
                        &dimension,
                        chunk_x,
                        chunk_z,
                        cache_epoch,
                        compression_threshold,
                    )
                }));
                log_chunk_payload_timing("saved", &dimension, chunk_x, chunk_z, started_at);
                match loaded {
                    Ok(Ok(Some(payload))) => reply.send(chunk_x, chunk_z, Ok(payload)),
                    Ok(Ok(None)) => {
                        let task = ChunkGenerateTask::BuildPayload {
                            world,
                            dimension,
                            chunk_x,
                            chunk_z,
                            cache_epoch,
                            compression_threshold,
                            reply,
                        };
                        if let Err(err) = generate_sender.send(task) {
                            let ChunkGenerateTask::BuildPayload {
                                chunk_x,
                                chunk_z,
                                reply,
                                ..
                            } = err.0;
                            reply.send(
                                chunk_x,
                                chunk_z,
                                Err("failed to queue chunk generation task because generation pool stopped".to_string()),
                            );
                        }
                    }
                    Ok(Err(err)) => reply.send(chunk_x, chunk_z, Err(err_message(&err))),
                    Err(panic) => reply.send(
                        chunk_x,
                        chunk_z,
                        Err(format!(
                            "chunk payload load task panicked: {}",
                            panic_payload_message(panic)
                        )),
                    ),
                }
            }
        }
    }
}

enum ChunkGenerateTask {
    BuildPayload {
        world: SharedWorld,
        dimension: String,
        chunk_x: i32,
        chunk_z: i32,
        cache_epoch: u64,
        compression_threshold: Option<i32>,
        reply: ChunkTaskReply,
    },
}

impl ChunkGenerateTask {
    fn run(self) {
        match self {
            ChunkGenerateTask::BuildPayload {
                world,
                dimension,
                chunk_x,
                chunk_z,
                cache_epoch,
                compression_threshold,
                reply,
            } => {
                let started_at = Instant::now();
                let log_dimension = dimension.clone();
                let payload = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    build_generated_chunk_payload_sync(
                        world,
                        dimension,
                        chunk_x,
                        chunk_z,
                        cache_epoch,
                        compression_threshold,
                    )
                }))
                .unwrap_or_else(|panic| {
                    Err(PlayError::msg(format!(
                        "chunk payload generation task panicked: {}",
                        panic_payload_message(panic)
                    )))
                });
                match &payload {
                    Ok(_) => log_chunk_payload_timing("generated", &log_dimension, chunk_x, chunk_z, started_at),
                    Err(_) => log_chunk_payload_timing("generated_error", &log_dimension, chunk_x, chunk_z, started_at),
                }
                reply.send(chunk_x, chunk_z, payload.map_err(|err| err_message(&err)));
            }
        }
    }
}

fn err_message(err: &PlayError) -> String {
    err.to_string()
}

fn log_chunk_payload_timing(
    kind: &str,
    dimension: &str,
    chunk_x: i32,
    chunk_z: i32,
    started_at: Instant,
) {
    let elapsed = started_at.elapsed();
    if elapsed >= CHUNK_PAYLOAD_TIMING_LOG_THRESHOLD {
        log::warn!(
            "{}",
            qexed_language::t("qexed.play.chunk.slow_build")
                .replace("%{kind}", kind)
                .replace("%{dimension}", dimension)
                .replace("%{x}", &chunk_x.to_string())
                .replace("%{z}", &chunk_z.to_string())
                .replace("%{ms}", &elapsed.as_millis().to_string())
        );
    } else {
        log::debug!(
            "chunk payload build: kind={kind}, dimension={dimension}, chunk=({chunk_x}, {chunk_z}), elapsed_ms={}",
            elapsed.as_millis()
        );
    }
}

fn panic_payload_message(panic: Box<dyn std::any::Any + Send>) -> String {
    if let Some(message) = panic.downcast_ref::<&'static str>() {
        return (*message).to_string();
    }
    if let Some(message) = panic.downcast_ref::<String>() {
        return message.clone();
    }
    "unknown panic payload".to_string()
}

enum ChunkTaskReply {
    OneShot(tokio::sync::oneshot::Sender<Result<ChunkPayloadResult>>),
    Channel(tokio::sync::mpsc::UnboundedSender<ChunkLoadResult>),
}

impl ChunkTaskReply {
    fn send(
        self,
        chunk_x: i32,
        chunk_z: i32,
        payload: std::result::Result<ChunkPayloadResult, String>,
    ) {
        match self {
            ChunkTaskReply::OneShot(sender) => {
                let _ = sender.send(payload.map_err(PlayError::msg));
            }
            ChunkTaskReply::Channel(sender) => {
                let (fluid_seeds, frame) = match payload {
                    Ok(payload) => (payload.fluid_seeds, Ok(payload.frame)),
                    Err(err) => (Vec::new(), Err(err)),
                };
                let _ = sender.send(ChunkLoadResult {
                    chunk_x,
                    chunk_z,
                    fluid_seeds,
                    frame,
                });
            }
        }
    }
}

impl ChunkSendState {
    pub fn new(
        dimension: String,
        center_x: i32,
        center_z: i32,
        view_distance: i32,
        load_parallelism: usize,
    ) -> Self {
        Self {
            dimension,
            center_x,
            center_z,
            view_distance: view_distance.max(1),
            load_parallelism: chunk_load_parallelism_limit(load_parallelism),
            visible_chunks: HashSet::new(),
            pending_chunks: VecDeque::new(),
            ready_chunks: VecDeque::new(),
            pending_unloads: HashMap::new(),
            loading_chunks: HashSet::new(),
            pending_center_update: None,
            center_update_delay: DEFAULT_CHUNK_CENTER_UPDATE_DELAY,
            desired_chunks_per_tick: START_CHUNKS_PER_TICK,
            batch_quota: 0.0,
            unacknowledged_batches: 0,
            max_unacknowledged_batches: INITIAL_MAX_UNACKNOWLEDGED_BATCHES,
            initial_view_logged: false,
        }
    }

    pub fn set_center_update_delay(&mut self, delay: Duration) {
        self.center_update_delay = delay;
    }

    pub async fn update_center<W>(
        &mut self,
        sink: &mut PacketSink<W>,
        chunk_sender: &tokio::sync::mpsc::UnboundedSender<ChunkLoadResult>,
        world: &SharedWorld,
        x: f64,
        z: f64,
    ) -> Result<()>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        let chunk_x = chunk_coord(x);
        let chunk_z = chunk_coord(z);
        if chunk_x == self.center_x && chunk_z == self.center_z {
            self.pending_center_update = None;
            return Ok(());
        }
        if self.center_update_delay.is_zero() {
            self.apply_center_update_to_chunk(sink, chunk_sender, world, chunk_x, chunk_z)
                .await?;
            return Ok(());
        }

        self.schedule_center_update();
        Ok(())
    }

    pub fn has_pending_center_update(&self) -> bool {
        self.pending_center_update.is_some()
    }

    pub fn pending_center_update_deadline(&self) -> Option<Instant> {
        self.pending_center_update
            .as_ref()
            .map(|pending| pending.due_at)
    }

    pub async fn apply_due_center_update<W>(
        &mut self,
        sink: &mut PacketSink<W>,
        chunk_sender: &tokio::sync::mpsc::UnboundedSender<ChunkLoadResult>,
        world: &SharedWorld,
        x: f64,
        z: f64,
    ) -> Result<bool>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        let Some(pending) = self.pending_center_update else {
            return Ok(false);
        };
        if pending.due_at > Instant::now() {
            return Ok(false);
        }
        self.pending_center_update = None;

        let chunk_x = chunk_coord(x);
        let chunk_z = chunk_coord(z);
        if chunk_x == self.center_x && chunk_z == self.center_z {
            return Ok(false);
        }

        if self.center_x == pending.origin_x
            && self.center_z == pending.origin_z
            && first_ring_neighbor(pending.origin_x, pending.origin_z, chunk_x, chunk_z)
        {
            self.apply_incremental_center_update(sink, chunk_sender, world, chunk_x, chunk_z)
                .await?;
            return Ok(true);
        }

        self.apply_center_update_to_chunk(sink, chunk_sender, world, chunk_x, chunk_z)
            .await?;
        Ok(true)
    }

    fn schedule_center_update(&mut self) {
        if self.pending_center_update.is_some() {
            return;
        }
        self.pending_center_update = Some(PendingCenterUpdate {
            origin_x: self.center_x,
            origin_z: self.center_z,
            due_at: Instant::now() + self.center_update_delay,
        });
    }

    async fn apply_center_update_to_chunk<W>(
        &mut self,
        sink: &mut PacketSink<W>,
        chunk_sender: &tokio::sync::mpsc::UnboundedSender<ChunkLoadResult>,
        world: &SharedWorld,
        chunk_x: i32,
        chunk_z: i32,
    ) -> Result<()>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        if chunk_x == self.center_x && chunk_z == self.center_z {
            return Ok(());
        }
        let next = visible_chunk_set(chunk_x, chunk_z, self.view_distance);
        sink.send(SetChunkCacheCenter {
            x: qexed_packet::net_types::VarInt(chunk_x),
            z: qexed_packet::net_types::VarInt(chunk_z),
        })
        .await?;

        self.center_x = chunk_x;
        self.center_z = chunk_z;
        self.mark_delayed_unloads(next, Instant::now() + CHUNK_UNLOAD_DELAY);
        self.refresh_pending_chunks();
        self.start_next_chunk_load(world, Some(chunk_sender), sink.compression_threshold());
        sink.flush().await?;
        log::debug!("chunk center update queued: center=({chunk_x}, {chunk_z})");
        Ok(())
    }

    async fn apply_incremental_center_update<W>(
        &mut self,
        sink: &mut PacketSink<W>,
        chunk_sender: &tokio::sync::mpsc::UnboundedSender<ChunkLoadResult>,
        world: &SharedWorld,
        chunk_x: i32,
        chunk_z: i32,
    ) -> Result<()>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        let Some((mut entering, leaving)) = incremental_chunk_delta(
            self.center_x,
            self.center_z,
            chunk_x,
            chunk_z,
            self.view_distance,
        ) else {
            self.apply_center_update_to_chunk(sink, chunk_sender, world, chunk_x, chunk_z)
                .await?;
            return Ok(());
        };

        sink.send(SetChunkCacheCenter {
            x: qexed_packet::net_types::VarInt(chunk_x),
            z: qexed_packet::net_types::VarInt(chunk_z),
        })
        .await?;

        self.center_x = chunk_x;
        self.center_z = chunk_z;

        let unload_at = Instant::now() + CHUNK_UNLOAD_DELAY;
        for chunk in &entering {
            self.pending_unloads.remove(chunk);
        }
        for chunk in &leaving {
            if self.visible_chunks.contains(chunk) {
                self.pending_unloads.entry(*chunk).or_insert(unload_at);
            }
        }

        self.retain_work_for_current_view();
        entering.sort_by_key(|(chunk_x, chunk_z)| {
            chunk_send_priority(self.center_x, self.center_z, *chunk_x, *chunk_z)
        });
        let entering_len = entering.len();
        for chunk in entering {
            if self.visible_chunks.contains(&chunk)
                || self.loading_chunks.contains(&chunk)
                || self.pending_chunks.contains(&chunk)
            {
                continue;
            }
            self.pending_chunks.push_back(chunk);
        }
        self.start_next_chunk_load(world, Some(chunk_sender), sink.compression_threshold());
        sink.flush().await?;
        log::debug!(
            "incremental chunk center update: center=({chunk_x}, {chunk_z}), entering={}, leaving={}",
            entering_len,
            leaving.len()
        );
        Ok(())
    }

    pub async fn reset_after_respawn<W>(
        &mut self,
        sink: &mut PacketSink<W>,
        chunk_sender: &tokio::sync::mpsc::UnboundedSender<ChunkLoadResult>,
        world: &SharedWorld,
        plugins: &dyn PluginEventSink,
        x: f64,
        z: f64,
    ) -> Result<Vec<FluidSeed>>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        let chunk_x = chunk_coord(x);
        let chunk_z = chunk_coord(z);
        sink.send(SetChunkCacheCenter {
            x: qexed_packet::net_types::VarInt(chunk_x),
            z: qexed_packet::net_types::VarInt(chunk_z),
        })
        .await?;

        self.reset_view(chunk_x, chunk_z);
        let fluid_seeds = self.send_center_chunk_first(sink, world, plugins).await?;
        self.start_next_chunk_load(world, Some(chunk_sender), sink.compression_threshold());
        sink.flush().await?;
        log::debug!("respawn chunk view reset: center=({chunk_x}, {chunk_z})");
        Ok(fluid_seeds)
    }

    pub async fn reset_dimension_after_respawn<W>(
        &mut self,
        sink: &mut PacketSink<W>,
        chunk_sender: &tokio::sync::mpsc::UnboundedSender<ChunkLoadResult>,
        world: &SharedWorld,
        plugins: &dyn PluginEventSink,
        dimension: impl Into<String>,
        x: f64,
        z: f64,
    ) -> Result<Vec<FluidSeed>>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        self.dimension = dimension.into();
        self.reset_after_respawn(sink, chunk_sender, world, plugins, x, z)
            .await
    }

    pub fn reset_view(&mut self, center_x: i32, center_z: i32) {
        self.center_x = center_x;
        self.center_z = center_z;
        self.pending_center_update = None;
        self.visible_chunks.clear();
        self.pending_chunks.clear();
        self.ready_chunks.clear();
        self.pending_unloads.clear();
        self.loading_chunks.clear();
        self.reset_batch_flow_control();
        self.initial_view_logged = false;
        self.refresh_pending_chunks();
    }

    pub async fn send_center_chunk_first<W>(
        &mut self,
        sink: &mut PacketSink<W>,
        world: &SharedWorld,
        plugins: &dyn PluginEventSink,
    ) -> Result<Vec<FluidSeed>>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        let chunk = (self.center_x, self.center_z);
        if self.visible_chunks.contains(&chunk) {
            return Ok(Vec::new());
        }

        self.remove_pending_chunk(chunk);
        self.loading_chunks.remove(&chunk);

        let cache_epoch = world.cache_epoch();
        log::info!(
            "center chunk payload build started: dimension={}, chunk=({}, {})",
            self.dimension,
            chunk.0,
            chunk.1
        );
        let chunk_payload = ChunkTaskPool::shared()
            .build_payload(
                world.clone(),
                self.dimension.clone(),
                chunk.0,
                chunk.1,
                cache_epoch,
                sink.compression_threshold(),
            )
            .await?;
        log::info!(
            "center chunk payload build completed: dimension={}, chunk=({}, {}), bytes={}",
            self.dimension,
            chunk.0,
            chunk.1,
            chunk_payload.frame.len()
        );

        log::info!(
            "center chunk send started: dimension={}, chunk=({}, {})",
            self.dimension,
            chunk.0,
            chunk.1
        );
        sink.send(ChunkBatchStart {}).await?;
        sink.send_encoded_frame(chunk_payload.frame).await?;
        for update in world.placed_block_updates(&self.dimension, chunk.0, chunk.1) {
            sink.send(
                qexed_protocol::to_client::play::block_update::BlockUpdate {
                    location: update.location,
                    block_state: qexed_packet::net_types::VarInt(update.block_state),
                },
            )
            .await?;
        }
        self.visible_chunks.insert(chunk);
        plugins.emit_chunk_load(&self.dimension, chunk.0, chunk.1);
        sink.send(ChunkBatchFinished {
            batch_size: qexed_packet::net_types::VarInt(1),
        })
        .await?;
        sink.flush().await?;
        log::info!(
            "center chunk send completed: dimension={}, chunk=({}, {})",
            self.dimension,
            chunk.0,
            chunk.1
        );
        self.log_initial_view_progress();
        Ok(chunk_payload.fluid_seeds)
    }

    pub async fn send_missing_chunks<W>(
        &mut self,
        sink: &mut PacketSink<W>,
        world: &SharedWorld,
        cache_epoch: u64,
    ) -> Result<Vec<FluidSeed>>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        let chunks = self.missing_chunks();
        if chunks.is_empty() {
            return Ok(Vec::new());
        }

        let mut fluid_seeds = Vec::new();
        sink.send(ChunkBatchStart {}).await?;
        for (chunk_x, chunk_z) in &chunks {
            let chunk_x = *chunk_x;
            let chunk_z = *chunk_z;
            self.remove_pending_chunk((chunk_x, chunk_z));
            self.loading_chunks.remove(&(chunk_x, chunk_z));
            let chunk_payload = ChunkTaskPool::shared()
                .build_payload(
                    world.clone(),
                    self.dimension.clone(),
                    chunk_x,
                    chunk_z,
                    cache_epoch,
                    sink.compression_threshold(),
                )
                .await?;
            sink.send_encoded_frame(chunk_payload.frame).await?;
            fluid_seeds.extend(chunk_payload.fluid_seeds);
        }
        sink.send(ChunkBatchFinished {
            batch_size: qexed_packet::net_types::VarInt(chunks.len() as i32),
        })
        .await?;
        sink.flush().await?;
        self.log_initial_view_progress();
        Ok(fluid_seeds)
    }

    pub async fn send_remaining_initial_chunks<W>(
        &mut self,
        sink: &mut PacketSink<W>,
        world: &SharedWorld,
        plugins: &dyn PluginEventSink,
    ) -> Result<Vec<FluidSeed>>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        let started_at = Instant::now();
        let cache_epoch = world.cache_epoch();
        let chunks = self.missing_chunks();
        if chunks.is_empty() {
            return Ok(Vec::new());
        }

        log::info!(
            "initial chunk bootstrap started: dimension={}, remaining_chunks={}",
            self.dimension,
            chunks.len()
        );

        let mut fluid_seeds = Vec::new();
        sink.send(ChunkBatchStart {}).await?;
        for (chunk_x, chunk_z) in &chunks {
            self.remove_pending_chunk((*chunk_x, *chunk_z));
            self.loading_chunks.remove(&(*chunk_x, *chunk_z));
            let chunk_payload = ChunkTaskPool::shared()
                .build_payload(
                    world.clone(),
                    self.dimension.clone(),
                    *chunk_x,
                    *chunk_z,
                    cache_epoch,
                    sink.compression_threshold(),
                )
                .await?;
            sink.send_encoded_frame(chunk_payload.frame).await?;
            fluid_seeds.extend(chunk_payload.fluid_seeds);
            for update in world.placed_block_updates(&self.dimension, *chunk_x, *chunk_z) {
                sink.send(
                    qexed_protocol::to_client::play::block_update::BlockUpdate {
                        location: update.location,
                        block_state: qexed_packet::net_types::VarInt(update.block_state),
                    },
                )
                .await?;
            }
            self.visible_chunks.insert((*chunk_x, *chunk_z));
            plugins.emit_chunk_load(&self.dimension, *chunk_x, *chunk_z);
        }
        sink.send(ChunkBatchFinished {
            batch_size: qexed_packet::net_types::VarInt(chunks.len() as i32),
        })
        .await?;
        sink.flush().await?;
        log::info!(
            "initial chunk bootstrap completed: dimension={}, chunks={}, elapsed_ms={}",
            self.dimension,
            chunks.len(),
            started_at.elapsed().as_millis()
        );
        self.log_initial_view_progress();
        Ok(fluid_seeds)
    }

    pub fn missing_chunks(&self) -> Vec<(i32, i32)> {
        let mut chunks = self
            .target_chunks()
            .into_iter()
            .filter(|chunk| {
                !self.visible_chunks.contains(chunk) && !self.loading_chunks.contains(chunk)
            })
            .collect::<Vec<_>>();
        chunks.sort_by_key(|(chunk_x, chunk_z)| {
            let dx = *chunk_x - self.center_x;
            let dz = *chunk_z - self.center_z;
            (
                dx * dx + dz * dz,
                dx.abs().max(dz.abs()),
                *chunk_z,
                *chunk_x,
            )
        });
        chunks
    }

    pub fn target_chunks(&self) -> HashSet<(i32, i32)> {
        visible_chunk_set(self.center_x, self.center_z, self.view_distance)
    }

    pub fn refresh_pending_chunks(&mut self) {
        self.pending_chunks = self.missing_chunks().into();
    }

    pub fn remove_pending_chunk(&mut self, chunk: (i32, i32)) -> bool {
        let Some(index) = self
            .pending_chunks
            .iter()
            .position(|queued| *queued == chunk)
        else {
            return false;
        };
        self.pending_chunks.remove(index);
        true
    }

    pub fn mark_delayed_unloads(
        &mut self,
        target_chunks: HashSet<(i32, i32)>,
        unload_at: Instant,
    ) {
        self.pending_unloads.retain(|chunk, _| {
            self.visible_chunks.contains(chunk) && !target_chunks.contains(chunk)
        });

        for chunk in &self.visible_chunks {
            if target_chunks.contains(chunk) {
                self.pending_unloads.remove(chunk);
            } else {
                self.pending_unloads.entry(*chunk).or_insert(unload_at);
            }
        }
    }

    pub async fn unload_expired_chunks<W>(
        &mut self,
        sink: &mut PacketSink<W>,
        plugins: &dyn PluginEventSink,
        now: Instant,
    ) -> Result<usize>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        if self.pending_unloads.is_empty() {
            return Ok(0);
        }

        let mut expired = self
            .pending_unloads
            .iter()
            .filter_map(|(chunk, unload_at)| {
                (*unload_at <= now
                    && self.visible_chunks.contains(chunk)
                    && !self.chunk_in_current_view(*chunk))
                .then_some(*chunk)
            })
            .collect::<Vec<_>>();

        expired.sort_unstable();
        for (chunk_x, chunk_z) in &expired {
            self.pending_unloads.remove(&(*chunk_x, *chunk_z));
            if self.visible_chunks.remove(&(*chunk_x, *chunk_z)) {
                sink.send(ForgetLevelChunk {
                    pos: qexed_protocol::to_client::play::forget_level_chunk::ChunkPos {
                        x: *chunk_x,
                        z: *chunk_z,
                    },
                })
                .await?;
                plugins.emit_chunk_unload(&self.dimension, *chunk_x, *chunk_z);
            }
        }

        if !expired.is_empty() {
            sink.flush().await?;
        }
        Ok(expired.len())
    }

    pub fn start_next_chunk_load(
        &mut self,
        world: &SharedWorld,
        sender: Option<&tokio::sync::mpsc::UnboundedSender<ChunkLoadResult>>,
        compression_threshold: Option<i32>,
    ) {
        let Some(sender) = sender else {
            return;
        };

        while self.loading_chunks.len() < self.load_parallelism {
            let Some((chunk_x, chunk_z)) = self.pending_chunks.pop_front() else {
                break;
            };
            let chunk = (chunk_x, chunk_z);
            if self.visible_chunks.contains(&chunk) || self.loading_chunks.contains(&chunk) {
                continue;
            }

            let cache_epoch = world.cache_epoch();
            ChunkTaskPool::shared().spawn_payload(
                world.clone(),
                self.dimension.clone(),
                chunk_x,
                chunk_z,
                cache_epoch,
                compression_threshold,
                sender.clone(),
            );
            self.loading_chunks.insert(chunk);
        }
    }

    pub fn queue_loaded_chunk(&mut self, loaded: ChunkLoadResult) {
        let chunk = (loaded.chunk_x, loaded.chunk_z);
        self.loading_chunks.remove(&chunk);
        if self.visible_chunks.contains(&chunk) || !self.chunk_in_current_view(chunk) {
            return;
        }
        self.ready_chunks.push_back(loaded);
    }

    pub async fn send_ready_chunks<W>(
        &mut self,
        sink: &mut PacketSink<W>,
        world: &SharedWorld,
        plugins: &dyn PluginEventSink,
        sender: &tokio::sync::mpsc::UnboundedSender<ChunkLoadResult>,
        replenish_quota: bool,
    ) -> Result<Vec<FluidSeed>>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        if self.ready_chunks.is_empty() {
            return Ok(Vec::new());
        }

        let initial_view_bootstrap = !self.initial_view_logged;
        if !initial_view_bootstrap && self.unacknowledged_batches >= self.max_unacknowledged_batches
        {
            return Ok(Vec::new());
        }

        if replenish_quota || initial_view_bootstrap {
            let max_batch_size = self.desired_chunks_per_tick.max(1.0);
            self.batch_quota =
                (self.batch_quota + self.desired_chunks_per_tick).min(max_batch_size);
        }
        if self.batch_quota < 1.0 {
            return Ok(Vec::new());
        }

        let selected = self
            .take_ready_chunks((self.batch_quota.floor() as usize).min(MAX_CHUNKS_PER_SEND_BATCH));
        if selected.is_empty() {
            self.start_next_chunk_load(world, Some(sender), sink.compression_threshold());
            return Ok(Vec::new());
        }

        let mut chunks = Vec::with_capacity(selected.len());
        for loaded in selected {
            match loaded.frame {
                Ok(frame) => {
                    chunks.push((loaded.chunk_x, loaded.chunk_z, frame, loaded.fluid_seeds))
                }
                Err(err) if is_expired_world_session_error(&err) => {
                    log::debug!(
                        "requeue expired chunk load result: dimension={}, chunk=({}, {})",
                        self.dimension,
                        loaded.chunk_x,
                        loaded.chunk_z
                    );
                    self.requeue_chunk_if_needed((loaded.chunk_x, loaded.chunk_z));
                }
                Err(err) => return Err(PlayError::msg(err)),
            }
        }
        if chunks.is_empty() {
            self.start_next_chunk_load(world, Some(sender), sink.compression_threshold());
            return Ok(Vec::new());
        }

        let mut fluid_seeds = Vec::new();
        sink.send(ChunkBatchStart {}).await?;
        for (chunk_x, chunk_z, chunk_frame, chunk_fluid_seeds) in &chunks {
            sink.send_encoded_frame(chunk_frame.clone()).await?;
            fluid_seeds.extend(chunk_fluid_seeds.iter().cloned());
            for update in world.placed_block_updates(&self.dimension, *chunk_x, *chunk_z) {
                sink.send(
                    qexed_protocol::to_client::play::block_update::BlockUpdate {
                        location: update.location,
                        block_state: qexed_packet::net_types::VarInt(update.block_state),
                    },
                )
                .await?;
            }
            self.visible_chunks.insert((*chunk_x, *chunk_z));
            plugins.emit_chunk_load(&self.dimension, *chunk_x, *chunk_z);
        }
        if !initial_view_bootstrap {
            self.unacknowledged_batches = self.unacknowledged_batches.saturating_add(1);
        }
        self.batch_quota = (self.batch_quota - chunks.len() as f32).max(0.0);
        sink.send(ChunkBatchFinished {
            batch_size: qexed_packet::net_types::VarInt(chunks.len() as i32),
        })
        .await?;
        sink.flush().await?;
        self.log_initial_view_progress();

        self.start_next_chunk_load(world, Some(sender), sink.compression_threshold());
        Ok(fluid_seeds)
    }

    fn take_ready_chunks(&mut self, max_count: usize) -> Vec<ChunkLoadResult> {
        if max_count == 0 {
            return Vec::new();
        }

        let target = self.target_chunks();
        let visible = self.visible_chunks.clone();
        let mut ready = self
            .ready_chunks
            .drain(..)
            .filter(|loaded| {
                let chunk = (loaded.chunk_x, loaded.chunk_z);
                target.contains(&chunk) && !visible.contains(&chunk)
            })
            .collect::<Vec<_>>();
        let center_x = self.center_x;
        let center_z = self.center_z;
        ready.sort_by_key(|loaded| {
            chunk_send_priority(center_x, center_z, loaded.chunk_x, loaded.chunk_z)
        });

        let split_at = ready.len().min(max_count);
        let remaining = ready.split_off(split_at);
        self.ready_chunks = remaining.into();
        ready
    }

    fn requeue_chunk_if_needed(&mut self, chunk: (i32, i32)) {
        if !self.chunk_in_current_view(chunk)
            || self.visible_chunks.contains(&chunk)
            || self.loading_chunks.contains(&chunk)
            || self.pending_chunks.contains(&chunk)
        {
            return;
        }
        self.pending_chunks.push_front(chunk);
    }

    pub fn on_chunk_batch_received(&mut self, desired_chunks_per_tick: f32) {
        self.unacknowledged_batches = self.unacknowledged_batches.saturating_sub(1);
        self.desired_chunks_per_tick = if desired_chunks_per_tick.is_nan() {
            MIN_CHUNKS_PER_TICK
        } else {
            desired_chunks_per_tick.clamp(MIN_CHUNKS_PER_TICK, MAX_CHUNKS_PER_TICK)
        };
        if self.unacknowledged_batches == 0 {
            self.batch_quota = 1.0;
        }
        self.max_unacknowledged_batches = MAX_UNACKNOWLEDGED_BATCHES;
    }

    fn reset_batch_flow_control(&mut self) {
        self.desired_chunks_per_tick = START_CHUNKS_PER_TICK;
        self.batch_quota = 0.0;
        self.unacknowledged_batches = 0;
        self.max_unacknowledged_batches = INITIAL_MAX_UNACKNOWLEDGED_BATCHES;
    }

    pub fn has_loading_chunks(&self) -> bool {
        !self.loading_chunks.is_empty()
    }

    pub fn has_ready_chunks(&self) -> bool {
        !self.ready_chunks.is_empty()
    }

    pub fn has_pending_unloads(&self) -> bool {
        !self.pending_unloads.is_empty()
    }

    pub fn initial_view_complete(&self) -> bool {
        self.pending_chunks.is_empty()
            && self.ready_chunks.is_empty()
            && self.loading_chunks.is_empty()
            && self.target_chunks().is_subset(&self.visible_chunks)
    }

    pub fn log_initial_view_progress(&mut self) {
        if self.initial_view_logged {
            return;
        }

        let target = self.target_chunks();
        let missing = target.difference(&self.visible_chunks).count();
        if missing == 0
            && self.pending_chunks.is_empty()
            && self.ready_chunks.is_empty()
            && self.loading_chunks.is_empty()
        {
            self.initial_view_logged = true;
            log::info!(
                "initial chunk view sent: dimension={}, center=({}, {}), view_distance={}, visible_chunks={}, target_chunks={}",
                self.dimension,
                self.center_x,
                self.center_z,
                self.view_distance,
                self.visible_chunks.len(),
                target.len()
            );
            return;
        }

        log::debug!(
            "initial chunk view pending: dimension={}, center=({}, {}), view_distance={}, visible={}, target={}, missing={}, pending={}, ready={}, loading={}, unacked={}",
            self.dimension,
            self.center_x,
            self.center_z,
            self.view_distance,
            self.visible_chunks.len(),
            target.len(),
            missing,
            self.pending_chunks.len(),
            self.ready_chunks.len(),
            self.loading_chunks.len(),
            self.unacknowledged_batches
        );
    }

    fn chunk_in_current_view(&self, chunk: (i32, i32)) -> bool {
        chunk_in_view(self.center_x, self.center_z, self.view_distance, chunk)
    }

    fn retain_work_for_current_view(&mut self) {
        let center_x = self.center_x;
        let center_z = self.center_z;
        let view_distance = self.view_distance;
        self.pending_chunks
            .retain(|chunk| chunk_in_view(center_x, center_z, view_distance, *chunk));
        self.ready_chunks.retain(|chunk| {
            chunk_in_view(
                center_x,
                center_z,
                view_distance,
                (chunk.chunk_x, chunk.chunk_z),
            )
        });
        self.loading_chunks
            .retain(|chunk| chunk_in_view(center_x, center_z, view_distance, *chunk));
    }
}

fn first_ring_neighbor(origin_x: i32, origin_z: i32, chunk_x: i32, chunk_z: i32) -> bool {
    let dx = chunk_x - origin_x;
    let dz = chunk_z - origin_z;
    (-1..=1).contains(&dx) && (-1..=1).contains(&dz) && (dx != 0 || dz != 0)
}

fn incremental_chunk_delta(
    old_x: i32,
    old_z: i32,
    new_x: i32,
    new_z: i32,
    view_distance: i32,
) -> Option<(Vec<(i32, i32)>, Vec<(i32, i32)>)> {
    if !first_ring_neighbor(old_x, old_z, new_x, new_z) {
        return None;
    }

    let view_distance = view_distance.max(1);
    let dx = new_x - old_x;
    let dz = new_z - old_z;
    let mut entering = HashSet::new();
    let mut leaving = HashSet::new();

    if dx > 0 {
        let enter_x = new_x + view_distance;
        let leave_x = old_x - view_distance;
        for chunk_z in new_z - view_distance..=new_z + view_distance {
            entering.insert((enter_x, chunk_z));
        }
        for chunk_z in old_z - view_distance..=old_z + view_distance {
            leaving.insert((leave_x, chunk_z));
        }
    } else if dx < 0 {
        let enter_x = new_x - view_distance;
        let leave_x = old_x + view_distance;
        for chunk_z in new_z - view_distance..=new_z + view_distance {
            entering.insert((enter_x, chunk_z));
        }
        for chunk_z in old_z - view_distance..=old_z + view_distance {
            leaving.insert((leave_x, chunk_z));
        }
    }

    if dz > 0 {
        let enter_z = new_z + view_distance;
        let leave_z = old_z - view_distance;
        for chunk_x in new_x - view_distance..=new_x + view_distance {
            entering.insert((chunk_x, enter_z));
        }
        for chunk_x in old_x - view_distance..=old_x + view_distance {
            leaving.insert((chunk_x, leave_z));
        }
    } else if dz < 0 {
        let enter_z = new_z - view_distance;
        let leave_z = old_z + view_distance;
        for chunk_x in new_x - view_distance..=new_x + view_distance {
            entering.insert((chunk_x, enter_z));
        }
        for chunk_x in old_x - view_distance..=old_x + view_distance {
            leaving.insert((chunk_x, leave_z));
        }
    }

    let mut entering = entering.into_iter().collect::<Vec<_>>();
    entering.sort_unstable();
    let mut leaving = leaving.into_iter().collect::<Vec<_>>();
    leaving.sort_unstable();
    Some((entering, leaving))
}

fn chunk_in_view(center_x: i32, center_z: i32, view_distance: i32, chunk: (i32, i32)) -> bool {
    let view_distance = view_distance.max(1);
    (chunk.0 - center_x).abs() <= view_distance && (chunk.1 - center_z).abs() <= view_distance
}

fn chunk_send_priority(
    center_x: i32,
    center_z: i32,
    chunk_x: i32,
    chunk_z: i32,
) -> (i32, i32, i32, i32) {
    let dx = chunk_x - center_x;
    let dz = chunk_z - center_z;
    (dx * dx + dz * dz, dx.abs().max(dz.abs()), chunk_z, chunk_x)
}

/// 过期会话判定（v4 检查 anyhow 错误链；v6 检查 PlayError 消息文本）。
fn is_expired_world_session_error(err: &str) -> bool {
    err.contains("world session expired")
}

fn build_saved_chunk_payload_sync(
    world: &dyn WorldChunkSource,
    dimension: &str,
    chunk_x: i32,
    chunk_z: i32,
    cache_epoch: u64,
    compression_threshold: Option<i32>,
) -> Result<Option<ChunkPayloadResult>> {
    let total_start = Instant::now();
    if let Some(payload) = world.precompiled_chunk_packet(
        dimension,
        chunk_x,
        chunk_z,
        cache_epoch,
        compression_threshold,
    )? {
        log::trace!(
            "chunk frame cache hit: phase=load, dimension={dimension}, chunk=({chunk_x}, {chunk_z}), bytes={}",
            payload.len()
        );
        let fluid_seeds = world.fluid_positions_in_chunk(dimension, chunk_x, chunk_z)?;
        return Ok(Some(ChunkPayloadResult {
            frame: payload,
            fluid_seeds,
        }));
    }

    let chunk_start = Instant::now();
    let Some(chunk) =
        world.saved_network_chunk_for_session(dimension, chunk_x, chunk_z, cache_epoch)?
    else {
        return Ok(None);
    };
    let chunk_elapsed = chunk_start.elapsed();

    let encode_start = Instant::now();
    let payload = encode_chunk_payload(
        world,
        dimension,
        chunk_x,
        chunk_z,
        chunk,
        compression_threshold,
    )?;
    let encode_elapsed = encode_start.elapsed();
    let total_elapsed = total_start.elapsed();

    if total_elapsed >= SLOW_CHUNK_PAYLOAD_LOG_THRESHOLD && log::log_enabled!(log::Level::Debug) {
        log::debug!(
            "chunk payload built: phase=load, dimension={dimension}, chunk=({chunk_x}, {chunk_z}), total_ms={:.2}, world_ms={:.2}, encode_ms={:.2}, bytes={}",
            duration_ms(total_elapsed),
            duration_ms(chunk_elapsed),
            duration_ms(encode_elapsed),
            payload.len()
        );
    }

    let fluid_seeds = world.fluid_positions_in_chunk(dimension, chunk_x, chunk_z)?;
    Ok(Some(ChunkPayloadResult {
        frame: payload,
        fluid_seeds,
    }))
}

fn build_generated_chunk_payload_sync(
    world: SharedWorld,
    dimension: String,
    chunk_x: i32,
    chunk_z: i32,
    cache_epoch: u64,
    compression_threshold: Option<i32>,
) -> Result<ChunkPayloadResult> {
    let total_start = Instant::now();
    if let Some(payload) = world.precompiled_chunk_packet(
        &dimension,
        chunk_x,
        chunk_z,
        cache_epoch,
        compression_threshold,
    )? {
        log::trace!(
            "chunk frame cache hit: phase=generate, dimension={dimension}, chunk=({chunk_x}, {chunk_z}), bytes={}",
            payload.len()
        );
        let fluid_seeds = world.fluid_positions_in_chunk(&dimension, chunk_x, chunk_z)?;
        return Ok(ChunkPayloadResult {
            frame: payload,
            fluid_seeds,
        });
    }

    let chunk_start = Instant::now();
    let chunk =
        world.generated_network_chunk_for_session(&dimension, chunk_x, chunk_z, cache_epoch)?;
    let chunk_elapsed = chunk_start.elapsed();

    let encode_start = Instant::now();
    let payload = encode_chunk_payload(
        world.as_ref(),
        &dimension,
        chunk_x,
        chunk_z,
        chunk,
        compression_threshold,
    )?;
    let encode_elapsed = encode_start.elapsed();
    let total_elapsed = total_start.elapsed();

    if total_elapsed >= SLOW_CHUNK_PAYLOAD_LOG_THRESHOLD && log::log_enabled!(log::Level::Debug) {
        log::debug!(
            "chunk payload built: phase=generate, dimension={dimension}, chunk=({chunk_x}, {chunk_z}), total_ms={:.2}, world_ms={:.2}, encode_ms={:.2}, bytes={}",
            duration_ms(total_elapsed),
            duration_ms(chunk_elapsed),
            duration_ms(encode_elapsed),
            payload.len()
        );
    }

    let fluid_seeds = world.fluid_positions_in_chunk(&dimension, chunk_x, chunk_z)?;
    Ok(ChunkPayloadResult {
        frame: payload,
        fluid_seeds,
    })
}

/// 编码区块帧；含预编译缓存拆分（前缀不含光照 + 光照现编，v4 语义）。
fn encode_chunk_payload(
    world: &dyn WorldChunkSource,
    dimension: &str,
    chunk_x: i32,
    chunk_z: i32,
    chunk: LevelChunkWithLight,
    compression_threshold: Option<i32>,
) -> Result<Bytes> {
    if !world.precompiled_chunk_packets_enabled() {
        let payload =
            PacketSink::<tokio::io::Sink>::build_send_packet(chunk)?;
        return PacketSink::<tokio::io::Sink>::encode_payload_frame_with_threshold(
            payload,
            compression_threshold,
        )
        .map_err(PlayError::from);
    }

    if world.precompiled_chunk_payload_includes_light() {
        let payload =
            PacketSink::<tokio::io::Sink>::build_send_packet(chunk)?;
        let frame =
            PacketSink::<tokio::io::Sink>::encode_payload_frame_with_threshold(
                payload,
                compression_threshold,
            )
            .map_err(PlayError::from)?;
        world.remember_precompiled_chunk_frame(
            dimension,
            chunk_x,
            chunk_z,
            frame.clone(),
            compression_threshold,
        );
        return Ok(frame);
    }

    // v4：前缀 = 包 id + chunk_x + chunk_z + data；v6 字段为 x/z（普通 i32）+ chunk_data + light_data。
    let mut buf = BytesMut::new();
    let mut writer = qexed_packet::PacketWriter::new(&mut buf);
    qexed_packet::net_types::VarInt(LevelChunkWithLight::ID).serialize(&mut writer)?;
    chunk.x.serialize(&mut writer)?;
    chunk.z.serialize(&mut writer)?;
    chunk.chunk_data.serialize(&mut writer)?;
    let prefix = buf.freeze();

    let mut payload = BytesMut::from(prefix.as_ref());
    let mut writer = qexed_packet::PacketWriter::new(&mut payload);
    chunk.light_data.serialize(&mut writer)?;
    world.remember_precompiled_chunk_packet_without_light(
        dimension,
        chunk_x,
        chunk_z,
        prefix,
        compression_threshold,
    );
    PacketSink::<tokio::io::Sink>::encode_payload_frame_with_threshold(
        payload.freeze(),
        compression_threshold,
    )
    .map_err(PlayError::from)
}

fn duration_ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

/// 区块加载并行度限制（v4 chunk_load_parallelism_limit）。
pub fn chunk_load_parallelism_limit(value: usize) -> usize {
    let value = if value == 0 {
        DEFAULT_CHUNK_LOAD_PARALLELISM
    } else {
        value
    };
    value.clamp(1, MAX_CHUNK_LOAD_PARALLELISM)
}

fn default_chunk_load_threads() -> usize {
    let available = std::thread::available_parallelism()
        .map(std::num::NonZeroUsize::get)
        .unwrap_or(DEFAULT_CHUNK_LOAD_PARALLELISM);
    (available / 2).clamp(1, MAX_CHUNK_LOAD_THREADS)
}

fn default_chunk_generate_threads() -> usize {
    let available = std::thread::available_parallelism()
        .map(std::num::NonZeroUsize::get)
        .unwrap_or(DEFAULT_CHUNK_LOAD_PARALLELISM);
    available
        .saturating_sub(1)
        .clamp(1, MAX_CHUNK_GENERATE_THREADS)
}

fn visible_chunk_set(center_x: i32, center_z: i32, view_distance: i32) -> HashSet<(i32, i32)> {
    visible_chunks(center_x, center_z, view_distance)
        .into_iter()
        .collect()
}

fn visible_chunks(center_x: i32, center_z: i32, view_distance: i32) -> Vec<(i32, i32)> {
    let view_distance = view_distance.max(1);
    let mut chunks = Vec::new();
    for chunk_z in center_z - view_distance..=center_z + view_distance {
        for chunk_x in center_x - view_distance..=center_x + view_distance {
            chunks.push((chunk_x, chunk_z));
        }
    }
    chunks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parallelism_is_clamped() {
        assert_eq!(chunk_load_parallelism_limit(0), DEFAULT_CHUNK_LOAD_PARALLELISM);
        assert_eq!(chunk_load_parallelism_limit(1), 1);
        assert_eq!(chunk_load_parallelism_limit(9999), MAX_PARALLELISM_FOR_TESTS);
    }

    #[test]
    fn incremental_delta_computes_entering_and_leaving() {
        let (entering, leaving) = incremental_chunk_delta(0, 0, 1, 0, 2).unwrap();
        assert!(entering.contains(&(3, 2)));
        assert!(entering.contains(&(3, -2)));
        assert!(leaving.contains(&(-2, 2)));
        assert!(leaving.contains(&(-2, -2)));

        // 非相邻移动退化为全量
        assert!(incremental_chunk_delta(0, 0, 3, 0, 2).is_none());
    }

    #[test]
    fn expired_session_error_matches_message_text() {
        assert!(is_expired_world_session_error(
            "chunk load cancelled because world session expired"
        ));
        assert!(!is_expired_world_session_error("io error"));
    }

    #[test]
    fn chunk_state_reports_view_completion() {
        let mut state = ChunkSendState::new("minecraft:overworld".to_string(), 0, 0, 1, 2);
        assert!(!state.initial_view_complete());
        state.refresh_pending_chunks();
        assert_eq!(state.pending_chunks.len(), 9);
        // 逐个入列已发送
        while let Some(chunk) = state.pending_chunks.pop_front() {
            state.visible_chunks.insert(chunk);
        }
        assert!(state.initial_view_complete());
    }
}

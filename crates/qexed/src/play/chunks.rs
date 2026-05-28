use anyhow::{Context, Result};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::{OnceLock, mpsc},
    time::{Duration, Instant},
};

use qexed_packet::net_types::VarInt;
use qexed_protocol::to_client::play::{
    chunk_batch_finished::ChunkBatchFinished, chunk_batch_start::ChunkBatchStart,
    forget_level_chunk::ForgetLevelChunk, update_view_position::UpdateViewPosition,
};

use crate::world::WorldManager;

use super::util::chunk_coord;

const CHUNK_UNLOAD_DELAY: Duration = Duration::from_secs(4);
const DEFAULT_CHUNK_LOAD_PARALLELISM: usize = 4;
const MAX_CHUNK_LOAD_PARALLELISM: usize = 64;
const SLOW_CHUNK_PAYLOAD_LOG_THRESHOLD: Duration = Duration::from_millis(250);
const MIN_CHUNKS_PER_TICK: f32 = 0.01;
const MAX_CHUNKS_PER_TICK: f32 = 64.0;
const START_CHUNKS_PER_TICK: f32 = 9.0;
const INITIAL_MAX_UNACKNOWLEDGED_BATCHES: usize = 1;
const MAX_UNACKNOWLEDGED_BATCHES: usize = 10;
const MAX_CHUNK_LOAD_THREADS: usize = 4;
const MAX_CHUNK_GENERATE_THREADS: usize = 8;

pub(super) struct ChunkSendState {
    pub(super) dimension: String,
    pub(super) center_x: i32,
    pub(super) center_z: i32,
    pub(super) view_distance: i32,
    pub(super) load_parallelism: usize,
    pub(super) visible_chunks: HashSet<(i32, i32)>,
    pub(super) pending_chunks: VecDeque<(i32, i32)>,
    pub(super) ready_chunks: VecDeque<ChunkLoadResult>,
    pub(super) pending_unloads: HashMap<(i32, i32), Instant>,
    pub(super) loading_chunks: HashSet<(i32, i32)>,
    pub(super) desired_chunks_per_tick: f32,
    pub(super) batch_quota: f32,
    pub(super) unacknowledged_batches: usize,
    pub(super) max_unacknowledged_batches: usize,
}

pub(super) struct ChunkLoadResult {
    pub(super) chunk_x: i32,
    pub(super) chunk_z: i32,
    payload: Result<bytes::Bytes>,
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
            let receiver = std::sync::Arc::new(std::sync::Mutex::new(receiver));
            let generate_receiver = std::sync::Arc::new(std::sync::Mutex::new(generate_receiver));
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
                "initialized chunk task pools: load_workers={load_worker_count}, generate_workers={generate_worker_count}"
            );
            Self { sender }
        })
    }

    async fn build_payload(
        &self,
        world: WorldManager,
        dimension: String,
        chunk_x: i32,
        chunk_z: i32,
        cache_epoch: u64,
    ) -> Result<bytes::Bytes> {
        let (sender, receiver) = tokio::sync::oneshot::channel();
        self.sender
            .send(ChunkTask::BuildPayload {
                world,
                dimension,
                chunk_x,
                chunk_z,
                cache_epoch,
                reply: ChunkTaskReply::OneShot(sender),
            })
            .context("queue chunk packet build task")?;
        receiver.await.context("join chunk packet build task")?
    }

    fn spawn_payload(
        &self,
        world: WorldManager,
        dimension: String,
        chunk_x: i32,
        chunk_z: i32,
        cache_epoch: u64,
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
                reply: ChunkTaskReply::Channel(sender),
            })
            .is_err()
        {
            log::warn!("failed to queue chunk payload task because chunk task pool stopped");
        }
    }
}

enum ChunkTask {
    BuildPayload {
        world: WorldManager,
        dimension: String,
        chunk_x: i32,
        chunk_z: i32,
        cache_epoch: u64,
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
                reply,
            } => {
                match build_saved_chunk_payload_sync(
                    &world,
                    &dimension,
                    chunk_x,
                    chunk_z,
                    cache_epoch,
                ) {
                    Ok(Some(payload)) => reply.send(chunk_x, chunk_z, Ok(payload)),
                    Ok(None) => {
                        if generate_sender
                            .send(ChunkGenerateTask::BuildPayload {
                                world,
                                dimension,
                                chunk_x,
                                chunk_z,
                                cache_epoch,
                                reply,
                            })
                            .is_err()
                        {
                            log::warn!(
                                "failed to queue chunk generation task because generation pool stopped"
                            );
                        }
                    }
                    Err(err) => reply.send(chunk_x, chunk_z, Err(err)),
                }
            }
        }
    }
}

enum ChunkGenerateTask {
    BuildPayload {
        world: WorldManager,
        dimension: String,
        chunk_x: i32,
        chunk_z: i32,
        cache_epoch: u64,
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
                reply,
            } => {
                let payload = build_generated_chunk_payload_sync(
                    world,
                    dimension,
                    chunk_x,
                    chunk_z,
                    cache_epoch,
                );
                reply.send(chunk_x, chunk_z, payload);
            }
        }
    }
}

enum ChunkTaskReply {
    OneShot(tokio::sync::oneshot::Sender<Result<bytes::Bytes>>),
    Channel(tokio::sync::mpsc::UnboundedSender<ChunkLoadResult>),
}

impl ChunkTaskReply {
    fn send(self, chunk_x: i32, chunk_z: i32, payload: Result<bytes::Bytes>) {
        match self {
            ChunkTaskReply::OneShot(sender) => {
                let _ = sender.send(payload);
            }
            ChunkTaskReply::Channel(sender) => {
                let _ = sender.send(ChunkLoadResult {
                    chunk_x,
                    chunk_z,
                    payload,
                });
            }
        }
    }
}

impl ChunkSendState {
    pub(super) fn new(
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
            desired_chunks_per_tick: START_CHUNKS_PER_TICK,
            batch_quota: 0.0,
            unacknowledged_batches: 0,
            max_unacknowledged_batches: INITIAL_MAX_UNACKNOWLEDGED_BATCHES,
        }
    }

    pub(super) async fn update_center<W>(
        &mut self,
        sink: &mut qexed_tcp_connect::PacketSink<W>,
        chunk_sender: &tokio::sync::mpsc::UnboundedSender<ChunkLoadResult>,
        world: &WorldManager,
        x: f64,
        z: f64,
    ) -> Result<()>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        let chunk_x = chunk_coord(x);
        let chunk_z = chunk_coord(z);
        if chunk_x == self.center_x && chunk_z == self.center_z {
            return Ok(());
        }

        let next = visible_chunk_set(chunk_x, chunk_z, self.view_distance);
        sink.send(UpdateViewPosition {
            chunk_x: VarInt(chunk_x),
            chunk_z: VarInt(chunk_z),
        })
        .await?;

        self.center_x = chunk_x;
        self.center_z = chunk_z;
        self.mark_delayed_unloads(next, Instant::now() + CHUNK_UNLOAD_DELAY);
        self.refresh_pending_chunks();
        self.start_next_chunk_load(world, Some(chunk_sender));
        sink.flush().await?;
        log::debug!(
            "鐜╁绉诲姩鍒版柊鍖哄潡锛屽凡琛ュ彂瑙嗚窛鍖哄潡: center=({chunk_x}, {chunk_z})"
        );
        Ok(())
    }

    pub(super) async fn reset_after_respawn<W>(
        &mut self,
        sink: &mut qexed_tcp_connect::PacketSink<W>,
        chunk_sender: &tokio::sync::mpsc::UnboundedSender<ChunkLoadResult>,
        world: &WorldManager,
        plugins: &crate::plugins::PluginManager,
        x: f64,
        z: f64,
    ) -> Result<()>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        let chunk_x = chunk_coord(x);
        let chunk_z = chunk_coord(z);
        sink.send(UpdateViewPosition {
            chunk_x: VarInt(chunk_x),
            chunk_z: VarInt(chunk_z),
        })
        .await?;

        self.reset_view(chunk_x, chunk_z);
        self.send_center_chunk_first(sink, world, plugins).await?;
        self.start_next_chunk_load(world, Some(chunk_sender));
        sink.flush().await?;
        log::debug!("respawn chunk view reset: center=({chunk_x}, {chunk_z})");
        Ok(())
    }

    pub(super) async fn reset_dimension_after_respawn<W>(
        &mut self,
        sink: &mut qexed_tcp_connect::PacketSink<W>,
        chunk_sender: &tokio::sync::mpsc::UnboundedSender<ChunkLoadResult>,
        world: &WorldManager,
        plugins: &crate::plugins::PluginManager,
        dimension: impl Into<String>,
        x: f64,
        z: f64,
    ) -> Result<()>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        self.dimension = dimension.into();
        self.reset_after_respawn(sink, chunk_sender, world, plugins, x, z)
            .await
    }

    pub(super) fn reset_view(&mut self, center_x: i32, center_z: i32) {
        self.center_x = center_x;
        self.center_z = center_z;
        self.visible_chunks.clear();
        self.pending_chunks.clear();
        self.ready_chunks.clear();
        self.pending_unloads.clear();
        self.loading_chunks.clear();
        self.reset_batch_flow_control();
        self.refresh_pending_chunks();
    }

    pub(super) async fn send_center_chunk_first<W>(
        &mut self,
        sink: &mut qexed_tcp_connect::PacketSink<W>,
        world: &WorldManager,
        plugins: &crate::plugins::PluginManager,
    ) -> Result<bool>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        let chunk = (self.center_x, self.center_z);
        if self.visible_chunks.contains(&chunk) {
            return Ok(false);
        }

        self.remove_pending_chunk(chunk);
        self.loading_chunks.remove(&chunk);

        let cache_epoch = world.cache_epoch();
        let chunk_payload = ChunkTaskPool::shared()
            .build_payload(
                world.clone(),
                self.dimension.clone(),
                chunk.0,
                chunk.1,
                cache_epoch,
            )
            .await?;

        sink.send(ChunkBatchStart {}).await?;
        sink.send_raw(chunk_payload).await?;
        for update in world.placed_block_updates(&self.dimension, chunk.0, chunk.1) {
            sink.send(update).await?;
        }
        self.visible_chunks.insert(chunk);
        plugins.emit_chunk_load(&self.dimension, chunk.0, chunk.1);
        self.unacknowledged_batches = self.unacknowledged_batches.saturating_add(1);
        sink.send(ChunkBatchFinished {
            batch_size: VarInt(1),
        })
        .await?;
        sink.flush().await?;
        Ok(true)
    }

    pub(super) async fn send_missing_chunks<W>(
        &mut self,
        sink: &mut qexed_tcp_connect::PacketSink<W>,
        world: &WorldManager,
        cache_epoch: u64,
    ) -> Result<usize>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        let chunks = self.missing_chunks();
        if chunks.is_empty() {
            return Ok(0);
        }

        sink.send(ChunkBatchStart {}).await?;
        for (chunk_x, chunk_z) in &chunks {
            let chunk_x = *chunk_x;
            let chunk_z = *chunk_z;
            let chunk_payload = ChunkTaskPool::shared()
                .build_payload(
                    world.clone(),
                    self.dimension.clone(),
                    chunk_x,
                    chunk_z,
                    cache_epoch,
                )
                .await?;
            sink.send_raw(chunk_payload).await?;
            for update in world.placed_block_updates(&self.dimension, chunk_x, chunk_z) {
                sink.send(update).await?;
            }
            self.visible_chunks.insert((chunk_x, chunk_z));
        }
        sink.send(ChunkBatchFinished {
            batch_size: VarInt(chunks.len() as i32),
        })
        .await?;
        Ok(chunks.len())
    }

    pub(super) fn missing_chunks(&self) -> Vec<(i32, i32)> {
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

    pub(super) fn target_chunks(&self) -> HashSet<(i32, i32)> {
        visible_chunk_set(self.center_x, self.center_z, self.view_distance)
    }

    pub(super) fn refresh_pending_chunks(&mut self) {
        self.pending_chunks = self.missing_chunks().into();
    }

    pub(super) fn remove_pending_chunk(&mut self, chunk: (i32, i32)) -> bool {
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

    pub(super) fn mark_delayed_unloads(
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

    pub(super) async fn unload_expired_chunks<W>(
        &mut self,
        sink: &mut qexed_tcp_connect::PacketSink<W>,
        plugins: &crate::plugins::PluginManager,
        now: Instant,
    ) -> Result<usize>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        if self.pending_unloads.is_empty() {
            return Ok(0);
        }

        let target_chunks = self.target_chunks();
        let mut expired = self
            .pending_unloads
            .iter()
            .filter_map(|(chunk, unload_at)| {
                (*unload_at <= now
                    && self.visible_chunks.contains(chunk)
                    && !target_chunks.contains(chunk))
                .then_some(*chunk)
            })
            .collect::<Vec<_>>();

        expired.sort_unstable();
        for (chunk_x, chunk_z) in &expired {
            self.pending_unloads.remove(&(*chunk_x, *chunk_z));
            if self.visible_chunks.remove(&(*chunk_x, *chunk_z)) {
                sink.send(ForgetLevelChunk {
                    chunk_x: *chunk_x,
                    chunk_z: *chunk_z,
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

    pub(super) fn start_next_chunk_load(
        &mut self,
        world: &WorldManager,
        sender: Option<&tokio::sync::mpsc::UnboundedSender<ChunkLoadResult>>,
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
                sender.clone(),
            );
            self.loading_chunks.insert(chunk);
        }
    }

    pub(super) fn queue_loaded_chunk(&mut self, loaded: ChunkLoadResult) {
        let chunk = (loaded.chunk_x, loaded.chunk_z);
        self.loading_chunks.remove(&chunk);
        if self.visible_chunks.contains(&chunk) || !self.target_chunks().contains(&chunk) {
            return;
        }
        self.ready_chunks.push_back(loaded);
    }

    pub(super) async fn send_ready_chunks<W>(
        &mut self,
        sink: &mut qexed_tcp_connect::PacketSink<W>,
        world: &WorldManager,
        plugins: &crate::plugins::PluginManager,
        sender: &tokio::sync::mpsc::UnboundedSender<ChunkLoadResult>,
        replenish_quota: bool,
    ) -> Result<usize>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        if self.ready_chunks.is_empty()
            || self.unacknowledged_batches >= self.max_unacknowledged_batches
        {
            return Ok(0);
        }

        if replenish_quota {
            let max_batch_size = self.desired_chunks_per_tick.max(1.0);
            self.batch_quota =
                (self.batch_quota + self.desired_chunks_per_tick).min(max_batch_size);
        }
        if self.batch_quota < 1.0 {
            return Ok(0);
        }

        let selected = self.take_ready_chunks(self.batch_quota.floor() as usize);
        if selected.is_empty() {
            self.start_next_chunk_load(world, Some(sender));
            return Ok(0);
        }

        let mut chunks = Vec::with_capacity(selected.len());
        for loaded in selected {
            chunks.push((loaded.chunk_x, loaded.chunk_z, loaded.payload?));
        }

        sink.send(ChunkBatchStart {}).await?;
        for (chunk_x, chunk_z, chunk_payload) in &chunks {
            sink.send_raw(chunk_payload.clone()).await?;
            for update in world.placed_block_updates(&self.dimension, *chunk_x, *chunk_z) {
                sink.send(update).await?;
            }
            self.visible_chunks.insert((*chunk_x, *chunk_z));
            plugins.emit_chunk_load(&self.dimension, *chunk_x, *chunk_z);
        }
        self.unacknowledged_batches = self.unacknowledged_batches.saturating_add(1);
        self.batch_quota = (self.batch_quota - chunks.len() as f32).max(0.0);
        sink.send(ChunkBatchFinished {
            batch_size: VarInt(chunks.len() as i32),
        })
        .await?;
        sink.flush().await?;

        self.start_next_chunk_load(world, Some(sender));
        Ok(chunks.len())
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

    pub(super) fn on_chunk_batch_received(&mut self, desired_chunks_per_tick: f32) {
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

    pub(super) fn has_loading_chunks(&self) -> bool {
        !self.loading_chunks.is_empty()
    }

    pub(super) fn has_ready_chunks(&self) -> bool {
        !self.ready_chunks.is_empty()
    }

    pub(super) fn has_pending_unloads(&self) -> bool {
        !self.pending_unloads.is_empty()
    }
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

fn build_saved_chunk_payload_sync(
    world: &WorldManager,
    dimension: &str,
    chunk_x: i32,
    chunk_z: i32,
    cache_epoch: u64,
) -> Result<Option<bytes::Bytes>> {
    let total_start = Instant::now();
    let chunk_start = Instant::now();
    let Some(chunk) =
        world.saved_network_chunk_for_session(dimension, chunk_x, chunk_z, cache_epoch)?
    else {
        return Ok(None);
    };
    let chunk_elapsed = chunk_start.elapsed();

    let encode_start = Instant::now();
    let payload = qexed_tcp_connect::PacketSink::<tokio::io::Sink>::build_send_packet(chunk)
        .context("encode saved chunk packet")?;
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

    Ok(Some(payload))
}

fn build_generated_chunk_payload_sync(
    world: WorldManager,
    dimension: String,
    chunk_x: i32,
    chunk_z: i32,
    cache_epoch: u64,
) -> Result<bytes::Bytes> {
    let total_start = Instant::now();
    let chunk_start = Instant::now();
    let chunk =
        world.generated_network_chunk_for_session(&dimension, chunk_x, chunk_z, cache_epoch)?;
    let chunk_elapsed = chunk_start.elapsed();

    let encode_start = Instant::now();
    let payload = qexed_tcp_connect::PacketSink::<tokio::io::Sink>::build_send_packet(chunk)
        .context("encode chunk packet")?;
    let encode_elapsed = encode_start.elapsed();
    let total_elapsed = total_start.elapsed();

    if total_elapsed >= SLOW_CHUNK_PAYLOAD_LOG_THRESHOLD && log::log_enabled!(log::Level::Debug) {
        log::debug!(
            "鍖哄潡 payload 鏋勫缓鑰楁椂: dimension={dimension}, chunk=({chunk_x}, {chunk_z}), total_ms={:.2}, world_ms={:.2}, encode_ms={:.2}, bytes={}",
            duration_ms(total_elapsed),
            duration_ms(chunk_elapsed),
            duration_ms(encode_elapsed),
            payload.len()
        );
    }

    Ok(payload)
}

fn duration_ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

pub(super) fn chunk_load_parallelism_limit(value: usize) -> usize {
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

#[allow(dead_code)]
pub(super) async fn send_spawn_chunks<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    world: &WorldManager,
    dimension: &str,
    center_x: i32,
    center_z: i32,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let mut chunk_state = ChunkSendState::new(dimension.to_string(), center_x, center_z, 1, 1);
    chunk_state
        .send_missing_chunks(sink, world, world.cache_epoch())
        .await?;
    Ok(())
}

#[cfg(test)]
pub(super) const DEFAULT_PARALLELISM_FOR_TESTS: usize = DEFAULT_CHUNK_LOAD_PARALLELISM;

#[cfg(test)]
pub(super) const MAX_PARALLELISM_FOR_TESTS: usize = MAX_CHUNK_LOAD_PARALLELISM;

#[cfg(test)]
mod tests {
    use super::*;

    fn loaded_chunk(chunk_x: i32, chunk_z: i32) -> ChunkLoadResult {
        ChunkLoadResult {
            chunk_x,
            chunk_z,
            payload: Ok(bytes::Bytes::new()),
        }
    }

    #[test]
    fn chunk_batch_received_clamps_rate_and_reopens_send_window() {
        let mut state = ChunkSendState::new("minecraft:overworld".to_string(), 0, 0, 1, 4);
        state.unacknowledged_batches = 1;
        state.max_unacknowledged_batches = 1;
        state.batch_quota = 0.0;

        state.on_chunk_batch_received(f32::NAN);

        assert_eq!(state.unacknowledged_batches, 0);
        assert_eq!(state.desired_chunks_per_tick, MIN_CHUNKS_PER_TICK);
        assert_eq!(state.batch_quota, 1.0);
        assert_eq!(state.max_unacknowledged_batches, MAX_UNACKNOWLEDGED_BATCHES);

        state.on_chunk_batch_received(128.0);
        assert_eq!(state.desired_chunks_per_tick, MAX_CHUNKS_PER_TICK);
        assert_eq!(state.unacknowledged_batches, 0);
    }

    #[test]
    fn reset_view_clears_ready_chunks_and_resets_flow_control() {
        let mut state = ChunkSendState::new("minecraft:overworld".to_string(), 0, 0, 1, 4);
        state.ready_chunks.push_back(loaded_chunk(0, 1));
        state.unacknowledged_batches = 3;
        state.max_unacknowledged_batches = MAX_UNACKNOWLEDGED_BATCHES;
        state.desired_chunks_per_tick = 32.0;
        state.batch_quota = 8.0;

        state.reset_view(2, -3);

        assert!(state.ready_chunks.is_empty());
        assert_eq!(state.unacknowledged_batches, 0);
        assert_eq!(
            state.max_unacknowledged_batches,
            INITIAL_MAX_UNACKNOWLEDGED_BATCHES
        );
        assert_eq!(state.desired_chunks_per_tick, START_CHUNKS_PER_TICK);
        assert_eq!(state.batch_quota, 0.0);
        assert!(state.pending_chunks.contains(&(2, -3)));
    }

    #[test]
    fn ready_chunks_are_taken_by_distance_from_center() {
        let mut state = ChunkSendState::new("minecraft:overworld".to_string(), 0, 0, 2, 4);
        state.ready_chunks.push_back(loaded_chunk(2, 0));
        state.ready_chunks.push_back(loaded_chunk(0, 1));
        state.ready_chunks.push_back(loaded_chunk(1, 0));
        state.ready_chunks.push_back(loaded_chunk(-2, 2));

        let selected = state.take_ready_chunks(3);
        let selected_positions = selected
            .iter()
            .map(|chunk| (chunk.chunk_x, chunk.chunk_z))
            .collect::<Vec<_>>();

        assert_eq!(selected_positions, vec![(1, 0), (0, 1), (2, 0)]);
        assert_eq!(state.ready_chunks.len(), 1);
        assert_eq!(
            state
                .ready_chunks
                .front()
                .map(|chunk| (chunk.chunk_x, chunk.chunk_z)),
            Some((-2, 2))
        );
    }

    #[test]
    fn stale_ready_chunks_are_discarded_before_send_selection() {
        let mut state = ChunkSendState::new("minecraft:overworld".to_string(), 0, 0, 1, 4);
        state.visible_chunks.insert((0, 1));
        state.ready_chunks.push_back(loaded_chunk(0, 1));
        state.ready_chunks.push_back(loaded_chunk(5, 5));
        state.ready_chunks.push_back(loaded_chunk(1, 0));

        let selected = state.take_ready_chunks(4);

        assert_eq!(selected.len(), 1);
        assert_eq!((selected[0].chunk_x, selected[0].chunk_z), (1, 0));
        assert!(state.ready_chunks.is_empty());
    }
}

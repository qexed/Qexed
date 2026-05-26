use anyhow::{Context, Result};
use std::{
    collections::{HashMap, HashSet, VecDeque},
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

pub(super) struct ChunkSendState {
    pub(super) dimension: String,
    pub(super) center_x: i32,
    pub(super) center_z: i32,
    pub(super) view_distance: i32,
    pub(super) load_parallelism: usize,
    pub(super) visible_chunks: HashSet<(i32, i32)>,
    pub(super) pending_chunks: VecDeque<(i32, i32)>,
    pub(super) pending_unloads: HashMap<(i32, i32), Instant>,
    pub(super) loading_chunks: HashSet<(i32, i32)>,
}

pub(super) struct ChunkLoadResult {
    chunk_x: i32,
    chunk_z: i32,
    payload: Result<bytes::Bytes>,
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
            pending_unloads: HashMap::new(),
            loading_chunks: HashSet::new(),
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

    pub(super) fn reset_view(&mut self, center_x: i32, center_z: i32) {
        self.center_x = center_x;
        self.center_z = center_z;
        self.visible_chunks.clear();
        self.pending_chunks.clear();
        self.pending_unloads.clear();
        self.loading_chunks.clear();
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

        let world_for_task = world.clone();
        let dimension = self.dimension.clone();
        let cache_epoch = world.cache_epoch();
        let chunk_payload = tokio::task::spawn_blocking(move || {
            build_chunk_payload_sync(world_for_task, dimension, chunk.0, chunk.1, cache_epoch)
        })
        .await
        .context("join center chunk packet build task")??;

        sink.send(ChunkBatchStart {}).await?;
        sink.send_raw(chunk_payload).await?;
        for update in world.placed_block_updates(&self.dimension, chunk.0, chunk.1) {
            sink.send(update).await?;
        }
        self.visible_chunks.insert(chunk);
        plugins.emit_chunk_load(&self.dimension, chunk.0, chunk.1);
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
            let world_for_task = world.clone();
            let dimension = self.dimension.clone();
            let chunk_x = *chunk_x;
            let chunk_z = *chunk_z;
            let chunk_payload = tokio::task::spawn_blocking(move || {
                build_chunk_payload_sync(world_for_task, dimension, chunk_x, chunk_z, cache_epoch)
            })
            .await
            .context("join chunk packet build task")??;
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

            let world = world.clone();
            let dimension = self.dimension.clone();
            let sender = sender.clone();
            let cache_epoch = world.cache_epoch();
            tokio::task::spawn_blocking(move || {
                let payload =
                    build_chunk_payload_sync(world, dimension, chunk_x, chunk_z, cache_epoch);
                let _ = sender.send(ChunkLoadResult {
                    chunk_x,
                    chunk_z,
                    payload,
                });
            });
            self.loading_chunks.insert(chunk);
        }
    }

    pub(super) fn has_chunk_work(&self) -> bool {
        !self.loading_chunks.is_empty() || !self.pending_chunks.is_empty()
    }

    pub(super) fn has_pending_unloads(&self) -> bool {
        !self.pending_unloads.is_empty()
    }

    pub(super) async fn send_loaded_chunk<W>(
        &mut self,
        sink: &mut qexed_tcp_connect::PacketSink<W>,
        world: &WorldManager,
        plugins: &crate::plugins::PluginManager,
        loaded: ChunkLoadResult,
        sender: &tokio::sync::mpsc::UnboundedSender<ChunkLoadResult>,
    ) -> Result<()>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        let chunk_x = loaded.chunk_x;
        let chunk_z = loaded.chunk_z;
        self.loading_chunks.remove(&(chunk_x, chunk_z));

        if self.visible_chunks.contains(&(chunk_x, chunk_z))
            || !visible_chunks(self.center_x, self.center_z, self.view_distance)
                .contains(&(chunk_x, chunk_z))
        {
            self.start_next_chunk_load(world, Some(sender));
            return Ok(());
        }

        let chunk_payload = loaded.payload?;

        sink.send(ChunkBatchStart {}).await?;
        sink.send_raw(chunk_payload).await?;
        for update in world.placed_block_updates(&self.dimension, chunk_x, chunk_z) {
            sink.send(update).await?;
        }
        self.visible_chunks.insert((chunk_x, chunk_z));
        plugins.emit_chunk_load(&self.dimension, chunk_x, chunk_z);
        sink.send(ChunkBatchFinished {
            batch_size: VarInt(1),
        })
        .await?;
        sink.flush().await?;

        self.start_next_chunk_load(world, Some(sender));
        Ok(())
    }
}

fn build_chunk_payload_sync(
    world: WorldManager,
    dimension: String,
    chunk_x: i32,
    chunk_z: i32,
    cache_epoch: u64,
) -> Result<bytes::Bytes> {
    let total_start = Instant::now();
    let chunk_start = Instant::now();
    let chunk = world.network_chunk_for_session(&dimension, chunk_x, chunk_z, cache_epoch)?;
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

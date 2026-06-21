use std::sync::{Arc, Mutex, mpsc};

use qexed_save::{DimensionId, SaveService};

use crate::{WorldLightAlgorithm, WorldStorage, generator_rpc, region};

#[derive(Clone, Debug)]
pub struct WorldManager {
    storage: WorldStorage,
    light_algorithm: WorldLightAlgorithm,
    write_queue: WorldWriteQueue,
    cache: Arc<Mutex<RegionChunkCache>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PrecompiledChunkSettings {
    pub enable: bool,
    pub light: bool,
    pub max_cached_packets: usize,
    pub max_cached_packet_bytes: usize,
    pub block_state_cache_limit: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NetworkChunkLoadSource {
    Saved,
    LocalGenerated,
    VanillaGenerated,
    EmptyFallback,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChunkSyncCause {
    InitialLogin,
    PlayerMove,
    CompletionTick,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChunkLoadEvent {
    pub chunk_x: i32,
    pub chunk_z: i32,
    pub source: NetworkChunkLoadSource,
}

impl ChunkLoadEvent {
    pub fn new(chunk_x: i32, chunk_z: i32, source: NetworkChunkLoadSource) -> Self {
        Self {
            chunk_x,
            chunk_z,
            source,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChunkUnloadEvent {
    pub chunk_x: i32,
    pub chunk_z: i32,
}

impl ChunkUnloadEvent {
    pub fn new(chunk_x: i32, chunk_z: i32) -> Self {
        Self { chunk_x, chunk_z }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct NetworkChunkSourceCounts {
    pub saved: usize,
    pub local_generated: usize,
    pub vanilla_generated: usize,
    pub empty_fallback: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChunkSyncEvent {
    pub cause: ChunkSyncCause,
    pub center_chunk_x: i32,
    pub center_chunk_z: i32,
    pub loaded: Vec<ChunkLoadEvent>,
    pub unloaded: Vec<ChunkUnloadEvent>,
}

impl ChunkSyncEvent {
    pub fn new(
        cause: ChunkSyncCause,
        center_chunk_x: i32,
        center_chunk_z: i32,
        loaded: Vec<ChunkLoadEvent>,
        unloaded: Vec<ChunkUnloadEvent>,
    ) -> Self {
        Self {
            cause,
            center_chunk_x,
            center_chunk_z,
            loaded,
            unloaded,
        }
    }

    pub fn loaded_count(&self) -> usize {
        self.loaded.len()
    }

    pub fn unloaded_count(&self) -> usize {
        self.unloaded.len()
    }

    pub fn source_counts(&self) -> NetworkChunkSourceCounts {
        let mut counts = NetworkChunkSourceCounts::default();
        for load in &self.loaded {
            match load.source {
                NetworkChunkLoadSource::Saved => counts.saved += 1,
                NetworkChunkLoadSource::LocalGenerated => counts.local_generated += 1,
                NetworkChunkLoadSource::VanillaGenerated => counts.vanilla_generated += 1,
                NetworkChunkLoadSource::EmptyFallback => counts.empty_fallback += 1,
            }
        }
        counts
    }
}

#[derive(Clone, Debug)]
pub struct NetworkChunkLoad {
    pub chunk: qexed_protocol::to_client::play::map_chunk::MapChunk,
    pub source: NetworkChunkLoadSource,
}

#[derive(Clone)]
struct WorldWriteQueue {
    sender: Arc<Mutex<Option<mpsc::Sender<WorldWriteTask>>>>,
}

impl std::fmt::Debug for WorldWriteQueue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("WorldWriteQueue")
    }
}

enum WorldWriteTask {
    Run(Box<dyn FnOnce() + Send + 'static>),
    Flush(mpsc::Sender<()>),
}

#[derive(Debug, Default)]
struct RegionChunkCache {
    chunks: std::collections::HashMap<ChunkKey, region::ChunkData>,
    order: std::collections::VecDeque<ChunkKey>,
    limit: usize,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct ChunkKey {
    dimension: DimensionId,
    chunk_x: i32,
    chunk_z: i32,
}

impl Default for PrecompiledChunkSettings {
    fn default() -> Self {
        Self {
            enable: false,
            light: true,
            max_cached_packets: 256,
            max_cached_packet_bytes: 16 * 1024 * 1024,
            block_state_cache_limit: 65_536,
        }
    }
}

impl WorldManager {
    pub fn new(save: SaveService) -> Self {
        Self {
            storage: WorldStorage::new(save),
            light_algorithm: WorldLightAlgorithm::default(),
            write_queue: WorldWriteQueue::new(),
            cache: Arc::new(Mutex::new(RegionChunkCache::new(256))),
        }
    }

    pub fn with_light_algorithm(mut self, light_algorithm: WorldLightAlgorithm) -> Self {
        self.light_algorithm = light_algorithm;
        self
    }

    pub fn storage(&self) -> &WorldStorage {
        &self.storage
    }

    pub fn cached_region_chunk_count(&self) -> usize {
        self.cache
            .lock()
            .expect("world cache poisoned")
            .chunks
            .len()
    }

    pub fn clear_cache(&self) -> usize {
        self.cache.lock().expect("world cache poisoned").clear()
    }

    pub fn load_region_chunk(
        &self,
        dimension: &DimensionId,
        chunk_x: i32,
        chunk_z: i32,
    ) -> anyhow::Result<Option<region::ChunkData>> {
        let key = ChunkKey {
            dimension: dimension.clone(),
            chunk_x,
            chunk_z,
        };
        if let Some(chunk) = self.cache.lock().expect("world cache poisoned").get(&key) {
            return Ok(Some(chunk));
        }

        let chunk = self
            .storage
            .load_region_chunk(dimension, chunk_x, chunk_z)?;
        if let Some(chunk) = &chunk {
            self.cache
                .lock()
                .expect("world cache poisoned")
                .insert(key, chunk.clone());
        }
        Ok(chunk)
    }

    pub fn write_region_chunk(
        &self,
        dimension: &DimensionId,
        chunk_x: i32,
        chunk_z: i32,
        chunk: region::ChunkData,
    ) -> anyhow::Result<()> {
        self.storage
            .write_region_chunk(dimension, chunk_x, chunk_z, chunk.clone())?;
        self.cache.lock().expect("world cache poisoned").insert(
            ChunkKey {
                dimension: dimension.clone(),
                chunk_x,
                chunk_z,
            },
            chunk,
        );
        Ok(())
    }

    pub fn queue_region_chunk_write(
        &self,
        dimension: DimensionId,
        chunk_x: i32,
        chunk_z: i32,
        chunk: region::ChunkData,
    ) {
        let manager = self.clone();
        self.write_queue.spawn(move || {
            if let Err(err) = manager.write_region_chunk(&dimension, chunk_x, chunk_z, chunk) {
                log::warn!("failed to write queued world chunk: {err:#}");
            }
        });
    }

    pub fn flush_writes(&self) {
        self.write_queue.flush();
    }

    pub fn network_chunk(
        &self,
        dimension: &DimensionId,
        chunk_x: i32,
        chunk_z: i32,
    ) -> anyhow::Result<Option<qexed_protocol::to_client::play::map_chunk::MapChunk>> {
        self.storage
            .load_network_chunk(dimension, chunk_x, chunk_z, self.light_algorithm)
    }

    pub fn block_state_at(
        &self,
        dimension: &DimensionId,
        position: &qexed_packet::net_types::Position,
    ) -> anyhow::Result<Option<i32>> {
        self.storage.block_state_at(dimension, position)
    }

    pub fn set_block_state(
        &self,
        dimension: &DimensionId,
        position: &qexed_packet::net_types::Position,
        block_state: i32,
        fallback_block_state: Option<i32>,
    ) -> anyhow::Result<()> {
        self.storage
            .set_block_state(dimension, position, block_state, fallback_block_state)?;
        self.cache
            .lock()
            .expect("world cache poisoned")
            .remove(&ChunkKey {
                dimension: dimension.clone(),
                chunk_x: position.x.div_euclid(16),
                chunk_z: position.z.div_euclid(16),
            });
        Ok(())
    }

    pub async fn request_vanilla_generated_chunk(
        &self,
        client: &generator_rpc::VanillaWorldgenClient,
        dimension: &DimensionId,
        chunk_x: i32,
        chunk_z: i32,
    ) -> anyhow::Result<Option<region::ChunkData>> {
        let chunk = self
            .storage
            .request_vanilla_generated_chunk(client, dimension, chunk_x, chunk_z)
            .await?;
        if let Some(chunk) = &chunk {
            self.cache.lock().expect("world cache poisoned").insert(
                ChunkKey {
                    dimension: dimension.clone(),
                    chunk_x,
                    chunk_z,
                },
                chunk.clone(),
            );
        }
        Ok(chunk)
    }

    pub fn request_local_generated_chunk(
        &self,
        generator: &qexed_worldgen::WorldGenerator,
        dimension: &DimensionId,
        chunk_x: i32,
        chunk_z: i32,
    ) -> anyhow::Result<Option<region::ChunkData>> {
        let chunk = self
            .storage
            .request_local_generated_chunk(generator, dimension, chunk_x, chunk_z)?;
        if let Some(chunk) = &chunk {
            self.cache.lock().expect("world cache poisoned").insert(
                ChunkKey {
                    dimension: dimension.clone(),
                    chunk_x,
                    chunk_z,
                },
                chunk.clone(),
            );
        }
        Ok(chunk)
    }

    pub async fn ensure_network_chunk(
        &self,
        local_generator: Option<&qexed_worldgen::WorldGenerator>,
        client: Option<&generator_rpc::VanillaWorldgenClient>,
        dimension: &DimensionId,
        chunk_x: i32,
        chunk_z: i32,
    ) -> anyhow::Result<qexed_protocol::to_client::play::map_chunk::MapChunk> {
        Ok(self
            .ensure_network_chunk_with_event(local_generator, client, dimension, chunk_x, chunk_z)
            .await?
            .chunk)
    }

    pub async fn ensure_network_chunk_with_event(
        &self,
        local_generator: Option<&qexed_worldgen::WorldGenerator>,
        client: Option<&generator_rpc::VanillaWorldgenClient>,
        dimension: &DimensionId,
        chunk_x: i32,
        chunk_z: i32,
    ) -> anyhow::Result<NetworkChunkLoad> {
        if let Some(chunk) = self.network_chunk(dimension, chunk_x, chunk_z)? {
            log::debug!(
                "loaded saved world chunk: dimension={}:{} chunk=({}, {})",
                dimension.namespace(),
                dimension.value(),
                chunk_x,
                chunk_z
            );
            return Ok(NetworkChunkLoad {
                chunk,
                source: NetworkChunkLoadSource::Saved,
            });
        }

        if let Some(generator) = local_generator {
            log::info!(
                "generating world chunk locally from Mojang cache: dimension={}:{} chunk=({}, {})",
                dimension.namespace(),
                dimension.value(),
                chunk_x,
                chunk_z
            );
            self.request_local_generated_chunk(generator, dimension, chunk_x, chunk_z)?;
            if let Some(chunk) = self.network_chunk(dimension, chunk_x, chunk_z)? {
                return Ok(NetworkChunkLoad {
                    chunk,
                    source: NetworkChunkLoadSource::LocalGenerated,
                });
            }
        }

        if let Some(client) = client {
            log::info!(
                "generating world chunk through Java worldgen test oracle: dimension={}:{} chunk=({}, {})",
                dimension.namespace(),
                dimension.value(),
                chunk_x,
                chunk_z
            );
            self.request_vanilla_generated_chunk(client, dimension, chunk_x, chunk_z)
                .await?;
            if let Some(generator) = local_generator
                && let Err(err) =
                    self.compare_local_worldgen_with_saved(generator, dimension, chunk_x, chunk_z)
            {
                log::warn!(
                    "local worldgen comparison failed after Java generation: dimension={}:{} chunk=({}, {}) error={err:#}",
                    dimension.namespace(),
                    dimension.value(),
                    chunk_x,
                    chunk_z
                );
            }
            if let Some(chunk) = self.network_chunk(dimension, chunk_x, chunk_z)? {
                return Ok(NetworkChunkLoad {
                    chunk,
                    source: NetworkChunkLoadSource::VanillaGenerated,
                });
            }
        }

        Ok(NetworkChunkLoad {
            chunk: crate::empty_chunk_packet(chunk_x, chunk_z)?,
            source: NetworkChunkLoadSource::EmptyFallback,
        })
    }

    fn compare_local_worldgen_with_saved(
        &self,
        generator: &qexed_worldgen::WorldGenerator,
        dimension: &DimensionId,
        chunk_x: i32,
        chunk_z: i32,
    ) -> anyhow::Result<()> {
        let comparison =
            self.compare_local_worldgen_with_region_chunk(generator, dimension, chunk_x, chunk_z)?;
        if comparison.equal {
            log::debug!(
                "local worldgen matches Java chunk: dimension={}:{} chunk=({}, {})",
                dimension.namespace(),
                dimension.value(),
                chunk_x,
                chunk_z
            );
        } else {
            log::warn!(
                "local worldgen differs from Java chunk: dimension={}:{} chunk=({}, {}) first_differences={}",
                dimension.namespace(),
                dimension.value(),
                chunk_x,
                chunk_z,
                comparison.differences.join(" | ")
            );
        }
        Ok(())
    }

    pub fn compare_local_worldgen_with_region_chunk(
        &self,
        generator: &qexed_worldgen::WorldGenerator,
        dimension: &DimensionId,
        chunk_x: i32,
        chunk_z: i32,
    ) -> anyhow::Result<qexed_worldgen::ChunkComparison> {
        let Some(saved) = self.load_region_chunk(dimension, chunk_x, chunk_z)? else {
            anyhow::bail!("region chunk is missing");
        };
        let raw = saved.decompress()?;
        let (_, expected_root) = qexed_nbt::from_slice(&raw)?;
        let actual_root = generator.generate_chunk_nbt(qexed_worldgen::ChunkRequest {
            dimension: &format!("{}:{}", dimension.namespace(), dimension.value()),
            chunk_x,
            chunk_z,
        })?;
        Ok(qexed_worldgen::compare_chunk_nbt(
            &expected_root,
            &actual_root,
        ))
    }
}

impl WorldWriteQueue {
    fn new() -> Self {
        Self {
            sender: Arc::new(Mutex::new(None)),
        }
    }

    fn sender(&self) -> mpsc::Sender<WorldWriteTask> {
        let mut sender = self.sender.lock().expect("world write queue poisoned");
        if let Some(sender) = sender.as_ref() {
            return sender.clone();
        }
        let (tx, receiver) = mpsc::channel::<WorldWriteTask>();
        std::thread::Builder::new()
            .name("qexed-world-write".to_string())
            .spawn(move || {
                while let Ok(task) = receiver.recv() {
                    match task {
                        WorldWriteTask::Run(task) => task(),
                        WorldWriteTask::Flush(done) => {
                            let _ = done.send(());
                        }
                    }
                }
            })
            .expect("create world write queue thread");
        *sender = Some(tx.clone());
        tx
    }

    fn spawn(&self, task: impl FnOnce() + Send + 'static) {
        if self
            .sender()
            .send(WorldWriteTask::Run(Box::new(task)))
            .is_err()
        {
            log::warn!("failed to queue world write task because write queue stopped");
        }
    }

    fn flush(&self) {
        let (sender, receiver) = mpsc::channel();
        if self.sender().send(WorldWriteTask::Flush(sender)).is_ok() {
            let _ = receiver.recv();
        }
    }
}

impl RegionChunkCache {
    fn new(limit: usize) -> Self {
        Self {
            chunks: std::collections::HashMap::new(),
            order: std::collections::VecDeque::new(),
            limit,
        }
    }

    fn get(&mut self, key: &ChunkKey) -> Option<region::ChunkData> {
        let chunk = self.chunks.get(key).cloned()?;
        self.touch(key.clone());
        Some(chunk)
    }

    fn insert(&mut self, key: ChunkKey, chunk: region::ChunkData) {
        self.chunks.insert(key.clone(), chunk);
        self.touch(key.clone());
        while self.chunks.len() > self.limit {
            let Some(oldest) = self.order.pop_front() else {
                break;
            };
            if oldest != key {
                self.chunks.remove(&oldest);
            }
        }
    }

    fn remove(&mut self, key: &ChunkKey) {
        self.chunks.remove(key);
        self.order.retain(|existing| existing != key);
    }

    fn clear(&mut self) -> usize {
        let cleared = self.chunks.len();
        self.chunks.clear();
        self.order.clear();
        cleared
    }

    fn touch(&mut self, key: ChunkKey) {
        self.order.retain(|existing| existing != &key);
        self.order.push_back(key);
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ChunkLoadEvent, ChunkSyncCause, ChunkSyncEvent, ChunkUnloadEvent, NetworkChunkLoadSource,
        WorldManager,
    };

    #[test]
    fn manager_caches_loaded_region_chunks_and_flushes_queued_writes() {
        let dir = tempfile::tempdir().unwrap();
        let mut config = qexed_config::app::qexed_save::Save::default();
        config.root.universe = dir.path().to_string_lossy().to_string();
        let manager = WorldManager::new(qexed_save::SaveService::new(config).unwrap());
        let dimension = qexed_save::DimensionId::overworld();
        let chunk = crate::region::ChunkData::zlib(b"queued").unwrap();

        manager.queue_region_chunk_write(dimension.clone(), 0, 0, chunk);
        manager.flush_writes();

        assert_eq!(manager.cached_region_chunk_count(), 1);
        let loaded = manager
            .load_region_chunk(&dimension, 0, 0)
            .unwrap()
            .unwrap();
        assert_eq!(loaded.decompress().unwrap(), b"queued");
        assert_eq!(manager.clear_cache(), 1);
    }

    #[test]
    fn chunk_sync_event_counts_load_sources_and_unloads() {
        let event = ChunkSyncEvent::new(
            ChunkSyncCause::PlayerMove,
            2,
            -1,
            vec![
                ChunkLoadEvent::new(2, -1, NetworkChunkLoadSource::Saved),
                ChunkLoadEvent::new(3, -1, NetworkChunkLoadSource::LocalGenerated),
                ChunkLoadEvent::new(2, 0, NetworkChunkLoadSource::VanillaGenerated),
                ChunkLoadEvent::new(3, 0, NetworkChunkLoadSource::EmptyFallback),
            ],
            vec![ChunkUnloadEvent::new(0, -1)],
        );

        let counts = event.source_counts();

        assert_eq!(event.cause, ChunkSyncCause::PlayerMove);
        assert_eq!(event.loaded_count(), 4);
        assert_eq!(event.unloaded_count(), 1);
        assert_eq!(counts.saved, 1);
        assert_eq!(counts.local_generated, 1);
        assert_eq!(counts.vanilla_generated, 1);
        assert_eq!(counts.empty_fallback, 1);
    }
}

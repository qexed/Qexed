use anyhow::{Context, Result};
use qexed_packet::{PacketCodec, net_types::VarInt};
use qexed_protocol::to_client::play::{
    block_update::BlockUpdate,
    light_update::LightUpdate,
    map_chunk::{Light, MapChunk},
};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, AtomicUsize, Ordering},
    mpsc,
};

use super::{
    AIR_BLOCK_STATE_ID, CHUNK_DAMPENING_LEN, LightDampeningNeighborhood, WORLD_MAX_Y, WORLD_MIN_Y,
    WorldLightAlgorithm, WorldLightMode, block_light_dampening_index, chunk_nbt, generator,
    gpu_light, light_for_mode, light_from_sky_values, light_update_data, region, replace_sky_light,
    sky_light_from_neighbourhood,
};
#[derive(Clone, Debug)]
pub struct WorldManager {
    save_path: std::path::PathBuf,
    worlds: Arc<std::collections::HashMap<String, WorldStorage>>,
    instances: Arc<std::collections::HashMap<String, WorldInstanceStorage>>,
    read_only: bool,
    light_mode: WorldLightMode,
    light_algorithm: WorldLightAlgorithm,
    light_gpu: Option<Arc<gpu_light::GpuLightEngine>>,
    generator: Arc<dyn generator::WorldChunkGenerator>,
    placed_blocks: Arc<Mutex<std::collections::HashMap<BlockKey, PendingBlock>>>,
    region_chunk_cache: Arc<Mutex<RegionChunkCache>>,
    precompiled_chunk_cache: Arc<Mutex<PrecompiledChunkCache>>,
    block_state_cache: Arc<Mutex<BlockStateCache>>,
    dirty_chunk_sections:
        Arc<Mutex<std::collections::HashMap<ChunkKey, std::collections::BTreeSet<i32>>>>,
    precompiled_chunks: PrecompiledChunkSettings,
    chunk_light_dampening: Arc<Mutex<std::collections::HashMap<ChunkKey, Vec<u8>>>>,
    region_locks: Arc<Mutex<std::collections::HashMap<RegionKey, Arc<Mutex<()>>>>>,
    block_write_queue: WorldWriteQueue,
    block_write_revision: Arc<AtomicU64>,
    active_sessions: Arc<AtomicUsize>,
    cache_epoch: Arc<AtomicU64>,
}

const REGION_CHUNK_CACHE_LIMIT: usize = 256;
const DEFAULT_PRECOMPILED_CHUNK_PACKET_LIMIT: usize = 256;
const DEFAULT_PRECOMPILED_CHUNK_PACKET_BYTES: usize = 16 * 1024 * 1024;
const DEFAULT_BLOCK_STATE_CACHE_LIMIT: usize = 65_536;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PrecompiledChunkSettings {
    pub enable: bool,
    pub light: bool,
    pub max_cached_packets: usize,
    pub max_cached_packet_bytes: usize,
    pub block_state_cache_limit: usize,
}

impl Default for PrecompiledChunkSettings {
    fn default() -> Self {
        Self {
            enable: false,
            light: true,
            max_cached_packets: DEFAULT_PRECOMPILED_CHUNK_PACKET_LIMIT,
            max_cached_packet_bytes: DEFAULT_PRECOMPILED_CHUNK_PACKET_BYTES,
            block_state_cache_limit: DEFAULT_BLOCK_STATE_CACHE_LIMIT,
        }
    }
}

impl From<&qexed_config::app::qexed::server::PrecompiledChunks> for PrecompiledChunkSettings {
    fn from(config: &qexed_config::app::qexed::server::PrecompiledChunks) -> Self {
        Self {
            enable: config.enable,
            light: config.light,
            max_cached_packets: config.max_cached_packets,
            max_cached_packet_bytes: config.max_cached_packet_bytes,
            block_state_cache_limit: config.block_state_cache_limit,
        }
    }
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
    #[cfg(test)]
    Flush(mpsc::Sender<()>),
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
                        #[cfg(test)]
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

    #[cfg(test)]
    fn flush(&self) {
        let (sender, receiver) = mpsc::channel();
        if self.sender().send(WorldWriteTask::Flush(sender)).is_ok() {
            let _ = receiver.recv();
        }
    }
}

#[derive(Debug, Default)]
struct RegionChunkCache {
    chunks: std::collections::HashMap<ChunkKey, region::ChunkData>,
    order: std::collections::VecDeque<ChunkKey>,
}

impl RegionChunkCache {
    fn get(&mut self, key: &ChunkKey) -> Option<region::ChunkData> {
        let chunk = self.chunks.get(key).cloned()?;
        self.touch(key.clone());
        Some(chunk)
    }

    fn insert(&mut self, key: ChunkKey, chunk: region::ChunkData) {
        self.chunks.insert(key.clone(), chunk);
        self.touch(key.clone());
        while self.chunks.len() > REGION_CHUNK_CACHE_LIMIT {
            let Some(oldest) = self.order.pop_front() else {
                break;
            };
            if oldest != key {
                self.chunks.remove(&oldest);
            }
        }
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

#[derive(Debug, Default)]
struct PrecompiledChunkCache {
    packets: std::collections::HashMap<PrecompiledChunkKey, PrecompiledChunkPacket>,
    order: std::collections::VecDeque<PrecompiledChunkKey>,
    total_bytes: usize,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct PrecompiledChunkKey {
    chunk: ChunkKey,
    compression_threshold: Option<i32>,
}

impl PrecompiledChunkKey {
    fn new(dimension: &str, x: i32, z: i32, compression_threshold: Option<i32>) -> Self {
        Self {
            chunk: ChunkKey::new(dimension, x, z),
            compression_threshold,
        }
    }
}

#[derive(Debug, Clone)]
enum PrecompiledChunkPacket {
    FullFrame { frame: bytes::Bytes },
    WithoutLight { prefix: bytes::Bytes },
}

impl PrecompiledChunkPacket {
    fn byte_len(&self) -> usize {
        match self {
            Self::FullFrame { frame } => frame.len(),
            Self::WithoutLight { prefix } => prefix.len(),
        }
    }
}

impl PrecompiledChunkCache {
    fn get(&mut self, key: &PrecompiledChunkKey) -> Option<PrecompiledChunkPacket> {
        let packet = self.packets.get(key)?;
        let packet = packet.clone();
        self.touch(key.clone());
        Some(packet)
    }

    fn insert(
        &mut self,
        key: PrecompiledChunkKey,
        packet: PrecompiledChunkPacket,
        packet_limit: usize,
        byte_limit: usize,
    ) {
        let packet_bytes = packet.byte_len();
        if byte_limit != 0 && packet_bytes > byte_limit {
            self.remove(&key);
            return;
        }

        if let Some(previous) = self.packets.insert(key.clone(), packet) {
            self.total_bytes = self.total_bytes.saturating_sub(previous.byte_len());
        }
        self.total_bytes = self.total_bytes.saturating_add(packet_bytes);
        self.touch(key.clone());
        while (packet_limit != 0 && self.packets.len() > packet_limit)
            || (byte_limit != 0 && self.total_bytes > byte_limit)
        {
            let Some(oldest) = self.order.pop_front() else {
                break;
            };
            if self.packets.len() == 1 && oldest == key {
                self.order.push_back(oldest);
                break;
            }
            if let Some(removed) = self.packets.remove(&oldest) {
                self.total_bytes = self.total_bytes.saturating_sub(removed.byte_len());
            }
        }
    }

    fn invalidate(&mut self, key: &ChunkKey) -> bool {
        let matching = self
            .packets
            .keys()
            .filter(|packet_key| packet_key.chunk == *key)
            .cloned()
            .collect::<Vec<_>>();
        let removed = !matching.is_empty();
        for key in matching {
            self.remove(&key);
        }
        removed
    }

    fn remove(&mut self, key: &PrecompiledChunkKey) -> Option<PrecompiledChunkPacket> {
        self.order.retain(|existing| existing != key);
        let removed = self.packets.remove(key)?;
        self.total_bytes = self.total_bytes.saturating_sub(removed.byte_len());
        Some(removed)
    }

    fn clear(&mut self) -> usize {
        let cleared = self.packets.len();
        self.packets.clear();
        self.order.clear();
        self.total_bytes = 0;
        cleared
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.packets.len()
    }

    #[cfg(test)]
    fn total_bytes(&self) -> usize {
        self.total_bytes
    }

    fn touch(&mut self, key: PrecompiledChunkKey) {
        self.order.retain(|existing| existing != &key);
        self.order.push_back(key);
    }
}

#[derive(Debug, Default)]
struct BlockStateCache {
    blocks: std::collections::HashMap<BlockKey, i32>,
    order: std::collections::VecDeque<BlockKey>,
}

impl BlockStateCache {
    fn get(&mut self, key: &BlockKey) -> Option<i32> {
        let block_state = *self.blocks.get(key)?;
        self.touch(key.clone());
        Some(block_state)
    }

    fn insert(&mut self, key: BlockKey, block_state: i32, limit: usize) {
        if limit == 0 {
            return;
        }
        self.blocks.insert(key.clone(), block_state);
        self.touch(key.clone());
        while self.blocks.len() > limit {
            let Some(oldest) = self.order.pop_front() else {
                break;
            };
            if oldest != key {
                self.blocks.remove(&oldest);
            }
        }
    }

    fn invalidate_chunk(&mut self, chunk_key: &ChunkKey) {
        self.blocks.retain(|key, _| {
            !(key.dimension == chunk_key.dimension
                && key.x.div_euclid(16) == chunk_key.x
                && key.z.div_euclid(16) == chunk_key.z)
        });
        self.order.retain(|key| {
            !(key.dimension == chunk_key.dimension
                && key.x.div_euclid(16) == chunk_key.x
                && key.z.div_euclid(16) == chunk_key.z)
        });
    }

    fn clear(&mut self) -> usize {
        let cleared = self.blocks.len();
        self.blocks.clear();
        self.order.clear();
        cleared
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.blocks.len()
    }

    fn touch(&mut self, key: BlockKey) {
        self.order.retain(|existing| existing != &key);
        self.order.push_back(key);
    }
}

impl WorldManager {
    #[cfg(test)]
    pub fn new(save_path: impl Into<std::path::PathBuf>) -> Self {
        Self::with_light_mode(
            save_path,
            WorldLightMode::default(),
            WorldLightAlgorithm::default(),
            None,
            false,
        )
    }

    #[cfg(test)]
    pub fn with_light_mode(
        save_path: impl Into<std::path::PathBuf>,
        light_mode: WorldLightMode,
        light_algorithm: WorldLightAlgorithm,
        light_gpu: Option<Arc<gpu_light::GpuLightEngine>>,
        read_only: bool,
    ) -> Self {
        Self::with_generator(
            save_path,
            light_mode,
            light_algorithm,
            light_gpu,
            read_only,
            Arc::new(generator::EmptyWorldGenerator),
        )
    }

    pub fn with_generator(
        save_path: impl Into<std::path::PathBuf>,
        light_mode: WorldLightMode,
        light_algorithm: WorldLightAlgorithm,
        light_gpu: Option<Arc<gpu_light::GpuLightEngine>>,
        read_only: bool,
        generator: Arc<dyn generator::WorldChunkGenerator>,
    ) -> Self {
        Self {
            save_path: save_path.into(),
            worlds: Default::default(),
            instances: Default::default(),
            read_only,
            light_mode,
            light_algorithm,
            light_gpu,
            generator,
            placed_blocks: Default::default(),
            region_chunk_cache: Default::default(),
            precompiled_chunk_cache: Default::default(),
            block_state_cache: Default::default(),
            dirty_chunk_sections: Default::default(),
            precompiled_chunks: PrecompiledChunkSettings::default(),
            chunk_light_dampening: Default::default(),
            region_locks: Default::default(),
            block_write_queue: WorldWriteQueue::new(),
            block_write_revision: Default::default(),
            active_sessions: Default::default(),
            cache_epoch: Default::default(),
        }
    }

    pub fn with_precompiled_chunks(mut self, settings: PrecompiledChunkSettings) -> Self {
        self.precompiled_chunks = settings;
        self
    }

    pub fn with_instances(
        mut self,
        instances: &[qexed_config::app::qexed::server::WorldInstance],
    ) -> Self {
        self.instances = Arc::new(
            instances
                .iter()
                .filter_map(|instance| WorldInstanceStorage::from_config(&self.save_path, instance))
                .map(|instance| (instance.dimension.clone(), instance))
                .collect(),
        );
        self
    }

    pub fn with_worlds(
        mut self,
        worlds: &[qexed_config::app::qexed::server::WorldStorage],
    ) -> Self {
        self.worlds = Arc::new(
            worlds
                .iter()
                .filter_map(WorldStorage::from_config)
                .map(|world| (world.dimension.clone(), world))
                .collect(),
        );
        self
    }

    #[allow(dead_code)]
    pub fn save_path(&self) -> &std::path::Path {
        &self.save_path
    }

    #[allow(dead_code)]
    pub fn read_only(&self) -> bool {
        self.read_only
    }

    pub fn begin_session(&self) -> WorldSession {
        self.active_sessions.fetch_add(1, Ordering::AcqRel);
        WorldSession {
            world: self.clone(),
            active: true,
        }
    }

    pub fn cache_epoch(&self) -> u64 {
        self.cache_epoch.load(Ordering::Acquire)
    }

    pub fn precompiled_chunk_packets_enabled(&self) -> bool {
        self.precompiled_chunks.enable
    }

    pub fn precompiled_chunk_payload_includes_light(&self) -> bool {
        self.precompiled_chunks.light && !self.dynamic_light_enabled()
    }

    pub fn precompiled_chunk_packet(
        &self,
        dimension: &str,
        chunk_x: i32,
        chunk_z: i32,
        cache_epoch: u64,
        compression_threshold: Option<i32>,
    ) -> Result<Option<bytes::Bytes>> {
        if !self.precompiled_chunk_packets_enabled() {
            return Ok(None);
        }
        let packet = self
            .precompiled_chunk_cache
            .lock()
            .expect("world precompiled chunk cache poisoned")
            .get(&PrecompiledChunkKey::new(
                dimension,
                chunk_x,
                chunk_z,
                compression_threshold,
            ));
        match packet {
            Some(PrecompiledChunkPacket::FullFrame { frame }) => Ok(Some(frame)),
            Some(PrecompiledChunkPacket::WithoutLight { prefix }) => {
                let mut buf = bytes::BytesMut::from(prefix.as_ref());
                let mut writer = qexed_packet::PacketWriter::new(&mut buf);
                self.chunk_light_for_payload(dimension, chunk_x, chunk_z, Some(cache_epoch))
                    .serialize(&mut writer)?;
                Ok(Some(
                    qexed_tcp_connect::PacketSink::<tokio::io::Sink>::encode_payload_frame_with_threshold(
                        buf.freeze(),
                        compression_threshold,
                    )?,
                ))
            }
            None => Ok(None),
        }
    }

    pub fn remember_precompiled_chunk_frame(
        &self,
        dimension: &str,
        chunk_x: i32,
        chunk_z: i32,
        frame: bytes::Bytes,
        compression_threshold: Option<i32>,
    ) {
        if !self.can_remember_precompiled_chunk(dimension, chunk_x, chunk_z) {
            return;
        }
        if !self.precompiled_chunk_payload_includes_light() {
            return;
        }
        self.remember_precompiled_chunk_entry(
            dimension,
            chunk_x,
            chunk_z,
            PrecompiledChunkPacket::FullFrame { frame },
            compression_threshold,
        );
    }

    pub fn remember_precompiled_chunk_packet_without_light(
        &self,
        dimension: &str,
        chunk_x: i32,
        chunk_z: i32,
        prefix: bytes::Bytes,
        compression_threshold: Option<i32>,
    ) {
        if !self.can_remember_precompiled_chunk(dimension, chunk_x, chunk_z) {
            return;
        }
        self.remember_precompiled_chunk_entry(
            dimension,
            chunk_x,
            chunk_z,
            PrecompiledChunkPacket::WithoutLight { prefix },
            compression_threshold,
        );
    }

    fn can_remember_precompiled_chunk(&self, dimension: &str, chunk_x: i32, chunk_z: i32) -> bool {
        if !self.precompiled_chunk_packets_enabled() {
            return false;
        }
        let key = ChunkKey::new(dimension, chunk_x, chunk_z);
        let dirty = self
            .dirty_chunk_sections
            .lock()
            .expect("world dirty chunk section store poisoned")
            .contains_key(&key);
        if dirty {
            return false;
        }
        !self.chunk_has_pending_block_overlay(&key)
    }

    fn remember_precompiled_chunk_entry(
        &self,
        dimension: &str,
        chunk_x: i32,
        chunk_z: i32,
        packet: PrecompiledChunkPacket,
        compression_threshold: Option<i32>,
    ) {
        let key = PrecompiledChunkKey::new(dimension, chunk_x, chunk_z, compression_threshold);
        self.precompiled_chunk_cache
            .lock()
            .expect("world precompiled chunk cache poisoned")
            .insert(
                key.clone(),
                packet,
                self.precompiled_chunks.max_cached_packets,
                self.precompiled_chunks.max_cached_packet_bytes,
            );
        self.dirty_chunk_sections
            .lock()
            .expect("world dirty chunk section store poisoned")
            .remove(&key.chunk);
    }

    #[cfg(test)]
    pub(crate) fn cached_light_chunk_count(&self) -> usize {
        self.chunk_light_dampening
            .lock()
            .expect("world light cache poisoned")
            .len()
    }

    #[cfg(test)]
    pub(crate) fn cached_region_chunk_count(&self) -> usize {
        self.region_chunk_cache
            .lock()
            .expect("world region chunk cache poisoned")
            .chunks
            .len()
    }

    #[cfg(test)]
    pub(crate) fn cached_precompiled_chunk_count(&self) -> usize {
        self.precompiled_chunk_cache
            .lock()
            .expect("world precompiled chunk cache poisoned")
            .len()
    }

    #[cfg(test)]
    pub(crate) fn cached_precompiled_chunk_bytes(&self) -> usize {
        self.precompiled_chunk_cache
            .lock()
            .expect("world precompiled chunk cache poisoned")
            .total_bytes()
    }

    #[cfg(test)]
    pub(crate) fn cached_block_state_count(&self) -> usize {
        self.block_state_cache
            .lock()
            .expect("world block state cache poisoned")
            .len()
    }

    #[cfg(test)]
    pub(crate) fn dirty_chunk_sections(
        &self,
        dimension: &str,
        chunk_x: i32,
        chunk_z: i32,
    ) -> Vec<i32> {
        self.dirty_chunk_sections
            .lock()
            .expect("world dirty chunk section store poisoned")
            .get(&ChunkKey::new(dimension, chunk_x, chunk_z))
            .map(|sections| sections.iter().copied().collect())
            .unwrap_or_default()
    }

    #[cfg(test)]
    pub(crate) fn flush_block_writes(&self) {
        self.block_write_queue.flush();
    }

    #[cfg(test)]
    pub(crate) fn network_chunk(
        &self,
        dimension: &str,
        chunk_x: i32,
        chunk_z: i32,
    ) -> Result<MapChunk> {
        self.network_chunk_inner(dimension, chunk_x, chunk_z, None)
    }

    pub fn saved_network_chunk_for_session(
        &self,
        dimension: &str,
        chunk_x: i32,
        chunk_z: i32,
        cache_epoch: u64,
    ) -> Result<Option<MapChunk>> {
        if !self.cache_epoch_is_active(cache_epoch) {
            anyhow::bail!("chunk load cancelled because world session expired");
        }
        self.saved_network_chunk_inner(dimension, chunk_x, chunk_z, Some(cache_epoch))
    }

    pub fn generated_network_chunk_for_session(
        &self,
        dimension: &str,
        chunk_x: i32,
        chunk_z: i32,
        cache_epoch: u64,
    ) -> Result<MapChunk> {
        if !self.cache_epoch_is_active(cache_epoch) {
            anyhow::bail!("chunk load cancelled because world session expired");
        }
        self.generated_network_chunk_inner(dimension, chunk_x, chunk_z, Some(cache_epoch))
    }

    #[cfg(test)]
    fn network_chunk_inner(
        &self,
        dimension: &str,
        chunk_x: i32,
        chunk_z: i32,
        cache_epoch: Option<u64>,
    ) -> Result<MapChunk> {
        if let Some(packet) =
            self.saved_network_chunk_inner(dimension, chunk_x, chunk_z, cache_epoch)?
        {
            return Ok(packet);
        }
        self.generated_network_chunk_inner(dimension, chunk_x, chunk_z, cache_epoch)
    }

    fn saved_network_chunk_inner(
        &self,
        dimension: &str,
        chunk_x: i32,
        chunk_z: i32,
        cache_epoch: Option<u64>,
    ) -> Result<Option<MapChunk>> {
        if let Some(chunk) = self.read_region_chunk_for_network(dimension, chunk_x, chunk_z)? {
            match chunk_nbt::network_chunk_and_light_dampening_from_region(
                chunk_x,
                chunk_z,
                &chunk,
                WorldLightAlgorithm::Fast,
            ) {
                Ok((mut packet, dampening)) => {
                    self.remember_chunk_light_dampening(
                        dimension,
                        chunk_x,
                        chunk_z,
                        dampening,
                        cache_epoch,
                    );
                    if let WorldLightMode::Fixed(_) = self.light_mode {
                        packet.light = self.chunk_light();
                    } else {
                        let algorithm_light =
                            self.calculated_chunk_light(dimension, chunk_x, chunk_z, cache_epoch);
                        replace_sky_light(&mut packet.light, algorithm_light);
                    }
                    log::debug!(
                        "loaded saved chunk as network chunk: dimension={dimension}, chunk=({chunk_x}, {chunk_z})"
                    );
                    return Ok(Some(packet));
                }
                Err(err) => {
                    log::warn!(
                        "failed to convert saved chunk, falling back to generated chunk: dimension={dimension}, chunk=({chunk_x}, {chunk_z}), error={err:#}"
                    );
                }
            }
        }

        Ok(None)
    }

    fn generated_network_chunk_inner(
        &self,
        dimension: &str,
        chunk_x: i32,
        chunk_z: i32,
        cache_epoch: Option<u64>,
    ) -> Result<MapChunk> {
        log::trace!("generating chunk: dimension={dimension}, chunk=({chunk_x}, {chunk_z})");
        let generated =
            self.generator
                .generate(dimension, chunk_x, chunk_z, self.light_algorithm)?;
        self.remember_chunk_light_dampening(
            dimension,
            chunk_x,
            chunk_z,
            generated.light_dampening,
            cache_epoch,
        );
        if let Some(region_chunk) = generated.region_chunk
            && self.should_cache_full_region_chunks()
        {
            self.remember_region_chunk(dimension, chunk_x, chunk_z, region_chunk);
        }
        let mut packet = generated.packet;
        packet.light = match self.light_mode {
            WorldLightMode::Fixed(_) => self.chunk_light(),
            WorldLightMode::Static => {
                let mut light = packet.light;
                replace_sky_light(&mut light, self.chunk_light());
                light
            }
            WorldLightMode::Dynamic => {
                self.calculated_chunk_light(dimension, chunk_x, chunk_z, cache_epoch)
            }
        };
        Ok(packet)
    }

    pub fn dynamic_light_enabled(&self) -> bool {
        self.light_mode == WorldLightMode::Dynamic
    }

    pub fn light_update(&self, dimension: &str, chunk_x: i32, chunk_z: i32) -> LightUpdate {
        LightUpdate {
            chunk_x: VarInt(chunk_x),
            chunk_z: VarInt(chunk_z),
            light: light_update_data(self.chunk_light_for_update(dimension, chunk_x, chunk_z)),
        }
    }

    fn chunk_light(&self) -> Light {
        light_for_mode(self.light_mode)
    }

    fn load_region_chunk_unlocked(
        &self,
        dimension: &str,
        chunk_x: i32,
        chunk_z: i32,
    ) -> Result<Option<region::ChunkData>> {
        log::trace!(
            "从存档读取区块: dimension={dimension}, chunk=({chunk_x}, {chunk_z}), root={}",
            self.save_path.display()
        );
        let region_path = self
            .dimension_region_path(dimension)
            .join(region_file_name(chunk_x, chunk_z));
        if !region_path.exists() {
            let Some(instance) = self.instances.get(dimension) else {
                return Ok(None);
            };
            if !instance.copy_on_write {
                return Ok(None);
            }
            return self.load_instance_source_chunk(instance, chunk_x, chunk_z);
        }

        let region = region::AnvilRegion::from_file(&region_path)
            .with_context(|| format!("读取区域文件失败: {}", region_path.display()))?;
        let overlay_chunk = region
            .read_chunk(chunk_x, chunk_z)
            .with_context(|| format!("读取区块失败: {chunk_x}, {chunk_z}"))?;
        if overlay_chunk.is_some() {
            return Ok(overlay_chunk);
        }

        let Some(instance) = self.instances.get(dimension) else {
            return Ok(None);
        };
        if !instance.copy_on_write {
            return Ok(None);
        }

        self.load_instance_source_chunk(instance, chunk_x, chunk_z)
    }

    fn load_instance_source_chunk(
        &self,
        instance: &WorldInstanceStorage,
        chunk_x: i32,
        chunk_z: i32,
    ) -> Result<Option<region::ChunkData>> {
        let source_region_path = instance
            .source_region_path(self)
            .join(region_file_name(chunk_x, chunk_z));
        if !source_region_path.exists() {
            return Ok(None);
        }
        let source_region =
            region::AnvilRegion::from_file(&source_region_path).with_context(|| {
                format!(
                    "read source region failed: {}",
                    source_region_path.display()
                )
            })?;
        source_region
            .read_chunk(chunk_x, chunk_z)
            .with_context(|| format!("read source chunk failed: {chunk_x}, {chunk_z}"))
    }

    pub fn load_region_chunk(
        &self,
        dimension: &str,
        chunk_x: i32,
        chunk_z: i32,
    ) -> Result<Option<region::ChunkData>> {
        if !self.should_cache_full_region_chunks() {
            return self.with_region_io_lock(dimension, chunk_x, chunk_z, || {
                self.load_region_chunk_unlocked(dimension, chunk_x, chunk_z)
            });
        }

        let key = ChunkKey::new(dimension, chunk_x, chunk_z);
        if let Some(chunk) = self
            .region_chunk_cache
            .lock()
            .expect("world region chunk cache poisoned")
            .get(&key)
        {
            return Ok(Some(chunk));
        }

        let loaded = self.with_region_io_lock(dimension, chunk_x, chunk_z, || {
            self.load_region_chunk_unlocked(dimension, chunk_x, chunk_z)
        })?;
        if let Some(chunk) = loaded.as_ref() {
            self.remember_region_chunk(dimension, chunk_x, chunk_z, chunk.clone());
        }
        Ok(loaded)
    }

    fn read_region_chunk_for_network(
        &self,
        dimension: &str,
        chunk_x: i32,
        chunk_z: i32,
    ) -> Result<Option<region::ChunkData>> {
        if self.precompiled_chunk_packets_enabled() {
            return self.with_region_io_lock(dimension, chunk_x, chunk_z, || {
                self.load_region_chunk_unlocked(dimension, chunk_x, chunk_z)
            });
        }
        self.load_region_chunk(dimension, chunk_x, chunk_z)
    }

    fn should_cache_full_region_chunks(&self) -> bool {
        !self.precompiled_chunk_packets_enabled()
    }

    #[allow(dead_code)]
    fn write_region_chunk_unlocked(
        &self,
        dimension: &str,
        chunk_x: i32,
        chunk_z: i32,
        chunk: region::ChunkData,
    ) -> Result<()> {
        if self.read_only {
            anyhow::bail!("world is read-only");
        }

        log::trace!(
            "写入区块到存档: dimension={dimension}, chunk=({chunk_x}, {chunk_z}), root={}",
            self.save_path.display()
        );
        let region_dir = self.dimension_region_path(dimension);
        std::fs::create_dir_all(&region_dir)
            .with_context(|| format!("创建区域目录失败: {}", region_dir.display()))?;

        let region_path = region_dir.join(format!(
            "r.{}.{}.mca",
            floor_div(chunk_x, 32),
            floor_div(chunk_z, 32)
        ));
        let mut region = if region_path.exists() {
            region::AnvilRegion::from_file(&region_path)?
        } else {
            region::AnvilRegion::new(&region_path)
        };
        let cached = self
            .should_cache_full_region_chunks()
            .then(|| chunk.clone());
        region.write_chunk(chunk_x, chunk_z, chunk)?;
        region.save()?;
        if let Some(cached) = cached {
            self.remember_region_chunk(dimension, chunk_x, chunk_z, cached);
        }
        Ok(())
    }

    #[allow(dead_code)]
    pub fn write_region_chunk(
        &self,
        dimension: &str,
        chunk_x: i32,
        chunk_z: i32,
        chunk: region::ChunkData,
    ) -> Result<()> {
        if self.read_only {
            anyhow::bail!("world is read-only");
        }

        self.with_region_io_lock(dimension, chunk_x, chunk_z, || {
            self.write_region_chunk_unlocked(dimension, chunk_x, chunk_z, chunk)
        })
    }

    pub fn ensure_storage(&self, dimension: &str) -> Result<()> {
        if self.read_only {
            log::debug!("skipped world storage creation because world is read-only");
            return Ok(());
        }

        std::fs::create_dir_all(self.dimension_region_path(dimension))
            .with_context(|| format!("创建世界存档目录失败: {}", self.save_path.display()))
    }

    pub fn ensure_configured_storage(
        &self,
        world: &qexed_config::app::qexed::server::World,
    ) -> Result<()> {
        for dimension in world.configured_dimension_names() {
            self.ensure_storage(&dimension)?;
        }
        self.ensure_storage(&world.default_play_dimension())?;
        for instance in &world.instances {
            let dimension = instance.dimension.trim();
            if !dimension.is_empty() {
                self.ensure_storage(dimension)?;
            }
        }
        Ok(())
    }

    pub fn place_block(
        &self,
        dimension: &str,
        position: qexed_packet::net_types::Position,
        block_state: i32,
    ) {
        if self.read_only {
            log::debug!(
                "ignored block placement because world is read-only: dimension={dimension}, position=({}, {}, {})",
                position.x,
                position.y,
                position.z
            );
            return;
        }

        let pending = PendingBlock {
            block_state,
            revision: self.next_block_write_revision(),
        };
        self.placed_blocks
            .lock()
            .expect("world block store poisoned")
            .insert(BlockKey::new(dimension, &position), pending);
        self.mark_chunk_dirty_for_block_change(dimension, &position);
        self.queue_block_persist(dimension.to_string(), position.clone(), pending);
        self.mark_placed_block_light_dampening(
            dimension,
            &position,
            if block_state == AIR_BLOCK_STATE_ID {
                0
            } else {
                15
            },
        );
    }

    pub fn place_blocks(
        &self,
        dimension: &str,
        blocks: impl IntoIterator<Item = (qexed_packet::net_types::Position, i32)>,
    ) -> Result<Vec<BlockUpdate>> {
        if self.read_only {
            log::debug!("ignored bulk block placement because world is read-only");
            return Ok(Vec::new());
        }

        let blocks = blocks.into_iter().collect::<Vec<_>>();
        if blocks.is_empty() {
            return Ok(Vec::new());
        }

        let updates = blocks
            .iter()
            .map(|(position, block_state)| BlockUpdate {
                location: position.clone(),
                block_state: VarInt(*block_state),
            })
            .collect::<Vec<_>>();

        let mut expected = Vec::with_capacity(blocks.len());
        {
            let mut placed_blocks = self
                .placed_blocks
                .lock()
                .expect("world block store poisoned");
            for (position, block_state) in &blocks {
                let pending = PendingBlock {
                    block_state: *block_state,
                    revision: self.next_block_write_revision(),
                };
                placed_blocks.insert(BlockKey::new(dimension, position), pending);
                expected.push((position.clone(), pending));
                self.mark_chunk_dirty_for_block_change(dimension, position);
            }
        }

        self.queue_blocks_persist(dimension.to_string(), blocks.clone(), expected);

        for (position, block_state) in &blocks {
            self.mark_placed_block_light_dampening(
                dimension,
                position,
                if *block_state == AIR_BLOCK_STATE_ID {
                    0
                } else {
                    15
                },
            );
        }

        Ok(updates)
    }

    fn queue_block_persist(
        &self,
        dimension: String,
        position: qexed_packet::net_types::Position,
        pending: PendingBlock,
    ) {
        let world = self.clone();
        self.block_write_queue.spawn(move || {
            if !world.placed_block_is_current(&dimension, &position, pending) {
                return;
            }
            match world.persist_block_change(&dimension, &position, pending.block_state) {
                Ok(()) => {
                    world.remove_placed_block_if_current(&dimension, &position, pending);
                    world.clear_dirty_chunk_if_clean(&dimension, &position);
                }
                Err(err) => {
                    log::warn!(
                        "failed to persist block change, keeping in memory overlay: dimension={dimension}, position=({}, {}, {}), error={err:#}",
                        position.x,
                        position.y,
                        position.z
                    );
                }
            }
        });
    }

    fn queue_blocks_persist(
        &self,
        dimension: String,
        blocks: Vec<(qexed_packet::net_types::Position, i32)>,
        expected: Vec<(qexed_packet::net_types::Position, PendingBlock)>,
    ) {
        let world = self.clone();
        self.block_write_queue.spawn(move || {
            let current = blocks
                .into_iter()
                .zip(expected)
                .filter(|((position, _), (_, pending))| {
                    world.placed_block_is_current(&dimension, position, *pending)
                })
                .collect::<Vec<_>>();
            if current.is_empty() {
                return;
            }

            let blocks = current
                .iter()
                .map(|((position, block_state), _)| (position.clone(), *block_state))
                .collect::<Vec<_>>();
            let expected = current
                .into_iter()
                .map(|(_, expected)| expected)
                .collect::<Vec<_>>();

            match world.persist_block_changes(&dimension, &blocks) {
                Ok(()) => {
                    world.remove_placed_blocks_if_current(&dimension, &expected);
                    for (position, _) in &expected {
                        world.clear_dirty_chunk_if_clean(&dimension, position);
                    }
                }
                Err(err) => {
                    log::warn!(
                        "failed to persist bulk block changes, keeping in memory overlay: dimension={dimension}, count={}, error={err:#}",
                        blocks.len()
                    );
                }
            }
        });
    }

    fn placed_block_is_current(
        &self,
        dimension: &str,
        position: &qexed_packet::net_types::Position,
        expected: PendingBlock,
    ) -> bool {
        let key = BlockKey::new(dimension, position);
        self.placed_blocks
            .lock()
            .expect("world block store poisoned")
            .get(&key)
            .copied()
            == Some(expected)
    }

    fn chunk_has_pending_block_overlay(&self, chunk_key: &ChunkKey) -> bool {
        self.placed_blocks
            .lock()
            .expect("world block store poisoned")
            .keys()
            .any(|key| {
                key.dimension == chunk_key.dimension
                    && key.x.div_euclid(16) == chunk_key.x
                    && key.z.div_euclid(16) == chunk_key.z
            })
    }

    fn clear_dirty_chunk_if_clean(
        &self,
        dimension: &str,
        position: &qexed_packet::net_types::Position,
    ) {
        let chunk_key = ChunkKey::new(
            dimension,
            position.x.div_euclid(16),
            position.z.div_euclid(16),
        );
        if self.chunk_has_pending_block_overlay(&chunk_key) {
            return;
        }
        self.dirty_chunk_sections
            .lock()
            .expect("world dirty chunk section store poisoned")
            .remove(&chunk_key);
    }

    fn remove_placed_block_if_current(
        &self,
        dimension: &str,
        position: &qexed_packet::net_types::Position,
        expected: PendingBlock,
    ) {
        let key = BlockKey::new(dimension, position);
        let mut placed_blocks = self
            .placed_blocks
            .lock()
            .expect("world block store poisoned");
        if placed_blocks.get(&key).copied() == Some(expected) {
            placed_blocks.remove(&key);
        }
    }

    fn remove_placed_blocks_if_current(
        &self,
        dimension: &str,
        blocks: &[(qexed_packet::net_types::Position, PendingBlock)],
    ) {
        let mut placed_blocks = self
            .placed_blocks
            .lock()
            .expect("world block store poisoned");
        for (position, expected) in blocks {
            let key = BlockKey::new(dimension, position);
            if placed_blocks.get(&key).copied() == Some(*expected) {
                placed_blocks.remove(&key);
            }
        }
    }

    fn next_block_write_revision(&self) -> u64 {
        self.block_write_revision.fetch_add(1, Ordering::AcqRel) + 1
    }

    pub fn block_state_at(
        &self,
        dimension: &str,
        position: &qexed_packet::net_types::Position,
    ) -> Option<i32> {
        if let Some(block_state) = self
            .placed_blocks
            .lock()
            .expect("world block store poisoned")
            .get(&BlockKey::new(dimension, position))
            .map(|pending| pending.block_state)
        {
            return Some(block_state);
        }

        let block_key = BlockKey::new(dimension, position);
        if let Some(block_state) = self
            .block_state_cache
            .lock()
            .expect("world block state cache poisoned")
            .get(&block_key)
        {
            return Some(block_state);
        }

        let chunk_x = position.x.div_euclid(16);
        let chunk_z = position.z.div_euclid(16);
        match self.load_region_chunk(dimension, chunk_x, chunk_z) {
            Ok(Some(chunk)) => match chunk_nbt::block_state_at_from_region(&chunk, position) {
                Ok(block_state) => {
                    if let Some(block_state) = block_state {
                        self.remember_block_state(block_key, block_state);
                        return Some(block_state);
                    }
                }
                Err(err) => {
                    log::warn!(
                        "failed to read saved block state: dimension={dimension}, position=({}, {}, {}), error={err:#}",
                        position.x,
                        position.y,
                        position.z
                    );
                }
            },
            Ok(None) => {}
            Err(err) => {
                log::warn!(
                    "failed to load saved chunk for block state: dimension={dimension}, chunk=({chunk_x}, {chunk_z}), error={err:#}"
                );
            }
        }

        let block_state = self.generator.block_state_at(dimension, position);
        if let Some(block_state) = block_state {
            self.remember_block_state(block_key, block_state);
        }
        block_state
    }

    pub fn placed_block_updates(
        &self,
        dimension: &str,
        chunk_x: i32,
        chunk_z: i32,
    ) -> Vec<BlockUpdate> {
        self.placed_blocks
            .lock()
            .expect("world block store poisoned")
            .iter()
            .filter_map(|(key, block_state)| {
                (key.dimension == dimension
                    && key.x.div_euclid(16) == chunk_x
                    && key.z.div_euclid(16) == chunk_z)
                    .then(|| BlockUpdate {
                        location: qexed_packet::net_types::Position {
                            x: key.x,
                            y: key.y,
                            z: key.z,
                        },
                        block_state: VarInt(block_state.block_state),
                    })
            })
            .collect()
    }

    fn remember_region_chunk(
        &self,
        dimension: &str,
        chunk_x: i32,
        chunk_z: i32,
        chunk: region::ChunkData,
    ) {
        self.region_chunk_cache
            .lock()
            .expect("world region chunk cache poisoned")
            .insert(ChunkKey::new(dimension, chunk_x, chunk_z), chunk);
    }

    fn remember_block_state(&self, key: BlockKey, block_state: i32) {
        if !self.precompiled_chunk_packets_enabled()
            || self.precompiled_chunks.block_state_cache_limit == 0
        {
            return;
        }
        self.block_state_cache
            .lock()
            .expect("world block state cache poisoned")
            .insert(
                key,
                block_state,
                self.precompiled_chunks.block_state_cache_limit,
            );
    }

    fn mark_chunk_dirty_for_block_change(
        &self,
        dimension: &str,
        position: &qexed_packet::net_types::Position,
    ) {
        if !self.precompiled_chunk_packets_enabled() {
            return;
        }
        self.mark_precompiled_chunk_dirty(dimension, position);
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn mark_precompiled_chunk_dirty(
        &self,
        dimension: &str,
        position: &qexed_packet::net_types::Position,
    ) {
        let chunk_key = ChunkKey::new(
            dimension,
            position.x.div_euclid(16),
            position.z.div_euclid(16),
        );
        self.precompiled_chunk_cache
            .lock()
            .expect("world precompiled chunk cache poisoned")
            .invalidate(&chunk_key);
        self.block_state_cache
            .lock()
            .expect("world block state cache poisoned")
            .invalidate_chunk(&chunk_key);
        if (WORLD_MIN_Y..=WORLD_MAX_Y).contains(&position.y) {
            self.dirty_chunk_sections
                .lock()
                .expect("world dirty chunk section store poisoned")
                .entry(chunk_key)
                .or_default()
                .insert(position.y.div_euclid(16));
        }
    }

    fn dimension_region_path(&self, dimension: &str) -> std::path::PathBuf {
        if let Some(instance) = self.instances.get(dimension) {
            return configured_world_region_path(&instance.path);
        }
        if let Some(world) = self.worlds.get(dimension) {
            return configured_world_region_path(&world.path);
        }
        vanilla_dimension_region_path(&self.save_path, dimension)
    }
}

fn configured_world_region_path(root: &std::path::Path) -> std::path::PathBuf {
    root.join("region")
}

fn vanilla_dimension_region_path(root: &std::path::Path, dimension: &str) -> std::path::PathBuf {
    match dimension {
        "minecraft:the_nether" => root.join("DIM-1").join("region"),
        "minecraft:the_end" => root.join("DIM1").join("region"),
        _ => root.join("region"),
    }
}

fn region_file_name(chunk_x: i32, chunk_z: i32) -> String {
    format!(
        "r.{}.{}.mca",
        floor_div(chunk_x, 32),
        floor_div(chunk_z, 32)
    )
}

#[derive(Clone, Debug)]
struct WorldStorage {
    dimension: String,
    path: std::path::PathBuf,
}

impl WorldStorage {
    fn from_config(config: &qexed_config::app::qexed::server::WorldStorage) -> Option<Self> {
        let dimension = config.dimension.trim();
        if dimension.is_empty() {
            return None;
        }
        let path = config.path.trim();
        if path.is_empty() {
            return None;
        }
        Some(Self {
            dimension: dimension.to_string(),
            path: std::path::PathBuf::from(path),
        })
    }
}

#[derive(Clone, Debug)]
struct WorldInstanceStorage {
    dimension: String,
    path: std::path::PathBuf,
    source_dimension: String,
    source_path: std::path::PathBuf,
    copy_on_write: bool,
}

impl WorldInstanceStorage {
    fn from_config(
        default_save_path: &std::path::Path,
        config: &qexed_config::app::qexed::server::WorldInstance,
    ) -> Option<Self> {
        let dimension = config.dimension.trim();
        if dimension.is_empty() {
            return None;
        }
        let path = if config.path.trim().is_empty() {
            default_save_path.join(sanitize_dimension_path(dimension))
        } else {
            std::path::PathBuf::from(config.path.trim())
        };
        Some(Self {
            dimension: dimension.to_string(),
            path,
            source_dimension: config.source_dimension.trim().to_string(),
            source_path: default_save_path.to_path_buf(),
            copy_on_write: config.copy_on_write,
        })
    }

    fn source_region_path(&self, world: &WorldManager) -> std::path::PathBuf {
        if let Some(source_world) = world.worlds.get(&self.source_dimension) {
            return configured_world_region_path(&source_world.path);
        }
        let source_root = if self.source_path.as_os_str().is_empty() {
            world.save_path.as_path()
        } else {
            &self.source_path
        };
        vanilla_dimension_region_path(source_root, &self.source_dimension)
    }
}

fn sanitize_dimension_path(dimension: &str) -> String {
    dimension
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

impl WorldManager {
    fn persist_block_change(
        &self,
        dimension: &str,
        position: &qexed_packet::net_types::Position,
        block_state: i32,
    ) -> Result<()> {
        let chunk_x = position.x.div_euclid(16);
        let chunk_z = position.z.div_euclid(16);
        let fallback_block_state = self.generator.block_state_at(dimension, position);
        let cached_or_generated =
            if let Some(chunk) = self.load_region_chunk(dimension, chunk_x, chunk_z)? {
                Some(chunk)
            } else {
                self.generator.region_chunk(dimension, chunk_x, chunk_z)?
            };

        self.with_region_io_lock(dimension, chunk_x, chunk_z, || {
            let existing = self.load_region_chunk_unlocked(dimension, chunk_x, chunk_z)?;
            let chunk = chunk_nbt::set_block_state_in_region(
                chunk_x,
                chunk_z,
                existing.as_ref().or(cached_or_generated.as_ref()),
                position,
                block_state,
                fallback_block_state,
            )?;
            self.write_region_chunk_unlocked(dimension, chunk_x, chunk_z, chunk)
        })
    }

    fn persist_block_changes(
        &self,
        dimension: &str,
        blocks: &[(qexed_packet::net_types::Position, i32)],
    ) -> Result<()> {
        let mut by_chunk: std::collections::BTreeMap<
            (i32, i32),
            Vec<(qexed_packet::net_types::Position, i32, Option<i32>)>,
        > = std::collections::BTreeMap::new();
        for (position, block_state) in blocks {
            by_chunk
                .entry((position.x.div_euclid(16), position.z.div_euclid(16)))
                .or_default()
                .push((
                    position.clone(),
                    *block_state,
                    self.generator.block_state_at(dimension, position),
                ));
        }

        for ((chunk_x, chunk_z), chunk_blocks) in by_chunk {
            let cached_or_generated =
                if let Some(chunk) = self.load_region_chunk(dimension, chunk_x, chunk_z)? {
                    Some(chunk)
                } else {
                    self.generator.region_chunk(dimension, chunk_x, chunk_z)?
                };
            self.with_region_io_lock(dimension, chunk_x, chunk_z, || {
                let existing = self.load_region_chunk_unlocked(dimension, chunk_x, chunk_z)?;
                let chunk = chunk_nbt::set_block_states_in_region(
                    chunk_x,
                    chunk_z,
                    existing.as_ref().or(cached_or_generated.as_ref()),
                    &chunk_blocks,
                )?;
                self.write_region_chunk_unlocked(dimension, chunk_x, chunk_z, chunk)
            })?;
        }

        Ok(())
    }

    fn with_region_io_lock<T>(
        &self,
        dimension: &str,
        chunk_x: i32,
        chunk_z: i32,
        action: impl FnOnce() -> Result<T>,
    ) -> Result<T> {
        let key = RegionKey::from_chunk(dimension, chunk_x, chunk_z);
        let lock = {
            let mut locks = self
                .region_locks
                .lock()
                .expect("world region locks poisoned");
            locks
                .entry(key)
                .or_insert_with(|| Arc::new(Mutex::new(())))
                .clone()
        };
        let _guard = lock.lock().expect("world region lock poisoned");
        action()
    }

    pub(crate) fn remember_chunk_light_dampening(
        &self,
        dimension: &str,
        chunk_x: i32,
        chunk_z: i32,
        dampening: Vec<u8>,
        cache_epoch: Option<u64>,
    ) {
        let mut chunks = self
            .chunk_light_dampening
            .lock()
            .expect("world light cache poisoned");
        if cache_epoch.is_some_and(|epoch| !self.cache_epoch_is_active(epoch)) {
            return;
        }
        chunks.insert(ChunkKey::new(dimension, chunk_x, chunk_z), dampening);
    }

    fn mark_placed_block_light_dampening(
        &self,
        dimension: &str,
        position: &qexed_packet::net_types::Position,
        dampening: u8,
    ) {
        if !(WORLD_MIN_Y..=WORLD_MAX_Y).contains(&position.y) {
            return;
        }

        let chunk_x = position.x.div_euclid(16);
        let chunk_z = position.z.div_euclid(16);
        let local_x = position.x.rem_euclid(16) as usize;
        let local_z = position.z.rem_euclid(16) as usize;
        let mut chunks = self
            .chunk_light_dampening
            .lock()
            .expect("world light cache poisoned");
        let light_dampening = chunks
            .entry(ChunkKey::new(dimension, chunk_x, chunk_z))
            .or_insert_with(|| vec![0; CHUNK_DAMPENING_LEN]);
        light_dampening[block_light_dampening_index(local_x, position.y, local_z)] =
            dampening.min(15);
    }

    fn chunk_light_for_update(&self, dimension: &str, chunk_x: i32, chunk_z: i32) -> Light {
        self.chunk_light_for_payload(dimension, chunk_x, chunk_z, None)
    }

    fn chunk_light_for_payload(
        &self,
        dimension: &str,
        chunk_x: i32,
        chunk_z: i32,
        cache_epoch: Option<u64>,
    ) -> Light {
        match self.light_mode {
            WorldLightMode::Static | WorldLightMode::Fixed(_) => self.chunk_light(),
            WorldLightMode::Dynamic => {
                self.calculated_chunk_light(dimension, chunk_x, chunk_z, cache_epoch)
            }
        }
    }

    fn calculated_chunk_light(
        &self,
        dimension: &str,
        chunk_x: i32,
        chunk_z: i32,
        cache_epoch: Option<u64>,
    ) -> Light {
        if let WorldLightMode::Fixed(_) = self.light_mode {
            return self.chunk_light();
        }

        match self.light_algorithm {
            WorldLightAlgorithm::Fast => {
                let neighbourhood =
                    self.chunk_light_neighbourhood(dimension, chunk_x, chunk_z, cache_epoch);
                if let Some(light) = self
                    .light_gpu
                    .as_ref()
                    .and_then(|engine| engine.fast_sky_light(&neighbourhood).ok())
                {
                    return light_from_sky_values(&light);
                }
                sky_light_from_neighbourhood(&neighbourhood, WorldLightAlgorithm::Fast)
            }
            WorldLightAlgorithm::RayTrace => {
                let neighbourhood =
                    self.chunk_light_neighbourhood(dimension, chunk_x, chunk_z, cache_epoch);
                sky_light_from_neighbourhood(&neighbourhood, WorldLightAlgorithm::RayTrace)
            }
        }
    }

    fn chunk_light_neighbourhood(
        &self,
        dimension: &str,
        center_x: i32,
        center_z: i32,
        cache_epoch: Option<u64>,
    ) -> LightDampeningNeighborhood {
        let mut neighbourhood = LightDampeningNeighborhood::default();
        for offset_z in -1..=1 {
            for offset_x in -1..=1 {
                let chunk_x = center_x + offset_x;
                let chunk_z = center_z + offset_z;
                if let Some(dampening) = self.cached_or_load_chunk_light_dampening(
                    dimension,
                    chunk_x,
                    chunk_z,
                    cache_epoch,
                ) {
                    neighbourhood.set(offset_x, offset_z, dampening);
                }
            }
        }
        neighbourhood
    }

    fn cached_or_load_chunk_light_dampening(
        &self,
        dimension: &str,
        chunk_x: i32,
        chunk_z: i32,
        cache_epoch: Option<u64>,
    ) -> Option<Vec<u8>> {
        let key = ChunkKey::new(dimension, chunk_x, chunk_z);
        if let Some(dampening) = self
            .chunk_light_dampening
            .lock()
            .expect("world light cache poisoned")
            .get(&key)
            .cloned()
        {
            return Some(dampening);
        }

        let chunk = match self.load_region_chunk(dimension, chunk_x, chunk_z) {
            Ok(Some(chunk)) => chunk,
            Ok(None) => {
                return match self.generator.light_dampening(
                    dimension,
                    chunk_x,
                    chunk_z,
                    self.light_algorithm,
                ) {
                    Ok(dampening) => {
                        self.remember_chunk_light_dampening(
                            dimension,
                            chunk_x,
                            chunk_z,
                            dampening.clone(),
                            cache_epoch,
                        );
                        Some(dampening)
                    }
                    Err(err) => {
                        log::warn!(
                            "failed to generate neighbour chunk light data: dimension={dimension}, chunk=({chunk_x}, {chunk_z}), error={err:#}"
                        );
                        None
                    }
                };
            }
            Err(err) => {
                log::warn!(
                    "failed to load neighbour chunk light data: dimension={dimension}, chunk=({chunk_x}, {chunk_z}), error={err:#}"
                );
                return None;
            }
        };

        match chunk_nbt::light_dampening_from_region(&chunk) {
            Ok(dampening) => {
                self.remember_chunk_light_dampening(
                    dimension,
                    chunk_x,
                    chunk_z,
                    dampening.clone(),
                    cache_epoch,
                );
                Some(dampening)
            }
            Err(err) => {
                log::warn!(
                    "failed to parse neighbour chunk light data: dimension={dimension}, chunk=({chunk_x}, {chunk_z}), error={err:#}"
                );
                None
            }
        }
    }

    fn cache_epoch_is_active(&self, cache_epoch: u64) -> bool {
        self.cache_epoch.load(Ordering::Acquire) == cache_epoch
            && self.active_sessions.load(Ordering::Acquire) > 0
    }

    fn end_session(&self) {
        if self.active_sessions.fetch_sub(1, Ordering::AcqRel) != 1 {
            return;
        }

        self.cache_epoch.fetch_add(1, Ordering::AcqRel);
        let cleared = if self.precompiled_chunk_packets_enabled() {
            0
        } else {
            let mut chunks = self
                .chunk_light_dampening
                .lock()
                .expect("world light cache poisoned");
            let cleared = chunks.len();
            *chunks = std::collections::HashMap::new();
            cleared
        };
        let region_cleared = self
            .region_chunk_cache
            .lock()
            .expect("world region chunk cache poisoned")
            .clear();
        let precompiled_cleared = if self.precompiled_chunk_packets_enabled() {
            0
        } else {
            self.precompiled_chunk_cache
                .lock()
                .expect("world precompiled chunk cache poisoned")
                .clear()
        };
        let block_state_cleared = if self.precompiled_chunk_packets_enabled() {
            0
        } else {
            self.block_state_cache
                .lock()
                .expect("world block state cache poisoned")
                .clear()
        };
        if region_cleared > 0 || precompiled_cleared > 0 || block_state_cleared > 0 {
            log::debug!(
                "cleared world caches: region_chunks={region_cleared}, precompiled_packets={precompiled_cleared}, block_states={block_state_cleared}"
            );
        }
        if cleared > 0 {
            log::debug!("已清理世界区块光照缓存: chunks={cleared}");
        }
    }
}

#[derive(Debug)]
pub struct WorldSession {
    world: WorldManager,
    active: bool,
}

impl Drop for WorldSession {
    fn drop(&mut self) {
        if self.active {
            self.world.end_session();
            self.active = false;
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct BlockKey {
    dimension: String,
    x: i32,
    y: i32,
    z: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PendingBlock {
    block_state: i32,
    revision: u64,
}

impl BlockKey {
    fn new(dimension: &str, position: &qexed_packet::net_types::Position) -> Self {
        Self {
            dimension: dimension.to_string(),
            x: position.x,
            y: position.y,
            z: position.z,
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct ChunkKey {
    dimension: String,
    x: i32,
    z: i32,
}

impl ChunkKey {
    fn new(dimension: &str, x: i32, z: i32) -> Self {
        Self {
            dimension: dimension.to_string(),
            x,
            z,
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct RegionKey {
    dimension: String,
    x: i32,
    z: i32,
}

impl RegionKey {
    fn from_chunk(dimension: &str, chunk_x: i32, chunk_z: i32) -> Self {
        Self {
            dimension: dimension.to_string(),
            x: floor_div(chunk_x, 32),
            z: floor_div(chunk_z, 32),
        }
    }
}

fn floor_div(value: i32, divisor: i32) -> i32 {
    value.div_euclid(divisor)
}

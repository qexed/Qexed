use anyhow::{Context, Result};
use qexed_packet::net_types::VarInt;
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
    read_only: bool,
    light_mode: WorldLightMode,
    light_algorithm: WorldLightAlgorithm,
    light_gpu: Option<Arc<gpu_light::GpuLightEngine>>,
    generator: Arc<dyn generator::WorldChunkGenerator>,
    placed_blocks: Arc<Mutex<std::collections::HashMap<BlockKey, PendingBlock>>>,
    chunk_light_dampening: Arc<Mutex<std::collections::HashMap<ChunkKey, Vec<u8>>>>,
    region_locks: Arc<Mutex<std::collections::HashMap<RegionKey, Arc<Mutex<()>>>>>,
    block_write_queue: WorldWriteQueue,
    block_write_revision: Arc<AtomicU64>,
    active_sessions: Arc<AtomicUsize>,
    cache_epoch: Arc<AtomicU64>,
}

#[derive(Clone)]
struct WorldWriteQueue {
    sender: mpsc::Sender<WorldWriteTask>,
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
        let (sender, receiver) = mpsc::channel::<WorldWriteTask>();
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
        Self { sender }
    }

    fn spawn(&self, task: impl FnOnce() + Send + 'static) {
        if self
            .sender
            .send(WorldWriteTask::Run(Box::new(task)))
            .is_err()
        {
            log::warn!("failed to queue world write task because write queue stopped");
        }
    }

    #[cfg(test)]
    fn flush(&self) {
        let (sender, receiver) = mpsc::channel();
        if self.sender.send(WorldWriteTask::Flush(sender)).is_ok() {
            let _ = receiver.recv();
        }
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
            read_only,
            light_mode,
            light_algorithm,
            light_gpu,
            generator,
            placed_blocks: Default::default(),
            chunk_light_dampening: Default::default(),
            region_locks: Default::default(),
            block_write_queue: WorldWriteQueue::new(),
            block_write_revision: Default::default(),
            active_sessions: Default::default(),
            cache_epoch: Default::default(),
        }
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

    #[cfg(test)]
    pub(crate) fn cached_light_chunk_count(&self) -> usize {
        self.chunk_light_dampening
            .lock()
            .expect("world light cache poisoned")
            .len()
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

    pub fn network_chunk_for_session(
        &self,
        dimension: &str,
        chunk_x: i32,
        chunk_z: i32,
        cache_epoch: u64,
    ) -> Result<MapChunk> {
        if !self.cache_epoch_is_active(cache_epoch) {
            anyhow::bail!("chunk load cancelled because world session expired");
        }
        self.network_chunk_inner(dimension, chunk_x, chunk_z, Some(cache_epoch))
    }

    fn network_chunk_inner(
        &self,
        dimension: &str,
        chunk_x: i32,
        chunk_z: i32,
        cache_epoch: Option<u64>,
    ) -> Result<MapChunk> {
        if let Some(chunk) = self.load_region_chunk(dimension, chunk_x, chunk_z)? {
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
                    return Ok(packet);
                }
                Err(err) => {
                    log::warn!(
                        "failed to convert saved chunk, falling back to empty chunk: dimension={dimension}, chunk=({chunk_x}, {chunk_z}), error={err:#}"
                    );
                }
            }
        }

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
        let region_path = self.dimension_region_path(dimension).join(format!(
            "r.{}.{}.mca",
            floor_div(chunk_x, 32),
            floor_div(chunk_z, 32)
        ));
        if !region_path.exists() {
            return Ok(None);
        }

        let region = region::AnvilRegion::from_file(&region_path)
            .with_context(|| format!("读取区域文件失败: {}", region_path.display()))?;
        region
            .read_chunk(chunk_x, chunk_z)
            .with_context(|| format!("读取区块失败: {chunk_x}, {chunk_z}"))
    }

    pub fn load_region_chunk(
        &self,
        dimension: &str,
        chunk_x: i32,
        chunk_z: i32,
    ) -> Result<Option<region::ChunkData>> {
        self.with_region_io_lock(dimension, chunk_x, chunk_z, || {
            self.load_region_chunk_unlocked(dimension, chunk_x, chunk_z)
        })
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
        region.write_chunk(chunk_x, chunk_z, chunk)?;
        region.save()?;
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

        let chunk_x = position.x.div_euclid(16);
        let chunk_z = position.z.div_euclid(16);
        match self.load_region_chunk(dimension, chunk_x, chunk_z) {
            Ok(Some(chunk)) => match chunk_nbt::block_state_at_from_region(&chunk, position) {
                Ok(block_state) => {
                    if block_state.is_some() {
                        return block_state;
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

        self.generator.block_state_at(dimension, position)
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

    fn dimension_region_path(&self, dimension: &str) -> std::path::PathBuf {
        match dimension {
            "minecraft:the_nether" => self.save_path.join("DIM-1").join("region"),
            "minecraft:the_end" => self.save_path.join("DIM1").join("region"),
            _ => self.save_path.join("region"),
        }
    }

    fn persist_block_change(
        &self,
        dimension: &str,
        position: &qexed_packet::net_types::Position,
        block_state: i32,
    ) -> Result<()> {
        let chunk_x = position.x.div_euclid(16);
        let chunk_z = position.z.div_euclid(16);
        let fallback_block_state = self.generator.block_state_at(dimension, position);
        let generated = if self
            .load_region_chunk(dimension, chunk_x, chunk_z)?
            .is_none()
        {
            self.generator.region_chunk(dimension, chunk_x, chunk_z)?
        } else {
            None
        };

        self.with_region_io_lock(dimension, chunk_x, chunk_z, || {
            let existing = self.load_region_chunk_unlocked(dimension, chunk_x, chunk_z)?;
            let chunk = chunk_nbt::set_block_state_in_region(
                chunk_x,
                chunk_z,
                existing.as_ref().or(generated.as_ref()),
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
            let generated = if self
                .load_region_chunk(dimension, chunk_x, chunk_z)?
                .is_none()
            {
                self.generator.region_chunk(dimension, chunk_x, chunk_z)?
            } else {
                None
            };
            self.with_region_io_lock(dimension, chunk_x, chunk_z, || {
                let existing = self.load_region_chunk_unlocked(dimension, chunk_x, chunk_z)?;
                let chunk = chunk_nbt::set_block_states_in_region(
                    chunk_x,
                    chunk_z,
                    existing.as_ref().or(generated.as_ref()),
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
        match self.light_mode {
            WorldLightMode::Static | WorldLightMode::Fixed(_) => self.chunk_light(),
            WorldLightMode::Dynamic => {
                self.calculated_chunk_light(dimension, chunk_x, chunk_z, None)
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
        let mut chunks = self
            .chunk_light_dampening
            .lock()
            .expect("world light cache poisoned");
        let cleared = chunks.len();
        *chunks = std::collections::HashMap::new();
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

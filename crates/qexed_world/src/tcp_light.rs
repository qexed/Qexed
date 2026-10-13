//! TCP 区块光照覆写：给 [crate::tcp_source::TcpChunkSource] 的回包补真实光照。
//!
//! 背景：chunk-summon 服务下发的 `LevelChunkWithLight` 携带的是 Java 侧
//! `LevelLightEngine.EMPTY`（掩码全零、无数组），qexed 客户端拿到的区块
//! 光照恒为空——qexed_light 从未参与 TCP 路径。本模块把回包的 section
//! 调色板解码为光学体（[crate::blocks::block_props] 的 26.1.2 光学表），
//! 交 [qexed_light::LightEngine] 算天光+方块光，合成协议 Light 后覆写。
//!
//! 邻域语义：单 chunk 独立计算会在区块边界留光照接缝（山洞尤其明显）——
//! 光照引擎拿不到相邻 chunk 的不透明度/光照，边界两侧各算各的。[TcpLightCache]
//! 提供跨 chunk 缓存 + 两阶段 API（plan → 补取邻居 → finish）：中心 chunk
//! 用 3×3 不透明度邻域 + 已算邻居光照播种重算，边界光照与 qexed_light
//! 引擎「缺邻居不播种、不臆造开天空」的邻域语义对齐。
//!
//! 协议细节（26.1.2，按 qexed_chunk_by_tcp 金标样本 chunk_0_0.bin 实证）：
//! - section：双 i16（非空气/液体计数）+ 方块调色板容器 + 生物群系容器；
//! - 调色板容器（1.21.5 起）：bits=0 → 单值 VarInt id（无数据数组）；
//!   bits>0 → VarInt palLen + palLen×VarInt id + 打包 longs，
//!   **无长度前缀**，longs = ceil(entry_count / (64/bits))（整数除），
//!   每 long 独立打包（低位在前、尾部 padding、不跨 long 拼接）；
//! - 生物群系容器同构，仅 64 entries，光照不需要，解码时跳过。

use std::collections::{HashMap, VecDeque};
use std::sync::OnceLock;

use bytes::BytesMut;
use qexed_light::data::{
    BlockLightProps, ChunkLightData, ChunkOpacity, LightNeighborhood, OpacityNeighborhood,
};
use qexed_light::{DimensionGeometry, LightEngine, LightPayload, SkyLightAlgorithm};
use qexed_packet::net_types::VarInt;
use qexed_packet::{PacketCodec, PacketReader};
use qexed_protocol::to_client::play::map_chunk::{Light, MapChunk};

use crate::blocks::{block_props, BlockIndex};
use crate::chunk::{SECTION_COUNT, SECTION_HEIGHT};
use crate::error::{Result, WorldError};
use crate::light::payload_to_light;

/// 单个 section 的方块数：16×16×16。
const SECTION_BLOCKS: usize = 16 * 16 * 16;
/// 生物群系 section 的条目数：4×4×4。
const BIOME_ENTRIES: usize = 4 * 4 * 4;

/// 解码后的 section：调色板（全局 state id）+ 4096 个调色板索引 + 计数头。
struct DecodedSection {
    non_air: i16,
    fluid: i16,
    palette: Vec<i32>,
    entries: Vec<u16>,
}

/// 解码一个调色板容器，返回 (bits, palette, 打包数据)。
///
/// bits=0 时 data 为 None（单值容器，所有索引指向 palette[0]）。
fn read_palette_container(
    reader: &mut PacketReader<'_>,
    entry_count: usize,
    what: &str,
) -> Result<(u8, Vec<i32>, Option<Vec<u64>>)> {
    let mut bits = 0u8;
    bits.deserialize(reader)?;
    let bits = bits;
    if bits == 0 {
        // 单值 palette：一个 VarInt 注册表 id，无数据数组（1.21.5+）。
        let mut id = VarInt::default();
        id.deserialize(reader)?;
        return Ok((0, vec![id.0], None));
    }
    if bits > 15 {
        return Err(WorldError::Other(format!(
            "{what}: 调色板位宽 {bits} 越界（> 15）"
        )));
    }
    let mut palette_len = VarInt::default();
    palette_len.deserialize(reader)?;
    let palette_len = usize::try_from(palette_len.0)
        .map_err(|_| WorldError::Other(format!("{what}: 调色板长度 {} 为负", palette_len.0)))?;
    if palette_len == 0 {
        return Err(WorldError::Other(format!("{what}: 直接调色板为空")));
    }
    let mut palette = Vec::with_capacity(palette_len.min(entry_count));
    for _ in 0..palette_len {
        let mut id = VarInt::default();
        id.deserialize(reader)?;
        palette.push(id.0);
    }
    // 每 long 独立打包：entries/long = 64/bits（整数除，尾部位是 padding）。
    let per_long = 64 / bits as usize;
    let longs = entry_count.div_ceil(per_long);
    let mut data = Vec::with_capacity(longs);
    for _ in 0..longs {
        let mut word = 0i64;
        word.deserialize(reader)?;
        data.push(word as u64);
    }
    Ok((bits, palette, Some(data)))
}

/// 每 long 独立打包的索引解包：低位在前，long 尾部 padding 忽略。
fn unpack_indices(bits: u8, data: &[u64], entry_count: usize) -> Vec<u16> {
    let bits = bits as usize;
    let per_long = 64 / bits;
    let mask = (1u64 << bits) - 1;
    (0..entry_count)
        .map(|index| {
            let word = data[index / per_long];
            ((word >> ((index % per_long) * bits)) & mask) as u16
        })
        .collect()
}

/// 解码整个 section blob（主世界恒 24 个 section，无外层长度前缀——
/// 协议里 blob 前的 VarInt 由 Chunk.data 字段自身编码，解码时已消费）。
fn decode_section_blob(blob: &[u8]) -> Result<Vec<DecodedSection>> {
    if blob.len() < 6 {
        return Err(WorldError::Other(format!(
            "section blob 过短: {} 字节",
            blob.len()
        )));
    }
    let mut buf = BytesMut::from(blob);
    let mut reader = PacketReader::new(&mut buf);
    let mut sections = Vec::with_capacity(SECTION_COUNT);
    for _ in 0..SECTION_COUNT {
        let mut non_air = 0i16;
        non_air.deserialize(&mut reader)?;
        let mut fluid = 0i16;
        fluid.deserialize(&mut reader)?;
        let (bits, palette, data) = read_palette_container(&mut reader, SECTION_BLOCKS, "blocks")?;
        let entries = match &data {
            Some(d) => unpack_indices(bits, d, SECTION_BLOCKS),
            None => vec![0u16; SECTION_BLOCKS],
        };
        // 生物群系容器同构（64 entries），光照不需要，跳过。
        let _ = read_palette_container(&mut reader, BIOME_ENTRIES, "biomes")?;
        sections.push(DecodedSection {
            non_air,
            fluid,
            palette,
            entries,
        });
    }
    Ok(sections)
}

/// state id → 光学属性 反查表。调色板值是协议全局 state 序号，而
/// blocks.json 报告只给 name+properties → id 的正向映射；物化成下标直查的
/// Vec（26.1.2 约 2.7 万 state，内存 ~30KB），进程内构建一次（OnceLock）。
struct StatePropsTable {
    props_by_id: Vec<BlockLightProps>,
}

impl StatePropsTable {
    fn build() -> Result<Self> {
        let report = qexed_registry::load_blocks_report()
            .map_err(|e| WorldError::Other(format!("blocks.json 报告加载失败: {e}")))?;
        let entries = report
            .as_object()
            .ok_or_else(|| WorldError::Other("blocks.json 报告不是对象".into()))?;

        let mut max_id: i64 = 0;
        for block in entries.values() {
            let Some(states) = block.get("states").and_then(|v| v.as_array()) else {
                continue;
            };
            for state in states {
                if let Some(id) = state.get("id").and_then(|v| v.as_i64()) {
                    max_id = max_id.max(id);
                }
            }
        }
        let len = usize::try_from(max_id + 1)
            .map_err(|_| WorldError::Other(format!("blocks.json 最大 state id {max_id} 越界")))?;
        // 缺省 = 实心挡光（与 block_props 的兜底一致：未知 id 宁可挡光）。
        let mut props_by_id = vec![BlockLightProps::SOLID; len];

        for (name, block) in entries {
            let Some(states) = block.get("states").and_then(|v| v.as_array()) else {
                continue;
            };
            for state in states {
                let Some(id) = state.get("id").and_then(|v| v.as_i64()) else {
                    continue;
                };
                if id < 0 {
                    continue;
                }
                let id = id as usize;
                if id >= len {
                    continue;
                }
                let props: Vec<(String, String)> = state
                    .get("properties")
                    .and_then(|v| v.as_object())
                    .map(|map| {
                        map.iter()
                            .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                            .collect()
                    })
                    .unwrap_or_default();
                props_by_id[id] = resolve_props(name, &props);
            }
        }
        Ok(Self { props_by_id })
    }

    #[inline]
    fn get(&self, state_id: i32) -> BlockLightProps {
        self.props_by_id
            .get(state_id as usize)
            .copied()
            .unwrap_or(BlockLightProps::SOLID)
    }
}

/// 进程级缓存的反查表（blocks.json 只读一次）。
fn state_table() -> Result<&'static StatePropsTable> {
    static TABLE: OnceLock<std::result::Result<StatePropsTable, String>> = OnceLock::new();
    let table = TABLE.get_or_init(|| StatePropsTable::build().map_err(|e| e.to_string()));
    table
        .as_ref()
        .map_err(|e| WorldError::Other(e.clone()))
}

/// 带 state 属性的光学属性解析。
///
/// block_props 的光学表有两类键：纯方块名（`minecraft:water`）与
/// 「名字+状态」精确键（`minecraft:redstone_ore[lit=true]`）。先拼
/// name[k=v,...]（属性排序规则同 BlockIndex::state_key）精确命中；带属性
/// 键未命中时退回方块名级——受 optics 表的兜底 SOLID 影响无法区分「键未
/// 命中」与「方块本身实心」，因此约定：只有方块名级查出的结果**非实心**
/// 才采用（`wall_torch[waterlogged=false]` → 名字级 {0,14} ✓；
/// `redstone_ore[lit=false]` → 名字级 SOLID ✓ 恰为正确值）。
fn resolve_props(name: &str, properties: &[(String, String)]) -> BlockLightProps {
    if properties.is_empty() {
        return block_props(name);
    }
    let keyed = BlockIndex::state_key(name, properties);
    if keyed == name {
        return block_props(name);
    }
    match block_props(&keyed) {
        props if props != BlockLightProps::SOLID => props,
        _ => block_props(name),
    }
}

/// 调色板索引 → state id（越界即服务端编码异常，报错）。
fn state_id_at(section: &DecodedSection, index: usize) -> Result<i32> {
    let entry = section.entries[index] as usize;
    section.palette.get(entry).copied().ok_or_else(|| {
        WorldError::Other(format!(
            "section 调色板索引 {entry} 越界（palette len = {}）",
            section.palette.len()
        ))
    })
}

/// 3×3 邻域偏移（不含中心）。
const NEIGHBOR_OFFSETS: [(i32, i32); 8] = [
    (-1, -1),
    (0, -1),
    (1, -1),
    (-1, 0),
    (1, 0),
    (-1, 1),
    (0, 1),
    (1, 1),
];

/// 由解码后的 section 构建不透明度光学体。全空 chunk（空区块回退/纯空气
/// 生成）返回 None：光照恒为静态满级天光，无需引擎参与。
fn build_opacity(sections: &[DecodedSection]) -> Result<Option<ChunkOpacity>> {
    if sections.iter().all(|s| s.non_air == 0 && s.fluid == 0) {
        return Ok(None);
    }
    let table = state_table()?;
    let geometry = DimensionGeometry::overworld().geometry;
    let mut opacity = ChunkOpacity::empty(&geometry);
    for (section_index, section) in sections.iter().enumerate() {
        // 26.1+ 调色板索引序：y 高 12 位（每 y 层 256 格）、z 中 4 位、x 低 4 位。
        for i in 0..SECTION_BLOCKS {
            let props = table.get(state_id_at(section, i)?);
            if props.opacity == 0 && props.luminance == 0 {
                continue; // 空气/植物等透光无光：保持基线 0
            }
            let y = geometry.world_min_y
                + (section_index as i32) * SECTION_HEIGHT
                + (i >> 8) as i32;
            let z = ((i >> 4) & 0xf) as usize;
            let x = (i & 0xf) as usize;
            opacity
                .set(x, y, z, props)
                .expect("索引序展开的坐标必在 chunk 内");
        }
    }
    Ok(Some(opacity))
}

/// 缓存条目：光学体（邻居重算用）+ 光照（播种用）+ 协议 Light（命中回放）。
struct CacheEntry {
    opacity: ChunkOpacity,
    light: ChunkLightData,
    /// Some = 该 chunk 已作为中心完成邻域重算，协议 Light 可直接回放；
    /// None = 仅批量预取/播种用（单 chunk 光照不可直接下发）。
    packet_light: Option<Light>,
    /// false = plan 阶段占位（光照未算），不参与邻居播种。
    computed: bool,
}

/// plan_overwrite 产出的计划：邻居补取 + 播种后交 finish_overwrite 收尾。
pub struct LightPlan {
    center: (i32, i32),
    opacity: ChunkOpacity,
}

/// TCP 路径的跨 chunk 光照缓存 + 邻域重算驱动。
///
/// 单 chunk 独立算光照会在区块边界留接缝（山洞尤其明显）：中心 chunk 拿不到
/// 邻居的不透明度与光照，边界两侧各算各的。两阶段用法（上层负责异步补取）：
///
/// 1. plan_overwrite：解码入缓存，产出待邻居补取的 LightPlan；
/// 2. 上层对 missing_neighbors 逐个异步请求 MapChunk，经 plan_neighbor_packet
///    把邻居光学体/光照补进缓存；
/// 3. seed_neighbors + finish_overwrite：3×3 邻域播种重算，中心光照回填回包。
pub struct TcpLightCache {
    entries: HashMap<(i32, i32), CacheEntry>,
    /// LRU 驱逐序：front = 最旧。
    order: VecDeque<(i32, i32)>,
    capacity: usize,
}

impl Default for TcpLightCache {
    fn default() -> Self {
        Self::new()
    }
}

impl TcpLightCache {
    /// 默认容量：覆盖视野半径 ≤ 8 的跑图环 + 邻域扩圈（每条目约 0.3 MB）。
    pub fn new() -> Self {
        Self::with_capacity(512)
    }

    /// 指定容量（最小 27：中心 + 8 邻居必须同时存活）。
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            entries: HashMap::new(),
            order: VecDeque::new(),
            capacity: capacity.max(27),
        }
    }

    /// 缓存条目数（测试/诊断用）。
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// 缓存是否为空。
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// LRU 触碰：移到队尾。
    fn touch(&mut self, key: (i32, i32)) {
        if let Some(pos) = self.order.iter().position(|k| *k == key) {
            self.order.remove(pos);
            self.order.push_back(key);
        }
    }

    /// 入缓存；容量满则先驱逐最旧条目。
    fn insert(&mut self, key: (i32, i32), entry: CacheEntry) {
        if self.entries.len() >= self.capacity {
            if let Some(oldest) = self.order.pop_front() {
                self.entries.remove(&oldest);
            }
        }
        self.entries.insert(key, entry);
        self.order.push_back(key);
    }

    /// 已作为中心算过的协议 Light（同坐标命中免解码重算）。
    pub fn cached_light(&mut self, chunk_x: i32, chunk_z: i32) -> Option<Light> {
        let hit = self
            .entries
            .get(&(chunk_x, chunk_z))
            .filter(|entry| entry.computed)
            .and_then(|entry| entry.packet_light.clone())?;
        self.touch((chunk_x, chunk_z));
        Some(hit)
    }

    /// 取出该 chunk 已算好的协议 Light（增量流送复用中心 chunk 缓存）。
    ///
    /// 取走后条目不可再回放（packet_light 清空），光照/光学体仍可播种。
    pub fn take_packet_light(&mut self, chunk_x: i32, chunk_z: i32) -> Option<Light> {
        let entry = self.entries.get_mut(&(chunk_x, chunk_z))?;
        let light = entry.packet_light.take()?;
        entry.computed = false;
        self.touch((chunk_x, chunk_z));
        Some(light)
    }

    /// 阶段 1：服务端已带真实光照 → None（不动回包）；缓存命中 → 直接回填
    /// 并返回 None；否则解码入缓存，返回待邻居补取的 LightPlan。
    pub fn plan_overwrite(&mut self, chunk: &mut MapChunk) -> Result<Option<LightPlan>> {
        let _span = qexed_profiler::profile_span!("world:light_plan");
        if server_sent_light(chunk) {
            return Ok(None);
        }
        let center = (chunk.chunk_x, chunk.chunk_z);
        if let Some(light) = self.cached_light(center.0, center.1) {
            chunk.light = light;
            return Ok(None);
        }
        let sections = decode_section_blob(&chunk.data.data)?;
        let geometry = DimensionGeometry::overworld().geometry;
        match build_opacity(&sections)? {
            // 全空 chunk：光照恒为静态满级天光（与 empty_chunk_packet 字节
            // 形态一致），无需邻居；照常入缓存供邻 chunk 播种。
            None => {
                let mut light = ChunkLightData::empty(&geometry);
                light.sky_light_mut().fill(15);
                self.insert(
                    center,
                    CacheEntry {
                        opacity: ChunkOpacity::empty(&geometry),
                        light,
                        packet_light: Some(crate::chunk::static_light()),
                        computed: true,
                    },
                );
                chunk.light = crate::chunk::static_light();
                Ok(None)
            }
            Some(opacity) => {
                // 占位入缓存（播种层不读中心槽，空体安全；finish 时替换）。
                self.insert(
                    center,
                    CacheEntry {
                        opacity: opacity.clone(),
                        light: ChunkLightData::empty(&geometry),
                        packet_light: None,
                        computed: false,
                    },
                );
                Ok(Some(LightPlan { center, opacity }))
            }
        }
    }

    /// 该坐标是否已在缓存且 3×3 邻居全部就绪（预完成判定）。
    pub fn has_full_neighborhood(&self, center_x: i32, center_z: i32) -> bool {
        NEIGHBOR_OFFSETS.iter().all(|&(dx, dz)| {
            self.entries
                .get(&(center_x + dx, center_z + dz))
                .is_some_and(|entry| entry.computed)
        })
    }

    /// 中心 3×3 里缓存缺失的邻居坐标（供上层异步补取）。
    pub fn missing_neighbors(&self, center_x: i32, center_z: i32) -> Vec<(i32, i32)> {
        NEIGHBOR_OFFSETS
            .iter()
            .map(|(dx, dz)| (center_x + dx, center_z + dz))
            .filter(|key| !self.entries.contains_key(key))
            .collect()
    }

    /// 阶段 2：邻居回包解码，光学体 + 单 chunk 光照补进缓存。
    ///
    /// 已在缓存即跳过；解码失败由调用方记日志后放弃该邻居（边界接缝兜底
    /// 到引擎「缺邻居不播种」语义）。
    pub fn plan_neighbor_packet(&mut self, chunk: &MapChunk) -> Result<()> {
        let _span = qexed_profiler::profile_span!("world:light_neighbor");
        let key = (chunk.chunk_x, chunk.chunk_z);
        if self.entries.contains_key(&key) {
            self.touch(key);
            return Ok(());
        }
        let sections = decode_section_blob(&chunk.data.data)?;
        let geometry = DimensionGeometry::overworld().geometry;
        match build_opacity(&sections)? {
            None => {
                let mut light = ChunkLightData::empty(&geometry);
                light.sky_light_mut().fill(15);
                self.insert(
                    key,
                    CacheEntry {
                        opacity: ChunkOpacity::empty(&geometry),
                        light,
                        packet_light: None,
                        computed: true,
                    },
                );
            }
            Some(opacity) => {
                let neighborhood = OpacityNeighborhood::single(opacity.clone());
                let engine =
                    LightEngine::new(DimensionGeometry::overworld(), SkyLightAlgorithm::Fast);
                let sky = engine.compute_sky_light(&neighborhood);
                let block = engine.compute_block_light(&neighborhood, None);
                let mut light = sky;
                light.block_light_mut().copy_from_slice(block.block_light());
                self.insert(
                    key,
                    CacheEntry {
                        opacity,
                        light,
                        packet_light: None,
                        computed: true,
                    },
                );
            }
        }
        Ok(())
    }

    /// 阶段 3：3×3 邻域（不透明度 + 播种光照）重算中心光照，回填回包并把
    /// 中心条目升级为已计算（供后续邻 chunk 播种）。
    pub fn finish_overwrite(&mut self, chunk: &mut MapChunk, plan: LightPlan) {
        let _span = qexed_profiler::profile_span!("world:light_finish");
        let dimension = DimensionGeometry::overworld();
        let geometry = dimension.geometry;
        let mut opacity_neighbors = OpacityNeighborhood::single(plan.opacity.clone());
        let mut light_neighbors = LightNeighborhood::single(ChunkLightData::empty(&geometry));
        for (dx, dz) in NEIGHBOR_OFFSETS {
            if let Some(entry) = self.entries.get(&(plan.center.0 + dx, plan.center.1 + dz)) {
                if entry.computed {
                    opacity_neighbors.set(dx, dz, entry.opacity.clone());
                    light_neighbors.set(dx, dz, entry.light.clone());
                }
            }
        }
        let engine = LightEngine::new(dimension, SkyLightAlgorithm::Fast);
        let sky = engine.compute_sky_light_with_neighbors(&opacity_neighbors, &light_neighbors);
        let block = engine.compute_block_light(&opacity_neighbors, Some(&light_neighbors));
        let mut center_light = sky;
        center_light
            .block_light_mut()
            .copy_from_slice(block.block_light());
        let payload =
            LightPayload::from_raw(&geometry, center_light.sky_light(), center_light.block_light());
        chunk.light = payload_to_light(&payload);
        match self.entries.get_mut(&plan.center) {
            Some(entry) => {
                entry.light = center_light;
                entry.packet_light = Some(chunk.light.clone());
                entry.computed = true;
            }
            None => {
                // 极小容量下被挤出：重新入缓存（不影响本次回包）。
                self.insert(
                    plan.center,
                    CacheEntry {
                        opacity: plan.opacity,
                        light: center_light,
                        packet_light: Some(chunk.light.clone()),
                        computed: true,
                    },
                );
            }
        }
    }
}

fn server_sent_light(chunk: &MapChunk) -> bool {
    chunk.light.sky_light_mask.0.iter().any(|&w| w != 0)
        || chunk.light.block_light_mask.0.iter().any(|&w| w != 0)
}

/// 为 TCP 回包覆写真实光照（无邻居补取的兼容入口）。
///
/// 生产路径请改用 TcpLightCache 两阶段 API + 上层补取邻居消除边界接缝；
/// 此包装等价于旧单 chunk 语义：中心邻域 + 引擎内建回退。
///
/// - 服务端已带非空光照掩码时不动（未来 Java 侧接真实光照引擎后自动让位）；
/// - 解码/计算失败时警告并保留服务端形态（空光照可玩：只是黑，不阻断
///   登录流程）——地形字节永远是权威数据，光照只是增强。
pub fn overwrite_chunk_light(chunk: &mut MapChunk) {
    overwrite_chunk_light_with(&mut TcpLightCache::new(), chunk);
}

/// 同 overwrite_chunk_light，但用调用方提供的缓存（同坐标命中免重算）。
pub fn overwrite_chunk_light_with(cache: &mut TcpLightCache, chunk: &mut MapChunk) {
    match cache.plan_overwrite(chunk) {
        Ok(None) => {}
        Ok(Some(plan)) => {
            cache.finish_overwrite(chunk, plan);
        }
        Err(e) => log::warn!(
            "tcp 区块 ({},{}) 光照计算失败，保留服务端空光照: {e}",
            chunk.chunk_x,
            chunk.chunk_z
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// 单元测试锚定 workspace 的 mojang 缓存（同 vanilla_pipeline.rs 的方式；
    /// cargo test 的 CWD 是 crate 目录，默认相对路径解析会落空）。
    fn setup_mojang_cache() {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let mojang_cache = manifest_dir
            .parent()
            .and_then(|p| p.parent())
            .map(|root| root.join("run").join("cache").join("mojang"))
            .expect("workspace root");
        let _ = qexed_mojang_data::set_cache_dir(&mojang_cache);
    }

    #[test]
    fn unpack_indices_per_long_independent() {
        // bits=4：一个 long 装 16 个索引；0x5432 = [2,3,4,5,0,...]（低位在前）。
        let data = [0x0000_0000_0005_4321u64];
        let out = unpack_indices(4, &data, 16);
        assert_eq!(&out[..5], &[1, 2, 3, 4, 5]);
        assert!(out[5..].iter().all(|&v| v == 0));
    }

    #[test]
    fn palette_container_roundtrip_without_length_prefix() {
        // 手写编码：bits=4 + palette 2 项 + 256 long（无长度前缀，1.21.5+）。
        let mut buf = BytesMut::new();
        {
            let mut w = qexed_packet::PacketWriter::new(&mut buf);
            let mut four = 4u8;
            four.serialize(&mut w).unwrap();
            VarInt(2).serialize(&mut w).unwrap();
            let mut a = VarInt(7);
            a.serialize(&mut w).unwrap();
            let mut b = VarInt(9);
            b.serialize(&mut w).unwrap();
            for i in 0..256 {
                let mut word = if i % 2 == 0 {
                    0x1111_1111_1111_1111i64
                } else {
                    0i64
                };
                word.serialize(&mut w).unwrap();
            }
        }
        let mut reader = PacketReader::new(&mut buf);
        let (bits, palette, data) =
            read_palette_container(&mut reader, SECTION_BLOCKS, "blocks").unwrap();
        assert_eq!(bits, 4);
        assert_eq!(palette, vec![7, 9]);
        let entries = unpack_indices(bits, data.as_ref().unwrap(), SECTION_BLOCKS);
        // 0x1111.. 每 4 位都是 1 → 偶数 long 全部索引 = 1；奇数 long 全 0。
        assert_eq!(entries[0], 1);
        assert_eq!(entries[15], 1);
        assert_eq!(entries[16], 0);
        assert_eq!(entries[255], 0);
    }

    #[test]
    fn empty_sections_yield_static_light() {
        let blob = crate::chunk::empty_section_bytes();
        let sections = decode_section_blob(&blob).unwrap();
        assert_eq!(sections.len(), SECTION_COUNT);
        // 全空 section 光学体为 None → plan 路径直接落静态满级天光。
        assert!(build_opacity(&sections).unwrap().is_none());
        let light = crate::chunk::static_light();
        assert!(light.sky_light_mask.0.iter().any(|&w| w != 0));
    }

    #[test]
    fn overwrite_skips_when_server_sent_real_light() {
        let mut chunk = crate::chunk::empty_chunk_packet(1, 2);
        let real = crate::chunk::static_light();
        chunk.light = real.clone();
        overwrite_chunk_light(&mut chunk);
        assert_eq!(chunk.light, real);
    }

    #[test]
    fn resolve_props_state_key_precedence() {
        // 带状态键命中：点亮红石矿 {15, 9}。
        let lit = resolve_props(
            "minecraft:redstone_ore",
            &[("lit".to_string(), "true".to_string())],
        );
        assert_eq!(lit, BlockLightProps { opacity: 15, luminance: 9 });
        // 带状态键未命中、方块名级非实心：火把系保持发光。
        let torch = resolve_props(
            "minecraft:wall_torch",
            &[("waterlogged".to_string(), "false".to_string())],
        );
        assert_eq!(torch, BlockLightProps { opacity: 0, luminance: 14 });
        // 未命中且名字级实心：普通实心。
        let ore = resolve_props(
            "minecraft:redstone_ore",
            &[("lit".to_string(), "false".to_string())],
        );
        assert_eq!(ore, BlockLightProps::SOLID);
    }

    #[test]
    fn state_table_known_ids() {
        setup_mojang_cache();
        let table = state_table().unwrap();
        assert_eq!(table.get(0), BlockLightProps::AIR); // minecraft:air
    }
}

//! 原版生成/存档地形来源：qexed 服务端作为 **TCP 客户端** 向
//! chunk-summon 服务（tools/mc-vanilla-chunk-summon，默认 127.0.0.1:54132）
//! 请求区块——客户端是因为要生成新区块才发起请求；服务端不缓存区块内容，
//! 带存档启动时命中原版存档读盘、未命中原版生成。
//!
//! 两条接入路径：
//! 1. [TcpChunkSource]：异步请求 + 解码，拿到协议 MapChunk；
//! 2. [TcpChunkProvider]：适配同步 ChunkPacketProvider，直接接入
//!    [crate::stream::SpawnChunkStreamer]（出生流送/玩家跑图扩展圈），
//!    请求失败可配置回退空区块（连接断开的服务不拖垮整个登录流程）。
//!
//! 回包即完整 ClientboundLevelChunkWithLightPacket 字节（原版编码，
//! 含 26.1.2 section 头部双 i16 与非 NBT heightmap），与 qexed_protocol
//! 的 MapChunk 结构一一对应，反序列化后可直接经 PacketSink::send 下发。

use std::collections::{HashMap, VecDeque};

use bytes::BytesMut;
use qexed_chunk_by_tcp::{ChunkClientError, Dimension as TcpDimension, PooledChunkClient};
use qexed_packet::{Packet, PacketReader};
use qexed_protocol::to_client::play::map_chunk::MapChunk;

use crate::error::{Result, WorldError};
use crate::stream::ChunkPacketProvider;
use crate::tcp_light::TcpLightCache;

/// chunk-summon 连接池大小：每连接一线程串行生成，8 连接 ≈ 8 路并行生成。
const POOL_SIZE: usize = 8;

/// 区块地形来源客户端：持有到 chunk-summon 服务的单个连接。
///
/// 连接是有状态流（帧序列），不跨任务共享；每个流送任务持有一个实例。
pub struct TcpChunkSource {
    client: PooledChunkClient,
    dimension: TcpDimension,
    /// 请求失败时回退空区块（chunk_packet 语义需要）；false = 直接报错。
    fallback_to_empty: bool,
    /// 跨 chunk 光照缓存（3×3 邻域重算 + 同坐标回放），随连接生命周期。
    light_cache: TcpLightCache,
    /// 邻块预完成缓存：补取时 3×3 已齐的邻居直接按中心语义算好整包光照,
    /// 该块稍后成为中心时免 Java 二次生成（服务端无内容缓存,重复生成
    /// 占跑图请求量近半）。LRU 限量,包体 ~20KB/块。
    ready_chunks: HashMap<(i32, i32), MapChunk>,
    ready_order: VecDeque<(i32, i32)>,
}

impl TcpChunkSource {
    /// 连接指定地址的 chunk-summon 服务。
    ///
    /// 开 [POOL_SIZE] 条连接：chunk-summon 每连接一线程逐个生成，多连接
    /// 即多路并行生成（单连接流水线只省 RTT，生成仍串行）。
    pub async fn connect(
        addr: impl tokio::net::ToSocketAddrs + Clone,
    ) -> std::result::Result<Self, ChunkClientError> {
        Ok(Self {
            client: PooledChunkClient::connect(addr, POOL_SIZE).await?,
            dimension: TcpDimension::Overworld,
            fallback_to_empty: false,
            light_cache: TcpLightCache::new(),
            ready_chunks: HashMap::new(),
            ready_order: VecDeque::new(),
        })
    }

    /// 连接本机默认端口 54132。
    pub async fn connect_local() -> std::result::Result<Self, ChunkClientError> {
        Self::connect(("127.0.0.1", qexed_chunk_by_tcp::DEFAULT_PORT)).await
    }

    /// 连接 "host:port" 形式的地址（配置项 chunk_tcp_addr）。
    pub async fn connect_addr(
        addr: &str,
    ) -> std::result::Result<Self, ChunkClientError> {
        Self::connect(addr).await
    }

    /// 设置请求维度（服务端当前仅实现主世界，该字段为协议预留）。
    pub fn with_dimension(mut self, dimension: TcpDimension) -> Self {
        self.dimension = dimension;
        self
    }

    /// 请求失败回退空区块（默认不回退，直接报 [WorldError::ChunkTcp]）。
    pub fn with_fallback_to_empty(mut self, fallback: bool) -> Self {
        self.fallback_to_empty = fallback;
        self
    }

    /// keep-alive 探测（连接健康检查）。
    pub async fn ping(&mut self, id: i64) -> std::result::Result<i64, ChunkClientError> {
        self.client.ping(id).await
    }

    /// 请求一个区块并解码为协议 MapChunk（含 3×3 邻域光照重算）。
    ///
    /// 服务端按需产出（存档命中读盘 / 未命中生成），无内容缓存——同一坐标
    /// 两次请求会得到两次独立编码的字节（同一 JVM 内确定一致）。
    pub async fn request_map_chunk(&mut self, chunk_x: i32, chunk_z: i32) -> Result<MapChunk> {
        // 邻块预完成命中：整包回放,仍走光照管线（邻居应已入库,补取为 0）。
        if let Some(chunk) = self.take_ready(chunk_x, chunk_z) {
            return self.finish_pipeline(chunk).await;
        }
        let data = match self
            .client
            .request_chunks(&[(chunk_x, chunk_z)])
            .await
            .map(|mut datas| datas.pop().expect("单元素请求必有回包"))
        {
            Ok(data) => data,
            Err(e) if self.fallback_to_empty => {
                log::warn!("tcp 区块请求失败 ({chunk_x},{chunk_z})，回退空区块: {e}");
                return Ok(crate::chunk::empty_chunk_packet(chunk_x, chunk_z));
            }
            Err(e) => return Err(WorldError::ChunkTcp(e)),
        };
        let mut chunk = decode_map_chunk(&data)?;
        debug_assert_eq!(
            (chunk.chunk_x, chunk.chunk_z),
            (chunk_x, chunk_z),
            "服务端回包坐标必须与请求一致（客户端已校验）"
        );
        // 两阶段光照：plan（缓存命中直接回放）→ 补取缺失 3×3 邻居 → finish。
        if let Some(plan) = self.light_cache.plan_overwrite(&mut chunk)? {
            for (nx, nz) in self
                .light_cache
                .missing_neighbors(chunk_x, chunk_z)
            {
                self.fetch_neighbor(nx, nz).await;
            }
            self.light_cache.finish_overwrite(&mut chunk, plan);
        }
        Ok(chunk)
    }

    /// 流水线批量请求一批区块（各自含 3×3 邻域光照重算）。
    ///
    /// 批内互为邻居的坐标互相补齐光学体/光照：先解码全部回包，plan 出每个
    /// 中心的缺失邻居，再一次批量请求补取（大部分邻居已在批内，实际网络
    /// 往返远小于 9N），最后逐个 finish。回包顺序与 coords 一致。
    pub async fn request_map_chunks(
        &mut self,
        coords: &[(i32, i32)],
    ) -> Result<Vec<MapChunk>> {
        if coords.is_empty() {
            return Ok(Vec::new());
        }
        // 邻块预完成命中的坐标直接回放,其余才走网络。
        let mut pending = Vec::with_capacity(coords.len());
        let mut ready_hits: Vec<Option<MapChunk>> = Vec::with_capacity(coords.len());
        for &(x, z) in coords {
            if let Some(chunk) = self.take_ready(x, z) {
                ready_hits.push(Some(chunk));
            } else {
                ready_hits.push(None);
                pending.push((x, z));
            }
        }
        if pending.is_empty() {
            let mut out = Vec::with_capacity(ready_hits.len());
            for hit in ready_hits.into_iter().flatten() {
                out.push(self.finish_pipeline(hit).await?);
            }
            return Ok(out);
        }
        let coords = &pending;
        let wire_start = std::time::Instant::now();
        let datas = match self.client.request_chunks(coords).await {
            Ok(datas) => datas,
            Err(e) if self.fallback_to_empty => {
                log::warn!("tcp 批量区块请求失败，整批回退空区块: {e}");
                return Ok(coords
                    .iter()
                    .map(|&(x, z)| crate::chunk::empty_chunk_packet(x, z))
                    .collect());
            }
            Err(e) => return Err(WorldError::ChunkTcp(e)),
        };
        let wire_elapsed = wire_start.elapsed();
        if wire_elapsed.as_millis() > 50 {
            log::info!(
                "tcp 批量请求 {} 块: 网络耗时 {:?} (~{} ms/块)",
                coords.len(),
                wire_elapsed,
                wire_elapsed.as_millis() / coords.len() as u128
            );
        }
        // 解码 + plan；fallback 语义下单区块解码失败也回退空区块。
        let mut chunks = Vec::with_capacity(coords.len());
        let mut plans = Vec::with_capacity(coords.len());
        for data in &datas {
            let mut chunk = match decode_map_chunk(data) {
                Ok(chunk) => chunk,
                Err(e) if self.fallback_to_empty => {
                    log::warn!("tcp 区块解码失败 ({},{}),回退空区块: {e}", data.chunk_x, data.chunk_z);
                    crate::chunk::empty_chunk_packet(data.chunk_x, data.chunk_z)
                }
                Err(e) => return Err(e),
            };
            debug_assert_eq!(
                (chunk.chunk_x, chunk.chunk_z),
                (data.chunk_x, data.chunk_z),
                "服务端回包坐标必须与请求一致（客户端已校验）"
            );
            let plan = match self.light_cache.plan_overwrite(&mut chunk) {
                Ok(plan) => plan,
                Err(e) => {
                    // 光照失败保留服务端形态（空光照可玩），不拖垮整批。
                    log::warn!(
                        "tcp 区块 ({},{}) 光照计算失败，保留服务端空光照: {e}",
                        chunk.chunk_x,
                        chunk.chunk_z
                    );
                    None
                }
            };
            chunks.push(chunk);
            plans.push(plan);
        }
        // 收集全部中心的缺失邻居，一次批量补取（批内已有的自动跳过）。
        let mut missing: Vec<(i32, i32)> = Vec::new();
        for (chunk, plan) in chunks.iter().zip(&plans) {
            if plan.is_some() {
                for key in self
                    .light_cache
                    .missing_neighbors(chunk.chunk_x, chunk.chunk_z)
                {
                    if !missing.contains(&key) {
                        missing.push(key);
                    }
                }
            }
        }
        if !missing.is_empty() {
            log::info!(
                "tcp 邻居补取 {} 块 (批中心 {})",
                missing.len(),
                chunks.len()
            );
            if let Err(e) = self.prefetch_neighbors(&missing).await {
                log::warn!("tcp 邻居补取失败（{} 个），边界光照退化为无播种: {e}", missing.len());
            }
        }
        // finish 回填（plan 顺序即 coords 顺序）。
        for (chunk, plan) in chunks.iter_mut().zip(plans.into_iter()) {
            if let Some(plan) = plan {
                self.light_cache.finish_overwrite(chunk, plan);
            }
        }
        // 按原 coords 顺序合并;预完成命中的块也走光照管线收尾。
        let mut fetched = chunks.into_iter();
        let mut merged: Vec<MapChunk> = Vec::with_capacity(ready_hits.len());
        for hit in ready_hits {
            match hit {
                Some(chunk) => merged.push(self.finish_pipeline(chunk).await?),
                None => merged.push(fetched.next().expect("fetched length matches")),
            }
        }
        Ok(merged)
    }

    /// 单个邻居补取：解码失败/请求失败仅告警（缺邻居 → 引擎不播种语义兜底）。
    async fn fetch_neighbor(&mut self, nx: i32, nz: i32) {
        match self
            .client
            .request_chunks(&[(nx, nz)])
            .await
            .map(|mut datas| datas.pop().expect("单元素请求必有回包"))
        {
            Ok(data) => match decode_map_chunk(&data) {
                Ok(chunk) => {
                    if let Err(e) = self.light_cache.plan_neighbor_packet(&chunk) {
                        log::warn!("tcp 邻居 ({nx},{nz}) 光照解码失败: {e}");
                    }
                }
                Err(e) => log::warn!("tcp 邻居 ({nx},{nz}) 解码失败: {e}"),
            },
            Err(e) => log::warn!("tcp 邻居请求失败 ({nx},{nz}): {e}"),
        }
    }

    /// 批量邻居补取（批内重复自动去重过的坐标集）。
    ///
    /// raw 缓存命中的邻居直接入光照缓存（零网络），只有真缺的才请求 Java。
    async fn prefetch_neighbors(&mut self, keys: &[(i32, i32)]) -> std::result::Result<(), ChunkClientError> {
        let mut pending: Vec<(i32, i32)> = Vec::with_capacity(keys.len());
        for &(x, z) in keys {
            if let Some(chunk) = self.take_ready(x, z) {
                if let Err(e) = self.light_cache.plan_neighbor_packet(&chunk) {
                    log::warn!("tcp 邻居 ({x},{z}) 光照解码失败: {e}");
                }
                // 放回 raw 缓存：该块日后成为中心时仍需整包回放。
                self.store_ready(chunk);
            } else {
                pending.push((x, z));
            }
        }
        if pending.is_empty() {
            return Ok(());
        }
        let datas = self
            .client
            .request_chunks(&pending)
            .await?;
        for data in &datas {
            match decode_map_chunk(data) {
                Ok(chunk) => {
                    if let Err(e) = self.light_cache.plan_neighbor_packet(&chunk) {
                        log::warn!("tcp 邻居 ({},{}) 光照解码失败: {e}", chunk.chunk_x, chunk.chunk_z);
                        continue;
                    }
                    // 无条件入 raw 缓存：该块稍后成为中心时免 Java 二次生成。
                    self.store_ready(chunk);
                }
                Err(e) => {
                    log::warn!("tcp 邻居 ({},{}) 解码失败: {e}", data.chunk_x, data.chunk_z)
                }
            }
        }
        Ok(())
    }

    /// 增量流送复用：取走某坐标已算好的协议 Light（无则 None）。
    pub fn take_cached_light(&mut self, chunk_x: i32, chunk_z: i32) -> Option<qexed_protocol::to_client::play::map_chunk::Light> {
        self.light_cache.take_packet_light(chunk_x, chunk_z)
    }

    /// 邻块预完成缓存命中：整包直接回放（含已算光照），免 Java 二次生成。
    fn take_ready(&mut self, chunk_x: i32, chunk_z: i32) -> Option<MapChunk> {
        if let Some(chunk) = self.ready_chunks.remove(&(chunk_x, chunk_z)) {
            self.ready_order.retain(|k| *k != (chunk_x, chunk_z));
            return Some(chunk);
        }
        None
    }

    /// 邻块预完成入库：无条件缓存原始地形包（含 Java 编码的 heightmap/sections）。
    /// 该块稍后成为中心时直接取出，邻居此刻已在光照缓存 → 免 Java 二次生成。
    fn store_ready(&mut self, chunk: MapChunk) {
        let key = (chunk.chunk_x, chunk.chunk_z);
        if self.ready_chunks.contains_key(&key) {
            return;
        }
        if self.ready_order.len() >= 256 {
            if let Some(oldest) = self.ready_order.pop_front() {
                self.ready_chunks.remove(&oldest);
            }
        }
        self.ready_order.push_back(key);
        self.ready_chunks.insert(key, chunk);
    }

    /// 光照管线：plan → 补缺失邻居 → finish（失败时保留服务端空光照）。
    async fn finish_pipeline(&mut self, mut chunk: MapChunk) -> Result<MapChunk> {
        let plan = match self.light_cache.plan_overwrite(&mut chunk) {
            Ok(plan) => plan,
            Err(e) => {
                log::warn!(
                    "tcp 区块 ({},{}) 光照计算失败，保留服务端空光照: {e}",
                    chunk.chunk_x,
                    chunk.chunk_z
                );
                return Ok(chunk);
            }
        };
        if let Some(plan) = plan {
            for (nx, nz) in self.light_cache.missing_neighbors(chunk.chunk_x, chunk.chunk_z) {
                if self.ready_chunks.contains_key(&(nx, nz)) {
                    // raw 缓存已有该邻居：直接入光照缓存，零网络。
                    let ready = self.take_ready(nx, nz).expect("contains_key checked");
                    if let Err(e) = self.light_cache.plan_neighbor_packet(&ready) {
                        log::warn!("tcp 邻居 ({nx},{nz}) 光照解码失败: {e}");
                    }
                    self.store_ready(ready);
                    continue;
                }
                self.fetch_neighbor(nx, nz).await;
            }
            self.light_cache.finish_overwrite(&mut chunk, plan);
        }
        Ok(chunk)
    }
}

/// 解码 chunk_data 包体为协议 MapChunk，并复核坐标。
fn decode_map_chunk(data: &qexed_chunk_by_tcp::ChunkData) -> Result<MapChunk> {
    let mut buf = BytesMut::from(&data.packet[..]);
    let mut reader = PacketReader::new(&mut buf);
    let mut chunk = MapChunk::default();
    // MapChunk 只实现 Packet（顶层包结构体走 packet 宏）；子结构体才是 PacketCodec
    Packet::deserialize(&mut chunk, &mut reader)?;
    if chunk.chunk_x != data.chunk_x || chunk.chunk_z != data.chunk_z {
        return Err(WorldError::Other(format!(
            "chunk 包内坐标 ({},{}) 与载荷坐标 ({},{}) 不一致",
            chunk.chunk_x, chunk.chunk_z, data.chunk_x, data.chunk_z
        )));
    }
    Ok(chunk)
}

/// [ChunkPacketProvider] 适配器：把异步 [TcpChunkSource] 接入同步的
/// 出生流送计划（SpawnChunkStreamer::next_batch_with）。
///
/// 借用式包装：来源归会话所有（增量跑图复用连接与光照缓存），流送期
/// 临时可变借用。内部用 block_in_place + Handle::block_on 桥接；仅可在
/// 多线程 tokio 运行时中使用。
pub struct TcpChunkProvider<'a> {
    source: &'a mut TcpChunkSource,
    handle: tokio::runtime::Handle,
}

impl<'a> TcpChunkProvider<'a> {
    /// 在 tokio 运行时上下文中包装来源的可变借用。
    pub fn new(source: &'a mut TcpChunkSource) -> Self {
        Self {
            source,
            handle: tokio::runtime::Handle::current(),
        }
    }
}

impl ChunkPacketProvider for TcpChunkProvider<'_> {
    fn chunk_packet(&mut self, cx: i32, cz: i32) -> Result<Option<MapChunk>> {
        let handle = self.handle.clone();
        let source = &mut *self.source;
        tokio::task::block_in_place(|| handle.block_on(source.request_map_chunk(cx, cz)))
            .map(Some)
    }

    /// 流水线批量：一次发送窗口收齐整批（含批内邻域光照重算），
    /// 出生流送每批的网络往返从 N 次降为 1 次。
    fn chunk_packets(&mut self, coords: &[(i32, i32)]) -> Result<Vec<MapChunk>> {
        if coords.is_empty() {
            return Ok(Vec::new());
        }
        let handle = self.handle.clone();
        let source = &mut *self.source;
        tokio::task::block_in_place(|| handle.block_on(source.request_map_chunks(coords)))
    }
}

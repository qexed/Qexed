//! 出生区区块批次流送。
//!
//! 对齐 Pumpkin `net/chunk_sender.rs` 的最小裁剪版：
//! - 批次包裹：`ChunkBatchStart` → N × `ChunkData` → `ChunkBatchFinished`
//! - 客户端回 `ChunkBatchReceived(desired_chunks_per_tick)` 调节后续节奏
//!   （首版记录但不做动态调速，固定每批大小）
//! - 视野圈按切比雪夫距离环形展开，由近到远，先中心后边缘

use qexed_protocol::to_client::play::{chunk_batch_finished::ChunkBatchFinished, chunk_batch_start::ChunkBatchStart, map_chunk::MapChunk};

use crate::chunk::empty_chunk_packet;

/// 单个批次的区块数。Pumpkin 初始速率为 9 区块/tick，首版取等值。
pub const CHUNKS_PER_BATCH: usize = 9;

/// 区块包来源抽象：给定坐标产出真实编码区块（anvil 管线实现此接口）。
/// 返回 `None` 表示无真实区块，由调用方回退 [`crate::chunk::empty_chunk_packet`]。
pub trait ChunkPacketProvider {
    fn chunk_packet(&mut self, cx: i32, cz: i32) -> crate::error::Result<Option<MapChunk>>;

    /// 批量取一批区块，返回长度必须等于 `coords.len()`。
    ///
    /// 默认实现逐个调用 [`Self::chunk_packet`]（每坐标一次往返）；
    /// 网络来源的实现方应覆写为流水线批量请求（一次发送窗口收齐整批），
    /// 出生流送的 33 个批次从 N 次往返降为每批 1 次。
    fn chunk_packets(&mut self, coords: &[(i32, i32)]) -> crate::error::Result<Vec<MapChunk>> {
        let mut out = Vec::with_capacity(coords.len());
        for &(x, z) in coords {
            out.push(
                self.chunk_packet(x, z)?
                    .unwrap_or_else(|| empty_chunk_packet(x, z)),
            );
        }
        Ok(out)
    }
}

/// 默认来源：全部空区块（保持既有内存构造行为，无 IO）。
pub struct EmptyChunkProvider;

impl ChunkPacketProvider for EmptyChunkProvider {
    fn chunk_packet(&mut self, _cx: i32, _cz: i32) -> crate::error::Result<Option<MapChunk>> {
        Ok(None)
    }
}

/// 出生区流送计划：中心 + 螺旋展开的视野圈。
#[derive(Debug, Clone)]
pub struct SpawnChunkStreamer {
    center: (i32, i32),
    view_distance: i32,
    next: usize,
}

impl SpawnChunkStreamer {
    pub fn new(center: (i32, i32), view_distance: i32) -> Self {
        Self {
            center,
            view_distance: view_distance.clamp(0, 32),
            next: 0,
        }
    }

    /// 视野圈内的区块总数：(2r+1)^2。
    pub fn total_chunks(&self) -> usize {
        let r = self.view_distance as usize;
        (2 * r + 1) * (2 * r + 1)
    }

    /// 是否已全部计划。
    pub fn is_complete(&self) -> bool {
        self.next >= self.total_chunks()
    }

    /// 第 n 个应发送的区块坐标（公开只读访问；计划顺序见 plan 测试锚定）。
    pub fn plan_public(&self, n: usize) -> (i32, i32) {
        self.plan(n)
    }

    /// 第 n 个应发送的区块坐标：按切比雪夫环从内到外，环内按行扫描。
    fn plan(&self, n: usize) -> (i32, i32) {
        let (cx, cz) = self.center;
        // 环半径：中心 1 个，第 ring 环（ring ≥ 1）共 8·ring 个。
        // 注意边界：n = (2k+1)^2 时应进入下一环 offset 0。
        let mut ring = 0usize;
        let mut remaining = n;
        if remaining >= 1 {
            remaining -= 1;
            ring = 1;
        }
        while ring > 0 && remaining >= 8 * ring {
            remaining -= 8 * ring;
            ring += 1;
        }
        if ring == 0 {
            return (cx, cz);
        }
        // 环上第 remaining 个：顺时针、西北角起步。
        // 北行含两端角共 2r+1 点，东/南/西各 2r/2r/(2r-1) 点，合计 8r。
        let r = ring as i32;
        let off = remaining as i32;
        let (dx, dz) = if off <= 2 * r {
            (off - r, -r)        // 北：z = -r，x 从 -r 到 +r
        } else if off <= 4 * r {
            (r, off - 3 * r)     // 东：x = +r，z 从 -r+1 到 +r
        } else if off <= 6 * r {
            (5 * r - off, r)     // 南：z = +r，x 从 +r-1 到 -r
        } else {
            (-r, 7 * r - off)    // 西：x = -r，z 从 +r-1 到 -r+1
        };
        (cx + dx, cz + dz)
    }

    /// 下一批应发送的坐标集（最多 [CHUNKS_PER_BATCH] 个，近到远顺序）；
    /// 计划完成后返回 None。供异步流送任务（无 provider 场景）按批取坐标。
    pub fn next_coords(&mut self) -> Option<Vec<(i32, i32)>> {
        if self.is_complete() {
            return None;
        }
        let end = (self.next + CHUNKS_PER_BATCH).min(self.total_chunks());
        let coords: Vec<(i32, i32)> = (self.next..end).map(|i| self.plan(i)).collect();
        self.next = end;
        Some(coords)
    }

    /// 取下一批（最多 [CHUNKS_PER_BATCH] 个），内容由 provider 产出；
    /// provider 返回 None 的坐标回退空区块。出错时 `next` 不推进，可重试。
    ///
    /// 走 [`ChunkPacketProvider::chunk_packets`] 批量接口（网络来源
    /// 流水线化，一次往返收齐整批）。
    pub fn next_batch_with<P>(
        &mut self,
        provider: &mut P,
    ) -> crate::error::Result<Vec<MapChunk>>
    where
        P: ChunkPacketProvider,
    {
        let end = (self.next + CHUNKS_PER_BATCH).min(self.total_chunks());
        let coords: Vec<(i32, i32)> = (self.next..end).map(|i| self.plan(i)).collect();
        let batch = provider.chunk_packets(&coords)?;
        self.next = end;
        Ok(batch)
    }

    /// 取下一批（最多 [CHUNKS_PER_BATCH] 个）空区块包。
    pub fn next_batch(&mut self) -> Vec<MapChunk> {
        self.next_batch_with(&mut EmptyChunkProvider)
            .expect("EmptyChunkProvider is infallible")
    }
}

/// 一次完整的出生流送：按批次推送全部视野区块，内容由 provider 产出。
///
/// 真实区块（anvil 管线）经 [`ChunkPacketProvider`] 注入；
/// [`send_spawn_chunks`] 为全空区块的默认路径。
pub async fn send_spawn_chunks_with<W, P>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    center: (i32, i32),
    view_distance: i32,
    provider: &mut P,
) -> Result<usize, crate::error::WorldError>
where
    W: tokio::io::AsyncWrite + Unpin,
    P: ChunkPacketProvider,
{
    let mut streamer = SpawnChunkStreamer::new(center, view_distance);
    let mut sent = 0usize;

    while !streamer.is_complete() {
        let batch = streamer.next_batch_with(provider)?;
        let batch_size = batch.len() as i32;
        sink.send(ChunkBatchStart {}).await?;
        for chunk in batch {
            sink.send(chunk).await?;
        }
        sink.send(ChunkBatchFinished {
            batch_size: qexed_packet::net_types::VarInt(batch_size),
        })
        .await?;
        sink.flush().await?;
        sent += batch_size as usize;
    }

    log::debug!(
        "spawn chunks streamed: center=({},{}) view={} chunks={}",
        center.0,
        center.1,
        view_distance,
        sent
    );
    Ok(sent)
}

/// 方形视野圈（切比雪夫距离 ≤ 半径），用于增量跑图的差分计算。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewCircle {
    center: (i32, i32),
    radius: i32,
}

impl ViewCircle {
    pub fn new(center: (i32, i32), radius: i32) -> Self {
        Self {
            center,
            radius: radius.clamp(0, 32),
        }
    }

    pub fn center(&self) -> (i32, i32) {
        self.center
    }

    pub fn radius(&self) -> i32 {
        self.radius
    }

    /// 坐标是否在圈内（切比雪夫距离）。
    pub fn contains(&self, x: i32, z: i32) -> bool {
        (x - self.center.0).abs() <= self.radius && (z - self.center.1).abs() <= self.radius
    }

    /// moved − old：新圈里旧圈没有的坐标（增量加载集），按距新中心由近到远。
    pub fn gained(old: &ViewCircle, moved: &ViewCircle) -> Vec<(i32, i32)> {
        let mut out = Vec::new();
        let r = moved.radius;
        for dz in -r..=r {
            for dx in -r..=r {
                let key = (moved.center.0 + dx, moved.center.1 + dz);
                if !old.contains(key.0, key.1) {
                    out.push(key);
                }
            }
        }
        out.sort_by_key(|&(x, z)| {
            let dx = (x - moved.center.0).abs();
            let dz = (z - moved.center.1).abs();
            (dx.max(dz), dx + dz)
        });
        out
    }

    /// old − moved：旧圈里新圈没有的坐标（增量卸载集）。
    pub fn lost(old: &ViewCircle, moved: &ViewCircle) -> Vec<(i32, i32)> {
        let mut out = Vec::new();
        let r = old.radius;
        for dz in -r..=r {
            for dx in -r..=r {
                let key = (old.center.0 + dx, old.center.1 + dz);
                if !moved.contains(key.0, key.1) {
                    out.push(key);
                }
            }
        }
        out
    }
}

/// 增量区块发送：一个批次（ChunkBatchStart → N×ChunkData → ChunkBatchFinished）。
///
/// 与出生流送同一包裹形态，客户端按已建立的批次节奏消化。
pub async fn send_chunk_batch<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    chunks: &[MapChunk],
) -> Result<(), crate::error::WorldError>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    if chunks.is_empty() {
        return Ok(());
    }
    sink.send(ChunkBatchStart {}).await?;
    for chunk in chunks {
        sink.send(chunk.clone()).await?;
    }
    sink.send(ChunkBatchFinished {
        batch_size: qexed_packet::net_types::VarInt(chunks.len() as i32),
    })
    .await?;
    sink.flush().await?;
    Ok(())
}

/// 一次完整的出生流送：按批次推送全部视野空区块（内存构造，无 IO）。
pub async fn send_spawn_chunks<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    center: (i32, i32),
    view_distance: i32,
) -> Result<usize, crate::error::WorldError>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    send_spawn_chunks_with(sink, center, view_distance, &mut EmptyChunkProvider).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_covers_center_first() {
        let s = SpawnChunkStreamer::new((0, 0), 2);
        assert_eq!(s.plan(0), (0, 0)); // 中心
        assert_eq!(s.plan(1), (-1, -1)); // 半径 1 环：西北角起步，顺时针
        assert_eq!(s.plan(2), (0, -1)); // 北行第二格
        assert_eq!(s.plan(5), (1, 1)); // 东列末端（东北角）
        assert_eq!(s.plan(8), (-1, 0)); // 西列末端
        assert_eq!(s.plan(9), (-2, -2)); // 半径 2 环开始（西北角）
        assert_eq!(s.plan(24), (-2, -1)); // 最后一格（西列末端）
        assert_eq!(s.total_chunks(), 25);
    }

    #[test]
    fn plan_with_negative_center() {
        let s = SpawnChunkStreamer::new((-10, 3), 1);
        assert_eq!(s.plan(0), (-10, 3));
        assert_eq!(s.plan(8), (-11, 3)); // 半径 1 环西列末端
        assert_eq!(s.plan(9), (-12, 1)); // 半径 2 环西北角
        assert_eq!(s.plan(4), (-9, 3));
    }

    /// provider 产出的内容被原样采用，None 坐标回退空区块。
    #[test]
    fn next_batch_with_uses_provider_and_falls_back() {
        struct Marked;
        impl ChunkPacketProvider for Marked {
            fn chunk_packet(
                &mut self,
                cx: i32,
                cz: i32,
            ) -> crate::error::Result<Option<MapChunk>> {
                if (cx, cz) == (0, 0) {
                    let mut chunk = empty_chunk_packet(cx, cz);
                    chunk.data.heightmaps[0].data[0] = 0xdead_beef;
                    return Ok(Some(chunk));
                }
                Ok(None)
            }
        }
        let mut s = SpawnChunkStreamer::new((0, 0), 1);
        let mut batch = s.next_batch_with(&mut Marked).unwrap();
        assert_eq!(batch.len(), 9);
        // 中心区块带标记，其余回退全零。
        let center = batch.remove(0);
        assert_eq!((center.chunk_x, center.chunk_z), (0, 0));
        assert_eq!(center.data.heightmaps[0].data[0], 0xdead_beef);
        assert!(batch.iter().all(|c| c.data.heightmaps[0].data[0] == 0));
        // 默认路径不受影响。
        let mut s2 = SpawnChunkStreamer::new((0, 0), 1);
        assert_eq!(s2.next_batch()[0].data.heightmaps[0].data[0], 0);
    }

    /// provider 出错时错误向上传播，且游标不推进（可重试）。
    #[test]
    fn next_batch_with_propagates_errors_without_advancing() {
        struct Fails;
        impl ChunkPacketProvider for Fails {
            fn chunk_packet(
                &mut self,
                _cx: i32,
                _cz: i32,
            ) -> crate::error::Result<Option<MapChunk>> {
                Err(crate::error::WorldError::Other("boom".into()))
            }
        }
        let mut s = SpawnChunkStreamer::new((0, 0), 1);
        assert!(s.next_batch_with(&mut Fails).is_err());
        assert_eq!(s.next, 0);
    }

    #[test]
    fn batches_cover_exactly_once() {
        let mut s = SpawnChunkStreamer::new((5, -7), 4);
        let total = s.total_chunks(); // 81
        let mut seen = std::collections::HashSet::new();
        let mut batches = 0;
        while !s.is_complete() {
            for (x, z) in s.next_batch().into_iter().map(|c| (c.chunk_x, c.chunk_z)) {
                assert!(seen.insert((x, z)), "duplicate chunk ({x},{z})");
            }
            batches += 1;
        }
        assert_eq!(seen.len(), total);
        assert_eq!(batches, total.div_ceil(CHUNKS_PER_BATCH));
    }
}
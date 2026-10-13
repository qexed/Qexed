//! 增量区块流送 worker：把「网络请求 + 两阶段光照」整体搬出 PLAY 主循环。
//!
//! 主循环的 `tokio::select!` 单任务多路复用——旧实现在移动包处理里内联
//! `await` 批量取块（网络 + 光照 ~0.4s/批），期间 keepalive / 移动 / 交互
//! 全部停摆。worker 把 `TcpChunkSource` 整体接管，主循环只做：
//! 入队（`enqueue`，通道容量 8 自然背压）→ `done_rx` 分支收成品并下发。
//! 请求串行处理（单源单调吞吐即可，pool 已在连接层并行）。

use std::collections::HashSet;

use tokio::sync::mpsc;

use qexed_protocol::to_client::play::map_chunk::MapChunk;
use qexed_world::tcp_source::TcpChunkSource;

/// 一批区块的完成通知：请求坐标 + 取块结果（错误已 String 化，主循环降级用）。
pub struct ChunkJobDone {
    pub coords: Vec<(i32, i32)>,
    pub result: std::result::Result<Vec<MapChunk>, String>,
    /// 出生流送批次标记：主循环用它统计加载进度与收尾日志（增量批恒 false）。
    pub spawn_batch: bool,
}

/// 会话侧句柄：入队 + 收成品 + 在途去重。
pub struct ChunkWorker {
    req_tx: mpsc::UnboundedSender<Vec<(i32, i32)>>,
    pub done_rx: mpsc::UnboundedReceiver<ChunkJobDone>,
    in_flight: HashSet<(i32, i32)>,
}

impl ChunkWorker {
    /// 接管 source 并启动流送任务。任务随 `req_tx` 全部丢弃（会话结束）而退出。
    pub fn spawn(mut source: TcpChunkSource) -> Self {
        let (req_tx, req_rx) = mpsc::unbounded_channel::<Vec<(i32, i32)>>();
        let (done_tx, done_rx) = mpsc::unbounded_channel::<ChunkJobDone>();
        Self::run_task(source, req_rx, done_tx, Vec::new());
        Self {
            req_tx,
            done_rx,
            in_flight: HashSet::new(),
        }
    }

    /// 接管 source 并启动**出生圈流送任务**：按 SpawnChunkStreamer 计划顺序
    /// 逐批产出（近到远，客户端边收边渲染），全部完成后任务**原地转为增量
    /// 模式**继续消费 `req_tx`——连接、TCP 池与光照/ready 缓存全程复用，
    /// 出生期与跑图期共用同一任务。
    ///
    /// 与旧的「主循环内 block_in_place 同步桥接」相比：整个加载期 keepalive /
    /// 移动 / 插件帧照常处理，客户端不会因主循环停摆而卡 loading terrain。
    /// 出生批以 `spawn_batch = true` 上报，主循环按序下发即可。
    pub fn spawn_stream(
        source: TcpChunkSource,
        center: (i32, i32),
        view_distance: i32,
    ) -> Self {
        let mut streamer = qexed_world::stream::SpawnChunkStreamer::new(center, view_distance);
        let mut spawn_batches = Vec::new();
        while let Some(coords) = streamer.next_coords() {
            spawn_batches.push(coords);
        }
        let (req_tx, req_rx) = mpsc::unbounded_channel::<Vec<(i32, i32)>>();
        let (done_tx, done_rx) = mpsc::unbounded_channel::<ChunkJobDone>();
        Self::run_task(source, req_rx, done_tx, spawn_batches);
        Self {
            req_tx,
            done_rx,
            in_flight: HashSet::new(),
        }
    }

    /// 共享任务循环：先逐批处理预注入的出生批（spawn_batch = true），随后
    /// 进入增量循环消费 req_rx。出生批耗尽且 req_tx 全部丢弃（会话结束）后
    /// 任务退出。双向 unbounded：worker 永不阻塞在 done，主循环 select 持续
    /// 排水；批量跑图入队也无需阻塞主循环。
    fn run_task(
        mut source: TcpChunkSource,
        mut req_rx: mpsc::UnboundedReceiver<Vec<(i32, i32)>>,
        done_tx: mpsc::UnboundedSender<ChunkJobDone>,
        spawn_batches: Vec<Vec<(i32, i32)>>,
    ) {
        tokio::spawn(async move {
            for coords in spawn_batches {
                let result = source
                    .request_map_chunks(&coords)
                    .await
                    .map_err(|e| e.to_string());
                if done_tx
                    .send(ChunkJobDone { coords, result, spawn_batch: true })
                    .is_err()
                {
                    return; // 会话已结束
                }
            }
            while let Some(coords) = req_rx.recv().await {
                let result = source
                    .request_map_chunks(&coords)
                    .await
                    .map_err(|e| e.to_string());
                if done_tx
                    .send(ChunkJobDone { coords, result, spawn_batch: false })
                    .is_err()
                {
                    break; // 会话已结束
                }
            }
        });
    }

    /// 入队一批：纯内存操作（unbounded 通道 + 在途去重），永不阻塞主循环。
    /// 已在途的坐标自动剔除，避免重复网络/光照开销。
    pub fn enqueue(&mut self, coords: Vec<(i32, i32)>) {
        let fresh: Vec<(i32, i32)> = coords
            .into_iter()
            .filter(|&(x, z)| self.in_flight.insert((x, z)))
            .collect();
        if fresh.is_empty() {
            return;
        }
        let _ = self.req_tx.send(fresh);
    }

    /// 该坐标是否已请求、尚未完成（重复请求去重）。
    pub fn is_queued(&self, x: i32, z: i32) -> bool {
        self.in_flight.contains(&(x, z))
    }

    /// 批次完成（成功或失败都算）：从在途集合移除。
    pub fn complete(&mut self, x: i32, z: i32) {
        self.in_flight.remove(&(x, z));
    }
}

/// 会话持有的可选 worker：给主循环 `select!` 提供统一分支。
/// `select!` 的分支表达式必须是 Future 本身（`Option<Future>` 不行），
/// 故由这里收口：无 worker 时永久挂起（分支永不就绪），
/// worker 通道关闭时返回 `None` 使分支被禁用（会话收尾语义）。
pub struct SessionWorker(pub Option<ChunkWorker>);

impl SessionWorker {
    /// select 分支专用：等待下一批完成；None = 通道关闭（worker 已退出）。
    pub async fn recv_done(&mut self) -> Option<ChunkJobDone> {
        match self.0.as_mut() {
            Some(w) => w.done_rx.recv().await,
            None => std::future::pending().await,
        }
    }

    pub fn as_mut(&mut self) -> Option<&mut ChunkWorker> {
        self.0.as_mut()
    }
}

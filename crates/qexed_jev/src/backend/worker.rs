//! 内嵌推理 worker：每实例一条专用线程 + 有界 MPSC 队列 + 微批。
//!
//! 性能模型（为什么不用 tokio 任务 + Mutex<Engine>）：
//! - **不偷 tokio worker 时间片**：jev 推理是毫秒~秒级 CPU/加速器密集型工作，
//!   放进异步任务会阻塞 tokio runtime 的 worker 线程，拖慢所有玩家的网络包处理。
//!   这里用一条专用 OS 线程独占引擎，主进程只需要 `try_send`（永不阻塞）。
//! - **有界队列 = 硬背压**：队列满时 [`EmbeddedBackend::complete`] 立刻返回
//!   [`JevError::QueueFull`]，上层可以直接给玩家"服务器繁忙"提示；
//!   内存占用上限恒为 `queue_capacity` 条请求，不会被刷屏玩家打爆。
//! - **微批**：worker 每次最多取 `micro_batch` 条就绪请求一次性喂给引擎，
//!   真实 jev 权重接入后可显著提升加速器吞吐（回声引擎只是逐条执行这批请求）。
//! - **快速失败**：引擎初始化失败或 worker 退出时，等待方拿到
//!   [`JevError::WorkerDown`] / [`JevError::Inference`]，不会静默挂死。
//! - **每实例独立**：worker 生命周期跟随 [`EmbeddedBackend`]（Drop 即断开队列、
//!   线程退出），不搞进程级单例，测试与多实例部署都干净。

use crate::config::embedded::JevEmbeddedConfig;
use crate::engine::{JevEngine, JevRequest};
use crate::error::JevError;
use std::sync::mpsc::{Receiver, SyncSender, TrySendError};
use std::time::{Duration, Instant};
use tokio::sync::oneshot;

/// 单条排队请求：引擎入参 + 结果回传通道。
struct Job {
    request: JevRequest,
    timeout_at: Instant,
    reply: oneshot::Sender<Result<String, JevError>>,
}

/// 内嵌后端：克隆不出第二个 worker（引擎 + 队列被 Arc 语义独占）。
pub struct EmbeddedBackend {
    tx: SyncSender<Job>,
    queue_capacity: usize,
    request_timeout_ms: u64,
    micro_batch: usize,
}

impl EmbeddedBackend {
    /// 启动 worker 线程。引擎在 worker 线程内构造：
    /// 真实 jev 权重加载可能耗时数秒，不能卡住服务器启动路径。
    pub fn spawn(config: &JevEmbeddedConfig) -> Result<Self, JevError> {
        config.validate()?;
        let (tx, rx) = std::sync::mpsc::sync_channel::<Job>(config.queue_capacity);
        let thread_config = config.clone();
        std::thread::Builder::new()
            .name("qexed-jev-worker".into())
            .spawn(move || worker_loop(thread_config, rx))
            .map_err(|err| JevError::WorkerSpawn(err.to_string()))?;
        log::info!(
            "jev embedded worker started (queue_capacity={}, micro_batch={}, idle_priority={})",
            config.queue_capacity,
            config.micro_batch,
            config.thread_priority_idle
        );
        Ok(Self {
            tx,
            queue_capacity: config.queue_capacity,
            request_timeout_ms: config.request_timeout_ms,
            micro_batch: config.micro_batch,
        })
    }

    /// 提交一次内嵌推理。队列满立刻返回 [`JevError::QueueFull`]（硬背压）。
    pub async fn complete(&self, request: JevRequest, timeout_ms: u64) -> Result<String, JevError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        let job = Job {
            request,
            timeout_at: Instant::now() + Duration::from_millis(timeout_ms),
            reply: reply_tx,
        };
        match self.tx.try_send(job) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => {
                return Err(JevError::QueueFull { capacity: self.queue_capacity })
            }
            Err(TrySendError::Disconnected(_)) => return Err(JevError::WorkerDown),
        }
        let reply = tokio::time::timeout(Duration::from_millis(timeout_ms), reply_rx)
            .await
            .map_err(|_| JevError::Timeout { timeout_ms })?;
        match reply {
            Ok(result) => result,
            // worker 掉线时 oneshot 关闭：把错误反馈给等待方而不是挂死
            Err(_) => Err(JevError::WorkerDown),
        }
    }

    /// 暴露给日志/监控的配置快照。
    pub fn stats(&self) -> (usize, usize, u64) {
        (self.queue_capacity, self.micro_batch, self.request_timeout_ms)
    }
}

impl Drop for EmbeddedBackend {
    fn drop(&mut self) {
        // 断开发送端：worker 在下次 recv 失败时自行退出
        log::info!("jev embedded backend dropped; worker will exit");
    }
}

/// worker 主循环：凑批 → 引擎 → 回发。
fn worker_loop(config: JevEmbeddedConfig, rx: Receiver<Job>) {
    let mut engine = match build_embedded_engine_config(&config) {
        Ok(engine) => engine,
        Err(err) => {
            log::error!("jev engine init failed, failing every queued job: {err}");
            for job in rx.into_iter() {
                let _ = job.reply.send(Err(JevError::Inference(err.to_string())));
            }
            return;
        }
    };
    log::info!("jev engine ready");

    let micro_batch = config.micro_batch.max(1);
    loop {
        // 阻塞等第一条；发送端全部 Drop（后端关闭）就退出
        let first = match rx.recv() {
            Ok(job) => job,
            Err(_) => {
                log::info!("jev worker exiting (backend dropped)");
                return;
            }
        };
        let mut batch = vec![first];
        // 尽量凑满 micro_batch 的就绪请求
        while batch.len() < micro_batch {
            match rx.try_recv() {
                Ok(job) => batch.push(job),
                Err(_) => break,
            }
        }

        for job in batch {
            if Instant::now() >= job.timeout_at {
                let _ = job.reply.send(Err(JevError::Timeout {
                    timeout_ms: config.request_timeout_ms,
                }));
                continue;
            }
            let result = engine.complete(job.request);
            let _ = job.reply.send(result.map(|c| c.text));
        }
    }
}

use crate::config::JevConfig;

/// 适配层：把 embedded 段配置包成完整配置喂给引擎构造器。
fn build_embedded_engine_config(
    config: &JevEmbeddedConfig,
) -> Result<Box<dyn JevEngine>, JevError> {
    let full = JevConfig {
        embedded: config.clone(),
        ..JevConfig::default()
    };
    crate::engine::build_embedded_engine(&full)
}

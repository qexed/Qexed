#[derive(Debug, thiserror::Error)]
pub enum JevError {
    /// 配置加载失败（qexed_config 底层错误）。
    #[error("config error: {0}")]
    Config(#[from] qexed_config::error::ConfigError),

    /// 内嵌 worker 线程启动失败。
    #[error("jev worker spawn failed: {0}")]
    WorkerSpawn(String),

    /// 内嵌 worker 已退出（panic 或被显式关闭），无法再接受推理请求。
    #[error("jev worker is not running")]
    WorkerDown,

    /// 推理请求队列已满：系统过载，直接拒绝而不是无限堆积内存。
    #[error("jev request queue full (capacity={capacity})")]
    QueueFull { capacity: usize },

    /// 单次请求在 deadline 内没有拿到结果（排队 + 推理总耗时）。
    #[error("jev inference timed out after {timeout_ms}ms")]
    Timeout { timeout_ms: u64 },

    /// 引擎内部的模型/算子错误（内嵌引擎或远端推理透传）。
    #[error("jev inference failed: {0}")]
    Inference(String),

    /// 模式/backend 配置非法（如 remote 模式没有编译 remote feature）。
    #[error("invalid jev config: {0}")]
    InvalidConfig(String),

    /// 输入内容命中拦截类规则（Action::Block），会话被拒绝。
    #[error("message rejected by moderation (categories={categories})")]
    Moderated { categories: String },
}

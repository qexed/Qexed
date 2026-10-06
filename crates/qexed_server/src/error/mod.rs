//! qexed_server 错误类型：覆盖 server/bootstrap/console/commands/status/services/
//! cluster/warden/content_filter/code_of_conduct/audit/placeholders/l10n 全部模块。
//! 禁止 anyhow —— 所有错误统一走这里的 thiserror 类型。

/// qexed_server 统一错误类型。
#[derive(Debug, thiserror::Error)]
pub enum ServerError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Config Error: {0}")]
    ConfigError(#[from] qexed_config::error::ConfigError),
    /// JSON 序列化/反序列化失败（审计日志、语言文件、集群 RPC 等）。
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    /// 数据包编解码失败（命令树、SystemChat 广播、集群帧负载等）。
    #[error("packet error: {0}")]
    Packet(#[from] qexed_packet::PacketError),
    /// NBT 编解码失败。
    #[error("nbt error: {0}")]
    Nbt(#[from] qexed_nbt::NbtError),
    /// Mojang 注册表数据未就绪。
    #[error("mojang data error: {0}")]
    MojangData(#[from] qexed_mojang_data::error::MojangDataError),
    /// tokio 任务 Join 失败（全局服务 tick / spawn_blocking 等）。
    #[error("task join error: {0}")]
    Join(String),
    /// 网络地址解析失败（集群 endpoint / 监听地址）。
    #[error("invalid endpoint: {0}")]
    InvalidEndpoint(String),
    /// 集群 RPC 帧超过大小上限。
    #[error("cluster frame too large: {0} bytes")]
    ClusterFrameTooLarge(usize),
    /// 带上下文的 IO 错误（路径/操作说明 + 源错误）。
    #[error("{context}: {source}")]
    IoContext {
        context: String,
        #[source]
        source: std::io::Error,
    },
    /// 带上下文的配置错误。
    #[error("config error: {context}: {source}")]
    ConfigContext {
        context: String,
        #[source]
        source: qexed_config::error::ConfigError,
    },
    /// 通用业务错误消息（内容过滤、行为准则、l10n 初始化等）。
    #[error("{0}")]
    Message(String),
}

impl ServerError {
    /// 构造通用消息错误。
    pub fn msg(message: impl Into<String>) -> Self {
        Self::Message(message.into())
    }

    /// 给 IO 结果附加上下文（类似 anyhow::Context 但走 thiserror 变体）。
    pub fn io_ctx<T>(
        result: std::result::Result<T, std::io::Error>,
        context: impl Into<String>,
    ) -> std::result::Result<T, Self> {
        result.map_err(|source| Self::IoContext {
            context: context.into(),
            source,
        })
    }
}

/// 本 crate 统一 Result 别名。
pub type Result<T> = std::result::Result<T, ServerError>;

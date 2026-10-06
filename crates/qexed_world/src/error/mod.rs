//! qexed_world 错误类型：覆盖 world/generator/manager/light/region/chunk_nbt/
//! cluster/rules/ore_pits 全部模块（迁移规则：禁止 anyhow，一律 thiserror）。

/// qexed_world 统一错误类型。
#[derive(Debug, thiserror::Error)]
pub enum WorldError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Config Error: {0}")]
    ConfigError(#[from] qexed_config::error::ConfigError),
    /// NBT 编解码失败（区块存档解析 / 序列化）。
    #[error("nbt error: {0}")]
    Nbt(#[from] qexed_nbt::NbtError),
    /// 数据包编解码失败（区块网络包 / 光照层数据）。
    #[error("packet error: {0}")]
    Packet(#[from] qexed_packet::PacketError),
    /// JSON 解析失败（noise settings / flat preset）。
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    /// Mojang 注册表数据未就绪（blocks.json 报告缺失等）。
    #[error("registry error: {0}")]
    Registry(#[from] qexed_mojang_data::registry_sync::RegistryError),
    /// toml 读写失败（维度规则文件）。
    #[error("toml error: {0}")]
    Toml(#[from] qexed_toml::TomlError),

    /// v4 anyhow::bail!/context 的显式化（region/chunk_nbt/rules 等）。
    #[error("{context}: {source}")]
    IoContext {
        context: String,
        #[source]
        source: std::io::Error,
    },
    /// 带上下文的 NBT 错误（区块 NBT 解析 / 序列化定位）。
    #[error("{context}: {source}")]
    NbtContext {
        context: String,
        #[source]
        source: qexed_nbt::NbtError,
    },
    /// 通用业务错误消息（区块数据损坏 / 光照长度校验 / 集群路由错误等）。
    #[error("{0}")]
    Message(String),
}

impl WorldError {
    /// 等价 anyhow::anyhow!/bail! 的裸消息构造。
    pub fn msg(message: impl Into<String>) -> Self {
        Self::Message(message.into())
    }

    /// 等价 anyhow::Context 的 `.context(...)`（io）。
    pub fn io_context(context: impl Into<String>, source: std::io::Error) -> Self {
        Self::IoContext {
            context: context.into(),
            source,
        }
    }

    /// 给 IO 结果附加上下文（`WorldError::io_ctx(result, "…")`）。
    pub fn io_ctx<T>(
        result: std::result::Result<T, std::io::Error>,
        context: impl Into<String>,
    ) -> std::result::Result<T, Self> {
        result.map_err(|source| Self::IoContext {
            context: context.into(),
            source,
        })
    }

    /// 给 NBT 结果附加上下文。
    pub fn nbt_ctx<T>(
        result: std::result::Result<T, qexed_nbt::NbtError>,
        context: impl Into<String>,
    ) -> std::result::Result<T, Self> {
        result.map_err(|source| Self::NbtContext {
            context: context.into(),
            source,
        })
    }
}

/// 本 crate 统一 Result 别名。
pub type Result<T> = std::result::Result<T, WorldError>;

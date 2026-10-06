//! qexed_plugins 错误类型（禁 anyhow，一律 thiserror）。

#[derive(Debug, thiserror::Error)]
pub enum PluginsError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Config Error: {0}")]
    ConfigError(#[from] qexed_config::error::ConfigError),
    /// 动态库加载/符号解析失败（libloading）。
    #[error("load library failed: {0}")]
    LoadLibrary(#[from] libloading::Error),
    /// 插件缺失必需的导出符号（如 qexed_plugin_alloc / host 表）。
    #[error("plugin {plugin} missing required export: {symbol}")]
    MissingExport { plugin: String, symbol: String },
    /// 插件事件/查询 payload 编解码失败。
    #[error("plugin payload codec error: {0}")]
    Payload(#[source] Box<dyn std::error::Error + Send + Sync>),
    /// 插件事件/查询 payload 超过大小限制。
    #[error("plugin payload too large: {size} > {max}")]
    PayloadTooLarge { size: usize, max: usize },
    /// 插件调用返回长度溢出/非法值。
    #[error("plugin {plugin} returned invalid length: {value}")]
    InvalidLength { plugin: String, value: i64 },
    /// 经济系统存储后端错误。
    #[error("economy storage error: {0}")]
    EconomyStorage(String),
    /// 插件结构化存储后端错误。
    #[error("structured storage error: {0}")]
    StructuredStorage(String),
}

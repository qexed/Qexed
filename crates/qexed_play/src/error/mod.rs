//! qexed_play 错误类型（迁移规则：禁止 anyhow，一律 thiserror）。

#[derive(Debug, thiserror::Error)]
pub enum PlayError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Config Error: {0}")]
    ConfigError(#[from] qexed_config::error::ConfigError),
    #[error("packet encode error: {0}")]
    PacketEncode(#[from] qexed_packet::PacketError),
    #[error("player domain error: {0}")]
    Player(#[from] qexed_player::PlayerError),
    #[error("entities domain error: {0}")]
    Entities(#[from] qexed_entities::error::EntitiesError),
    /// 传输层写出（PacketSink send/flush）失败。
    #[error("transport write error: {0}")]
    PacketWrite(#[from] qexed_connection::transport::PacketWriteError),
    /// 传输层读入（PacketStream read）失败。
    #[error("transport read error: {0}")]
    PacketRead(#[from] qexed_connection::transport::PacketReadError),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    /// 世界域错误（WorldRulesManager 的 time/gamerule 写路径）。
    #[error("world domain error: {0}")]
    World(#[from] qexed_world::error::WorldError),
    /// 原版注册表数据（blocks.json / registries.json 报告）加载失败。
    #[error("registry data error: {0}")]
    Registry(#[from] qexed_mojang_data::registry_sync::RegistryError),
    #[error("{0}")]
    Message(String),
}

impl PlayError {
    /// 兼容 v4 anyhow 风格的动态错误消息构造。
    pub fn msg(message: impl Into<String>) -> Self {
        Self::Message(message.into())
    }
}

pub type Result<T> = std::result::Result<T, PlayError>;

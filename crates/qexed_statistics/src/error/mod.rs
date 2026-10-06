//! 统计域错误。

#[derive(Debug, thiserror::Error)]
pub enum StatisticsError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("nbt error: {0}")]
    Nbt(#[from] qexed_nbt::NbtError),
    #[error("packet error: {0}")]
    Packet(#[from] qexed_packet::PacketError),
    #[error("config error: {0}")]
    Config(#[from] qexed_config::error::ConfigError),
    #[error("{0}")]
    Message(String),
}

impl StatisticsError {
    pub fn msg(message: impl Into<String>) -> Self {
        Self::Message(message.into())
    }
}

pub type Result<T> = std::result::Result<T, StatisticsError>;

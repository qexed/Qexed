#[derive(Debug, thiserror::Error)]
pub enum PlayerError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Config Error: {0}")]
    ConfigError(#[from] qexed_config::error::ConfigError),
    #[error("packet error: {0}")]
    Packet(#[from] qexed_packet::PacketError),
    #[error("nbt error: {0}")]
    Nbt(#[from] qexed_nbt::NbtError),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("base64 error: {0}")]
    Base64(#[from] base64::DecodeError),
    #[error("{0}")]
    Message(String),
}

impl PlayerError {
    pub fn msg(message: impl Into<String>) -> Self {
        Self::Message(message.into())
    }
}

pub type Result<T> = std::result::Result<T, PlayerError>;

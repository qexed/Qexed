//! qexed_server 错误。

#[derive(Debug, thiserror::Error)]
pub enum ServerError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("packet codec error: {0}")]
    Packet(#[from] qexed_packet::error::PacketError),

    #[error("packet ID mismatch: expected {expected}, got {actual}")]
    PacketIdMismatch { expected: i32, actual: i32 },

    #[error("connection closed while reading {what}")]
    ConnectionClosed { what: &'static str },

    #[error("invalid packet length: {0}")]
    InvalidPacketLength(i32),

    #[error("packet too large: {len} > {max}")]
    PacketTooLarge { len: usize, max: usize },

    #[error("compression format error: {0}")]
    Compression(String),

    #[error("unexpected state: {0}")]
    UnexpectedState(String),

    #[error("registry sync error: {0}")]
    RegistrySync(String),
}

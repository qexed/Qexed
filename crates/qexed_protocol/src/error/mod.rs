pub use qexed_packet::error::PacketError;

/// qexed_protocol 统一错误类型。
///
/// 编解码路径统一使用 [`PacketError`]（定义于 `qexed_packet`）；
/// `ProtocolError` 面向上层调用方，可将编解码错误与协议层错误一并传播。
#[derive(Debug, thiserror::Error)]
pub enum ProtocolError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("config error: {0}")]
    ConfigError(#[from] qexed_config::error::ConfigError),

    #[error("packet codec error: {0}")]
    Packet(#[from] PacketError),

    #[error("packet id mismatch: expected {expected}, got {got}")]
    PacketIdMismatch { expected: i32, got: i32 },
}

pub type Result<T> = std::result::Result<T, ProtocolError>;

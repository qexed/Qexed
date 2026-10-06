//! TCP 传输层（v4 独立 crate qexed_tcp_connect 迁移到 qexed_connection::transport）。
//!
//! v4 的 rust_i18n + qexed_error_macros 在 v6 不存在：错误统一 thiserror，
//! 用户可见文案走 qexed_language::t；加密从 openssl 换成 RustCrypto aes+cfb8。

pub mod bind;
mod packet_read;
mod packet_write;

pub use bind::{BindError, bind};
pub use packet_read::{PacketReadError, PacketReadVarIntParseError, PacketStream};
pub use packet_write::{PacketSend, PacketSink, PacketWriteError};

/// 传输层错误：读/写二选一的包装（供 error::ConnectionError::Transport 使用）。
#[derive(Debug, thiserror::Error)]
pub enum TransportError {
    #[error("{0}")]
    Read(#[from] PacketReadError),
    #[error("{0}")]
    Write(#[from] PacketWriteError),
}

impl From<PacketReadError> for crate::error::ConnectionError {
    fn from(value: PacketReadError) -> Self {
        crate::error::ConnectionError::Transport(TransportError::Read(value))
    }
}

impl From<PacketWriteError> for crate::error::ConnectionError {
    fn from(value: PacketWriteError) -> Self {
        crate::error::ConnectionError::Transport(TransportError::Write(value))
    }
}

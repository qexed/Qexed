//! PLAY 阶段错误类型。

use qexed_chat::ChatError;

#[derive(Debug, thiserror::Error)]
pub enum PlayError {
    // ── 外部 crate 源错误 ──
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("tcp write error: {0}")]
    TcpWrite(#[from] qexed_tcp_connect::error::TcpConnectError),

    #[error("chat error: {0}")]
    Chat(#[from] ChatError),

    #[error("config error: {0}")]
    ConfigError(#[from] qexed_config::error::ConfigError),

    // ── PLAY 自身 ──
    #[error("protocol violation: {0}")]
    Protocol(String),

    #[error("timeout: {0}")]
    Timeout(String),

    #[error("chunk tcp source error: {0}")]
    ChunkTcp(String),
}

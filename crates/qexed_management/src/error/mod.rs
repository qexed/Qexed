//! 管理协议错误类型。

#[derive(Debug, thiserror::Error)]
pub enum ManagementError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("websocket error: {0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),
    #[error("config error: {0}")]
    Config(#[from] qexed_config::error::ConfigError),
    #[error("{0}")]
    Message(String),
}

impl ManagementError {
    pub fn msg(message: impl Into<String>) -> Self {
        Self::Message(message.into())
    }
}

pub type Result<T> = std::result::Result<T, ManagementError>;

/// JSON-RPC 错误码（对齐规范常用段）。
pub mod rpc_code {
    pub const PARSE_ERROR: i64 = -32700;
    pub const INVALID_REQUEST: i64 = -32600;
    pub const METHOD_NOT_FOUND: i64 = -32601;
    pub const INVALID_PARAMS: i64 = -32602;
    pub const INTERNAL_ERROR: i64 = -32603;
    pub const UNAUTHORIZED: i64 = 401;
}

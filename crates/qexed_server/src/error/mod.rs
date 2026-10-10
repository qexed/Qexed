#[derive(Debug, thiserror::Error)]
pub enum ServerError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("config error: {0}")]
    Config(#[from] qexed_config::error::ConfigError),
    #[error("tcp connect error: {0}")]
    TcpConnect(#[from] qexed_tcp_connect::error::TcpConnectError),
    #[error("auth error: {0}")]
    Auth(#[from] qexed_auth::error::AuthError),
    #[error("protocol error: {0}")]
    Protocol(String),
}
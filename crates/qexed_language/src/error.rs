// 语言服务错误
#[derive(Debug, thiserror::Error)]
pub enum LanguageError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Config Error: {0}")]
    ConfigError(#[from] qexed_config::error::ConfigError),
    #[error("remote fetch failed: {0}")]
    Remote(String),
    #[error("http client error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("all language sources failed: {0}")]
    AllSourcesFailed(String),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}

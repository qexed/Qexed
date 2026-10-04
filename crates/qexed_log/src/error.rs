// 日志错误
#[derive(Debug, thiserror::Error)]
pub enum LogError{


    // #[error("config path already initialized")]
    // AlreadyInitialized,

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Config Error: {0}")]
    ConfigError(#[from] qexed_config::error::ConfigError),
    
}
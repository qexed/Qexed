// 配置文件错误
#[derive(Debug, thiserror::Error)]
pub enum ConfigError{
    #[error("toml error: {0}")]
    TomlError(#[from] qexed_toml::TomlError),

    #[error("config path already initialized")]
    AlreadyInitialized,

    #[error("config path not initialized")]
    NotInitialized,

    #[error("invalid config name `{0}`: only alphanumeric and `_` allowed")]
    InvalidName(String),

    #[error("invalid config path: {0}")]
    InvalidPath(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
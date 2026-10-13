#[derive(Debug, thiserror::Error)]
pub enum StatError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Config Error: {0}")]
    ConfigError(#[from] qexed_config::error::ConfigError),
    #[error("registry error: {0}")]
    Registry(#[from] qexed_registry::error::RegistryError),
    // 下面更多的自定义错误
}

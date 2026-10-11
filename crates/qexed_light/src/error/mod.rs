#[derive(Debug, thiserror::Error)]
pub enum LightError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Config Error: {0}")]
    ConfigError(#[from] qexed_config::error::ConfigError),
    #[error("block coordinate out of chunk: x={x}, y={y}, z={z}")]
    BlockOutOfBounds { x: usize, y: i32, z: usize },
    #[error("light data length mismatch: expected {expected}, got {got}")]
    LengthMismatch { expected: usize, got: usize },
}

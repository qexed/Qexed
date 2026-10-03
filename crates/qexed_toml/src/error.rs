
use std::io;

#[derive(Debug, thiserror::Error)]
pub enum TomlError {
    /// 文件已存在
    #[error("file already exists")]
    AlreadyExists,
    /// 文件不存在
    #[error("file not found")]
    NotFound,
    /// 路径非空且不是文件
    #[error("{0}")]
    InvalidInput(String),
    /// 其他Io错误
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
    // toml_edit的Error
    #[error("toml_edit ser error: {0}")]
    TomlEditSerError(#[from] toml_edit::ser::Error),
    #[error("toml_edit de error: {0}")]
    TomlEditDeError(#[from] toml_edit::de::Error),

    #[error("toml_edit error: {0}")]
    TomlEditError(#[from] toml_edit::TomlError),
}


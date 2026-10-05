use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum MojangDataError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("{context}: {source}")]
    IoContext {
        context: String,
        #[source]
        source: std::io::Error,
    },
    #[error("Config Error: {0}")]
    ConfigError(#[from] qexed_config::error::ConfigError),
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("zip error: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("Mojang version manifest 中找不到版本: {0}")]
    VersionNotFound(String),
    #[error("Mojang 版本元数据缺少 server/client 下载项: {0}")]
    MissingDownloads(String),
    #[error("SHA1 不匹配: expected={expected}, actual={actual}")]
    Sha1Mismatch { expected: String, actual: String },
    #[error("Mojang jar 大小不匹配: expected={expected}, actual={actual}")]
    SizeMismatch { expected: u64, actual: usize },
    #[error("Mojang jar 中没有找到 data/minecraft JSON 数据")]
    NoMinecraftData,
    #[error("Mojang jar 中无法提取 registry data，已尝试: {0}")]
    ExtractFailed(String),
    #[error("Mojang 数据缓存初始化后仍缺少 data/minecraft: {0}")]
    DataNotReady(PathBuf),
    #[error("等待 Mojang 下载锁超时: {0}")]
    LockTimeout(PathBuf),
    #[error("后台下载任务失败: {0}")]
    Join(String),
}

pub(crate) trait IoCtx<T> {
    fn io_ctx(self, context: impl Into<String>) -> Result<T, MojangDataError>;
}

impl<T> IoCtx<T> for Result<T, std::io::Error> {
    fn io_ctx(self, context: impl Into<String>) -> Result<T, MojangDataError> {
        self.map_err(|source| MojangDataError::IoContext {
            context: context.into(),
            source,
        })
    }
}

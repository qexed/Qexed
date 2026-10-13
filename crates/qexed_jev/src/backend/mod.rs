//! 推理后端分发：内嵌 worker（默认）或远端 HTTP，对上层统一为 [`JevBackend`]。

#[cfg(feature = "remote")]
pub mod remote;
mod worker;

use crate::config::{JevConfig, JevMode};
use crate::engine::JevRequest;
use crate::error::JevError;

pub use worker::EmbeddedBackend;

/// 后端句柄：[`JevService`] 独占一个实例。
pub enum JevBackend {
    /// 内嵌 worker（默认）：专用线程 + 有界队列 + 微批。
    Embedded(EmbeddedBackend),
    /// 远端 HTTP（需要 `remote` feature）。
    #[cfg(feature = "remote")]
    Remote(remote::RemoteBackend),
}

/// 按配置构造后端；构造即校验，配置错误在启动期暴露。
pub fn build_backend(config: &JevConfig) -> Result<JevBackend, JevError> {
    match config.mode {
        JevMode::Embedded => Ok(JevBackend::Embedded(EmbeddedBackend::spawn(
            &config.embedded,
        )?)),
        #[cfg(feature = "remote")]
        JevMode::Remote => Ok(JevBackend::Remote(remote::RemoteBackend::new(
            config.remote.clone(),
        )?)),
        #[cfg(not(feature = "remote"))]
        JevMode::Remote => Err(JevError::InvalidConfig(
            "mode = \"remote\" requires compiling with the \'remote\' feature".into(),
        )),
    }
}

impl JevBackend {
    /// 提交一次推理。返回回复文本；超时对两种模式统一生效。
    pub async fn complete(&self, request: JevRequest, timeout_ms: u64) -> Result<String, JevError> {
        match self {
            JevBackend::Embedded(embedded) => embedded.complete(request, timeout_ms).await,
            #[cfg(feature = "remote")]
            JevBackend::Remote(remote) => remote.complete(request).await,
        }
    }
}

//! jev 推理引擎抽象层。
//!
//! 分发规则（`build_embedded_engine`）：
//! - `embedded.model_path` 为空 → EchoEngine（确定性回声，仅联调用）；
//! - 指向 GGUF 文件 → LocalEngine（llama.cpp CPU 推理，加载失败硬失败）。
//!
//! 模型路径解析（`resolve_model_path`）：绝对路径原样使用；相对路径锚定
//! 「服务器主目录」（配置根目录的上一级，dev 下为 `run/`），与进程 CWD 无关，
//! 且在启动时即定死——生产环境按部署布局摆放权重即可，不要求存在 `run/`。

mod echo;
mod local;

use crate::config::JevConfig;
use crate::error::JevError;
use std::path::{Path, PathBuf};

pub use echo::EchoEngine;
pub use local::LocalEngine;

/// 一次推理请求（引擎视角，不含排队元数据）。
#[derive(Debug, Clone)]
pub struct JevRequest {
    /// 模型标识；内嵌单模型时忽略，远端模式映射到 `model` 字段。
    pub model: Option<String>,
    /// 对话上下文（含系统提示与历史，调用方负责裁剪长度）。
    pub messages: Vec<(String, String)>,
    /// 采样温度；内嵌引擎固定贪心采样，此字段仅远端模式使用。
    pub temperature: Option<f32>,
}

/// 一次推理结果。
#[derive(Debug, Clone)]
pub struct JevCompletion {
    pub text: String,
    /// 真实推理耗时（不含排队）。
    pub elapsed_ms: u64,
}

/// jev 推理引擎 trait：内嵌服务与远端 HTTP 共用的最低公共接口。
///
/// 实现方约束：
/// - [`Self::complete`] 允许阻塞（在内嵌 worker 专用线程上调用），
///   实现内部不应该再依赖 tokio 运行时；
/// - 实现必须可以在多线程间移动（[`Send`] + `'static`），worker 线程独占持有。
pub trait JevEngine: Send + 'static {
    fn complete(&mut self, request: JevRequest) -> Result<JevCompletion, JevError>;
}

/// 相对 `base` 解析模型路径；绝对路径原样返回（纯函数，便于测试）。
fn resolve_model_path_at(base: &Path, raw: &str) -> PathBuf {
    let path = Path::new(raw);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        base.join(path)
    }
}

/// 解析 `embedded.model_path` 为实际文件路径。
///
/// 相对路径的锚点是「服务器主目录」= 配置根目录（`--config-path`，默认
/// `./config`）的上一级：dev 下 chdir 进 `run/` 后即 `run/`；生产下由部署方
/// 通过 `--config-path` 决定布局。解析在启动时一次性完成，之后 chdir 不再影响。
fn resolve_model_path(raw: &str) -> Result<PathBuf, JevError> {
    let path = Path::new(raw);
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }
    let config_root = qexed_config::config_path().map_err(|err| {
        JevError::InvalidConfig(format!(
            "embedded.model_path `{raw}` 是相对路径，但配置根目录未初始化: {err}"
        ))
    })?;
    // "./config" 的 parent 是空串，语义上就是当前目录。
    let home = match config_root.parent() {
        Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
        _ => PathBuf::from("."),
    };
    // absolute() 只做词法处理（不要求目录存在），按当前 CWD 定死为绝对路径。
    let home = std::path::absolute(&home)
        .map_err(|err| JevError::InvalidConfig(format!("解析服务器主目录失败: {err}")))?;
    Ok(resolve_model_path_at(&home, raw))
}

/// 按配置构造内嵌引擎。
pub fn build_embedded_engine(config: &JevConfig) -> Result<Box<dyn JevEngine>, JevError> {
    let model_path = config.embedded.model_path.trim();
    if model_path.is_empty() {
        // 明确的回声兜底：语义判定（semantic）对回声输出永远解析失败，
        // 即审核退化为"纯关键词规则"——服务端启动时会为此打警告。
        log::warn!(
            "jev embedded.model_path is empty: using EchoEngine (keyword-only moderation, \
             no model understanding)"
        );
        return Ok(Box::new(EchoEngine::new()));
    }
    // 启动时一次性把相对路径定死为绝对路径，之后（哪怕发生 chdir）不再漂移。
    let resolved = resolve_model_path(model_path)?;
    log::info!(
        "jev embedded.model_path `{model_path}` resolved to {}",
        resolved.display()
    );
    let engine = LocalEngine::load(&resolved)?;
    Ok(Box::new(engine))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absolute_path_is_used_verbatim() {
        let p = resolve_model_path_at(Path::new("/srv/qexed"), "C:\\models\\a.gguf");
        assert_eq!(p, PathBuf::from("C:\\models\\a.gguf"));
        let p = resolve_model_path_at(Path::new("/srv/qexed"), "/opt/models/a.gguf");
        assert_eq!(p, PathBuf::from("/opt/models/a.gguf"));
    }

    #[test]
    fn relative_path_joins_base() {
        let p = resolve_model_path_at(Path::new("E:\\srv\\qexed"), "models/a.gguf");
        assert_eq!(p, PathBuf::from("E:\\srv\\qexed").join("models/a.gguf"));
        let p = resolve_model_path_at(Path::new("/srv/qexed"), "models/a.gguf");
        assert_eq!(p, PathBuf::from("/srv/qexed/models/a.gguf"));
    }

    #[test]
    fn config_root_parent_becomes_home() {
        let config_root = Path::new("/srv/qexed/config");
        let home = config_root.parent().unwrap();
        assert_eq!(home, Path::new("/srv/qexed"));
        let p = resolve_model_path_at(home, "models/a.gguf");
        assert_eq!(p, PathBuf::from("/srv/qexed/models/a.gguf"));
    }
}

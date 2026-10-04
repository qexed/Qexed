//! 语言文件服务器：语言翻译文件的只读 HTTP 视图与静态导出。
//!
//! 数据目录结构（服务与静态导出同构，可直接由 nginx / 对象存储托管导出产物）：
//!
//! ```text
//! <root>/qexed/<commit>/language.json               qexed 本体语言支持列表
//! <root>/qexed/<commit>/<language>.json             qexed 本体对应语言翻译
//! <root>/<author>/<plugin>/<commit>/language.json   插件语言支持列表
//! <root>/<author>/<plugin>/<commit>/<language>.json 插件对应语言翻译
//! ```
//!
//! HTTP 路由规则（与 LanguageConfig.server_url 注释约定一致）：
//!
//! ```text
//! GET /api/v1/qexed/{commit}/language.json
//! GET /api/v1/qexed/{commit}/{language}.json
//! GET /api/v1/{author}/{plugin}/{commit}/language.json
//! GET /api/v1/{author}/{plugin}/{commit}/{language}.json
//! ```

pub mod error;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use std::path::PathBuf;

/// 数据根目录（语言文件树的根）。
#[derive(Debug, Clone)]
pub struct LanguageRoot(PathBuf);

impl LanguageRoot {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self(root.into())
    }

    fn file_path(&self, segments: &[&str]) -> Option<PathBuf> {
        // 路径段安全校验：拒绝空段、相对引用与非法字符，防目录穿越。
        let ok = segments.iter().all(|s| {
            !s.is_empty()
                && *s != "." && *s != ".."
                && s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
        });
        if !ok {
            return None;
        }
        let mut path = self.0.clone();
        for s in segments {
            path.push(s);
        }
        path.set_extension("json");
        Some(path)
    }
}

type ApiResult = Result<Json<serde_json::Value>, ApiError>;

/// 统一 API 错误：转 HTTP 状态码 + JSON 体。
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("invalid path segment")]
    BadSegment,
    #[error("language file not found")]
    NotFound,
    #[error("commit not found")]
    CommitNotFound,
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid json: {0}")]
    InvalidJson(#[from] serde_json::Error),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let status = match &self {
            ApiError::BadSegment => StatusCode::BAD_REQUEST,
            ApiError::NotFound => StatusCode::NOT_FOUND,
            ApiError::CommitNotFound => StatusCode::NOT_FOUND,
            ApiError::Io(_) | ApiError::InvalidJson(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (status, Json(serde_json::json!({ "error": self.to_string() }))).into_response()
    }
}

/// 读取一份语言 json。
async fn read_json(path: Option<PathBuf>) -> ApiResult {
    let Some(path) = path else {
        return Err(ApiError::BadSegment);
    };
    let bytes = tokio::fs::read(&path).await.map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => ApiError::NotFound,
        _ => ApiError::Io(e),
    })?;
    let value = serde_json::from_slice(&bytes)?;
    Ok(Json(value))
}

/// 语言支持列表：目录内除 language.json 外的全部 json 文件名（不含扩展名），排序输出。
async fn language_list(dir: PathBuf) -> ApiResult {
    let mut entries = tokio::fs::read_dir(&dir).await.map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => ApiError::CommitNotFound,
        _ => ApiError::Io(e),
    })?;
    let mut languages = Vec::new();
    while let Some(entry) = entries.next_entry().await? {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some(stem) = name.strip_suffix(".json") else {
            continue;
        };
        if stem != "language" && !stem.is_empty() {
            languages.push(stem.to_string());
        }
    }
    languages.sort();
    Ok(Json(serde_json::json!({ "languages": languages })))
}

async fn qexed_language_list(
    State(root): State<LanguageRoot>,
    Path(commit): Path<String>,
) -> ApiResult {
    let mut dir = root.0.join("qexed").join(&commit);
    dir.set_extension("");
    language_list(dir).await
}

async fn qexed_language_file(
    State(root): State<LanguageRoot>,
    Path((commit, language)): Path<(String, String)>,
) -> ApiResult {
    read_json(root.file_path(&["qexed", &commit, &language])).await
}

async fn plugin_language_list(
    State(root): State<LanguageRoot>,
    Path((author, plugin, commit)): Path<(String, String, String)>,
) -> ApiResult {
    let dir = root.0.join(&author).join(&plugin).join(&commit);
    language_list(dir).await
}

async fn plugin_language_file(
    State(root): State<LanguageRoot>,
    Path((author, plugin, commit, language)): Path<(String, String, String, String)>,
) -> ApiResult {
    read_json(root.file_path(&[&author, &plugin, &commit, &language])).await
}

/// 构建语言服务器 Router。
pub fn router(root: LanguageRoot) -> Router {
    Router::new()
        .route("/api/v1/qexed/{commit}/language.json", get(qexed_language_list))
        .route("/api/v1/qexed/{commit}/{language}", get(qexed_language_file))
        .route("/api/v1/{author}/{plugin}/{commit}/language.json", get(plugin_language_list))
        .route("/api/v1/{author}/{plugin}/{commit}/{language}", get(plugin_language_file))
        .with_state(root)
}

/// 静态导出：把整个语言文件树按服务器同构布局写进 out_dir。
///
/// commit_filter 传 None 导出全部 commit；传 Some 前缀集合只导出匹配项。
/// 导出产物可直接交给 nginx / 对象存储托管。
pub async fn export(root: &LanguageRoot, out_dir: &std::path::Path) -> Result<(), error::ServerError> {
    copy_tree(&root.0, out_dir).await
}

fn copy_tree<'a>(src: &'a std::path::Path, dst: &'a std::path::Path) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), error::ServerError>> + Send + 'a>> {
    Box::pin(async move {
        tokio::fs::create_dir_all(dst).await?;
        let mut entries = tokio::fs::read_dir(src).await?;
        while let Some(entry) = entries.next_entry().await? {
            let ty = entry.file_type().await?;
            let target = dst.join(entry.file_name());
            if ty.is_dir() {
                copy_tree(&entry.path(), &target).await?;
            } else {
                tokio::fs::copy(entry.path(), &target).await?;
            }
        }
        Ok(())
    })
}

//! 聊天内容过滤。
//! 迁移自 v4 crates/qexed/src/content_filter.rs；
//! anyhow → crate::error::ServerError，配置类型换成 crate::config::ContentFilterConfig。
//!
//! Api 引擎走 reqwest（POST api_url，Bearer api_token，
//! ApiRequest{text} → ApiResponse{block,filtered_text,reason}）。

use serde::{Deserialize, Serialize};

use crate::config::{ContentFilterConfig, ContentFilterEngine};
use crate::error::{Result, ServerError};

#[derive(Debug, Clone)]
pub struct ContentFilter {
    config: ContentFilterConfig,
    words: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilterAction {
    Allow(String),
    Block { reason: String },
}

impl ContentFilter {
    pub fn from_config(config: &ContentFilterConfig) -> Result<Self> {
        let mut words = config.words.clone();
        if config.enable && config.engine == ContentFilterEngine::Knowledge {
            words.extend(load_words(&config.knowledge_path)?);
        }
        words.retain(|word| !word.trim().is_empty());
        words.sort();
        words.dedup();

        Ok(Self {
            config: config.clone(),
            words,
        })
    }

    /// 完全禁用态（配置加载失败时的兜底）。
    pub fn disabled() -> Self {
        Self {
            config: ContentFilterConfig::default(),
            words: Vec::new(),
        }
    }

    /// 检查一条聊天消息；固定词/词库引擎做替换，Api 引擎委托远端。
    pub async fn check_chat(&self, message: &str) -> Result<FilterAction> {
        if !self.config.enable {
            return Ok(FilterAction::Allow(message.to_string()));
        }

        match self.config.engine {
            ContentFilterEngine::Fixed | ContentFilterEngine::Knowledge => {
                Ok(self.check_words(message))
            }
            ContentFilterEngine::Api => self.check_api(message).await,
        }
    }

    fn check_words(&self, message: &str) -> FilterAction {
        let mut filtered = message.to_string();
        let mut matched = false;
        for word in &self.words {
            if word.is_empty() {
                continue;
            }
            if filtered.contains(word) {
                filtered = filtered.replace(word, &self.config.replacement);
                matched = true;
            }
        }

        if matched {
            FilterAction::Allow(filtered)
        } else {
            FilterAction::Allow(message.to_string())
        }
    }

    async fn check_api(&self, message: &str) -> Result<FilterAction> {
        if self.config.api_url.trim().is_empty() {
            log::warn!("{}", qexed_language::t("qexed.server.content_filter.api_url_empty"));
            return Ok(FilterAction::Allow(message.to_string()));
        }

        // v4 语义：POST api_url（Bearer api_token）ApiRequest{text}
        // → ApiResponse{block,filtered_text,reason}。失败放行并记警告
        // （过滤服务不可用不应阻断聊天）。
        let client = api_client();
        let mut request = client
            .post(self.config.api_url.trim())
            .json(&ApiRequest { text: message });
        let token = self.config.api_token.trim();
        if !token.is_empty() {
            request = request.bearer_auth(token);
        }

        match request.send().await {
            Ok(response) => match response.error_for_status() {
                Ok(response) => match response.json::<ApiResponse>().await {
                    Ok(data) if data.block => Ok(FilterAction::Block {
                        reason: data.reason.unwrap_or_default(),
                    }),
                    Ok(data) => Ok(FilterAction::Allow(
                        data.filtered_text.unwrap_or_else(|| message.to_string()),
                    )),
                    Err(err) => {
                        log::warn!(
                            "{}",
                            qexed_language::t("qexed.server.content_filter.api_invalid_response")
                                .replace("%{error}", &err.to_string())
                        );
                        Ok(FilterAction::Allow(message.to_string()))
                    }
                },
                Err(err) => {
                    log::warn!(
                        "{}",
                        qexed_language::t("qexed.server.content_filter.api_status_error")
                            .replace("%{error}", &err.to_string())
                    );
                    Ok(FilterAction::Allow(message.to_string()))
                }
            },
            Err(err) => {
                log::warn!(
                    "{}",
                    qexed_language::t("qexed.server.content_filter.api_unavailable")
                        .replace("%{error}", &err.to_string())
                );
                Ok(FilterAction::Allow(message.to_string()))
            }
        }
    }

    pub fn block_message(&self) -> &str {
        &self.config.block_message
    }
}

/// 从敏感词知识库文件加载词表（每行一词，# 开头为注释）。
fn load_words(path: &str) -> Result<Vec<String>> {
    if path.trim().is_empty() {
        return Ok(Vec::new());
    }

    let content = std::fs::read_to_string(path).map_err(|source| ServerError::IoContext {
        context: format!("读取敏感词知识库失败: {path}"),
        source,
    })?;
    Ok(content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(ToOwned::to_owned)
        .collect())
}

/// Api 引擎请求体（v4 结构）。
#[derive(Serialize)]
struct ApiRequest<'a> {
    text: &'a str,
}

/// Api 引擎响应体（v4 结构）。
#[derive(Deserialize, Default)]
struct ApiResponse {
    #[serde(default)]
    block: bool,
    filtered_text: Option<String>,
    reason: Option<String>,
}

/// 过滤 API 客户端（OnceLock 复用连接池）。
fn api_client() -> &'static reqwest::Client {
    static CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .unwrap_or_default()
    })
}

#[cfg(test)]
mod tests {
    use super::{ContentFilter, FilterAction};
    use crate::config::{ContentFilterConfig, ContentFilterEngine};

    #[tokio::test]
    async fn disabled_filter_allows_original_message() {
        let filter = ContentFilter::from_config(&ContentFilterConfig::default()).unwrap();

        assert_eq!(
            filter.check_chat("hello").await.unwrap(),
            FilterAction::Allow("hello".to_string())
        );
    }

    #[tokio::test]
    async fn fixed_filter_replaces_configured_words() {
        let config = ContentFilterConfig {
            enable: true,
            engine: ContentFilterEngine::Fixed,
            words: vec!["bad".to_string()],
            ..ContentFilterConfig::default()
        };
        let filter = ContentFilter::from_config(&config).unwrap();

        assert_eq!(
            filter.check_chat("bad text").await.unwrap(),
            FilterAction::Allow("*** text".to_string())
        );
    }

    #[tokio::test]
    async fn api_engine_with_empty_url_allows_message() {
        let config = ContentFilterConfig {
            enable: true,
            engine: ContentFilterEngine::Api,
            ..ContentFilterConfig::default()
        };
        let filter = ContentFilter::from_config(&config).unwrap();

        assert_eq!(
            filter.check_chat("hello").await.unwrap(),
            FilterAction::Allow("hello".to_string())
        );
    }

    #[tokio::test]
    async fn api_engine_unreachable_service_allows_message() {
        // 不可达端点：请求失败走放行路径（过滤服务故障不阻断聊天）。
        let config = ContentFilterConfig {
            enable: true,
            engine: ContentFilterEngine::Api,
            api_url: "http://127.0.0.1:1/filter".to_string(),
            ..ContentFilterConfig::default()
        };
        let filter = ContentFilter::from_config(&config).unwrap();

        assert_eq!(
            filter.check_chat("hello").await.unwrap(),
            FilterAction::Allow("hello".to_string())
        );
    }
}

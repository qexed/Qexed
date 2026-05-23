use anyhow::{Context, Result};
use qexed_config::app::qexed::server::{ContentFilter as ContentFilterConfig, ContentFilterEngine};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct ContentFilter {
    config: ContentFilterConfig,
    words: Vec<String>,
    client: reqwest::Client,
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
            client: reqwest::Client::new(),
        })
    }

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
            log::warn!("内容过滤 API 已启用但 api_url 为空，消息按通过处理");
            return Ok(FilterAction::Allow(message.to_string()));
        }

        let mut request = self
            .client
            .post(&self.config.api_url)
            .json(&ApiRequest { text: message });
        if !self.config.api_token.trim().is_empty() {
            request = request.bearer_auth(&self.config.api_token);
        }

        let response = request
            .send()
            .await
            .context("call content filter API")?
            .error_for_status()
            .context("content filter API returned error status")?
            .json::<ApiResponse>()
            .await
            .context("decode content filter API response")?;

        if response.block {
            return Ok(FilterAction::Block {
                reason: response
                    .reason
                    .unwrap_or_else(|| self.config.block_message.clone()),
            });
        }

        Ok(FilterAction::Allow(
            response
                .filtered_text
                .unwrap_or_else(|| message.to_string()),
        ))
    }

    pub fn block_message(&self) -> &str {
        &self.config.block_message
    }
}

fn load_words(path: &str) -> Result<Vec<String>> {
    if path.trim().is_empty() {
        return Ok(Vec::new());
    }

    let content =
        std::fs::read_to_string(path).with_context(|| format!("读取敏感词知识库失败: {path}"))?;
    Ok(content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(ToOwned::to_owned)
        .collect())
}

#[derive(Serialize)]
struct ApiRequest<'a> {
    text: &'a str,
}

#[derive(Deserialize)]
struct ApiResponse {
    #[serde(default)]
    block: bool,
    filtered_text: Option<String>,
    reason: Option<String>,
}

#[cfg(test)]
mod tests {
    use qexed_config::app::qexed::server::{
        ContentFilter as ContentFilterConfig, ContentFilterEngine,
    };

    use super::{ContentFilter, FilterAction};

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
}

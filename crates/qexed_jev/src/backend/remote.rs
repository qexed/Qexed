//! 远端后端：OpenAI 兼容 `/chat/completions`。
//!
//! 复用 reqwest 内部连接池（[`RemoteBackend`] 克隆的是 Arc 客户端），
//! 避免 qexed_chat 高频消息反复建连。

use crate::config::remote::JevRemoteConfig;
use crate::engine::{JevCompletion, JevRequest};
use crate::error::JevError;

/// 远端 HTTP 后端（需要编译 `remote` feature）。
#[derive(Clone)]
pub struct RemoteBackend {
    client: reqwest::Client,
    config: JevRemoteConfig,
}

impl RemoteBackend {
    pub fn new(config: JevRemoteConfig) -> Result<Self, JevError> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_millis(config.timeout_ms.max(1)))
            .build()
            .map_err(|err| JevError::InvalidConfig(format!("http client: {err}")))?;
        Ok(Self { client, config })
    }

    /// 执行一次远端推理。超时由 client 统一控制（[`JevRemoteConfig::timeout_ms`]）。
    pub async fn complete(&self, request: JevRequest) -> Result<String, JevError> {
        let started = std::time::Instant::now();
        let model = request
            .model
            .unwrap_or_else(|| self.config.model.clone());
        let messages: Vec<serde_json::Value> = request
            .messages
            .iter()
            .map(|(role, content)| {
                serde_json::json!({ "role": role, "content": content })
            })
            .collect();
        let mut body = serde_json::json!({ "model": model, "messages": messages });
        if let Some(temperature) = request.temperature {
            body["temperature"] = serde_json::json!(temperature);
        }

        let url = format!(
            "{}/chat/completions",
            self.config.base_url.trim_end_matches('/')
        );
        let mut req = self.client.post(url).json(&body);
        if !self.config.api_key.is_empty() {
            req = req.bearer_auth(&self.config.api_key);
        }
        let response = req
            .send()
            .await
            .map_err(|err| JevError::Inference(format!("remote request: {err}")))?;
        let status = response.status();
        if !status.is_success() {
            let body_text = response.text().await.unwrap_or_default();
            return Err(JevError::Inference(format!(
                "remote status {status}: {}",
                truncate(&body_text, 512)
            )));
        }
        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|err| JevError::Inference(format!("remote decode: {err}")))?;
        let text = json["choices"][0]["message"]["content"]
            .as_str()
            .ok_or_else(|| {
                JevError::Inference("remote response missing choices[0].message.content".into())
            })?
            .to_string();
        Ok(text)
    }
}

fn truncate(s: &str, max: usize) -> &str {
    match s.char_indices().nth(max) {
        Some((idx, _)) => &s[..idx],
        None => s,
    }
}

// 让 remote 编译路径也能引用 JevCompletion（留作流式扩展）。
#[allow(dead_code)]
fn _assert_completion_shape(c: JevCompletion) -> String {
    c.text
}

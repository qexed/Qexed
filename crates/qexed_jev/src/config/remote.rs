//! 远端模式配置：OpenAI 兼容服务地址与凭据。
//!
//! `api_key` 是机密字段（见 [`crate::config::JevConfig`] 的 `<Secrets>` 标签）：
//! `load_and_create_default` 会把它拆到 `config/.secrets/jev.toml`，主配置文件
//! 里只留 `<stored in .secrets>` 占位符，避免密钥随配置仓库泄露。

use qexed_doc_macros::Doc;
use serde::{Deserialize, Serialize};

/// ```autodoc
/// <Name>qexed.crates.jev.config.JevRemoteConfig</Name>
/// <Attr name="writable" />
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Doc)]
pub struct JevRemoteConfig {
    /// ```autodoc
    /// <Name>qexed.crates.jev.config.JevRemoteConfig.base_url</Name>
    /// <Default>"http://127.0.0.1:8080/v1"</Default>
    /// <Attr name="writable" />
    /// ```
    pub base_url: String,
    /// ```autodoc
    /// <Name>qexed.crates.jev.config.JevRemoteConfig.model</Name>
    /// <Default>"jev"</Default>
    /// <Attr name="writable" />
    /// ```
    pub model: String,
    /// ```autodoc
    /// <Name>qexed.crates.jev.config.JevRemoteConfig.api_key</Name>
    /// <Default>""</Default>
    /// <Secret />
    /// <Attr name="writable" />
    /// ```
    pub api_key: String,
    /// ```autodoc
    /// <Name>qexed.crates.jev.config.JevRemoteConfig.timeout_ms</Name>
    /// <Default>10000</Default>
    /// <Attr name="writable" />
    /// ```
    pub timeout_ms: u64,
}

impl Default for JevRemoteConfig {
    fn default() -> Self {
        Self {
            base_url: "http://127.0.0.1:8080/v1".to_string(),
            model: "jev".to_string(),
            api_key: String::new(),
            timeout_ms: 10_000,
        }
    }
}

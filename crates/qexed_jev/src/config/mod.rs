pub mod embedded;
pub mod mode;
pub mod remote;

pub use self::mode::JevMode;

use qexed_doc_macros::Doc;
use serde::{Deserialize, Serialize};

// autodoc <Name> 模板：crate 名去掉 qexed_ 前缀，结构体为 PascalCase + Config。
//   qexed_jev -> qexed.crates.jev.config.JevConfig
/// ```autodoc
/// <Name>qexed.crates.jev.config.JevConfig</Name>
/// <Attr name="writable" />
/// <Secrets>
/// remote.api_key
/// </Secrets>
/// ```
#[qexed_config_macros::app_config("/", "jev")]
#[derive(Debug, Serialize, Deserialize, Doc)]
pub struct JevConfig {
    /// ```autodoc
    /// <Name>qexed.crates.jev.config.JevConfig.enable</Name>
    /// <Default>true</Default>
    /// <Attr name="writable" />
    /// ```
    pub enable: bool,
    /// ```autodoc
    /// <Name>qexed.crates.jev.config.JevConfig.mode</Name>
    /// <Value name="system">JevMode::default()</Value>
    /// <Default>Embedded</Default>
    /// <Attr name="writable" />
    /// ```
    pub mode: mode::JevMode,
    /// ```autodoc
    /// <Name>qexed.crates.jev.config.JevConfig.embedded</Name>
    /// <Attr name="sub" />
    /// ```
    pub embedded: embedded::JevEmbeddedConfig,
    /// ```autodoc
    /// <Name>qexed.crates.jev.config.JevConfig.remote</Name>
    /// <Attr name="sub" />
    /// ```
    pub remote: remote::JevRemoteConfig,
    /// ```autodoc
    /// <Name>qexed.crates.jev.config.JevConfig.max_history_turns</Name>
    /// <Default>8</Default>
    /// <Attr name="writable" />
    /// ```
    pub max_history_turns: usize,
    /// 内容检测策略（广告 / 号码 / 违法）。
    /// ```autodoc
    /// <Name>qexed.crates.jev.config.JevConfig.moderation</Name>
    /// <Attr name="sub" />
    /// ```
    pub moderation: crate::moderation::ModerationPolicy,
}

impl Default for JevConfig {
    fn default() -> Self {
        Self {
            enable: true,
            mode: mode::JevMode::default(),
            embedded: embedded::JevEmbeddedConfig::default(),
            remote: remote::JevRemoteConfig::default(),
            max_history_turns: 8,
            moderation: crate::moderation::ModerationPolicy::default(),
        }
    }
}

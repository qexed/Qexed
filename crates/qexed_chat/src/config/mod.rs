use qexed_doc_macros::Doc;
use serde::{Deserialize, Serialize};

// autodoc <Name> 模板：crate 名去掉 qexed_ 前缀，结构体为 PascalCase + Config。
//   qexed_mojang_data -> qexed.crates.mojang_data.config.MojangDataConfig
//   qexed_a           -> qexed.crates.a.config.AConfig
//   qexed_log         -> qexed.crates.log.config.LogConfig
/// ```autodoc
/// <Name>qexed.crates.chat.config.ChatConfig</Name>
/// <Attr name="writable" />
/// ```
#[qexed_config_macros::app_config("/", "chat")]
#[derive(Debug, Serialize, Deserialize, Doc)]
pub struct ChatConfig {
    /// ```autodoc
    /// <Name>qexed.crates.chat.config.ChatConfig.rate_limit_window_secs</Name>
    /// <Attr name="writable" />
    /// ```
    pub rate_limit_window_secs: u64,

    /// ```autodoc
    /// <Name>qexed.crates.chat.config.ChatConfig.rate_limit_max_messages</Name>
    /// <Attr name="writable" />
    /// ```
    pub rate_limit_max_messages: usize,

    /// ```autodoc
    /// <Name>qexed.crates.chat.config.ChatConfig.max_length</Name>
    /// <Attr name="writable" />
    /// ```
    pub max_length: usize,

    /// ```autodoc
    /// <Name>qexed.crates.chat.config.ChatConfig.secure_chat_enabled</Name>
    /// <Attr name="writable" />
    /// ```
    pub secure_chat_enabled: bool,
}

// 手动默认值：derive(Default) 会得到全 0（max_length=0 → 兜底 1，
// 任何消息都 "Message too long (n > 1)"）。这里给可用的开箱值。
impl Default for ChatConfig {
    fn default() -> Self {
        Self {
            rate_limit_window_secs: 10,
            rate_limit_max_messages: 20,
            max_length: 256,
            secure_chat_enabled: true,
        }
    }
}
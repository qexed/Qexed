use qexed_doc_macros::Doc;
use serde::{Deserialize, Serialize};

// autodoc <Name> 模板：crate 名去掉 qexed_ 前缀，结构体为 PascalCase + Config。
//   qexed_mojang_data -> qexed.crates.mojang_data.config.MojangDataConfig
//   qexed_a           -> qexed.crates.a.config.AConfig
//   qexed_log         -> qexed.crates.log.config.LogConfig
/// ```autodoc
/// <Name>qexed.crates.auth.config.AuthConfig</Name>
/// <Attr name="writable" />
/// ```
#[qexed_config_macros::app_config("/", "auth")]
#[derive(Debug, Serialize, Deserialize, Doc)]
pub struct AuthConfig {
    /// ```autodoc
    /// <Name>qexed.crates.auth.config.AuthConfig.session_server_url</Name>
    /// <Default>https://sessionserver.mojang.com/session/minecraft/hasJoined</Default>
    /// ```
    pub session_server_url: String,
    /// ```autodoc
    /// <Name>qexed.crates.auth.config.AuthConfig.services_public_keys_url</Name>
    /// <Default>https://api.minecraftservices.com/publickeys</Default>
    /// ```
    pub services_public_keys_url: String,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            session_server_url: "https://sessionserver.mojang.com/session/minecraft/hasJoined".to_string(),
            services_public_keys_url: "https://api.minecraftservices.com/publickeys".to_string(),
        }
    }
}
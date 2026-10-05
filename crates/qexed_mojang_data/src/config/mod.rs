use qexed_doc_macros::Doc;
use serde::{Deserialize, Serialize};

// autodoc <Name> 模板：crate 名去掉 qexed_ 前缀，结构体为 PascalCase + Config。
//   qexed_mojang_data → qexed.crates.mojang_data.config.MojangDataConfig
//   qexed_a           → qexed.crates.a.config.AConfig
//   qexed_log         → qexed.crates.log.config.LogConfig
/// ```autodoc
/// <Name>qexed.crates.mojang_data.config.MojangDataConfig</Name>
/// <Attr name="writable" />
/// ```
#[qexed_config_macros::app_config("/", "mojang_data")]
#[derive(Debug, Serialize, Deserialize, Doc)]
pub struct MojangDataConfig {
    /// ```autodoc
    /// <Name>qexed.crates.mojang_data.config.MojangDataConfig.version_manifest_url</Name>
    /// <Default>https://piston-meta.mojang.com/mc/game/version_manifest_v2.json</Default>
    /// ```
    pub version_manifest_url: String,
    /// ```autodoc
    /// <Name>qexed.crates.mojang_data.config.MojangDataConfig.http_timeout</Name>
    /// <Default>120</Default>
    /// ```
    pub http_timeout: u32,
}

impl Default for MojangDataConfig {
    fn default() -> Self {
        Self {
            version_manifest_url:
                "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json".to_string(),
            http_timeout: 120,
        }
    }
}

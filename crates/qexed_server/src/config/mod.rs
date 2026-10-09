use qexed_doc_macros::Doc;
use serde::{Deserialize, Serialize};
pub mod play;
// autodoc <Name> 模板：crate 名去掉 qexed_ 前缀，结构体为 PascalCase + Config。
//   qexed_mojang_data -> qexed.crates.mojang_data.config.MojangDataConfig
//   qexed_a           -> qexed.crates.a.config.AConfig
//   qexed_log         -> qexed.crates.log.config.LogConfig
/// ```autodoc
/// <Name>qexed.crates.server.config.ServerConfig</Name>
/// <Attr name="writable" />
/// ```
#[qexed_config_macros::app_config("/", "server")]
#[derive(Debug, Serialize, Deserialize, Doc)]
pub struct ServerConfig {
    /// ```autodoc
    /// <Name>qexed.crates.server.config.ServerConfig.bind</Name>
    /// <Attr name="writable" />
    /// ```
    pub bind: String,
    /// ```autodoc
    /// <Name>qexed.crates.server.config.ServerConfig.compression_threshold</Name>
    /// <Attr name="writable" />
    /// ```
    pub compression_threshold: i32,
    /// ```autodoc
    /// <Name>qexed.crates.server.config.ServerConfig.play</Name>
    /// <Attr name="sub" />
    /// ```
    pub play: play::PlayOptionsConfig,
    /// ```autodoc
    /// <Name>qexed.crates.server.config.ServerConfig.motd</Name>
    /// <Attr name="writable" />
    /// ```
    pub motd: String,
    /// ```autodoc
    /// <Name>qexed.crates.server.config.ServerConfig.max_players</Name>
    /// <Attr name="writable" />
    /// ```
    pub max_players: i32,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind: "0.0.0.0:25565".to_string(),
            compression_threshold: 256,
            play: play::PlayOptionsConfig::default(),
            motd: "A qexed server".to_string(),
            max_players: 20,
        }
    }
}

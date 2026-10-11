pub mod algorithm;
pub mod mode;

use qexed_doc_macros::Doc;
use serde::{Deserialize, Serialize};

use self::algorithm::LightAlgorithm;
use self::mode::LightMode;

// autodoc <Name> 模板：crate 名去掉 qexed_ 前缀，结构体为 PascalCase + Config。
/// ```autodoc
/// <Name>qexed.crates.light.config.LightConfig</Name>
/// <Attr name="writable" />
/// ```
#[qexed_config_macros::app_config("/", "light")]
#[derive(Debug, Serialize, Deserialize, Doc)]
pub struct LightConfig {
    /// ```autodoc
    /// <Name>qexed.crates.light.config.LightConfig.enable</Name>
    /// <Default>true</Default>
    /// ```
    pub enable: bool,
    /// ```autodoc
    /// <Name>qexed.crates.light.config.LightConfig.mode</Name>
    /// <Value name="system">LightMode::default()</Value>
    /// <Default>Static</Default>
    /// ```
    pub mode: LightMode,
    /// ```autodoc
    /// <Name>qexed.crates.light.config.LightConfig.algorithm</Name>
    /// <Default>Fast</Default>
    /// ```
    pub algorithm: LightAlgorithm,
    /// ```autodoc
    /// <Name>qexed.crates.light.config.LightConfig.fixed_level</Name>
    /// <Default>15</Default>
    /// ```
    pub fixed_level: u8,
}

impl Default for LightConfig {
    fn default() -> Self {
        Self {
            enable: true,
            mode: LightMode::default(),
            algorithm: LightAlgorithm::default(),
            fixed_level: 15,
        }
    }
}

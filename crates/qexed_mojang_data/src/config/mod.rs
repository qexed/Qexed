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
    /// ```autodoc
    /// <Name>qexed.crates.mojang_data.config.MojangDataConfig.java_path</Name>
    /// <Default>""</Default>
    /// ```
    /// 手动指定 java 可执行文件路径；留空时自动发现（JAVA_HOME → PATH → 自动下载 Temurin JRE）。
    #[serde(default)]
    pub java_path: String,
    /// ```autodoc
    /// <Name>qexed.crates.mojang_data.config.MojangDataConfig.datagen</Name>
    /// <Default>true</Default>
    /// ```
    /// 是否运行 Mojang 数据生成器产出 reports（registries.json / blocks.json）。
    /// 静态注册表数据不再从 assets 获取，改由 datagen 自动生成。
    #[serde(default = "default_true")]
    pub datagen: bool,
    /// ```autodoc
    /// <Name>qexed.crates.mojang_data.config.MojangDataConfig.jdk_download</Name>
    /// <Default>true</Default>
    /// ```
    /// 系统上找不到满足要求的 Java 时，是否自动从 Adoptium 下载 Temurin JRE 到缓存目录。
    #[serde(default = "default_true")]
    pub jdk_download: bool,
}

fn default_true() -> bool {
    true
}

impl Default for MojangDataConfig {
    fn default() -> Self {
        Self {
            version_manifest_url:
                "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json".to_string(),
            http_timeout: 120,
            java_path: String::new(),
            datagen: true,
            jdk_download: true,
        }
    }
}

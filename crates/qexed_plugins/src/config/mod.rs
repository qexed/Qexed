//! qexed_plugins 配置（app_config 宏定义，替代 v4 qexed_config::app::qexed::server 下的
//! Economy / PluginStructuredStorage 配置路径）。

use qexed_doc_macros::DocValue;
use serde::{Deserialize, Serialize};

/// 经济系统存储引擎。
/// v6 workspace 未引入 mysql/mongodb/redis 依赖，非 sqlite 引擎仅保留枚举位，
/// 运行时回退 sqlite 并告警（TODO(storage)）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, DocValue)]
pub enum EconomyStorageEngine {
    Sqlite,
    Mysql,
    Redis,
    Mongodb,
}

impl Default for EconomyStorageEngine {
    fn default() -> Self {
        Self::Sqlite
    }
}

impl EconomyStorageEngine {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Sqlite => "sqlite",
            Self::Mysql => "mysql",
            Self::Redis => "redis",
            Self::Mongodb => "mongodb",
        }
    }
}

/// 插件结构化存储引擎。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, DocValue)]
pub enum StructuredStorageEngine {
    Sqlite,
    Mysql,
    Mongodb,
}

impl Default for StructuredStorageEngine {
    fn default() -> Self {
        Self::Sqlite
    }
}

impl StructuredStorageEngine {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Sqlite => "sqlite",
            Self::Mysql => "mysql",
            Self::Mongodb => "mongodb",
        }
    }
}

/// 单条货币定义。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurrencyConfig {
    pub id: String,
    pub name: String,
    pub symbol: String,
    #[serde(default)]
    pub fractional_digits: i32,
    #[serde(default)]
    pub storage: EconomyStorageEngine,
}

impl Default for CurrencyConfig {
    fn default() -> Self {
        Self {
            id: "qexed:coin".to_string(),
            name: "Coin".to_string(),
            symbol: "Q".to_string(),
            fractional_digits: 2,
            storage: EconomyStorageEngine::Sqlite,
        }
    }
}
/// ```autodoc
/// <Name>qexed.crates.plugins.config.PluginsConfig</Name>
/// <Attr name="writable" />
/// ```
#[qexed_config_macros::app_config("/", "plugins")]
#[derive(Debug, Serialize, Deserialize)]
pub struct PluginsConfig {
    /// 插件目录（相对工作目录）。
    pub plugin_dir: String,
    /// 经济系统配置。
    pub economy: EconomyConfig,
    /// 插件结构化存储配置。
    pub structured_storage: StructuredStorageConfig,
}

/// 经济系统配置（v4 qexed_config::app::qexed::server::Economy 迁移）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EconomyConfig {
    pub currencies: Vec<CurrencyConfig>,
}

impl Default for EconomyConfig {
    fn default() -> Self {
        Self {
            currencies: Vec::new(),
        }
    }
}

/// 插件结构化存储配置（v4 PluginStructuredStorage 迁移）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuredStorageConfig {
    pub engine: StructuredStorageEngine,
}

impl Default for StructuredStorageConfig {
    fn default() -> Self {
        Self {
            engine: StructuredStorageEngine::Sqlite,
        }
    }
}

impl Default for PluginsConfig {
    fn default() -> Self {
        Self {
            plugin_dir: "plugins".to_string(),
            economy: EconomyConfig::default(),
            structured_storage: StructuredStorageConfig::default(),
        }
    }
}

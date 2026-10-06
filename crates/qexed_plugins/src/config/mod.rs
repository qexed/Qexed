//! qexed_plugins 配置（app_config 宏定义，替代 v4 qexed_config::app::qexed::server 下的
//! Economy / PluginStructuredStorage 配置路径）。

use qexed_doc_macros::DocValue;
use serde::{Deserialize, Serialize};

/// 经济系统存储引擎。
/// Sqlite 槽位由文件存储承载（v6 无 rusqlite）；Mysql/Mongodb/Redis 为
/// v4 economy.rs 实现的完整迁移，初始化失败时该槽位不装并告警。
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
///
/// - Sqlite：v6 无 rusqlite，语义由 FileStructuredStorage 文件后端承载；
/// - Mysql / Mongodb：v4 plugins/structured_storage.rs 的实现已迁入（见
///   structured_storage.rs）。
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
    /// mysql 存储引擎连接配置（有货币选 mysql 时使用）。
    #[serde(default)]
    pub mysql: MysqlStorageConfig,
    /// mongodb 存储引擎连接配置。
    #[serde(default)]
    pub mongodb: MongoStorageConfig,
    /// redis 存储引擎连接配置（v4 PikaConfig 的 v6 精简版）。
    #[serde(default)]
    pub redis: RedisStorageConfig,
}

impl Default for EconomyConfig {
    fn default() -> Self {
        Self {
            currencies: Vec::new(),
            mysql: MysqlStorageConfig::default(),
            mongodb: MongoStorageConfig::default(),
            redis: RedisStorageConfig::default(),
        }
    }
}

/// 插件结构化存储配置（v4 PluginStructuredStorage 迁移）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuredStorageConfig {
    pub engine: StructuredStorageEngine,
    /// mysql 引擎连接配置。
    #[serde(default)]
    pub mysql: MysqlStorageConfig,
    /// mongodb 引擎连接配置。
    #[serde(default)]
    pub mongodb: MongoStorageConfig,
}

impl Default for StructuredStorageConfig {
    fn default() -> Self {
        Self {
            engine: StructuredStorageEngine::Sqlite,
            mysql: MysqlStorageConfig::default(),
            mongodb: MongoStorageConfig::default(),
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

/// MySQL 连接配置（v4 qexed_config::public::mysql::MysqlConfig 迁移；
/// qexed_player::config 有完整版，插件侧只留连接所需字段，字段语义一致）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MysqlStorageConfig {
    #[serde(default = "default_storage_mysql_ip")]
    pub ip: String,
    #[serde(default = "default_storage_mysql_port")]
    pub port: u16,
    #[serde(default = "default_storage_mysql_user")]
    pub username: String,
    /// ```autodoc
    /// <Secret />
    /// ```
    #[serde(default = "default_storage_mysql_pass")]
    pub password: String,
    #[serde(default = "default_storage_mysql_db")]
    pub database: String,
}

fn default_storage_mysql_ip() -> String {
    "127.0.0.1".to_string()
}

fn default_storage_mysql_port() -> u16 {
    3306
}

fn default_storage_mysql_user() -> String {
    "qexed".to_string()
}

fn default_storage_mysql_pass() -> String {
    "qexed".to_string()
}

fn default_storage_mysql_db() -> String {
    "qexed".to_string()
}

impl Default for MysqlStorageConfig {
    fn default() -> Self {
        Self {
            ip: default_storage_mysql_ip(),
            port: default_storage_mysql_port(),
            username: default_storage_mysql_user(),
            password: default_storage_mysql_pass(),
            database: default_storage_mysql_db(),
        }
    }
}

impl MysqlStorageConfig {
    /// 纯逻辑校验（连接前调用，测试不依赖真实数据库）。
    pub fn validate(&self) -> Result<(), String> {
        if self.username.is_empty() {
            return Err("MySQL config error: username must not be empty".to_string());
        }
        if self.database.is_empty() {
            return Err("MySQL config error: database must not be empty".to_string());
        }
        Ok(())
    }
}

/// MongoDB 连接配置（v4 qexed_config::public::mongodb::MongoConfig 迁移，精简版）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MongoStorageConfig {
    #[serde(default = "default_storage_mongo_host")]
    pub host: String,
    #[serde(default = "default_storage_mongo_port")]
    pub port: u16,
    #[serde(default)]
    pub username: Option<String>,
    /// ```autodoc
    /// <Secret />
    /// ```
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default = "default_storage_mongo_db")]
    pub database: String,
    #[serde(default)]
    pub auth_source: Option<String>,
    /// 连接超时（毫秒）。
    #[serde(default = "default_storage_mongo_connect_timeout_ms")]
    pub connect_timeout_ms: u64,
}

fn default_storage_mongo_host() -> String {
    "127.0.0.1".to_string()
}

fn default_storage_mongo_port() -> u16 {
    27017
}

fn default_storage_mongo_db() -> String {
    "qexed".to_string()
}

fn default_storage_mongo_connect_timeout_ms() -> u64 {
    10_000
}

impl Default for MongoStorageConfig {
    fn default() -> Self {
        Self {
            host: default_storage_mongo_host(),
            port: default_storage_mongo_port(),
            username: None,
            password: None,
            database: default_storage_mongo_db(),
            auth_source: None,
            connect_timeout_ms: default_storage_mongo_connect_timeout_ms(),
        }
    }
}

impl MongoStorageConfig {
    /// 连接 URI（standalone，无 srv；与 v4 mongo_connection_uri 同构）。
    pub fn connection_uri(&self) -> String {
        let mut uri = "mongodb://".to_string();
        if let (Some(user), Some(pass)) = (&self.username, &self.password) {
            uri.push_str(&format!("{user}:{pass}@"));
        }
        uri.push_str(&format!("{}:{}/{}", self.host, self.port, self.database));
        let mut options = vec![format!("connectTimeoutMS={}", self.connect_timeout_ms)];
        if let Some(source) = &self.auth_source {
            options.push(format!("authSource={source}"));
        }
        format!("{uri}?{}", options.join("&"))
    }

    /// 纯逻辑校验。
    pub fn validate(&self) -> Result<(), String> {
        if self.database.is_empty() {
            return Err("MongoDB config error: database must not be empty".to_string());
        }
        match (&self.username, &self.password) {
            (Some(_), None) => {
                return Err("MongoDB config error: password is required with username".to_string())
            }
            (None, Some(_)) => {
                return Err("MongoDB config error: username is required with password".to_string())
            }
            _ => {}
        }
        Ok(())
    }
}

/// Redis 连接配置（v4 qexed_config::public::pika::PikaConfig 的 v6 精简版：
/// 只保留 standalone 单节点 + db + 密码，sentinel/cluster 不再单列枚举，
/// 多节点场景直接写 nodes 由运维侧代理）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RedisStorageConfig {
    #[serde(default = "default_storage_redis_host")]
    pub host: String,
    #[serde(default = "default_storage_redis_port")]
    pub port: u16,
    /// ```autodoc
    /// <Secret />
    /// ```
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub database: i64,
}

fn default_storage_redis_host() -> String {
    "127.0.0.1".to_string()
}

fn default_storage_redis_port() -> u16 {
    6379
}

impl Default for RedisStorageConfig {
    fn default() -> Self {
        Self {
            host: default_storage_redis_host(),
            port: default_storage_redis_port(),
            password: None,
            database: 0,
        }
    }
}

impl RedisStorageConfig {
    /// 连接 URL（redis://[:pass@]host:port/dbn）。
    pub fn connection_url(&self) -> String {
        let mut url = "redis://".to_string();
        if let Some(pass) = &self.password {
            url.push_str(&format!(":{pass}@"));
        }
        url.push_str(&format!("{}:{}", self.host, self.port));
        if self.database != 0 {
            url.push_str(&format!("/{}", self.database));
        }
        url
    }

    /// 纯逻辑校验。
    pub fn validate(&self) -> Result<(), String> {
        if self.port == 0 {
            return Err("Redis config error: port must not be 0".to_string());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn storage_config_validate_is_pure_logic() {
        assert!(MysqlStorageConfig::default().validate().is_ok());
        assert!(MongoStorageConfig::default().validate().is_ok());
        assert!(RedisStorageConfig::default().validate().is_ok());

        let mut mysql = MysqlStorageConfig::default();
        mysql.username.clear();
        assert!(mysql.validate().is_err());

        let mut mongo = MongoStorageConfig::default();
        mongo.username = Some("qexed".to_string());
        assert!(mongo.validate().is_err());

        let mut redis = RedisStorageConfig::default();
        redis.port = 0;
        assert!(redis.validate().is_err());
    }

    #[test]
    fn mongo_storage_connection_uri_shape() {
        let config = MongoStorageConfig::default();
        assert_eq!(
            config.connection_uri(),
            "mongodb://127.0.0.1:27017/qexed?connectTimeoutMS=10000"
        );
    }

    #[test]
    fn redis_storage_connection_url_shape() {
        let mut config = RedisStorageConfig {
            password: Some("secret".to_string()),
            database: 2,
            ..Default::default()
        };
        assert_eq!(config.connection_url(), "redis://:secret@127.0.0.1:6379/2");
        config.database = 0;
        assert_eq!(config.connection_url(), "redis://:secret@127.0.0.1:6379");
    }
}

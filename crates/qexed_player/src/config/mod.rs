use qexed_doc_macros::DocValue;
use serde::{Deserialize, Serialize};

/// 玩家数据存储引擎。
/// v4 对应 qexed_config::app::qexed::server::PlayerDataEngine。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, DocValue)]
#[serde(rename_all = "snake_case")]
pub enum PlayerDataEngine {
    Vanilla,
    Mongodb,
    Mysql,
}

impl Default for PlayerDataEngine {
    fn default() -> Self {
        Self::Vanilla
    }
}

/// 玩家数据持久化配置（v4 对应 qexed_config::app::qexed::server::PlayerData）。
/// mongodb/mysql 连接参数（MongoConfig/MysqlConfig，v4 在 qexed_config::public）
/// 随 storage 依赖一并迁入本模块。
///
/// ```autodoc
/// <Name>qexed.crates.player.config.PlayerDataConfig</Name>
/// <Attr name="writable" />
/// ```
#[qexed_config_macros::app_config("/", "player_data")]
#[derive(Debug, Serialize, Deserialize)]
pub struct PlayerDataConfig {
    /// ```autodoc
    /// <Name>qexed.crates.player.config.PlayerDataConfig.enable</Name>
    /// <Default>true</Default>
    /// ```
    pub enable: bool,
    /// ```autodoc
    /// <Name>qexed.crates.player.config.PlayerDataConfig.autosave_interval_secs</Name>
    /// <Default>120</Default>
    /// ```
    pub autosave_interval_secs: u64,
    /// ```autodoc
    /// <Name>qexed.crates.player.config.PlayerDataConfig.engine</Name>
    /// <Default>Vanilla</Default>
    /// ```
    pub engine: PlayerDataEngine,
    /// mongodb 集合名。
    /// ```autodoc
    /// <Name>qexed.crates.player.config.PlayerDataConfig.collection</Name>
    /// <Default>qexed_players</Default>
    /// ```
    pub collection: String,
    /// mysql 表名。
    /// ```autodoc
    /// <Name>qexed.crates.player.config.PlayerDataConfig.table</Name>
    /// <Default>qexed_player_data</Default>
    /// ```
    pub table: String,
    /// MongoDB 连接配置（engine = mongodb 时使用）。
    /// ```autodoc
    /// <Name>qexed.crates.player.config.PlayerDataConfig.mongodb</Name>
    /// <Default>127.0.0.1:27017/qexed</Default>
    /// ```
    #[serde(default)]
    pub mongodb: MongoConfig,
    /// MySQL 连接配置（engine = mysql 时使用）。
    /// ```autodoc
    /// <Name>qexed.crates.player.config.PlayerDataConfig.mysql</Name>
    /// <Default>127.0.0.1:3306/qexed</Default>
    /// ```
    #[serde(default)]
    pub mysql: MysqlConfig,
}

impl Default for PlayerDataConfig {
    fn default() -> Self {
        Self {
            enable: true,
            autosave_interval_secs: 120,
            engine: PlayerDataEngine::default(),
            collection: "qexed_players".to_string(),
            table: "qexed_player_data".to_string(),
            mongodb: MongoConfig::default(),
            mysql: MysqlConfig::default(),
        }
    }
}

/// MongoDB 连接配置（v4 qexed_config::public::mongodb::MongoConfig 迁移；
/// humantime_serde 依赖以毫秒数直存替代，字段语义一致）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MongoConfig {
    #[serde(default = "default_mongo_host")]
    pub host: String,
    #[serde(default = "default_mongo_port")]
    pub port: u16,
    #[serde(default)]
    pub username: Option<String>,
    /// ```autodoc
    /// <Secret />
    /// ```
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub database: String,
    #[serde(default = "default_mongo_app_name")]
    pub app_name: Option<String>,
    #[serde(default)]
    pub replica_set: Option<String>,
    #[serde(default)]
    pub auth_source: Option<String>,
    #[serde(default)]
    pub use_tls: bool,
    /// 连接超时（毫秒）。
    #[serde(default = "default_mongo_connect_timeout_ms")]
    pub connect_timeout_ms: u64,
    /// 套接字超时（毫秒）。
    #[serde(default = "default_mongo_socket_timeout_ms")]
    pub socket_timeout_ms: u64,
    #[serde(default = "default_mongo_max_pool_size")]
    pub max_pool_size: u32,
    #[serde(default = "default_mongo_min_pool_size")]
    pub min_pool_size: u32,
    /// 最大空闲时间（毫秒），None 用驱动默认。
    #[serde(default)]
    pub max_idle_time_ms: Option<u64>,
}

fn default_mongo_host() -> String {
    "127.0.0.1".to_string()
}

fn default_mongo_port() -> u16 {
    27017
}

fn default_mongo_app_name() -> Option<String> {
    Some("qexed".to_string())
}

fn default_mongo_connect_timeout_ms() -> u64 {
    10_000
}

fn default_mongo_socket_timeout_ms() -> u64 {
    5_000
}

fn default_mongo_max_pool_size() -> u32 {
    100
}

fn default_mongo_min_pool_size() -> u32 {
    0
}

impl Default for MongoConfig {
    fn default() -> Self {
        Self {
            host: default_mongo_host(),
            port: default_mongo_port(),
            username: None,
            password: None,
            database: "qexed".to_string(),
            app_name: default_mongo_app_name(),
            replica_set: None,
            auth_source: None,
            use_tls: false,
            connect_timeout_ms: default_mongo_connect_timeout_ms(),
            socket_timeout_ms: default_mongo_socket_timeout_ms(),
            max_pool_size: default_mongo_max_pool_size(),
            min_pool_size: default_mongo_min_pool_size(),
            max_idle_time_ms: Some(60_000),
        }
    }
}

impl MongoConfig {
    /// 连接 URI（与 v4 connection_uri 同构，超时/池参数拼为 URI 选项）。
    pub fn connection_uri(&self) -> String {
        let mut uri = if self.use_tls {
            "mongodb+srv://".to_string()
        } else {
            "mongodb://".to_string()
        };
        if let (Some(user), Some(pass)) = (&self.username, &self.password) {
            uri.push_str(&format!("{user}:{pass}@"));
        }
        uri.push_str(&format!("{}:{}/{}", self.host, self.port, self.database));
        let mut options = Vec::new();
        if let Some(name) = &self.app_name {
            options.push(format!("appName={name}"));
        }
        if let Some(rs) = &self.replica_set {
            options.push(format!("replicaSet={rs}"));
        }
        if let Some(source) = &self.auth_source {
            options.push(format!("authSource={source}"));
        }
        options.push(format!("connectTimeoutMS={}", self.connect_timeout_ms));
        options.push(format!("socketTimeoutMS={}", self.socket_timeout_ms));
        options.push(format!("maxPoolSize={}", self.max_pool_size));
        options.push(format!("minPoolSize={}", self.min_pool_size));
        if let Some(idle) = self.max_idle_time_ms {
            options.push(format!("maxIdleTimeMS={idle}"));
        }
        if self.use_tls {
            options.push("tls=true".to_string());
        }
        if options.is_empty() {
            uri
        } else {
            format!("{uri}?{}", options.join("&"))
        }
    }

    /// 纯逻辑校验（连接前调用，测试不依赖真实数据库）。
    pub fn validate(&self) -> Result<(), String> {
        if self.database.is_empty() {
            return Err("MongoDB config error: database must not be empty".to_string());
        }
        if self.max_pool_size < self.min_pool_size {
            return Err(
                "MongoDB config error: max_pool_size must be at least min_pool_size".to_string(),
            );
        }
        match (&self.username, &self.password) {
            (Some(_), None) => {
                return Err(
                    "MongoDB config error: password is required with username".to_string(),
                )
            }
            (None, Some(_)) => {
                return Err(
                    "MongoDB config error: username is required with password".to_string(),
                )
            }
            _ => {}
        }
        Ok(())
    }
}

/// MySQL 连接配置（v4 qexed_config::public::mysql::MysqlConfig 迁移）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MysqlConfig {
    #[serde(default = "default_mysql_ip")]
    pub ip: String,
    #[serde(default = "default_mysql_port")]
    pub port: u16,
    pub username: String,
    /// ```autodoc
    /// <Secret />
    /// ```
    pub password: String,
    pub database: String,
    #[serde(default = "default_mysql_pool_max_size")]
    pub pool_max_size: u32,
    #[serde(default = "default_mysql_pool_min_idle")]
    pub pool_min_idle: u32,
    /// 连接超时（毫秒）。
    #[serde(default = "default_mysql_connect_timeout_ms")]
    pub connect_timeout_ms: u64,
    /// 空闲超时（毫秒），None 用驱动默认。
    #[serde(default)]
    pub idle_timeout_ms: Option<u64>,
    #[serde(default)]
    pub use_ssl: bool,
    #[serde(default = "default_mysql_charset")]
    pub charset: String,
}

fn default_mysql_ip() -> String {
    "127.0.0.1".to_string()
}

fn default_mysql_port() -> u16 {
    3306
}

fn default_mysql_pool_max_size() -> u32 {
    10
}

fn default_mysql_pool_min_idle() -> u32 {
    2
}

fn default_mysql_connect_timeout_ms() -> u64 {
    30_000
}

fn default_mysql_charset() -> String {
    "utf8mb4".to_string()
}

impl Default for MysqlConfig {
    fn default() -> Self {
        Self {
            ip: default_mysql_ip(),
            port: default_mysql_port(),
            username: "qexed".to_string(),
            password: String::new(),
            database: "qexed".to_string(),
            pool_max_size: default_mysql_pool_max_size(),
            pool_min_idle: default_mysql_pool_min_idle(),
            connect_timeout_ms: default_mysql_connect_timeout_ms(),
            idle_timeout_ms: Some(300_000),
            use_ssl: false,
            charset: default_mysql_charset(),
        }
    }
}

impl MysqlConfig {
    /// 连接字符串（与 v4 connection_string 同构）。
    pub fn connection_string(&self) -> String {
        let ssl_flag = if self.use_ssl {
            "require"
        } else {
            "prefer"
        };
        format!(
            "mysql://{}:{}@{}:{}/{}?charset={}&ssl={}",
            self.username, self.password, self.ip, self.port, self.database, self.charset, ssl_flag
        )
    }

    /// 直连参数（mysql_async OptsBuilder 用，绕开 URL 转义问题）。
    pub fn opts_fields(&self) -> (&str, u16, &str, &str, &str) {
        (
            &self.ip,
            self.port,
            &self.username,
            &self.password,
            &self.database,
        )
    }

    /// 纯逻辑校验（连接前调用，测试不依赖真实数据库）。
    pub fn validate(&self) -> Result<(), String> {
        if self.username.is_empty() {
            return Err("MySQL config error: username must not be empty".to_string());
        }
        if self.database.is_empty() {
            return Err("MySQL config error: database must not be empty".to_string());
        }
        if self.pool_max_size < self.pool_min_idle {
            return Err(
                "MySQL config error: pool_max_size must be at least pool_min_idle".to_string(),
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mongo_config_validate_is_pure_logic() {
        let mut config = MongoConfig::default();
        assert!(config.validate().is_ok());

        config.database.clear();
        assert!(config.validate().is_err());

        let mut unbalanced = MongoConfig::default();
        unbalanced.username = Some("qexed".to_string());
        assert!(unbalanced.validate().is_err());
    }

    #[test]
    fn mongo_connection_uri_includes_pool_and_timeout_options() {
        let config = MongoConfig::default();
        let uri = config.connection_uri();
        assert!(uri.starts_with("mongodb://127.0.0.1:27017/qexed"));
        assert!(uri.contains("maxPoolSize=100"));
        assert!(uri.contains("connectTimeoutMS=10000"));
    }

    #[test]
    fn mysql_config_validate_is_pure_logic() {
        let mut config = MysqlConfig::default();
        assert!(config.validate().is_ok());

        config.username.clear();
        assert!(config.validate().is_err());

        let mut inverted = MysqlConfig::default();
        inverted.pool_min_idle = 32;
        assert!(inverted.validate().is_err());
    }

    #[test]
    fn mysql_connection_string_shape_matches_v4() {
        let config = MysqlConfig::default();
        assert_eq!(
            config.connection_string(),
            "mysql://qexed:@127.0.0.1:3306/qexed?charset=utf8mb4&ssl=prefer"
        );
    }
}

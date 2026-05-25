use qexed_config_macros::AutoDoc;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, AutoDoc)]
pub struct MysqlConfig {
    #[serde(default = "default_ip")]
    #[AutoDoc(key = "config.public.mysql.ip")]
    pub ip: String,

    #[serde(default = "default_port")]
    #[AutoDoc(key = "config.public.mysql.port")]
    pub port: u16,

    #[AutoDoc(key = "config.public.mysql.username")]
    pub username: String,

    #[AutoDoc(
        key = "config.public.mysql.password",
        sensitive,
        default_display = "<stored in .secrets>"
    )]
    pub password: String,

    #[AutoDoc(key = "config.public.mysql.database")]
    pub database: String,

    #[serde(default = "default_pool_max_size")]
    #[AutoDoc(key = "config.public.mysql.pool_max_size")]
    pub pool_max_size: u32,

    #[serde(default = "default_pool_min_idle")]
    #[AutoDoc(key = "config.public.mysql.pool_min_idle")]
    pub pool_min_idle: u32,

    #[serde(with = "humantime_serde", default = "default_connection_timeout")]
    #[AutoDoc(key = "config.public.mysql.connection_timeout")]
    pub connection_timeout: Duration,

    #[serde(with = "humantime_serde", default = "default_idle_timeout")]
    #[AutoDoc(key = "config.public.mysql.idle_timeout")]
    pub idle_timeout: Option<Duration>,

    #[serde(default)]
    #[AutoDoc(key = "config.public.mysql.use_ssl")]
    pub use_ssl: bool,

    #[serde(default = "default_charset")]
    #[AutoDoc(key = "config.public.mysql.charset")]
    pub charset: String,

    #[serde(default)]
    #[AutoDoc(key = "config.public.mysql.options")]
    pub options: Vec<(String, String)>,
}

fn default_ip() -> String {
    "127.0.0.1".to_string()
}

fn default_port() -> u16 {
    3306
}

fn default_pool_max_size() -> u32 {
    10
}

fn default_pool_min_idle() -> u32 {
    2
}

fn default_connection_timeout() -> Duration {
    Duration::from_secs(30)
}

fn default_idle_timeout() -> Option<Duration> {
    Some(Duration::from_secs(300))
}

fn default_charset() -> String {
    "utf8mb4".to_string()
}

fn default_password() -> String {
    nanoid::nanoid!()
}

impl Default for MysqlConfig {
    fn default() -> Self {
        Self {
            ip: default_ip(),
            port: default_port(),
            username: String::new(),
            password: default_password(),
            database: String::new(),
            pool_max_size: default_pool_max_size(),
            pool_min_idle: default_pool_min_idle(),
            connection_timeout: default_connection_timeout(),
            idle_timeout: default_idle_timeout(),
            use_ssl: false,
            charset: default_charset(),
            options: Vec::new(),
        }
    }
}

impl MysqlConfig {
    pub fn connection_string(&self) -> String {
        let ssl_flag = if self.use_ssl { "require" } else { "prefer" };
        format!(
            "mysql://{}:{}@{}:{}/{}?charset={}&ssl={}",
            self.username, self.password, self.ip, self.port, self.database, self.charset, ssl_flag
        )
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.username.is_empty() {
            return Err("用户名不能为空".to_string());
        }
        if self.database.is_empty() {
            return Err("数据库名不能为空".to_string());
        }
        if self.pool_max_size < self.pool_min_idle {
            return Err("连接池最大大小不能小于最小空闲连接数".to_string());
        }
        Ok(())
    }
}

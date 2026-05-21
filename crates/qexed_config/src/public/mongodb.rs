use qexed_config_macros::AutoDoc;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, AutoDoc)]
pub struct MongoConfig {
    #[serde(default = "default_host")]
    #[AutoDoc(key = "config.public.mongodb.host")]
    pub host: String,

    #[serde(default = "default_mongo_port")]
    #[AutoDoc(key = "config.public.mongodb.port")]
    pub port: u16,

    #[serde(default)]
    #[AutoDoc(key = "config.public.mongodb.username")]
    pub username: Option<String>,

    #[serde(default)]
    #[AutoDoc(key = "config.public.mongodb.password")]
    pub password: Option<String>,

    #[serde(default)]
    #[AutoDoc(key = "config.public.mongodb.database")]
    pub database: String,

    #[serde(default = "default_app_name")]
    #[AutoDoc(key = "config.public.mongodb.app_name")]
    pub app_name: Option<String>,

    #[serde(default = "default_replica_set")]
    #[AutoDoc(key = "config.public.mongodb.replica_set")]
    pub replica_set: Option<String>,

    #[serde(default)]
    #[AutoDoc(key = "config.public.mongodb.auth_source")]
    pub auth_source: Option<String>,

    #[serde(default)]
    #[AutoDoc(key = "config.public.mongodb.use_tls")]
    pub use_tls: bool,

    #[serde(with = "humantime_serde", default = "default_connect_timeout_ms")]
    #[AutoDoc(key = "config.public.mongodb.connect_timeout")]
    pub connect_timeout: Duration,

    #[serde(with = "humantime_serde", default = "default_socket_timeout_ms")]
    #[AutoDoc(key = "config.public.mongodb.socket_timeout")]
    pub socket_timeout: Duration,

    #[serde(default = "default_max_pool_size")]
    #[AutoDoc(key = "config.public.mongodb.max_pool_size")]
    pub max_pool_size: u32,

    #[serde(default = "default_min_pool_size")]
    #[AutoDoc(key = "config.public.mongodb.min_pool_size")]
    pub min_pool_size: u32,

    #[serde(with = "humantime_serde", default = "default_max_idle_time_ms")]
    #[AutoDoc(key = "config.public.mongodb.max_idle_time")]
    pub max_idle_time: Option<Duration>,
}

fn default_host() -> String {
    "127.0.0.1".to_string()
}

fn default_mongo_port() -> u16 {
    27017
}

fn default_app_name() -> Option<String> {
    Some("my_rust_app".to_string())
}

fn default_replica_set() -> Option<String> {
    None
}

fn default_connect_timeout_ms() -> Duration {
    Duration::from_millis(10000)
}

fn default_socket_timeout_ms() -> Duration {
    Duration::from_millis(5000)
}

fn default_max_pool_size() -> u32 {
    100
}

fn default_min_pool_size() -> u32 {
    0
}

fn default_max_idle_time_ms() -> Option<Duration> {
    Some(Duration::from_secs(60))
}

impl Default for MongoConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_mongo_port(),
            username: None,
            password: None,
            database: String::new(),
            app_name: default_app_name(),
            replica_set: default_replica_set(),
            auth_source: None,
            use_tls: false,
            connect_timeout: default_connect_timeout_ms(),
            socket_timeout: default_socket_timeout_ms(),
            max_pool_size: default_max_pool_size(),
            min_pool_size: default_min_pool_size(),
            max_idle_time: default_max_idle_time_ms(),
        }
    }
}

impl MongoConfig {
    pub fn connection_uri(&self) -> String {
        let mut uri = if self.use_tls {
            "mongodb+srv://".to_string()
        } else {
            "mongodb://".to_string()
        };

        if let (Some(user), Some(pass)) = (&self.username, &self.password) {
            uri.push_str(&format!("{}:{}@", user, pass));
        }

        uri.push_str(&format!("{}:{}", self.host, self.port));
        uri.push_str(&format!("/{}?", self.database));

        let mut options = Vec::new();
        if let Some(name) = &self.app_name {
            options.push(format!("appName={}", name));
        }
        if let Some(rs) = &self.replica_set {
            options.push(format!("replicaSet={}", rs));
        }
        if let Some(source) = &self.auth_source {
            options.push(format!("authSource={}", source));
        } else if !self.database.is_empty() {
            options.push(format!("authSource={}", self.database));
        }
        options.push(format!(
            "connectTimeoutMS={}",
            self.connect_timeout.as_millis()
        ));
        options.push(format!(
            "socketTimeoutMS={}",
            self.socket_timeout.as_millis()
        ));
        options.push(format!("maxPoolSize={}", self.max_pool_size));
        options.push(format!("minPoolSize={}", self.min_pool_size));
        if let Some(idle) = self.max_idle_time {
            options.push(format!("maxIdleTimeMS={}", idle.as_millis()));
        }
        if self.use_tls {
            options.push("tls=true".to_string());
        }

        uri.push_str(&options.join("&"));
        uri
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.database.is_empty() {
            return Err("MongoDB 配置错误：数据库名不能为空".to_string());
        }
        if self.max_pool_size < self.min_pool_size {
            return Err("MongoDB 配置错误：最大连接池大小不能小于最小连接池大小".to_string());
        }
        match (&self.username, &self.password) {
            (Some(_), None) => return Err("MongoDB 配置错误：提供了用户名但未提供密码".to_string()),
            (None, Some(_)) => return Err("MongoDB 配置错误：提供了密码但未提供用户名".to_string()),
            _ => {}
        }
        Ok(())
    }
}

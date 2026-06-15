use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct MongoConfig {
    #[serde(default = "default_host")]
    pub host: String,

    #[serde(default = "default_mongo_port")]
    pub port: u16,

    #[serde(default)]
    pub username: Option<String>,

    #[serde(default)]
    pub password: Option<String>,

    #[serde(default)]
    pub database: String,

    #[serde(default = "default_app_name")]
    pub app_name: Option<String>,

    #[serde(default = "default_replica_set")]
    pub replica_set: Option<String>,

    #[serde(default)]
    pub auth_source: Option<String>,

    #[serde(default)]
    pub use_tls: bool,

    #[serde(with = "humantime_serde", default = "default_connect_timeout_ms")]
    pub connect_timeout: Duration,

    #[serde(with = "humantime_serde", default = "default_socket_timeout_ms")]
    pub socket_timeout: Duration,

    #[serde(default = "default_max_pool_size")]
    pub max_pool_size: u32,

    #[serde(default = "default_min_pool_size")]
    pub min_pool_size: u32,

    #[serde(with = "humantime_serde", default = "default_max_idle_time_ms")]
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

fn default_username() -> Option<String> {
    Some("qexed".to_string())
}

fn default_password() -> Option<String> {
    Some(nanoid::nanoid!())
}

impl Default for MongoConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_mongo_port(),
            username: default_username(),
            password: default_password(),
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
            return Err("MongoDB config error: database must not be empty".to_string());
        }
        if self.max_pool_size < self.min_pool_size {
            return Err(
                "MongoDB config error: max_pool_size must be at least min_pool_size".to_string(),
            );
        }
        match (&self.username, &self.password) {
            (Some(_), None) => {
                return Err("MongoDB config error: password is required with username".to_string());
            }
            (None, Some(_)) => {
                return Err("MongoDB config error: username is required with password".to_string());
            }
            _ => {}
        }
        Ok(())
    }
}

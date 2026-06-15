use serde::{Deserialize, Serialize};
use std::{hash::Hash, time::Duration};

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, Hash)]
pub struct PikaConfig {
    #[serde(default)]
    pub mode: ConnectionMode,

    #[serde(default = "default_redis_host")]
    pub host: String,

    #[serde(default = "default_redis_port")]
    pub port: u16,

    #[serde(default)]
    pub password: Option<String>,

    #[serde(default)]
    pub database: i64,

    #[serde(default = "default_pool_size")]
    pub pool_max_size: u32,

    #[serde(default = "default_pool_idle_size")]
    pub pool_min_idle: u32,

    #[serde(with = "humantime_serde", default = "default_timeout_secs")]
    pub timeout: Duration,

    #[serde(with = "humantime_serde", default = "default_connection_timeout_secs")]
    pub connection_timeout: Duration,

    #[serde(default)]
    pub master_name: Option<String>,

    #[serde(default)]
    pub nodes: Vec<String>,

    #[serde(default)]
    pub use_tls: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, Hash, Default)]
pub enum ConnectionMode {
    #[default]
    Standalone,
    Sentinel,
    Cluster,
}

fn default_redis_host() -> String {
    "127.0.0.1".to_string()
}

fn default_redis_port() -> u16 {
    9221
}

fn default_pool_size() -> u32 {
    10
}

fn default_pool_idle_size() -> u32 {
    2
}

fn default_timeout_secs() -> Duration {
    Duration::from_secs(5)
}

fn default_connection_timeout_secs() -> Duration {
    Duration::from_secs(1)
}

fn default_password() -> Option<String> {
    Some(nanoid::nanoid!())
}

impl Default for PikaConfig {
    fn default() -> Self {
        Self {
            mode: ConnectionMode::Standalone,
            host: default_redis_host(),
            port: default_redis_port(),
            password: default_password(),
            database: 0,
            pool_max_size: default_pool_size(),
            pool_min_idle: default_pool_idle_size(),
            timeout: default_timeout_secs(),
            connection_timeout: default_connection_timeout_secs(),
            master_name: None,
            nodes: Vec::new(),
            use_tls: false,
        }
    }
}

impl PikaConfig {
    pub fn connection_params(&self) -> String {
        match self.mode {
            ConnectionMode::Standalone => {
                let protocol = if self.use_tls { "rediss" } else { "redis" };
                format!("{}://{}:{}", protocol, self.host, self.port)
            }
            ConnectionMode::Sentinel => {
                let mut params = "redis+sentinel://".to_string();
                if let Some(pass) = &self.password {
                    params.push_str(&format!(":{}@", pass));
                }
                if !self.nodes.is_empty() {
                    params.push_str(&self.nodes.join(","));
                } else {
                    params.push_str(&format!("{}:{}", self.host, 26379));
                }
                if let Some(name) = &self.master_name {
                    params.push_str(&format!("/{}", name));
                }
                if self.database != 0 {
                    params.push_str(&format!("?db={}", self.database));
                }
                params
            }
            ConnectionMode::Cluster => {
                if self.nodes.is_empty() {
                    format!("redis://{}:{}", self.host, self.port)
                } else {
                    let protocol = if self.use_tls { "rediss" } else { "redis" };
                    format!("{}://{}", protocol, self.nodes.join(","))
                }
            }
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.port == 0 {
            return Err("Pika 閰嶇疆閿欒锛氱鍙ｅ彿鏃犳晥".to_string());
        }

        if self.pool_max_size < self.pool_min_idle {
            return Err(
                "Pika 閰嶇疆閿欒锛氳繛鎺ユ睜鏈€澶уぇ灏忎笉鑳藉皬浜庢渶灏忕┖闂茶繛鎺ユ暟"
                    .to_string(),
            );
        }

        match self.mode {
            ConnectionMode::Sentinel => {
                if self.master_name.is_none() {
                    return Err(
                        "Pika 閰嶇疆閿欒锛氬摠鍏垫ā寮忓繀椤绘寚瀹?'master_name'".to_string()
                    );
                }
            }
            ConnectionMode::Cluster => {
                if self.nodes.is_empty() {
                    log::warn!("Pika 闆嗙兢妯″紡寤鸿鎻愪緵澶氫釜鑺傜偣鍦板潃");
                }
            }
            _ => {}
        }

        Ok(())
    }
}

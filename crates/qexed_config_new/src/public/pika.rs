use qexed_config_macros::AutoDoc;
use serde::{Deserialize, Serialize};
use std::{hash::Hash, time::Duration};

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, Hash, AutoDoc)]
pub struct PikaConfig {
    #[serde(default)]
    #[AutoDoc(key = "config.public.pika.mode")]
    pub mode: ConnectionMode,

    #[serde(default = "default_redis_host")]
    #[AutoDoc(key = "config.public.pika.host")]
    pub host: String,

    #[serde(default = "default_redis_port")]
    #[AutoDoc(key = "config.public.pika.port")]
    pub port: u16,

    #[serde(default)]
    #[AutoDoc(key = "config.public.pika.password")]
    pub password: Option<String>,

    #[serde(default)]
    #[AutoDoc(key = "config.public.pika.database")]
    pub database: i64,

    #[serde(default = "default_pool_size")]
    #[AutoDoc(key = "config.public.pika.pool_max_size")]
    pub pool_max_size: u32,

    #[serde(default = "default_pool_idle_size")]
    #[AutoDoc(key = "config.public.pika.pool_min_idle")]
    pub pool_min_idle: u32,

    #[serde(with = "humantime_serde", default = "default_timeout_secs")]
    #[AutoDoc(key = "config.public.pika.timeout")]
    pub timeout: Duration,

    #[serde(with = "humantime_serde", default = "default_connection_timeout_secs")]
    #[AutoDoc(key = "config.public.pika.connection_timeout")]
    pub connection_timeout: Duration,

    #[serde(default)]
    #[AutoDoc(key = "config.public.pika.master_name")]
    pub master_name: Option<String>,

    #[serde(default)]
    #[AutoDoc(key = "config.public.pika.nodes")]
    pub nodes: Vec<String>,

    #[serde(default)]
    #[AutoDoc(key = "config.public.pika.use_tls")]
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

impl Default for PikaConfig {
    fn default() -> Self {
        Self {
            mode: ConnectionMode::Standalone,
            host: default_redis_host(),
            port: default_redis_port(),
            password: None,
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
            return Err("Pika 配置错误：端口号无效".to_string());
        }

        if self.pool_max_size < self.pool_min_idle {
            return Err("Pika 配置错误：连接池最大大小不能小于最小空闲连接数".to_string());
        }

        match self.mode {
            ConnectionMode::Sentinel => {
                if self.master_name.is_none() {
                    return Err("Pika 配置错误：哨兵模式必须指定 'master_name'".to_string());
                }
            }
            ConnectionMode::Cluster => {
                if self.nodes.is_empty() {
                    log::warn!("Pika 集群模式建议提供多个节点地址");
                }
            }
            _ => {}
        }

        Ok(())
    }
}

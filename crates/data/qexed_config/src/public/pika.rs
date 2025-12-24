use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Pika (Redis协议兼容) 配置
/// 支持单节点、哨兵和集群模式（通过`mode`字段指定）
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct PikaConfig {
    // 连接模式
    #[serde(default)]
    pub mode: ConnectionMode,
    
    // 基础连接信息 (用于单节点或集群节点发现)
    #[serde(default = "default_redis_host")]
    pub host: String,
    
    #[serde(default = "default_redis_port")]
    pub port: u16,
    
    // 认证
    #[serde(default)]
    pub password: Option<String>, // 序列化时通常不跳过，因为Redis配置常以明文存储
    
    #[serde(default)]
    pub database: i64, // Redis/Pika 的数据库索引，默认为 0
    
    // 连接池与超时设置
    #[serde(default = "default_pool_size")]
    pub pool_max_size: u32,
    
    #[serde(default = "default_pool_idle_size")]
    pub pool_min_idle: u32,
    
    #[serde(with = "humantime_serde", default = "default_timeout_secs")]
    pub timeout: Duration,
    
    #[serde(with = "humantime_serde", default = "default_connection_timeout_secs")]
    pub connection_timeout: Duration,
    
    // 哨兵模式专用
    #[serde(default)]
    pub master_name: Option<String>,
    
    // 哨兵或集群节点列表 (host:port 格式)
    #[serde(default)]
    pub nodes: Vec<String>,
    
    // TLS
    #[serde(default)]
    pub use_tls: bool,
}

/// 连接模式
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Default)]
pub enum ConnectionMode {
    #[default]
    Standalone, // 单节点
    Sentinel,   // 哨兵模式
    Cluster,    // 集群模式
}

// 默认值函数
fn default_redis_host() -> String { "127.0.0.1".to_string() }
fn default_redis_port() -> u16 { 9221 } // Pika 默认端口，Redis 通常是 6379
fn default_pool_size() -> u32 { 10 }
fn default_pool_idle_size() -> u32 { 2 }
fn default_timeout_secs() -> Duration { Duration::from_secs(5) }
fn default_connection_timeout_secs() -> Duration { Duration::from_secs(1) }

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
    /// 生成适合 redis-rs 库的连接参数字符串
    pub fn connection_params(&self) -> String {
        match self.mode {
            ConnectionMode::Standalone => {
                let protocol = if self.use_tls { "rediss" } else { "redis" };
                format!(
                    "{}://{}:{}",
                    protocol,
                    self.host,
                    self.port
                )
            }
            ConnectionMode::Sentinel => {
                // 哨兵模式连接字符串格式
                let mut params = format!("redis+sentinel://");
                if let Some(pass) = &self.password {
                    params.push_str(&format!(":{}@", pass));
                }
                // 哨兵节点
                if !self.nodes.is_empty() {
                    params.push_str(&self.nodes.join(","));
                } else {
                    params.push_str(&format!("{}:{}", self.host, 26379)); // 哨兵默认端口
                }
                // 主节点名称
                if let Some(name) = &self.master_name {
                    params.push_str(&format!("/{}", name));
                }
                // 数据库索引
                if self.database != 0 {
                    params.push_str(&format!("?db={}", self.database));
                }
                params
            }
            ConnectionMode::Cluster => {
                // 集群模式：节点列表是必须的
                if self.nodes.is_empty() {
                    format!("redis://{}:{}", self.host, self.port)
                } else {
                    let protocol = if self.use_tls { "rediss" } else { "redis" };
                    format!(
                        "{}://{}",
                        protocol,
                        self.nodes.join(",")
                    )
                }
            }
        }
    }
    
    pub fn validate(&self) -> Result<(), String> {
        // 验证端口范围
        if self.port == 0{
            return Err("Pika 配置错误：端口号无效".to_string());
        }
        
        // 验证连接池设置
        if self.pool_max_size < self.pool_min_idle {
            return Err("Pika 配置错误：连接池最大大小不能小于最小空闲连接数".to_string());
        }
        
        // 模式特定验证
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
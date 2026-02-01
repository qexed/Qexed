use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct MongoConfig {
    // 基础连接信息
    #[serde(default = "default_host")]
    pub host: String,
    
    #[serde(default = "default_mongo_port")]
    pub port: u16,
    
    #[serde(default)]
    pub username: Option<String>, // 可选认证
    
    #[serde(default)]
    pub password: Option<String>, // 可选认证，序列化时跳过
    
    #[serde(default)]
    pub database: String,
    
    // 连接选项（可映射为 MongoDB 连接字符串参数）
    #[serde(default = "default_app_name")]
    pub app_name: Option<String>,
    
    #[serde(default = "default_replica_set")]
    pub replica_set: Option<String>,
    
    #[serde(default)]
    pub auth_source: Option<String>, // 认证数据库，默认为 `database`
    
    #[serde(default)]
    pub use_tls: bool,
    
    #[serde(with = "humantime_serde", default = "default_connect_timeout_ms")]
    pub connect_timeout: Duration,
    
    #[serde(with = "humantime_serde", default = "default_socket_timeout_ms")]
    pub socket_timeout: Duration,
    
    // 连接池配置
    #[serde(default = "default_max_pool_size")]
    pub max_pool_size: u32,
    
    #[serde(default = "default_min_pool_size")]
    pub min_pool_size: u32,
    
    #[serde(with = "humantime_serde", default = "default_max_idle_time_ms")]
    pub max_idle_time: Option<Duration>,
}

// 默认值函数
fn default_host() -> String { "127.0.0.1".to_string() }
fn default_mongo_port() -> u16 { 27017 }
fn default_app_name() -> Option<String> { Some("my_rust_app".to_string()) }
fn default_replica_set() -> Option<String> { None }
fn default_connect_timeout_ms() -> Duration { Duration::from_millis(10000) }
fn default_socket_timeout_ms() -> Duration { Duration::from_millis(5000) }
fn default_max_pool_size() -> u32 { 100 }
fn default_min_pool_size() -> u32 { 0 }
fn default_max_idle_time_ms() -> Option<Duration> { Some(Duration::from_secs(60)) }

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
    /// 生成 MongoDB 连接 URI (符合官方规范)
    pub fn connection_uri(&self) -> String {
        let mut uri = if self.use_tls {
            "mongodb+srv://".to_string()
        } else {
            "mongodb://".to_string()
        };
        
        // 添加认证信息
        if let (Some(user), Some(pass)) = (&self.username, &self.password) {
            uri.push_str(&format!("{}:{}@", user, pass));
        }
        
        // 添加主机和端口
        uri.push_str(&format!("{}:{}", self.host, self.port));
        
        // 添加数据库和选项
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
        options.push(format!("connectTimeoutMS={}", self.connect_timeout.as_millis()));
        options.push(format!("socketTimeoutMS={}", self.socket_timeout.as_millis()));
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
        // 如果有用户名，则密码也必须提供（反之亦然）
        match (&self.username, &self.password) {
            (Some(_), None) => return Err("MongoDB 配置错误：提供了用户名但未提供密码".to_string()),
            (None, Some(_)) => return Err("MongoDB 配置错误：提供了密码但未提供用户名".to_string()),
            _ => {}
        }
        Ok(())
    }
}
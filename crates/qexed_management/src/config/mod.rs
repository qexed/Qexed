//! 管理服务器配置（server.properties 的 management-* 段对应）。

use serde::{Deserialize, Serialize};

#[qexed_config_macros::app_config("/", "management")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManagementConfig {
    /// 是否启用管理 API。
    pub enabled: bool,
    /// 监听地址（默认 localhost）。
    pub host: String,
    /// 监听端口（0 = 随机分配，启动后可查询实际端口）。
    pub port: u16,
    /// 认证密钥；为空时启动生成 40 位字母数字并写回。
    pub secret: String,
    /// 允许的 Origin 列表（空 = 拒绝所有跨源）。
    pub allowed_origins: Vec<String>,
    /// 状态心跳间隔（秒）。
    pub status_heartbeat_interval_seconds: u64,
}

impl Default for ManagementConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            host: "localhost".to_string(),
            port: 0,
            secret: String::new(),
            allowed_origins: Vec::new(),
            status_heartbeat_interval_seconds: 10,
        }
    }
}

impl ManagementConfig {
    /// 生成 40 位字母数字密钥（A-Z a-z 0-9）。
    pub fn generate_secret() -> String {
        use rand::Rng;
        const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
        let mut rng = rand::thread_rng();
        (0..40).map(|_| CHARSET[rng.gen_range(0..CHARSET.len())] as char).collect()
    }

    /// 密钥为空时生成并返回（写回由调用方持久化）。
    pub fn ensure_secret(&mut self) {
        if self.secret.trim().is_empty() {
            self.secret = Self::generate_secret();
        }
    }
}

impl ManagementConfig {
    /// 加载（不存在则建默认）并保证 secret 存在且持久化。
    /// 固定端口 25566；密钥一旦生成不随重启变化（桌面客户端可缓存）。
    pub fn load_persistent() -> crate::Result<Self> {
        use qexed_config::Config as _;
        let mut config = Self::load_and_create_default(true).unwrap_or_else(|_| Self::persistent_default());
        // 管理面按持久默认语义强制：固定端口 25566 + 启用（secret 保留文件值跨重启稳定）。
        config.enabled = true;
        config.host = "127.0.0.1".to_string();
        if config.port == 0 {
            config.port = 25566;
        }
        if config.secret.trim().is_empty() {
            config.secret = Self::generate_secret();
        }
        if let Err(err) = Self::save_file(&config) {
            log::warn!("management config save failed: {err}");
        }
        Ok(config)
    }

    /// 固定端口默认。
    pub fn persistent_default() -> Self {
        Self {
            enabled: true,
            host: "127.0.0.1".to_string(),
            port: 25566,
            secret: String::new(),
            allowed_origins: Vec::new(),
            status_heartbeat_interval_seconds: 10,
        }
    }
}

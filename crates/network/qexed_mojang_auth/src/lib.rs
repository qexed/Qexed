// src/lib.rs
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum MojangAuthError {
    #[error("网络请求失败: {0}")]
    NetworkError(#[from] reqwest::Error),
    
    #[error("Mojang 验证失败")]
    ValidationFailed,
    
    #[error("玩家不存在或未购买游戏")]
    PlayerNotFound,
    
    #[error("无效的服务器哈希")]
    InvalidServerHash,
    
    #[error("服务器繁忙，请稍后重试")]
    ServerBusy,
    
    #[error("IP 检查失败")]
    IpCheckFailed,
    
    #[error("UUID 格式错误: {0}")]
    UuidError(String),
}

// 确保错误类型是线程安全的
unsafe impl Send for MojangAuthError {}
unsafe impl Sync for MojangAuthError {}

#[derive(Debug, Clone, Deserialize)]
pub struct MojangProfile {
    /// 玩家UUID（不带连字符的32位十六进制字符串）
    pub id: String,
    /// 玩家用户名
    pub name: String,
    /// 玩家属性（皮肤、披风等）
    pub properties: Vec<PlayerProperty>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PlayerProperty {
    pub name: String,
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
}

// 确保这些结构体是 Send + Sync
unsafe impl Send for MojangProfile {}
unsafe impl Sync for MojangProfile {}

unsafe impl Send for PlayerProperty {}
unsafe impl Sync for PlayerProperty {}

#[derive(Debug, Clone)]
pub struct MojangAuthConfig {
    pub timeout: Duration,
    pub max_retries: u32,
    pub retry_delay: Duration,
    /// 是否验证玩家IP地址（对应 server.properties 中的 prevent-proxy-connections）
    pub prevent_proxy_connections: bool,
}

impl Default for MojangAuthConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(10),
            max_retries: 3,
            retry_delay: Duration::from_millis(500),
            prevent_proxy_connections: false,
        }
    }
}

// 使用 Arc 包装内部结构以实现线程安全
#[derive(Debug, Clone)]
pub struct MojangAuthClient {
    client: Arc<Client>,
    config: MojangAuthConfig,
}

impl MojangAuthClient {
    pub fn new(config: Option<MojangAuthConfig>) -> Self {
        let config = config.unwrap_or_default();
        let client = Client::builder()
            .timeout(config.timeout)
            .user_agent("qexed-mojang-auth/0.1.0")
            .build()
            .expect("Failed to build HTTP client");
        
        Self { 
            client: Arc::new(client), 
            config 
        }
    }
    
    /// 验证玩家加入服务器（通过 hasJoined API）
    pub async fn has_joined(
        &self,
        username: &str,
        server_hash: &str,
        player_ip: Option<&str>,
    ) -> Result<MojangProfile, MojangAuthError> {
        // 构建请求URL
        let mut url = format!(
            "https://sessionserver.mojang.com/session/minecraft/hasJoined?username={}&serverId={}",
            username, server_hash
        );
        
        // 如果配置了防止代理连接，添加IP参数
        if self.config.prevent_proxy_connections {
            if let Some(ip) = player_ip {
                // 验证IP地址格式
                if Self::is_valid_ip(ip) {
                    url.push_str(&format!("&ip={}", ip));
                } else {
                    return Err(MojangAuthError::IpCheckFailed);
                }
            }
        }
        
        // 带重试机制的请求
        self.send_has_joined_with_retry(&url).await
    }
    
    /// 验证玩家加入并返回格式化后的数据
    pub async fn verify_and_extract(
        &self,
        username: &str,
        server_hash: &str,
        player_ip: Option<&str>,
    ) -> Result<(Uuid, String, Vec<PlayerProperty>), MojangAuthError> {
        let profile = self.has_joined(username, server_hash, player_ip).await?;
        
        // 格式化UUID
        let uuid_str = Self::format_uuid(&profile.id)?;
        let uuid = Uuid::parse_str(&uuid_str)
            .map_err(|e| MojangAuthError::UuidError(e.to_string()))?;
        
        Ok((uuid, profile.name, profile.properties))
    }
    
    /// 从玩家UUID获取玩家资料（可选功能）
    pub async fn get_profile_from_uuid(
        &self,
        uuid: &str,
    ) -> Result<MojangProfile, MojangAuthError> {
        // 确保UUID是32位十六进制格式（无连字符）
        let uuid = uuid.replace('-', "");
        if uuid.len() != 32 {
            return Err(MojangAuthError::ValidationFailed);
        }
        
        let url = format!(
            "https://sessionserver.mojang.com/session/minecraft/profile/{}",
            uuid
        );
        
        let response = self.client
            .get(&url)
            .send()
            .await?;
        
        match response.status().as_u16() {
            200 => {
                let profile: MojangProfile = response.json().await?;
                Ok(profile)
            }
            204 => Err(MojangAuthError::PlayerNotFound),
            429 => Err(MojangAuthError::ServerBusy),
            _ => Err(MojangAuthError::ValidationFailed),
        }
    }
    
    /// 从用户名获取UUID（可选功能）
    pub async fn get_uuid_from_username(
        &self,
        username: &str,
    ) -> Result<String, MojangAuthError> {
        let url = format!(
            "https://api.mojang.com/users/profiles/minecraft/{}",
            username
        );
        
        #[derive(Debug, Deserialize)]
        struct UuidResponse {
            id: String,
            name: String,
        }
        
        let response = self.client
            .get(&url)
            .send()
            .await?;
        
        match response.status().as_u16() {
            200 => {
                let resp: UuidResponse = response.json().await?;
                Ok(resp.id)  // 返回无连字符的UUID
            }
            204 => Err(MojangAuthError::PlayerNotFound),
            429 => Err(MojangAuthError::ServerBusy),
            _ => Err(MojangAuthError::ValidationFailed),
        }
    }
    
    /// 批量获取玩家资料（可选功能）
    pub async fn get_profiles_from_usernames(
        &self,
        usernames: &[String],
    ) -> Result<Vec<MojangProfile>, MojangAuthError> {
        let url = "https://api.mojang.com/profiles/minecraft";
        
        let response = self.client
            .post(url)
            .json(usernames)
            .send()
            .await?;
        
        if response.status().is_success() {
            let profiles: Vec<MojangProfile> = response.json().await?;
            Ok(profiles)
        } else {
            Err(MojangAuthError::ValidationFailed)
        }
    }
    
    /// 获取Mojang服务器状态
    pub async fn get_server_status(&self) -> Result<serde_json::Value, MojangAuthError> {
        let url = "https://status.mojang.com/check";
        
        let response = self.client
            .get(url)
            .send()
            .await?;
        
        if response.status().is_success() {
            let status: serde_json::Value = response.json().await?;
            Ok(status)
        } else {
            Err(MojangAuthError::NetworkError(response.error_for_status().err().unwrap()))
        }
    }
    
    /// 格式化UUID：从 "11111111222233334444555555555555" 转换为 "11111111-2222-3333-4444-555555555555"
    pub fn format_uuid(uuid_str: &str) -> Result<String, MojangAuthError> {
        let uuid_str = uuid_str.replace('-', "");
        
        if uuid_str.len() != 32 {
            return Err(MojangAuthError::ValidationFailed);
        }
        
        // 检查是否为有效的十六进制
        if !uuid_str.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(MojangAuthError::ValidationFailed);
        }
        
        let mut chars: Vec<char> = uuid_str.chars().collect();
        
        // 在适当位置插入连字符
        chars.insert(20, '-');
        chars.insert(16, '-');
        chars.insert(12, '-');
        chars.insert(8, '-');
        
        Ok(chars.iter().collect())
    }
    
    /// 解析UUID：从带连字符的格式转换为无连字符格式
    pub fn parse_uuid(uuid_str: &str) -> Result<String, MojangAuthError> {
        let uuid_str = uuid_str.replace('-', "");
        
        if uuid_str.len() != 32 {
            return Err(MojangAuthError::ValidationFailed);
        }
        
        if !uuid_str.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(MojangAuthError::ValidationFailed);
        }
        
        Ok(uuid_str.to_lowercase())
    }
    
    // 私有辅助方法
    async fn send_has_joined_with_retry(
        &self,
        url: &str,
    ) -> Result<MojangProfile, MojangAuthError> {
        for attempt in 0..=self.config.max_retries {
            match self.client.get(url).send().await {
                Ok(response) => {
                    match response.status().as_u16() {
                        200 => {
                            let profile: MojangProfile = response.json().await?;
                            
                            // 验证返回的用户名与请求的用户名匹配（不区分大小写）
                            if let Some(query_username) = url
                                .split('=')
                                .nth(1)
                                .and_then(|s| s.split('&').next())
                            {
                                if profile.name.to_lowercase() != query_username.to_lowercase() {
                                    return Err(MojangAuthError::ValidationFailed);
                                }
                            }
                            
                            return Ok(profile);
                        }
                        204 => return Err(MojangAuthError::PlayerNotFound),
                        429 => { // 请求过多
                            if attempt < self.config.max_retries {
                                let delay = self.config.retry_delay * (attempt as u32 + 1);
                                tokio::time::sleep(delay).await;
                                continue;
                            }
                            return Err(MojangAuthError::ServerBusy);
                        }
                        400 => return Err(MojangAuthError::InvalidServerHash),
                        403 => return Err(MojangAuthError::ValidationFailed),
                        _ => {
                            if attempt < self.config.max_retries {
                                tokio::time::sleep(self.config.retry_delay).await;
                                continue;
                            }
                            return Err(MojangAuthError::ValidationFailed);
                        }
                    }
                }
                Err(e) => {
                    if attempt == self.config.max_retries {
                        return Err(MojangAuthError::NetworkError(e));
                    }
                    tokio::time::sleep(self.config.retry_delay).await;
                }
            }
        }
        
        Err(MojangAuthError::ServerBusy)
    }
    
    fn is_valid_ip(ip: &str) -> bool {
        // 简单的IPv4验证
        if ip.split('.').count() == 4 {
            return ip.split('.').all(|octet| {
                octet.parse::<u8>().is_ok()
            });
        }
        
        // 简单的IPv6验证
        if ip.contains(':') {
            return ip.split(':').all(|part| {
                if part.is_empty() {
                    true
                } else {
                    u16::from_str_radix(part, 16).is_ok()
                }
            });
        }
        
        false
    }
}

// 服务器哈希计算函数
pub fn calculate_server_hash(server_id: &str, shared_secret: &[u8], public_key: &[u8]) -> String {
    use sha1::{Sha1, Digest};
    
    let mut hasher = Sha1::new();
    hasher.update(server_id.as_bytes());
    hasher.update(shared_secret);
    hasher.update(public_key);
    
    // Minecraft 使用特定的哈希格式：带符号的BigInteger十六进制
    let hash = hasher.finalize();
    let hex = hex::encode(hash);
    
    // 如果哈希是负数，需要特殊处理
    if (hash[0] & 0x80) != 0 {
        // 转换为Java的BigInteger格式
        format!("-{}", hex.trim_start_matches("ff"))
    } else {
        hex
    }
}

/// 线程安全的登录处理函数
pub async fn handle_login(
    username: String,
    client_ip: Option<std::net::SocketAddr>,
    server_hash: String,
) -> Result<(Uuid, String, Vec<PlayerProperty>), MojangAuthError> {
    // 创建认证客户端
    let config = MojangAuthConfig {
        timeout: Duration::from_secs(10),
        max_retries: 3,
        retry_delay: Duration::from_millis(500),
        prevent_proxy_connections: false, // 从server.properties读取
    };
    
    let auth_client = MojangAuthClient::new(Some(config)); 
    let player_ip = client_ip.map(|p| p.ip().to_string());
    
    // 使用新的方法验证并提取数据
    auth_client.verify_and_extract(
        &username,
        &server_hash,
        player_ip.as_deref(),
    ).await
}

/// 线程安全的异步登录处理器
#[derive(Clone)]
pub struct AsyncLoginHandler {
    client: MojangAuthClient,
}

impl AsyncLoginHandler {
    pub fn new(config: Option<MojangAuthConfig>) -> Self {
        Self {
            client: MojangAuthClient::new(config),
        }
    }
    
    pub async fn handle_login(
        &self,
        username: String,
        client_ip: Option<std::net::SocketAddr>,
        server_hash: String,
    ) -> Result<(Uuid, String, Vec<PlayerProperty>), MojangAuthError> {
        let player_ip = client_ip.map(|p| p.ip().to_string());
        
        self.client.verify_and_extract(
            &username,
            &server_hash,
            player_ip.as_deref(),
        ).await
    }
    
    pub fn get_client(&self) -> &MojangAuthClient {
        &self.client
    }
}
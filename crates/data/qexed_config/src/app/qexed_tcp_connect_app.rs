use serde::{Deserialize, Serialize};

use crate::tool::AppConfigTrait;

#[derive(Debug, Serialize, Deserialize)]
pub struct TcpConnect {
    pub version: i32,
    pub ip: String,
    // Mojang 认证
    pub online_mode: bool,
    /// 网络数据包压缩
    pub network_compression_threshold: usize,
    /// 是否启用代理
    pub proxy:bool,
    /// 代理端协议
    pub proxy_protocol: ForwardingMode,
    /// 认证密钥
    pub proxy_token: String,
    /// 同IP连接频率限制 - 时间窗口（秒）
    pub rate_limit_window_secs: u64,
    /// 同IP连接频率限制 - 窗口内最大允许次数
    pub rate_limit_max_attempts: u32,
    /// Status数据包检测延迟
    pub status_timeout_secs:i32,

}



#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub enum ForwardingMode {
    Default,
    QTunnel,
    Victory,
    BungeeCord,
    None,
}
impl Default for ForwardingMode {
    fn default() -> Self {
        ForwardingMode::Default
    }
}

// 为ForwardingMode实现Display trait
impl std::fmt::Display for ForwardingMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ForwardingMode::Default => write!(f, "Default"),
            ForwardingMode::QTunnel => write!(f, "QTunnel"),
            ForwardingMode::Victory => write!(f, "Victory"),
            ForwardingMode::BungeeCord => write!(f, "BungeeCord"),
            ForwardingMode::None => write!(f, "None"),
        }
    }
}

// 为ForwardingMode实现FromStr用于解析
impl std::str::FromStr for ForwardingMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "default" => Ok(ForwardingMode::Default),
            "qtunnel" => Ok(ForwardingMode::QTunnel),
            "victory" => Ok(ForwardingMode::Victory),
            "bungeecord" => Ok(ForwardingMode::BungeeCord),
            "none" => Ok(ForwardingMode::None),
            _ => Err(format!("未知的转发模式: {}", s)),
        }
    }
}

impl Default for TcpConnect {
    fn default() -> Self {
        Self {
            version: 0,
            ip: "0.0.0.0:25565".to_string(),
            online_mode: true,
            network_compression_threshold: 256,
            proxy: false,
            proxy_protocol: ForwardingMode::QTunnel,
            proxy_token: qexed_random::token::token(),
            rate_limit_window_secs: 60,
            rate_limit_max_attempts: 6,
            status_timeout_secs: 5,
            
        }
    }
}
impl AppConfigTrait for TcpConnect {
    const PATH: &'static str = "./config/qexed_player_list/";

    const NAME: &'static str = "config";
}

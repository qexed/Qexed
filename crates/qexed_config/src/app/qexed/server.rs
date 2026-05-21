use serde::{Deserialize, Serialize};
use rust_i18n::t;
#[derive(Debug, Serialize, Deserialize)]
pub struct Server {
    // 服务器IP地址
    pub ip:String,
    // 是否启用正版验证
    pub online:bool,
    // 最大玩家数(-1为不限制)
    pub max_player:i32,
    // 是否显示玩家数
    // 此选项会使得其他代理端无法获取到玩家数,若启用返回-1
    pub display_players:bool,
    // Mojang 认证
    pub online_mode: bool,
    /// 网络数据包压缩
    pub network_compression_threshold: isize,
    /// 是否启用代理
    pub proxy:bool,
    /// 代理端协议
    pub proxy_protocol: ForwardingMode,
    /// 认证密钥
    pub proxy_token: String,
    /// 总TCP连接同时在线限制数
    /// 若为0则65535（int16范围)
    pub max_port_connections:u16,
    /// 同IP连接频率限制 - 时间窗口（秒）
    pub rate_limit_window_secs: u64,
    /// 同IP连接频率限制 - 窗口内最大允许次数
    pub rate_limit_max_attempts: u32,
    /// 服务器描述随机内容
    pub motd: Vec<String>,
    /// 服务器logo
    pub favicon:String,
}
impl Default for Server {
    fn default() -> Self {
        Self {
            ip:"0.0.0.0:25565".to_owned(),
            online:false,
            max_player:-1,
            display_players:true,
            online_mode: true,
            network_compression_threshold: 256,
            proxy: false,
            proxy_protocol: ForwardingMode::QTunnel,
            proxy_token: nanoid::nanoid!(),
            rate_limit_window_secs: 60,
            rate_limit_max_attempts: 6,
            motd: vec![t!("qexed_config.config.server.motd1").to_string(), t!("qexed_config.config.server.motd2").to_string()],
            favicon: "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAEAAAABACAYAAACqaXHeAAAACXBIWXMAAA9hAAAPYQGoP6dpAAACtklEQVR42u2ay0rDQBSGJ2EQCipqERU3SkFQQUERRJSCuHDrQvcu3Powbn0DH6IIohQKIi26ELRF8FLxAlbsyksTmTC2yVwyk3ZizmySkjaT/zvnP5mT1PpuDJTgYaOEDwAAAAAAAAAAAAAAAAAAAAAAAABI5MAyX7YsS3lC07pvLCs+jAAd4DpqARXxia8BpsOzk5r6QgB0iTfZOnbU0TO9bthRCIhT0cRRpX4U4v2yUnUezJokrA10iReZX7XWYN0CdNUO0Wj7BUzm+rGJvpSJKn2c/M7ZikKQArC/1RqVnYPvjon3gyELwWp+N+j3QyJ8cHYNTU30uPuvz1V3W8yd/IHAmph3UToLqOi5uAAc8dnNDU/0eeW95SSf10UPQlgAQZEPC0U0kzAv5R3xtPDFhRQaHc+4+7flK1R5SqHba30W0HUHoe2gVAOIeFo4EZ8v1Bt7dTSzuuTCCqoHKqmvAoRAYM2PWdF3PH9eeQwUvzyf8WpB6cZiimve6rrdqmYMcylMCh6d8seFO088GbkiZkaB5ftOd4yYl/601x/KvylPxBN7DPUidC+Yjn4FTtQqYazBswETgCNueHygIR41xL94wi8ua2gk/eEer771ofvTI7SX/wrl7043Tszb4O6ijda3s6746bFu1J8e8rLCEX92WHL3afG8KDdHsB0AWHNw1wEOhJG5FTQ52uV+fqk9+grnpTHP68YCIBDoEZTuMguhZiBGA5CdTHYl2I5nCEHnhldj/1mcyBrDCABBdaEd/YUdx6jpPI8xAHQWQJl+w6gMoK0QNhNkmy3jLCCyihRprCJ5JqialrINjEhEVd8VYNPEs+4MUSynjXwsLmMJ7W+GTIh+OxsmOy7iY7cUjoN4aIaiAhCX6AcWQdX1eJz+TYbjfPFQAwAAAAAAAAAAAACl8QOub9TOwLTmGwAAAABJRU5ErkJggg==".to_string(),
            max_port_connections:u16::MAX,

        }
    }
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
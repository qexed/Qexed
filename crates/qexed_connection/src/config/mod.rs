//! qexed_connection 配置。
//!
//! v4 的 `qexed_config::app::qexed::server::Server` 里连接相关字段
//! （在线模式 / 代理转发 / 压缩 / MOTD / favicon / 行为守则开关 / 资源包）
//! 在 v6 不存在，这里用 app_config 宏在本 crate 重新定义。
//! 字段名与默认值尽量与 v4 对齐（snake_case）。

use qexed_doc_macros::{Doc, DocValue};
use serde::{Deserialize, Serialize};

/// ```autodoc
/// <Name>qexed.crates.connection.config.ConnectionConfig</Name>
/// <Attr name="writable" />
/// ```
#[qexed_config_macros::app_config("/", "connection")]
#[derive(Debug, Serialize, Deserialize, Doc)]
pub struct ConnectionConfig {
    /// ```autodoc
    /// <Name>qexed.crates.connection.config.ConnectionConfig.ip</Name>
    /// <Default>0.0.0.0:25565</Default>
    /// ```
    pub ip: String,
    /// ```autodoc
    /// <Name>qexed.crates.connection.config.ConnectionConfig.online_mode</Name>
    /// <Default>true</Default>
    /// ```
    pub online_mode: bool,
    /// ```autodoc
    /// <Name>qexed.crates.connection.config.ConnectionConfig.max_player</Name>
    /// <Default>-1</Default>
    /// ```
    pub max_player: i32,
    /// ```autodoc
    /// <Name>qexed.crates.connection.config.ConnectionConfig.display_players</Name>
    /// <Default>true</Default>
    /// ```
    pub display_players: bool,
    /// ```autodoc
    /// <Name>qexed.crates.connection.config.ConnectionConfig.network_compression_threshold</Name>
    /// <Default>256</Default>
    /// ```
    pub network_compression_threshold: isize,
    /// ```autodoc
    /// <Name>qexed.crates.connection.config.ConnectionConfig.motd</Name>
    /// <Default>Qexed服务器awa</Default>
    /// ```
    pub motd: String,
    /// ```autodoc
    /// <Name>qexed.crates.connection.config.ConnectionConfig.code_of_conduct</Name>
    /// <Default>false</Default>
    /// ```
    pub code_of_conduct: bool,
    /// ```autodoc
    /// <Name>qexed.crates.connection.config.ConnectionConfig.favicon</Name>
    /// <Default>""</Default>
    /// ```
    pub favicon: String,
    /// ```autodoc
    /// <Name>qexed.crates.connection.config.ConnectionConfig.proxy</Name>
    /// <Default>false</Default>
    /// ```
    pub proxy: bool,
    /// ```autodoc
    /// <Name>qexed.crates.connection.config.ConnectionConfig.proxy_protocol</Name>
    /// <Default>QTunnel</Default>
    /// ```
    pub proxy_protocol: ForwardingMode,
    /// ```autodoc
    /// <Name>qexed.crates.connection.config.ConnectionConfig.proxy_token</Name>
    /// <Secret />
    /// <Default>""</Default>
    /// ```
    pub proxy_token: String,
    /// ```autodoc
    /// <Name>qexed.crates.connection.config.ConnectionConfig.proxy_online_mode</Name>
    /// <Default>true</Default>
    /// ```
    pub proxy_online_mode: bool,
    /// ```autodoc
    /// <Name>qexed.crates.connection.config.ConnectionConfig.resource_pack</Name>
    /// ```
    pub resource_pack: ResourcePack,
}

impl Default for ConnectionConfig {
    fn default() -> Self {
        Self {
            ip: "0.0.0.0:25565".to_string(),
            online_mode: true,
            max_player: -1,
            display_players: true,
            network_compression_threshold: 256,
            motd: "Qexed服务器awa".to_string(),
            code_of_conduct: false,
            favicon: String::new(),
            proxy: false,
            proxy_protocol: ForwardingMode::default(),
            proxy_token: String::new(),
            proxy_online_mode: true,
            resource_pack: ResourcePack::default(),
        }
    }
}

/// 代理转发模式，与 v4 `ForwardingMode` 对齐。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, DocValue)]
pub enum ForwardingMode {
    Default,
    QTunnel,
    Victory,
    Velocity,
    BungeeCord,
    None,
}

impl Default for ForwardingMode {
    fn default() -> Self {
        Self::Default
    }
}

impl std::fmt::Display for ForwardingMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Default => write!(f, "Default"),
            Self::QTunnel => write!(f, "QTunnel"),
            Self::Victory => write!(f, "Victory"),
            Self::Velocity => write!(f, "Velocity"),
            Self::BungeeCord => write!(f, "BungeeCord"),
            Self::None => write!(f, "None"),
        }
    }
}

impl std::str::FromStr for ForwardingMode {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim() {
            "Default" => Ok(Self::Default),
            "QTunnel" => Ok(Self::QTunnel),
            "Victory" => Ok(Self::Victory),
            "Velocity" => Ok(Self::Velocity),
            "BungeeCord" => Ok(Self::BungeeCord),
            "None" => Ok(Self::None),
            other => Err(format!("unknown forwarding mode: {other}")),
        }
    }
}

/// 资源包推送配置（v4 `ResourcePack` 的连接相关子集）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResourcePack {
    /// ```autodoc
    /// <Name>qexed.crates.connection.config.ResourcePack.enable</Name>
    /// <Default>false</Default>
    /// ```
    pub enable: bool,
    /// ```autodoc
    /// <Name>qexed.crates.connection.config.ResourcePack.id</Name>
    /// <Default>00000000-0000-0000-0000-000000000000</Default>
    /// ```
    pub id: String,
    /// ```autodoc
    /// <Name>qexed.crates.connection.config.ResourcePack.url</Name>
    /// <Default>""</Default>
    /// ```
    pub url: String,
    /// ```autodoc
    /// <Name>qexed.crates.connection.config.ResourcePack.hash</Name>
    /// <Default>""</Default>
    /// ```
    pub hash: String,
    /// ```autodoc
    /// <Name>qexed.crates.connection.config.ResourcePack.required</Name>
    /// <Default>false</Default>
    /// ```
    pub required: bool,
    /// ```autodoc
    /// <Name>qexed.crates.connection.config.ResourcePack.prompt</Name>
    /// <Default>""</Default>
    /// ```
    pub prompt: String,
    /// ```autodoc
    /// <Name>qexed.crates.connection.config.ResourcePack.disconnect_message</Name>
    /// <Default>Resource pack is required</Default>
    /// ```
    pub disconnect_message: String,
}

// TODO(doc): <Attr name="sub"/> 在当前 qexed_doc_macros 生成 &str 与 String 参数不匹配，
// 暂以宽松 DocValue（接受任意对象）代替嵌套 schema；上游宏修复后改回 sub + Doc 派生。
impl qexed_doc::DocValue for ResourcePack {
    const TYPE: &'static str = "object";
    const VARIANTS: &'static [&'static str] = &[];
    fn validate(value: &qexed_doc::serde_json::Value) -> Result<(), String> {
        if value.is_object() {
            Ok(())
        } else {
            Err("expected resource pack object".to_string())
        }
    }
}
impl Default for ResourcePack {
    fn default() -> Self {
        Self {
            enable: false,
            id: "00000000-0000-0000-0000-000000000000".to_string(),
            url: String::new(),
            hash: String::new(),
            required: false,
            prompt: String::new(),
            disconnect_message: "Resource pack is required".to_string(),
        }
    }
}

impl ConnectionConfig {
    /// MOTD 按行切分（配置文件内用 \n 分行，对应 v4 的 Vec&lt;String&gt;）。
    pub fn motd_lines(&self) -> Vec<String> {
        self.motd
            .split('\n')
            .map(str::to_string)
            .collect()
    }
}

impl ResourcePack {
    /// 解析资源包 UUID；解析失败回退 nil（v4 直接存 Uuid，v6 文档系统需要标量字段）。
    pub fn pack_id(&self) -> uuid::Uuid {
        uuid::Uuid::parse_str(&self.id).unwrap_or_else(|_| uuid::Uuid::nil())
    }
}

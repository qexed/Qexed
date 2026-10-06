//! qexed_server 配置：对应 v4 的 qexed_config::app::qexed::* 里 server 域的配置。
//! v6 的 qexed_config 不再有那些路径，这里用 app_config 宏重新定义（迁移规则 4）。
//!
//! v4 原配置拆分参考（v4 qexed_config::app::qexed::server）：
//! ip/max_player/motd/favicon/online_mode/proxy/... 收敛为 ServerConfig；
//! 内容过滤收敛为 ContentFilterConfig；玩家审计收敛为 PlayerAuditConfig；
//! warden 封禁数据收敛为 WardenConfig。world/entities/play 域配置归各自 crate，
//! 此处不重复定义。
//!
//! 说明：Doc 宏要求每个字段类型实现 DocValue（基本类型 + unit 枚举）；
//! 含 Vec/嵌套结构体字段的配置不派生 Doc（app_config 不依赖 Doc，schema 文档跳过）。

use qexed_doc_macros::{Doc, DocValue};
use serde::{Deserialize, Serialize};

/// 服务器核心配置（监听 / 人数 / 在线模式 / 代理转发）。
///
/// ```autodoc
/// <Name>qexed.crates.server.config.ServerConfig</Name>
/// <Attr name="writable" />
/// ```
#[qexed_config_macros::app_config("/", "server")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// ```autodoc
    /// <Name>qexed.crates.server.config.ServerConfig.ip</Name>
    /// <Default>0.0.0.0:25565</Default>
    /// ```
    pub ip: String,
    /// ```autodoc
    /// <Name>qexed.crates.server.config.ServerConfig.max_player</Name>
    /// <Default>-1</Default>
    /// ```
    pub max_player: i32,
    /// ```autodoc
    /// <Name>qexed.crates.server.config.ServerConfig.max_port_connections</Name>
    /// <Default>1024</Default>
    /// ```
    pub max_port_connections: u32,
    /// v4 motd 为多行字符串数组。
    #[serde(default = "default_motd")]
    pub motd: Vec<String>,
    /// ```autodoc
    /// <Name>qexed.crates.server.config.ServerConfig.favicon</Name>
    /// <Default></Default>
    /// ```
    #[serde(default)]
    pub favicon: String,
    /// ```autodoc
    /// <Name>qexed.crates.server.config.ServerConfig.display_players</Name>
    /// <Default>true</Default>
    /// ```
    #[serde(default = "default_true")]
    pub display_players: bool,
    /// ```autodoc
    /// <Name>qexed.crates.server.config.ServerConfig.online_mode</Name>
    /// <Default>true</Default>
    /// ```
    #[serde(default = "default_true")]
    pub online_mode: bool,
    /// ```autodoc
    /// <Name>qexed.crates.server.config.ServerConfig.language</Name>
    /// <Default>zh-CN</Default>
    /// ```
    #[serde(default = "default_language")]
    pub language: String,
    /// ```autodoc
    /// <Name>qexed.crates.server.config.ServerConfig.proxy</Name>
    /// <Default>false</Default>
    /// ```
    #[serde(default)]
    pub proxy: bool,
    /// ```autodoc
    /// <Name>qexed.crates.server.config.ServerConfig.proxy_online_mode</Name>
    /// <Default>true</Default>
    /// ```
    #[serde(default = "default_true")]
    pub proxy_online_mode: bool,
    /// ```autodoc
    /// <Name>qexed.crates.server.config.ServerConfig.proxy_protocol</Name>
    /// <Select>None</Select>
    /// <Select>Velocity</Select>
    /// <Select>Victory</Select>
    /// <Default>None</Default>
    /// ```
    #[serde(default)]
    pub proxy_protocol: ForwardingMode,
    /// ```autodoc
    /// <Name>qexed.crates.server.config.ServerConfig.log_level</Name>
    /// <Select>Trace</Select>
    /// <Select>Debug</Select>
    /// <Select>Info</Select>
    /// <Select>Warn</Select>
    /// <Select>Error</Select>
    /// <Select>Off</Select>
    /// <Default>Info</Default>
    /// ```
    #[serde(default)]
    pub log_level: ServerLogLevel,
}

fn default_motd() -> Vec<String> {
    vec!["A Qexed Server".to_string()]
}

fn default_true() -> bool {
    true
}

fn default_language() -> String {
    "zh-CN".to_string()
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            ip: "0.0.0.0:25565".to_string(),
            max_player: -1,
            max_port_connections: 1024,
            motd: default_motd(),
            favicon: String::new(),
            display_players: true,
            online_mode: true,
            language: default_language(),
            proxy: false,
            proxy_online_mode: true,
            proxy_protocol: ForwardingMode::default(),
            log_level: ServerLogLevel::default(),
        }
    }
}

/// 代理转发模式（v4 qexed_config::app::qexed::server::ForwardingMode）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, DocValue)]
pub enum ForwardingMode {
    #[default]
    None,
    Velocity,
    Victory,
}

/// 服务器日志级别（v4 qexed_config::app::qexed::server::ServerLogLevel）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, DocValue)]
pub enum ServerLogLevel {
    Trace,
    Debug,
    #[default]
    Info,
    Warn,
    Error,
    Off,
}

impl ServerLogLevel {
    /// 日志级别名（与 qexed_log 配置里的级别名一致）。
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Trace => "Trace",
            Self::Debug => "Debug",
            Self::Info => "Info",
            Self::Warn => "Warn",
            Self::Error => "Error",
            Self::Off => "Off",
        }
    }
}

/// 内容过滤配置（v4 qexed_config::app::qexed::server::ContentFilter）。
#[qexed_config_macros::app_config("/", "content_filter")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentFilterConfig {
    /// ```autodoc
    /// <Name>qexed.crates.server.config.ContentFilterConfig.enable</Name>
    /// <Default>false</Default>
    /// ```
    #[serde(default)]
    pub enable: bool,
    /// ```autodoc
    /// <Name>qexed.crates.server.config.ContentFilterConfig.engine</Name>
    /// <Select>Fixed</Select>
    /// <Select>Knowledge</Select>
    /// <Select>Api</Select>
    /// <Default>Fixed</Default>
    /// ```
    #[serde(default)]
    pub engine: ContentFilterEngine,
    /// 敏感词表（Fixed 引擎直接使用）。
    #[serde(default)]
    pub words: Vec<String>,
    /// ```autodoc
    /// <Name>qexed.crates.server.config.ContentFilterConfig.replacement</Name>
    /// <Default>***</Default>
    /// ```
    #[serde(default = "default_replacement")]
    pub replacement: String,
    /// ```autodoc
    /// <Name>qexed.crates.server.config.ContentFilterConfig.knowledge_path</Name>
    /// <Default></Default>
    /// ```
    #[serde(default)]
    pub knowledge_path: String,
    /// ```autodoc
    /// <Name>qexed.crates.server.config.ContentFilterConfig.api_url</Name>
    /// <Default></Default>
    /// ```
    #[serde(default)]
    pub api_url: String,
    /// ```autodoc
    /// <Name>qexed.crates.server.config.ContentFilterConfig.api_token</Name>
    /// <Default></Default>
    /// <Secret />
    /// ```
    #[serde(default)]
    pub api_token: String,
    /// ```autodoc
    /// <Name>qexed.crates.server.config.ContentFilterConfig.block_message</Name>
    /// <Default>消息包含违规内容，已被拦截</Default>
    /// ```
    #[serde(default = "default_block_message")]
    pub block_message: String,
}

fn default_replacement() -> String {
    "***".to_string()
}

fn default_block_message() -> String {
    "消息包含违规内容，已被拦截".to_string()
}

impl Default for ContentFilterConfig {
    fn default() -> Self {
        Self {
            enable: false,
            engine: ContentFilterEngine::default(),
            words: Vec::new(),
            replacement: default_replacement(),
            knowledge_path: String::new(),
            api_url: String::new(),
            api_token: String::new(),
            block_message: default_block_message(),
        }
    }
}

/// 内容过滤引擎（v4 qexed_config::app::qexed::server::ContentFilterEngine）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, DocValue)]
pub enum ContentFilterEngine {
    /// 固定词表（配置里的 words）。
    #[default]
    Fixed,
    /// 词库文件（knowledge_path 指向的敏感词知识库）。
    Knowledge,
    /// 远程 API（api_url + api_token）。
    Api,
}

/// 玩家审计配置（v4 qexed_config::app::qexed::server::PlayerAudit）。
#[qexed_config_macros::app_config("/", "player_audit")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerAuditConfig {
    /// ```autodoc
    /// <Name>qexed.crates.server.config.PlayerAuditConfig.enable</Name>
    /// <Default>false</Default>
    /// ```
    #[serde(default)]
    pub enable: bool,
    /// ```autodoc
    /// <Name>qexed.crates.server.config.PlayerAuditConfig.storage</Name>
    /// <Select>File</Select>
    /// <Select>Stdout</Select>
    /// <Select>FileAndStdout</Select>
    /// <Default>Stdout</Default>
    /// ```
    #[serde(default)]
    pub storage: PlayerAuditStorage,
    /// ```autodoc
    /// <Name>qexed.crates.server.config.PlayerAuditConfig.file_path</Name>
    /// <Default>./log/player_audit.jsonl</Default>
    /// ```
    #[serde(default = "default_audit_path")]
    pub file_path: String,
    /// 各事件开关。
    #[serde(default)]
    pub events: PlayerAuditEvents,
}

fn default_audit_path() -> String {
    "./log/player_audit.jsonl".to_string()
}

impl Default for PlayerAuditConfig {
    fn default() -> Self {
        Self {
            enable: false,
            storage: PlayerAuditStorage::default(),
            file_path: default_audit_path(),
            events: PlayerAuditEvents::default(),
        }
    }
}

/// 审计存储方式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, DocValue)]
pub enum PlayerAuditStorage {
    File,
    #[default]
    Stdout,
    FileAndStdout,
}

/// 审计事件开关（嵌套结构体，Doc 跳过）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerAuditEvents {
    #[serde(default = "default_true")]
    pub block_place: bool,
    #[serde(default = "default_true")]
    pub block_break: bool,
    #[serde(default = "default_true")]
    pub item_switch: bool,
    #[serde(default = "default_true")]
    pub command: bool,
}

impl Default for PlayerAuditEvents {
    fn default() -> Self {
        Self {
            block_place: true,
            block_break: true,
            item_switch: true,
            command: true,
        }
    }
}

/// Warden 封禁数据（v4 qexed_config::app::qexed_warden::QexedWarden 的 data 部分）。
/// v4 的 BanRecord 持久化在配置文件里，v6 保持同构。
#[qexed_config_macros::app_config("/", "warden")]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WardenConfig {
    /// 显式封禁记录。
    #[serde(default)]
    pub bans: Vec<BanRecord>,
    /// v4 遗留的旧版黑名单（只存 UUID，一律视为永久封禁）。
    #[serde(default)]
    pub player_list: Vec<uuid::Uuid>,
}

/// 封禁记录（v4 qexed_config::app::qexed_warden::data::BanRecord）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BanRecord {
    pub uuid: uuid::Uuid,
    pub username: String,
    pub reason: String,
    pub permanent: bool,
    pub created_at_unix_secs: i64,
}

/// 占位符配置（v4 qexed_config::app::qexed::server::Placeholders）。
///
/// ```autodoc
/// <Name>qexed.crates.server.config.PlaceholdersConfig</Name>
/// <Attr name="writable" />
/// ```
#[qexed_config_macros::app_config("/", "placeholders")]
#[derive(Debug, Clone, Serialize, Deserialize, Doc)]
pub struct PlaceholdersConfig {
    /// ```autodoc
    /// <Name>qexed.crates.server.config.PlaceholdersConfig.enable</Name>
    /// <Default>true</Default>
    /// ```
    #[serde(default = "default_true")]
    pub enable: bool,
}

impl Default for PlaceholdersConfig {
    fn default() -> Self {
        Self { enable: true }
    }
}

/// 行为准则（code of conduct）展示开关与目录。
#[qexed_config_macros::app_config("/", "code_of_conduct")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeOfConductConfig {
    /// ```autodoc
    /// <Name>qexed.crates.server.config.CodeOfConductConfig.enable</Name>
    /// <Default>false</Default>
    /// ```
    #[serde(default)]
    pub enable: bool,
    /// ```autodoc
    /// <Name>qexed.crates.server.config.CodeOfConductConfig.dir</Name>
    /// <Default>config/enable-code-of-conduct</Default>
    /// ```
    #[serde(default = "default_coc_dir")]
    pub dir: String,
}

fn default_coc_dir() -> String {
    crate::code_of_conduct::DEFAULT_CODE_OF_CONDUCT_DIR.to_string()
}

impl Default for CodeOfConductConfig {
    fn default() -> Self {
        Self {
            enable: false,
            dir: default_coc_dir(),
        }
    }
}

/// 世界集群（world cluster）配置：v4 qexed_config::app::qexed::server::WorldCluster。
#[qexed_config_macros::app_config("/", "world_cluster")]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorldClusterConfig {
    /// ```autodoc
    /// <Name>qexed.crates.server.config.WorldClusterConfig.enable</Name>
    /// <Default>false</Default>
    /// ```
    #[serde(default)]
    pub enable: bool,
    /// 分片列表。
    #[serde(default)]
    pub shards: Vec<ClusterShardConfig>,
}

/// 集群分片配置（v4 WorldCluster.shards 元素；区域用 x/z_range 表达）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterShardConfig {
    /// 分片 id（用于实体 id 基址与日志）。
    pub id: String,
    /// 分片 RPC endpoint，形如 tcp://host:port。
    pub endpoint: String,
    /// 该分片负责的 chunk x 范围 [min, max] 闭区间（未配置则不限制）。
    #[serde(default)]
    pub x_range: Option<(i32, i32)>,
    /// 该分片负责的 chunk z 范围 [min, max] 闭区间（未配置则不限制）。
    #[serde(default)]
    pub z_range: Option<(i32, i32)>,
}

/// LAN 发现配置（v4 lan_discovery 域收敛；v6 服务器范围仅保留开关与 MOTD）。
///
/// ```autodoc
/// <Name>qexed.crates.server.config.LanDiscoveryConfig</Name>
/// <Attr name="writable" />
/// ```
#[qexed_config_macros::app_config("/", "lan_discovery")]
#[derive(Debug, Clone, Serialize, Deserialize, Doc)]
pub struct LanDiscoveryConfig {
    /// ```autodoc
    /// <Name>qexed.crates.server.config.LanDiscoveryConfig.enable</Name>
    /// <Default>false</Default>
    /// ```
    #[serde(default)]
    pub enable: bool,
    /// ```autodoc
    /// <Name>qexed.crates.server.config.LanDiscoveryConfig.motd</Name>
    /// <Default></Default>
    /// ```
    #[serde(default)]
    pub motd: String,
}

impl Default for LanDiscoveryConfig {
    fn default() -> Self {
        Self {
            enable: false,
            motd: String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use qexed_config::Config;

    #[test]
    fn server_config_defaults() {
        let config = ServerConfig::default();
        assert_eq!(config.max_player, -1);
        assert!(config.online_mode);
        assert_eq!(config.proxy_protocol, ForwardingMode::None);
        assert_eq!(ServerConfig::PATH, "/");
        assert_eq!(ServerConfig::NAME, "server");
    }

    #[test]
    fn app_config_constants_are_stable() {
        assert_eq!(WardenConfig::NAME, "warden");
        assert_eq!(WorldClusterConfig::NAME, "world_cluster");
        assert_eq!(LanDiscoveryConfig::NAME, "lan_discovery");
        // api_token 声明了 <Secret />：应进入 SECRETS。
        assert!(ContentFilterConfig::SECRETS.contains(&"api_token"));
    }
}

// ------- 资源包配置（v4 迁移）-------
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct ResourcePack {
    #[serde(default)]
    pub enable: bool,

    #[serde(default)]
    pub source: ResourcePackSource,

    #[serde(default = "default_resource_pack_id")]
    pub id: uuid::Uuid,

    #[serde(default)]
    pub url: String,

    #[serde(default = "default_resource_pack_path")]
    pub path: String,

    #[serde(default = "default_resource_pack_download_bind")]
    pub download_bind: String,

    #[serde(default)]
    pub download_host: String,

    #[serde(default)]
    pub object_storage: ResourcePackObjectStorage,

    #[serde(default)]
    pub hash: String,

    #[serde(default)]
    pub required: bool,

    #[serde(default)]
    pub prompt: String,

    #[serde(default = "default_resource_pack_disconnect_message")]
    pub disconnect_message: String,
}

impl Default for ResourcePack {
    fn default() -> Self {
        Self {
            enable: false,
            source: ResourcePackSource::default(),
            id: default_resource_pack_id(),
            url: String::new(),
            path: default_resource_pack_path(),
            download_bind: default_resource_pack_download_bind(),
            download_host: String::new(),
            object_storage: ResourcePackObjectStorage::default(),
            hash: String::new(),
            required: false,
            prompt: String::new(),
            disconnect_message: default_resource_pack_disconnect_message(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourcePackSource {
    #[default]
    Url,
    Local,
    ObjectStorage,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct ResourcePackObjectStorage {
    #[serde(default)]
    pub provider: ResourcePackObjectStorageProvider,

    #[serde(default)]
    pub public_base_url: String,

    #[serde(default)]
    pub endpoint: String,

    #[serde(default)]
    pub bucket: String,

    #[serde(default = "default_resource_pack_object_key")]
    pub object_key: String,

    #[serde(default)]
    pub force_path_style: bool,
}

impl Default for ResourcePackObjectStorage {
    fn default() -> Self {
        Self {
            provider: ResourcePackObjectStorageProvider::default(),
            public_base_url: String::new(),
            endpoint: String::new(),
            bucket: String::new(),
            object_key: default_resource_pack_object_key(),
            force_path_style: false,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourcePackObjectStorageProvider {
    #[default]
    Generic,
    TencentCos,
    TencentEo,
    HuaweiObs,
    HuaweiCdn,
    AliyunOss,
    AwsS3,
}

fn default_resource_pack_id() -> uuid::Uuid {
    uuid::Uuid::from_u128(0x11111111_2222_3333_4444_555555555555)
}

fn default_resource_pack_path() -> String {
    "resourcepacks/server.zip".to_string()
}

fn default_resource_pack_object_key() -> String {
    "resourcepacks/server.zip".to_string()
}

fn default_resource_pack_download_bind() -> String {
    "0.0.0.0:25566".to_string()
}

fn default_resource_pack_disconnect_message() -> String {
    "This server requires its resource pack.".to_string()
}

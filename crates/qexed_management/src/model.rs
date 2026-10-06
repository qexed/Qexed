//! 协议数据模型（对齐官方 Schema 段）。

use serde::{Deserialize, Serialize};

/// 玩家引用（id 或 name 至少其一）。
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct PlayerRef {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

impl PlayerRef {
    pub fn by_id(id: impl Into<String>) -> Self {
        Self { id: Some(id.into()), name: None }
    }

    pub fn by_name(name: impl Into<String>) -> Self {
        Self { id: None, name: Some(name.into()) }
    }
}

/// 难度。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Difficulty {
    Peaceful,
    Easy,
    Normal,
    Hard,
}

impl Difficulty {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Peaceful => "peaceful",
            Self::Easy => "easy",
            Self::Normal => "normal",
            Self::Hard => "hard",
        }
    }
}

/// 游戏模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GameMode {
    Survival,
    Creative,
    Adventure,
    Spectator,
}

impl GameMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Survival => "survival",
            Self::Creative => "creative",
            Self::Adventure => "adventure",
            Self::Spectator => "spectator",
        }
    }
}

/// 可翻译消息（translatable 优先于 literal）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Message {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub translatable: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub translatable_params: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub literal: Option<String>,
}

impl Message {
    pub fn literal(text: impl Into<String>) -> Self {
        Self { literal: Some(text.into()), ..Default::default() }
    }

    pub fn translatable(key: impl Into<String>, params: Vec<String>) -> Self {
        Self {
            translatable: Some(key.into()),
            translatable_params: if params.is_empty() { None } else { Some(params) },
            literal: None,
        }
    }
}

/// 用户封禁。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UserBan {
    pub player: PlayerRef,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// ISO-8601；None = 永久。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires: Option<String>,
}

/// IP 封禁。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IpBan {
    pub ip: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires: Option<String>,
}

/// 踢出请求。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KickRequest {
    pub player: PlayerRef,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<Message>,
}

/// 操作员。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperatorEntry {
    pub player: PlayerRef,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permission_level: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bypasses_player_limit: Option<bool>,
}

/// 白名单条目（Player 形态）。
pub type AllowlistEntry = PlayerRef;

/// 服务器版本。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionInfo {
    pub name: String,
    pub protocol: i32,
}

/// 服务器状态。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerState {
    pub started: bool,
    pub players: Vec<PlayerRef>,
    pub version: VersionInfo,
}

/// 系统消息（聊天/动作栏）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMessage {
    pub message: Message,
    /// false = 聊天；true = 动作栏。
    pub overlay: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub receiving_players: Option<Vec<PlayerRef>>,
}

/// 类型化游戏规则。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypedGameRule {
    pub key: String,
    /// "integer" | "boolean"
    #[serde(rename = "type")]
    pub rule_type: String,
    pub value: serde_json::Value,
}

/// 未类型化游戏规则（更新请求用）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UntypedGameRule {
    pub key: String,
    pub value: serde_json::Value,
}

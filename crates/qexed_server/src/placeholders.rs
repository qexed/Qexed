//! 占位符渲染：%online_players% / %player_name% / {servers} …
//! 迁移自 v4 crates/qexed/src/placeholders.rs。
//!
//! v4 依赖 qexed_plugin_api::NATIVE_PLACEHOLDER_DOCS 与 crate::plugins::PluginManager；
//! v6 里 qexed_plugins 还是空壳，这里把原生的 PlaceholderDoc/PlaceholderScope 定义
//! 收进本模块，插件侧占位符通过 PlaceholderProvider trait 注入（qexed_plugins
//! 填充实现后即可无缝接回）。

/// 占位符作用域（v4 qexed_plugin_api::placeholders::PlaceholderScope）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaceholderScope {
    Global,
    Lobby,
    Player,
}

impl PlaceholderScope {
    pub const fn key(self) -> &'static str {
        match self {
            Self::Global => "global",
            Self::Lobby => "lobby",
            Self::Player => "player",
        }
    }
}

/// 占位符文档（v4 qexed_plugin_api::placeholders::PlaceholderDoc）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlaceholderDoc {
    pub token: &'static str,
    pub aliases: &'static [&'static str],
    pub scope: PlaceholderScope,
    pub since: &'static str,
    pub example: &'static str,
    pub zh: &'static str,
    pub en: &'static str,
}

/// 原生占位符文档表（v4 NATIVE_PLACEHOLDER_DOCS 原样迁移）。
pub const NATIVE_PLACEHOLDER_DOCS: &[PlaceholderDoc] = &[
    PlaceholderDoc {
        token: "%online_players%",
        aliases: &[],
        scope: PlaceholderScope::Global,
        since: "0.1.0",
        example: "12",
        zh: "当前在线玩家数量。",
        en: "Current online player count.",
    },
    PlaceholderDoc {
        token: "%max_players%",
        aliases: &[],
        scope: PlaceholderScope::Global,
        since: "0.1.0",
        example: "100",
        zh: "服务器最大玩家数量；配置为无限制时显示为本地化文本。",
        en: "Maximum player count; renders as localized text when the server is unlimited.",
    },
    PlaceholderDoc {
        token: "%lobby_online_servers%",
        aliases: &["{online_servers}"],
        scope: PlaceholderScope::Lobby,
        since: "0.1.0",
        example: "3",
        zh: "大厅后端中当前在线的服务器数量。",
        en: "Number of currently online lobby backend servers.",
    },
    PlaceholderDoc {
        token: "%lobby_total_servers%",
        aliases: &["{total_servers}"],
        scope: PlaceholderScope::Lobby,
        since: "0.1.0",
        example: "5",
        zh: "大厅配置中的后端服务器总数。",
        en: "Total number of backend servers configured for the lobby.",
    },
    PlaceholderDoc {
        token: "%lobby_servers%",
        aliases: &["{servers}"],
        scope: PlaceholderScope::Lobby,
        since: "0.1.0",
        example: "survival, prison",
        zh: "大厅后端服务器状态摘要文本。",
        en: "Lobby backend server status summary text.",
    },
    PlaceholderDoc {
        token: "%player_name%",
        aliases: &[],
        scope: PlaceholderScope::Player,
        since: "0.1.0",
        example: "Steve",
        zh: "当前玩家名称；仅在有玩家上下文的文本中可用。",
        en: "Current player name; only available when player context exists.",
    },
    PlaceholderDoc {
        token: "%player_uuid%",
        aliases: &[],
        scope: PlaceholderScope::Player,
        since: "0.1.0",
        example: "00000000-0000-0000-0000-000000000000",
        zh: "当前玩家 UUID；仅在有玩家上下文的文本中可用。",
        en: "Current player UUID; only available when player context exists.",
    },
    PlaceholderDoc {
        token: "%player_language%",
        aliases: &[],
        scope: PlaceholderScope::Player,
        since: "0.1.0",
        example: "zh-CN",
        zh: "当前玩家客户端语言；仅在有玩家上下文的文本中可用。",
        en: "Current player's client language; only available when player context exists.",
    },
    PlaceholderDoc {
        token: "%player_dimension%",
        aliases: &[],
        scope: PlaceholderScope::Player,
        since: "0.1.0",
        example: "minecraft:overworld",
        zh: "当前玩家所在维度；仅在有玩家上下文的文本中可用。",
        en: "Current player's dimension; only available when player context exists.",
    },
];

pub fn native_placeholder_docs() -> &'static [PlaceholderDoc] {
    NATIVE_PLACEHOLDER_DOCS
}

/// 插件占位符查询入参（v4 qexed::plugins::PlaceholderQuery 的 server 侧视图）。
#[derive(Debug, Clone)]
pub struct PlaceholderQuery {
    pub player: Option<PlayerPayloadOwned>,
    pub text: String,
    pub context: Vec<PlaceholderContextEntry>,
}

/// 玩家上下文快照（v4 qexed::plugins::PlayerPayloadOwned）。
#[derive(Debug, Clone)]
pub struct PlayerPayloadOwned {
    pub uuid: String,
    pub username: String,
    pub entity_id: i32,
    pub language: String,
    pub dimension: String,
}

/// 键值上下文条目（v4 qexed::plugins::PlaceholderContext）。
#[derive(Debug, Clone)]
pub struct PlaceholderContextEntry {
    pub key: String,
    pub value: String,
}

/// 插件占位符替换结果（v4 qexed::plugins::PlaceholderReplacement）。
#[derive(Debug, Clone)]
pub struct PlaceholderReplacement {
    pub key: String,
    pub value: String,
}

/// 插件占位符提供方：v4 里由 PluginManager::placeholder_replacements 实现。
/// qexed_plugins 落地后实现此 trait 并传入 format_placeholders 即可。
pub trait PlaceholderProvider {
    fn placeholder_replacements(&self, query: PlaceholderQuery) -> Vec<PlaceholderReplacement>;
}

/// 无插件时的空提供方。
#[derive(Debug, Default, Clone, Copy)]
pub struct NoPlaceholders;

impl PlaceholderProvider for NoPlaceholders {
    fn placeholder_replacements(&self, _query: PlaceholderQuery) -> Vec<PlaceholderReplacement> {
        Vec::new()
    }
}

/// 占位符渲染的玩家视图（v4 qexed::players::OnlinePlayer 的最小投影）。
#[derive(Debug, Clone)]
pub struct PlaceholderPlayer {
    pub uuid: uuid::Uuid,
    pub username: String,
    pub entity_id: i32,
    pub language: String,
    pub dimension: String,
}

#[derive(Debug, Clone)]
pub struct PlaceholderContext {
    pub online_players: usize,
    pub max_players: i32,
    pub lobby_online_servers: usize,
    pub lobby_total_servers: usize,
    pub lobby_servers: String,
}

impl PlaceholderContext {
    pub fn max_players_label(&self, player: Option<&PlaceholderPlayer>) -> String {
        if self.max_players < 0 {
            unlimited_label(player)
        } else {
            self.max_players.to_string()
        }
    }
}

/// 完整占位符渲染：原生占位符 + 插件占位符。
pub fn format_placeholders(
    enabled: bool,
    plugins: &dyn PlaceholderProvider,
    player: Option<&PlaceholderPlayer>,
    text: &str,
    context: &PlaceholderContext,
) -> String {
    if !enabled {
        return text.to_string();
    }

    let mut rendered = apply_native_placeholders(player, text, context);
    let query = PlaceholderQuery {
        player: player.map(player_payload_owned),
        text: rendered.clone(),
        context: vec![
            context_entry("online_players", context.online_players.to_string()),
            context_entry("max_players", context.max_players_label(player)),
            context_entry(
                "lobby_online_servers",
                context.lobby_online_servers.to_string(),
            ),
            context_entry(
                "lobby_total_servers",
                context.lobby_total_servers.to_string(),
            ),
            context_entry("lobby_servers", context.lobby_servers.clone()),
        ],
    };
    for replacement in plugins.placeholder_replacements(query) {
        let key = replacement.key.trim();
        if key.is_empty() {
            continue;
        }
        rendered = rendered.replace(&placeholder_token(key), &replacement.value);
        rendered = rendered.replace(&format!("{{{key}}}"), &replacement.value);
    }
    rendered
}

/// 只渲染原生占位符。
pub fn apply_native_placeholders(
    player: Option<&PlaceholderPlayer>,
    text: &str,
    context: &PlaceholderContext,
) -> String {
    let mut rendered = text.to_string();
    for doc in NATIVE_PLACEHOLDER_DOCS {
        let Some(value) = native_placeholder_value(doc.token, doc.scope, player, context) else {
            continue;
        };
        rendered = rendered.replace(doc.token, &value);
        for alias in doc.aliases {
            rendered = rendered.replace(alias, &value);
        }
    }
    rendered
}

fn native_placeholder_value(
    token: &str,
    scope: PlaceholderScope,
    player: Option<&PlaceholderPlayer>,
    context: &PlaceholderContext,
) -> Option<String> {
    if scope == PlaceholderScope::Player && player.is_none() {
        return None;
    }

    let value = match token {
        "%online_players%" => context.online_players.to_string(),
        "%max_players%" => context.max_players_label(player),
        "%lobby_online_servers%" => context.lobby_online_servers.to_string(),
        "%lobby_total_servers%" => context.lobby_total_servers.to_string(),
        "%lobby_servers%" => context.lobby_servers.clone(),
        "%player_name%" => player
            .map(|player| player.username.clone())
            .unwrap_or_default(),
        "%player_uuid%" => player
            .map(|player| player.uuid.to_string())
            .unwrap_or_default(),
        "%player_language%" => player
            .map(|player| player.language.clone())
            .unwrap_or_default(),
        "%player_dimension%" => player
            .map(|player| player.dimension.clone())
            .unwrap_or_default(),
        _ => return None,
    };
    Some(value)
}

fn player_payload_owned(player: &PlaceholderPlayer) -> PlayerPayloadOwned {
    PlayerPayloadOwned {
        uuid: player.uuid.to_string(),
        username: player.username.clone(),
        entity_id: player.entity_id,
        language: player.language.clone(),
        dimension: player.dimension.clone(),
    }
}

fn context_entry(
    key: impl Into<String>,
    value: impl Into<String>,
) -> PlaceholderContextEntry {
    PlaceholderContextEntry {
        key: key.into(),
        value: value.into(),
    }
}

fn placeholder_token(key: &str) -> String {
    if key.starts_with('%') && key.ends_with('%') {
        key.to_string()
    } else {
        format!("%{key}%")
    }
}

fn unlimited_label(player: Option<&PlaceholderPlayer>) -> String {
    let language = player
        .map(|player| player.language.as_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .replace('-', "_");
    if language.starts_with("zh") {
        "无上限".to_string()
    } else {
        "unlimited".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::{NoPlaceholders, PlaceholderContext, PlaceholderPlayer, apply_native_placeholders, format_placeholders};

    #[test]
    fn native_placeholders_render_counts_and_lobby_status() {
        let rendered = apply_native_placeholders(
            None,
            "Online %online_players%/%max_players% Backends {online_servers}/{total_servers}",
            &PlaceholderContext {
                online_players: 2,
                max_players: -1,
                lobby_online_servers: 1,
                lobby_total_servers: 3,
                lobby_servers: "Lobby".to_string(),
            },
        );

        assert_eq!(rendered, "Online 2/unlimited Backends 1/3");
    }

    #[test]
    fn native_placeholders_render_unlimited_by_player_language() {
        let player = test_player("zh_cn");
        let rendered = apply_native_placeholders(
            Some(&player),
            "在线 %online_players%/%max_players%",
            &PlaceholderContext {
                online_players: 2,
                max_players: -1,
                lobby_online_servers: 1,
                lobby_total_servers: 1,
                lobby_servers: "跑路谷".to_string(),
            },
        );

        assert_eq!(rendered, "在线 2/无上限");
    }

    #[test]
    fn format_placeholders_passthrough_when_disabled() {
        let context = PlaceholderContext {
            online_players: 1,
            max_players: 10,
            lobby_online_servers: 0,
            lobby_total_servers: 0,
            lobby_servers: String::new(),
        };
        assert_eq!(
            format_placeholders(false, &NoPlaceholders, None, "%online_players%", &context),
            "%online_players%"
        );
        assert_eq!(
            format_placeholders(true, &NoPlaceholders, None, "%online_players%", &context),
            "1"
        );
    }

    fn test_player(language: &str) -> PlaceholderPlayer {
        PlaceholderPlayer {
            uuid: uuid::Uuid::nil(),
            username: "Tester".to_string(),
            entity_id: 1,
            language: language.to_string(),
            dimension: "minecraft:overworld".to_string(),
        }
    }
}

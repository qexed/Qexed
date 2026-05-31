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

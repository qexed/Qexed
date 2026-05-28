use crate::{players::OnlinePlayer, plugins::PluginManager};

#[derive(Debug, Clone)]
pub struct PlaceholderContext {
    pub online_players: usize,
    pub max_players: i32,
    pub lobby_online_servers: usize,
    pub lobby_total_servers: usize,
    pub lobby_servers: String,
}

impl PlaceholderContext {
    pub fn max_players_label(&self) -> String {
        if self.max_players < 0 {
            "unlimited".to_string()
        } else {
            self.max_players.to_string()
        }
    }
}

pub fn format_placeholders(
    enabled: bool,
    plugins: &PluginManager,
    player: Option<&OnlinePlayer>,
    text: &str,
    context: &PlaceholderContext,
) -> String {
    if !enabled {
        return text.to_string();
    }

    let mut rendered = apply_native_placeholders(player, text, context);
    let query = crate::plugins::PlaceholderQuery {
        player: player.map(player_payload_owned),
        text: rendered.clone(),
        context: vec![
            context_entry("online_players", context.online_players.to_string()),
            context_entry("max_players", context.max_players_label()),
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

pub fn apply_native_placeholders(
    player: Option<&OnlinePlayer>,
    text: &str,
    context: &PlaceholderContext,
) -> String {
    let mut rendered = text
        .replace("%online_players%", &context.online_players.to_string())
        .replace("%max_players%", &context.max_players_label())
        .replace(
            "%lobby_online_servers%",
            &context.lobby_online_servers.to_string(),
        )
        .replace(
            "%lobby_total_servers%",
            &context.lobby_total_servers.to_string(),
        )
        .replace("%lobby_servers%", &context.lobby_servers)
        .replace(
            "{online_servers}",
            &context.lobby_online_servers.to_string(),
        )
        .replace("{total_servers}", &context.lobby_total_servers.to_string())
        .replace("{servers}", &context.lobby_servers);
    if let Some(player) = player {
        rendered = rendered
            .replace("%player_name%", &player.profile.username)
            .replace("%player_uuid%", &player.profile.uuid.to_string())
            .replace("%player_language%", &player.language)
            .replace("%player_dimension%", &player.dimension);
    }
    rendered
}

fn player_payload_owned(player: &OnlinePlayer) -> crate::plugins::PlayerPayloadOwned {
    crate::plugins::PlayerPayloadOwned {
        uuid: player.profile.uuid.to_string(),
        username: player.profile.username.clone(),
        entity_id: player.entity_id,
        language: player.language.clone(),
        dimension: player.dimension.clone(),
    }
}

fn context_entry(
    key: impl Into<String>,
    value: impl Into<String>,
) -> crate::plugins::PlaceholderContext {
    crate::plugins::PlaceholderContext {
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

#[cfg(test)]
mod tests {
    use super::{PlaceholderContext, apply_native_placeholders};

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
}

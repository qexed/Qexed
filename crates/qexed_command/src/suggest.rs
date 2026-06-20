use crate::catalog::{command_names_for_suggestions, normalize_command_name};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandSuggestionMatches {
    pub start: usize,
    pub length: usize,
    pub values: Vec<String>,
}

pub fn command_suggestion_matches(text: &str) -> CommandSuggestionMatches {
    command_suggestion_matches_with_candidates(text, &[])
}

pub fn command_suggestion_matches_with_candidates(
    text: &str,
    extra_candidates: &[String],
) -> CommandSuggestionMatches {
    command_suggestion_matches_with_sources(text, &[], extra_candidates)
}

pub fn command_suggestion_matches_with_sources(
    text: &str,
    online_players: &[String],
    server_candidates: &[String],
) -> CommandSuggestionMatches {
    let raw_token_start = text
        .char_indices()
        .rev()
        .find_map(|(index, ch)| ch.is_whitespace().then_some(index + ch.len_utf8()))
        .unwrap_or(0);
    let token_start = if raw_token_start == 0 && text.starts_with('/') {
        1
    } else {
        raw_token_start
    };
    let prefix = &text[token_start..];
    let lower_prefix = prefix.trim_start_matches('/').to_ascii_lowercase();
    let mut values = command_suggestion_candidates(text, online_players, server_candidates)
        .into_iter()
        .filter(|candidate| {
            candidate
                .trim_start_matches('/')
                .to_ascii_lowercase()
                .starts_with(&lower_prefix)
        })
        .collect::<Vec<_>>();
    values.sort();
    values.dedup();
    CommandSuggestionMatches {
        start: token_start,
        length: text.len().saturating_sub(token_start),
        values,
    }
}

fn command_suggestion_candidates(
    text: &str,
    online_players: &[String],
    server_candidates: &[String],
) -> Vec<String> {
    let trimmed = text.trim_start_matches('/').trim_start();
    let has_argument_separator = trimmed.chars().any(char::is_whitespace);
    let mut parts = trimmed.split_whitespace();
    if trimmed.is_empty() || !has_argument_separator {
        let mut commands = command_names_for_suggestions()
            .into_iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        commands.extend(server_candidates.iter().cloned());
        return commands;
    }

    let command = parts.next().map(normalize_command_name);
    let arguments = parts.collect::<Vec<_>>();
    let argument_index = if text.chars().last().is_some_and(char::is_whitespace) {
        arguments.len()
    } else {
        arguments.len().saturating_sub(1)
    };

    match command.as_deref() {
        None | Some("") => Vec::new(),
        Some("help") => command_names_for_suggestions()
            .into_iter()
            .map(ToString::to_string)
            .collect(),
        Some("teleport") => teleport_suggestions(online_players),
        Some("gamemode") if argument_index == 0 => gamemode_suggestions(),
        Some("gamemode") => online_players.to_vec(),
        Some("give") if argument_index == 0 => {
            let mut values = online_players.to_vec();
            values.extend(common_item_suggestions());
            values
        }
        Some("give") => common_item_suggestions(),
        Some("server") => server_candidates.to_vec(),
        Some("entity") => ["list", "spawn", "move", "remove", "npc", "hologram"]
            .into_iter()
            .map(ToString::to_string)
            .collect(),
        Some("scoreboard") => [
            "objectives",
            "players",
            "sidebar",
            "list",
            "add",
            "remove",
            "setdisplay",
            "set",
            "reset",
            "on",
            "off",
            "reload",
        ]
        .into_iter()
        .map(ToString::to_string)
        .collect(),
        Some("time") if argument_index == 0 => ["set", "add", "query"]
            .into_iter()
            .map(ToString::to_string)
            .collect(),
        Some("time") if argument_index == 1 => default_dimension_suggestions(),
        Some("gamerule") if argument_index == 0 => default_dimension_suggestions(),
        Some("gamerule") if argument_index == 1 => gamerule_suggestions(),
        Some("gamerule") => gamerule_value_suggestions(arguments.get(1).copied()),
        _ => Vec::new(),
    }
}

fn teleport_suggestions(online_players: &[String]) -> Vec<String> {
    let mut values = online_players.to_vec();
    values.extend(default_dimension_suggestions());
    values
}

fn gamemode_suggestions() -> Vec<String> {
    ["survival", "creative", "adventure", "spectator"]
        .into_iter()
        .map(ToString::to_string)
        .collect()
}

fn gamerule_suggestions() -> Vec<String> {
    [
        "doDaylightCycle",
        "doBlockUpdates",
        "doWorldReadOnly",
        "timeTickStep",
        "fixedTime",
        "worldLight",
    ]
    .into_iter()
    .map(ToString::to_string)
    .collect()
}

fn gamerule_value_suggestions(rule: Option<&str>) -> Vec<String> {
    match rule {
        Some("doDaylightCycle" | "doBlockUpdates" | "doWorldReadOnly") => ["true", "false"]
            .into_iter()
            .map(ToString::to_string)
            .collect(),
        Some("fixedTime") => vec!["none".to_string()],
        Some("worldLight") => ["static", "dynamic"]
            .into_iter()
            .map(ToString::to_string)
            .collect(),
        _ => Vec::new(),
    }
}

fn common_item_suggestions() -> Vec<String> {
    [
        "minecraft:stone",
        "minecraft:dirt",
        "minecraft:grass_block",
        "minecraft:oak_planks",
        "minecraft:diamond",
    ]
    .into_iter()
    .map(ToString::to_string)
    .collect()
}

fn default_dimension_suggestions() -> Vec<String> {
    vec![
        "minecraft:overworld".to_string(),
        "minecraft:the_nether".to_string(),
        "minecraft:the_end".to_string(),
    ]
}

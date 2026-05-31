use std::collections::HashSet;

use qexed_packet::net_types::VarInt;
use qexed_protocol::to_client::play::commands::{BrigadierString, Commands, Node, Varies};

pub fn command_tree() -> Commands {
    command_tree_for(builtin_command_literals())
}

pub fn command_tree_for(commands: &[&str]) -> Commands {
    command_tree_for_lobby(commands, &[])
}

pub fn command_tree_for_lobby(commands: &[&str], server_ids: &[String]) -> Commands {
    command_tree_for_lobby_with_extra(commands, server_ids, &[])
}

pub fn command_tree_for_lobby_with_extra(
    commands: &[&str],
    server_ids: &[String],
    extra_literals: &[String],
) -> Commands {
    let server_ids = server_command_literals(server_ids);
    let extra_literals = server_command_literals(extra_literals);
    let mut nodes = vec![Node {
        flags: 0x00,
        children: Vec::new(),
        ..Node::default()
    }];
    for command in commands {
        let index = if *command == "lobby" {
            let index = nodes.len() as i32;
            nodes.push(lobby_literal(index + 1, index + 2));
            nodes.push(executable_literal("status"));
            nodes.push(executable_literal("refresh"));
            index
        } else if *command == "server" {
            let index = nodes.len() as i32;
            nodes.push(server_literal(index + 1, index + 2, server_ids.len()));
            nodes.push(word_string_argument("target", true));
            for server_id in &server_ids {
                nodes.push(executable_literal(server_id));
            }
            index
        } else if *command == "teleport" {
            append_teleport_command_nodes(&mut nodes)
        } else if *command == "tp" {
            append_teleport_alias_command_nodes(&mut nodes)
        } else if *command == "scoreboard" {
            append_scoreboard_command_nodes(&mut nodes)
        } else if *command == "entity" {
            append_entity_command_nodes(&mut nodes)
        } else if *command == "npc" {
            append_npc_command_nodes(&mut nodes)
        } else if *command == "structure" {
            append_structure_command_nodes(&mut nodes)
        } else {
            let index = nodes.len() as i32;
            nodes.push(literal_with_optional_greedy_argument(
                command,
                (nodes.len() + 1) as i32,
            ));
            nodes.push(greedy_string_argument("args"));
            index
        };
        nodes[0].children.push(VarInt(index));
    }
    for literal in extra_literals {
        if commands.contains(&literal.as_str()) {
            continue;
        }
        let index = nodes.len() as i32;
        nodes.push(literal_with_optional_greedy_argument(
            &literal,
            (nodes.len() + 1) as i32,
        ));
        nodes.push(greedy_string_argument("args"));
        nodes[0].children.push(VarInt(index));
    }

    Commands {
        nodes,
        root_index: VarInt(0),
    }
}

fn server_command_literals(server_ids: &[String]) -> Vec<String> {
    let mut ids = server_ids
        .iter()
        .map(|id| id.trim())
        .filter(|id| !id.is_empty() && !id.chars().any(char::is_whitespace))
        .map(str::to_string)
        .collect::<Vec<_>>();
    ids.sort();
    ids.dedup();
    ids
}

pub async fn visible_commands(
    permissions: &crate::permissions::PermissionManager,
    profile: &qexed_packet::net_types::GameProfile,
) -> anyhow::Result<Vec<&'static str>> {
    let mut commands = Vec::new();
    let mut seen = HashSet::new();
    for command in builtin_command_literals().iter().copied() {
        if !seen.insert(command) {
            continue;
        }
        if permissions.can_run_command(profile, command).await? {
            commands.push(command);
        }
    }
    commands.sort_unstable();
    Ok(commands)
}

pub fn is_known_vanilla_command(command: &str) -> bool {
    let command = command.trim().trim_start_matches('/').to_ascii_lowercase();
    if command.is_empty() {
        return false;
    }
    vanilla_command_literals()
        .iter()
        .any(|candidate| *candidate == command)
}

pub fn normalize_command_name(command: &str) -> String {
    match command
        .trim()
        .trim_start_matches('/')
        .to_ascii_lowercase()
        .as_str()
    {
        "tp" => "teleport".to_string(),
        "w" | "tell" => "msg".to_string(),
        "xp" => "experience".to_string(),
        name => name.to_string(),
    }
}

pub fn permission_node(command: &str) -> Option<String> {
    let name = command
        .trim()
        .trim_start_matches('/')
        .split_ascii_whitespace()
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let name = normalize_command_name(&name);
    (!name.is_empty()).then(|| format!("qexed.command.{name}"))
}

pub fn command_names_for_suggestions() -> Vec<&'static str> {
    let mut commands = builtin_command_literals().to_vec();
    commands.sort_unstable();
    commands.dedup();
    commands
}

fn executable_literal(name: &str) -> Node {
    Node {
        flags: 0x01 | 0x04,
        children: Vec::new(),
        name: Some(name.to_string()),
        ..Node::default()
    }
}

fn literal_with_optional_greedy_argument(name: &str, argument_index: i32) -> Node {
    Node {
        flags: 0x01 | 0x04,
        children: vec![VarInt(argument_index)],
        name: Some(name.to_string()),
        ..Node::default()
    }
}

fn lobby_literal(status_index: i32, refresh_index: i32) -> Node {
    Node {
        flags: 0x01 | 0x04,
        children: vec![VarInt(status_index), VarInt(refresh_index)],
        name: Some("lobby".to_string()),
        ..Node::default()
    }
}

fn server_literal(argument_index: i32, literal_start_index: i32, literal_count: usize) -> Node {
    let mut children = vec![VarInt(argument_index)];
    children.extend(
        (0..literal_count)
            .map(|offset| VarInt(literal_start_index + i32::try_from(offset).unwrap_or(0))),
    );
    Node {
        flags: 0x01 | 0x04,
        children,
        name: Some("server".to_string()),
        ..Node::default()
    }
}

fn append_teleport_alias_command_nodes(nodes: &mut Vec<Node>) -> i32 {
    let teleport_index = append_teleport_command_nodes(nodes);
    nodes[teleport_index as usize].name = Some("tp".to_string());
    teleport_index
}

fn append_teleport_command_nodes(nodes: &mut Vec<Node>) -> i32 {
    let root = nodes.len() as i32;
    nodes.push(Node {
        flags: 0x01,
        children: Vec::new(),
        name: Some("teleport".to_string()),
        ..Node::default()
    });

    let destination_player = nodes.len() as i32;
    nodes.push(game_profile_argument("destination", true));
    let target = nodes.len() as i32;
    nodes.push(game_profile_argument("target", false));
    let destination_dimension = nodes.len() as i32;
    nodes.push(dimension_argument("dimension", false));

    let target_destination_player = nodes.len() as i32;
    nodes.push(game_profile_argument("destination", true));
    let target_dimension = nodes.len() as i32;
    nodes.push(dimension_argument("dimension", false));

    let pos = nodes.len() as i32;
    nodes.push(vec3_argument("location", true));
    let pos_rotation = nodes.len() as i32;
    nodes.push(rotation_argument("rotation", true));

    let target_pos = nodes.len() as i32;
    nodes.push(vec3_argument("location", true));
    let target_pos_rotation = nodes.len() as i32;
    nodes.push(rotation_argument("rotation", true));

    let dimension_pos = nodes.len() as i32;
    nodes.push(vec3_argument("location", true));
    let dimension_pos_rotation = nodes.len() as i32;
    nodes.push(rotation_argument("rotation", true));

    let target_dimension_pos = nodes.len() as i32;
    nodes.push(vec3_argument("location", true));
    let target_dimension_pos_rotation = nodes.len() as i32;
    nodes.push(rotation_argument("rotation", true));

    nodes[root as usize].children = vec![
        VarInt(destination_player),
        VarInt(target),
        VarInt(destination_dimension),
        VarInt(pos),
    ];
    nodes[target as usize].children = vec![
        VarInt(target_destination_player),
        VarInt(target_dimension),
        VarInt(target_pos),
    ];
    nodes[pos as usize].children = vec![VarInt(pos_rotation)];
    nodes[target_pos as usize].children = vec![VarInt(target_pos_rotation)];
    nodes[destination_dimension as usize].children = vec![VarInt(dimension_pos)];
    nodes[dimension_pos as usize].children = vec![VarInt(dimension_pos_rotation)];
    nodes[target_dimension as usize].children = vec![VarInt(target_dimension_pos)];
    nodes[target_dimension_pos as usize].children = vec![VarInt(target_dimension_pos_rotation)];

    root
}

fn append_entity_command_nodes(nodes: &mut Vec<Node>) -> i32 {
    let entity_index = nodes.len() as i32;
    nodes.push(Node {
        flags: 0x01 | 0x04,
        children: Vec::new(),
        name: Some("entity".to_string()),
        ..Node::default()
    });

    let list_index = nodes.len() as i32;
    nodes.push(executable_literal("list"));

    let spawn_index = nodes.len() as i32;
    nodes.push(Node {
        flags: 0x01 | 0x04,
        children: Vec::new(),
        name: Some("spawn".to_string()),
        ..Node::default()
    });

    let move_index = nodes.len() as i32;
    nodes.push(Node {
        flags: 0x01 | 0x04,
        children: Vec::new(),
        name: Some("move".to_string()),
        ..Node::default()
    });

    let remove_index = nodes.len() as i32;
    nodes.push(Node {
        flags: 0x01 | 0x04,
        children: Vec::new(),
        name: Some("remove".to_string()),
        ..Node::default()
    });

    let spawn_entity_index = nodes.len() as i32;
    nodes.push(Node {
        flags: 0x01 | 0x04,
        children: Vec::new(),
        name: Some("entity".to_string()),
        ..Node::default()
    });
    let spawn_hologram_index = nodes.len() as i32;
    nodes.push(Node {
        flags: 0x01 | 0x04,
        children: Vec::new(),
        name: Some("hologram".to_string()),
        ..Node::default()
    });

    let spawn_params_index = nodes.len() as i32;
    nodes.push(greedy_string_argument("params"));

    let move_id_index = nodes.len() as i32;
    nodes.push(word_string_argument("id", true));
    let remove_id_index = nodes.len() as i32;
    nodes.push(word_string_argument("id", true));

    nodes[entity_index as usize].children = vec![
        VarInt(list_index),
        VarInt(spawn_index),
        VarInt(move_index),
        VarInt(remove_index),
    ];
    nodes[spawn_index as usize].children =
        vec![VarInt(spawn_entity_index), VarInt(spawn_hologram_index)];
    nodes[move_index as usize].children = vec![VarInt(move_id_index)];
    nodes[remove_index as usize].children = vec![VarInt(remove_id_index)];
    nodes[spawn_entity_index as usize].children = vec![VarInt(spawn_params_index)];
    nodes[spawn_hologram_index as usize].children = vec![VarInt(spawn_params_index)];

    entity_index
}

fn append_npc_command_nodes(nodes: &mut Vec<Node>) -> i32 {
    let npc_index = nodes.len() as i32;
    nodes.push(Node {
        flags: 0x01 | 0x04,
        children: Vec::new(),
        name: Some("npc".to_string()),
        ..Node::default()
    });

    let list_index = nodes.len() as i32;
    nodes.push(executable_literal("list"));

    let spawn_index = nodes.len() as i32;
    nodes.push(Node {
        flags: 0x01 | 0x04,
        children: Vec::new(),
        name: Some("spawn".to_string()),
        ..Node::default()
    });

    let move_index = nodes.len() as i32;
    nodes.push(Node {
        flags: 0x01 | 0x04,
        children: Vec::new(),
        name: Some("move".to_string()),
        ..Node::default()
    });

    let remove_index = nodes.len() as i32;
    nodes.push(Node {
        flags: 0x01 | 0x04,
        children: Vec::new(),
        name: Some("remove".to_string()),
        ..Node::default()
    });

    let spawn_params_index = nodes.len() as i32;
    nodes.push(greedy_string_argument("params"));
    let move_id_index = nodes.len() as i32;
    nodes.push(word_string_argument("id", true));
    let remove_id_index = nodes.len() as i32;
    nodes.push(word_string_argument("id", true));

    nodes[npc_index as usize].children = vec![
        VarInt(list_index),
        VarInt(spawn_index),
        VarInt(move_index),
        VarInt(remove_index),
    ];
    nodes[spawn_index as usize].children = vec![VarInt(spawn_params_index)];
    nodes[move_index as usize].children = vec![VarInt(move_id_index)];
    nodes[remove_index as usize].children = vec![VarInt(remove_id_index)];

    npc_index
}

fn append_structure_command_nodes(nodes: &mut Vec<Node>) -> i32 {
    let structure_index = nodes.len() as i32;
    nodes.push(Node {
        flags: 0x01 | 0x04,
        children: Vec::new(),
        name: Some("structure".to_string()),
        ..Node::default()
    });

    let list_index = nodes.len() as i32;
    nodes.push(executable_literal("list"));
    let place_index = nodes.len() as i32;
    nodes.push(Node {
        flags: 0x01 | 0x04,
        children: Vec::new(),
        name: Some("place".to_string()),
        ..Node::default()
    });
    let locate_index = nodes.len() as i32;
    nodes.push(Node {
        flags: 0x01 | 0x04,
        children: Vec::new(),
        name: Some("locate".to_string()),
        ..Node::default()
    });

    let place_id_index = nodes.len() as i32;
    nodes.push(word_string_argument("id", true));
    let locate_id_index = nodes.len() as i32;
    nodes.push(word_string_argument("id", true));
    let place_position_index = nodes.len() as i32;
    nodes.push(greedy_string_argument("position"));

    nodes[structure_index as usize].children = vec![
        VarInt(list_index),
        VarInt(place_index),
        VarInt(locate_index),
    ];
    nodes[place_index as usize].children = vec![VarInt(place_id_index)];
    nodes[locate_index as usize].children = vec![VarInt(locate_id_index)];
    nodes[place_id_index as usize].children = vec![VarInt(place_position_index)];

    structure_index
}

fn greedy_string_argument(name: &str) -> Node {
    Node {
        flags: 0x02 | 0x04,
        children: Vec::new(),
        name: Some(name.to_string()),
        parser_id: Some(VarInt(5)),
        properties: Some(Varies::BrigadierString(BrigadierString {
            behavior: VarInt(2),
        })),
        ..Node::default()
    }
}

fn word_string_argument(name: &str, executable: bool) -> Node {
    Node {
        flags: 0x02 | if executable { 0x04 } else { 0 },
        children: Vec::new(),
        name: Some(name.to_string()),
        parser_id: Some(VarInt(5)),
        properties: Some(Varies::BrigadierString(BrigadierString {
            behavior: VarInt(0),
        })),
        ..Node::default()
    }
}

fn game_profile_argument(name: &str, executable: bool) -> Node {
    Node {
        flags: 0x02 | if executable { 0x04 } else { 0 } | 0x10,
        children: Vec::new(),
        name: Some(name.to_string()),
        parser_id: Some(VarInt(7)),
        suggestions_type: Some("minecraft:ask_server".to_string()),
        ..Node::default()
    }
}

fn vec3_argument(name: &str, executable: bool) -> Node {
    Node {
        flags: 0x02 | if executable { 0x04 } else { 0 },
        children: Vec::new(),
        name: Some(name.to_string()),
        parser_id: Some(VarInt(10)),
        ..Node::default()
    }
}

fn rotation_argument(name: &str, executable: bool) -> Node {
    Node {
        flags: 0x02 | if executable { 0x04 } else { 0 },
        children: Vec::new(),
        name: Some(name.to_string()),
        parser_id: Some(VarInt(29)),
        ..Node::default()
    }
}

fn dimension_argument(name: &str, executable: bool) -> Node {
    Node {
        flags: 0x02 | if executable { 0x04 } else { 0 } | 0x10,
        children: Vec::new(),
        name: Some(name.to_string()),
        parser_id: Some(VarInt(41)),
        suggestions_type: Some("minecraft:ask_server".to_string()),
        ..Node::default()
    }
}

fn integer_argument(name: &str, executable: bool) -> Node {
    Node {
        flags: 0x02 | if executable { 0x04 } else { 0 },
        children: Vec::new(),
        name: Some(name.to_string()),
        parser_id: Some(VarInt(3)),
        ..Node::default()
    }
}

fn component_argument(name: &str, executable: bool) -> Node {
    Node {
        flags: 0x02 | if executable { 0x04 } else { 0 },
        children: Vec::new(),
        name: Some(name.to_string()),
        parser_id: Some(VarInt(18)),
        ..Node::default()
    }
}

fn append_scoreboard_command_nodes(nodes: &mut Vec<Node>) -> i32 {
    let root = nodes.len() as i32;
    nodes.push(Node {
        flags: 0x01,
        children: Vec::new(),
        name: Some("scoreboard".to_string()),
        ..Node::default()
    });

    let objectives = nodes.len() as i32;
    nodes.push(Node {
        flags: 0x01,
        children: Vec::new(),
        name: Some("objectives".to_string()),
        ..Node::default()
    });
    let players = nodes.len() as i32;
    nodes.push(Node {
        flags: 0x01,
        children: Vec::new(),
        name: Some("players".to_string()),
        ..Node::default()
    });
    let sidebar = nodes.len() as i32;
    nodes.push(Node {
        flags: 0x01,
        children: Vec::new(),
        name: Some("sidebar".to_string()),
        ..Node::default()
    });

    let objectives_list = nodes.len() as i32;
    nodes.push(executable_literal("list"));
    let objectives_add = nodes.len() as i32;
    nodes.push(Node {
        flags: 0x01,
        children: Vec::new(),
        name: Some("add".to_string()),
        ..Node::default()
    });
    let objectives_remove = nodes.len() as i32;
    nodes.push(Node {
        flags: 0x01,
        children: Vec::new(),
        name: Some("remove".to_string()),
        ..Node::default()
    });
    let objectives_setdisplay = nodes.len() as i32;
    nodes.push(Node {
        flags: 0x01,
        children: Vec::new(),
        name: Some("setdisplay".to_string()),
        ..Node::default()
    });

    let add_objective = nodes.len() as i32;
    nodes.push(word_string_argument("objective", false));
    let add_criteria = nodes.len() as i32;
    nodes.push(word_string_argument("criteria", true));
    let add_display = nodes.len() as i32;
    nodes.push(component_argument("displayName", true));
    let remove_objective = nodes.len() as i32;
    nodes.push(word_string_argument("objective", true));
    let display_slot = nodes.len() as i32;
    nodes.push(Node {
        flags: 0x01,
        children: Vec::new(),
        name: Some("sidebar".to_string()),
        ..Node::default()
    });
    let display_objective = nodes.len() as i32;
    nodes.push(word_string_argument("objective", true));

    let players_list = nodes.len() as i32;
    nodes.push(executable_literal("list"));
    let players_set = nodes.len() as i32;
    nodes.push(Node {
        flags: 0x01,
        children: Vec::new(),
        name: Some("set".to_string()),
        ..Node::default()
    });
    let players_add = nodes.len() as i32;
    nodes.push(Node {
        flags: 0x01,
        children: Vec::new(),
        name: Some("add".to_string()),
        ..Node::default()
    });
    let players_remove = nodes.len() as i32;
    nodes.push(Node {
        flags: 0x01,
        children: Vec::new(),
        name: Some("remove".to_string()),
        ..Node::default()
    });
    let players_reset = nodes.len() as i32;
    nodes.push(Node {
        flags: 0x01,
        children: Vec::new(),
        name: Some("reset".to_string()),
        ..Node::default()
    });

    let set_target = nodes.len() as i32;
    nodes.push(word_string_argument("targets", false));
    let set_objective = nodes.len() as i32;
    nodes.push(word_string_argument("objective", false));
    let set_score = nodes.len() as i32;
    nodes.push(integer_argument("score", true));
    let add_target = nodes.len() as i32;
    nodes.push(word_string_argument("targets", false));
    let add_objective_name = nodes.len() as i32;
    nodes.push(word_string_argument("objective", false));
    let add_score = nodes.len() as i32;
    nodes.push(integer_argument("score", true));
    let remove_target = nodes.len() as i32;
    nodes.push(word_string_argument("targets", false));
    let remove_objective_name = nodes.len() as i32;
    nodes.push(word_string_argument("objective", false));
    let remove_score = nodes.len() as i32;
    nodes.push(integer_argument("score", true));
    let reset_target = nodes.len() as i32;
    nodes.push(word_string_argument("targets", true));
    let reset_objective = nodes.len() as i32;
    nodes.push(word_string_argument("objective", true));

    let sidebar_on = nodes.len() as i32;
    nodes.push(executable_literal("on"));
    let sidebar_off = nodes.len() as i32;
    nodes.push(executable_literal("off"));
    let sidebar_reload = nodes.len() as i32;
    nodes.push(executable_literal("reload"));

    nodes[root as usize].children = vec![VarInt(objectives), VarInt(players), VarInt(sidebar)];
    nodes[objectives as usize].children = vec![
        VarInt(objectives_list),
        VarInt(objectives_add),
        VarInt(objectives_remove),
        VarInt(objectives_setdisplay),
    ];
    nodes[objectives_add as usize].children = vec![VarInt(add_objective)];
    nodes[add_objective as usize].children = vec![VarInt(add_criteria)];
    nodes[add_criteria as usize].children = vec![VarInt(add_display)];
    nodes[objectives_remove as usize].children = vec![VarInt(remove_objective)];
    nodes[objectives_setdisplay as usize].children = vec![VarInt(display_slot)];
    nodes[display_slot as usize].children = vec![VarInt(display_objective)];

    nodes[players as usize].children = vec![
        VarInt(players_list),
        VarInt(players_set),
        VarInt(players_add),
        VarInt(players_remove),
        VarInt(players_reset),
    ];
    nodes[players_set as usize].children = vec![VarInt(set_target)];
    nodes[set_target as usize].children = vec![VarInt(set_objective)];
    nodes[set_objective as usize].children = vec![VarInt(set_score)];
    nodes[players_add as usize].children = vec![VarInt(add_target)];
    nodes[add_target as usize].children = vec![VarInt(add_objective_name)];
    nodes[add_objective_name as usize].children = vec![VarInt(add_score)];
    nodes[players_remove as usize].children = vec![VarInt(remove_target)];
    nodes[remove_target as usize].children = vec![VarInt(remove_objective_name)];
    nodes[remove_objective_name as usize].children = vec![VarInt(remove_score)];
    nodes[players_reset as usize].children = vec![VarInt(reset_target)];
    nodes[reset_target as usize].children = vec![VarInt(reset_objective)];

    nodes[sidebar as usize].children = vec![
        VarInt(sidebar_on),
        VarInt(sidebar_off),
        VarInt(sidebar_reload),
    ];
    root
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandHelpEntry {
    pub usage: String,
    pub description: Option<String>,
}

impl CommandHelpEntry {
    fn new(usage: impl Into<String>, description: Option<String>) -> Self {
        Self {
            usage: usage.into(),
            description,
        }
    }
}

pub fn localized_builtin_help_entries(commands: &[&str], locale: &str) -> Vec<CommandHelpEntry> {
    let locale = i18n_locale(locale);
    let mut entries = commands
        .iter()
        .filter_map(|command| localized_builtin_help_entry(command, locale))
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| left.usage.cmp(&right.usage));
    entries
}

pub fn localized_plugin_help_entry(
    command: &str,
    description_key: &str,
    locale: &str,
) -> CommandHelpEntry {
    let locale = i18n_locale(locale);
    let description_key = description_key.trim();
    let description = if description_key.is_empty() {
        None
    } else {
        let translated = rust_i18n::t!(description_key, locale = locale).to_string();
        (translated != description_key).then_some(translated)
    };
    CommandHelpEntry::new(format!("/{command}"), description)
}

pub fn localized_server_help(locale: &str, entries: &[CommandHelpEntry]) -> String {
    localized_help(
        locale,
        "qexed.command.help.header",
        entries,
        None::<&[CommandHelpEntry]>,
    )
}

pub fn localized_console_help(locale: &str) -> String {
    let locale_key = i18n_locale(locale);
    let server_entries = localized_builtin_help_entries(builtin_command_literals(), locale_key);
    let console_entries = [
        CommandHelpEntry::new(
            "help, ?",
            Some(rust_i18n::t!("qexed.console.help.description", locale = locale_key).to_string()),
        ),
        CommandHelpEntry::new(
            "status",
            Some(
                rust_i18n::t!("qexed.console.status.description", locale = locale_key).to_string(),
            ),
        ),
        CommandHelpEntry::new(
            "list, players",
            Some(rust_i18n::t!("qexed.console.list.description", locale = locale_key).to_string()),
        ),
        CommandHelpEntry::new(
            "say <message>",
            Some(rust_i18n::t!("qexed.console.say.description", locale = locale_key).to_string()),
        ),
        CommandHelpEntry::new(
            "stop, exit, quit",
            Some(rust_i18n::t!("qexed.console.stop.description", locale = locale_key).to_string()),
        ),
    ];
    localized_help(
        locale_key,
        "qexed.command.help.console_header",
        &server_entries,
        Some(console_entries.as_slice()),
    )
}

pub fn localized_list(locale: &str, online: usize, max: i32, names: &[String]) -> String {
    let locale = i18n_locale(locale);
    rust_i18n::t!(
        "qexed.command.list.message",
        locale = locale,
        online = online,
        max = localized_max_label(locale, max),
        players = names.join(", ")
    )
    .to_string()
}

pub fn localized_unimplemented(locale: &str, command: &str) -> String {
    let locale = i18n_locale(locale);
    rust_i18n::t!(
        "qexed.command.unimplemented",
        locale = locale,
        command = command
    )
    .to_string()
}

pub fn localized_max_label(locale: &str, max: i32) -> String {
    if max < 0 {
        rust_i18n::t!(
            "qexed.command.max_players.unlimited",
            locale = i18n_locale(locale)
        )
        .to_string()
    } else {
        max.to_string()
    }
}

pub fn i18n_locale(locale: &str) -> &'static str {
    let normalized = locale.trim().replace('_', "-").to_ascii_lowercase();
    if normalized.starts_with("en") {
        "en"
    } else {
        "zh-CN"
    }
}

fn localized_help(
    locale: &str,
    header_key: &str,
    entries: &[CommandHelpEntry],
    console_entries: Option<&[CommandHelpEntry]>,
) -> String {
    let locale = i18n_locale(locale);
    let mut lines = vec![rust_i18n::t!(header_key, locale = locale).to_string()];
    lines.extend(entries.iter().map(localized_help_line));
    if let Some(console_entries) = console_entries {
        lines.push(String::new());
        lines.push(rust_i18n::t!("qexed.console.header", locale = locale).to_string());
        lines.extend(console_entries.iter().map(localized_help_line));
    }
    lines.join("\n")
}

fn localized_help_line(entry: &CommandHelpEntry) -> String {
    match &entry.description {
        Some(description) => format!("{} - {description}", entry.usage),
        None => entry.usage.clone(),
    }
}

fn localized_builtin_help_entry(command: &str, locale: &str) -> Option<CommandHelpEntry> {
    let (usage, description_key) = match command {
        "help" => ("/help", "qexed.command.help.description"),
        "list" => ("/list", "qexed.command.list.description"),
        "lobby" => ("/lobby [status|refresh]", "qexed.command.lobby.description"),
        "server" => ("/server [id]", "qexed.command.server.description"),
        "spawn" => ("/spawn", "qexed.command.spawn.description"),
        "teleport" => (
            "/teleport <target|location>",
            "qexed.command.teleport.description",
        ),
        "tp" => ("/tp <target|location>", "qexed.command.tp.description"),
        "time" => (
            "/time set|add|query <dimension> ...",
            "qexed.command.time.description",
        ),
        "gamerule" => (
            "/gamerule <dimension> <rule> [value]",
            "qexed.command.gamerule.description",
        ),
        "scoreboard" => ("/scoreboard ...", "qexed.command.scoreboard.description"),
        "entity" => (
            "/entity list|spawn|move|remove ...",
            "qexed.command.entity.description",
        ),
        "npc" => (
            "/npc list|spawn <id> [entity_type] [name...]|move|remove ...",
            "qexed.command.npc.description",
        ),
        "structure" => (
            "/structure list|place|locate ...",
            "qexed.command.structure.description",
        ),
        _ => return None,
    };
    Some(CommandHelpEntry::new(
        usage,
        Some(rust_i18n::t!(description_key, locale = locale).to_string()),
    ))
}

fn builtin_command_literals() -> &'static [&'static str] {
    &[
        "help",
        "list",
        "lobby",
        "server",
        "spawn",
        "teleport",
        "tp",
        "time",
        "gamerule",
        "scoreboard",
        "entity",
        "npc",
        "structure",
    ]
}

fn vanilla_command_literals() -> &'static [&'static str] {
    // Source: Minecraft Java 1.21.4 command registry cross-checked against
    // https://mappings.dev/1.21.4/net/minecraft/server/commands/index.html
    // Snapshot date: 2026-05-28. Deprecated/testing/debug-only commands are excluded.
    &[
        "advancement",
        "attribute",
        "ban",
        "ban-ip",
        "banlist",
        "bossbar",
        "clear",
        "clone",
        "damage",
        "data",
        "datapack",
        "debug",
        "defaultgamemode",
        "deop",
        "difficulty",
        "effect",
        "enchant",
        "execute",
        "experience",
        "fill",
        "fillbiome",
        "forceload",
        "function",
        "gamemode",
        "gamerule",
        "give",
        "help",
        "item",
        "jfr",
        "kick",
        "kill",
        "list",
        "locate",
        "loot",
        "me",
        "msg",
        "op",
        "pardon",
        "pardon-ip",
        "particle",
        "perf",
        "place",
        "playsound",
        "publish",
        "random",
        "recipe",
        "reload",
        "return",
        "ride",
        "rotate",
        "save-all",
        "save-off",
        "save-on",
        "say",
        "schedule",
        "scoreboard",
        "seed",
        "setblock",
        "setidletimeout",
        "setworldspawn",
        "spawnpoint",
        "spectate",
        "spreadplayers",
        "stop",
        "stopsound",
        "summon",
        "tag",
        "team",
        "teammsg",
        "teleport",
        "tell",
        "tellraw",
        "tick",
        "time",
        "title",
        "tm",
        "tp",
        "transfer",
        "trigger",
        "w",
        "weather",
        "whitelist",
        "worldborder",
    ]
}

#[cfg(test)]
mod tests {
    use qexed_packet::Packet;

    #[test]
    fn command_tree_contains_help_and_list() {
        let tree = super::command_tree();
        assert_eq!(tree.root_index.0, 0);
        assert_eq!(tree.nodes[0].children.len(), 13);

        let root_command_names = tree.nodes[0]
            .children
            .iter()
            .map(|index| {
                tree.nodes[index.0 as usize]
                    .name
                    .as_deref()
                    .unwrap_or_default()
            })
            .collect::<Vec<_>>();
        assert!(root_command_names.contains(&"help"));
        assert!(root_command_names.contains(&"list"));
        assert!(root_command_names.contains(&"lobby"));
        assert!(root_command_names.contains(&"server"));
        assert!(root_command_names.contains(&"spawn"));
        assert!(root_command_names.contains(&"teleport"));
        assert!(root_command_names.contains(&"tp"));
        assert!(root_command_names.contains(&"time"));
        assert!(root_command_names.contains(&"gamerule"));
        assert!(root_command_names.contains(&"scoreboard"));
        assert!(root_command_names.contains(&"entity"));
        assert!(root_command_names.contains(&"npc"));
        assert!(root_command_names.contains(&"structure"));

        let mut buf = bytes::BytesMut::new();
        let mut writer = qexed_packet::PacketWriter::new(&mut buf);
        tree.serialize(&mut writer).unwrap();
        assert!(!buf.is_empty());
    }

    #[test]
    fn command_tree_can_be_filtered_by_permissions() {
        let tree = super::command_tree_for(&["server"]);
        assert_eq!(tree.root_index.0, 0);
        assert_eq!(tree.nodes.len(), 3);
        assert_eq!(
            tree.nodes[0].children,
            vec![qexed_packet::net_types::VarInt(1)]
        );
        assert_eq!(tree.nodes[1].name.as_deref(), Some("server"));
        assert_eq!(tree.nodes[2].name.as_deref(), Some("target"));
    }

    #[test]
    fn server_command_tree_includes_configured_server_literals() {
        let tree = super::command_tree_for_lobby(
            &["server"],
            &[
                "survival".to_string(),
                "minigames".to_string(),
                "survival".to_string(),
                "bad id".to_string(),
            ],
        );
        assert_eq!(tree.root_index.0, 0);
        assert_eq!(tree.nodes.len(), 5);
        assert_eq!(tree.nodes[1].name.as_deref(), Some("server"));
        assert_eq!(
            tree.nodes[1].children,
            vec![
                qexed_packet::net_types::VarInt(2),
                qexed_packet::net_types::VarInt(3),
                qexed_packet::net_types::VarInt(4),
            ]
        );
        assert_eq!(tree.nodes[2].name.as_deref(), Some("target"));
        assert_eq!(tree.nodes[3].name.as_deref(), Some("minigames"));
        assert_eq!(tree.nodes[4].name.as_deref(), Some("survival"));
    }

    #[test]
    fn lobby_command_contains_status_subcommand() {
        let tree = super::command_tree_for(&["lobby"]);
        assert_eq!(tree.root_index.0, 0);
        assert_eq!(tree.nodes.len(), 4);
        assert_eq!(
            tree.nodes[0].children,
            vec![qexed_packet::net_types::VarInt(1)]
        );
        assert_eq!(tree.nodes[1].name.as_deref(), Some("lobby"));
        assert_eq!(
            tree.nodes[1].children,
            vec![
                qexed_packet::net_types::VarInt(2),
                qexed_packet::net_types::VarInt(3),
            ]
        );
        assert_eq!(tree.nodes[2].name.as_deref(), Some("status"));
        assert_eq!(tree.nodes[3].name.as_deref(), Some("refresh"));
    }

    #[test]
    fn entity_command_contains_subcommands() {
        let tree = super::command_tree_for(&["entity"]);
        assert_eq!(tree.root_index.0, 0);
        assert_eq!(tree.nodes.len(), 11);
        assert_eq!(tree.nodes[1].name.as_deref(), Some("entity"));
        assert_eq!(
            tree.nodes[1].children,
            vec![
                qexed_packet::net_types::VarInt(2),
                qexed_packet::net_types::VarInt(3),
                qexed_packet::net_types::VarInt(4),
                qexed_packet::net_types::VarInt(5),
            ]
        );
        assert_eq!(tree.nodes[2].name.as_deref(), Some("list"));
        assert_eq!(tree.nodes[3].name.as_deref(), Some("spawn"));
        assert_eq!(tree.nodes[4].name.as_deref(), Some("move"));
        assert_eq!(tree.nodes[5].name.as_deref(), Some("remove"));
        assert_eq!(tree.nodes[6].name.as_deref(), Some("entity"));
        assert_eq!(tree.nodes[7].name.as_deref(), Some("hologram"));
        assert_eq!(tree.nodes[8].name.as_deref(), Some("params"));
        assert_eq!(tree.nodes[9].name.as_deref(), Some("id"));
        assert_eq!(tree.nodes[10].name.as_deref(), Some("id"));
    }

    #[test]
    fn npc_command_contains_subcommands() {
        let tree = super::command_tree_for(&["npc"]);
        assert_eq!(tree.root_index.0, 0);
        assert_eq!(tree.nodes.len(), 9);
        assert_eq!(tree.nodes[1].name.as_deref(), Some("npc"));
        assert_eq!(
            tree.nodes[1].children,
            vec![
                qexed_packet::net_types::VarInt(2),
                qexed_packet::net_types::VarInt(3),
                qexed_packet::net_types::VarInt(4),
                qexed_packet::net_types::VarInt(5),
            ]
        );
        assert_eq!(tree.nodes[2].name.as_deref(), Some("list"));
        assert_eq!(tree.nodes[3].name.as_deref(), Some("spawn"));
        assert_eq!(tree.nodes[4].name.as_deref(), Some("move"));
        assert_eq!(tree.nodes[5].name.as_deref(), Some("remove"));
        assert_eq!(tree.nodes[6].name.as_deref(), Some("params"));
        assert_eq!(tree.nodes[7].name.as_deref(), Some("id"));
        assert_eq!(tree.nodes[8].name.as_deref(), Some("id"));
    }

    #[test]
    fn structure_command_contains_subcommands() {
        let tree = super::command_tree_for(&["structure"]);
        assert_eq!(tree.root_index.0, 0);
        assert_eq!(tree.nodes.len(), 8);
        assert_eq!(tree.nodes[1].name.as_deref(), Some("structure"));
        assert_eq!(
            tree.nodes[1].children,
            vec![
                qexed_packet::net_types::VarInt(2),
                qexed_packet::net_types::VarInt(3),
                qexed_packet::net_types::VarInt(4),
            ]
        );
        assert_eq!(tree.nodes[2].name.as_deref(), Some("list"));
        assert_eq!(tree.nodes[3].name.as_deref(), Some("place"));
        assert_eq!(tree.nodes[4].name.as_deref(), Some("locate"));
        assert_eq!(tree.nodes[5].name.as_deref(), Some("id"));
        assert_eq!(tree.nodes[6].name.as_deref(), Some("id"));
        assert_eq!(tree.nodes[7].name.as_deref(), Some("position"));
    }

    #[test]
    fn command_permission_nodes_are_stable() {
        assert_eq!(
            super::permission_node("list").as_deref(),
            Some("qexed.command.list")
        );
        assert_eq!(
            super::permission_node("/help extra").as_deref(),
            Some("qexed.command.help")
        );
        assert_eq!(super::permission_node("   "), None);
    }

    #[test]
    fn localized_help_uses_requested_locale_without_pages() {
        let entries = super::localized_builtin_help_entries(&["help", "list"], "zh_cn");
        let help = super::localized_server_help("zh_cn", &entries);
        assert!(help.contains("/help"));
        assert!(help.contains("显示帮助"));
        assert!(!help.contains("page"));
        assert!(!help.contains("第"));

        let entries = super::localized_builtin_help_entries(&["help", "list"], "en_us");
        let help = super::localized_server_help("en_us", &entries);
        assert!(help.contains("Shows this help"));
    }

    #[test]
    fn suggestions_only_include_implemented_commands() {
        let commands = super::command_names_for_suggestions();
        assert!(commands.contains(&"help"));
        assert!(commands.contains(&"time"));
        assert!(commands.contains(&"gamerule"));
        assert!(!commands.contains(&"reload"));
    }
}

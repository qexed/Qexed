use std::{collections::HashSet, path::PathBuf, sync::OnceLock};

use anyhow::{Context, Result};
use qexed_packet::net_types::VarInt;
use qexed_protocol::{
    to_client::play::commands::{BrigadierString, Commands, Node, Varies},
    types::TextComponent,
};
use serde::{Deserialize, Serialize};

const COMMAND_CONFIG_PATH: &str = "config/qexed_commands.toml";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandMessages {
    #[serde(default = "default_help")]
    pub help: String,

    #[serde(default = "default_list")]
    pub list: String,

    #[serde(default = "default_unknown")]
    pub unknown: String,

    #[serde(default = "default_lobby")]
    pub lobby: String,

    #[serde(default = "default_lobby_unavailable")]
    pub lobby_unavailable: String,

    #[serde(default = "default_lobby_status")]
    pub lobby_status: String,

    #[serde(default = "default_spawn")]
    pub spawn: String,

    #[serde(default = "default_server_list")]
    pub server_list: String,

    #[serde(default = "default_server_missing")]
    pub server_missing: String,
}

impl Default for CommandMessages {
    fn default() -> Self {
        Self {
            help: default_help(),
            list: default_list(),
            unknown: default_unknown(),
            lobby: default_lobby(),
            lobby_unavailable: default_lobby_unavailable(),
            lobby_status: default_lobby_status(),
            spawn: default_spawn(),
            server_list: default_server_list(),
            server_missing: default_server_missing(),
        }
    }
}

impl CommandMessages {
    pub fn load_or_create() -> Self {
        match Self::load_or_create_inner() {
            Ok(messages) => messages,
            Err(err) => {
                log::warn!("failed to load command messages, using defaults: {err:#}");
                Self::default()
            }
        }
    }

    fn load_or_create_inner() -> Result<Self> {
        let path = workspace_root().join(COMMAND_CONFIG_PATH);
        if !path.exists() {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("create {}", parent.display()))?;
            }
            let content = toml::to_string_pretty(&Self::default())?;
            std::fs::write(&path, content).with_context(|| format!("write {}", path.display()))?;
        }

        let content =
            std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        toml::from_str(&content).with_context(|| format!("parse {}", path.display()))
    }

    pub fn render_help(&self) -> TextComponent {
        text_component(&self.help)
    }

    pub fn render_list(&self, online: usize, max: i32, names: &[String]) -> TextComponent {
        text_component(
            self.list
                .replace("{online}", &online.to_string())
                .replace("{max}", max_label(max).as_str())
                .replace("{players}", &names.join(", ")),
        )
    }

    pub fn render_unknown(&self, command: &str) -> TextComponent {
        text_component(self.unknown.replace("{command}", command))
    }

    pub fn render_lobby(&self) -> TextComponent {
        text_component(&self.lobby)
    }

    pub fn render_lobby_unavailable(&self) -> TextComponent {
        text_component(&self.lobby_unavailable)
    }

    pub fn render_lobby_status(&self, summary: &str, servers: &[String]) -> TextComponent {
        let servers = if servers.is_empty() {
            "none".to_string()
        } else {
            servers.join(", ")
        };
        text_component(
            self.lobby_status
                .replace("{summary}", summary)
                .replace("{servers}", &servers),
        )
    }

    pub fn render_spawn(&self) -> TextComponent {
        text_component(&self.spawn)
    }

    pub fn render_server_list(&self, servers: &[String]) -> TextComponent {
        let servers = if servers.is_empty() {
            "none".to_string()
        } else {
            servers.join(", ")
        };
        text_component(self.server_list.replace("{servers}", &servers))
    }

    pub fn render_server_missing(&self, server: &str) -> TextComponent {
        text_component(self.server_missing.replace("{server}", server))
    }
}

pub fn command_tree() -> Commands {
    command_tree_for(&[
        "help",
        "list",
        "lobby",
        "server",
        "spawn",
        "entity",
        "structure",
    ])
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
        } else if *command == "entity" {
            append_entity_command_nodes(&mut nodes)
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
    for command in builtin_command_literals()
        .iter()
        .chain(vanilla_command_literals().iter())
        .copied()
    {
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
    match command.trim().trim_start_matches('/').to_ascii_lowercase().as_str() {
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
    (!name.is_empty()).then(|| format!("qexed.command.{name}"))
}

pub fn messages() -> &'static CommandMessages {
    static MESSAGES: OnceLock<CommandMessages> = OnceLock::new();
    MESSAGES.get_or_init(CommandMessages::load_or_create)
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
    let spawn_npc_index = nodes.len() as i32;
    nodes.push(Node {
        flags: 0x01 | 0x04,
        children: Vec::new(),
        name: Some("npc".to_string()),
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
    nodes[spawn_index as usize].children = vec![
        VarInt(spawn_entity_index),
        VarInt(spawn_npc_index),
        VarInt(spawn_hologram_index),
    ];
    nodes[move_index as usize].children = vec![VarInt(move_id_index)];
    nodes[remove_index as usize].children = vec![VarInt(remove_id_index)];
    nodes[spawn_entity_index as usize].children = vec![VarInt(spawn_params_index)];
    nodes[spawn_npc_index as usize].children = vec![VarInt(spawn_params_index)];
    nodes[spawn_hologram_index as usize].children = vec![VarInt(spawn_params_index)];

    entity_index
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

fn default_help() -> String {
    "---- Minecraft Help ----\n/list - Lists players on the server.\n/lobby - Opens the lobby menu.\n/lobby status - Shows lobby backend status.\n/lobby refresh - Refreshes lobby backend status.\n/server [id] - Lists or joins a backend server.\n/spawn - Returns to spawn.\n/entity list|spawn|move|remove - Manages runtime lobby entities.\n/structure list|place|locate - Manages built-in structures.".to_string()
}

fn default_list() -> String {
    "There are {online} of a max of {max} players online: {players}".to_string()
}

fn default_unknown() -> String {
    "Unknown or incomplete command, see below for error\n/{command}<--[HERE]".to_string()
}

fn default_lobby() -> String {
    "Opened lobby menu.".to_string()
}

fn default_lobby_unavailable() -> String {
    "Lobby is not enabled on this server.".to_string()
}

fn default_lobby_status() -> String {
    "{summary}: {servers}".to_string()
}

fn default_spawn() -> String {
    "Teleported to spawn.".to_string()
}

fn default_server_list() -> String {
    "Available servers: {servers}".to_string()
}

fn default_server_missing() -> String {
    "Server is unavailable: {server}".to_string()
}

fn max_label(max: i32) -> String {
    if max < 0 {
        "unlimited".to_string()
    } else {
        max.to_string()
    }
}

fn text_component(text: impl Into<String>) -> TextComponent {
    let mut map = std::collections::HashMap::new();
    map.insert(
        "text".to_string(),
        qexed_nbt::Tag::String(std::sync::Arc::from(text.into())),
    );
    qexed_nbt::Tag::Compound(std::sync::Arc::new(map))
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn builtin_command_literals() -> &'static [&'static str] {
    &["help", "list", "lobby", "server", "spawn", "entity", "structure"]
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
        assert_eq!(tree.nodes[0].children.len(), 7);

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
        assert!(root_command_names.contains(&"entity"));
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
        assert_eq!(tree.nodes.len(), 12);
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
        assert_eq!(tree.nodes[7].name.as_deref(), Some("npc"));
        assert_eq!(tree.nodes[8].name.as_deref(), Some("hologram"));
        assert_eq!(tree.nodes[9].name.as_deref(), Some("params"));
        assert_eq!(tree.nodes[10].name.as_deref(), Some("id"));
        assert_eq!(tree.nodes[11].name.as_deref(), Some("id"));
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
    fn command_messages_include_lobby_unavailable_default() {
        let messages = super::CommandMessages::default();
        assert_eq!(
            messages.lobby_unavailable,
            "Lobby is not enabled on this server."
        );
        let rendered = messages.render_lobby_unavailable();
        let qexed_nbt::Tag::Compound(map) = rendered else {
            panic!("expected text component compound");
        };
        assert_eq!(
            map.get("text"),
            Some(&qexed_nbt::Tag::String(std::sync::Arc::from(
                "Lobby is not enabled on this server."
            )))
        );
    }
}

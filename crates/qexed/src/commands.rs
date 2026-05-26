use std::{path::PathBuf, sync::OnceLock};

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
    command_tree_for(&["help", "list", "lobby", "server", "spawn", "entity"])
}

pub fn command_tree_for(commands: &[&str]) -> Commands {
    command_tree_for_lobby(commands, &[])
}

pub fn command_tree_for_lobby(commands: &[&str], server_ids: &[String]) -> Commands {
    let server_ids = server_command_literals(server_ids);
    let mut nodes = vec![Node {
        flags: 0x00,
        children: Vec::new(),
        ..Node::default()
    }];
    for command in commands {
        let index = nodes.len() as i32;
        nodes[0].children.push(VarInt(index));
        if *command == "lobby" {
            nodes.push(lobby_literal(index + 1, index + 2));
            nodes.push(executable_literal("status"));
            nodes.push(executable_literal("refresh"));
        } else if *command == "server" {
            nodes.push(server_literal(index + 1, index + 2, server_ids.len()));
            nodes.push(greedy_string_argument("target"));
            for server_id in &server_ids {
                nodes.push(executable_literal(server_id));
            }
        } else if *command == "entity" {
            nodes.push(entity_literal(index + 1));
            nodes.push(greedy_string_argument("action"));
        } else {
            nodes.push(executable_literal(command));
        }
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
    for command in ["help", "list", "lobby", "server", "spawn", "entity"] {
        if permissions.can_run_command(profile, command).await? {
            commands.push(command);
        }
    }
    Ok(commands)
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

fn entity_literal(argument_index: i32) -> Node {
    Node {
        flags: 0x01 | 0x04,
        children: vec![VarInt(argument_index)],
        name: Some("entity".to_string()),
        ..Node::default()
    }
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

fn default_help() -> String {
    "---- Minecraft Help ----\n/list - Lists players on the server.\n/lobby - Opens the lobby menu.\n/lobby status - Shows lobby backend status.\n/lobby refresh - Refreshes lobby backend status.\n/server [id] - Lists or joins a backend server.\n/spawn - Returns to spawn.\n/entity list|spawn|move|remove - Manages runtime lobby entities.".to_string()
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

#[cfg(test)]
mod tests {
    use qexed_packet::Packet;

    #[test]
    fn command_tree_contains_help_and_list() {
        let tree = super::command_tree();
        assert_eq!(tree.root_index.0, 0);
        assert_eq!(tree.nodes.len(), 11);
        assert_eq!(tree.nodes[1].name.as_deref(), Some("help"));
        assert_eq!(tree.nodes[2].name.as_deref(), Some("list"));
        assert_eq!(tree.nodes[3].name.as_deref(), Some("lobby"));
        assert_eq!(tree.nodes[4].name.as_deref(), Some("status"));
        assert_eq!(tree.nodes[5].name.as_deref(), Some("refresh"));
        assert_eq!(tree.nodes[6].name.as_deref(), Some("server"));
        assert_eq!(tree.nodes[7].name.as_deref(), Some("target"));
        assert_eq!(tree.nodes[9].name.as_deref(), Some("entity"));
        assert_eq!(tree.nodes[10].name.as_deref(), Some("action"));

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
    fn entity_command_accepts_greedy_action() {
        let tree = super::command_tree_for(&["entity"]);
        assert_eq!(tree.root_index.0, 0);
        assert_eq!(tree.nodes.len(), 3);
        assert_eq!(tree.nodes[1].name.as_deref(), Some("entity"));
        assert_eq!(
            tree.nodes[1].children,
            vec![qexed_packet::net_types::VarInt(2)]
        );
        assert_eq!(tree.nodes[2].name.as_deref(), Some("action"));
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

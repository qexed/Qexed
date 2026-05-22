use std::{path::PathBuf, sync::OnceLock};

use anyhow::{Context, Result};
use qexed_packet::net_types::VarInt;
use qexed_protocol::{
    to_client::play::commands::{Commands, Node},
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
}

impl Default for CommandMessages {
    fn default() -> Self {
        Self {
            help: default_help(),
            list: default_list(),
            unknown: default_unknown(),
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
}

pub fn command_tree() -> Commands {
    Commands {
        nodes: vec![
            Node {
                flags: 0x00,
                children: vec![VarInt(1), VarInt(2)],
                ..Node::default()
            },
            executable_literal("help"),
            executable_literal("list"),
        ],
        root_index: VarInt(0),
    }
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

fn default_help() -> String {
    "---- Minecraft Help ----\n/list - Lists players on the server.".to_string()
}

fn default_list() -> String {
    "There are {online} of a max of {max} players online: {players}".to_string()
}

fn default_unknown() -> String {
    "Unknown or incomplete command, see below for error\n/{command}<--[HERE]".to_string()
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
        assert_eq!(tree.nodes.len(), 3);
        assert_eq!(tree.nodes[1].name.as_deref(), Some("help"));
        assert_eq!(tree.nodes[2].name.as_deref(), Some("list"));

        let mut buf = bytes::BytesMut::new();
        let mut writer = qexed_packet::PacketWriter::new(&mut buf);
        tree.serialize(&mut writer).unwrap();
        assert!(!buf.is_empty());
    }
}

mod catalog;
mod execute;
mod help;
mod suggest;
mod tree;

pub use catalog::{
    builtin_command_literals, command_names_for_suggestions, is_known_vanilla_command,
    normalize_command_name, permission_node, visible_commands,
};
pub use execute::{CommandContext, CommandResponse, execute_builtin};
pub use help::{
    CommandHelpEntry, i18n_locale, localized_builtin_help_entries, localized_command_help,
    localized_console_help, localized_list, localized_max_label, localized_plugin_help_entry,
    localized_server_help, localized_server_help_page, localized_unimplemented,
};
pub use suggest::{
    CommandSuggestionMatches, command_suggestion_matches,
    command_suggestion_matches_with_candidates, command_suggestion_matches_with_sources,
};
pub use tree::{
    command_tree, command_tree_for, command_tree_for_lobby, command_tree_for_lobby_with_extra,
    command_tree_for_servers, command_tree_for_servers_with_extra,
};

#[cfg(test)]
mod tests {
    use qexed_packet::Packet;

    #[test]
    fn command_tree_contains_help_and_list() {
        let tree = super::command_tree();
        assert_eq!(tree.root_index.0, 0);
        assert_eq!(
            tree.nodes[0].children.len(),
            super::builtin_command_literals().len()
        );

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
        assert!(root_command_names.contains(&"version"));
        assert!(root_command_names.contains(&"plugins"));
        assert!(root_command_names.contains(&"gamemode"));
        assert!(root_command_names.contains(&"give"));
        assert!(root_command_names.contains(&"reload"));
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
    fn simple_commands_do_not_expose_generic_args() {
        let tree = super::command_tree_for(&["help", "version"]);

        let version = tree.nodes[0]
            .children
            .iter()
            .map(|index| &tree.nodes[index.0 as usize])
            .find(|node| node.name.as_deref() == Some("version"))
            .unwrap();
        assert!(version.children.is_empty());
        assert_eq!(version.flags & 0x04, 0x04);

        assert!(
            !tree
                .nodes
                .iter()
                .any(|node| matches!(node.name.as_deref(), Some("args" | "arguments")))
        );
    }

    #[test]
    fn help_command_exposes_page_and_command_arguments() {
        let tree = super::command_tree_for(&["help"]);

        assert_eq!(tree.nodes[1].name.as_deref(), Some("help"));
        assert_eq!(
            tree.nodes[1].children,
            vec![
                qexed_packet::net_types::VarInt(2),
                qexed_packet::net_types::VarInt(3),
            ]
        );
        assert_eq!(tree.nodes[2].name.as_deref(), Some("page"));
        assert_eq!(tree.nodes[2].parser_id.as_ref().map(|id| id.0), Some(3));
        assert_eq!(tree.nodes[3].name.as_deref(), Some("command"));
        assert_eq!(tree.nodes[3].parser_id.as_ref().map(|id| id.0), Some(5));
        assert_eq!(tree.nodes[3].flags & 0x10, 0x10);
        assert_eq!(
            tree.nodes[3].suggestions_type.as_deref(),
            Some("minecraft:ask_server")
        );
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
    fn localized_help_supports_pages() {
        let entries = super::localized_builtin_help_entries(&["help", "list"], "zh_cn");
        let help = super::localized_server_help("zh_cn", &entries);
        assert!(help.contains("/help"));
        assert!(help.contains("显示帮助"));
        assert!(help.contains("第 1/1 页"));

        let entries =
            super::localized_builtin_help_entries(super::builtin_command_literals(), "zh_cn");
        let first_page = super::localized_server_help_page("zh_cn", &entries, 1);
        let second_page = super::localized_server_help_page("zh_cn", &entries, 2);
        assert!(first_page.contains("第 1/3 页"));
        assert!(second_page.contains("第 2/3 页"));
        assert!(first_page.contains("/help"));
        assert!(!first_page.contains("/time set"));
        assert!(second_page.contains("/time set"));
    }

    #[test]
    fn suggestions_only_include_implemented_commands() {
        let commands = super::command_names_for_suggestions();
        assert!(commands.contains(&"help"));
        assert!(commands.contains(&"time"));
        assert!(commands.contains(&"gamerule"));
        assert!(commands.contains(&"reload"));
        assert!(!commands.contains(&"lobby"));
    }

    #[test]
    fn execute_builtin_returns_basic_messages() {
        let context = super::CommandContext {
            locale: "zh_cn".to_string(),
            online_players: vec!["Steve".to_string()],
            max_players: 20,
            version: "qexed test".to_string(),
            plugins: Vec::new(),
        };

        assert_eq!(
            super::execute_builtin("version", &context),
            super::CommandResponse::Message("qexed test".to_string())
        );
        assert!(matches!(
            super::execute_builtin("definitely_unknown", &context),
            super::CommandResponse::Unknown { .. }
        ));
    }

    #[test]
    fn execute_help_accepts_page_number() {
        let context = super::CommandContext {
            locale: "zh_cn".to_string(),
            online_players: vec!["Steve".to_string()],
            max_players: 20,
            version: "qexed test".to_string(),
            plugins: Vec::new(),
        };

        let super::CommandResponse::Message(message) = super::execute_builtin("help 2", &context)
        else {
            panic!("help command should return a message");
        };

        assert!(message.contains("第 2/3 页"));
        assert!(message.contains("/time set"));
    }

    #[test]
    fn execute_help_accepts_command_name() {
        let context = super::CommandContext {
            locale: "zh_cn".to_string(),
            online_players: vec!["Steve".to_string()],
            max_players: 20,
            version: "qexed test".to_string(),
            plugins: Vec::new(),
        };

        let super::CommandResponse::Message(message) =
            super::execute_builtin("help version", &context)
        else {
            panic!("help command should return a message");
        };

        assert!(message.contains("/version - 显示服务器版本。"));
        assert!(!message.contains("/help - 显示帮助。"));
        assert!(!message.contains("第 "));
    }

    #[test]
    fn execute_help_reports_unknown_command_name() {
        let context = super::CommandContext {
            locale: "zh_cn".to_string(),
            online_players: vec!["Steve".to_string()],
            max_players: 20,
            version: "qexed test".to_string(),
            plugins: Vec::new(),
        };

        let super::CommandResponse::Message(message) =
            super::execute_builtin("help missing_command", &context)
        else {
            panic!("help command should return a message");
        };

        assert!(message.contains("未找到指令帮助：/missing_command"));
    }

    #[test]
    fn command_suggestions_match_current_token() {
        let matches = super::command_suggestion_matches("/ver");

        assert_eq!(matches.start, 1);
        assert_eq!(matches.length, 3);
        assert_eq!(matches.values, vec!["version".to_string()]);
    }

    #[test]
    fn command_suggestions_support_help_arguments() {
        let matches = super::command_suggestion_matches("/help v");

        assert_eq!(matches.start, 6);
        assert_eq!(matches.length, 1);
        assert_eq!(matches.values, vec!["version".to_string()]);
    }

    #[test]
    fn command_suggestions_use_argument_position() {
        let matches = super::command_suggestion_matches_with_sources(
            "/gamemode creative S",
            &["Steve".to_string()],
            &[],
        );

        assert_eq!(matches.start, 19);
        assert_eq!(matches.length, 1);
        assert_eq!(matches.values, vec!["Steve".to_string()]);

        let matches = super::command_suggestion_matches("/time ");
        assert_eq!(
            matches.values,
            vec!["add".to_string(), "query".to_string(), "set".to_string()]
        );
    }
}

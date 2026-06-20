use crate::catalog::{
    builtin_command_literals, is_known_vanilla_command, normalize_command_name, visible_commands,
};
use crate::help::{
    localized_builtin_help_entries, localized_command_help, localized_list,
    localized_server_help_page, localized_unimplemented,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandContext {
    pub locale: String,
    pub online_players: Vec<String>,
    pub max_players: i32,
    pub version: String,
    pub plugins: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandResponse {
    Message(String),
    Unknown { command: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum HelpQuery {
    Page(usize),
    Command(String),
}

pub fn execute_builtin(command: &str, context: &CommandContext) -> CommandResponse {
    let command = command.trim().trim_start_matches('/');
    let mut parts = command.split_whitespace();
    let raw_name = parts.next().unwrap_or_default();
    let name = normalize_command_name(raw_name);
    let argument = parts.collect::<Vec<_>>().join(" ");

    match name.as_str() {
        "" => CommandResponse::Unknown {
            command: command.to_string(),
        },
        "help" => {
            let commands = visible_commands();
            let entries = localized_builtin_help_entries(&commands, &context.locale);
            let message = match parse_help_query(&argument) {
                HelpQuery::Page(page) => {
                    localized_server_help_page(&context.locale, &entries, page)
                }
                HelpQuery::Command(command) => {
                    localized_command_help(&context.locale, &entries, &command)
                }
            };
            CommandResponse::Message(message)
        }
        "list" => {
            let mut names = context.online_players.clone();
            names.sort();
            CommandResponse::Message(localized_list(
                &context.locale,
                names.len(),
                context.max_players,
                &names,
            ))
        }
        "version" => CommandResponse::Message(context.version.clone()),
        "plugins" => CommandResponse::Message(plugin_list_message(&context.plugins)),
        name if is_known_vanilla_command(name) || builtin_command_literals().contains(&name) => {
            CommandResponse::Message(localized_unimplemented(&context.locale, name))
        }
        _ => CommandResponse::Unknown {
            command: if argument.is_empty() {
                name
            } else {
                format!("{name} {argument}")
            },
        },
    }
}

fn plugin_list_message(plugins: &[String]) -> String {
    if plugins.is_empty() {
        "插件（0）：无".to_string()
    } else {
        format!("插件（{}）：{}", plugins.len(), plugins.join(", "))
    }
}

fn parse_help_query(argument: &str) -> HelpQuery {
    let Some(value) = argument.split_whitespace().next() else {
        return HelpQuery::Page(1);
    };

    if let Ok(page) = value.parse::<isize>() {
        return HelpQuery::Page((page.max(1)) as usize);
    }

    HelpQuery::Command(normalize_command_name(value))
}

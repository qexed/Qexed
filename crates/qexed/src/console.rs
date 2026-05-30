use anyhow::Context;
use qexed_packet::{Packet, PacketCodec};
use qexed_protocol::{to_client::play::system_chat::SystemChat, types::TextComponent};
use tokio::{
    io::{self, AsyncBufReadExt},
    sync::watch,
};

use crate::connection::ServerContext;

#[derive(Debug, Clone, PartialEq, Eq)]
enum ConsoleCommand {
    Empty,
    Help,
    Status,
    List,
    Say(String),
    Stop,
    Unknown(String),
}

impl ConsoleCommand {
    fn parse(input: &str) -> Self {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Self::Empty;
        }
        let command = trimmed.strip_prefix('/').unwrap_or(trimmed);
        let mut parts = command.splitn(2, char::is_whitespace);
        let name = parts.next().unwrap_or_default().to_ascii_lowercase();
        let argument = parts.next().unwrap_or_default().trim_start().to_string();

        match name.as_str() {
            "help" | "?" => Self::Help,
            "status" | "stats" => Self::Status,
            "list" | "players" => Self::List,
            "say" | "broadcast" => Self::Say(argument),
            "stop" | "exit" | "quit" => Self::Stop,
            _ => Self::Unknown(command.to_string()),
        }
    }
}

pub(crate) fn spawn(context: ServerContext, shutdown: watch::Sender<bool>) {
    tokio::spawn(async move {
        if let Err(err) = run(context, shutdown).await {
            log::warn!("terminal console stopped: {err:#}");
        }
    });
}

pub(crate) fn spawn_ctrl_c_shutdown(shutdown: watch::Sender<bool>) {
    tokio::spawn(async move {
        if let Err(err) = tokio::signal::ctrl_c().await {
            log::warn!("failed to listen for Ctrl+C: {err:#}");
            return;
        }
        print_console(rust_i18n::t!("qexed.console.ctrl_c"));
        let _ = shutdown.send(true);
    });
}

async fn run(context: ServerContext, shutdown: watch::Sender<bool>) -> anyhow::Result<()> {
    let locale = crate::commands::i18n_locale(&context.config.language);
    print_console(rust_i18n::t!("qexed.console.enabled", locale = locale));
    let stdin = io::BufReader::new(io::stdin());
    let mut lines = stdin.lines();

    while let Some(line) = lines
        .next_line()
        .await
        .context("read terminal console input")?
    {
        if execute(ConsoleCommand::parse(&line), &context, &shutdown)? {
            break;
        }
    }

    Ok(())
}

fn execute(
    command: ConsoleCommand,
    context: &ServerContext,
    shutdown: &watch::Sender<bool>,
) -> anyhow::Result<bool> {
    let locale = crate::commands::i18n_locale(&context.config.language);
    match command {
        ConsoleCommand::Empty => {}
        ConsoleCommand::Help => print_help(context),
        ConsoleCommand::Status => print_status(context),
        ConsoleCommand::List => print_players(context),
        ConsoleCommand::Say(message) => broadcast_message(context, &message)?,
        ConsoleCommand::Stop => {
            print_console(rust_i18n::t!("qexed.console.shutdown", locale = locale));
            let _ = shutdown.send(true);
            return Ok(true);
        }
        ConsoleCommand::Unknown(command) => {
            print_console(rust_i18n::t!(
                "qexed.console.unknown",
                locale = locale,
                command = command
            ));
        }
    }

    Ok(false)
}

fn print_help(context: &ServerContext) {
    print_console(crate::commands::localized_console_help(
        &context.config.language,
    ));
}

fn print_status(context: &ServerContext) {
    let locale = crate::commands::i18n_locale(&context.config.language);
    let online = context.players.online_count();
    let max_player = crate::commands::localized_max_label(locale, context.config.server.max_player);
    let proxy = if context.config.server.proxy {
        rust_i18n::t!("qexed.console.enabled_value", locale = locale).to_string()
    } else {
        rust_i18n::t!("qexed.console.disabled_value", locale = locale).to_string()
    };
    print_console(rust_i18n::t!(
        "qexed.console.status.listen",
        locale = locale,
        ip = &context.config.server.ip
    ));
    print_console(rust_i18n::t!(
        "qexed.console.status.players",
        locale = locale,
        online = online,
        max = max_player
    ));
    print_console(rust_i18n::t!(
        "qexed.console.status.dimension",
        locale = locale,
        dimension = &context.config.world.dimension
    ));
    print_console(rust_i18n::t!(
        "qexed.console.status.proxy",
        locale = locale,
        state = proxy,
        protocol = format!("{:?}", context.config.server.proxy_protocol)
    ));
}

fn print_players(context: &ServerContext) {
    let locale = crate::commands::i18n_locale(&context.config.language);
    let mut names = context.players.online_names();
    names.sort();
    if names.is_empty() {
        print_console(rust_i18n::t!(
            "qexed.console.players.empty",
            locale = locale
        ));
    } else {
        print_console(rust_i18n::t!(
            "qexed.console.players.list",
            locale = locale,
            players = names.join(", ")
        ));
    }
}

fn broadcast_message(context: &ServerContext, message: &str) -> anyhow::Result<()> {
    let locale = crate::commands::i18n_locale(&context.config.language);
    let message = message.trim();
    if message.is_empty() {
        print_console(rust_i18n::t!("qexed.console.say.usage", locale = locale));
        return Ok(());
    }

    let online = context.players.online_count();
    if online == 0 {
        print_console(rust_i18n::t!(
            "qexed.console.say.no_players",
            locale = locale
        ));
        return Ok(());
    }

    let packet = packet_bytes(SystemChat {
        content: text_component(format!("[Server] {message}")),
        overlay: false,
    })?;
    context.players.broadcast_packets(vec![packet]);
    print_console(rust_i18n::t!(
        "qexed.console.say.sent",
        locale = locale,
        online = online
    ));
    Ok(())
}

fn packet_bytes<T: Packet>(packet: T) -> anyhow::Result<bytes::Bytes> {
    let mut buf = bytes::BytesMut::new();
    let mut writer = qexed_packet::PacketWriter::new(&mut buf);
    qexed_packet::net_types::VarInt(T::ID).serialize(&mut writer)?;
    packet.serialize(&mut writer)?;
    Ok(buf.freeze())
}

fn text_component(text: impl Into<String>) -> TextComponent {
    let mut map = std::collections::HashMap::new();
    map.insert(
        "text".to_string(),
        qexed_nbt::Tag::String(std::sync::Arc::from(text.into())),
    );
    qexed_nbt::Tag::Compound(std::sync::Arc::new(map))
}

fn print_console(message: impl AsRef<str>) {
    println!("[Console] {}", message.as_ref());
}

#[cfg(test)]
mod tests {
    use super::ConsoleCommand;

    #[test]
    fn parses_console_command_aliases() {
        assert_eq!(ConsoleCommand::parse(""), ConsoleCommand::Empty);
        assert_eq!(ConsoleCommand::parse("   "), ConsoleCommand::Empty);
        assert_eq!(ConsoleCommand::parse("?"), ConsoleCommand::Help);
        assert_eq!(ConsoleCommand::parse("/help"), ConsoleCommand::Help);
        assert_eq!(ConsoleCommand::parse("players"), ConsoleCommand::List);
        assert_eq!(ConsoleCommand::parse("exit"), ConsoleCommand::Stop);
    }

    #[test]
    fn parses_console_say_argument() {
        assert_eq!(
            ConsoleCommand::parse("say hello world"),
            ConsoleCommand::Say("hello world".to_string())
        );
        assert_eq!(
            ConsoleCommand::parse("/broadcast   hello"),
            ConsoleCommand::Say("hello".to_string())
        );
    }

    #[test]
    fn parses_unknown_console_command() {
        assert_eq!(
            ConsoleCommand::parse("/reload now"),
            ConsoleCommand::Unknown("reload now".to_string())
        );
    }
}

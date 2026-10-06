//! 终端控制台：stdin 命令解析与执行 + Ctrl+C 关停。
//! 迁移自 v4 crates/qexed/src/console.rs。
//!
//! 适配差异（v6）：
//! - rust_i18n::t!(key, locale=..) → qexed_language::t(key)（全局语言表）
//! - v4 的 context.permissions / context.players / crate::profiler /
//!   crate::play::command_tree_packet_for_player 收敛进 ServerRuntime trait；
//!   profiler 未迁移（v4 qexed_profiler 独立 crate），profile 命令留 TODO。

use qexed_packet::{Packet, PacketCodec};
use qexed_protocol::to_client::play::system_chat::SystemChat;
use std::io::BufRead;
use tokio::sync::{mpsc, watch};

use crate::context::{ServerRuntime, ServerServices};
use crate::error::{Result, ServerError};

#[derive(Debug, Clone, PartialEq, Eq)]
enum ConsoleCommand {
    Empty,
    Help,
    Status,
    List,
    Version,
    Plugins,
    Say(String),
    Op(String),
    Reload,
    Stop,
    Profile(ProfileAction),
    Unknown(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProfileAction {
    Start,
    Stop,
    Report,
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
            "version" | "ver" => Self::Version,
            "plugins" => Self::Plugins,
            "say" | "broadcast" => Self::Say(argument),
            "op" => Self::Op(argument),
            "reload" => Self::Reload,
            "stop" | "exit" | "quit" => Self::Stop,
            "profile" => match argument.as_str() {
                "start" | "on" | "enable" => Self::Profile(ProfileAction::Start),
                "stop" | "off" | "disable" => Self::Profile(ProfileAction::Stop),
                _ => Self::Profile(ProfileAction::Report),
            },
            _ => Self::Unknown(command.to_string()),
        }
    }
}

pub fn spawn(
    runtime: std::sync::Arc<dyn ServerRuntime>,
    services: std::sync::Arc<ServerServices>,
    shutdown: watch::Sender<bool>,
) {
    let shutdown_rx = shutdown.subscribe();
    tokio::spawn(async move {
        if let Err(err) = run(runtime, services, shutdown, shutdown_rx).await {
            log::warn!("terminal console stopped: {err}");
        }
    });
}

pub fn spawn_ctrl_c_shutdown(shutdown: watch::Sender<bool>) {
    tokio::spawn(async move {
        if let Err(err) = tokio::signal::ctrl_c().await {
            log::warn!("failed to listen for Ctrl+C: {err}");
            return;
        }
        print_console(qexed_language::t("qexed.console.ctrl_c"));
        let _ = shutdown.send(true);
    });
}

async fn run(
    runtime: std::sync::Arc<dyn ServerRuntime>,
    services: std::sync::Arc<ServerServices>,
    shutdown: watch::Sender<bool>,
    mut shutdown_rx: watch::Receiver<bool>,
) -> Result<()> {
    print_console(qexed_language::t("qexed.console.enabled"));
    let mut lines = spawn_stdin_reader()?;

    loop {
        tokio::select! {
            changed = shutdown_rx.changed() => {
                if changed.is_err() || *shutdown_rx.borrow() {
                    break;
                }
            }
            line = lines.recv() => {
                let Some(line) = line else {
                    break;
                };
                let line = line?;
                if execute(ConsoleCommand::parse(&line), &runtime, &services, &shutdown).await? {
                    break;
                }
            }
        }
    }

    Ok(())
}

fn spawn_stdin_reader() -> Result<mpsc::UnboundedReceiver<Result<String>>> {
    let (sender, receiver) = mpsc::unbounded_channel();
    std::thread::Builder::new()
        .name("qexed-console-stdin".to_string())
        .spawn(move || {
            let stdin = std::io::stdin();
            let reader = std::io::BufReader::new(stdin.lock());
            for line in reader.lines() {
                let line = line.map_err(|source| ServerError::IoContext {
                    context: "read terminal console input".to_string(),
                    source,
                });
                if sender.send(line).is_err() {
                    break;
                }
            }
        })
        .map_err(|source| ServerError::IoContext {
            context: "spawn terminal console stdin reader".to_string(),
            source,
        })?;
    Ok(receiver)
}

async fn execute(
    command: ConsoleCommand,
    runtime: &std::sync::Arc<dyn ServerRuntime>,
    services: &std::sync::Arc<ServerServices>,
    shutdown: &watch::Sender<bool>,
) -> Result<bool> {
    match command {
        ConsoleCommand::Empty => {}
        ConsoleCommand::Help => print_help(),
        ConsoleCommand::Status => print_status(runtime),
        ConsoleCommand::List => print_players(runtime),
        ConsoleCommand::Version => print_version(),
        ConsoleCommand::Plugins => print_plugins(runtime),
        ConsoleCommand::Say(message) => broadcast_message(runtime, &message)?,
        ConsoleCommand::Op(target) => op_player(runtime, &target).await?,
        ConsoleCommand::Reload => reload_server(runtime),
        ConsoleCommand::Stop => {
            print_console(qexed_language::t("qexed.console.shutdown"));
            let _ = shutdown.send(true);
            return Ok(true);
        }
        ConsoleCommand::Profile(action) => handle_profile(action),
        ConsoleCommand::Unknown(command) => {
            print_console(
                qexed_language::t("qexed.console.unknown").replace("%{command}", &command),
            );
        }
    }
    let _ = services; // 各命令按需取 services（warden/audit/filter），当前命令集未用到
    Ok(false)
}

fn handle_profile(action: ProfileAction) {
    let Some(p) = qexed_profiler::get() else {
        print_console("Profiler not initialized");
        return;
    };
    match action {
        ProfileAction::Start => {
            p.enable();
            print_console("Profiler started. Run 'profile report' to generate.");
        }
        ProfileAction::Stop => {
            p.disable();
            print_console("Profiler stopped.");
        }
        ProfileAction::Report => {
            p.disable();
            let html = p.report_html();
            let path = std::env::current_dir()
                .unwrap_or_default()
                .join("profile_report.html");
            match std::fs::write(&path, &html) {
                Ok(_) => print_console(format!("Report written: {}", path.display())),
                Err(err) => print_console(format!("Failed to write report: {err}")),
            }
        }
    }
}

fn print_help() {
    print_console(crate::commands::localized_console_help());
}

fn print_status(runtime: &std::sync::Arc<dyn ServerRuntime>) {
    let online = runtime.player_snapshots().len();
    let max_player = crate::commands::localized_max_label(runtime.server_config().max_player);
    print_console(
        qexed_language::t("qexed.console.status.listen")
            .replace("%{ip}", &runtime.server_config().ip),
    );
    print_console(
        qexed_language::t("qexed.console.status.players")
            .replace("%{online}", &online.to_string())
            .replace("%{max}", &max_player),
    );
    print_console(
        qexed_language::t("qexed.console.status.dimension")
            .replace("%{dimension}", &runtime.default_dimension()),
    );
    let state = if runtime.server_config().proxy {
        qexed_language::t("qexed.console.enabled_value")
    } else {
        qexed_language::t("qexed.console.disabled_value")
    };
    print_console(
        qexed_language::t("qexed.console.status.proxy")
            .replace("%{state}", &state)
            .replace("%{protocol}", &format!("{:?}", runtime.server_config().proxy_protocol)),
    );
}

fn print_players(runtime: &std::sync::Arc<dyn ServerRuntime>) {
    let mut names = runtime
        .player_snapshots()
        .into_iter()
        .map(|player| player.username)
        .collect::<Vec<_>>();
    names.sort();
    if names.is_empty() {
        print_console(qexed_language::t("qexed.console.players.empty"));
    } else {
        print_console(
            qexed_language::t("qexed.console.players.list")
                .replace("%{players}", &names.join(", ")),
        );
    }
}

fn print_version() {
    print_console(format!(
        "{} {}{} (Minecraft {})",
        env!("CARGO_PKG_NAME"),
        if cfg!(debug_assertions) { "dev-" } else { "" },
        env!("CARGO_PKG_VERSION"),
        qexed_config::MC_VERSION,
    ));
}

fn print_plugins(runtime: &std::sync::Arc<dyn ServerRuntime>) {
    let summaries = runtime.plugin_summaries();
    if summaries.is_empty() {
        print_console("Plugins (0): none");
    } else {
        print_console(format!(
            "Plugins ({}): {}",
            summaries.len(),
            summaries.join(", ")
        ));
    }
}

fn reload_server(runtime: &std::sync::Arc<dyn ServerRuntime>) {
    runtime.emit_config_reload("config/server.toml");
    print_console(
        "Reloaded plugin configuration events. Runtime server config changes require restart.",
    );
}

async fn op_player(runtime: &std::sync::Arc<dyn ServerRuntime>, target: &str) -> Result<()> {
    let target = target.trim();
    if target.is_empty() {
        print_console("Usage: op <player>");
        return Ok(());
    }
    let Some(player) = runtime
        .player_snapshots()
        .into_iter()
        .find(|player| player.username.eq_ignore_ascii_case(target))
    else {
        print_console(format!("Player not found or offline: {target}"));
        return Ok(());
    };
    let uuid = uuid::Uuid::from_bytes(player.profile_id);
    runtime.grant_global_wildcard(uuid, &player.username).await_ok_or_none();
    runtime.refresh_command_tree(uuid)?;
    print_console(format!(
        "Granted '*' permission to {} ({uuid})",
        player.username
    ));
    Ok(())
}

fn broadcast_message(runtime: &std::sync::Arc<dyn ServerRuntime>, message: &str) -> Result<()> {
    let message = message.trim();
    if message.is_empty() {
        print_console(qexed_language::t("qexed.console.say.usage"));
        return Ok(());
    }

    let online = runtime.player_snapshots().len();
    if online == 0 {
        print_console(qexed_language::t("qexed.console.say.no_players"));
        return Ok(());
    }

    let packet = packet_bytes(SystemChat {
        content: text_component(format!("[Server] {message}")),
        overlay: false,
    })?;
    runtime.send_packets_to(uuid::Uuid::nil(), vec![packet]);
    print_console(
        qexed_language::t("qexed.console.say.sent").replace("%{online}", &online.to_string()),
    );
    Ok(())
}

/// v4 里 grant_global_wildcard 是 async；ServerRuntime 里是同步 trait 方法，
/// 包一层避免签名漂移（真实实现可以是阻塞的）。
trait GrantResultExt {
    fn await_ok_or_none(self);
}

impl GrantResultExt for Result<()> {
    fn await_ok_or_none(self) {
        if let Err(err) = self {
            log::warn!("op grant failed: {err}");
        }
    }
}

fn packet_bytes<T: Packet>(packet: T) -> Result<bytes::Bytes> {
    let mut buf = bytes::BytesMut::new();
    let mut writer = qexed_packet::PacketWriter::new(&mut buf);
    qexed_packet::net_types::VarInt(T::ID).serialize(&mut writer)?;
    packet.serialize(&mut writer)?;
    Ok(buf.freeze())
}

fn text_component(text: impl Into<String>) -> qexed_protocol::types::TextComponent {
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
        assert_eq!(ConsoleCommand::parse("ver"), ConsoleCommand::Version);
        assert_eq!(ConsoleCommand::parse("plugins"), ConsoleCommand::Plugins);
        assert_eq!(
            ConsoleCommand::parse("op Steve"),
            ConsoleCommand::Op("Steve".to_string())
        );
        assert_eq!(ConsoleCommand::parse("reload"), ConsoleCommand::Reload);
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
            ConsoleCommand::parse("/missing now"),
            ConsoleCommand::Unknown("missing now".to_string())
        );
    }
}

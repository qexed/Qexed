use anyhow::Result;
use qexed_protocol::to_client::play::system_chat::SystemChat;

use crate::players::PlayerManager;

use super::util::text_component;

pub(super) async fn handle_chat_command<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    config: &qexed_config::app::qexed::Qexed,
    players: &PlayerManager,
    permissions: &crate::permissions::PermissionManager,
    profile: &qexed_packet::net_types::GameProfile,
    command: &str,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let command = command.trim();
    if !permissions.can_run_command(profile, command).await? {
        sink.send(SystemChat {
            content: text_component(permissions.denied_message()),
            overlay: false,
        })
        .await?;
        return Ok(());
    }

    let messages = crate::commands::messages();
    let content = match command {
        "help" => messages.render_help(),
        "list" => {
            let mut names = players.online_names();
            names.sort();
            messages.render_list(players.online_count(), config.server.max_player, &names)
        }
        _ => messages.render_unknown(command),
    };

    sink.send(SystemChat {
        content,
        overlay: false,
    })
    .await?;
    Ok(())
}

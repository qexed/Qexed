use qexed_protocol::{
    to_client,
    to_server::{
        handshaking::set_protocol::SetProtocol,
        login::{
            encryption_begin::EncryptionBegin as ServerboundKey,
            login_acknowledged::LoginAcknowledged, login_plugin_response::LoginPluginResponse,
            login_start::LoginStart,
        },
    },
};

use crate::auth::offline_profile;

use super::{ServerContext, codec::read_expected_packet, configuration::handle_configuration};

pub(super) async fn handle_login<R, W>(
    handshake: SetProtocol,
    packets: &mut qexed_tcp_connect::PacketStream<R>,
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    context: &ServerContext,
) -> anyhow::Result<()>
where
    R: tokio::io::AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
{
    if handshake.protocol_version.0 != qexed_config::PROTOCOL_VERSION {
        disconnect_login(
            sink,
            format!(
                "Unsupported protocol {}. This server expects {} ({})",
                handshake.protocol_version.0,
                qexed_config::PROTOCOL_VERSION,
                qexed_config::MC_VERSION
            ),
        )
        .await?;
        return Ok(());
    }

    let login_start = read_expected_packet::<LoginStart, _>(packets).await?;
    let login = match resolve_login(handshake, packets, sink, context, &login_start).await {
        Ok(login) => login,
        Err(err) => {
            disconnect_login(sink, format!("Authentication failed: {err}")).await?;
            return Ok(());
        }
    };

    if context.config.server.network_compression_threshold >= 0 {
        let threshold = context.config.server.network_compression_threshold as i32;
        sink.send(to_client::login::compress::Compress {
            threshold: qexed_packet::net_types::VarInt(threshold),
        })
        .await?;
        sink.set_compression_threshold(threshold);
        packets.set_compression_threshold(threshold);
    }

    sink.send(to_client::login::success::Success {
        game_profile: login.profile.clone(),
    })
    .await?;

    read_expected_packet::<LoginAcknowledged, _>(packets).await?;
    handle_configuration(packets, sink, context, &login.login_host).await?;
    crate::play::initialize(
        packets,
        sink,
        &context.config,
        &context.authenticator,
        &context.world,
        &context.world_rules,
        &context.players,
        &context.entities,
        &context.player_data,
        &context.permissions,
        &context.plugins,
        &context.player_audit,
        &context.content_filter,
        &login.profile,
    )
    .await?;

    Ok(())
}

#[derive(Debug)]
struct Login {
    profile: qexed_packet::net_types::GameProfile,
    login_host: String,
}

async fn resolve_login<R, W>(
    handshake: SetProtocol,
    packets: &mut qexed_tcp_connect::PacketStream<R>,
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    context: &ServerContext,
    login_start: &LoginStart,
) -> anyhow::Result<Login>
where
    R: tokio::io::AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
{
    if context.config.server.proxy {
        match context.config.server.proxy_protocol {
            qexed_config::app::qexed::server::ForwardingMode::BungeeCord => {
                let forwarded = crate::proxy_forwarding::parse_bungeecord_forwarding(
                    &handshake.server_host,
                    &login_start.username,
                )?;
                log::debug!(
                    "accepted BungeeCord forwarded login for {}",
                    forwarded.profile.username
                );
                return Ok(Login {
                    profile: forwarded.profile,
                    login_host: forwarded.login_host,
                });
            }
            qexed_config::app::qexed::server::ForwardingMode::Velocity
            | qexed_config::app::qexed::server::ForwardingMode::Victory => {
                let profile =
                    request_velocity_forwarding(packets, sink, &context.config.server.proxy_token)
                        .await?;
                log::debug!("accepted Velocity forwarded login for {}", profile.username);
                return Ok(Login {
                    profile,
                    login_host: handshake.server_host,
                });
            }
            qexed_config::app::qexed::server::ForwardingMode::Default
            | qexed_config::app::qexed::server::ForwardingMode::QTunnel
            | qexed_config::app::qexed::server::ForwardingMode::None => {}
        }
    }

    let profile = if context.config.server.online_mode {
        authenticate_online(packets, sink, context, login_start).await?
    } else {
        offline_profile(&login_start.username)
    };

    Ok(Login {
        profile,
        login_host: handshake.server_host,
    })
}

async fn request_velocity_forwarding<R, W>(
    packets: &mut qexed_tcp_connect::PacketStream<R>,
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    secret: &str,
) -> anyhow::Result<qexed_packet::net_types::GameProfile>
where
    R: tokio::io::AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
{
    sink.send(to_client::login::login_plugin_request::LoginPluginRequest {
        message_id: qexed_packet::net_types::VarInt(
            crate::proxy_forwarding::VELOCITY_FORWARDING_MESSAGE_ID,
        ),
        payload: qexed_packet::net_types::CustomQueryPayload {
            channel: crate::proxy_forwarding::VELOCITY_FORWARDING_CHANNEL.to_string(),
            data: qexed_packet::net_types::RestBuffer(Vec::new()),
        },
    })
    .await?;
    sink.flush().await?;

    let response = read_expected_packet::<LoginPluginResponse, _>(packets).await?;
    if response.message_id.0 != crate::proxy_forwarding::VELOCITY_FORWARDING_MESSAGE_ID {
        anyhow::bail!(
            "Velocity forwarding response message ID mismatch: expected {}, actual {}",
            crate::proxy_forwarding::VELOCITY_FORWARDING_MESSAGE_ID,
            response.message_id.0
        );
    }

    let Some(data) = response.data else {
        anyhow::bail!("Velocity proxy did not provide forwarded player info");
    };

    crate::proxy_forwarding::parse_velocity_forwarding_response(&data.0, secret)
}

async fn authenticate_online<R, W>(
    packets: &mut qexed_tcp_connect::PacketStream<R>,
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    context: &ServerContext,
    login_start: &LoginStart,
) -> anyhow::Result<qexed_packet::net_types::GameProfile>
where
    R: tokio::io::AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
{
    let verify_token = random_verify_token();
    sink.send(to_client::login::encryption_begin::EncryptionBegin {
        server_id: String::new(),
        public_key: context.authenticator.public_key_der().to_vec().into(),
        verify_token: verify_token.clone().into(),
        should_authenticate: true,
    })
    .await?;

    let key_packet = read_expected_packet::<ServerboundKey, _>(packets).await?;
    let shared_secret = context
        .authenticator
        .decrypt_login_key(&key_packet, &verify_token)?;

    packets.enable_encryption(&shared_secret)?;
    sink.enable_encryption(&shared_secret)?;

    let authenticated = context
        .authenticator
        .verify_session(&login_start.username, &shared_secret, None)
        .await?;

    Ok(authenticated.into())
}

async fn disconnect_login<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    reason: impl Into<String>,
) -> anyhow::Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    sink.send(to_client::login::disconnect::Disconnect {
        reason: qexed_packet::net_types::JsonValue(serde_json::json!({
            "text": reason.into(),
        })),
    })
    .await?;
    Ok(())
}

fn random_verify_token() -> Vec<u8> {
    let mut token = vec![0_u8; 4];
    rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut token);
    token
}

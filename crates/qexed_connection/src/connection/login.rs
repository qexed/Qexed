//! 登录处理（v4 connection::login 迁移）。
//!
//! 包名对齐 v6：SetProtocol→ClientIntention，LoginStart→Hello，
//! EncryptionBegin(server)→Hello(login)，EncryptionBegin(client)→Key，
//! LoginPluginResponse→CustomQueryAnswer，LoginPluginRequest→CustomQuery，
//! Compress→LoginCompression，Success→LoginFinished，
//! Disconnect(login)→LoginDisconnect。
//!
//! v4 登录成功后直接调 crate::play::initialize 进入 play 域；v6 play 域在
//! qexed_play crate，此处以 LoginOutcome 返回结果 + TODO(hook) 留接口。
//! v4 的 warden 封禁检查（BanRecord）属于 qexed_server 域，同样留 TODO(hook)。

use qexed_protocol::{
    to_client,
    to_server::{
        handshaking::client_intention::ClientIntention,
        login::{
            custom_query_answer::CustomQueryAnswer, hello::Hello, key::Key,
            login_acknowledged::LoginAcknowledged,
        },
    },
};

use crate::auth::offline_profile;
use crate::config::ForwardingMode;

use super::{ServerContext, codec::read_expected_packet, configuration::handle_configuration};
use crate::error::{ConnectionError, Result};

/// 登录流程最终产出：交给 play 域（qexed_play / qexed_server）的资料。
/// TODO(hook): 待 qexed_server 组装层消费前暂无读取方，允许 dead_code。
#[derive(Debug)]
#[allow(dead_code)]
pub struct LoginOutcome {
    pub profile: qexed_packet::net_types::GameProfile,
    pub login_host: String,
    pub client_config: super::configuration::ClientConfiguration,
}

pub(super) async fn handle_login<R, W>(
    handshake: ClientIntention,
    packets: &mut crate::transport::PacketStream<R>,
    sink: &mut crate::transport::PacketSink<W>,
    context: &ServerContext,
    _shutdown: tokio::sync::watch::Receiver<bool>,
) -> Result<()>
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

    let login_start = read_expected_packet::<Hello, _>(packets).await?;
    let login = match resolve_login(&handshake, packets, sink, context, &login_start).await {
        Ok(login) => login,
        Err(err) => {
            disconnect_login(sink, format!("Authentication failed: {err}")).await?;
            return Ok(());
        }
    };

    // TODO(hook): v4 在此调用 context.warden.ban_for(uuid) 做封禁检查并断开；
    // v6 warden 属于 qexed_server 域，需要由组装层注入封禁查询回调。

    if context.config.network_compression_threshold >= 0 {
        let threshold = context.config.network_compression_threshold as i32;
        sink.send(to_client::login::login_compression::LoginCompression {
            compression_threshold: qexed_packet::net_types::VarInt(threshold),
        })
        .await?;
        sink.set_compression_threshold(threshold);
        packets.set_compression_threshold(threshold);
    }

    sink.send(to_client::login::login_finished::LoginFinished {
        game_profile: login.profile.clone(),
        session_id: uuid::Uuid::nil(),
    })
    .await?;

    read_expected_packet::<LoginAcknowledged, _>(packets).await?;
    let client_config = handle_configuration(packets, sink, context, &login.login_host).await?;
    context.ensure_plugins_initialized();

    // TODO(hook): v4 在此调用 crate::play::initialize(...) 进入 play 域，
    // 携带 world/players/entities 等全部运行时；v6 由 qexed_server 组装层
    // 拿到 LoginOutcome 后调用 qexed_play 的入口。
    let _outcome = LoginOutcome {
        profile: login.profile,
        login_host: login.login_host,
        client_config,
    };

    Ok(())
}

#[derive(Debug)]
struct Login {
    profile: qexed_packet::net_types::GameProfile,
    login_host: String,
}


async fn resolve_login<R, W>(
    handshake: &ClientIntention,
    packets: &mut crate::transport::PacketStream<R>,
    sink: &mut crate::transport::PacketSink<W>,
    context: &ServerContext,
    login_start: &Hello,
) -> Result<Login>
where
    R: tokio::io::AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
{
    if context.config.proxy {
        match context.config.proxy_protocol {
            ForwardingMode::BungeeCord => {
                let forwarded = crate::proxy_forwarding::parse_bungeecord_forwarding(
                    &handshake.host_name,
                    &login_start.name,
                )?;
                log::debug!(
                    "{}",
                    qexed_language::t("qexed.connection.login.bungeecord_accepted")
                        .replace("%{name}", &forwarded.profile.username),
                );
                return Ok(Login {
                    profile: forwarded.profile,
                    login_host: forwarded.login_host,
                });
            }
            ForwardingMode::Velocity | ForwardingMode::Victory => {
                let profile = request_velocity_forwarding(
                    packets,
                    sink,
                    &context.config.proxy_token,
                )
                .await?;
                log::debug!(
                    "{}",
                    qexed_language::t("qexed.connection.login.velocity_accepted")
                        .replace("%{name}", &profile.username),
                );
                return Ok(Login {
                    profile,
                    login_host: handshake.host_name.clone(),
                });
            }
            ForwardingMode::Default
            | ForwardingMode::QTunnel
            | ForwardingMode::None => {}
        }
    }

    let online_mode = if context.config.proxy
        && matches!(context.config.proxy_protocol, ForwardingMode::Victory)
    {
        context.config.proxy_online_mode
    } else {
        context.config.online_mode
    };

    let profile = if online_mode {
        authenticate_online(packets, sink, context, login_start).await?
    } else {
        offline_profile(&login_start.name)
    };

    Ok(Login {
        profile,
        login_host: handshake.host_name.clone(),
    })
}

async fn request_velocity_forwarding<R, W>(
    packets: &mut crate::transport::PacketStream<R>,
    sink: &mut crate::transport::PacketSink<W>,
    secret: &str,
) -> Result<qexed_packet::net_types::GameProfile>
where
    R: tokio::io::AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
{
    sink.send(to_client::login::custom_query::CustomQuery {
        transaction_id: qexed_packet::net_types::VarInt(
            crate::proxy_forwarding::VELOCITY_FORWARDING_MESSAGE_ID,
        ),
        payload: qexed_packet::net_types::CustomQueryPayload {
            channel: crate::proxy_forwarding::VELOCITY_FORWARDING_CHANNEL.to_string(),
            data: qexed_packet::net_types::RestBuffer(Vec::new()),
        },
    })
    .await?;
    sink.flush().await?;

    let response = read_expected_packet::<CustomQueryAnswer, _>(packets).await?;
    if response.transaction_id.0 != crate::proxy_forwarding::VELOCITY_FORWARDING_MESSAGE_ID {
        return Err(ConnectionError::ProxyForwarding(format!(
            "Velocity forwarding response message ID mismatch: expected {}, actual {}",
            crate::proxy_forwarding::VELOCITY_FORWARDING_MESSAGE_ID,
            response.transaction_id.0
        )));
    }

    let Some(data) = response.payload else {
        return Err(ConnectionError::ProxyForwarding(
            "Velocity proxy did not provide forwarded player info".to_string(),
        ));
    };

    crate::proxy_forwarding::parse_velocity_forwarding_response(&data.data.0, secret)
}

async fn authenticate_online<R, W>(
    packets: &mut crate::transport::PacketStream<R>,
    sink: &mut crate::transport::PacketSink<W>,
    context: &ServerContext,
    login_start: &Hello,
) -> Result<qexed_packet::net_types::GameProfile>
where
    R: tokio::io::AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
{
    let verify_token = random_verify_token();
    sink.send(to_client::login::hello::Hello {
        server_id: String::new(),
        public_key: context.authenticator.public_key_der()?.into(),
        challenge: verify_token.clone().into(),
        should_authenticate: true,
    })
    .await?;

    let key_packet = read_expected_packet::<Key, _>(packets).await?;
    let shared_secret = context
        .authenticator
        .decrypt_login_key(&key_packet, &verify_token)?;

    packets.enable_encryption(&shared_secret)?;
    sink.enable_encryption(&shared_secret)?;

    let authenticated = context
        .authenticator
        .verify_session(&login_start.name, &shared_secret, None)
        .await?;

    Ok(authenticated.into())
}

async fn disconnect_login<W>(
    sink: &mut crate::transport::PacketSink<W>,
    reason: impl Into<String>,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    sink.send(to_client::login::login_disconnect::LoginDisconnect {
        reason: super::text_component(reason),
    })
    .await?;
    Ok(())
}

fn random_verify_token() -> Vec<u8> {
    let mut token = vec![0_u8; 4];
    rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut token);
    token
}
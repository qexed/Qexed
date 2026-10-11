//! Login 阶段：LoginStart →（可选正版认证）→ LoginSuccess → LoginAcknowledged
//!            → Configuration → （暂）Play Disconnect。

use qexed_auth::{offline_profile, Authenticator};
use qexed_packet::net_types::GameProfile;
use qexed_protocol::{
    to_client,
    to_server::{
        handshaking::set_protocol::SetProtocol,
        login::{
            encryption_begin::EncryptionBegin as ServerboundEncryptionBegin,
            login_acknowledged::LoginAcknowledged,
            login_start::LoginStart,
        },
    },
};

use crate::codec::{read_expected_packet, text_component};
use crate::configuration::handle_configuration;
use crate::consts::{LOGIN_TIMEOUT, SESSION_VERIFY_TIMEOUT};
use crate::error::ServerError;
use crate::handle::ServerContext;

pub(crate) async fn handle_login<R, W>(
    handshake: SetProtocol,
    packets: &mut qexed_tcp_connect::PacketStream<R>,
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    context: &ServerContext,
    peer_ip: std::net::IpAddr,
) -> Result<(), ServerError>
where
    R: tokio::io::AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
{
    // 协议版本不匹配 → 直接断开
    if handshake.protocol_version.0 != qexed_mojang_data::PROTOCOL_VERSION {
        disconnect_login(
            sink,
            format!(
                "Unsupported protocol {}. This server expects {} ({})",
                handshake.protocol_version.0,
                qexed_mojang_data::PROTOCOL_VERSION,
                qexed_mojang_data::MC_VERSION,
            ),
        )
        .await?;
        return Ok(());
    }

    // 1. LoginStart（带超时）
    let login_start = read_expected_packet::<LoginStart, _>(packets, LOGIN_TIMEOUT).await?;
    log::debug!("login start: username={}", login_start.username);

    // 2. 解析 profile：正版 or 离线
    let profile = if context.config.online_mode {
        match authenticate_online(packets, sink, &context.authenticator, &login_start, peer_ip)
            .await
        {
            Ok(profile) => profile,
            Err(err) => {
                log::debug!("online auth failed for {}: {err}", login_start.username);
                disconnect_login(sink, format!("Authentication failed: {err}")).await?;
                return Ok(());
            }
        }
    } else {
        offline_profile(&login_start.username)
    };
    log::debug!("profile resolved: uuid={}", profile.uuid);

    // 3. LoginSuccess
    sink.send(to_client::login::success::Success {
        game_profile: profile.clone(),
    })
    .await?;
    sink.flush().await?;

    // 4. LoginAcknowledged（带超时）
    read_expected_packet::<LoginAcknowledged, _>(packets, LOGIN_TIMEOUT).await?;

    // 5. Configuration（内部管理 KeepAlive + 读超时）
    let client = handle_configuration(packets, sink).await?;
    log::debug!(
        "configuration done: locale={:?}, skin_parts=0x{:02x}",
        client.locale,
        client.displayed_skin_parts,
    );

    // 6. Play 阶段暂未实现 → Play Disconnect
    sink.send(to_client::play::disconnect::Disconnect {
        reason: text_component("Play phase is not implemented yet."),
    })
    .await?;
    sink.flush().await?;

    Ok(())
}

async fn authenticate_online<R, W>(
    packets: &mut qexed_tcp_connect::PacketStream<R>,
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    authenticator: &Authenticator,
    login_start: &LoginStart,
    peer_ip: std::net::IpAddr,
) -> Result<GameProfile, ServerError>
where
    R: tokio::io::AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
{
    let verify_token = random_verify_token();

    sink.send(to_client::login::encryption_begin::EncryptionBegin {
        server_id: String::new(),
        public_key: authenticator.public_key_der()?.into(),
        verify_token: verify_token.clone().into(),
        should_authenticate: true,
    })
    .await?;
    sink.flush().await?;

    let key_packet =
        read_expected_packet::<ServerboundEncryptionBegin, _>(packets, LOGIN_TIMEOUT).await?;
    let shared_secret = authenticator.decrypt_login_key(&key_packet, &verify_token)?;

    packets.enable_encryption(&shared_secret)?;
    sink.enable_encryption(&shared_secret)?;

    // 会话服务校验带独立超时（网络 IO）
    let authenticated = tokio::time::timeout(
        SESSION_VERIFY_TIMEOUT,
        authenticator.verify_session(&login_start.username, &shared_secret, Some(peer_ip)),
    )
    .await
    .map_err(|_| ServerError::Protocol("session verification timed out".into()))??;

    Ok(authenticated.into())
}

async fn disconnect_login<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    reason: impl Into<String>,
) -> Result<(), ServerError>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    sink.send(to_client::login::disconnect::Disconnect {
        reason: qexed_packet::net_types::JsonValue(serde_json::json!({
            "text": reason.into(),
            "color": "red",
        })),
    })
    .await?;
    sink.flush().await?;
    Ok(())
}

fn random_verify_token() -> Vec<u8> {
    let mut token = vec![0_u8; 4];
    rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut token);
    token
}
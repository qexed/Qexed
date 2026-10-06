//! 登录处理（v4 connection::login 迁移）。
//!
//! 包名对齐 v6：SetProtocol→ClientIntention，LoginStart→Hello，
//! EncryptionBegin(server)→Hello(login)，EncryptionBegin(client)→Key，
//! LoginPluginResponse→CustomQueryAnswer，LoginPluginRequest→CustomQuery，
//! Compress→LoginCompression，Success→LoginFinished，
//! Disconnect(login)→LoginDisconnect。
//!
//! v4 登录成功后直接调 crate::play::initialize 进入 play 域；v6 play 域在
//! qexed_play crate，经 ServerContext::play_launcher 回调注入（组装层接线），
//! 未注入时登录完成后断开。v4 的 warden 封禁检查（BanRecord）属于 qexed_server
//! 域，经 ServerContext::ban_check 回调注入：登录起点（客户端自报 UUID，代理
//! 转发场景可在加密前拦截）与认证完成后（最终 profile UUID）各查一次。

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

/// 登录流程最终产出：交给 play 域（qexed_play 经组装层注入的启动器）的资料。
/// 组装层启动器直接接收字段实参（profile/locale/skin_parts），该结构体作为
/// 连接域对外 API 保留（调试/日志/未来重连场景读取登录上下文）。
#[derive(Debug)]
pub struct LoginOutcome {
    pub profile: qexed_packet::net_types::GameProfile,
    pub login_host: String,
    pub client_config: super::configuration::ClientConfiguration,
}

pub(super) async fn handle_login(
    handshake: ClientIntention,
    mut packets: crate::transport::PacketStream<tokio::io::ReadHalf<tokio::net::TcpStream>>,
    mut sink: crate::transport::PacketSink<tokio::io::WriteHalf<tokio::net::TcpStream>>,
    context: &ServerContext,
    shutdown: tokio::sync::watch::Receiver<bool>,
) -> Result<()> {
    if handshake.protocol_version.0 != qexed_config::PROTOCOL_VERSION {
        disconnect_login(
            &mut sink,
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

    let login_start = read_expected_packet::<Hello, _>(&mut packets).await?;
    // 封禁检查（组装层注入的 warden 查询回调）：先按客户端自报 UUID 拦截，
    // 让被Ban玩家在加密握手前就被断开（省一轮 RSA/HTTP）。
    if let Some(ban_check) = &context.ban_check {
        if let Some(reason) = ban_check(login_start.profile_id) {
            disconnect_login(&mut sink, ban_disconnect_reason(&reason)).await?;
            return Ok(());
        }
    }

    let login = match resolve_login(&handshake, &mut packets, &mut sink, context, &login_start).await {
        Ok(login) => login,
        Err(err) => {
            disconnect_login(&mut sink, format!("Authentication failed: {err}")).await?;
            return Ok(());
        }
    };

    // 认证后封禁复查（v4 语义：warden.ban_for(最终 profile.uuid)）。
    // 登录起点已按客户端自报 UUID 查过一次；这里按认证/转发得到的最终 UUID 再查，
    // 覆盖 BungeeCord/Velocity 转发与离线模式（自报与最终 UUID 可能不同）。
    if let Some(ban_check) = &context.ban_check {
        if let Some(reason) = ban_check(login.profile.uuid) {
            disconnect_login(&mut sink, ban_disconnect_reason(&reason)).await?;
            return Ok(());
        }
    }

    if context.config.network_compression_threshold >= 0 {
        let threshold = context.config.network_compression_threshold as i32;
        sink.send(to_client::login::login_compression::LoginCompression {
            compression_threshold: qexed_packet::net_types::VarInt(threshold),
        })
        .await?;
        sink.set_compression_threshold(threshold);
        packets.set_compression_threshold(threshold);
    }

    // 26.3 客户端用该 id 关联聊天会话；nil 会让客户端报聊天"无法验证"警告，
    // 每次登录生成新 v4（原版语义：每个登录流程一个随机会话 id）。
    let __lf_packet = to_client::login::login_finished::LoginFinished {
        game_profile: login.profile.clone(),
        session_id: uuid::Uuid::new_v4(),
    };
    {
        use qexed_packet::{Packet as _, PacketCodec as _};
        let mut __buf = bytes::BytesMut::new();
        let mut __w = qexed_packet::PacketWriter::new(&mut __buf);
        let _ = qexed_packet::net_types::VarInt(to_client::login::login_finished::LoginFinished::ID).serialize(&mut __w);
        let _ = __lf_packet.serialize(&mut __w);
        let n = __buf.len().min(120);
        log::info!("[diag] loginfinished {}B hex={}", __buf.len(), __buf[..n].iter().map(|b| format!("{b:02x}")).collect::<String>());
    }
    sink.send(__lf_packet).await?;

    read_expected_packet::<LoginAcknowledged, _>(&mut packets).await?;
    let client_config = handle_configuration(&mut packets, &mut sink, context, &login.login_host).await?;
    context.ensure_plugins_initialized();

    // play 域初始化：由组装层注入的启动器接管（避免 connection→play 依赖环）。
    let outcome = LoginOutcome {
        profile: login.profile,
        login_host: login.login_host,
        client_config,
    };

    if let Some(launcher) = context.play_launcher.clone() {
        let outcome_result = launcher(
            packets,
            sink,
            outcome.profile.clone(),
            outcome.client_config.locale.clone(),
            outcome.client_config.displayed_skin_parts,
            shutdown,
        )
        .await;
        if let Err(err) = &outcome_result {
            log::error!("[play] 会话错误: {err}");
        }
        return outcome_result;
    }
    Ok(())
}

#[derive(Debug)]
struct Login {
    profile: qexed_packet::net_types::GameProfile,
    login_host: String,
}

/// v4 ban_disconnect_reason：空原因时给通用文案（BanRecord.reason 摘要注入）。
fn ban_disconnect_reason(reason: &str) -> String {
    if reason.trim().is_empty() {
        "You are banned from this server.".to_string()
    } else {
        format!("You are banned from this server: {reason}")
    }
}


async fn resolve_login(
    handshake: &ClientIntention,
    packets: &mut crate::transport::PacketStream<tokio::io::ReadHalf<tokio::net::TcpStream>>,
    sink: &mut crate::transport::PacketSink<tokio::io::WriteHalf<tokio::net::TcpStream>>,
    context: &ServerContext,
    login_start: &Hello,
) -> Result<Login>
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
                    &mut *packets,
                    &mut *sink,
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

async fn request_velocity_forwarding(
    packets: &mut crate::transport::PacketStream<tokio::io::ReadHalf<tokio::net::TcpStream>>,
    sink: &mut crate::transport::PacketSink<tokio::io::WriteHalf<tokio::net::TcpStream>>,
    secret: &str,
) -> Result<qexed_packet::net_types::GameProfile>
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

async fn authenticate_online(
    packets: &mut crate::transport::PacketStream<tokio::io::ReadHalf<tokio::net::TcpStream>>,
    sink: &mut crate::transport::PacketSink<tokio::io::WriteHalf<tokio::net::TcpStream>>,
    context: &ServerContext,
    login_start: &Hello,
) -> Result<qexed_packet::net_types::GameProfile>
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

    // 皮肤诊断：确认 hasJoined 响应的 profile.properties（textures 等）到达。
    // properties 透传链：auth/model.rs SessionProfile → AuthenticatedProfile →
    // GameProfile → LoginFinished / PlayerInfoUpdate（ADD_PLAYER 携带 properties）。
    let property_names: Vec<&str> = authenticated
        .properties
        .iter()
        .map(|property| property.name.as_str())
        .collect();
    if property_names.is_empty() {
        log::warn!(
            "online auth profile has no properties (textures missing); skin will not load for {}",
            authenticated.name
        );
    } else {
        log::info!(
            "online auth profile properties for {}: [{}] (textures {}signed)",
            authenticated.name,
            property_names.join(", "),
            authenticated
                .properties
                .iter()
                .any(|property| property.name == "textures" && property.signature.is_some())
                .then_some("yes-").unwrap_or("un")
        );
    }

    Ok(authenticated.into())
}

async fn disconnect_login(
    sink: &mut crate::transport::PacketSink<tokio::io::WriteHalf<tokio::net::TcpStream>>,
    reason: impl Into<String>,
) -> Result<()>
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

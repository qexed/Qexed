use bytes::BytesMut;
use qexed_auth::{Authenticator, offline_profile};
use qexed_config::Config;
use qexed_packet::net_types::VarInt;
use qexed_packet::{Packet, PacketCodec, PacketReader, PacketWriter};
use qexed_protocol::{
    to_client,
    to_server::{
        configuration::{
            finish_configuration::FinishConfiguration as ServerboundFinishConfiguration,
            select_known_packs::SelectKnownPacks as ServerboundSelectKnownPacks,
            settings::Settings as ServerboundSettings,
        },
        handshaking::set_protocol::SetProtocol,
        login::{
            encryption_begin::EncryptionBegin as ServerboundEncryptionBegin,
            login_acknowledged::LoginAcknowledged,
            login_start::LoginStart,
        },
        status::{ping::Ping as StatusPing, ping_start::PingStart},
    },
};

use crate::error::ServerError;

pub mod config;
pub mod error;

const SERVER_BRAND: &str = "qexed";
const DEFAULT_DISPLAYED_SKIN_PARTS: u8 = 0x7f;

// ─────────────────────────────────────────────────────────
// 上下文
// ─────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct ServerContext {
    pub config: std::sync::Arc<config::ServerConfig>,
    pub authenticator: std::sync::Arc<Authenticator>,
}

// ─────────────────────────────────────────────────────────
// 启动
// ─────────────────────────────────────────────────────────

pub async fn init() -> Result<(), ServerError> {
    qexed_auth::init()?;

    let config = config::ServerConfig::load_and_create_default(true)?;
    let listener: tokio::net::TcpListener = qexed_tcp_connect::bind(&config.bind).await?;

    log::info!(
        "server listening on {} (online_mode={})",
        config.bind,
        config.online_mode,
    );

    let context = ServerContext {
        config: std::sync::Arc::new(config),
        authenticator: std::sync::Arc::new(Authenticator::new()?),
    };

    loop {
        let (stream, peer) = match listener.accept().await {
            Ok(pair) => pair,
            Err(err) => {
                log::warn!("accept failed: {err}");
                continue;
            }
        };

        let ctx = context.clone();
        tokio::spawn(async move {
            if let Err(err) = handle_connection(stream, peer, ctx).await {
                log::debug!("connection ended: peer={peer}, error={err:#}");
            }
        });
    }
}

// ─────────────────────────────────────────────────────────
// 单连接
// ─────────────────────────────────────────────────────────

async fn handle_connection(
    stream: tokio::net::TcpStream,
    peer: std::net::SocketAddr,
    context: ServerContext,
) -> Result<(), ServerError> {
    let _ = stream.set_nodelay(true);

    let (reader, writer) = tokio::io::split(stream);
    let mut packets = qexed_tcp_connect::PacketStream::new(reader);
    let mut sink = qexed_tcp_connect::PacketSink::new(writer);

    // 1. 握手
    let Some(payload) = packets.read_packet().await? else {
        log::debug!("connection closed before handshake: peer={peer}");
        return Ok(());
    };
    let handshake = decode_handshake(payload)?;

    log::debug!(
        "handshake: peer={peer}, protocol={}, host={}, port={}, next_state={}",
        handshake.protocol_version.0,
        handshake.server_host,
        handshake.server_port,
        handshake.next_state.0,
    );

    // 2. 分发
    match handshake.next_state.0 {
        1 => handle_status(&mut packets, &mut sink).await?,
        2 => handle_login(handshake, &mut packets, &mut sink, &context, peer.ip()).await?,
        other => log::warn!("unsupported next_state from {peer}: {other}"),
    }

    let _ = sink.flush().await;
    Ok(())
}

// ─────────────────────────────────────────────────────────
// 握手解析
// ─────────────────────────────────────────────────────────

fn decode_handshake(mut payload: BytesMut) -> Result<SetProtocol, ServerError> {
    let mut reader = PacketReader::new(&mut payload);
    let packet_id: VarInt = reader
        .deserialize()
        .map_err(|err| ServerError::Protocol(err.to_string()))?;
    if packet_id.0 != SetProtocol::ID {
        return Err(ServerError::Protocol(format!(
            "unexpected handshake packet id: expected {}, got {}",
            SetProtocol::ID,
            packet_id.0
        )));
    }
    let mut handshake = SetProtocol::default();
    handshake
        .deserialize(&mut reader)
        .map_err(|err| ServerError::Protocol(err.to_string()))?;
    Ok(handshake)
}

// ─────────────────────────────────────────────────────────
// Status
// ─────────────────────────────────────────────────────────

async fn handle_status<R, W>(
    packets: &mut qexed_tcp_connect::PacketStream<R>,
    sink: &mut qexed_tcp_connect::PacketSink<W>,
) -> Result<(), ServerError>
where
    R: tokio::io::AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
{
    read_expected_packet::<PingStart, _>(packets).await?;

    let json = serde_json::json!({
        "version": {
            "name": qexed_mojang_data::MC_VERSION,
            "protocol": qexed_mojang_data::PROTOCOL_VERSION,
        },
        "players": {
            "max": 20,
            "online": 0,
            "sample": [],
        },
        "description": {
            "text": "Qexed",
        },
    });

    sink.send(to_client::status::server_info::ServerInfo {
        response: qexed_packet::net_types::JsonValue(json),
    })
    .await?;
    sink.flush().await?;

    if let Some(mut payload) = packets.read_packet().await? {
        let id = read_packet_id(&mut payload)?;
        if id == StatusPing::ID {
            let ping = decode_payload::<StatusPing>(&mut payload)?;
            sink.send(to_client::status::ping::Ping { time: ping.time })
                .await?;
            sink.flush().await?;
        }
    }

    Ok(())
}

// ─────────────────────────────────────────────────────────
// Login
// ─────────────────────────────────────────────────────────

async fn handle_login<R, W>(
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

    // 1. LoginStart
    let login_start = read_expected_packet::<LoginStart, _>(packets).await?;
    log::debug!("login start: username={}", login_start.username);

    // 2. 解析 profile（正版 or 离线）
    let profile = if context.config.online_mode {
        match authenticate_online(
            packets,
            sink,
            &context.authenticator,
            &login_start,
            peer_ip,
        )
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

    // 4. LoginAcknowledged
    read_expected_packet::<LoginAcknowledged, _>(packets).await?;

    // 5. configuration
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
) -> Result<qexed_packet::net_types::GameProfile, ServerError>
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

    let key_packet = read_expected_packet::<ServerboundEncryptionBegin, _>(packets).await?;
    let shared_secret = authenticator.decrypt_login_key(&key_packet, &verify_token)?;

    packets.enable_encryption(&shared_secret)?;
    sink.enable_encryption(&shared_secret)?;

    let authenticated = authenticator
        .verify_session(&login_start.username, &shared_secret, Some(peer_ip))
        .await?;

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

// ─────────────────────────────────────────────────────────
// Configuration
// ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
struct ClientConfiguration {
    locale: Option<String>,
    displayed_skin_parts: u8,
}

async fn handle_configuration<R, W>(
    packets: &mut qexed_tcp_connect::PacketStream<R>,
    sink: &mut qexed_tcp_connect::PacketSink<W>,
) -> Result<ClientConfiguration, ServerError>
where
    R: tokio::io::AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
{
    // 1. 品牌
    sink.send(to_client::configuration::custom_payload::CustomPayload {
        channel: "minecraft:brand".to_string(),
        data: qexed_packet::net_types::RestBuffer(string_payload(SERVER_BRAND)?),
    })
    .await?;

    // 2. 功能开关
    sink.send(to_client::configuration::feature_flags::FeatureFlags {
        features: vec![qexed_registry::VANILLA_FEATURE.to_string()],
    })
    .await?;

    // 3. 请求客户端选择 known packs
    let known_packs = qexed_registry::known_packs();
    sink.send(to_client::configuration::select_known_packs::SelectKnownPacks {
        known_packs: known_packs.clone(),
    })
    .await?;
    sink.flush().await?;

    // 4. 等客户端 SelectKnownPacks（中途可能先收到 Settings）
    let mut client = ClientConfiguration {
        displayed_skin_parts: DEFAULT_DISPLAYED_SKIN_PARTS,
        ..Default::default()
    };
    let selected = loop {
        let Some(mut payload) = packets.read_packet().await? else {
            return Err(ServerError::Protocol(
                "connection closed while waiting for known packs".into(),
            ));
        };
        let id = read_packet_id(&mut payload)?;
        if id == ServerboundSettings::ID {
            let settings = decode_payload::<ServerboundSettings>(&mut payload)?;
            client.locale = Some(settings.locale);
            client.displayed_skin_parts = settings.displayed_skin_parts;
            continue;
        }
        if id == ServerboundSelectKnownPacks::ID {
            break decode_payload::<ServerboundSelectKnownPacks>(&mut payload)?;
        }
        log::debug!("skip config packet while waiting for known packs: {id}");
    };

    // 5. 客户端没选中 vanilla core → 发完整注册表内容
    let include_full = !qexed_registry::accepts_vanilla_core_pack(&selected.entries);
    log::debug!(
        "client known packs: {:?}, include_full_registry={}",
        selected.entries,
        include_full,
    );

    // 6. Registry Data
    for packet in qexed_registry::load_registry_packets(include_full)
        .map_err(|err| ServerError::Protocol(format!("registry load failed: {err}")))?
    {
        sink.send(packet).await?;
    }

    // 7. Tags
    sink.send(
        qexed_registry::load_tag_packet()
            .map_err(|err| ServerError::Protocol(format!("tag load failed: {err}")))?,
    )
    .await?;

    // 8. FinishConfiguration
    sink.send(to_client::configuration::finish_configuration::FinishConfiguration {})
        .await?;
    sink.flush().await?;

    // 9. 等客户端 ack
    read_expected_packet::<ServerboundFinishConfiguration, _>(packets).await?;

    Ok(client)
}

// ─────────────────────────────────────────────────────────
// codec 辅助
// ─────────────────────────────────────────────────────────

async fn read_expected_packet<T, R>(
    packets: &mut qexed_tcp_connect::PacketStream<R>,
) -> Result<T, ServerError>
where
    T: Packet + Default,
    R: tokio::io::AsyncRead + Unpin,
{
    let Some(mut payload) = packets.read_packet().await? else {
        return Err(ServerError::Protocol(
            "connection closed while reading packet".into(),
        ));
    };
    let id = read_packet_id(&mut payload)?;
    if id != T::ID {
        return Err(ServerError::Protocol(format!(
            "packet ID mismatch: expected {}, actual {}",
            T::ID,
            id
        )));
    }
    decode_payload::<T>(&mut payload)
}

fn read_packet_id(payload: &mut BytesMut) -> Result<i32, ServerError> {
    let mut reader = PacketReader::new(payload);
    let mut id = VarInt::default();
    id.deserialize(&mut reader)
        .map_err(|err| ServerError::Protocol(err.to_string()))?;
    Ok(id.0)
}

fn decode_payload<T>(payload: &mut BytesMut) -> Result<T, ServerError>
where
    T: Packet + Default,
{
    let mut reader = PacketReader::new(payload);
    let mut packet = T::default();
    packet
        .deserialize(&mut reader)
        .map_err(|err| ServerError::Protocol(err.to_string()))?;
    Ok(packet)
}

fn string_payload(value: &str) -> Result<Vec<u8>, ServerError> {
    let mut payload = BytesMut::new();
    let mut writer = PacketWriter::new(&mut payload);
    value
        .to_string()
        .serialize(&mut writer)
        .map_err(|err| ServerError::Protocol(err.to_string()))?;
    Ok(payload.to_vec())
}

fn text_component(text: impl Into<String>) -> qexed_protocol::types::TextComponent {
    let mut map = std::collections::HashMap::new();
    map.insert(
        "text".to_string(),
        qexed_nbt::Tag::String(std::sync::Arc::from(text.into())),
    );
    qexed_nbt::Tag::Compound(std::sync::Arc::new(map))
}
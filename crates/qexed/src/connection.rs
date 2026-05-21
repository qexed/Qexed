use bytes::BytesMut;
use qexed_packet::{Packet, PacketCodec};
use qexed_protocol::{
    to_client,
    to_server::{
        handshaking::set_protocol::SetProtocol,
        login::{encryption_begin::EncryptionBegin as ServerboundKey, login_start::LoginStart},
        status::{ping::Ping as StatusPing, ping_start::PingStart},
    },
};
use tokio::net::TcpStream;

use crate::auth::{Authenticator, offline_profile};

#[derive(Clone)]
pub struct ServerContext {
    pub config: std::sync::Arc<qexed_config::app::qexed::Qexed>,
    pub authenticator: std::sync::Arc<Authenticator>,
}

impl ServerContext {
    pub fn new(config: qexed_config::app::qexed::Qexed) -> anyhow::Result<Self> {
        Ok(Self {
            config: std::sync::Arc::new(config),
            authenticator: std::sync::Arc::new(Authenticator::new()?),
        })
    }
}

pub async fn handle(stream: TcpStream, peer_addr: std::net::SocketAddr, context: ServerContext) {
    if let Err(err) = handle_inner(stream, peer_addr, context).await {
        log::error!("连接处理错误: {err}");
    }
    log::info!("连接关闭: {peer_addr}");
}

async fn handle_inner(
    stream: TcpStream,
    _peer_addr: std::net::SocketAddr,
    context: ServerContext,
) -> anyhow::Result<()> {
    let (reader, writer) = tokio::io::split(stream);
    let mut packets = qexed_tcp_connect::PacketStream::new(reader);
    let mut sink = qexed_tcp_connect::PacketSink::new(writer);

    let handshake = read_expected_packet::<SetProtocol, _>(&mut packets).await?;
    match handshake.next_state.0 {
        1 => handle_status(&mut packets, &mut sink, &context).await,
        2 => handle_login(handshake, &mut packets, &mut sink, &context).await,
        state => anyhow::bail!("不支持的握手目标状态: {state}"),
    }
}

async fn handle_status<R, W>(
    packets: &mut qexed_tcp_connect::PacketStream<R>,
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    context: &ServerContext,
) -> anyhow::Result<()>
where
    R: tokio::io::AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
{
    read_expected_packet::<PingStart, _>(packets).await?;
    sink.send(to_client::status::server_info::ServerInfo {
        response: qexed_packet::net_types::JsonValue(crate::status::response(&context.config)),
    })
    .await?;

    if let Some(mut payload) = packets.read_packet().await? {
        let packet_id = read_packet_id(&mut payload)?;
        if packet_id == StatusPing::ID {
            let ping = decode_payload::<StatusPing>(&mut payload)?;
            sink.send(to_client::status::ping::Ping { time: ping.time })
                .await?;
        }
    }

    Ok(())
}

async fn handle_login<R, W>(
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
    let profile = if context.config.server.online_mode {
        match authenticate_online(packets, sink, context, &login_start).await {
            Ok(profile) => profile,
            Err(err) => {
                disconnect_login(sink, format!("Authentication failed: {err}")).await?;
                return Ok(());
            }
        }
    } else {
        offline_profile(&login_start.username)
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
        game_profile: profile,
    })
    .await?;

    Ok(())
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

async fn read_expected_packet<T, R>(
    packets: &mut qexed_tcp_connect::PacketStream<R>,
) -> anyhow::Result<T>
where
    T: qexed_packet::Packet + Default,
    R: tokio::io::AsyncRead + Unpin,
{
    let Some(mut payload) = packets.read_packet().await? else {
        anyhow::bail!("连接在读取数据包时关闭");
    };

    let packet_id = read_packet_id(&mut payload)?;
    if packet_id != T::ID {
        anyhow::bail!("数据包 ID 不匹配: 期望 {}, 实际 {}", T::ID, packet_id);
    }

    decode_payload::<T>(&mut payload)
}

fn read_packet_id(payload: &mut BytesMut) -> anyhow::Result<i32> {
    let mut reader = qexed_packet::PacketReader::new(payload);
    let mut packet_id = qexed_packet::net_types::VarInt::default();
    packet_id.deserialize(&mut reader)?;
    Ok(packet_id.0)
}

fn decode_payload<T>(payload: &mut BytesMut) -> anyhow::Result<T>
where
    T: qexed_packet::Packet + Default,
{
    let mut reader = qexed_packet::PacketReader::new(payload);
    let mut packet = T::default();
    packet.deserialize(&mut reader)?;
    Ok(packet)
}

fn random_verify_token() -> Vec<u8> {
    let mut token = vec![0_u8; 4];
    rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut token);
    token
}

#[cfg(test)]
mod tests {
    use qexed_packet::{Packet, PacketCodec};
    use qexed_protocol::to_server::handshaking::set_protocol::SetProtocol;

    use super::{decode_payload, read_packet_id};

    #[test]
    fn reading_packet_id_leaves_payload_at_packet_body() {
        let packet = SetProtocol {
            protocol_version: qexed_packet::net_types::VarInt(qexed_config::PROTOCOL_VERSION),
            server_host: "127.0.0.1".to_string(),
            server_port: 25565,
            next_state: qexed_packet::net_types::VarInt(2),
        };
        let mut payload = bytes::BytesMut::new();
        let mut writer = qexed_packet::PacketWriter::new(&mut payload);
        qexed_packet::net_types::VarInt(SetProtocol::ID)
            .serialize(&mut writer)
            .unwrap();
        packet.serialize(&mut writer).unwrap();

        assert_eq!(read_packet_id(&mut payload).unwrap(), SetProtocol::ID);
        let decoded = decode_payload::<SetProtocol>(&mut payload).unwrap();

        assert_eq!(decoded.protocol_version.0, qexed_config::PROTOCOL_VERSION);
        assert_eq!(decoded.next_state.0, 2);
    }
}

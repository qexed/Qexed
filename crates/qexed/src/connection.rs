use bytes::BytesMut;
use qexed_packet::{Packet, PacketCodec};
use qexed_protocol::{
    to_client,
    to_server::{
        configuration::{
            accept_code_of_conduct::AcceptCodeOfConduct,
            finish_configuration::FinishConfiguration as ServerboundFinishConfiguration,
            select_known_packs::SelectKnownPacks as ServerboundSelectKnownPacks,
        },
        handshaking::set_protocol::SetProtocol,
        login::{
            encryption_begin::EncryptionBegin as ServerboundKey,
            login_acknowledged::LoginAcknowledged, login_start::LoginStart,
        },
        status::{ping::Ping as StatusPing, ping_start::PingStart},
    },
};
use tokio::net::TcpStream;

use crate::auth::{Authenticator, offline_profile};

const SERVER_BRAND: &str = "qexed";

#[derive(Clone)]
pub struct ServerContext {
    pub config: std::sync::Arc<qexed_config::app::qexed::Qexed>,
    pub authenticator: std::sync::Arc<Authenticator>,
    pub world: std::sync::Arc<crate::world::WorldManager>,
    pub players: std::sync::Arc<crate::players::PlayerManager>,
}

impl ServerContext {
    pub fn new(config: qexed_config::app::qexed::Qexed) -> anyhow::Result<Self> {
        let world = crate::world::WorldManager::with_light_mode(
            config.server.world.path.clone(),
            crate::world::WorldLightMode::from(&config.server.world.light),
            crate::world::WorldLightAlgorithm::from(&config.server.world.light_algorithm),
            crate::world::light_gpu_from_config(&config.server.world.gpu),
        );
        world.ensure_storage(&config.server.world.dimension)?;
        Ok(Self {
            config: std::sync::Arc::new(config),
            authenticator: std::sync::Arc::new(Authenticator::new()?),
            world: std::sync::Arc::new(world),
            players: std::sync::Arc::new(crate::players::PlayerManager::new()),
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
        game_profile: profile.clone(),
    })
    .await?;

    read_expected_packet::<LoginAcknowledged, _>(packets).await?;
    handle_configuration(packets, sink, context).await?;
    crate::play::initialize(
        packets,
        sink,
        &context.config,
        &context.authenticator,
        &context.world,
        &context.players,
        &profile,
    )
    .await?;

    Ok(())
}

async fn handle_configuration<R, W>(
    packets: &mut qexed_tcp_connect::PacketStream<R>,
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    context: &ServerContext,
) -> anyhow::Result<()>
where
    R: tokio::io::AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
{
    log::debug!("发送配置态服务端品牌: {SERVER_BRAND}");
    sink.send(to_client::configuration::custom_payload::CustomPayload {
        channel: "minecraft:brand".to_string(),
        data: qexed_packet::net_types::RestBuffer(string_payload(SERVER_BRAND)?),
    })
    .await?;

    log::debug!(
        "发送配置态功能标志: {}",
        crate::registry_sync::VANILLA_FEATURE
    );
    sink.send(to_client::configuration::feature_flags::FeatureFlags {
        features: vec![crate::registry_sync::VANILLA_FEATURE.to_string()],
    })
    .await?;

    let known_packs = crate::registry_sync::known_packs();
    log::debug!("请求客户端确认已知资源包: {known_packs:?}");
    sink.send(to_client::configuration::select_known_packs::SelectKnownPacks { known_packs })
        .await?;
    sink.flush().await?;

    let selected_packs = wait_for_known_packs(packets).await?;
    log::debug!("客户端选择已知资源包: {:?}", selected_packs.entries);
    let include_registry_contents =
        !crate::registry_sync::accepts_vanilla_core_pack(&selected_packs.entries);
    log::debug!("发送注册表数据，包含完整内容: {include_registry_contents}");

    let registry_packets = crate::registry_sync::load_registry_packets(include_registry_contents)?;
    log::debug!("准备发送注册表数据包数量: {}", registry_packets.len());
    for packet in registry_packets {
        log::trace!("发送注册表: {}", packet.id);
        sink.send(packet).await?;
    }

    let tag_packet = crate::registry_sync::load_tag_packet()?;
    log::debug!("发送标签数据，注册表数量: {}", tag_packet.tags.len());
    sink.send(tag_packet).await?;

    let code_of_conduct = context.config.server.code_of_conduct.trim();
    if !code_of_conduct.is_empty() {
        log::debug!("发送入服准则并等待客户端接受");
        sink.send(to_client::configuration::code_of_conduct::CodeOfConduct {
            code_of_conduct: code_of_conduct.to_string(),
        })
        .await?;
        wait_for_code_of_conduct_accept(packets).await?;
        log::debug!("客户端已接受入服准则");
    }

    log::debug!("发送配置完成包");
    sink.send(to_client::configuration::finish_configuration::FinishConfiguration {})
        .await?;
    sink.flush().await?;
    wait_for_finish_configuration(packets).await?;
    log::debug!("客户端已完成配置");

    Ok(())
}

async fn wait_for_known_packs<R>(
    packets: &mut qexed_tcp_connect::PacketStream<R>,
) -> anyhow::Result<ServerboundSelectKnownPacks>
where
    R: tokio::io::AsyncRead + Unpin,
{
    loop {
        let Some(mut payload) = packets.read_packet().await? else {
            anyhow::bail!("连接在等待已知资源包选择时关闭");
        };

        let packet_id = read_packet_id(&mut payload)?;
        if packet_id == ServerboundSelectKnownPacks::ID {
            return decode_payload::<ServerboundSelectKnownPacks>(&mut payload);
        }

        log::debug!("等待已知资源包选择时跳过配置态数据包 ID: {packet_id}");
    }
}

async fn wait_for_code_of_conduct_accept<R>(
    packets: &mut qexed_tcp_connect::PacketStream<R>,
) -> anyhow::Result<()>
where
    R: tokio::io::AsyncRead + Unpin,
{
    loop {
        let Some(mut payload) = packets.read_packet().await? else {
            anyhow::bail!("连接在等待接受入服准则时关闭");
        };

        let packet_id = read_packet_id(&mut payload)?;
        if packet_id == AcceptCodeOfConduct::ID {
            decode_payload::<AcceptCodeOfConduct>(&mut payload)?;
            return Ok(());
        }

        log::debug!("等待接受入服准则时跳过配置态数据包 ID: {packet_id}");
    }
}

async fn wait_for_finish_configuration<R>(
    packets: &mut qexed_tcp_connect::PacketStream<R>,
) -> anyhow::Result<()>
where
    R: tokio::io::AsyncRead + Unpin,
{
    loop {
        let Some(mut payload) = packets.read_packet().await? else {
            anyhow::bail!("连接在等待完成配置时关闭");
        };

        let packet_id = read_packet_id(&mut payload)?;
        if packet_id == ServerboundFinishConfiguration::ID {
            decode_payload::<ServerboundFinishConfiguration>(&mut payload)?;
            return Ok(());
        }

        log::debug!("等待完成配置时跳过配置态数据包 ID: {packet_id}");
    }
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

pub(crate) fn read_packet_id(payload: &mut BytesMut) -> anyhow::Result<i32> {
    let mut reader = qexed_packet::PacketReader::new(payload);
    let mut packet_id = qexed_packet::net_types::VarInt::default();
    packet_id.deserialize(&mut reader)?;
    Ok(packet_id.0)
}

pub(crate) fn decode_payload<T>(payload: &mut BytesMut) -> anyhow::Result<T>
where
    T: qexed_packet::Packet + Default,
{
    let mut reader = qexed_packet::PacketReader::new(payload);
    let mut packet = T::default();
    packet.deserialize(&mut reader)?;
    Ok(packet)
}

fn string_payload(value: &str) -> anyhow::Result<Vec<u8>> {
    let mut payload = BytesMut::new();
    let mut writer = qexed_packet::PacketWriter::new(&mut payload);
    value.to_string().serialize(&mut writer)?;
    Ok(payload.to_vec())
}

fn random_verify_token() -> Vec<u8> {
    let mut token = vec![0_u8; 4];
    rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut token);
    token
}

#[cfg(test)]
mod tests {
    use qexed_packet::{Packet, PacketCodec};
    use qexed_protocol::{
        to_client::configuration::{
            code_of_conduct::CodeOfConduct,
            custom_payload::CustomPayload as ClientboundCustomPayload, feature_flags::FeatureFlags,
            finish_configuration::FinishConfiguration as ClientboundFinishConfiguration,
            registry_data::RegistryData,
            select_known_packs::SelectKnownPacks as ClientboundSelectKnownPacks, tags::Tags,
        },
        to_server::{
            configuration::{
                accept_code_of_conduct::AcceptCodeOfConduct, custom_payload::CustomPayload,
                finish_configuration::FinishConfiguration as ServerboundFinishConfiguration,
                select_known_packs::SelectKnownPacks as ServerboundSelectKnownPacks,
            },
            handshaking::set_protocol::SetProtocol,
        },
    };
    use tokio::io::duplex;

    use super::{
        SERVER_BRAND, ServerContext, decode_payload, handle_configuration, read_packet_id,
    };

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

    #[tokio::test]
    async fn empty_code_of_conduct_skips_prompt_but_finishes_configuration() {
        let context = ServerContext::new(qexed_config::app::qexed::Qexed::default()).unwrap();
        let (server_io, client_io) = duplex(32 * 1024 * 1024);
        let (server_reader, server_writer) = tokio::io::split(server_io);
        let (client_reader, client_writer) = tokio::io::split(client_io);
        let mut packets = qexed_tcp_connect::PacketStream::new(server_reader);
        let mut sink = qexed_tcp_connect::PacketSink::new(server_writer);
        let mut client_packets = qexed_tcp_connect::PacketStream::new(client_reader);
        let mut client_sink = qexed_tcp_connect::PacketSink::new(client_writer);

        let server_task =
            tokio::spawn(
                async move { handle_configuration(&mut packets, &mut sink, &context).await },
            );

        drive_known_pack_selection(&mut client_packets, &mut client_sink).await;

        read_configuration_until_finish(&mut client_packets, false).await;
        client_sink
            .send(ServerboundFinishConfiguration {})
            .await
            .unwrap();
        client_sink.flush().await.unwrap();

        server_task.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn non_empty_code_of_conduct_sends_prompt_and_waits_for_accept() {
        let mut config = qexed_config::app::qexed::Qexed::default();
        config.server.code_of_conduct = "遵守服务器规则".to_string();
        let context = ServerContext::new(config).unwrap();

        let (server_io, client_io) = duplex(32 * 1024 * 1024);
        let (server_reader, server_writer) = tokio::io::split(server_io);
        let (client_reader, client_writer) = tokio::io::split(client_io);
        let mut server_packets = qexed_tcp_connect::PacketStream::new(server_reader);
        let mut server_sink = qexed_tcp_connect::PacketSink::new(server_writer);
        let mut client_packets = qexed_tcp_connect::PacketStream::new(client_reader);
        let mut client_sink = qexed_tcp_connect::PacketSink::new(client_writer);

        let server_task = tokio::spawn(async move {
            handle_configuration(&mut server_packets, &mut server_sink, &context).await
        });

        drive_known_pack_selection(&mut client_packets, &mut client_sink).await;

        let prompt = read_configuration_until_code_of_conduct(&mut client_packets).await;
        assert_eq!(prompt.code_of_conduct, "遵守服务器规则");

        client_sink.send(AcceptCodeOfConduct {}).await.unwrap();
        client_sink.flush().await.unwrap();

        let finish =
            read_server_packet_as::<ClientboundFinishConfiguration, _>(&mut client_packets).await;
        assert_eq!(finish, ClientboundFinishConfiguration {});
        client_sink
            .send(ServerboundFinishConfiguration {})
            .await
            .unwrap();
        client_sink.flush().await.unwrap();

        server_task.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn code_of_conduct_wait_skips_prior_configuration_packets() {
        let mut config = qexed_config::app::qexed::Qexed::default();
        config.server.code_of_conduct = "遵守服务器规则".to_string();
        let context = ServerContext::new(config).unwrap();

        let (server_io, client_io) = duplex(32 * 1024 * 1024);
        let (server_reader, server_writer) = tokio::io::split(server_io);
        let (client_reader, client_writer) = tokio::io::split(client_io);
        let mut server_packets = qexed_tcp_connect::PacketStream::new(server_reader);
        let mut server_sink = qexed_tcp_connect::PacketSink::new(server_writer);
        let mut client_packets = qexed_tcp_connect::PacketStream::new(client_reader);
        let mut client_sink = qexed_tcp_connect::PacketSink::new(client_writer);

        let server_task = tokio::spawn(async move {
            handle_configuration(&mut server_packets, &mut server_sink, &context).await
        });

        drive_known_pack_selection(&mut client_packets, &mut client_sink).await;
        let prompt = read_configuration_until_code_of_conduct(&mut client_packets).await;
        assert_eq!(prompt.code_of_conduct, "遵守服务器规则");

        let custom_payload = qexed_tcp_connect::PacketSink::<
            tokio::io::WriteHalf<tokio::io::DuplexStream>,
        >::build_send_packet(CustomPayload {
            channel: "minecraft:brand".to_string(),
            data: qexed_packet::net_types::RestBuffer(Vec::new()),
        })
        .unwrap();
        let accept_payload = qexed_tcp_connect::PacketSink::<
            tokio::io::WriteHalf<tokio::io::DuplexStream>,
        >::build_send_packet(AcceptCodeOfConduct {})
        .unwrap();
        client_sink.send_raw(custom_payload).await.unwrap();
        client_sink.send_raw(accept_payload).await.unwrap();
        client_sink.flush().await.unwrap();

        let finish =
            read_server_packet_as::<ClientboundFinishConfiguration, _>(&mut client_packets).await;
        assert_eq!(finish, ClientboundFinishConfiguration {});
        client_sink
            .send(ServerboundFinishConfiguration {})
            .await
            .unwrap();
        client_sink.flush().await.unwrap();

        server_task.await.unwrap().unwrap();
    }

    async fn drive_known_pack_selection<R, W>(
        client_packets: &mut qexed_tcp_connect::PacketStream<R>,
        client_sink: &mut qexed_tcp_connect::PacketSink<W>,
    ) where
        R: tokio::io::AsyncRead + Unpin,
        W: tokio::io::AsyncWrite + Unpin,
    {
        let brand = read_server_packet_as::<ClientboundCustomPayload, _>(client_packets).await;
        assert_eq!(brand.channel, "minecraft:brand");
        assert_eq!(decode_string_payload(&brand.data.0), SERVER_BRAND);

        let features = read_server_packet_as::<FeatureFlags, _>(client_packets).await;
        assert_eq!(
            features.features,
            vec![crate::registry_sync::VANILLA_FEATURE.to_string()]
        );

        let known_packs =
            read_server_packet_as::<ClientboundSelectKnownPacks, _>(client_packets).await;
        client_sink
            .send(ServerboundSelectKnownPacks {
                entries: known_packs.known_packs,
            })
            .await
            .unwrap();
        client_sink.flush().await.unwrap();
    }

    async fn read_configuration_until_code_of_conduct<R>(
        client_packets: &mut qexed_tcp_connect::PacketStream<R>,
    ) -> CodeOfConduct
    where
        R: tokio::io::AsyncRead + Unpin,
    {
        let mut saw_registry_data = false;
        let mut saw_tags = false;

        loop {
            let (packet_id, mut payload) = read_server_packet(client_packets).await;
            match packet_id {
                RegistryData::ID => {
                    let packet = decode_payload::<RegistryData>(&mut payload).unwrap();
                    assert!(!packet.entries.is_empty());
                    saw_registry_data = true;
                }
                Tags::ID => {
                    let packet = decode_payload::<Tags>(&mut payload).unwrap();
                    assert!(
                        packet
                            .tags
                            .iter()
                            .any(|tags| tags.registry == "minecraft:worldgen/biome")
                    );
                    saw_tags = true;
                }
                CodeOfConduct::ID => {
                    assert!(saw_registry_data);
                    assert!(saw_tags);
                    return decode_payload::<CodeOfConduct>(&mut payload).unwrap();
                }
                ClientboundFinishConfiguration::ID => {
                    panic!("入服准则未确认前不应完成配置");
                }
                _ => {}
            }
        }
    }

    async fn read_configuration_until_finish<R>(
        client_packets: &mut qexed_tcp_connect::PacketStream<R>,
        expect_code_of_conduct: bool,
    ) where
        R: tokio::io::AsyncRead + Unpin,
    {
        let mut saw_registry_data = false;
        let mut saw_tags = false;

        loop {
            let (packet_id, mut payload) = read_server_packet(client_packets).await;
            match packet_id {
                RegistryData::ID => {
                    let packet = decode_payload::<RegistryData>(&mut payload).unwrap();
                    assert!(!packet.entries.is_empty());
                    saw_registry_data = true;
                }
                Tags::ID => {
                    let packet = decode_payload::<Tags>(&mut payload).unwrap();
                    assert!(
                        packet
                            .tags
                            .iter()
                            .any(|tags| tags.registry == "minecraft:block")
                    );
                    saw_tags = true;
                }
                CodeOfConduct::ID if !expect_code_of_conduct => {
                    panic!("空入服准则配置不应发送入服准则提示");
                }
                ClientboundFinishConfiguration::ID => {
                    decode_payload::<ClientboundFinishConfiguration>(&mut payload).unwrap();
                    assert!(saw_registry_data);
                    assert!(saw_tags);
                    return;
                }
                _ => {}
            }
        }
    }

    async fn read_server_packet_as<T, R>(
        client_packets: &mut qexed_tcp_connect::PacketStream<R>,
    ) -> T
    where
        T: Packet + Default,
        R: tokio::io::AsyncRead + Unpin,
    {
        let (packet_id, mut payload) = read_server_packet(client_packets).await;
        assert_eq!(packet_id, T::ID);
        decode_payload::<T>(&mut payload).unwrap()
    }

    async fn read_server_packet<R>(
        client_packets: &mut qexed_tcp_connect::PacketStream<R>,
    ) -> (i32, bytes::BytesMut)
    where
        R: tokio::io::AsyncRead + Unpin,
    {
        let mut payload = client_packets.read_packet().await.unwrap().unwrap();
        let packet_id = read_packet_id(&mut payload).unwrap();
        (packet_id, payload)
    }

    fn decode_string_payload(payload: &[u8]) -> String {
        let mut bytes = bytes::BytesMut::from(payload);
        let mut reader = qexed_packet::PacketReader::new(&mut bytes);
        let mut value = String::new();
        value.deserialize(&mut reader).unwrap();
        value
    }
}

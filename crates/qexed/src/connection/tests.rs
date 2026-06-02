use qexed_packet::{Packet, PacketCodec};
use qexed_protocol::{
    to_client::configuration::{
        add_resource_pack::AddResourcePack, code_of_conduct::CodeOfConduct,
        custom_payload::CustomPayload as ClientboundCustomPayload, feature_flags::FeatureFlags,
        finish_configuration::FinishConfiguration as ClientboundFinishConfiguration,
        registry_data::RegistryData,
        select_known_packs::SelectKnownPacks as ClientboundSelectKnownPacks, tags::Tags,
    },
    to_server::{
        configuration::{
            accept_code_of_conduct::AcceptCodeOfConduct, custom_payload::CustomPayload,
            finish_configuration::FinishConfiguration as ServerboundFinishConfiguration,
            resource_pack_receive::ResourcePackReceive,
            select_known_packs::SelectKnownPacks as ServerboundSelectKnownPacks,
            settings::Settings as ServerboundSettings,
        },
        handshaking::set_protocol::SetProtocol,
    },
};
use tokio::io::duplex;

use super::{SERVER_BRAND, ServerContext, decode_payload, handle_configuration, read_packet_id};

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
    let context = ServerContext::new(qexed_config::app::qexed::Qexed::default())
        .await
        .unwrap();
    let (server_io, client_io) = duplex(32 * 1024 * 1024);
    let (server_reader, server_writer) = tokio::io::split(server_io);
    let (client_reader, client_writer) = tokio::io::split(client_io);
    let mut packets = qexed_tcp_connect::PacketStream::new(server_reader);
    let mut sink = qexed_tcp_connect::PacketSink::new(server_writer);
    let mut client_packets = qexed_tcp_connect::PacketStream::new(client_reader);
    let mut client_sink = qexed_tcp_connect::PacketSink::new(client_writer);

    let server_task = tokio::spawn(async move {
        handle_configuration(&mut packets, &mut sink, &context, "127.0.0.1").await
    });

    drive_known_pack_selection(&mut client_packets, &mut client_sink).await;

    read_configuration_until_finish(&mut client_packets, false).await;
    client_sink
        .send(ServerboundFinishConfiguration {})
        .await
        .unwrap();
    client_sink.flush().await.unwrap();

    let client_config = server_task.await.unwrap().unwrap();
    assert_eq!(client_config.locale.as_deref(), None);
    assert_eq!(
        client_config.displayed_skin_parts,
        crate::players::DEFAULT_DISPLAYED_SKIN_PARTS
    );
}

#[tokio::test]
async fn non_empty_code_of_conduct_sends_prompt_and_waits_for_accept() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("en_us.txt"), "Follow the server rules").unwrap();

    let mut config = qexed_config::app::qexed::Qexed::default();
    config.server.code_of_conduct = true;
    let context = ServerContext::new_with_code_of_conduct_dir(config, dir.path())
        .await
        .unwrap();

    let (server_io, client_io) = duplex(32 * 1024 * 1024);
    let (server_reader, server_writer) = tokio::io::split(server_io);
    let (client_reader, client_writer) = tokio::io::split(client_io);
    let mut server_packets = qexed_tcp_connect::PacketStream::new(server_reader);
    let mut server_sink = qexed_tcp_connect::PacketSink::new(server_writer);
    let mut client_packets = qexed_tcp_connect::PacketStream::new(client_reader);
    let mut client_sink = qexed_tcp_connect::PacketSink::new(client_writer);

    let server_task = tokio::spawn(async move {
        handle_configuration(&mut server_packets, &mut server_sink, &context, "127.0.0.1").await
    });

    drive_known_pack_selection(&mut client_packets, &mut client_sink).await;

    let prompt = read_configuration_until_code_of_conduct(&mut client_packets).await;
    assert_eq!(prompt.code_of_conduct, "Follow the server rules");

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
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("en_us.txt"), "Follow the server rules").unwrap();

    let mut config = qexed_config::app::qexed::Qexed::default();
    config.server.code_of_conduct = true;
    let context = ServerContext::new_with_code_of_conduct_dir(config, dir.path())
        .await
        .unwrap();

    let (server_io, client_io) = duplex(32 * 1024 * 1024);
    let (server_reader, server_writer) = tokio::io::split(server_io);
    let (client_reader, client_writer) = tokio::io::split(client_io);
    let mut server_packets = qexed_tcp_connect::PacketStream::new(server_reader);
    let mut server_sink = qexed_tcp_connect::PacketSink::new(server_writer);
    let mut client_packets = qexed_tcp_connect::PacketStream::new(client_reader);
    let mut client_sink = qexed_tcp_connect::PacketSink::new(client_writer);

    let server_task = tokio::spawn(async move {
        handle_configuration(&mut server_packets, &mut server_sink, &context, "127.0.0.1").await
    });

    drive_known_pack_selection(&mut client_packets, &mut client_sink).await;
    let prompt = read_configuration_until_code_of_conduct(&mut client_packets).await;
    assert_eq!(prompt.code_of_conduct, "Follow the server rules");

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

#[tokio::test]
async fn code_of_conduct_uses_client_language_with_english_fallback() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("en_us.txt"), "English rules").unwrap();
    std::fs::write(dir.path().join("zh_cn.txt"), "Chinese rules").unwrap();

    let mut config = qexed_config::app::qexed::Qexed::default();
    config.server.code_of_conduct = true;
    let context = ServerContext::new_with_code_of_conduct_dir(config, dir.path())
        .await
        .unwrap();

    let (server_io, client_io) = duplex(32 * 1024 * 1024);
    let (server_reader, server_writer) = tokio::io::split(server_io);
    let (client_reader, client_writer) = tokio::io::split(client_io);
    let mut server_packets = qexed_tcp_connect::PacketStream::new(server_reader);
    let mut server_sink = qexed_tcp_connect::PacketSink::new(server_writer);
    let mut client_packets = qexed_tcp_connect::PacketStream::new(client_reader);
    let mut client_sink = qexed_tcp_connect::PacketSink::new(client_writer);

    let server_task = tokio::spawn(async move {
        handle_configuration(&mut server_packets, &mut server_sink, &context, "127.0.0.1").await
    });

    drive_known_pack_selection_with_locale(&mut client_packets, &mut client_sink, "zh_CN").await;

    let prompt = read_configuration_until_code_of_conduct(&mut client_packets).await;
    assert_eq!(prompt.code_of_conduct, "Chinese rules");

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

    let client_config = server_task.await.unwrap().unwrap();
    assert_eq!(client_config.locale.as_deref(), Some("zh_CN"));
    assert_eq!(
        client_config.displayed_skin_parts,
        crate::players::DEFAULT_DISPLAYED_SKIN_PARTS
    );
}

#[tokio::test]
async fn enabled_resource_pack_is_sent_and_waits_for_loaded_status() {
    let mut config = qexed_config::app::qexed::Qexed::default();
    config.server.resource_pack.enable = true;
    config.server.resource_pack.id = uuid::Uuid::from_u128(0x00112233_4455_6677_8899_aabbccddeeff);
    config.server.resource_pack.url = "https://example.com/qexed.zip".to_string();
    config.server.resource_pack.hash = "0123456789abcdef0123456789abcdef01234567".to_string();
    config.server.resource_pack.prompt = "Install resources".to_string();
    let resource_pack_id = config.server.resource_pack.id;
    let context = ServerContext::new(config).await.unwrap();

    let (server_io, client_io) = duplex(32 * 1024 * 1024);
    let (server_reader, server_writer) = tokio::io::split(server_io);
    let (client_reader, client_writer) = tokio::io::split(client_io);
    let mut server_packets = qexed_tcp_connect::PacketStream::new(server_reader);
    let mut server_sink = qexed_tcp_connect::PacketSink::new(server_writer);
    let mut client_packets = qexed_tcp_connect::PacketStream::new(client_reader);
    let mut client_sink = qexed_tcp_connect::PacketSink::new(client_writer);

    let server_task = tokio::spawn(async move {
        handle_configuration(&mut server_packets, &mut server_sink, &context, "127.0.0.1").await
    });

    drive_known_pack_selection(&mut client_packets, &mut client_sink).await;

    let resource_pack = read_configuration_until_resource_pack(&mut client_packets).await;
    assert_eq!(resource_pack.id, resource_pack_id);
    assert_eq!(resource_pack.url, "https://example.com/qexed.zip");
    assert_eq!(
        resource_pack.hash,
        "0123456789abcdef0123456789abcdef01234567"
    );
    assert!(!resource_pack.required);
    assert!(resource_pack.prompt.is_some());

    client_sink
        .send(ResourcePackReceive {
            uuid: resource_pack_id,
            result: qexed_packet::net_types::VarInt(0),
        })
        .await
        .unwrap();
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
async fn local_resource_pack_uses_generated_download_url_and_hash() {
    let dir = tempfile::tempdir().unwrap();
    let pack_path = dir.path().join("server.zip");
    tokio::fs::write(&pack_path, b"pack-data").await.unwrap();

    let mut config = qexed_config::app::qexed::Qexed::default();
    config.server.resource_pack.enable = true;
    config.server.resource_pack.source =
        qexed_config::app::qexed::server::ResourcePackSource::Local;
    config.server.resource_pack.id = uuid::Uuid::from_u128(0x00112233_4455_6677_8899_aabbccddeeff);
    config.server.resource_pack.path = pack_path.to_string_lossy().to_string();
    config.server.resource_pack.download_bind = "127.0.0.1:0".to_string();
    let resource_pack_id = config.server.resource_pack.id;
    let context = ServerContext::new(config).await.unwrap();

    let (server_io, client_io) = duplex(32 * 1024 * 1024);
    let (server_reader, server_writer) = tokio::io::split(server_io);
    let (client_reader, client_writer) = tokio::io::split(client_io);
    let mut server_packets = qexed_tcp_connect::PacketStream::new(server_reader);
    let mut server_sink = qexed_tcp_connect::PacketSink::new(server_writer);
    let mut client_packets = qexed_tcp_connect::PacketStream::new(client_reader);
    let mut client_sink = qexed_tcp_connect::PacketSink::new(client_writer);

    let server_task = tokio::spawn(async move {
        handle_configuration(&mut server_packets, &mut server_sink, &context, "127.0.0.1").await
    });

    drive_known_pack_selection(&mut client_packets, &mut client_sink).await;

    let resource_pack = read_configuration_until_resource_pack(&mut client_packets).await;
    assert_eq!(resource_pack.id, resource_pack_id);
    assert!(resource_pack.url.starts_with("http://127.0.0.1:"));
    assert!(
        resource_pack
            .url
            .ends_with(&format!("/resource-pack/{resource_pack_id}.zip"))
    );
    assert_eq!(
        resource_pack.hash,
        "8923b3ae3180a64d86da4d2080c540f501e453c2"
    );

    let http = reqwest::Client::builder().no_proxy().build().unwrap();
    let response = http.get(&resource_pack.url).send().await.unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    assert_eq!(response.bytes().await.unwrap(), "pack-data");

    client_sink
        .send(ResourcePackReceive {
            uuid: resource_pack_id,
            result: qexed_packet::net_types::VarInt(0),
        })
        .await
        .unwrap();
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
async fn required_resource_pack_decline_disconnects_configuration() {
    let mut config = qexed_config::app::qexed::Qexed::default();
    config.server.resource_pack.enable = true;
    config.server.resource_pack.required = true;
    config.server.resource_pack.id = uuid::Uuid::from_u128(0x00112233_4455_6677_8899_aabbccddeeff);
    config.server.resource_pack.url = "https://example.com/qexed.zip".to_string();
    config.server.resource_pack.disconnect_message = "Resource pack required".to_string();
    let resource_pack_id = config.server.resource_pack.id;
    let context = ServerContext::new(config).await.unwrap();

    let (server_io, client_io) = duplex(32 * 1024 * 1024);
    let (server_reader, server_writer) = tokio::io::split(server_io);
    let (client_reader, client_writer) = tokio::io::split(client_io);
    let mut server_packets = qexed_tcp_connect::PacketStream::new(server_reader);
    let mut server_sink = qexed_tcp_connect::PacketSink::new(server_writer);
    let mut client_packets = qexed_tcp_connect::PacketStream::new(client_reader);
    let mut client_sink = qexed_tcp_connect::PacketSink::new(client_writer);

    let server_task = tokio::spawn(async move {
        handle_configuration(&mut server_packets, &mut server_sink, &context, "127.0.0.1").await
    });

    drive_known_pack_selection(&mut client_packets, &mut client_sink).await;
    let resource_pack = read_configuration_until_resource_pack(&mut client_packets).await;
    assert!(resource_pack.required);

    client_sink
        .send(ResourcePackReceive {
            uuid: resource_pack_id,
            result: qexed_packet::net_types::VarInt(1),
        })
        .await
        .unwrap();
    client_sink.flush().await.unwrap();

    let disconnect = read_server_packet_as::<
        qexed_protocol::to_client::configuration::disconnect::Disconnect,
        _,
    >(&mut client_packets)
    .await;
    assert_eq!(
        disconnect.reason,
        super::text_component("Resource pack required")
    );
    assert!(server_task.await.unwrap().is_err());
}

async fn drive_known_pack_selection<R, W>(
    client_packets: &mut qexed_tcp_connect::PacketStream<R>,
    client_sink: &mut qexed_tcp_connect::PacketSink<W>,
) where
    R: tokio::io::AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
{
    drive_known_pack_selection_inner(client_packets, client_sink, None).await;
}

async fn drive_known_pack_selection_with_locale<R, W>(
    client_packets: &mut qexed_tcp_connect::PacketStream<R>,
    client_sink: &mut qexed_tcp_connect::PacketSink<W>,
    locale: &str,
) where
    R: tokio::io::AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
{
    drive_known_pack_selection_inner(client_packets, client_sink, Some(locale)).await;
}

async fn drive_known_pack_selection_inner<R, W>(
    client_packets: &mut qexed_tcp_connect::PacketStream<R>,
    client_sink: &mut qexed_tcp_connect::PacketSink<W>,
    locale: Option<&str>,
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

    let known_packs = read_server_packet_as::<ClientboundSelectKnownPacks, _>(client_packets).await;
    if let Some(locale) = locale {
        client_sink
            .send(ServerboundSettings {
                locale: locale.to_string(),
                view_distance: 10,
                chat_mode: qexed_packet::net_types::VarInt(0),
                chat_colors: true,
                displayed_skin_parts: 0x7f,
                main_hand: qexed_packet::net_types::VarInt(1),
                enable_text_filtering: false,
                allow_server_listings: true,
                particle_status: qexed_packet::net_types::VarInt(0),
            })
            .await
            .unwrap();
    }
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
                panic!("configuration should not finish before code-of-conduct acceptance");
            }
            _ => {}
        }
    }
}

async fn read_configuration_until_resource_pack<R>(
    client_packets: &mut qexed_tcp_connect::PacketStream<R>,
) -> AddResourcePack
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
                assert!(!packet.tags.is_empty());
                saw_tags = true;
            }
            AddResourcePack::ID => {
                assert!(saw_registry_data);
                assert!(saw_tags);
                return decode_payload::<AddResourcePack>(&mut payload).unwrap();
            }
            ClientboundFinishConfiguration::ID => {
                panic!("configuration should not finish before resource-pack response");
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
                panic!("empty code-of-conduct configuration should not send prompt");
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

async fn read_server_packet_as<T, R>(client_packets: &mut qexed_tcp_connect::PacketStream<R>) -> T
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

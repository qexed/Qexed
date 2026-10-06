//! 连接域测试（v4 connection/tests.rs 迁移）。
//!
//! 编解码与资源包 URL 生成为纯逻辑测试，始终运行；配置阶段全链路测试
//! （v4 tests.rs 的 7 项）需要 qexed_mojang_data 的注册表数据（工作目录下
//! cache/mojang/<ver>/ 或 assets/vanilla_json/），与
//! qexed_mojang_data::registry_sync::tests 同一前提，沿用其 #[ignore] 惯例，
//! 前提就绪后 cargo test -- --ignored 运行。

use qexed_packet::{Packet, PacketCodec};
use qexed_protocol::{
    to_client::configuration::{
        code_of_conduct::CodeOfConduct, custom_payload::CustomPayload as ClientboundCustomPayload,
        finish_configuration::FinishConfiguration as ClientboundFinishConfiguration,
        registry_data::RegistryData, resource_pack_push::ResourcePackPush,
        select_known_packs::SelectKnownPacks as ClientboundSelectKnownPacks,
        update_enabled_features::UpdateEnabledFeatures, update_tags::UpdateTags,
    },
    to_server::{
        configuration::{
            accept_code_of_conduct::AcceptCodeOfConduct,
            client_information::ClientInformation as ServerboundClientInformation,
            finish_configuration::FinishConfiguration as ServerboundFinishConfiguration,
            resource_pack::ResourcePack as ServerboundResourcePack,
            select_known_packs::SelectKnownPacks as ServerboundSelectKnownPacks,
        },
        handshaking::client_intention::ClientIntention,
    },
};

use super::{
    SERVER_BRAND,
    codec::{decode_payload, read_packet_id},
    configuration::handle_configuration,
};

#[test]
fn reading_packet_id_leaves_payload_at_packet_body() {
    let packet = ClientIntention {
        protocol_version: qexed_packet::net_types::VarInt(qexed_config::PROTOCOL_VERSION),
        host_name: "127.0.0.1".to_string(),
        port: 25565,
        intention: qexed_packet::net_types::VarInt(1),
    };
    let mut payload = bytes::BytesMut::new();
    let mut writer = qexed_packet::PacketWriter::new(&mut payload);
    qexed_packet::net_types::VarInt(ClientIntention::ID)
        .serialize(&mut writer)
        .unwrap();
    packet.serialize(&mut writer).unwrap();

    assert_eq!(
        read_packet_id(&mut payload).unwrap(),
        ClientIntention::ID
    );
    let decoded = decode_payload::<ClientIntention>(&mut payload).unwrap();

    assert_eq!(decoded.protocol_version.0, qexed_config::PROTOCOL_VERSION);
    assert_eq!(decoded.intention.0, 1);
}

// ---------------------------------------------------------------------------
// 资源包 URL/hash 解析（纯逻辑，无需注册表数据）
// ---------------------------------------------------------------------------

mod resource_pack_cases {
    use super::super::resource_pack::{
        object_storage_download_url, request_path, resolve_offer, sha1_hex,
    };
    use crate::config::{ResourcePack, ResourcePackSource};

    fn config(source: ResourcePackSource) -> ResourcePack {
        ResourcePack {
            enable: true,
            source,
            ..ResourcePack::default()
        }
    }

    #[test]
    fn request_path_accepts_origin_and_absolute_form_targets() {
        let request = "GET /resource-pack/test.zip HTTP/1.1
Host: 127.0.0.1

";
        assert_eq!(request_path(request), Some("/resource-pack/test.zip"));

        let request = "GET http://127.0.0.1:25566/resource-pack/test.zip?cache=1 HTTP/1.1

";
        assert_eq!(request_path(request), Some("/resource-pack/test.zip"));
    }

    #[test]
    fn sha1_hex_matches_reference_vector() {
        assert_eq!(sha1_hex(b"pack-data"), "8923b3ae3180a64d86da4d2080c540f501e453c2");
    }

    #[tokio::test]
    async fn disabled_pack_resolves_to_none() {
        let mut cfg = ResourcePack::default();
        cfg.enable = false;
        assert!(resolve_offer(&cfg, "127.0.0.1").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn url_source_uses_configured_url_and_hash() {
        let mut cfg = config(ResourcePackSource::Url);
        cfg.url = "  https://example.com/qexed.zip ".to_string();
        cfg.hash = " abc123 ".to_string();

        let offer = resolve_offer(&cfg, "127.0.0.1").await.unwrap().unwrap();
        assert_eq!(offer.url, "https://example.com/qexed.zip");
        assert_eq!(offer.hash, "abc123");
    }

    #[tokio::test]
    async fn url_source_without_url_skips_with_none() {
        let cfg = config(ResourcePackSource::Url);
        assert!(resolve_offer(&cfg, "127.0.0.1").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn object_storage_url_prefers_public_base_url() {
        let mut cfg = config(ResourcePackSource::ObjectStorage);
        cfg.object_storage.public_base_url = "https://packs.example.com/cache/".to_string();
        cfg.object_storage.object_key = "/minecraft/server.zip".to_string();

        let offer = resolve_offer(&cfg, "ignored").await.unwrap().unwrap();
        assert_eq!(offer.url, "https://packs.example.com/cache/minecraft/server.zip");
    }

    #[tokio::test]
    async fn object_storage_url_builds_virtual_host_endpoint() {
        let mut cfg = config(ResourcePackSource::ObjectStorage);
        cfg.object_storage.endpoint = "obs.cn-north-4.myhuaweicloud.com".to_string();
        cfg.object_storage.bucket = "qexed-pack".to_string();
        cfg.object_storage.object_key = "resourcepacks/server.zip".to_string();

        let offer = resolve_offer(&cfg, "ignored").await.unwrap().unwrap();
        assert_eq!(
            offer.url,
            "https://qexed-pack.obs.cn-north-4.myhuaweicloud.com/resourcepacks/server.zip"
        );
    }

    #[tokio::test]
    async fn object_storage_url_builds_path_style_endpoint() {
        let mut cfg = config(ResourcePackSource::ObjectStorage);
        cfg.object_storage.endpoint = "https://cos.ap-guangzhou.myqcloud.com".to_string();
        cfg.object_storage.bucket = "qexed-1250000000".to_string();
        cfg.object_storage.object_key = "resourcepacks/server.zip".to_string();
        cfg.object_storage.force_path_style = true;

        let offer = resolve_offer(&cfg, "ignored").await.unwrap().unwrap();
        assert_eq!(
            offer.url,
            "https://cos.ap-guangzhou.myqcloud.com/qexed-1250000000/resourcepacks/server.zip"
        );
    }

    #[tokio::test]
    async fn object_storage_without_key_skips_with_none() {
        let cfg = config(ResourcePackSource::ObjectStorage);
        assert!(resolve_offer(&cfg, "ignored").await.unwrap().is_none());
        assert_eq!(object_storage_download_url(&cfg), None);
    }

    #[tokio::test]
    async fn local_source_missing_file_fails_cleanly() {
        let mut cfg = config(ResourcePackSource::Local);
        cfg.path = "definitely-missing/qexed-pack.zip".to_string();
        assert!(resolve_offer(&cfg, "127.0.0.1").await.is_err());
    }
}

// ---------------------------------------------------------------------------
// 本地资源包 HTTP 下载服务（Local 来源全链路，无需注册表数据）
// ---------------------------------------------------------------------------

mod local_download_service {
    use super::super::resource_pack::{resolve_offer, sha1_hex};
    use crate::config::{ResourcePack, ResourcePackSource};

    #[tokio::test]
    async fn local_source_serves_pack_bytes_over_http() {
        let dir = tempfile::tempdir().unwrap();
        let pack_path = dir.path().join("server.zip");
        tokio::fs::write(&pack_path, b"pack-data").await.unwrap();

        let mut config = ResourcePack {
            enable: true,
            source: ResourcePackSource::Local,
            ..ResourcePack::default()
        };
        config.path = pack_path.to_string_lossy().to_string();
        config.download_bind = "127.0.0.1:0".to_string();

        let offer = resolve_offer(&config, "127.0.0.1").await.unwrap().unwrap();
        assert!(offer.url.starts_with("http://127.0.0.1:"));
        assert!(
            offer
                .url
                .ends_with(&format!("/resource-pack/{}.zip", config.pack_id()))
        );
        assert_eq!(offer.hash, sha1_hex(b"pack-data"));

        let http = reqwest::Client::builder().no_proxy().build().unwrap();
        let response = http.get(&offer.url).send().await.unwrap();
        assert_eq!(response.status(), reqwest::StatusCode::OK);
        assert_eq!(response.bytes().await.unwrap(), "pack-data");
    }

    #[tokio::test]
    async fn local_source_configured_hash_wins_over_computed() {
        let dir = tempfile::tempdir().unwrap();
        let pack_path = dir.path().join("server.zip");
        tokio::fs::write(&pack_path, b"pack-data").await.unwrap();

        let mut config = ResourcePack {
            enable: true,
            source: ResourcePackSource::Local,
            hash: "0123456789abcdef0123456789abcdef01234567".to_string(),
            ..ResourcePack::default()
        };
        config.path = pack_path.to_string_lossy().to_string();
        config.download_bind = "127.0.0.1:0".to_string();

        let offer = resolve_offer(&config, "127.0.0.1").await.unwrap().unwrap();
        assert_eq!(offer.hash, "0123456789abcdef0123456789abcdef01234567");
    }

    #[tokio::test]
    async fn local_source_download_host_base_url_overrides_login_host() {
        let dir = tempfile::tempdir().unwrap();
        let pack_path = dir.path().join("server.zip");
        tokio::fs::write(&pack_path, b"pack-data").await.unwrap();

        let mut config = ResourcePack {
            enable: true,
            source: ResourcePackSource::Local,
            download_host: "https://cdn.example.org/packs".to_string(),
            ..ResourcePack::default()
        };
        config.path = pack_path.to_string_lossy().to_string();
        config.download_bind = "127.0.0.1:0".to_string();

        let offer = resolve_offer(&config, "ignored").await.unwrap().unwrap();
        assert_eq!(
            offer.url,
            format!(
                "https://cdn.example.org/packs/resource-pack/{}.zip",
                config.pack_id()
            )
        );
    }
}

// ---------------------------------------------------------------------------
// 配置阶段全链路（v4 tests.rs 7 项恢复；前提：注册表数据就绪，见模块文档）
// ---------------------------------------------------------------------------

mod configuration_flow {
    use super::*;

    fn context(config: super::super::context::ServerContext) -> super::super::context::ServerContext {
        config
    }

    #[ignore = "需要注册表数据（工作目录 cache/mojang/<ver>/ 或 assets/vanilla_json/）；前提就绪后 --ignored 运行"]
    #[tokio::test]
    async fn empty_code_of_conduct_skips_prompt_but_finishes_configuration() {
        let context = context(test_context(false, None).await);
        let (server_io, client_io) = tokio::io::duplex(32 * 1024 * 1024);
        let (server_reader, server_writer) = tokio::io::split(server_io);
        let (client_reader, client_writer) = tokio::io::split(client_io);
        let mut packets = crate::transport::PacketStream::new(server_reader);
        let mut sink = crate::transport::PacketSink::new(server_writer);
        let mut client_packets = crate::transport::PacketStream::new(client_reader);
        let mut client_sink = crate::transport::PacketSink::new(client_writer);

        let server_task = tokio::spawn(async move {
            handle_configuration(&mut packets, &mut sink, &context, "127.0.0.1").await
        });

        drive_known_pack_selection(&mut client_packets, &mut client_sink, None).await;
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
            super::super::configuration::DEFAULT_DISPLAYED_SKIN_PARTS
        );
    }

    #[ignore = "需要注册表数据（工作目录 cache/mojang/<ver>/ 或 assets/vanilla_json/）；前提就绪后 --ignored 运行"]
    #[tokio::test]
    async fn non_empty_code_of_conduct_sends_prompt_and_waits_for_accept() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("en_us.txt"), "Follow the server rules").unwrap();
        let context = test_context_with_coc_dir(true, dir.path()).await;

        let (server_io, client_io) = tokio::io::duplex(32 * 1024 * 1024);
        let (server_reader, server_writer) = tokio::io::split(server_io);
        let (client_reader, client_writer) = tokio::io::split(client_io);
        let mut server_packets = crate::transport::PacketStream::new(server_reader);
        let mut server_sink = crate::transport::PacketSink::new(server_writer);
        let mut client_packets = crate::transport::PacketStream::new(client_reader);
        let mut client_sink = crate::transport::PacketSink::new(client_writer);

        let server_task = tokio::spawn(async move {
            handle_configuration(&mut server_packets, &mut server_sink, &context, "127.0.0.1").await
        });

        drive_known_pack_selection(&mut client_packets, &mut client_sink, None).await;

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

    #[ignore = "需要注册表数据（工作目录 cache/mojang/<ver>/ 或 assets/vanilla_json/）；前提就绪后 --ignored 运行"]
    #[tokio::test]
    async fn code_of_conduct_wait_skips_prior_configuration_packets() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("en_us.txt"), "Follow the server rules").unwrap();
        let context = test_context_with_coc_dir(true, dir.path()).await;

        let (server_io, client_io) = tokio::io::duplex(32 * 1024 * 1024);
        let (server_reader, server_writer) = tokio::io::split(server_io);
        let (client_reader, client_writer) = tokio::io::split(client_io);
        let mut server_packets = crate::transport::PacketStream::new(server_reader);
        let mut server_sink = crate::transport::PacketSink::new(server_writer);
        let mut client_packets = crate::transport::PacketStream::new(client_reader);
        let mut client_sink = crate::transport::PacketSink::new(client_writer);

        let server_task = tokio::spawn(async move {
            handle_configuration(&mut server_packets, &mut server_sink, &context, "127.0.0.1").await
        });

        drive_known_pack_selection(&mut client_packets, &mut client_sink, None).await;
        let prompt = read_configuration_until_code_of_conduct(&mut client_packets).await;
        assert_eq!(prompt.code_of_conduct, "Follow the server rules");

        // 客户端先发一个无关配置包再接受守则：服务端等待循环必须跳过前者。
        let custom_payload = crate::transport::PacketSink::<
            tokio::io::WriteHalf<tokio::io::DuplexStream>,
        >::build_send_packet(ClientboundCustomPayload {
            channel: "minecraft:brand".to_string(),
            data: qexed_packet::net_types::RestBuffer(Vec::new()),
        })
        .unwrap();
        let accept_payload = crate::transport::PacketSink::<
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

    #[ignore = "需要注册表数据（工作目录 cache/mojang/<ver>/ 或 assets/vanilla_json/）；前提就绪后 --ignored 运行"]
    #[tokio::test]
    async fn code_of_conduct_uses_client_language_with_english_fallback() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("en_us.txt"), "English rules").unwrap();
        std::fs::write(dir.path().join("zh_cn.txt"), "Chinese rules").unwrap();
        let context = test_context_with_coc_dir(true, dir.path()).await;

        let (server_io, client_io) = tokio::io::duplex(32 * 1024 * 1024);
        let (server_reader, server_writer) = tokio::io::split(server_io);
        let (client_reader, client_writer) = tokio::io::split(client_io);
        let mut server_packets = crate::transport::PacketStream::new(server_reader);
        let mut server_sink = crate::transport::PacketSink::new(server_writer);
        let mut client_packets = crate::transport::PacketStream::new(client_reader);
        let mut client_sink = crate::transport::PacketSink::new(client_writer);

        let server_task = tokio::spawn(async move {
            handle_configuration(&mut server_packets, &mut server_sink, &context, "127.0.0.1").await
        });

        drive_known_pack_selection(&mut client_packets, &mut client_sink, Some("zh_CN")).await;

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
    }

    #[ignore = "需要注册表数据（工作目录 cache/mojang/<ver>/ 或 assets/vanilla_json/）；前提就绪后 --ignored 运行"]
    #[tokio::test]
    async fn enabled_resource_pack_is_sent_and_waits_for_loaded_status() {
        let mut config = crate::config::ConnectionConfig::default();
        config.resource_pack.enable = true;
        config.resource_pack.id =
            "00112233-4455-6677-8899-aabbccddeeff".to_string();
        config.resource_pack.url = "https://example.com/qexed.zip".to_string();
        config.resource_pack.hash = "0123456789abcdef0123456789abcdef01234567".to_string();
        config.resource_pack.prompt = "Install resources".to_string();
        let resource_pack_id = config.resource_pack.pack_id();
        let context = super::super::context::ServerContext::new(config).await.unwrap();

        let (server_io, client_io) = tokio::io::duplex(32 * 1024 * 1024);
        let (server_reader, server_writer) = tokio::io::split(server_io);
        let (client_reader, client_writer) = tokio::io::split(client_io);
        let mut server_packets = crate::transport::PacketStream::new(server_reader);
        let mut server_sink = crate::transport::PacketSink::new(server_writer);
        let mut client_packets = crate::transport::PacketStream::new(client_reader);
        let mut client_sink = crate::transport::PacketSink::new(client_writer);

        let server_task = tokio::spawn(async move {
            handle_configuration(&mut server_packets, &mut server_sink, &context, "127.0.0.1").await
        });

        drive_known_pack_selection(&mut client_packets, &mut client_sink, None).await;

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
            .send(ServerboundResourcePack {
                id: resource_pack_id,
                action: qexed_packet::net_types::VarInt(0),
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

    #[ignore = "需要注册表数据（工作目录 cache/mojang/<ver>/ 或 assets/vanilla_json/）；前提就绪后 --ignored 运行"]
    #[tokio::test]
    async fn local_resource_pack_uses_generated_download_url_and_hash() {
        let dir = tempfile::tempdir().unwrap();
        let pack_path = dir.path().join("server.zip");
        tokio::fs::write(&pack_path, b"pack-data").await.unwrap();

        let mut config = crate::config::ConnectionConfig::default();
        config.resource_pack.enable = true;
        config.resource_pack.source = crate::config::ResourcePackSource::Local;
        config.resource_pack.id =
            "00112233-4455-6677-8899-aabbccddeeff".to_string();
        config.resource_pack.path = pack_path.to_string_lossy().to_string();
        config.resource_pack.download_bind = "127.0.0.1:0".to_string();
        let resource_pack_id = config.resource_pack.pack_id();
        let context = super::super::context::ServerContext::new(config).await.unwrap();

        let (server_io, client_io) = tokio::io::duplex(32 * 1024 * 1024);
        let (server_reader, server_writer) = tokio::io::split(server_io);
        let (client_reader, client_writer) = tokio::io::split(client_io);
        let mut server_packets = crate::transport::PacketStream::new(server_reader);
        let mut server_sink = crate::transport::PacketSink::new(server_writer);
        let mut client_packets = crate::transport::PacketStream::new(client_reader);
        let mut client_sink = crate::transport::PacketSink::new(client_writer);

        let server_task = tokio::spawn(async move {
            handle_configuration(&mut server_packets, &mut server_sink, &context, "127.0.0.1").await
        });

        drive_known_pack_selection(&mut client_packets, &mut client_sink, None).await;

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
            .send(ServerboundResourcePack {
                id: resource_pack_id,
                action: qexed_packet::net_types::VarInt(0),
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

    #[ignore = "需要注册表数据（工作目录 cache/mojang/<ver>/ 或 assets/vanilla_json/）；前提就绪后 --ignored 运行"]
    #[tokio::test]
    async fn required_resource_pack_decline_disconnects_configuration() {
        let mut config = crate::config::ConnectionConfig::default();
        config.resource_pack.enable = true;
        config.resource_pack.required = true;
        config.resource_pack.id =
            "00112233-4455-6677-8899-aabbccddeeff".to_string();
        config.resource_pack.url = "https://example.com/qexed.zip".to_string();
        config.resource_pack.disconnect_message = "Resource pack required".to_string();
        let resource_pack_id = config.resource_pack.pack_id();
        let context = super::super::context::ServerContext::new(config).await.unwrap();

        let (server_io, client_io) = tokio::io::duplex(32 * 1024 * 1024);
        let (server_reader, server_writer) = tokio::io::split(server_io);
        let (client_reader, client_writer) = tokio::io::split(client_io);
        let mut server_packets = crate::transport::PacketStream::new(server_reader);
        let mut server_sink = crate::transport::PacketSink::new(server_writer);
        let mut client_packets = crate::transport::PacketStream::new(client_reader);
        let mut client_sink = crate::transport::PacketSink::new(client_writer);

        let server_task = tokio::spawn(async move {
            handle_configuration(&mut server_packets, &mut server_sink, &context, "127.0.0.1").await
        });

        drive_known_pack_selection(&mut client_packets, &mut client_sink, None).await;
        let resource_pack = read_configuration_until_resource_pack(&mut client_packets).await;
        assert!(resource_pack.required);

        client_sink
            .send(ServerboundResourcePack {
                id: resource_pack_id,
                action: qexed_packet::net_types::VarInt(1),
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
            super::super::text_component("Resource pack required")
        );
        assert!(server_task.await.unwrap().is_err());
    }

    // ---- 辅助 ----

    async fn test_context(
        code_of_conduct: bool,
        _resource_pack: Option<crate::config::ResourcePack>,
    ) -> super::super::context::ServerContext {
        let mut config = crate::config::ConnectionConfig::default();
        config.code_of_conduct = code_of_conduct;
        super::super::context::ServerContext::new(config).await.unwrap()
    }

    async fn test_context_with_coc_dir(
        code_of_conduct: bool,
        dir: impl AsRef<std::path::Path>,
    ) -> super::super::context::ServerContext {
        let mut config = crate::config::ConnectionConfig::default();
        config.code_of_conduct = code_of_conduct;
        super::super::context::ServerContext::new_with_code_of_conduct_dir(config, dir)
            .await
            .unwrap()
    }

    async fn drive_known_pack_selection<R, W>(
        client_packets: &mut crate::transport::PacketStream<R>,
        client_sink: &mut crate::transport::PacketSink<W>,
        locale: Option<&str>,
    ) where
        R: tokio::io::AsyncRead + Unpin,
        W: tokio::io::AsyncWrite + Unpin,
    {
        let brand = read_server_packet_as::<ClientboundCustomPayload, _>(client_packets).await;
        assert_eq!(brand.channel, "minecraft:brand");
        assert_eq!(decode_string_payload(&brand.data.0), SERVER_BRAND);

        let features =
            read_server_packet_as::<UpdateEnabledFeatures, _>(client_packets).await;
        assert_eq!(
            features.features,
            vec![qexed_mojang_data::registry_sync::VANILLA_FEATURE.to_string()]
        );

        let known_packs =
            read_server_packet_as::<ClientboundSelectKnownPacks, _>(client_packets).await;
        if let Some(locale) = locale {
            client_sink
                .send(ServerboundClientInformation {
                    language: locale.to_string(),
                    view_distance: 10,
                    chat_visibility: qexed_packet::net_types::VarInt(0),
                    chat_colors: true,
                    model_customisation: 0x7f,
                    main_hand: qexed_packet::net_types::VarInt(1),
                    text_filtering_enabled: false,
                    allows_listing: true,
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
        client_packets: &mut crate::transport::PacketStream<R>,
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
                UpdateTags::ID => {
                    let packet = decode_payload::<UpdateTags>(&mut payload).unwrap();
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
        client_packets: &mut crate::transport::PacketStream<R>,
    ) -> ResourcePackPush
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
                UpdateTags::ID => {
                    let packet = decode_payload::<UpdateTags>(&mut payload).unwrap();
                    assert!(!packet.tags.is_empty());
                    saw_tags = true;
                }
                ResourcePackPush::ID => {
                    assert!(saw_registry_data);
                    assert!(saw_tags);
                    return decode_payload::<ResourcePackPush>(&mut payload).unwrap();
                }
                ClientboundFinishConfiguration::ID => {
                    panic!("configuration should not finish before resource-pack response");
                }
                _ => {}
            }
        }
    }

    async fn read_configuration_until_finish<R>(
        client_packets: &mut crate::transport::PacketStream<R>,
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
                UpdateTags::ID => {
                    let packet = decode_payload::<UpdateTags>(&mut payload).unwrap();
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

    async fn read_server_packet_as<T, R>(
        client_packets: &mut crate::transport::PacketStream<R>,
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
        client_packets: &mut crate::transport::PacketStream<R>,
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

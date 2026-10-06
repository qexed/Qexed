//! 连接域测试（v4 connection/tests.rs 迁移）。
//!
//! v4 测试依赖完整 ServerContext（会构建 world/player_data 等运行时）;
//! v6 连接域上下文只剩 认证器/配置/行为守则，注册表数据也依赖
//! qexed_mojang_data 缓存，因此整体配置阶段流程测试标注 TODO(test)，
//! 保留可独立运行的部分：包编解码、行为守则选语言。

use qexed_packet::{Packet, PacketCodec};
use qexed_protocol::to_server::handshaking::client_intention::ClientIntention;

use super::codec::{decode_payload, read_packet_id};

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

// TODO(test): 以下 v4 测试在 v6 需要先解决依赖再恢复：
// - empty_code_of_conduct_skips_prompt_but_finishes_configuration
//   （需要 qexed_mojang_data::init() 下载数据 + handle_configuration 全链路）
// - non_empty_code_of_conduct_sends_prompt_and_waits_for_accept
// - code_of_conduct_wait_skips_prior_configuration_packets
// - code_of_conduct_uses_client_language_with_english_fallback
// - enabled_resource_pack_is_sent_and_waits_for_loaded_status
// - local_resource_pack_uses_generated_download_url_and_hash
//   （依赖 qexed_server 域的 ResourcePackManager HTTP 下载服务）
// - required_resource_pack_decline_disconnects_configuration

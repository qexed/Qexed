use qexed_packet::Packet;
use qexed_packet::PacketCodec;
use qexed_protocol::protocol_26_1_2::{
    PacketDirection, ProtocolState, packet_by_id, packet_by_name,
};

#[test]
fn registry_has_26_1_2_play_ids() {
    let login = packet_by_name(
        ProtocolState::Play,
        PacketDirection::Clientbound,
        "clientbound_login",
    )
    .unwrap();
    assert_eq!(login.id, 0x31);

    let keep_alive = packet_by_name(
        ProtocolState::Play,
        PacketDirection::Serverbound,
        "serverbound_keep_alive",
    )
    .unwrap();
    assert_eq!(keep_alive.id, 0x1C);

    let packet = packet_by_id(ProtocolState::Play, PacketDirection::Clientbound, 0x8C).unwrap();
    assert_eq!(packet.name, "clientbound_show_dialog");
}

#[test]
fn existing_packet_types_use_26_1_2_ids() {
    assert_eq!(qexed_protocol::to_client::play::login::Login::ID, 0x31);
    assert_eq!(
        qexed_protocol::to_client::play::keep_alive::KeepAlive::ID,
        0x2C
    );
    assert_eq!(
        qexed_protocol::to_server::play::keep_alive::KeepAlive::ID,
        0x1C
    );
    assert_eq!(qexed_protocol::to_server::play::pong::Pong::ID, 0x2D);
}

#[test]
fn raw_packet_can_represent_unmodeled_packet_payloads() {
    type ShowDialogRaw = qexed_protocol::RawPacket<0x8C>;
    assert_eq!(ShowDialogRaw::ID, 0x8C);
}

#[test]
fn status_response_parses_json_payload() {
    let mut buf = bytes::BytesMut::new();
    let mut writer = qexed_packet::PacketWriter::new(&mut buf);
    r#"{"version":{"name":"26.1.2","protocol":0},"players":{"max":20,"online":0}}"#
        .to_string()
        .serialize(&mut writer)
        .unwrap();

    let mut bytes = buf.freeze();
    let mut reader = qexed_packet::PacketReader::new(&mut bytes);
    let mut packet = qexed_protocol::to_client::status::server_info::ServerInfo::default();
    packet.deserialize(&mut reader).unwrap();

    assert_eq!(packet.response.0["players"]["max"], 20);
}

#[test]
fn login_hello_uses_varint_prefixed_byte_arrays() {
    let packet = qexed_protocol::to_client::login::encryption_begin::EncryptionBegin {
        server_id: String::new(),
        public_key: vec![1, 2, 3].into(),
        verify_token: vec![4, 5].into(),
        should_authenticate: true,
    };

    let mut buf = bytes::BytesMut::new();
    let mut writer = qexed_packet::PacketWriter::new(&mut buf);
    packet.serialize(&mut writer).unwrap();

    assert_eq!(&buf[..], &[0, 3, 1, 2, 3, 2, 4, 5, 1]);
}

#[test]
fn login_finished_uses_game_profile_layout() {
    let uuid = uuid::Uuid::from_u128(0x00112233445566778899aabbccddeeff);
    let packet = qexed_protocol::to_client::login::success::Success {
        game_profile: qexed_packet::net_types::GameProfile {
            uuid,
            username: "Steve".to_string(),
            properties: vec![qexed_packet::net_types::ProfileProperty {
                name: "textures".to_string(),
                value: "value".to_string(),
                signature: Some("signature".to_string()),
            }],
        },
    };

    let mut buf = bytes::BytesMut::new();
    let mut writer = qexed_packet::PacketWriter::new(&mut buf);
    packet.serialize(&mut writer).unwrap();

    let mut bytes = buf.freeze();
    let mut reader = qexed_packet::PacketReader::new(&mut bytes);
    let mut decoded = qexed_protocol::to_client::login::success::Success::default();
    decoded.deserialize(&mut reader).unwrap();

    assert_eq!(decoded.game_profile.uuid, uuid);
    assert_eq!(decoded.game_profile.username, "Steve");
    assert_eq!(
        decoded.game_profile.properties[0].signature.as_deref(),
        Some("signature")
    );
}

#[test]
fn configuration_code_of_conduct_packets_use_26_1_2_ids() {
    assert_eq!(
        qexed_protocol::to_client::configuration::code_of_conduct::CodeOfConduct::ID,
        0x13
    );
    assert_eq!(
        qexed_protocol::to_server::configuration::accept_code_of_conduct::AcceptCodeOfConduct::ID,
        0x09
    );
}

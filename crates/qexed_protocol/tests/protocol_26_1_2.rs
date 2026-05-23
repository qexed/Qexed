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
        qexed_protocol::to_client::play::initialize_border::InitializeBorder::ID,
        0x2B
    );
    assert_eq!(
        qexed_protocol::to_client::play::keep_alive::KeepAlive::ID,
        0x2C
    );
    assert_eq!(
        qexed_protocol::to_client::play::light_update::LightUpdate::ID,
        0x30
    );
    assert_eq!(
        qexed_protocol::to_client::play::player_info_update::PlayerInfoUpdate::ID,
        0x46
    );
    assert_eq!(
        qexed_protocol::to_client::play::forget_level_chunk::ForgetLevelChunk::ID,
        0x25
    );
    assert_eq!(
        qexed_protocol::to_client::play::add_entity::AddEntity::ID,
        0x01
    );
    assert_eq!(
        qexed_protocol::to_client::play::add_entity::PlayerInfoRemove::ID,
        0x45
    );
    assert_eq!(
        qexed_protocol::to_client::play::add_entity::RemoveEntities::ID,
        0x4D
    );
    assert_eq!(
        qexed_protocol::to_client::play::add_entity::RotateHead::ID,
        0x53
    );
    assert_eq!(
        qexed_protocol::to_client::play::add_entity::TeleportEntity::ID,
        0x7D
    );
    assert_eq!(
        qexed_protocol::to_client::play::add_entity::EntityPositionSync::ID,
        0x23
    );
    assert_eq!(
        qexed_protocol::to_client::play::block_changed_ack::BlockChangedAck::ID,
        0x04
    );
    assert_eq!(
        qexed_protocol::to_client::play::block_update::BlockUpdate::ID,
        0x08
    );
    assert_eq!(
        qexed_protocol::to_client::play::commands::Commands::ID,
        0x10
    );
    assert_eq!(
        qexed_protocol::to_client::play::set_equipment::SetEquipment::ID,
        0x66
    );
    assert_eq!(
        qexed_protocol::to_client::play::set_player_inventory::SetPlayerInventory::ID,
        0x6C
    );
    assert_eq!(
        qexed_protocol::to_client::play::set_entity_data::SetEntityData::ID,
        0x63
    );
    assert_eq!(
        qexed_protocol::to_client::play::player_chat::PlayerChat::ID,
        0x41
    );
    assert_eq!(qexed_protocol::to_server::play::chat_ack::ChatAck::ID, 0x06);
    assert_eq!(
        qexed_protocol::to_client::play::server_data::ServerData::ID,
        0x56
    );
    assert_eq!(
        qexed_protocol::to_client::play::set_experience::SetExperience::ID,
        0x67
    );
    assert_eq!(
        qexed_protocol::to_client::play::set_health::SetHealth::ID,
        0x68
    );
    assert_eq!(
        qexed_protocol::to_client::play::set_held_slot::SetHeldSlot::ID,
        0x69
    );
    assert_eq!(
        qexed_protocol::to_client::play::set_simulation_distance::SetSimulationDistance::ID,
        0x6F
    );
    assert_eq!(qexed_protocol::to_client::play::set_time::SetTime::ID, 0x71);
    assert_eq!(
        qexed_protocol::to_client::play::ticking_state::TickingState::ID,
        0x7F
    );
    assert_eq!(
        qexed_protocol::to_server::play::keep_alive::KeepAlive::ID,
        0x1C
    );
    assert_eq!(
        qexed_protocol::to_server::play::move_player_pos::MovePlayerPos::ID,
        0x1E
    );
    assert_eq!(
        qexed_protocol::to_server::play::move_player_pos_rot::MovePlayerPosRot::ID,
        0x1F
    );
    assert_eq!(
        qexed_protocol::to_server::play::move_player_rot::MovePlayerRot::ID,
        0x20
    );
    assert_eq!(
        qexed_protocol::to_server::play::move_player_status_only::MovePlayerStatusOnly::ID,
        0x21
    );
    assert_eq!(
        qexed_protocol::to_server::play::pick_item_from_block::PickItemFromBlock::ID,
        0x24
    );
    assert_eq!(
        qexed_protocol::to_server::play::set_carried_item::SetCarriedItem::ID,
        0x35
    );
    assert_eq!(
        qexed_protocol::to_server::play::set_creative_mode_slot::SetCreativeModeSlot::ID,
        0x38
    );
    assert_eq!(
        qexed_protocol::to_server::play::use_item_on::UseItemOn::ID,
        0x42
    );
    assert_eq!(qexed_protocol::to_server::play::use_item::UseItem::ID, 0x43);
    assert_eq!(
        qexed_protocol::to_server::play::chat_session_update::ChatSessionUpdate::ID,
        0x0A
    );
    assert_eq!(qexed_protocol::to_server::play::pong::Pong::ID, 0x2D);
}

#[test]
fn player_info_update_initializing_uses_fixed_enum_bitset() {
    let packet = qexed_protocol::to_client::play::player_info_update::PlayerInfoUpdate {
        actions: qexed_protocol::to_client::play::player_info_update::PlayerInfoActions::player_initializing(),
        entries: vec![
            qexed_protocol::to_client::play::player_info_update::PlayerInfoEntry::from_profile(
                &qexed_packet::net_types::GameProfile {
                    uuid: uuid::Uuid::from_u128(0x00112233445566778899aabbccddeeff),
                    username: "Steve".to_string(),
                    properties: Vec::new(),
                },
                1,
            ),
        ],
    };

    let mut buf = bytes::BytesMut::new();
    let mut writer = qexed_packet::PacketWriter::new(&mut buf);
    packet.serialize(&mut writer).unwrap();

    assert_eq!(buf[0], 0xff);
    assert_eq!(buf[1], 1);
}

#[test]
fn player_chat_uses_nullable_components() {
    let packet = qexed_protocol::to_client::play::player_chat::PlayerChat::pass_through(
        0,
        uuid::Uuid::from_u128(0x00112233445566778899aabbccddeeff),
        0,
        qexed_protocol::types::MessageSignature(vec![7; 256]),
        Vec::new(),
        "hello",
        123,
        456,
        text_component("Steve"),
    );

    let mut buf = bytes::BytesMut::new();
    let mut writer = qexed_packet::PacketWriter::new(&mut buf);
    packet.serialize(&mut writer).unwrap();

    let mut bytes = buf.freeze();
    let mut reader = qexed_packet::PacketReader::new(&mut bytes);
    let mut decoded = qexed_protocol::to_client::play::player_chat::PlayerChat::default();
    decoded.deserialize(&mut reader).unwrap();

    assert_eq!(decoded.unsigned_chat_content, None);
    assert_eq!(decoded.network_target_name, None);
    assert_eq!(decoded.plain_message, "hello");
    assert_eq!(decoded.signature.unwrap().0, vec![7; 256]);
}

fn text_component(text: &str) -> qexed_protocol::types::TextComponent {
    let mut map = std::collections::HashMap::new();
    map.insert(
        "text".to_string(),
        qexed_nbt::Tag::String(std::sync::Arc::from(text.to_string())),
    );
    qexed_nbt::Tag::Compound(std::sync::Arc::new(map))
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

#[test]
fn configuration_resource_pack_packets_use_26_1_2_ids() {
    assert_eq!(
        qexed_protocol::to_client::configuration::add_resource_pack::AddResourcePack::ID,
        0x09
    );
    assert_eq!(
        qexed_protocol::to_client::configuration::remove_resource_pack::RemoveResourcePack::ID,
        0x08
    );
    assert_eq!(
        qexed_protocol::to_server::configuration::resource_pack_receive::ResourcePackReceive::ID,
        0x06
    );
}

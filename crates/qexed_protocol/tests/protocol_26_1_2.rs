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
        qexed_protocol::to_client::play::reset_score::ResetScore::ID,
        0x4F
    );
    assert_eq!(qexed_protocol::to_client::play::respawn::Respawn::ID, 0x52);
    assert_eq!(
        qexed_protocol::to_client::play::set_display_objective::SetDisplayObjective::ID,
        0x62
    );
    assert_eq!(
        qexed_protocol::to_client::play::set_objective::SetObjective::ID,
        0x6A
    );
    assert_eq!(
        qexed_protocol::to_client::play::set_score::SetScore::ID,
        0x6E
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
        qexed_protocol::to_client::play::update_recipes::UpdateRecipes::ID,
        0x85
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
    assert_eq!(
        qexed_protocol::to_server::play::client_command::ClientCommand::ID,
        0x0C
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

#[test]
fn scoreboard_packets_round_trip_minimal_sidebar_state() {
    let objective = qexed_protocol::to_client::play::set_objective::SetObjective::create(
        "qexed",
        text_component("Qexed"),
    );
    let decoded_objective = round_trip(objective);
    assert_eq!(decoded_objective.objective_name, "qexed");
    assert_eq!(
        decoded_objective.method,
        qexed_protocol::to_client::play::set_objective::METHOD_ADD
    );
    assert_eq!(
        decoded_objective.render_type.0,
        qexed_protocol::to_client::play::set_objective::RENDER_TYPE_INTEGER
    );
    assert_eq!(decoded_objective.number_format, None);

    let display = round_trip(
        qexed_protocol::to_client::play::set_display_objective::SetDisplayObjective::sidebar(
            "qexed",
        ),
    );
    assert_eq!(
        display.slot.0,
        qexed_protocol::to_client::play::set_display_objective::DISPLAY_SLOT_SIDEBAR
    );
    assert_eq!(display.objective_name, "qexed");

    let score = round_trip(qexed_protocol::to_client::play::set_score::SetScore::new(
        "qexed_line_0",
        "qexed",
        15,
        Some(text_component("line")),
    ));
    assert_eq!(score.owner, "qexed_line_0");
    assert_eq!(score.objective_name, "qexed");
    assert_eq!(score.score.0, 15);
    assert_eq!(
        score.number_format,
        Some(qexed_protocol::types::NumberFormat::Blank)
    );

    let reset = round_trip(qexed_protocol::to_client::play::reset_score::ResetScore {
        owner: "qexed_line_0".to_string(),
        objective_name: Some("qexed".to_string()),
    });
    assert_eq!(reset.owner, "qexed_line_0");
    assert_eq!(reset.objective_name.as_deref(), Some("qexed"));
}

#[test]
fn command_integer_argument_without_properties_writes_default_flags() {
    let commands = qexed_protocol::to_client::play::commands::Commands {
        nodes: vec![
            qexed_protocol::to_client::play::commands::Node {
                flags: 0,
                children: vec![qexed_packet::net_types::VarInt(1)],
                ..Default::default()
            },
            qexed_protocol::to_client::play::commands::Node {
                flags: 1,
                children: vec![qexed_packet::net_types::VarInt(2)],
                name: Some("test".to_string()),
                ..Default::default()
            },
            qexed_protocol::to_client::play::commands::Node {
                flags: 2,
                name: Some("amount".to_string()),
                parser_id: Some(qexed_packet::net_types::VarInt(3)),
                properties: None,
                ..Default::default()
            },
        ],
        root_index: qexed_packet::net_types::VarInt(0),
    };

    let decoded = round_trip(commands);
    assert_eq!(decoded.root_index.0, 0);
    assert_eq!(decoded.nodes[2].parser_id.as_ref().map(|id| id.0), Some(3));
    assert_eq!(
        decoded.nodes[2].properties,
        Some(
            qexed_protocol::to_client::play::commands::Varies::BrigadierInteger(
                qexed_protocol::to_client::play::commands::Brigadier {
                    flags: 0,
                    min: None,
                    max: None,
                },
            ),
        )
    );
}

#[test]
fn respawn_and_client_command_packets_round_trip() {
    let respawn = round_trip(qexed_protocol::to_client::play::respawn::Respawn {
        dimension_type: qexed_packet::net_types::VarInt(1),
        dimension_name: "minecraft:overworld".to_string(),
        game_mode: 0,
        previous_game_mode: -1,
        data_to_keep: qexed_protocol::to_client::play::respawn::KEEP_ALL_DATA,
        ..qexed_protocol::to_client::play::respawn::Respawn::default()
    });
    assert_eq!(respawn.dimension_name, "minecraft:overworld");
    assert_eq!(
        respawn.data_to_keep,
        qexed_protocol::to_client::play::respawn::KEEP_ALL_DATA
    );

    let command = round_trip(
        qexed_protocol::to_server::play::client_command::ClientCommand::perform_respawn(),
    );
    assert_eq!(
        command.action.0,
        qexed_protocol::to_server::play::client_command::PERFORM_RESPAWN
    );
}

#[test]
fn recipe_book_packets_round_trip_with_26_1_2_slot_display_ids() {
    let item_display = qexed_protocol::types::SlotDisplay::Item(
        qexed_protocol::types::slot_display_types::minecraft::Item {
            item_type: qexed_packet::net_types::VarInt(36),
        },
    );
    let mut item_display_buf = bytes::BytesMut::new();
    item_display
        .serialize(&mut qexed_packet::PacketWriter::new(&mut item_display_buf))
        .unwrap();
    assert_eq!(item_display_buf[0], 4);

    let recipe = round_trip(
        qexed_protocol::to_client::play::recipe_book_add::RecipeBookAdd {
            entries: vec![qexed_protocol::to_client::play::recipe_book_add::Recipes {
                recipe: qexed_packet::net_types::VarInt(0),
                display: qexed_protocol::types::RecipeDisplay::MinecraftCraftingShapeless(
                    qexed_protocol::types::minecraft::CraftingShapeless {
                        ingredients: vec![item_display.clone()],
                        result: qexed_protocol::types::SlotDisplay::ItemStack(
                            qexed_protocol::types::slot_display_types::minecraft::ItemStack {
                                item_stack: qexed_protocol::types::Slot {
                                    item_count: qexed_packet::net_types::VarInt(1),
                                    item_id: Some(qexed_packet::net_types::VarInt(36)),
                                    number_of_components_to_add: Some(
                                        qexed_packet::net_types::VarInt(0),
                                    ),
                                    number_of_components_to_remove: Some(
                                        qexed_packet::net_types::VarInt(0),
                                    ),
                                    components_to_add: None,
                                    components_to_remove: None,
                                },
                            },
                        ),
                        crafting_station: qexed_protocol::types::SlotDisplay::Empty,
                    },
                ),
                group: qexed_packet::net_types::VarInt(0),
                category: qexed_packet::net_types::VarInt(0),
                ingredients: Some(vec![qexed_protocol::types::IDSet {
                    r#type: qexed_packet::net_types::VarInt(2),
                    tag_name: None,
                    ids: Some(vec![qexed_packet::net_types::VarInt(36)]),
                }]),
                flags: 0,
            }],
            replace: true,
        },
    );

    assert!(recipe.replace);
    assert_eq!(recipe.entries.len(), 1);

    let update_recipes = round_trip(
        qexed_protocol::to_client::play::update_recipes::UpdateRecipes {
            item_sets: vec![
                qexed_protocol::to_client::play::update_recipes::RecipePropertySetEntry {
                    key: "minecraft:furnace_input".to_string(),
                    items: Vec::new(),
                },
            ],
            stonecutter_recipes: Vec::new(),
        },
    );
    assert_eq!(update_recipes.item_sets[0].key, "minecraft:furnace_input");
}

fn round_trip<T>(packet: T) -> T
where
    T: qexed_packet::Packet,
{
    let mut buf = bytes::BytesMut::new();
    let mut writer = qexed_packet::PacketWriter::new(&mut buf);
    packet.serialize(&mut writer).unwrap();

    let mut bytes = buf.freeze();
    let mut reader = qexed_packet::PacketReader::new(&mut bytes);
    let mut decoded = T::default();
    decoded.deserialize(&mut reader).unwrap();
    decoded
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

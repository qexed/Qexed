use qexed_packet::Packet;
use qexed_packet::PacketCodec;
use qexed_protocol::protocol_26_3::{
    PacketDirection, ProtocolState, packet_by_id, packet_by_name,
};

#[test]
fn registry_has_26_3_play_ids() {
    let login = packet_by_name(
        ProtocolState::Play,
        PacketDirection::Clientbound,
        "clientbound_login",
    )
    .unwrap();
    assert_eq!(login.id, 0x32);

    let keep_alive = packet_by_name(
        ProtocolState::Play,
        PacketDirection::Serverbound,
        "serverbound_keep_alive",
    )
    .unwrap();
    assert_eq!(keep_alive.id, 0x1C);

    let packet = packet_by_id(ProtocolState::Play, PacketDirection::Clientbound, 0x8F).unwrap();
    assert_eq!(packet.name, "clientbound_show_dialog");
}

#[test]
fn existing_packet_types_use_26_3_ids() {
    assert_eq!(qexed_protocol::to_client::play::login::Login::ID, 0x32);
    assert_eq!(
        qexed_protocol::to_client::play::initialize_border::InitializeBorder::ID,
        0x2C
    );
    assert_eq!(
        qexed_protocol::to_client::play::keep_alive::KeepAlive::ID,
        0x2D
    );
    assert_eq!(
        qexed_protocol::to_client::play::light_update::LightUpdate::ID,
        0x31
    );
    assert_eq!(
        qexed_protocol::to_client::play::player_info_update::PlayerInfoUpdate::ID,
        0x47
    );
    assert_eq!(
        qexed_protocol::to_client::play::forget_level_chunk::ForgetLevelChunk::ID,
        0x26
    );
    assert_eq!(
        qexed_protocol::to_client::play::add_entity::AddEntity::ID,
        0x1
    );
    assert_eq!(
        qexed_protocol::to_client::play::player_info_remove::PlayerInfoRemove::ID,
        0x46
    );
    assert_eq!(
        qexed_protocol::to_client::play::remove_entities::RemoveEntities::ID,
        0x4E
    );
    assert_eq!(
        qexed_protocol::to_client::play::rotate_head::RotateHead::ID,
        0x55
    );
    assert_eq!(
        qexed_protocol::to_client::play::teleport_entity::TeleportEntity::ID,
        0x80
    );
    assert_eq!(
        qexed_protocol::to_client::play::entity_position_sync::EntityPositionSync::ID,
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
        0x68
    );
    assert_eq!(
        qexed_protocol::to_client::play::set_player_inventory::SetPlayerInventory::ID,
        0x6E
    );
    assert_eq!(
        qexed_protocol::to_client::play::set_entity_data::SetEntityData::ID,
        0x65
    );
    assert_eq!(
        qexed_protocol::to_client::play::reset_score::ResetScore::ID,
        0x50
    );
    assert_eq!(qexed_protocol::to_client::play::respawn::Respawn::ID, 0x54);
    assert_eq!(
        qexed_protocol::to_client::play::set_display_objective::SetDisplayObjective::ID,
        0x64
    );
    assert_eq!(
        qexed_protocol::to_client::play::set_objective::SetObjective::ID,
        0x6C
    );
    assert_eq!(
        qexed_protocol::to_client::play::set_score::SetScore::ID,
        0x70
    );
    assert_eq!(
        qexed_protocol::to_client::play::player_chat::PlayerChat::ID,
        0x42
    );
    assert_eq!(qexed_protocol::to_server::play::chat_ack::ChatAck::ID, 0x06);
    assert_eq!(
        qexed_protocol::to_client::play::server_data::ServerData::ID,
        0x58
    );
    assert_eq!(
        qexed_protocol::to_client::play::set_experience::SetExperience::ID,
        0x69
    );
    assert_eq!(
        qexed_protocol::to_client::play::set_health::SetHealth::ID,
        0x6A
    );
    assert_eq!(
        qexed_protocol::to_client::play::set_held_slot::SetHeldSlot::ID,
        0x6B
    );
    assert_eq!(
        qexed_protocol::to_client::play::set_simulation_distance::SetSimulationDistance::ID,
        0x71
    );
    assert_eq!(qexed_protocol::to_client::play::set_time::SetTime::ID, 0x73);
    assert_eq!(
        qexed_protocol::to_client::play::ticking_state::TickingState::ID,
        0x82
    );
    assert_eq!(
        qexed_protocol::to_client::play::update_recipes::UpdateRecipes::ID,
        0x88
    );
    assert_eq!(
        qexed_protocol::to_server::play::keep_alive::KeepAlive::ID,
        0x1C
    );
    assert_eq!(
        qexed_protocol::to_server::play::pos::Pos::ID,
        0x1E
    );
    assert_eq!(
        qexed_protocol::to_server::play::pos_rot::PosRot::ID,
        0x1F
    );
    assert_eq!(
        qexed_protocol::to_server::play::rot::Rot::ID,
        0x20
    );
    assert_eq!(
        qexed_protocol::to_server::play::status_only::StatusOnly::ID,
        0x21
    );
    assert_eq!(
        qexed_protocol::to_server::play::pick_item_from_block::PickItemFromBlock::ID,
        0x24
    );
    assert_eq!(
        qexed_protocol::to_server::play::player_loaded::PlayerLoaded::ID,
        0x2C
    );
    assert_eq!(
        qexed_protocol::to_server::play::set_carried_item::SetCarriedItem::ID,
        0x36
    );
    assert_eq!(
        qexed_protocol::to_server::play::set_creative_mode_slot::SetCreativeModeSlot::ID,
        0x39
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
fn scoreboard_packet_ids_use_26_3_registry() {
    assert_eq!(
        qexed_protocol::to_client::play::set_objective::SetObjective::ID,
        0x6C
    );
    assert_eq!(
        qexed_protocol::to_client::play::set_display_objective::SetDisplayObjective::ID,
        0x64
    );
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

    let item_stack_display = qexed_protocol::types::SlotDisplay::ItemStack(
        qexed_protocol::types::slot_display_types::minecraft::ItemStack {
            item_stack: qexed_protocol::types::Slot {
                item_count: qexed_packet::net_types::VarInt(1),
                item_id: Some(qexed_packet::net_types::VarInt(36)),
                number_of_components_to_add: Some(qexed_packet::net_types::VarInt(0)),
                number_of_components_to_remove: Some(qexed_packet::net_types::VarInt(0)),
                components_to_add: None,
                components_to_remove: None,
            },
        },
    );
    let mut item_stack_display_buf = bytes::BytesMut::new();
    item_stack_display
        .serialize(&mut qexed_packet::PacketWriter::new(
            &mut item_stack_display_buf,
        ))
        .unwrap();
    assert_eq!(item_stack_display_buf.as_ref(), &[5, 36, 1, 0, 0]);

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
                group: qexed_packet::net_types::OptionalVarInt(None),
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

    let mut group_buf = bytes::BytesMut::new();
    qexed_packet::net_types::OptionalVarInt(Some(qexed_packet::net_types::VarInt(37)))
        .serialize(&mut qexed_packet::PacketWriter::new(&mut group_buf))
        .unwrap();
    assert_eq!(group_buf.as_ref(), &[38]);

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
    let mut packet = qexed_protocol::to_client::status::status_response::StatusResponse::default();
    packet.deserialize(&mut reader).unwrap();

    assert_eq!(packet.status.0["players"]["max"], 20);
}

#[test]
fn login_hello_uses_varint_prefixed_byte_arrays() {
    let packet = qexed_protocol::to_client::login::hello::Hello {
        server_id: String::new(),
        public_key: vec![1, 2, 3].into(),
        challenge: vec![4, 5].into(),
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
    let packet = qexed_protocol::to_client::login::login_finished::LoginFinished {
        game_profile: qexed_packet::net_types::GameProfile {
            uuid,
            username: "Steve".to_string(),
            properties: vec![qexed_packet::net_types::ProfileProperty {
                name: "textures".to_string(),
                value: "value".to_string(),
                signature: Some("signature".to_string()),
            }],
        },
        session_id: uuid,
    };

    let mut buf = bytes::BytesMut::new();
    let mut writer = qexed_packet::PacketWriter::new(&mut buf);
    packet.serialize(&mut writer).unwrap();

    let mut bytes = buf.freeze();
    let mut reader = qexed_packet::PacketReader::new(&mut bytes);
    let mut decoded = qexed_protocol::to_client::login::login_finished::LoginFinished::default();
    decoded.deserialize(&mut reader).unwrap();

    assert_eq!(decoded.game_profile.uuid, uuid);
    assert_eq!(decoded.game_profile.username, "Steve");
    assert_eq!(
        decoded.game_profile.properties[0].signature.as_deref(),
        Some("signature")
    );
}

#[test]
fn configuration_code_of_conduct_packets_use_26_3_ids() {
    assert_eq!(
        qexed_protocol::to_client::configuration::code_of_conduct::CodeOfConduct::ID,
        0x14
    );
    assert_eq!(
        qexed_protocol::to_server::configuration::accept_code_of_conduct::AcceptCodeOfConduct::ID,
        0x09
    );
}

#[test]
fn configuration_resource_pack_packets_use_26_3_ids() {
    assert_eq!(
        qexed_protocol::to_client::configuration::resource_pack_push::ResourcePackPush::ID,
        0x9
    );
    assert_eq!(
        qexed_protocol::to_client::configuration::resource_pack_pop::ResourcePackPop::ID,
        0x8
    );
    assert_eq!(
        qexed_protocol::to_server::configuration::resource_pack::ResourcePack::ID,
        0x06
    );
}
#[test]
fn merchant_offers_count_prefix_round_trips() {
    use qexed_protocol::to_client::play::merchant_offers::{ItemCost, MerchantOffer, MerchantOffers};

    let cost_a = ItemCost {
        item: qexed_packet::net_types::VarInt(4),
        count: qexed_packet::net_types::VarInt(3),
        components: Vec::new(),
    };
    let cost_b = ItemCost {
        item: qexed_packet::net_types::VarInt(5),
        count: qexed_packet::net_types::VarInt(2),
        components: Vec::new(),
    };
    let offer = MerchantOffer {
        base_cost_a: cost_a,
        result: qexed_protocol::types::Slot {
            item_count: qexed_packet::net_types::VarInt(1),
            item_id: Some(qexed_packet::net_types::VarInt(36)),
            number_of_components_to_add: Some(qexed_packet::net_types::VarInt(0)),
            number_of_components_to_remove: Some(qexed_packet::net_types::VarInt(0)),
            components_to_add: None,
            components_to_remove: None,
        },
        cost_b: Some(cost_b),
        out_of_stock: false,
        uses: 1,
        max_uses: 12,
        villager_xp: 5,
        special_price_diff: -1,
        price_multiplier: 0.05,
        demand: 0,
    };
    let packet = MerchantOffers {
        container_id: qexed_packet::net_types::VarInt(1),
        offers: vec![offer],
        villager_level: qexed_packet::net_types::VarInt(2),
        villager_xp: qexed_packet::net_types::VarInt(10),
        show_progress: true,
        can_restock: true,
    };

    let mut buf = bytes::BytesMut::new();
    let mut writer = qexed_packet::PacketWriter::new(&mut buf);
    packet.serialize(&mut writer).unwrap();

    // offers 容器为计数前缀（ByteBufCodecs$26 readCount/writeCount）而非终止符：
    // container_id(0x01) 之后紧跟 offer 计数 0x01。
    assert_eq!(buf[0], 1);
    assert_eq!(buf[1], 1);

    let mut bytes = buf.freeze();
    let mut reader = qexed_packet::PacketReader::new(&mut bytes);
    let mut decoded = MerchantOffers::default();
    decoded.deserialize(&mut reader).unwrap();
    assert_eq!(decoded.offers.len(), 1);
    assert_eq!(decoded.offers[0].base_cost_a.item.0, 4);
    assert_eq!(decoded.offers[0].cost_b.as_ref().unwrap().count.0, 2);
    assert_eq!(decoded.offers[0].price_multiplier, 0.05);

    let empty = round_trip(MerchantOffers::default());
    assert!(empty.offers.is_empty());
}

#[test]
fn set_objective_conditional_fields_match_java() {
    use qexed_protocol::to_client::play::set_objective::SetObjective;

    let add = SetObjective {
        objective_name: "obj".to_string(),
        method: 0,
        display_name: text_component("Title"),
        render_type: qexed_packet::net_types::VarInt(0),
        number_format: None,
    };
    let mut buf = bytes::BytesMut::new();
    let mut writer = qexed_packet::PacketWriter::new(&mut buf);
    add.serialize(&mut writer).unwrap();
    let add_len = buf.len();

    let remove = SetObjective {
        method: 1,
        ..add.clone()
    };
    let mut buf2 = bytes::BytesMut::new();
    let mut writer2 = qexed_packet::PacketWriter::new(&mut buf2);
    remove.serialize(&mut writer2).unwrap();

    // method=1(remove) 时 Java 完全不写后三个字段：仅 name + method。
    assert!(buf2.len() < add_len);
    assert_eq!(buf2.len(), 2 + "obj".len());

    let mut bytes = buf2.freeze();
    let mut reader = qexed_packet::PacketReader::new(&mut bytes);
    let mut decoded = SetObjective::default();
    decoded.deserialize(&mut reader).unwrap();
    assert_eq!(decoded.method, 1);
    assert_eq!(decoded.number_format, None);
}

#[test]
fn set_player_team_conditional_fields_match_java() {
    use qexed_protocol::to_client::play::set_player_team::{SetPlayerTeam, TeamParameters};

    let parameters = TeamParameters {
        friendly_flags: 3,
        color: Some(qexed_packet::net_types::VarInt(4)),
        display_name: text_component("Red"),
        player_prefix: text_component("["),
        player_suffix: text_component("]"),
        name_tag_visibility: qexed_packet::net_types::VarInt(0),
        collision_rule: qexed_packet::net_types::VarInt(0),
    };
    let add = SetPlayerTeam {
        name: "team".to_string(),
        method: 0,
        players: vec!["Steve".to_string()],
        parameters: Some(parameters),
    };
    let mut buf = bytes::BytesMut::new();
    let mut writer = qexed_packet::PacketWriter::new(&mut buf);
    add.serialize(&mut writer).unwrap();
    let add_len = buf.len();

    let decoded = round_trip(add);
    assert_eq!(decoded.parameters.as_ref().unwrap().friendly_flags, 3);
    assert_eq!(decoded.players, vec!["Steve".to_string()]);

    // method=1(remove)：无 parameters 也无 players，仅 name + method。
    let remove = SetPlayerTeam {
        name: "team".to_string(),
        method: 1,
        players: Vec::new(),
        parameters: None,
    };
    let mut buf2 = bytes::BytesMut::new();
    let mut writer2 = qexed_packet::PacketWriter::new(&mut buf2);
    remove.serialize(&mut writer2).unwrap();
    assert!(buf2.len() < add_len);
    assert_eq!(buf2.len(), 2 + "team".len());

    // method=4(leave)：有 players 无 parameters。
    let leave = SetPlayerTeam {
        name: "team".to_string(),
        method: 4,
        players: vec!["Alex".to_string()],
        parameters: None,
    };
    let decoded_leave = round_trip(leave);
    assert!(decoded_leave.parameters.is_none());
    assert_eq!(decoded_leave.players, vec!["Alex".to_string()]);
}


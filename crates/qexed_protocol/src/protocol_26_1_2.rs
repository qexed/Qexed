#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolState {
    Handshaking,
    Status,
    Login,
    Configuration,
    Play,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PacketDirection {
    Serverbound,
    Clientbound,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PacketSpec {
    pub id: i32,
    pub name: &'static str,
}

pub const HANDSHAKING_SERVERBOUND_PACKETS: &[PacketSpec] = &[PacketSpec {
    id: 0x00,
    name: "client_intention",
}];

pub const STATUS_SERVERBOUND_PACKETS: &[PacketSpec] = &[
    PacketSpec {
        id: 0x00,
        name: "serverbound_status_request",
    },
    PacketSpec {
        id: 0x01,
        name: "serverbound_ping_request",
    },
];

pub const STATUS_CLIENTBOUND_PACKETS: &[PacketSpec] = &[
    PacketSpec {
        id: 0x00,
        name: "clientbound_status_response",
    },
    PacketSpec {
        id: 0x01,
        name: "clientbound_pong_response",
    },
];

pub const LOGIN_SERVERBOUND_PACKETS: &[PacketSpec] = &[
    PacketSpec {
        id: 0x00,
        name: "serverbound_hello",
    },
    PacketSpec {
        id: 0x01,
        name: "serverbound_key",
    },
    PacketSpec {
        id: 0x02,
        name: "serverbound_custom_query_answer",
    },
    PacketSpec {
        id: 0x03,
        name: "serverbound_login_acknowledged",
    },
    PacketSpec {
        id: 0x04,
        name: "serverbound_cookie_response",
    },
];

pub const LOGIN_CLIENTBOUND_PACKETS: &[PacketSpec] = &[
    PacketSpec {
        id: 0x00,
        name: "clientbound_login_disconnect",
    },
    PacketSpec {
        id: 0x01,
        name: "clientbound_hello",
    },
    PacketSpec {
        id: 0x02,
        name: "clientbound_login_finished",
    },
    PacketSpec {
        id: 0x03,
        name: "clientbound_login_compression",
    },
    PacketSpec {
        id: 0x04,
        name: "clientbound_custom_query",
    },
    PacketSpec {
        id: 0x05,
        name: "clientbound_cookie_request",
    },
];

pub const CONFIGURATION_SERVERBOUND_PACKETS: &[PacketSpec] = &[
    PacketSpec {
        id: 0x00,
        name: "serverbound_client_information",
    },
    PacketSpec {
        id: 0x01,
        name: "serverbound_cookie_response",
    },
    PacketSpec {
        id: 0x02,
        name: "serverbound_custom_payload",
    },
    PacketSpec {
        id: 0x03,
        name: "serverbound_finish_configuration",
    },
    PacketSpec {
        id: 0x04,
        name: "serverbound_keep_alive",
    },
    PacketSpec {
        id: 0x05,
        name: "serverbound_pong",
    },
    PacketSpec {
        id: 0x06,
        name: "serverbound_resource_pack",
    },
    PacketSpec {
        id: 0x07,
        name: "serverbound_select_known_packs",
    },
    PacketSpec {
        id: 0x08,
        name: "serverbound_custom_click_action",
    },
    PacketSpec {
        id: 0x09,
        name: "serverbound_accept_code_of_conduct",
    },
];

pub const CONFIGURATION_CLIENTBOUND_PACKETS: &[PacketSpec] = &[
    PacketSpec {
        id: 0x00,
        name: "clientbound_cookie_request",
    },
    PacketSpec {
        id: 0x01,
        name: "clientbound_custom_payload",
    },
    PacketSpec {
        id: 0x02,
        name: "clientbound_disconnect",
    },
    PacketSpec {
        id: 0x03,
        name: "clientbound_finish_configuration",
    },
    PacketSpec {
        id: 0x04,
        name: "clientbound_keep_alive",
    },
    PacketSpec {
        id: 0x05,
        name: "clientbound_ping",
    },
    PacketSpec {
        id: 0x06,
        name: "clientbound_reset_chat",
    },
    PacketSpec {
        id: 0x07,
        name: "clientbound_registry_data",
    },
    PacketSpec {
        id: 0x08,
        name: "clientbound_resource_pack_pop",
    },
    PacketSpec {
        id: 0x09,
        name: "clientbound_resource_pack_push",
    },
    PacketSpec {
        id: 0x0A,
        name: "clientbound_store_cookie",
    },
    PacketSpec {
        id: 0x0B,
        name: "clientbound_transfer",
    },
    PacketSpec {
        id: 0x0C,
        name: "clientbound_update_enabled_features",
    },
    PacketSpec {
        id: 0x0D,
        name: "clientbound_update_tags",
    },
    PacketSpec {
        id: 0x0E,
        name: "clientbound_select_known_packs",
    },
    PacketSpec {
        id: 0x0F,
        name: "clientbound_custom_report_details",
    },
    PacketSpec {
        id: 0x10,
        name: "clientbound_server_links",
    },
    PacketSpec {
        id: 0x11,
        name: "clientbound_clear_dialog",
    },
    PacketSpec {
        id: 0x12,
        name: "clientbound_show_dialog",
    },
    PacketSpec {
        id: 0x13,
        name: "clientbound_code_of_conduct",
    },
];

pub const PLAY_SERVERBOUND_PACKETS: &[PacketSpec] = &[
    PacketSpec {
        id: 0x00,
        name: "serverbound_accept_teleportation",
    },
    PacketSpec {
        id: 0x01,
        name: "serverbound_attack",
    },
    PacketSpec {
        id: 0x02,
        name: "serverbound_block_entity_tag_query",
    },
    PacketSpec {
        id: 0x03,
        name: "serverbound_bundle_item_selected",
    },
    PacketSpec {
        id: 0x04,
        name: "serverbound_change_difficulty",
    },
    PacketSpec {
        id: 0x05,
        name: "serverbound_change_game_mode",
    },
    PacketSpec {
        id: 0x06,
        name: "serverbound_chat_ack",
    },
    PacketSpec {
        id: 0x07,
        name: "serverbound_chat_command",
    },
    PacketSpec {
        id: 0x08,
        name: "serverbound_chat_command_signed",
    },
    PacketSpec {
        id: 0x09,
        name: "serverbound_chat",
    },
    PacketSpec {
        id: 0x0A,
        name: "serverbound_chat_session_update",
    },
    PacketSpec {
        id: 0x0B,
        name: "serverbound_chunk_batch_received",
    },
    PacketSpec {
        id: 0x0C,
        name: "serverbound_client_command",
    },
    PacketSpec {
        id: 0x0D,
        name: "serverbound_client_tick_end",
    },
    PacketSpec {
        id: 0x0E,
        name: "serverbound_client_information",
    },
    PacketSpec {
        id: 0x0F,
        name: "serverbound_command_suggestion",
    },
    PacketSpec {
        id: 0x10,
        name: "serverbound_configuration_acknowledged",
    },
    PacketSpec {
        id: 0x11,
        name: "serverbound_container_button_click",
    },
    PacketSpec {
        id: 0x12,
        name: "serverbound_container_click",
    },
    PacketSpec {
        id: 0x13,
        name: "serverbound_container_close",
    },
    PacketSpec {
        id: 0x14,
        name: "serverbound_container_slot_state_changed",
    },
    PacketSpec {
        id: 0x15,
        name: "serverbound_cookie_response",
    },
    PacketSpec {
        id: 0x16,
        name: "serverbound_custom_payload",
    },
    PacketSpec {
        id: 0x17,
        name: "serverbound_debug_subscription_request",
    },
    PacketSpec {
        id: 0x18,
        name: "serverbound_edit_book",
    },
    PacketSpec {
        id: 0x19,
        name: "serverbound_entity_tag_query",
    },
    PacketSpec {
        id: 0x1A,
        name: "serverbound_interact",
    },
    PacketSpec {
        id: 0x1B,
        name: "serverbound_jigsaw_generate",
    },
    PacketSpec {
        id: 0x1C,
        name: "serverbound_keep_alive",
    },
    PacketSpec {
        id: 0x1D,
        name: "serverbound_lock_difficulty",
    },
    PacketSpec {
        id: 0x1E,
        name: "serverbound_move_player_pos",
    },
    PacketSpec {
        id: 0x1F,
        name: "serverbound_move_player_pos_rot",
    },
    PacketSpec {
        id: 0x20,
        name: "serverbound_move_player_rot",
    },
    PacketSpec {
        id: 0x21,
        name: "serverbound_move_player_status_only",
    },
    PacketSpec {
        id: 0x22,
        name: "serverbound_move_vehicle",
    },
    PacketSpec {
        id: 0x23,
        name: "serverbound_paddle_boat",
    },
    PacketSpec {
        id: 0x24,
        name: "serverbound_pick_item_from_block",
    },
    PacketSpec {
        id: 0x25,
        name: "serverbound_pick_item_from_entity",
    },
    PacketSpec {
        id: 0x26,
        name: "serverbound_ping_request",
    },
    PacketSpec {
        id: 0x27,
        name: "serverbound_place_recipe",
    },
    PacketSpec {
        id: 0x28,
        name: "serverbound_player_abilities",
    },
    PacketSpec {
        id: 0x29,
        name: "serverbound_player_action",
    },
    PacketSpec {
        id: 0x2A,
        name: "serverbound_player_command",
    },
    PacketSpec {
        id: 0x2B,
        name: "serverbound_player_input",
    },
    PacketSpec {
        id: 0x2C,
        name: "serverbound_player_loaded",
    },
    PacketSpec {
        id: 0x2D,
        name: "serverbound_pong",
    },
    PacketSpec {
        id: 0x2E,
        name: "serverbound_recipe_book_change_settings",
    },
    PacketSpec {
        id: 0x2F,
        name: "serverbound_recipe_book_seen_recipe",
    },
    PacketSpec {
        id: 0x30,
        name: "serverbound_rename_item",
    },
    PacketSpec {
        id: 0x31,
        name: "serverbound_resource_pack",
    },
    PacketSpec {
        id: 0x32,
        name: "serverbound_seen_advancements",
    },
    PacketSpec {
        id: 0x33,
        name: "serverbound_select_trade",
    },
    PacketSpec {
        id: 0x34,
        name: "serverbound_set_beacon",
    },
    PacketSpec {
        id: 0x35,
        name: "serverbound_set_carried_item",
    },
    PacketSpec {
        id: 0x36,
        name: "serverbound_set_command_block",
    },
    PacketSpec {
        id: 0x37,
        name: "serverbound_set_command_minecart",
    },
    PacketSpec {
        id: 0x38,
        name: "serverbound_set_creative_mode_slot",
    },
    PacketSpec {
        id: 0x39,
        name: "serverbound_set_game_rule",
    },
    PacketSpec {
        id: 0x3A,
        name: "serverbound_set_jigsaw_block",
    },
    PacketSpec {
        id: 0x3B,
        name: "serverbound_set_structure_block",
    },
    PacketSpec {
        id: 0x3C,
        name: "serverbound_set_test_block",
    },
    PacketSpec {
        id: 0x3D,
        name: "serverbound_sign_update",
    },
    PacketSpec {
        id: 0x3E,
        name: "serverbound_spectate_entity",
    },
    PacketSpec {
        id: 0x3F,
        name: "serverbound_swing",
    },
    PacketSpec {
        id: 0x40,
        name: "serverbound_teleport_to_entity",
    },
    PacketSpec {
        id: 0x41,
        name: "serverbound_test_instance_block_action",
    },
    PacketSpec {
        id: 0x42,
        name: "serverbound_use_item_on",
    },
    PacketSpec {
        id: 0x43,
        name: "serverbound_use_item",
    },
    PacketSpec {
        id: 0x44,
        name: "serverbound_custom_click_action",
    },
];

pub const PLAY_CLIENTBOUND_PACKETS: &[PacketSpec] = &[
    PacketSpec {
        id: 0x00,
        name: "clientbound_bundle_delimiter",
    },
    PacketSpec {
        id: 0x01,
        name: "clientbound_add_entity",
    },
    PacketSpec {
        id: 0x02,
        name: "clientbound_animate",
    },
    PacketSpec {
        id: 0x03,
        name: "clientbound_award_stats",
    },
    PacketSpec {
        id: 0x04,
        name: "clientbound_block_changed_ack",
    },
    PacketSpec {
        id: 0x05,
        name: "clientbound_block_destruction",
    },
    PacketSpec {
        id: 0x06,
        name: "clientbound_block_entity_data",
    },
    PacketSpec {
        id: 0x07,
        name: "clientbound_block_event",
    },
    PacketSpec {
        id: 0x08,
        name: "clientbound_block_update",
    },
    PacketSpec {
        id: 0x09,
        name: "clientbound_boss_event",
    },
    PacketSpec {
        id: 0x0A,
        name: "clientbound_change_difficulty",
    },
    PacketSpec {
        id: 0x0B,
        name: "clientbound_chunk_batch_finished",
    },
    PacketSpec {
        id: 0x0C,
        name: "clientbound_chunk_batch_start",
    },
    PacketSpec {
        id: 0x0D,
        name: "clientbound_chunks_biomes",
    },
    PacketSpec {
        id: 0x0E,
        name: "clientbound_clear_titles",
    },
    PacketSpec {
        id: 0x0F,
        name: "clientbound_command_suggestions",
    },
    PacketSpec {
        id: 0x10,
        name: "clientbound_commands",
    },
    PacketSpec {
        id: 0x11,
        name: "clientbound_container_close",
    },
    PacketSpec {
        id: 0x12,
        name: "clientbound_container_set_content",
    },
    PacketSpec {
        id: 0x13,
        name: "clientbound_container_set_data",
    },
    PacketSpec {
        id: 0x14,
        name: "clientbound_container_set_slot",
    },
    PacketSpec {
        id: 0x15,
        name: "clientbound_cookie_request",
    },
    PacketSpec {
        id: 0x16,
        name: "clientbound_cooldown",
    },
    PacketSpec {
        id: 0x17,
        name: "clientbound_custom_chat_completions",
    },
    PacketSpec {
        id: 0x18,
        name: "clientbound_custom_payload",
    },
    PacketSpec {
        id: 0x19,
        name: "clientbound_damage_event",
    },
    PacketSpec {
        id: 0x1A,
        name: "clientbound_debug_block_value",
    },
    PacketSpec {
        id: 0x1B,
        name: "clientbound_debug_chunk_value",
    },
    PacketSpec {
        id: 0x1C,
        name: "clientbound_debug_entity_value",
    },
    PacketSpec {
        id: 0x1D,
        name: "clientbound_debug_event",
    },
    PacketSpec {
        id: 0x1E,
        name: "clientbound_debug_sample",
    },
    PacketSpec {
        id: 0x1F,
        name: "clientbound_delete_chat",
    },
    PacketSpec {
        id: 0x20,
        name: "clientbound_disconnect",
    },
    PacketSpec {
        id: 0x21,
        name: "clientbound_disguised_chat",
    },
    PacketSpec {
        id: 0x22,
        name: "clientbound_entity_event",
    },
    PacketSpec {
        id: 0x23,
        name: "clientbound_entity_position_sync",
    },
    PacketSpec {
        id: 0x24,
        name: "clientbound_explode",
    },
    PacketSpec {
        id: 0x25,
        name: "clientbound_forget_level_chunk",
    },
    PacketSpec {
        id: 0x26,
        name: "clientbound_game_event",
    },
    PacketSpec {
        id: 0x27,
        name: "clientbound_game_rule_values",
    },
    PacketSpec {
        id: 0x28,
        name: "clientbound_game_test_highlight_pos",
    },
    PacketSpec {
        id: 0x29,
        name: "clientbound_mount_screen_open",
    },
    PacketSpec {
        id: 0x2A,
        name: "clientbound_hurt_animation",
    },
    PacketSpec {
        id: 0x2B,
        name: "clientbound_initialize_border",
    },
    PacketSpec {
        id: 0x2C,
        name: "clientbound_keep_alive",
    },
    PacketSpec {
        id: 0x2D,
        name: "clientbound_level_chunk_with_light",
    },
    PacketSpec {
        id: 0x2E,
        name: "clientbound_level_event",
    },
    PacketSpec {
        id: 0x2F,
        name: "clientbound_level_particles",
    },
    PacketSpec {
        id: 0x30,
        name: "clientbound_light_update",
    },
    PacketSpec {
        id: 0x31,
        name: "clientbound_login",
    },
    PacketSpec {
        id: 0x32,
        name: "clientbound_low_disk_space_warning",
    },
    PacketSpec {
        id: 0x33,
        name: "clientbound_map_item_data",
    },
    PacketSpec {
        id: 0x34,
        name: "clientbound_merchant_offers",
    },
    PacketSpec {
        id: 0x35,
        name: "clientbound_move_entity_pos",
    },
    PacketSpec {
        id: 0x36,
        name: "clientbound_move_entity_pos_rot",
    },
    PacketSpec {
        id: 0x37,
        name: "clientbound_move_minecart_along_track",
    },
    PacketSpec {
        id: 0x38,
        name: "clientbound_move_entity_rot",
    },
    PacketSpec {
        id: 0x39,
        name: "clientbound_move_vehicle",
    },
    PacketSpec {
        id: 0x3A,
        name: "clientbound_open_book",
    },
    PacketSpec {
        id: 0x3B,
        name: "clientbound_open_screen",
    },
    PacketSpec {
        id: 0x3C,
        name: "clientbound_open_sign_editor",
    },
    PacketSpec {
        id: 0x3D,
        name: "clientbound_ping",
    },
    PacketSpec {
        id: 0x3E,
        name: "clientbound_pong_response",
    },
    PacketSpec {
        id: 0x3F,
        name: "clientbound_place_ghost_recipe",
    },
    PacketSpec {
        id: 0x40,
        name: "clientbound_player_abilities",
    },
    PacketSpec {
        id: 0x41,
        name: "clientbound_player_chat",
    },
    PacketSpec {
        id: 0x42,
        name: "clientbound_player_combat_end",
    },
    PacketSpec {
        id: 0x43,
        name: "clientbound_player_combat_enter",
    },
    PacketSpec {
        id: 0x44,
        name: "clientbound_player_combat_kill",
    },
    PacketSpec {
        id: 0x45,
        name: "clientbound_player_info_remove",
    },
    PacketSpec {
        id: 0x46,
        name: "clientbound_player_info_update",
    },
    PacketSpec {
        id: 0x47,
        name: "clientbound_player_look_at",
    },
    PacketSpec {
        id: 0x48,
        name: "clientbound_player_position",
    },
    PacketSpec {
        id: 0x49,
        name: "clientbound_player_rotation",
    },
    PacketSpec {
        id: 0x4A,
        name: "clientbound_recipe_book_add",
    },
    PacketSpec {
        id: 0x4B,
        name: "clientbound_recipe_book_remove",
    },
    PacketSpec {
        id: 0x4C,
        name: "clientbound_recipe_book_settings",
    },
    PacketSpec {
        id: 0x4D,
        name: "clientbound_remove_entities",
    },
    PacketSpec {
        id: 0x4E,
        name: "clientbound_remove_mob_effect",
    },
    PacketSpec {
        id: 0x4F,
        name: "clientbound_reset_score",
    },
    PacketSpec {
        id: 0x50,
        name: "clientbound_resource_pack_pop",
    },
    PacketSpec {
        id: 0x51,
        name: "clientbound_resource_pack_push",
    },
    PacketSpec {
        id: 0x52,
        name: "clientbound_respawn",
    },
    PacketSpec {
        id: 0x53,
        name: "clientbound_rotate_head",
    },
    PacketSpec {
        id: 0x54,
        name: "clientbound_section_blocks_update",
    },
    PacketSpec {
        id: 0x55,
        name: "clientbound_select_advancements_tab",
    },
    PacketSpec {
        id: 0x56,
        name: "clientbound_server_data",
    },
    PacketSpec {
        id: 0x57,
        name: "clientbound_set_action_bar_text",
    },
    PacketSpec {
        id: 0x58,
        name: "clientbound_set_border_center",
    },
    PacketSpec {
        id: 0x59,
        name: "clientbound_set_border_lerp_size",
    },
    PacketSpec {
        id: 0x5A,
        name: "clientbound_set_border_size",
    },
    PacketSpec {
        id: 0x5B,
        name: "clientbound_set_border_warning_delay",
    },
    PacketSpec {
        id: 0x5C,
        name: "clientbound_set_border_warning_distance",
    },
    PacketSpec {
        id: 0x5D,
        name: "clientbound_set_camera",
    },
    PacketSpec {
        id: 0x5E,
        name: "clientbound_set_chunk_cache_center",
    },
    PacketSpec {
        id: 0x5F,
        name: "clientbound_set_chunk_cache_radius",
    },
    PacketSpec {
        id: 0x60,
        name: "clientbound_set_cursor_item",
    },
    PacketSpec {
        id: 0x61,
        name: "clientbound_set_default_spawn_position",
    },
    PacketSpec {
        id: 0x62,
        name: "clientbound_set_display_objective",
    },
    PacketSpec {
        id: 0x63,
        name: "clientbound_set_entity_data",
    },
    PacketSpec {
        id: 0x64,
        name: "clientbound_set_entity_link",
    },
    PacketSpec {
        id: 0x65,
        name: "clientbound_set_entity_motion",
    },
    PacketSpec {
        id: 0x66,
        name: "clientbound_set_equipment",
    },
    PacketSpec {
        id: 0x67,
        name: "clientbound_set_experience",
    },
    PacketSpec {
        id: 0x68,
        name: "clientbound_set_health",
    },
    PacketSpec {
        id: 0x69,
        name: "clientbound_set_held_slot",
    },
    PacketSpec {
        id: 0x6A,
        name: "clientbound_set_objective",
    },
    PacketSpec {
        id: 0x6B,
        name: "clientbound_set_passengers",
    },
    PacketSpec {
        id: 0x6C,
        name: "clientbound_set_player_inventory",
    },
    PacketSpec {
        id: 0x6D,
        name: "clientbound_set_player_team",
    },
    PacketSpec {
        id: 0x6E,
        name: "clientbound_set_score",
    },
    PacketSpec {
        id: 0x6F,
        name: "clientbound_set_simulation_distance",
    },
    PacketSpec {
        id: 0x70,
        name: "clientbound_set_subtitle_text",
    },
    PacketSpec {
        id: 0x71,
        name: "clientbound_set_time",
    },
    PacketSpec {
        id: 0x72,
        name: "clientbound_set_title_text",
    },
    PacketSpec {
        id: 0x73,
        name: "clientbound_set_titles_animation",
    },
    PacketSpec {
        id: 0x74,
        name: "clientbound_sound_entity",
    },
    PacketSpec {
        id: 0x75,
        name: "clientbound_sound",
    },
    PacketSpec {
        id: 0x76,
        name: "clientbound_start_configuration",
    },
    PacketSpec {
        id: 0x77,
        name: "clientbound_stop_sound",
    },
    PacketSpec {
        id: 0x78,
        name: "clientbound_store_cookie",
    },
    PacketSpec {
        id: 0x79,
        name: "clientbound_system_chat",
    },
    PacketSpec {
        id: 0x7A,
        name: "clientbound_tab_list",
    },
    PacketSpec {
        id: 0x7B,
        name: "clientbound_tag_query",
    },
    PacketSpec {
        id: 0x7C,
        name: "clientbound_take_item_entity",
    },
    PacketSpec {
        id: 0x7D,
        name: "clientbound_teleport_entity",
    },
    PacketSpec {
        id: 0x7E,
        name: "clientbound_test_instance_block_status",
    },
    PacketSpec {
        id: 0x7F,
        name: "clientbound_ticking_state",
    },
    PacketSpec {
        id: 0x80,
        name: "clientbound_ticking_step",
    },
    PacketSpec {
        id: 0x81,
        name: "clientbound_transfer",
    },
    PacketSpec {
        id: 0x82,
        name: "clientbound_update_advancements",
    },
    PacketSpec {
        id: 0x83,
        name: "clientbound_update_attributes",
    },
    PacketSpec {
        id: 0x84,
        name: "clientbound_update_mob_effect",
    },
    PacketSpec {
        id: 0x85,
        name: "clientbound_update_recipes",
    },
    PacketSpec {
        id: 0x86,
        name: "clientbound_update_tags",
    },
    PacketSpec {
        id: 0x87,
        name: "clientbound_projectile_power",
    },
    PacketSpec {
        id: 0x88,
        name: "clientbound_custom_report_details",
    },
    PacketSpec {
        id: 0x89,
        name: "clientbound_server_links",
    },
    PacketSpec {
        id: 0x8A,
        name: "clientbound_waypoint",
    },
    PacketSpec {
        id: 0x8B,
        name: "clientbound_clear_dialog",
    },
    PacketSpec {
        id: 0x8C,
        name: "clientbound_show_dialog",
    },
];

pub fn packets(state: ProtocolState, direction: PacketDirection) -> &'static [PacketSpec] {
    match (state, direction) {
        (ProtocolState::Handshaking, PacketDirection::Serverbound) => {
            HANDSHAKING_SERVERBOUND_PACKETS
        }
        (ProtocolState::Handshaking, PacketDirection::Clientbound) => &[],
        (ProtocolState::Status, PacketDirection::Serverbound) => STATUS_SERVERBOUND_PACKETS,
        (ProtocolState::Status, PacketDirection::Clientbound) => STATUS_CLIENTBOUND_PACKETS,
        (ProtocolState::Login, PacketDirection::Serverbound) => LOGIN_SERVERBOUND_PACKETS,
        (ProtocolState::Login, PacketDirection::Clientbound) => LOGIN_CLIENTBOUND_PACKETS,
        (ProtocolState::Configuration, PacketDirection::Serverbound) => {
            CONFIGURATION_SERVERBOUND_PACKETS
        }
        (ProtocolState::Configuration, PacketDirection::Clientbound) => {
            CONFIGURATION_CLIENTBOUND_PACKETS
        }
        (ProtocolState::Play, PacketDirection::Serverbound) => PLAY_SERVERBOUND_PACKETS,
        (ProtocolState::Play, PacketDirection::Clientbound) => PLAY_CLIENTBOUND_PACKETS,
    }
}

pub fn packet_by_id(
    state: ProtocolState,
    direction: PacketDirection,
    id: i32,
) -> Option<&'static PacketSpec> {
    packets(state, direction)
        .iter()
        .find(|packet| packet.id == id)
}

pub fn packet_by_name(
    state: ProtocolState,
    direction: PacketDirection,
    name: &str,
) -> Option<&'static PacketSpec> {
    packets(state, direction)
        .iter()
        .find(|packet| packet.name == name)
}

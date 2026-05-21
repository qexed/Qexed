use qexed_packet::PacketCodec;
#[qexed_packet_macros::packet(id = 0x22)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct GameStateChange {
    pub reason:u8,
    // 值含义:
    // 0:no_respawn_block_available
    // 1:start_raining
    // 2:stop_raining
    // 3:change_game_mode
    // 4:win_game
    // 5:demo_event
    // 6:play_arrow_hit_sound
    // 7:rain_level_change
    // 8:thunder_level_change
    // 9:puffer_fish_sting
    // 10:guardian_elder_effect
    // 11:immediate_respawn
    // 12:limited_crafting
    // 13:level_chunks_load_start
    pub game_mode:f32,
}
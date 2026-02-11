use qexed_packet::{PacketCodec, net_types::VarInt};
#[qexed_packet_macros::packet(id = 0x00)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct Settings {
    pub locale: String,    // e.g. en_GB.
    pub view_distance: i8, // Client-side render distance, in chunks.
    pub chat_mode: VarInt,
    pub chat_colors: bool,
    pub displayed_skin_parts: u8,
    pub main_hand: VarInt,
    pub enable_text_filtering: bool,
    pub allow_server_listings: bool, // 事实上这个没用(你又不是机器人)
    pub particle_status: VarInt,
}
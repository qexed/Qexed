use qexed_packet::{PacketCodec, net_types::VarInt};

#[qexed_packet_macros::packet(id = 0x3B)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct OpenScreen {
    pub window_id: VarInt,
    pub menu_type: VarInt,
    pub title: crate::types::TextComponent,
}

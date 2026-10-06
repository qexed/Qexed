use qexed_packet::{PacketCodec, net_types::VarInt};

#[qexed_packet_macros::packet(id = 0x2)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct Animate {
    pub entity_id: VarInt,
    pub action_id: u8,
}

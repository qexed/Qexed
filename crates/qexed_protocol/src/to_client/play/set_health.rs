use qexed_packet::{PacketCodec, net_types::VarInt};

#[qexed_packet_macros::packet(id = 0x69)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct SetHealth {
    pub health: f32,
    pub food: VarInt,
    pub saturation: f32,
}

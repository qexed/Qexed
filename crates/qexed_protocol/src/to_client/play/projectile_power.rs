use qexed_packet::{PacketCodec, net_types::*};

#[qexed_packet_macros::packet(id = 0x8a)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct ProjectilePower {
    pub id: VarInt,
    pub acceleration_power: f64,
}

use qexed_packet::{PacketCodec, net_types::*};

#[qexed_packet_macros::packet(id = 0x6C)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct SetPassengers {
    pub vehicle: VarInt,
    pub passengers: Vec<VarInt>,
}

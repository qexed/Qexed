use qexed_packet::{PacketCodec, net_types::*};

#[qexed_packet_macros::packet(id = 0x5E)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct SetCamera {
    pub camera_id: VarInt,
}

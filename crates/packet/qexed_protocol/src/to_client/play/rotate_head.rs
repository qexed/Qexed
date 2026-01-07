use qexed_packet::{PacketCodec, net_types::{Angle, VarInt}};

#[qexed_packet_macros::packet(id = 0x4C)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct RotateHead {
    pub entity_id:VarInt,
    pub head_yaw:Angle,
}
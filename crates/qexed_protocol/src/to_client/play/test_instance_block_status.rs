use qexed_packet::{PacketCodec, net_types::*};
use crate::types::*;

#[qexed_packet_macros::packet(id = 0x80)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct TestInstanceBlockStatus {
    pub status: TextComponent,
    // TODO: Optional<Vec3i> 的 presence 由 Option 前缀 bool 表达
    pub size: Option<Vec3i>,
}

#[qexed_packet_macros::substruct]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct Vec3i {
    pub x: VarInt,
    pub y: VarInt,
    pub z: VarInt,
}

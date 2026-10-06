use qexed_packet::{PacketCodec, net_types::*};

#[qexed_packet_macros::packet(id = 0x5F)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct SetChunkCacheCenter {
    pub x: VarInt,
    pub z: VarInt,
}

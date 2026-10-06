use qexed_packet::{PacketCodec, net_types::*};

#[qexed_packet_macros::packet(id = 0x61)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct SetChunkCacheRadius {
    pub radius: VarInt,
}

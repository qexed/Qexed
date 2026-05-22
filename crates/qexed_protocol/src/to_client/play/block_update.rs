use qexed_packet::{
    PacketCodec,
    net_types::{Position, VarInt},
};

#[qexed_packet_macros::packet(id = 0x08)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct BlockUpdate {
    pub location: Position,
    pub block_state: VarInt,
}

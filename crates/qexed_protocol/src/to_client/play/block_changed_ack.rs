use qexed_packet::{PacketCodec, net_types::VarInt};

#[qexed_packet_macros::packet(id = 0x04)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct BlockChangedAck {
    pub sequence: VarInt,
}

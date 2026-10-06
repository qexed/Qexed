use qexed_packet::{PacketCodec, net_types::VarInt};

#[qexed_packet_macros::packet(id = 0x03)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct LoginCompression {
    pub compression_threshold: VarInt,
}

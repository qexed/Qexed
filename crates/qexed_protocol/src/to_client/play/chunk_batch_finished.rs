use qexed_packet::{PacketCodec, net_types::VarInt};

#[qexed_packet_macros::packet(id = 0xb)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct ChunkBatchFinished {
    pub batch_size: VarInt,
}

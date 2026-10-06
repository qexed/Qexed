use qexed_packet::PacketCodec;

#[qexed_packet_macros::packet(id = 0x0b)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct ChunkBatchReceived {
    pub desired_chunks_per_tick: f32,
}

use qexed_packet::PacketCodec;

use crate::data_type::chunk_section::ChunkSection;
#[derive(Debug, Default, PartialEq,Clone)]
pub struct Data {
    pub data: Vec<ChunkSection>
}

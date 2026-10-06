use qexed_packet::{PacketCodec, net_types::VarLong};

#[qexed_packet_macros::packet(id = 0x55)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct SectionBlocksUpdate {
    pub section_position: i64,
    pub blocks: Vec<VarLong>,
}

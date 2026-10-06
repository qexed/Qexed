use qexed_packet::{PacketCodec, net_types::*};

#[qexed_packet_macros::packet(id = 0x7e)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct TagQuery {
    pub transaction_id: VarInt,
    pub tag: qexed_nbt::Tag,
}

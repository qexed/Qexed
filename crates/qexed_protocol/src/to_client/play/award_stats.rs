use qexed_packet::{PacketCodec, net_types::VarInt};

#[qexed_packet_macros::packet(id = 0x3)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct AwardStats {
    pub stats: Vec<AwardStat>,
}

#[qexed_packet_macros::substruct]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct AwardStat {
    pub category_id: VarInt,
    pub stat_id: VarInt,
    pub value: VarInt,
}

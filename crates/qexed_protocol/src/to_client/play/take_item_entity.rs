use qexed_packet::{PacketCodec, net_types::VarInt};

#[qexed_packet_macros::packet(id = 0x7f)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct TakeItemEntity {
    pub item_id: VarInt,
    pub player_id: VarInt,
    pub amount: VarInt,
}

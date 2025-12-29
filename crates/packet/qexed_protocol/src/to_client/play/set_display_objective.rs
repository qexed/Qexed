use qexed_packet::{PacketCodec, net_types::VarInt};
#[qexed_packet_macros::packet(id = 0x5b)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct SetDisplayObjective {
    pub position:VarInt,
    pub name:String,
}

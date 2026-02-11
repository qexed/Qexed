use qexed_packet::{PacketCodec, net_types::VarInt};
#[qexed_packet_macros::packet(id = 0x6B)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct SetTitleText {
    pub text:qexed_nbt::Tag
}
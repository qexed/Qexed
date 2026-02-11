use qexed_packet::{PacketCodec, net_types::VarInt};
#[qexed_packet_macros::packet(id = 0x0d)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct Tags {
    pub tags:Vec<Tags2>,
}
#[qexed_packet_macros::substruct]
#[derive(Debug, Default, PartialEq,Clone)]
pub struct Tags2 {
    pub registry: String,
    pub tags: Vec<Tag>,
}
#[qexed_packet_macros::substruct]
#[derive(Debug, Default, PartialEq,Clone)]
pub struct Tag {
    pub name: String,
    pub entries: Vec<VarInt>,
}

use qexed_packet::{PacketCodec, net_types::VarInt};
#[qexed_packet_macros::packet(id = 0x13)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct ContainerSetData {
    pub window_id:VarInt,
    pub property:i16,
    pub value:i16,
}
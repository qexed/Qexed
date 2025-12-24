use qexed_packet::{PacketCodec, net_types::VarInt};
#[qexed_packet_macros::packet(id = 0x06)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct ResourcePackReceive {
    pub uuid:uuid::Uuid,
    pub result:VarInt,
}

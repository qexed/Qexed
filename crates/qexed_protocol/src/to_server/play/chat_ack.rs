use qexed_packet::PacketCodec;

#[qexed_packet_macros::packet(id = 0x06)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct ChatAck {
    pub offset: qexed_packet::net_types::VarInt,
}

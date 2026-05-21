use qexed_packet::PacketCodec;

#[qexed_packet_macros::packet(id = 0x0a)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct StoreCookie {
    pub key: String,
    pub payload: qexed_packet::net_types::ByteArray,
}

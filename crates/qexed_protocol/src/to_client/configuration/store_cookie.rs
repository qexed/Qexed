use qexed_packet::PacketCodec;

#[qexed_packet_macros::packet(id = 0xB)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct StoreCookie {
    pub key: String,
    pub payload: qexed_packet::net_types::ByteArray,
}

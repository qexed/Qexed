use qexed_packet::PacketCodec;

#[qexed_packet_macros::packet(id = 0x04)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct CookieResponse {
    pub key: String,
    pub payload: Option<qexed_packet::net_types::ByteArray>,
}

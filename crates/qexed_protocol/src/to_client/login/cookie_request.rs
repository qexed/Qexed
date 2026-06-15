use qexed_packet::PacketCodec;

#[qexed_packet_macros::packet(id = 0x05)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct CookieRequest {
    pub key: String,
}

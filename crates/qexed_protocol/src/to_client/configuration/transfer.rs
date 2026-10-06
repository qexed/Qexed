use qexed_packet::PacketCodec;

#[qexed_packet_macros::packet(id = 0xC)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct Transfer {
    pub host: String,
    pub port: qexed_packet::net_types::VarInt,
}
impl Transfer {
    pub fn new(host: String, port: qexed_packet::net_types::VarInt) -> Self {
        Transfer { host, port }
    }
}

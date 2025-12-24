use qexed_packet::PacketCodec;

#[qexed_packet_macros::packet(id = 0x00)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct ServerInfo {
    pub response:serde_json::Value,
}

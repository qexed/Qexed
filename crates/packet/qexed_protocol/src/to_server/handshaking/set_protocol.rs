use qexed_packet::PacketCodec;

#[qexed_packet_macros::packet(id = 0x00)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct SetProtocol {
    pub protocol_version:qexed_packet::net_types::VarInt,
    pub server_host:String,
    pub server_port:u16,
    pub next_state:qexed_packet::net_types::VarInt,
}

use qexed_packet::PacketCodec;
#[qexed_packet_macros::packet(id = 0x00)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct Disconnect {
    pub reason: qexed_packet::net_types::JsonValue,
}

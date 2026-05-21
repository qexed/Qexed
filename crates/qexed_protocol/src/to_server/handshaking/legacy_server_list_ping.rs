use qexed_packet::PacketCodec;

#[qexed_packet_macros::packet(id = 0xfe)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct LegacyServerListPing {
    pub payload:u8,
}

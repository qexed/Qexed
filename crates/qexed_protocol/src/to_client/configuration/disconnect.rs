use qexed_packet::PacketCodec;

#[qexed_packet_macros::packet(id = 0x2)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct Disconnect {
    pub reason: crate::types::TextComponent,
}

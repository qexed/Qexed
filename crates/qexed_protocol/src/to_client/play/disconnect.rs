use qexed_packet::PacketCodec;

#[qexed_packet_macros::packet(id = 0x1F)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct Disconnect {
    pub reason: crate::types::TextComponent,
}

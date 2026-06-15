use qexed_packet::PacketCodec;
#[qexed_packet_macros::packet(id = 0x72)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct SetTitleText {
    pub text: crate::types::TextComponent,
}

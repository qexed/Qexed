use qexed_packet::PacketCodec;

#[qexed_packet_macros::packet(id = 0x7B)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct SystemChat {
    pub content: crate::types::TextComponent,
    pub overlay: bool,
}

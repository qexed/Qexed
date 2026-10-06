use qexed_packet::PacketCodec;

#[qexed_packet_macros::packet(id = 0x7c)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct SystemChat {
    pub content: crate::types::TextComponent,
    pub overlay: bool,
}

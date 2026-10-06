use qexed_packet::PacketCodec;

#[qexed_packet_macros::packet(id = 0x57)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct ServerData {
    pub motd: crate::types::TextComponent,
    pub icon_bytes: Option<Vec<u8>>,
}

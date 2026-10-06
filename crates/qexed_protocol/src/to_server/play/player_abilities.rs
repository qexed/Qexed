use qexed_packet::PacketCodec;

#[qexed_packet_macros::packet(id = 0x28)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct PlayerAbilities {
    pub flags: u8,
}

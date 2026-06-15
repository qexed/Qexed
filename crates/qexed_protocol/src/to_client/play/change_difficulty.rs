use qexed_packet::PacketCodec;

#[qexed_packet_macros::packet(id = 0x0a)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct ChangeDifficulty {
    pub difficulty: u8,
    pub locked: bool,
}

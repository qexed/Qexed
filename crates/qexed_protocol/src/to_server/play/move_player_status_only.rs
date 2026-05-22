use qexed_packet::PacketCodec;

#[qexed_packet_macros::packet(id = 0x21)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct MovePlayerStatusOnly {
    pub flags: u8,
}

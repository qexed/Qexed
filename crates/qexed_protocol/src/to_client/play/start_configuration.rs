use qexed_packet::PacketCodec;

#[qexed_packet_macros::packet(id = 0x78)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct StartConfiguration {}

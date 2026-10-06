use qexed_packet::PacketCodec;

#[qexed_packet_macros::packet(id = 0x77)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct StartConfiguration {}

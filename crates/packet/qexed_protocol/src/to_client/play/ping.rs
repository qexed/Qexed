use qexed_packet::PacketCodec;
#[qexed_packet_macros::packet(id = 0x36)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct Ping {
    pub id:i32,
}
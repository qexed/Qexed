use qexed_packet::PacketCodec;
#[qexed_packet_macros::packet(id = 0x2d)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct Pong {
    pub id: i32,
}

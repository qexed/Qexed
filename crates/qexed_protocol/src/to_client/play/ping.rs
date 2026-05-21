use qexed_packet::PacketCodec;
#[qexed_packet_macros::packet(id = 0x3d)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct Ping {
    pub id: i32,
}

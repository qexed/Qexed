use qexed_packet::PacketCodec;

#[qexed_packet_macros::packet(id = 0x1e)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct MovePlayerPos {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub flags: u8,
}

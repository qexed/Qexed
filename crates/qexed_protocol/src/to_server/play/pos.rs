use qexed_packet::PacketCodec;

#[qexed_packet_macros::packet(id = 0x1E)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct Pos {
pub x: f64,
pub y: f64,
pub z: f64,
pub on_ground: bool,
}

use qexed_packet::PacketCodec;

#[qexed_packet_macros::packet(id = 0x1F)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct PosRot {
pub on_ground: bool,
pub x: f64,
pub y: f64,
pub z: f64,
pub yaw: i8,
pub pitch: i8,
}

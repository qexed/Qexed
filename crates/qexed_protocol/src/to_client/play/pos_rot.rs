use qexed_packet::{PacketCodec, net_types::VarInt};

#[qexed_packet_macros::packet(id = 0x36)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct PosRot {
pub entity_id: VarInt,
pub x: f64,
pub y: f64,
pub z: f64,
pub yaw: i8,
pub pitch: i8,
pub on_ground: bool,
}

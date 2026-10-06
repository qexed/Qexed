use qexed_packet::{PacketCodec, net_types::VarInt};

#[qexed_packet_macros::packet(id = 0x35)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct Pos {
pub entity_id: VarInt,
pub x: f64,
pub y: f64,
pub z: f64,
}

use qexed_packet::{PacketCodec, net_types::VarInt};

#[qexed_packet_macros::packet(id = 0x38)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct Rot {
pub entity_id: VarInt,
pub yaw: i8,
pub pitch: i8,
pub on_ground: bool,
}

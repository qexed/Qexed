use qexed_packet::{PacketCodec, net_types::{Angle, VarInt}};

#[qexed_packet_macros::packet(id = 0x2E)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct MoveEntityPos {
    pub entity_id:VarInt,
    pub delta_x:i16,
    pub delta_y:i16, 
    pub delta_z:i16,
    pub on_ground:bool,
}
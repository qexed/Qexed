use qexed_packet::{PacketCodec, net_types::{Angle, VarInt}};
#[qexed_packet_macros::packet(id = 0x2F)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct MoveEntityPosRot {
    pub entity_id:VarInt,
    pub delta_x:i16,
    pub delta_y:i16, 
    pub delta_z:i16,
    pub yaw:Angle,
    pub pitch:Angle,
    pub on_ground:bool,
}
use qexed_packet::{PacketCodec, net_types::{Angle, VarInt}};

 #[qexed_packet_macros::packet(id = 0x31)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct MoveEntityRot {
    pub entity_id:VarInt,
    pub yaw:Angle,
    pub pitch:Angle,
    pub on_ground:bool,
}
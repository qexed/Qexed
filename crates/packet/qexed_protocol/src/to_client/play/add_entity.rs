use qexed_packet::{PacketCodec, net_types::{Angle, VarInt}};
#[qexed_packet_macros::packet(id = 0x01)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct AddEntity {
    pub entity_id:VarInt,
    pub entity_uuid:uuid::Uuid,
    pub r#type:VarInt,
    pub x:f64,
    pub y:f64,
    pub z:f64,
    pub pitch:Angle,
    pub yaw:Angle,
    pub head_yaw:Angle,
    pub data:VarInt,
    pub velocity_x:i16,
    pub velocity_y:i16,
    pub velocity_z:i16,
}

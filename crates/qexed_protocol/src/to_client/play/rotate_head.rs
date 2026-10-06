use qexed_packet::{PacketCodec, net_types::*};

#[qexed_packet_macros::packet(id = 0x55)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct RotateHead {
    pub entity_id: VarInt,
    pub y_head_rot: i8,
}

impl RotateHead {
    pub fn new(entity_id: i32, y_head_rot: i8) -> Self {
        Self {
            entity_id: qexed_packet::net_types::VarInt(entity_id),
            y_head_rot,
        }
    }

    /// v4 兼容：角度值自动打包为字节。
    pub fn from_degrees(entity_id: i32, degrees: f32) -> Self {
        Self::new(entity_id, ((degrees * 256.0 / 360.0).floor() as i32 & 0xff) as i8)
    }
}

use qexed_packet::{PacketCodec};

#[qexed_packet_macros::packet(id = 0x49)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct PlayerRotation {
    pub y_rot: f32,
    pub relative_y: bool,
    pub x_rot: f32,
    pub relative_x: bool,
}

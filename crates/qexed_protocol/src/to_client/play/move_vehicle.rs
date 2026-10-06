use qexed_packet::{PacketCodec};

/// net.minecraft.core.PositionAndRotation：Vec3(3xf64) + yRot(f32) + xRot(f32)。
#[qexed_packet_macros::substruct]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct PositionAndRotation {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub y_rot: f32,
    pub x_rot: f32,
}

#[qexed_packet_macros::packet(id = 0x3a)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct MoveVehicle {
    pub moving_to: PositionAndRotation,
}

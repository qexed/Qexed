use qexed_packet::{PacketCodec};

/// `ServerboundMoveVehiclePacket` (play, to_server, id 0x22)。
#[qexed_packet_macros::packet(id = 0x22)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct MoveVehicle {
    pub moving_to: PositionAndRotation,
    pub on_ground: bool,
}

/// `net.minecraft.core.PositionAndRotation`：Vec3（x/y/z 三个 f64）+ yRot/xRot 两个 f32。
#[qexed_packet_macros::substruct]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct PositionAndRotation {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub y_rot: f32,
    pub x_rot: f32,
}

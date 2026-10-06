use qexed_packet::{PacketCodec, net_types::*};

/// net.minecraft.world.entity.Relative 位掩码（普通 i32，非 VarInt）。
pub const RELATIVE_X: i32 = 1 << 0;
pub const RELATIVE_Y: i32 = 1 << 1;
pub const RELATIVE_Z: i32 = 1 << 2;
pub const RELATIVE_Y_ROT: i32 = 1 << 3;
pub const RELATIVE_X_ROT: i32 = 1 << 4;
pub const RELATIVE_DELTA_X: i32 = 1 << 5;
pub const RELATIVE_DELTA_Y: i32 = 1 << 6;
pub const RELATIVE_DELTA_Z: i32 = 1 << 7;
pub const RELATIVE_ROTATE_DELTA: i32 = 1 << 8;

/// net.minecraft.world.entity.PositionMoveRotation：
/// Vec3 position(3xf64) + Vec3 deltaMovement(3xf64) + f32 yRot + f32 xRot。
#[qexed_packet_macros::substruct]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct PositionMoveRotation {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub delta_x: f64,
    pub delta_y: f64,
    pub delta_z: f64,
    pub y_rot: f32,
    pub x_rot: f32,
}

#[qexed_packet_macros::packet(id = 0x49)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct PlayerPosition {
    pub id: VarInt,
    pub change: PositionMoveRotation,
    /// java.util.Set<Relative> 经 SET_STREAM_CODEC 写为普通 i32 位掩码。
    pub relatives: i32,
}

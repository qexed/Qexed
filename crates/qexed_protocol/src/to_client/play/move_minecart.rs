use qexed_packet::{PacketCodec, net_types::*};

/// net.minecraft.world.entity.vehicle.minecart.NewMinecartBehavior$MinecartStep。
#[qexed_packet_macros::substruct]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct MinecartStep {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub movement_x: f64,
    pub movement_y: f64,
    pub movement_z: f64,
    /// ROTATION_BYTE：角度打包为单个字节。
    pub y_rot: i8,
    pub x_rot: i8,
    pub weight: f32,
}

#[qexed_packet_macros::packet(id = 0x37)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct MoveMinecart {
    pub entity_id: VarInt,
    pub lerp_steps: Vec<MinecartStep>,
}

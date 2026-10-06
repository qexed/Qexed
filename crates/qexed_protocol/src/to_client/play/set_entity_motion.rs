use qexed_packet::{PacketCodec, net_types::*};

#[qexed_packet_macros::packet(id = 0x67)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct SetEntityMotion {
    pub id: VarInt,
    // TODO: net.minecraft.world.phys.Vec3 -> 三个 f64 字段占位；26.3 实际使用 Vec3.LP_STREAM_CODEC
    // （LpVec3 变长压缩编码，见 add_entity.rs 的 write_lp_vec3/read_lp_vec3），扁平宏无法表达，需手工 codec
    pub movement_x: f64,
    pub movement_y: f64,
    pub movement_z: f64,
}

use qexed_packet::{PacketCodec, net_types::VarInt};

#[qexed_packet_macros::packet(id = 0x30)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct LevelParticles {
    // TODO: net.minecraft.core.particles.ParticleOptions — 粒子注册表 id（VarInt）+ 按粒子类型分发的选项数据
    // （方块/物品粒子带 VarInt 内容、红石粒子带 RGBA 等简单类型无额外数据）；选项负载暂未建模，先用
    // particle_options 占位。当前编解码只写/读粒子 id。
    pub particle_id: VarInt,
    pub override_limiter: bool,
    pub always_show: bool,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub x_dist: f32,
    pub y_dist: f32,
    pub z_dist: f32,
    pub x_max_speed: f32,
    pub y_max_speed: f32,
    pub z_max_speed: f32,
    pub count: VarInt,
    // TODO: ClientboundLevelParticlesPacket$RandomizationType — VarInt id
    // （0=DEFAULT 1=ALTERNATIVE 2=ALTERNATIVE_WITH_SPEED）
    pub randomization_type: VarInt,
}

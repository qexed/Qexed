use qexed_packet::{PacketCodec, net_types::VarInt};

#[qexed_packet_macros::packet(id = 0x23)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct Explode {
    // TODO: net.minecraft.world.phys.Vec3 — STREAM_CODEC 为 3 个 double，此处展开为三个字段
    pub center_x: f64,
    pub center_y: f64,
    pub center_z: f64,
    pub radius: f32,
    pub block_count: i32,
    // TODO: Optional<Vec3> — writeOptional：布尔存在标记 + 3 个 double，展开为 has_* 标记 + 三个字段
    pub has_player_knockback: bool,
    pub knockback_x: f64,
    pub knockback_y: f64,
    pub knockback_z: f64,
    // TODO: net.minecraft.core.particles.ParticleOptions — 粒子注册表 id + 按类型分发的选项数据，
    // 选项负载暂未建模（简单爆炸粒子无额外数据），当前编解码只写/读粒子 id
    pub explosion_particle_id: VarInt,
    // TODO: Holder<SoundEvent> — 注册表 id，用 VarInt 占位
    pub explosion_sound: VarInt,
    // WeightedList<ExplosionParticleInfo>：列表，每项 = VarInt 权重 + ExplosionParticleInfo
    pub block_particles: Vec<WeightedExplosionParticle>,
    pub play_sound: bool,
}

/// net.minecraft.util.random.Weighted<ExplosionParticleInfo>。
#[derive(Debug, Default, PartialEq, Clone)]
pub struct WeightedExplosionParticle {
    pub weight: VarInt,
    pub value: ExplosionParticleInfo,
}

impl PacketCodec for WeightedExplosionParticle {
    fn serialize(&self, w: &mut qexed_packet::PacketWriter) -> qexed_packet::Result<()> {
        self.weight.serialize(w)?;
        self.value.serialize(w)
    }

    fn deserialize(&mut self, r: &mut qexed_packet::PacketReader) -> qexed_packet::Result<()> {
        self.weight.deserialize(r)?;
        self.value.deserialize(r)
    }
}

/// net.minecraft.core.particles.ExplosionParticleInfo：ParticleOptions + 2 个 float。
#[derive(Debug, Default, PartialEq, Clone)]
pub struct ExplosionParticleInfo {
    // TODO: ParticleOptions — 粒子注册表 id + 按类型分发的选项，选项负载暂未建模
    pub particle_id: VarInt,
    pub scaling: f32,
    pub speed: f32,
}

impl PacketCodec for ExplosionParticleInfo {
    fn serialize(&self, w: &mut qexed_packet::PacketWriter) -> qexed_packet::Result<()> {
        self.particle_id.serialize(w)?;
        self.scaling.serialize(w)?;
        self.speed.serialize(w)
    }

    fn deserialize(&mut self, r: &mut qexed_packet::PacketReader) -> qexed_packet::Result<()> {
        self.particle_id.deserialize(r)?;
        self.scaling.deserialize(r)?;
        self.speed.deserialize(r)
    }
}

use qexed_packet::{PacketCodec, net_types::*};

#[qexed_packet_macros::packet(id = 0x76)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct SoundEntity {
    // TODO: net.minecraft.core.Holder<SoundEvent> -> VarInt 占位（注册表 id 或 0=按名字符串，此处取 id 形式）
    pub sound: VarInt,
    // TODO: net.minecraft.sounds.SoundSource -> VarInt 占位
    pub source: VarInt,
    pub id: VarInt,
    pub volume: f32,
    pub pitch: f32,
    pub seed: i64,
}

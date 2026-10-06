use qexed_packet::{PacketCodec, net_types::*};

#[qexed_packet_macros::packet(id = 0x4E)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct RemoveMobEffect {
    pub entity_id: VarInt,
    // TODO: net.minecraft.core.Holder<net.minecraft.world.effect.MobEffect> 注册表 id，用 VarInt 占位
    pub effect_id: VarInt,
}

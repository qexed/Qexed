use qexed_packet::{PacketCodec, net_types::*};

#[qexed_packet_macros::packet(id = 0x3A)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct OpenBook {
    // TODO: net.minecraft.world.InteractionHand 枚举 -> VarInt（0=MAIN_HAND, 1=OFF_HAND）
    pub hand: VarInt,
}

use qexed_packet::{PacketCodec, net_types::*};

#[qexed_packet_macros::packet(id = 0x7A)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct SwingAnimation {
    pub entity_id: VarInt,
    // TODO: net.minecraft.world.InteractionHand -> VarInt 占位（0=main 1=off）
    pub hand: VarInt,
    // TODO: net.minecraft.world.item.component.SwingAnimation -> VarInt 占位
    pub animation: VarInt,
}

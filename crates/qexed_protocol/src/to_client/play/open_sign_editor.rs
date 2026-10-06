use qexed_packet::{PacketCodec, net_types::*};

#[qexed_packet_macros::packet(id = 0x3d)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct OpenSignEditor {
    pub pos: Position,
    // TODO: net.minecraft.world.level.block.entity.SignTextSlot 枚举 -> VarInt（0=BACK, 1=FRONT）
    pub slot: VarInt,
}

use qexed_packet::{
    PacketCodec,
    net_types::{Position, VarInt},
};

#[qexed_packet_macros::packet(id = 0x7)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct BlockEvent {
    pub pos: Position,
    pub b0: u8,
    pub b1: u8,
    // TODO: net.minecraft.world.level.block.Block — 注册表 id，用 VarInt 占位
    pub block: VarInt,
}

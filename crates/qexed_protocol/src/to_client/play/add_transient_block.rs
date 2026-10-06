use qexed_packet::{PacketCodec, net_types::{Position, VarInt}};

#[qexed_packet_macros::packet(id = 0x24)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct AddTransientBlock {
    pub pos: Position,
    // TODO: BlockState — 方块状态 id（全局状态调色板），用 VarInt 占位
    pub block_state: VarInt,
}

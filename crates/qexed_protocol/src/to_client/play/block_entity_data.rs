use qexed_packet::{
    PacketCodec,
    net_types::{OptionalNbt, Position, VarInt},
};

#[qexed_packet_macros::packet(id = 0x6)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct BlockEntityData {
    pub pos: Position,
    // TODO: net.minecraft.world.level.block.entity.BlockEntityType<?> — 注册表 id，用 VarInt 占位
    pub block_entity_type: VarInt,
    pub tag: OptionalNbt,
}

use qexed_packet::{PacketCodec, net_types::*};

#[qexed_packet_macros::packet(id = 0x7F)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct TeleportEntity {
    pub id: VarInt,
    // TODO: net.minecraft.world.entity.PositionMoveRotation -> 复杂类型暂用 VarInt 占位，待展开
    pub change: VarInt,
    // TODO: java.util.Set<Relative>（Relative.SET_STREAM_CODEC）-> VarInt 位掩码占位，待展开
    pub relatives: VarInt,
    pub on_ground: bool,
}

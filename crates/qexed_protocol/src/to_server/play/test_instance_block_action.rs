use qexed_packet::{PacketCodec, net_types::*};

/// `ServerboundTestInstanceBlockActionPacket` (play, to_server, id 0x41)。
#[qexed_packet_macros::packet(id = 0x41)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct TestInstanceBlockAction {
    pub pos: Position,
    // TODO: ServerboundTestInstanceBlockActionPacket.Action 枚举
    //  （INIT=0, QUERY=1, SET=2, RESET=3, SAVE=4, EXPORT=5, RUN=6）→ VarInt。
    pub action: VarInt,
    // TODO: TestInstanceBlockEntity.Data 复杂记录
    //  （Optional<ResourceKey<GameTestInstance>>, Vec3i, Rotation, bool, Status,
    //   Optional<Component>，需 RegistryFriendlyByteBuf）占位为 VarInt，待展开。
    pub data: VarInt,
}

use qexed_packet::{PacketCodec, net_types::{Position, VarInt}};

#[qexed_packet_macros::packet(id = 0x19)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct DebugBlockValue {
    pub block_pos: Position,
    pub update: DebugSubscriptionUpdate,
}

/// net.minecraft.util.debug.DebugSubscription$Update：注册表 id（VarInt）后跟按订阅类型分发的可选值。
#[derive(Debug, Default, PartialEq, Clone)]
pub struct DebugSubscriptionUpdate {
    // TODO: DebugSubscription 注册表 id（VarInt）；值的编码取决于具体订阅类型，暂用 VarInt 占位
    pub subscription_id: VarInt,
    // TODO: Optional<T> — 值编解码器随订阅类型变化（BlockPos/路径/BrainDump 等），暂用 VarInt 占位
    pub value: Option<VarInt>,
}

impl PacketCodec for DebugSubscriptionUpdate {
    fn serialize(&self, w: &mut qexed_packet::PacketWriter) -> qexed_packet::Result<()> {
        self.subscription_id.serialize(w)?;
        self.value.serialize(w)
    }

    fn deserialize(&mut self, r: &mut qexed_packet::PacketReader) -> qexed_packet::Result<()> {
        self.subscription_id.deserialize(r)?;
        self.value.deserialize(r)
    }
}

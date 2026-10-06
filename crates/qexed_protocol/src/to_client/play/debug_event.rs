use qexed_packet::{PacketCodec, net_types::VarInt};

#[qexed_packet_macros::packet(id = 0x1C)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct DebugEvent {
    pub event: DebugSubscriptionEvent,
}

/// net.minecraft.util.debug.DebugSubscription$Event：注册表 id（VarInt）后跟按订阅类型分发的值。
#[derive(Debug, Default, PartialEq, Clone)]
pub struct DebugSubscriptionEvent {
    // TODO: DebugSubscription 注册表 id（VarInt）；值的编码取决于具体订阅类型，暂用 VarInt 占位
    pub subscription_id: VarInt,
    // TODO: T — 值编解码器随订阅类型变化，暂用 VarInt 占位
    pub value: VarInt,
}

impl PacketCodec for DebugSubscriptionEvent {
    fn serialize(&self, w: &mut qexed_packet::PacketWriter) -> qexed_packet::Result<()> {
        self.subscription_id.serialize(w)?;
        self.value.serialize(w)
    }

    fn deserialize(&mut self, r: &mut qexed_packet::PacketReader) -> qexed_packet::Result<()> {
        self.subscription_id.deserialize(r)?;
        self.value.deserialize(r)
    }
}

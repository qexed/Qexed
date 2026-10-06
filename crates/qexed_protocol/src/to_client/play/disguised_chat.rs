use qexed_packet::{PacketCodec, net_types::{AnyNbt, VarInt}};

#[qexed_packet_macros::packet(id = 0x21)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct DisguisedChat {
    // TODO: net.minecraft.network.chat.Component — 线上格式为 NBT 文本组件，用 AnyNbt 占位
    pub message: AnyNbt,
    pub chat_type: ChatTypeBound,
}

/// net.minecraft.network.chat.ChatType$Bound。
#[derive(Debug, Default, PartialEq, Clone)]
pub struct ChatTypeBound {
    // TODO: Holder<ChatType> — 注册表 id，用 VarInt 占位
    pub chat_type: VarInt,
    // TODO: Component — NBT 文本组件
    pub name: AnyNbt,
    // TODO: Optional<Component> — NBT 文本组件
    pub target_name: Option<AnyNbt>,
}

impl PacketCodec for ChatTypeBound {
    fn serialize(&self, w: &mut qexed_packet::PacketWriter) -> qexed_packet::Result<()> {
        self.chat_type.serialize(w)?;
        self.name.serialize(w)?;
        self.target_name.serialize(w)
    }

    fn deserialize(&mut self, r: &mut qexed_packet::PacketReader) -> qexed_packet::Result<()> {
        self.chat_type.deserialize(r)?;
        self.name.deserialize(r)?;
        self.target_name.deserialize(r)
    }
}

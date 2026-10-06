use qexed_packet::PacketCodec;

#[qexed_packet_macros::packet(id = 0x00)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct LoginDisconnect {
    // Java: net.minecraft.network.chat.Component -> 共享类型 TextComponent (NBT 文本组件)
    pub reason: crate::types::TextComponent,
}

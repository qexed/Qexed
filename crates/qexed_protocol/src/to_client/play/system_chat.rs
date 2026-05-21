use qexed_packet::PacketCodec;
#[qexed_packet_macros::packet(id = 0x72)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct SystemChat {
    pub content: qexed_nbt::Tag,  // 文本组件
    pub overlay: bool, // 是否显示在动作栏而非聊天框
}
use qexed_packet::PacketCodec;

#[qexed_packet_macros::packet(id = 0x9)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct ResourcePackPush {
    pub id: uuid::Uuid,
    pub url: String,
    // Java 端限制 hash 最长 40 字符（ByteBufCodecs.stringUtf8(40)）
    pub hash: String,
    pub required: bool,
    pub prompt: Option<crate::types::TextComponent>,
}

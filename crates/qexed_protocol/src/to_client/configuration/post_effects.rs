use qexed_packet::PacketCodec;

#[qexed_packet_macros::packet(id = 0xA)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct PostEffects {
    // TODO: net.minecraft.resources.Identifier 暂用 String 占位（namespace:path 格式），
    // 线上格式为 VarInt 长度前缀的 UTF-8 字符串（Identifier.STREAM_CODEC = STRING_UTF8）
    pub post_effects: Vec<String>,
}

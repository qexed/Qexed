use qexed_packet::PacketCodec;

#[qexed_packet_macros::packet(id = 0xD)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct UpdateEnabledFeatures {
    // TODO: net.minecraft.resources.Identifier 暂用 String 占位（namespace:path 格式）；
    // Java 侧为 Set<Identifier>，线上格式为 VarInt 数量 + 逐个字符串，与 Vec<String> 一致
    pub features: Vec<String>,
}

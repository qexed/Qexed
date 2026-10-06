use qexed_packet::{PacketCodec, net_types::VarInt};

// Java 原型: Map<ResourceKey<? extends Registry<?>>, TagNetworkSerialization$NetworkPayload>
// 线上格式: VarInt 注册表数量 + 每项 { 注册表标识符字符串, VarInt 标签数量 + 每项 { 标签名, VarInt id 数量 + ids } }
#[qexed_packet_macros::packet(id = 0xE)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct UpdateTags {
    pub tags: Vec<RegistryTags>,
}

#[qexed_packet_macros::substruct]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct RegistryTags {
    // TODO: ResourceKey 暂用 String 占位（注册表标识符，如 "minecraft:block"），
    // 线上格式即标识符字符串（ResourceKey.REGISTRY_STREAM_CODEC -> Identifier.STREAM_CODEC）
    pub registry: String,
    pub tags: Vec<RegistryTag>,
}

#[qexed_packet_macros::substruct]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct RegistryTag {
    // TODO: 标签名暂用 String 占位（如 "minecraft:wool"，实为 Identifier）
    pub name: String,
    // TODO: TagNetworkSerialization$NetworkPayload 每个标签为 IntList（注册表 id 列表），
    // 此处用 Vec<VarInt> 对应；负数位图等特殊优化未建模
    pub entries: Vec<VarInt>,
}

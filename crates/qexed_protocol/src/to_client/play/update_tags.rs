use qexed_packet::{PacketCodec, net_types::*};

#[qexed_packet_macros::packet(id = 0x88)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct UpdateTags {
    pub tags: Vec<RegistryTags>,
}

#[qexed_packet_macros::substruct]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct RegistryTags {
    // TODO: net.minecraft.resources.ResourceKey<? extends Registry<?>> -> String 占位（注册表标识符，如 "minecraft:block"）
    pub registry: String,
    pub tags: Vec<RegistryTag>,
}

#[qexed_packet_macros::substruct]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct RegistryTag {
    // TODO: 标签名暂用 String 占位（如 "minecraft:wool"）
    pub name: String,
    pub entries: Vec<VarInt>,
}

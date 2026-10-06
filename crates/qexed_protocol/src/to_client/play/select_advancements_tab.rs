use qexed_packet::{PacketCodec};

#[qexed_packet_macros::packet(id = 0x57)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct SelectAdvancementsTab {
    // TODO: net.minecraft.resources.Identifier 暂用 String 占位（namespace:path 格式）
    pub tab: String,
}

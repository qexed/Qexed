use qexed_packet::{PacketCodec};

#[qexed_packet_macros::packet(id = 0x52)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct PostEffects {
    // TODO: net.minecraft.resources.Identifier 暂用 String 占位（namespace:path 格式）
    pub post_effects: Vec<String>,
}

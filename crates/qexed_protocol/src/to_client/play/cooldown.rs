use qexed_packet::{PacketCodec, net_types::VarInt};

#[qexed_packet_macros::packet(id = 0x16)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct Cooldown {
    // TODO: net.minecraft.resources.Identifier — 线上格式为字符串（namespace:path），用 String 占位
    pub cooldown_group: String,
    pub duration: VarInt,
}

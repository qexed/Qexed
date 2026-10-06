use qexed_packet::{PacketCodec, net_types::*};

#[qexed_packet_macros::packet(id = 0x78)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct StopSound {
    pub flags: u8,
    // TODO: presence 由 flags 位掩码表达（0x01=source 0x02=name），Java 无额外前缀字节；
    // 扁平宏的 Option 会多写一个 bool 前缀，需手工 codec 修正
    // TODO: net.minecraft.sounds.SoundSource -> VarInt 占位
    pub source: Option<VarInt>,
    // TODO: net.minecraft.resources.Identifier -> String 占位（namespace:path 格式）
    pub name: Option<String>,
}

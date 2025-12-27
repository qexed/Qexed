use qexed_packet::PacketCodec;

use crate::data_type::paletted_container::PalettedContainer;
#[derive(Debug, Default, PartialEq,Clone)]
pub struct ChunkSection {
    // 非空气方块数量
    pub block_count:i16,
    // 方块状态列表(4096)
    pub block_states:PalettedContainer,
    // 生物群系(64)
    pub biomes:PalettedContainer,
}

use qexed_packet::PacketCodec;
#[derive(Debug, Default, PartialEq,Clone)]
pub struct PalettedContainer {
    // 确定编码条目所需的位数。请注意，并非所有数字都适用。
    pub bits_per_entry:u8,
    // pub palette:Palette,

}

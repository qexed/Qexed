use qexed_packet::{PacketCodec, net_types::VarInt};
#[derive(Debug, PartialEq,Clone)]
pub enum PalettedContainer {
    // 单值
    SingleValued(VarInt),// 0
    // 间接
    Indirect(Indirect),
    // 原始
    Direct(Vec<u64>),
    Unknown,
}
impl Default for PalettedContainer {
    fn default() -> Self {
        Self::Unknown
    }
}
#[derive(Debug, PartialEq,Clone)]
pub struct Indirect{
    pub palette:Vec<VarInt>,
    pub data:Vec<u64>,
}
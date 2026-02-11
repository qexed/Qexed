/// 空数据包,处理报错的
#[derive(Debug, Default, PartialEq,Clone)]
pub struct NullPacket {}
impl qexed_packet::Packet for NullPacket {
    const ID: u32=0xfff;
    fn serialize(&self, _w: &mut qexed_packet::PacketWriter) -> anyhow::Result<()> {Ok(())}
    fn deserialize(&mut self, _r: &mut qexed_packet::PacketReader) -> anyhow::Result<()> {Ok(())}
}

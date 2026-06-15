use qexed_packet::{Packet, PacketReader, PacketWriter};

#[derive(Debug, Default, PartialEq, Clone)]
pub struct PlayerLoaded;

impl Packet for PlayerLoaded {
    const ID: i32 = 0x2c;

    fn serialize(&self, _w: &mut PacketWriter) -> anyhow::Result<()> {
        Ok(())
    }

    fn deserialize(&mut self, _r: &mut PacketReader) -> anyhow::Result<()> {
        Ok(())
    }
}

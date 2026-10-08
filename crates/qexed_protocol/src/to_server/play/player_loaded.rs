use qexed_packet::{Packet, PacketReader, PacketWriter};
use qexed_packet::error::PacketError;

#[derive(Debug, Default, PartialEq, Clone)]
pub struct PlayerLoaded;

impl Packet for PlayerLoaded {
    const ID: i32 = 0x2c;

    fn serialize(&self, _w: &mut PacketWriter) -> Result<(), PacketError> {
        Ok(())
    }

    fn deserialize(&mut self, _r: &mut PacketReader) -> Result<(), PacketError> {
        Ok(())
    }
}

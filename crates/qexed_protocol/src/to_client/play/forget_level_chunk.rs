use qexed_packet::{Packet, PacketCodec, PacketReader, PacketWriter};
use qexed_packet::error::PacketError;

#[derive(Debug, Default, PartialEq, Clone)]
pub struct ForgetLevelChunk {
    pub chunk_x: i32,
    pub chunk_z: i32,
}

impl Packet for ForgetLevelChunk {
    const ID: i32 = 0x25;

    fn serialize(&self, w: &mut PacketWriter) -> Result<(), PacketError> {
        chunk_pos(self.chunk_x, self.chunk_z).serialize(w)
    }

    fn deserialize(&mut self, r: &mut PacketReader) -> Result<(), PacketError> {
        let mut packed = i64::default();
        packed.deserialize(r)?;
        self.chunk_x = packed as i32;
        self.chunk_z = (packed >> 32) as i32;
        Ok(())
    }
}

fn chunk_pos(chunk_x: i32, chunk_z: i32) -> i64 {
    ((chunk_z as i64) << 32) | (chunk_x as u32 as i64)
}

#[cfg(test)]
mod tests {
    use qexed_packet::Packet;

    use super::ForgetLevelChunk;

    #[test]
    fn serializes_chunk_pos_as_minecraft_packed_long() {
        let packet = ForgetLevelChunk {
            chunk_x: -1,
            chunk_z: 2,
        };
        let mut buf = bytes::BytesMut::new();
        let mut writer = qexed_packet::PacketWriter::new(&mut buf);
        packet.serialize(&mut writer).unwrap();

        let mut bytes = buf.freeze();
        let mut reader = qexed_packet::PacketReader::new(&mut bytes);
        let mut decoded = ForgetLevelChunk::default();
        decoded.deserialize(&mut reader).unwrap();

        assert_eq!(decoded, packet);
    }
}

use qexed_packet::PacketCodec;

#[qexed_packet_macros::packet(id = 0x25)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct ForgetLevelChunk {
    pub pos: ChunkPos,
}

/// net.minecraft.world.level.ChunkPos：writeChunkPos 写一个 long（x 低 32 位，z 高 32 位）。
#[derive(Debug, Default, PartialEq, Eq, Clone, Copy)]
pub struct ChunkPos {
    pub x: i32,
    pub z: i32,
}

impl PacketCodec for ChunkPos {
    fn serialize(&self, w: &mut qexed_packet::PacketWriter) -> qexed_packet::Result<()> {
        let packed = ((self.z as i64) << 32) | (self.x as u32 as i64);
        packed.serialize(w)
    }

    fn deserialize(&mut self, r: &mut qexed_packet::PacketReader) -> qexed_packet::Result<()> {
        let mut packed: i64 = 0;
        packed.deserialize(r)?;
        self.x = packed as i32;
        self.z = (packed >> 32) as i32;
        Ok(())
    }
}

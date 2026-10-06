use bytes::{Buf as _, BufMut as _};
use qexed_packet::{
    PacketCodec, PacketReader, PacketWriter,
    net_types::{ByteArray, VarInt},
};

#[qexed_packet_macros::packet(id = 0xC)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct ChunksBiomes {
    pub chunk_biome_data: Vec<ChunkBiomeData>,
}

/// ClientboundChunksBiomesPacket$ChunkBiomeData。
#[derive(Debug, Default, PartialEq, Clone)]
pub struct ChunkBiomeData {
    pub pos: ChunkPos,
    pub buffer: ByteArray,
}

impl PacketCodec for ChunkBiomeData {
    fn serialize(&self, w: &mut PacketWriter) -> qexed_packet::Result<()> {
        self.pos.serialize(w)?;
        self.buffer.serialize(w)
    }

    fn deserialize(&mut self, r: &mut PacketReader) -> qexed_packet::Result<()> {
        self.pos.deserialize(r)?;
        self.buffer.deserialize(r)
    }
}

/// net.minecraft.world.level.ChunkPos：writeChunkPos 写一个 long（x 低 32 位，z 高 32 位）。
#[derive(Debug, Default, PartialEq, Eq, Clone, Copy)]
pub struct ChunkPos {
    pub x: i32,
    pub z: i32,
}

impl PacketCodec for ChunkPos {
    fn serialize(&self, w: &mut PacketWriter) -> qexed_packet::Result<()> {
        let packed = ((self.z as i64) << 32) | (self.x as u32 as i64);
        packed.serialize(w)
    }

    fn deserialize(&mut self, r: &mut PacketReader) -> qexed_packet::Result<()> {
        let mut packed: i64 = 0;
        packed.deserialize(r)?;
        self.x = packed as i32;
        self.z = (packed >> 32) as i32;
        Ok(())
    }
}

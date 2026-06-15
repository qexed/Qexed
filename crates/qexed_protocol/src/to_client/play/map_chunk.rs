use qexed_packet::net_types::{Bitset, VarInt};
use qexed_packet::{PacketCodec, PacketReader, PacketWriter};

pub const LIGHT_ARRAY_BYTES: usize = 2048;

#[derive(Debug, PartialEq, Clone)]
pub struct LightArray(pub [u8; LIGHT_ARRAY_BYTES]);

impl Default for LightArray {
    fn default() -> Self {
        Self([0; LIGHT_ARRAY_BYTES])
    }
}

impl PacketCodec for LightArray {
    fn serialize(&self, w: &mut PacketWriter) -> anyhow::Result<()> {
        VarInt(LIGHT_ARRAY_BYTES as i32).serialize(w)?;
        for value in &self.0 {
            value.serialize(w)?;
        }
        Ok(())
    }

    fn deserialize(&mut self, r: &mut PacketReader) -> anyhow::Result<()> {
        let mut len = VarInt::default();
        len.deserialize(r)?;
        if len.0 != LIGHT_ARRAY_BYTES as i32 {
            anyhow::bail!(
                "invalid light array length: got {}, expected {}",
                len.0,
                LIGHT_ARRAY_BYTES
            );
        }
        for value in &mut self.0 {
            value.deserialize(r)?;
        }
        Ok(())
    }
}

#[qexed_packet_macros::packet(id = 0x2d)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct MapChunk {
    pub chunk_x: i32,
    pub chunk_z: i32,
    pub data: Chunk,
    pub light: Light,
}

#[qexed_packet_macros::substruct]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct Chunk {
    pub heightmaps: Vec<Heightmaps>,
    pub data: Vec<u8>,
    pub block_entities: Vec<BlockEntities>,
}
#[qexed_packet_macros::substruct]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct Heightmaps {
    pub type_id: VarInt,
    pub data: Vec<u64>,
}
#[qexed_packet_macros::substruct]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct BlockEntities {
    pub xz: u8,
    pub y: u16,
    pub entity_type: VarInt,
    pub nbt: qexed_packet::net_types::OptionalNbt,
}
#[qexed_packet_macros::substruct]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct Light {
    pub sky_light_mask: Bitset,
    pub block_light_mask: Bitset,
    pub empty_sky_light_mask: Bitset,
    pub empty_block_light_mask: Bitset,
    pub sky_light_arrays: Vec<LightArray>,
    pub block_light_arrays: Vec<LightArray>,
}

use qexed_packet::net_types::{Bitset, VarInt};
use qexed_packet::PacketCodec;
#[qexed_packet_macros::packet(id = 0x27)]
#[derive(Debug, Default, PartialEq,Clone)]
pub struct MapChunk {
    pub chunk_x: i32,
    pub chunk_z: i32,
    pub data:Chunk,
    pub light:Light,
}

#[qexed_packet_macros::substruct]
#[derive(Debug, Default, PartialEq,Clone)]
pub struct Chunk {
    pub heightmaps: Vec<Heightmaps>,
    pub data: Vec<u8>,
    pub block_entities:Vec<BlockEntities>
}
#[qexed_packet_macros::substruct]
#[derive(Debug, Default, PartialEq,Clone)]
pub struct Heightmaps {
    pub type_id: VarInt,
    pub data: Vec<u64>,
}
#[qexed_packet_macros::substruct]
#[derive(Debug, Default, PartialEq,Clone)]
pub struct BlockEntities {
    pub xz: u8,
    pub y: u16,
    pub entity_type: VarInt,
    pub nbt: Vec<qexed_nbt::Tag>,
}
#[qexed_packet_macros::substruct]
#[derive(Debug, Default, PartialEq,Clone)]
pub struct Light {
    pub sky_light_mask: Bitset,
    pub block_light_mask: Bitset,
    pub empty_sky_light_mask: Bitset,
    pub empty_block_light_mask: Bitset,
    pub sky_light_arrays: Vec<Vec<u8>>,
    pub block_light_arrays: Vec<Vec<u8>>,
}
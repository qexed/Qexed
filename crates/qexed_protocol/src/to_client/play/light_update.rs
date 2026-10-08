use qexed_packet::{
    PacketCodec,
    net_types::{Bitset, VarInt},
};

use super::map_chunk::LightArray;

#[qexed_packet_macros::packet(id = 0x30)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct LightUpdate {
    pub chunk_x: VarInt,
    pub chunk_z: VarInt,
    pub light: LightUpdateData,
}

#[qexed_packet_macros::substruct]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct LightUpdateData {
    pub sky_light_mask: Bitset,
    pub block_light_mask: Bitset,
    pub empty_sky_light_mask: Bitset,
    pub empty_block_light_mask: Bitset,
    pub sky_light_arrays: Vec<LightArray>,
    pub block_light_arrays: Vec<LightArray>,
}

use qexed_packet::{
    PacketCodec,
    net_types::{VarInt, VarLong},
};

#[qexed_packet_macros::packet(id = 0x2c)]
#[derive(Debug, PartialEq, Clone)]
pub struct InitializeBorder {
    pub center_x: f64,
    pub center_z: f64,
    pub old_size: f64,
    pub new_size: f64,
    pub lerp_time: VarLong,
    pub absolute_max_size: VarInt,
    pub warning_blocks: VarInt,
    pub warning_time: VarInt,
}

impl Default for InitializeBorder {
    fn default() -> Self {
        Self {
            center_x: 0.0,
            center_z: 0.0,
            old_size: 5.9999968e7,
            new_size: 5.9999968e7,
            lerp_time: VarLong(0),
            absolute_max_size: VarInt(29_999_984),
            warning_blocks: VarInt(5),
            warning_time: VarInt(15),
        }
    }
}

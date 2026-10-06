use qexed_packet::{PacketCodec, net_types::VarInt};

#[qexed_packet_macros::packet(id = 0x68)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct SetExperience {
    pub experience_progress: f32,
    pub experience_level: VarInt,
    pub total_experience: VarInt,
}

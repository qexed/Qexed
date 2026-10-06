use qexed_packet::{PacketCodec, net_types::*};

#[qexed_packet_macros::packet(id = 0x5b)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct SetBorderLerpSize {
    pub old_size: f64,
    pub new_size: f64,
    pub lerp_time: VarLong,
}

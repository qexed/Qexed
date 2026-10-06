use qexed_packet::{PacketCodec};

#[qexed_packet_macros::packet(id = 0x5c)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct SetBorderSize {
    pub size: f64,
}

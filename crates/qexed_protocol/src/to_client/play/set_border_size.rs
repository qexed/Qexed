use qexed_packet::{PacketCodec};

#[qexed_packet_macros::packet(id = 0x5B)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct SetBorderSize {
    pub size: f64,
}

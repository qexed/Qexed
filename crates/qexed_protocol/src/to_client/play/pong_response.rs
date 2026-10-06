use qexed_packet::{PacketCodec};

#[qexed_packet_macros::packet(id = 0x3f)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct PongResponse {
    pub time: i64,
}

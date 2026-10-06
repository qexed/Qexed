use qexed_packet::{PacketCodec, net_types::RestBuffer};

#[qexed_packet_macros::packet(id = 0x18)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct CustomPayload {
    pub channel: String,
    pub data: RestBuffer,
}

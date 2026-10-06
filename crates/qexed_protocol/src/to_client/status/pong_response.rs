use qexed_packet::{PacketCodec, net_types::{VarInt, VarLong}};

#[qexed_packet_macros::packet(id = 0x1)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct PongResponse {
    pub time: i64,
}

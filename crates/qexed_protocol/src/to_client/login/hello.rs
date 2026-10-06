use qexed_packet::{PacketCodec, net_types::ByteArray};

#[qexed_packet_macros::packet(id = 0x01)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct Hello {
    pub server_id: String,
    pub public_key: ByteArray,
    pub challenge: ByteArray,
    pub should_authenticate: bool,
}

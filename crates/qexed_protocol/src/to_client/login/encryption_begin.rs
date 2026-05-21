use qexed_packet::PacketCodec;
#[qexed_packet_macros::packet(id = 0x01)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct EncryptionBegin {
    pub server_id: String,
    pub public_key: qexed_packet::net_types::ByteArray,
    pub verify_token: qexed_packet::net_types::ByteArray,
    pub should_authenticate: bool,
}

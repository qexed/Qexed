use qexed_packet::PacketCodec;
#[qexed_packet_macros::packet(id = 0x01)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct EncryptionBegin {
    pub shared_secret: qexed_packet::net_types::ByteArray,
    pub verify_token: qexed_packet::net_types::ByteArray,
}

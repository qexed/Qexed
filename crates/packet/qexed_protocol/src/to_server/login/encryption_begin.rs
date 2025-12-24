use qexed_packet::PacketCodec;
#[qexed_packet_macros::packet(id = 0x01)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct EncryptionBegin {
    pub shared_secret:Vec<u8>,
    pub verify_token:Vec<u8>,
}
use qexed_packet::PacketCodec;
#[qexed_packet_macros::packet(id = 0x01)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct EncryptionBegin {
    pub server_id:String,
    pub public_key:Vec<u8>,
    pub verify_token:Vec<u8>,
    pub should_authenticate:bool,
}
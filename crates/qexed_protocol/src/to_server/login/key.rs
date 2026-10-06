use qexed_packet::{PacketCodec, net_types::ByteArray};

#[qexed_packet_macros::packet(id = 0x01)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct Key {
    pub keybytes: ByteArray,
    pub encrypted_challenge: ByteArray,
}

use qexed_packet::PacketCodec;
#[qexed_packet_macros::packet(id = 0x08)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct ChatMessage {
    pub message:String,
    pub timestamp:i64,
    pub salt:i64,
    pub offset:qexed_packet::net_types::VarInt,
    pub acknowledged:[u8; 3],
    pub checksum:i8,
}
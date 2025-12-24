use qexed_packet::PacketCodec;
#[qexed_packet_macros::packet(id = 0x04)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct LoginPluginRequest {
    pub message_id:qexed_packet::net_types::VarInt,
    pub channel:String,
    pub data:qexed_packet::net_types::RestBuffer,
}

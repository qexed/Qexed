use qexed_packet::PacketCodec;
#[qexed_packet_macros::packet(id = 0x02)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct LoginPluginResponse {
    pub message_id:qexed_packet::net_types::VarInt,
    pub data:Option<qexed_packet::net_types::RestBuffer>,
}

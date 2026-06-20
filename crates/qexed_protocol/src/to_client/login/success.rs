use qexed_packet::PacketCodec;
#[qexed_packet_macros::packet(id = 0x02)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct Success {
    pub game_profile: qexed_packet::net_types::GameProfile,
    pub session_id: uuid::Uuid,
}

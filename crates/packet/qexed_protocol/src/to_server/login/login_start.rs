use qexed_packet::PacketCodec;
#[qexed_packet_macros::packet(id = 0x00)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct LoginStart {
    pub username:String,
    pub player_uuid:uuid::Uuid,
}
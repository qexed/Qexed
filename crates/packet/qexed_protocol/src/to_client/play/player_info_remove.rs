
use qexed_packet::PacketCodec;
#[qexed_packet_macros::packet(id = 0x3E)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct PlayerInfoRemove {
    pub uuids:Vec<uuid::Uuid>
}

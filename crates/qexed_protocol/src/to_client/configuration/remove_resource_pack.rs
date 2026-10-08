use qexed_packet::PacketCodec;

#[qexed_packet_macros::packet(id = 0x08)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct RemoveResourcePack {
    pub id: Option<uuid::Uuid>,
}

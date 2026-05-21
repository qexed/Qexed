use qexed_packet::PacketCodec;

#[qexed_packet_macros::packet(id = 0x09)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct AddResourcePack {
    pub id: uuid::Uuid,
    pub url: String,
    pub hash: String,
    pub required: bool,
    pub prompt: Option<crate::types::TextComponent>,
}

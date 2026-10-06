use qexed_packet::{PacketCodec};
use crate::types::*;

#[qexed_packet_macros::packet(id = 0x51)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct ResourcePackPush {
    pub id: uuid::Uuid,
    pub url: String,
    pub hash: String,
    pub required: bool,
    pub prompt: Option<crate::types::TextComponent>,
}

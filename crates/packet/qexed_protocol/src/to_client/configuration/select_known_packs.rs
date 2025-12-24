use crate::types::KnownPacks;
use qexed_packet::PacketCodec;
#[qexed_packet_macros::packet(id = 0x0e)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct SelectKnownPacks {
    pub known_packs: Vec<KnownPacks>,
}
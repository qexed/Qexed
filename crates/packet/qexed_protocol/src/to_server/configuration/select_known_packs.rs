use qexed_packet::PacketCodec;

use crate::types::KnownPacks;
#[qexed_packet_macros::packet(id = 0x07)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct SelectKnownPacks {
    pub entries:Vec<KnownPacks>,
}

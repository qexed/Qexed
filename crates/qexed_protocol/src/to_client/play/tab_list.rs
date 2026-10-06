use qexed_packet::{PacketCodec};
use crate::types::*;

#[qexed_packet_macros::packet(id = 0x7C)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct TabList {
    pub header: TextComponent,
    pub footer: TextComponent,
}

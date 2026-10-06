use qexed_packet::{PacketCodec};
use crate::types::*;

#[qexed_packet_macros::packet(id = 0x61)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct SetCursorItem {
    pub contents: Slot,
}

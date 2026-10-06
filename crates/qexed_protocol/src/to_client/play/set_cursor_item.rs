use qexed_packet::{PacketCodec};
use crate::types::*;

#[qexed_packet_macros::packet(id = 0x62)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct SetCursorItem {
    pub contents: Slot,
}

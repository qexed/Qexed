use crate::types::Slot;
use qexed_packet::{PacketCodec, net_types::VarInt};

#[qexed_packet_macros::packet(id = 0x6e)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct SetPlayerInventory {
    pub slot: VarInt,
    pub contents: Slot,
}

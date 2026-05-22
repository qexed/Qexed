use crate::types::Slot;
use qexed_packet::PacketCodec;

#[qexed_packet_macros::packet(id = 0x38)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct SetCreativeModeSlot {
    pub slot_num: i16,
    pub item_stack: Slot,
}

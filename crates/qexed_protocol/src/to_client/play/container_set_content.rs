use crate::types::Slot;
use qexed_packet::{PacketCodec, net_types::VarInt};
#[qexed_packet_macros::packet(id = 0x11)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct ContainerSetContent {
    pub window_id: VarInt,
    pub state_id: VarInt,
    pub slot_data: Vec<Slot>,
    pub carried_item: Slot,
}

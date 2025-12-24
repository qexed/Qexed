use qexed_packet::{PacketCodec, net_types::VarInt};
use crate::types::Slot;
#[qexed_packet_macros::packet(id = 0x12)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct ContainerSetContent {
    pub window_id:VarInt,
    pub state_id: VarInt,
    pub slot_data:Vec<Slot>,
    pub carried_item:Slot,
}
use qexed_packet::{PacketCodec, net_types::VarInt};
use crate::types::Slot;
#[qexed_packet_macros::packet(id = 0x14)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct ContainerSetContent {
    pub window_id:VarInt,
    pub state_id: VarInt,
    pub slot:i16,
    pub slot_data:Slot,
}
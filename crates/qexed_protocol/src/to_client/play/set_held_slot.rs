use qexed_packet::{PacketCodec, net_types::VarInt};

#[qexed_packet_macros::packet(id = 0x69)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct SetHeldSlot {
    pub slot: VarInt,
}

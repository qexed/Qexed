use qexed_packet::{PacketCodec, net_types::VarInt};

#[qexed_packet_macros::packet(id = 0x6A)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct SetHeldSlot {
    pub slot: VarInt,
}

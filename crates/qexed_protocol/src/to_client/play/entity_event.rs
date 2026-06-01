use qexed_packet::{PacketCodec, net_types::VarInt};

#[qexed_packet_macros::packet(id = 0x22)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct EntityEvent {
    pub entity_id: VarInt,
    pub event_id: u8,
}

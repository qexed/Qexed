use qexed_packet::{PacketCodec, net_types::*};

#[qexed_packet_macros::packet(id = 0x4D)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct RemoveEntities {
    pub entity_ids: Vec<VarInt>,
}

impl RemoveEntities {
    pub fn one(entity_id: i32) -> Self {
        Self { entity_ids: vec![qexed_packet::net_types::VarInt(entity_id)] }
    }
}

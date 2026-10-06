use qexed_packet::{PacketCodec};

#[qexed_packet_macros::packet(id = 0x46)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct PlayerInfoRemove {
    pub profile_ids: Vec<uuid::Uuid>,
}

impl PlayerInfoRemove {
    pub fn one(profile_id: uuid::Uuid) -> Self {
        Self { profile_ids: vec![profile_id] }
    }
}

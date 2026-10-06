use qexed_packet::{PacketCodec, net_types::VarInt};

#[qexed_packet_macros::packet(id = 0x86)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct RemoveMobEffect {
    pub entity_id: VarInt,
    pub effect_id: VarInt,
}

#[qexed_packet_macros::packet(id = 0x84)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct UpdateMobEffect {
    pub entity_id: VarInt,
    pub effect_id: VarInt,
    pub amplifier: VarInt,
    pub duration: VarInt,
    pub flags: u8,
}

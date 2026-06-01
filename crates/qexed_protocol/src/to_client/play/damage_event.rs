use qexed_packet::{PacketCodec, net_types::VarInt};

#[qexed_packet_macros::packet(id = 0x19)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct DamageEvent {
    pub entity_id: VarInt,
    pub source_type_id: VarInt,
    pub source_cause_id: VarInt,
    pub source_direct_id: VarInt,
    pub has_source_position: bool,
    pub source_position: Option<DamageSourcePosition>,
}

#[qexed_packet_macros::substruct]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct DamageSourcePosition {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}
